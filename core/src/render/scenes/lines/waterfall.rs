//! The waterfall system: the recent spectrum laid out as a landscape and seen
//! through the shared perspective camera (ADR-0258).
//!
//! # One frame
//!
//! `update` reduces the analysis frame's band array to `elements` levels, shapes
//! and eases each one through the same helpers the `spectrum` readout uses
//! (`downsample`, `shape_and_ease`), and hands the eased levels to the
//! `Landscape`. That keeps a ring of past rows and pushes one every
//! `row_period` seconds of injected `dt`, so the rows are spaced in **time**,
//! not in frames, and the landscape scrolls at the same speed at any display
//! rate (ADR-0019).
//!
//! `render` lays the rows out on the ground plane: row `k` of the ring sits at
//! depth `(k + frac) * row_spacing` behind the front edge, where `frac` is the
//! fraction of a period since the last push, so every row slides back
//! continuously and a push moves nothing. Each row is a polyline of
//! `elements - 1` segments across the frequency axis, `height * level` tall,
//! clipped and culled through the camera block and drawn by this scene's own
//! `seg3d` renderer.
//!
//! # The front edge is live
//!
//! The nearest row is the **live** eased level, drawn at depth 0 every frame,
//! not the last pushed row: the front edge answers the music on the frame it
//! arrives, however long `row_period` is. A pushed row is born on top of it and
//! fades in over its first period as it slides away (`alpha = frac`), so a push
//! adds no line at once and the front edge never pops. The oldest row fades out
//! over its last period the same way.
//!
//! # Colour
//!
//! The palette runs along the frequency axis, as on `spectrum` (ADR-0059):
//! `hue` places the lowest band and `hue_spread` says how far the palette
//! travels to the highest. Age dims a row by `fade`: the farthest row keeps
//! `1 - fade` of the front edge's light.
//!
//! # Solid
//!
//! When the camera block's `solid` is on, every row segment also lays a
//! **skirt** under itself: a black band down to the ground plane, drawn through
//! the same `seg3d` pipeline and sorted with its row by where the row stands,
//! so a near row hides what lies behind it — hidden-line removal for a
//! ridgeline (ADR-0263). A skirted row costs twice the segments, and the rows
//! drawn are held to that cost each solid frame, announced as the load-time
//! row clamp is.

// Hot-path panic-denial pragma (Plan 0002 Phase 2; `render/` scan set).
// `update` and `render` run every displayed frame.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

use super::renderer::{LineRenderer, Segment3dInstance, joined_chord};
use super::spectrum::{CURVE_MAX, CURVE_MIN, downsample, shape_and_ease};
use super::{CapOverflow, ColorRamp, GeneratorConfig, OverflowContext};
use crate::dsp::AnalysisFrame;
use crate::preset::Easing;
use crate::render::camera::{self, CameraParams};
use crate::render::palette::Palette;
use crate::render::scenes::common::{PaletteParams, PanParams};
use crate::render::scenes::{FALLBACK_DT, ParamGroup, ParamKind, ParamSpec, Scene, default_of};

/// The fewest rows a landscape may ask for: the live front row and one pushed
/// row behind it.
pub const MIN_ROWS: u32 = 2;

/// The most rows a preset may ask for at load, before the tier's
/// `seg3d_segments` holds it further.
pub const MAX_ROWS: u32 = 1024;

/// The rows a preset that names none draws.
pub const DEFAULT_ROWS: u32 = 64;

/// The bands across a row a preset that names none draws.
pub const DEFAULT_ELEMENTS: usize = 48;

/// The shortest `row_period` a preset may ask for, in seconds. Below a frame
/// at 100 Hz every frame would push more than one copy of the same row.
pub const MIN_ROW_PERIOD: f32 = 0.01;

/// The longest `row_period` a preset may ask for, in seconds.
pub const MAX_ROW_PERIOD: f32 = 2.0;

/// The `row_period` a preset that names none pushes at, in seconds.
pub const DEFAULT_ROW_PERIOD: f32 = 0.04;

/// How far short of a whole `row_period` the accumulator may fall and still
/// push, in seconds.
///
/// A period that is a whole number of frames at some rate — 1/12 s is five
/// frames at 60 Hz and twelve at 144 Hz — is reached by a sum of rounded `dt`s
/// that can land a few nanoseconds short. Without this slack that push slips a
/// whole frame later at one rate and not at the other.
const PUSH_SLACK: f64 = 1e-6;

/// The world z of the front edge. The camera orbits the world origin, so the
/// landscape starts one unit in front of what the camera looks at and recedes
/// past it.
const FRONT_Z: f32 = 1.0;

/// The world half-width of a row: the frequency axis spans `x` in `[-1, 1]`.
const HALF_WIDTH: f32 = 1.0;

/// The world y of the ground the rows stand on: a band at level `0` lies on
/// it, and a solid row's skirts reach down to it (ADR-0263).
const GROUND_Y: f32 = 0.0;

/// The stroke profile every row is drawn with: a solid core over the inner half
/// and a ramp across the outer (ADR-0124), as on the plexus.
const SOFTNESS: f32 = 0.5;

/// The `[waterfall]` table, validated (ADR-0180 rule 1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WaterfallConfig {
    /// Bands across a row, `2..=`[`SPECTRUM_BINS`](crate::dsp::SPECTRUM_BINS).
    pub elements: usize,
    /// Rows drawn, the live front row included, in
    /// [`MIN_ROWS`]`..=`[`MAX_ROWS`]; the tier may hold it lower.
    pub rows: u32,
    /// Seconds between two pushed rows, in
    /// [`MIN_ROW_PERIOD`]`..=`[`MAX_ROW_PERIOD`].
    pub row_period: f32,
    /// Per-band easing, in the `[smoothing]` vocabulary (ADR-0035).
    pub easing: Easing,
}

impl Default for WaterfallConfig {
    fn default() -> Self {
        Self {
            elements: DEFAULT_ELEMENTS,
            rows: DEFAULT_ROWS,
            row_period: DEFAULT_ROW_PERIOD,
            easing: Easing::INSTANT,
        }
    }
}

const DEFAULT_HEIGHT: f32 = default_of(PARAMS, "height");
const DEFAULT_ROW_SPACING: f32 = default_of(PARAMS, "row_spacing");
const DEFAULT_CURVE: f32 = default_of(PARAMS, "curve");
const DEFAULT_FADE: f32 = default_of(PARAMS, "fade");
const DEFAULT_LINE_WIDTH: f32 = default_of(PARAMS, "line_width");
const DEFAULT_GLOW: f32 = default_of(PARAMS, "glow");
const DEFAULT_HUE: f32 = default_of(PARAMS, "hue");
const DEFAULT_HUE_SPREAD: f32 = default_of(PARAMS, "hue_spread");
const DEFAULT_BRIGHTNESS: f32 = default_of(PARAMS, "brightness");
const DEFAULT_PITCH: f32 = default_of(PARAMS, "pitch");
const DEFAULT_ZOOM: f32 = default_of(PARAMS, "zoom");

/// Parameter vocabulary — see [`fragment_field::PARAMS`](crate::render::scenes::fragment_field::PARAMS).
/// **Keep in sync with `set_param` below.**
pub const PARAMS: &[ParamSpec] = &[
    ParamSpec {
        name: "height",
        default: 0.6,
        range: Some([0.0, 2.0]),
        doc: "How tall a full band stands above the ground, in world units; a row is 2 across.",
        kind: ParamKind::Modal,
        group: ParamGroup::Shape,
        main: true,
    },
    ParamSpec {
        name: "row_spacing",
        default: 0.06,
        range: Some([0.01, 0.3]),
        doc: "How far behind each row the next one lies, in world units; with rows, how deep \
              the landscape reaches.",
        kind: ParamKind::Modal,
        group: ParamGroup::Shape,
        main: true,
    },
    ParamSpec {
        name: "curve",
        default: 1.0,
        range: Some([CURVE_MIN, CURVE_MAX]),
        doc: "Exponent on each band's level: 1 is linear, below 1 lifts quiet detail, above 1 \
              pushes it down.",
        kind: ParamKind::Modal,
        group: ParamGroup::Shape,
        main: false,
    },
    ParamSpec {
        name: "fade",
        default: 0.8,
        range: Some([0.0, 1.0]),
        doc: "How much light the farthest row has lost against the front edge; 0 keeps every \
              row as bright as the newest.",
        kind: ParamKind::Modal,
        group: ParamGroup::Light,
        main: true,
    },
    ParamSpec {
        name: "line_width",
        default: 1.5,
        range: Some([0.5, 8.0]),
        doc: "Line width in pixels at the focal plane; nearer rows are wider and farther ones \
              thinner.",
        kind: ParamKind::Modal,
        group: ParamGroup::Shape,
        main: true,
    },
    crate::render::scenes::lines::GLOW,
    crate::render::scenes::common::brightness(1.0),
    crate::render::scenes::common::hue(0.55),
    ParamSpec {
        name: "hue_spread",
        default: 0.6,
        range: Some([0.0, 1.0]),
        doc: "How far along the palette the colour travels from the lowest band to the highest.",
        kind: ParamKind::Modal,
        group: ParamGroup::Colour,
        main: false,
    },
    crate::render::scenes::common::SATURATION,
    crate::render::scenes::common::PALETTE_MIX,
    crate::render::scenes::common::PALETTE_STEPS,
    camera::YAW,
    PITCH,
    camera::DISTANCE,
    camera::FOV,
    camera::FOCUS,
    camera::APERTURE,
    camera::FOG,
    camera::SOLID,
    crate::render::scenes::common::zoom(1.0),
    crate::render::scenes::common::PAN_X,
    crate::render::scenes::common::PAN_Y,
];

/// The camera's `pitch`, raised so an unbound preset looks down the landscape
/// rather than along it. Range, kind and wording stay the shared block's.
const PITCH: ParamSpec = ParamSpec {
    default: 0.35,
    ..camera::PITCH
};

/// The rows of a landscape, before they are laid out: the live levels, a ring
/// of past rows, and the time since the last push. Arithmetic only — no device
/// in it — so the push cadence is testable on the CPU.
#[derive(Debug, Clone, Default)]
pub(crate) struct Landscape {
    /// This frame's downsampled levels, before shaping and easing.
    raw: Vec<f32>,
    /// The eased levels: the live front row, and the per-band envelope state.
    live: Vec<f32>,
    /// `capacity` past rows of `elements` levels each, row-major.
    ring: Vec<f32>,
    /// Bands across a row.
    elements: usize,
    /// How many past rows the ring holds: the drawn rows less the live one.
    capacity: usize,
    /// The ring slot of the newest pushed row.
    head: usize,
    /// How many slots hold a pushed row.
    len: usize,
    /// Seconds since the last push, accumulated in `f64` so a long run does not
    /// drift the cadence.
    since_push: f64,
    /// Seconds between two pushes.
    period: f32,
    /// Per-band easing.
    easing: Easing,
}

impl Landscape {
    /// Size every buffer for `rows` drawn rows of `elements` bands and empty
    /// them. Off the hot path: the only place this type allocates.
    pub(crate) fn resize(&mut self, elements: usize, rows: usize, period: f32, easing: Easing) {
        self.elements = elements;
        self.capacity = rows.saturating_sub(1);
        self.period = period;
        self.easing = easing;
        self.raw.clear();
        self.raw.resize(elements, 0.0);
        self.live.clear();
        self.live.resize(elements, 0.0);
        self.ring.clear();
        self.ring.resize(elements * self.capacity, 0.0);
        self.head = 0;
        self.len = 0;
        self.since_push = 0.0;
    }

    /// Advance one frame of `dt` seconds on the band array `spectrum`: ease the
    /// live row toward it, then push the live row once per whole `row_period`
    /// the frame completed. Allocation-free.
    pub(crate) fn step(&mut self, spectrum: &[f32], curve: f32, dt: f32) {
        downsample(spectrum, &mut self.raw);
        for (held, &raw) in self.live.iter_mut().zip(&self.raw) {
            *held = shape_and_ease(*held, raw, curve, self.easing, dt);
        }
        let period = f64::from(self.period);
        if period <= 0.0 || period.is_nan() || self.capacity == 0 {
            return;
        }
        self.since_push += f64::from(dt);
        // A frame longer than the whole ring pushes the ring full and no more:
        // the rows it would push past that are the same live row again.
        let mut pushes = 0;
        while self.since_push + PUSH_SLACK >= period && pushes < self.capacity {
            self.push();
            self.since_push -= period;
            pushes += 1;
        }
        if self.since_push >= period {
            self.since_push %= period;
        }
        self.since_push = self.since_push.max(0.0);
    }

    /// Copy the live row into the ring as its newest row, over the oldest when
    /// the ring is full.
    fn push(&mut self) {
        let n = self.elements;
        self.head = (self.head + 1) % self.capacity.max(1);
        let start = self.head * n;
        if let (Some(dst), Some(src)) = (self.ring.get_mut(start..start + n), self.live.get(..n)) {
            dst.copy_from_slice(src);
        }
        self.len = (self.len + 1).min(self.capacity);
    }

    /// The fraction of a period since the last push, in `[0, 1)`: how far every
    /// pushed row has slid back from its whole-row depth.
    pub(crate) fn frac(&self) -> f32 {
        if self.period > 0.0 {
            ((self.since_push / f64::from(self.period)) as f32).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    /// The live front row.
    pub(crate) fn live(&self) -> &[f32] {
        &self.live
    }

    /// Pushed row `k`, `0` the newest, or `None` past the rows pushed so far.
    pub(crate) fn row(&self, k: usize) -> Option<&[f32]> {
        if k >= self.len || self.capacity == 0 {
            return None;
        }
        let slot = (self.head + self.capacity - k) % self.capacity;
        let n = self.elements;
        self.ring.get(slot * n..slot * n + n)
    }

    /// How many rows have been pushed and are still held.
    pub(crate) fn len(&self) -> usize {
        self.len
    }

    /// How many past rows the ring holds when full.
    pub(crate) fn capacity(&self) -> usize {
        self.capacity
    }

    /// Bands across a row.
    pub(crate) fn elements(&self) -> usize {
        self.elements
    }
}

/// The waterfall scene: the landscape's rows and the `seg3d` renderer they are
/// drawn through.
pub struct WaterfallScene {
    /// This scene's own renderer (ADR-0258): the `seg3d` pipeline and an
    /// instance buffer of the tier's `seg3d_segments`.
    lines: LineRenderer,
    /// The tier's `seg3d_segments`: the most row segments one frame draws.
    seg3d_cap: usize,
    /// The tier's cap on the circle of confusion, in pixels.
    max_coc: f32,
    landscape: Landscape,
    /// Reused instance buffer, preallocated to `seg3d_cap`.
    instances: Vec<Segment3dInstance>,
    /// The blur clamp, when `aperture` passes the tier's cap this frame.
    clamp: Option<CapOverflow>,
    palette: Palette,
    dt: f32,
    /// The render target's size in pixels, handed in every frame.
    target: (u32, u32),

    height: f32,
    row_spacing: f32,
    curve: f32,
    fade: f32,
    line_width: f32,
    glow: f32,
    hue_spread: f32,
    zoom: f32,
    colour: PaletteParams,
    pan: PanParams,
    camera: CameraParams,
}

impl WaterfallScene {
    /// Build the scene with a `seg3d` buffer of `seg3d_cap` segments and blur
    /// held to `max_coc` pixels — the tier's caps. The ring is sized by
    /// `configure`, which the renderer runs on every preset switch.
    pub(crate) fn new(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        seg3d_cap: usize,
        max_coc: f32,
    ) -> Self {
        let mut scene = Self {
            lines: LineRenderer::new_3d(device, surface_format, seg3d_cap, "waterfall"),
            seg3d_cap,
            max_coc,
            landscape: Landscape::default(),
            instances: Vec::with_capacity(seg3d_cap),
            clamp: None,
            palette: Palette::default_spectrum(),
            dt: FALLBACK_DT,
            target: (1, 1),
            height: DEFAULT_HEIGHT,
            row_spacing: DEFAULT_ROW_SPACING,
            curve: DEFAULT_CURVE,
            fade: DEFAULT_FADE,
            line_width: DEFAULT_LINE_WIDTH,
            glow: DEFAULT_GLOW,
            hue_spread: DEFAULT_HUE_SPREAD,
            zoom: DEFAULT_ZOOM,
            colour: PaletteParams::new(DEFAULT_HUE, DEFAULT_BRIGHTNESS),
            pan: PanParams::default(),
            camera: CameraParams::default(),
        };
        scene.camera.pitch = DEFAULT_PITCH;
        // The default table fits every tier's buffer, so there is no clamp to
        // report before a preset is configured.
        let _ = scene.resize(&WaterfallConfig::default());
        scene
    }

    /// Size and empty the ring for `config`, with the rows held to what the
    /// `seg3d` buffer can draw, and return that clamp when it bit. Off the hot
    /// path: preset load only.
    fn resize(&mut self, config: &WaterfallConfig) -> Option<CapOverflow> {
        let (rows, overflow) = rows_clamp(config.rows, config.elements, self.seg3d_cap, false);
        self.landscape
            .resize(config.elements, rows, config.row_period, config.easing);
        overflow
    }

    /// The colour ramp this frame's palette knobs describe.
    fn ramp(&self) -> ColorRamp {
        ColorRamp {
            hue: self.colour.hue,
            hue_spread: self.hue_spread,
            palette_mix: self.colour.mix,
            palette_steps: self.colour.steps,
            saturation: self.colour.saturation,
            brightness: self.colour.brightness,
        }
    }
}

/// `rows` held to the most whole rows of `elements` bands a buffer of `cap`
/// segments draws — a row is `elements - 1` segments, and twice that when
/// `skirted`, since a solid row lays a skirt under each segment (ADR-0263) —
/// and the overflow to announce when the cap bit (ADR-0045), its `dropped`
/// counted in segments.
pub(crate) fn rows_clamp(
    rows: u32,
    elements: usize,
    cap: usize,
    skirted: bool,
) -> (usize, Option<CapOverflow>) {
    let per_row = elements.saturating_sub(1).max(1) * if skirted { 2 } else { 1 };
    let fits = cap / per_row;
    let asked = rows as usize;
    if asked <= fits {
        return (asked, None);
    }
    (
        fits,
        Some(CapOverflow {
            dropped: (asked - fits) * per_row,
            context: OverflowContext::Rows(rows, u32::try_from(per_row).unwrap_or(u32::MAX)),
            cap,
        }),
    )
}

impl Scene for WaterfallScene {
    fn name(&self) -> &'static str {
        "waterfall"
    }

    fn advance(&mut self, dt: f32) {
        self.dt = dt;
    }

    fn set_target_size(&mut self, width: u32, height: u32) {
        self.target = (width, height);
    }

    fn set_palette(&mut self, palette: &Palette) {
        self.palette = palette.clone();
    }

    fn configure(&mut self, cfg: &GeneratorConfig) -> Option<CapOverflow> {
        let GeneratorConfig::Waterfall(config) = cfg else {
            return None;
        };
        // A switch starts the incoming preset from an empty ring, with its rows
        // held to the tier and the clamp returned for the renderer to announce
        // with the preset.
        let overflow = self.resize(config);
        // The outgoing preset's per-frame clamp is not this one's to report.
        self.clamp = None;
        overflow
    }

    fn mirror_overflow(&self) -> Option<&CapOverflow> {
        self.clamp.as_ref()
    }

    fn reset_params(&mut self) {
        self.height = DEFAULT_HEIGHT;
        self.row_spacing = DEFAULT_ROW_SPACING;
        self.curve = DEFAULT_CURVE;
        self.fade = DEFAULT_FADE;
        self.line_width = DEFAULT_LINE_WIDTH;
        self.glow = DEFAULT_GLOW;
        self.hue_spread = DEFAULT_HUE_SPREAD;
        self.zoom = DEFAULT_ZOOM;
        self.colour.reset();
        self.pan.reset();
        self.camera.reset();
        // The shared block resets to its own pitch; this system rests on its own.
        self.camera.pitch = DEFAULT_PITCH;
    }

    fn set_param(&mut self, name: &str, value: f32) {
        // The shared param blocks first, this scene's own names after
        // (`scenes::common`, `render::camera`).
        if self.colour.set(name, value) || self.pan.set(name, value) || self.camera.set(name, value)
        {
            return;
        }
        match name {
            "height" => self.height = value,
            "row_spacing" => self.row_spacing = value,
            "curve" => self.curve = value,
            "fade" => self.fade = value,
            "line_width" => self.line_width = value,
            "glow" => self.glow = value,
            "hue_spread" => self.hue_spread = value,
            "zoom" => self.zoom = value,
            _ => {}
        }
    }

    fn update(&mut self, frame: &AnalysisFrame) {
        self.landscape.step(&frame.spectrum, self.curve, self.dt);
    }

    fn render(
        &mut self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        aspect: f32,
    ) {
        let geometry = RowGeometry::new(
            self.height,
            self.row_spacing,
            self.fade,
            self.landscape.capacity() + 1,
        );
        // The aspect is the render target's, handed in here (ADR-0037).
        let frame = self.camera.frame(
            aspect,
            self.zoom,
            [self.pan.x, self.pan.y],
            self.target,
            geometry.radius(),
            self.max_coc,
        );
        // A solid row costs its skirts too, so the rows the ring holds are held
        // again to what the buffer draws at twice the cost, nearest first, and
        // that clamp is announced ahead of the blur's (ADR-0263).
        let held = self.landscape.capacity() + 1;
        let (row_budget, row_clamp) = if frame.solid {
            rows_clamp(
                u32::try_from(held).unwrap_or(u32::MAX),
                self.landscape.elements(),
                self.seg3d_cap,
                true,
            )
        } else {
            (held, None)
        };
        self.clamp = row_clamp.or(frame.blur);
        let width = if self.line_width.is_finite() {
            self.line_width.max(0.0)
        } else {
            DEFAULT_LINE_WIDTH
        };
        let ramp = self.ramp();
        let frac = self.landscape.frac();
        let full = self.landscape.len() == self.landscape.capacity();

        let mut instances = std::mem::take(&mut self.instances);
        instances.clear();
        // The live front row, then every pushed row from the newest back.
        let rows = std::iter::once((0.0, 1.0, self.landscape.live())).chain(
            (0..self.landscape.len()).filter_map(|k| {
                let row = self.landscape.row(k)?;
                // A row is born on the live one and fades in over its first
                // period, and the oldest of a full ring fades out over its last.
                let mut alpha = 1.0;
                if k == 0 {
                    alpha *= frac;
                }
                if full && k + 1 == self.landscape.capacity() {
                    alpha *= 1.0 - frac;
                }
                Some((k as f32 + frac, alpha, row))
            }),
        );
        for (depth, alpha, row) in rows.take(row_budget) {
            if alpha <= 0.0 {
                continue;
            }
            let light = geometry.light(depth);
            let z = geometry.z(depth);
            let n = row.len();
            let span = n.saturating_sub(1).max(1) as f32;
            // A row is one open polyline (ADR-0263): each segment joins its
            // neighbours across the row, and the two outer bands are free.
            let point = |j: usize| row.get(j).map(|&l| [geometry.x(j, n), geometry.y(l), z]);
            let foot = |p: [f32; 3]| [p[0], GROUND_Y, p[2]];
            for (i, pair) in row.windows(2).enumerate() {
                let (Some(&l0), Some(&l1)) = (pair.first(), pair.get(1)) else {
                    continue;
                };
                let pa = [geometry.x(i, n), geometry.y(l0), z];
                let pb = [geometry.x(i + 1, n), geometry.y(l1), z];
                let before = i.checked_sub(1).and_then(point);
                let Some([prev, a, b, next]) =
                    joined_chord(&frame.view, before, pa, pb, point(i + 2))
                else {
                    continue;
                };
                let line_in = !frame.view.outside(a, b, frame.margin);
                // A skirt reaches the ground, so it can be on screen while
                // its line is above the frame; a foot behind the near plane
                // is kept rather than projected.
                let skirt_in = frame.solid && {
                    let (fa, fb) = (foot(a), foot(b));
                    let in_front = frame.view.depth(fa).min(frame.view.depth(fb)) >= camera::NEAR;
                    line_in || !in_front || !frame.view.outside(fa, fb, frame.margin)
                };
                if !line_in && !skirt_in {
                    continue;
                }
                if instances.len() + usize::from(line_in) + usize::from(skirt_in) > self.seg3d_cap {
                    break;
                }
                if skirt_in {
                    instances.push(Segment3dInstance {
                        a,
                        b,
                        color: [0.0; 3],
                        width,
                        alpha,
                        prev: a,
                        next: b,
                        skirt: 1.0,
                    });
                }
                if line_in {
                    let [r, g, bl] = ramp.at(&self.palette, (i as f32 + 0.5) / span);
                    instances.push(Segment3dInstance {
                        a,
                        b,
                        color: [r * light, g * light, bl * light],
                        width,
                        alpha,
                        prev,
                        next,
                        skirt: 0.0,
                    });
                }
            }
        }
        let glow = if self.glow.is_finite() {
            self.glow.max(0.0)
        } else {
            DEFAULT_GLOW
        };
        // The rows stand on the ground plane: a solid frame orders them by
        // their feet and fills each skirt down to it.
        self.lines.draw_3d_terrain(
            queue, encoder, view, &frame, glow, SOFTNESS, GROUND_Y, &instances,
        );
        self.instances = instances;
    }
}

/// Where a landscape's rows lie in the world, for one frame's bound params.
#[derive(Debug, Clone, Copy)]
pub(crate) struct RowGeometry {
    height: f32,
    spacing: f32,
    fade: f32,
    /// The deepest a drawn row reaches, in rows: the drawn count less one.
    deepest: f32,
}

impl RowGeometry {
    /// The geometry of `rows` drawn rows under the bound `height`,
    /// `row_spacing` and `fade`, each made safe here once.
    pub(crate) fn new(height: f32, spacing: f32, fade: f32, rows: usize) -> Self {
        let finite = |v: f32, fallback: f32| if v.is_finite() { v } else { fallback };
        Self {
            height: finite(height, DEFAULT_HEIGHT),
            spacing: finite(spacing, DEFAULT_ROW_SPACING).max(0.0),
            fade: finite(fade, DEFAULT_FADE).clamp(0.0, 1.0),
            deepest: rows.saturating_sub(1).max(1) as f32,
        }
    }

    /// The world x of band `i` of a row of `n`.
    pub(crate) fn x(&self, i: usize, n: usize) -> f32 {
        let span = n.saturating_sub(1).max(1) as f32;
        -HALF_WIDTH + 2.0 * HALF_WIDTH * i as f32 / span
    }

    /// The world y of a band at `level`.
    pub(crate) fn y(&self, level: f32) -> f32 {
        self.height * level
    }

    /// The world z of a row `depth` rows behind the front edge.
    pub(crate) fn z(&self, depth: f32) -> f32 {
        FRONT_Z - depth * self.spacing
    }

    /// The share of the front edge's light a row `depth` rows back keeps:
    /// `1` at the front, `1 - fade` at the deepest row.
    pub(crate) fn light(&self, depth: f32) -> f32 {
        (1.0 - self.fade * (depth / self.deepest).clamp(0.0, 1.0)).max(0.0)
    }

    /// The radius about the orbit target that holds the whole landscape — what
    /// `focus` is normalized across.
    pub(crate) fn radius(&self) -> f32 {
        let far = (FRONT_Z - self.deepest * self.spacing).abs().max(FRONT_Z);
        (HALF_WIDTH * HALF_WIDTH + far * far + self.height * self.height).sqrt()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::indexing_slicing, clippy::panic)]

    use super::*;
    use crate::dsp::SPECTRUM_BINS;

    /// A band array that changes over time: band `b` at window `w` reads a
    /// value no other `(b, w)` pair repeats.
    fn spectrum_at(window: u64) -> [f32; SPECTRUM_BINS] {
        let mut out = [0.0; SPECTRUM_BINS];
        for (b, band) in out.iter_mut().enumerate() {
            let phase = window as f32 * 0.37 + b as f32 * 0.11;
            *band = 0.5 + 0.45 * phase.sin();
        }
        out
    }

    /// Run `frames` frames at `rate` Hz. The band array is held over windows of
    /// 1/12 s, the shortest span both 60 Hz and 144 Hz frames tile exactly, and
    /// a frame reads the window its own interval lies in — so both rates ease
    /// toward the same value over the same stretch of time.
    fn run(rate: u64, frames: u64, period: f32, easing: Easing) -> Landscape {
        let mut landscape = Landscape::default();
        landscape.resize(16, 8, period, easing);
        let dt = 1.0 / rate as f32;
        for n in 1..=frames {
            let window = (n - 1) * 12 / rate;
            landscape.step(&spectrum_at(window), 1.0, dt);
        }
        landscape
    }

    /// **The rows are spaced in time, not in frames** (ADR-0019): two seconds
    /// of the same input at 60 Hz and at 144 Hz leave the same rows in the
    /// ring, at the same depth offset.
    ///
    /// The period is 5/12 s, a whole number of frames at both rates, so both
    /// push at the same instants and sample the same eased level; and two
    /// seconds is not a whole number of periods, so the offset compared is not
    /// the trivial zero.
    #[test]
    fn the_same_input_at_60_and_144_hz_leaves_the_same_rows() {
        let period = 5.0 / 12.0;
        let easing = Easing::symmetric(0.05);
        let a = run(60, 120, period, easing);
        let b = run(144, 288, period, easing);

        assert_eq!(a.len(), 4, "two seconds at 5/12 s a row pushes four rows");
        assert_eq!(
            a.len(),
            b.len(),
            "both rates pushed the same number of rows"
        );
        for k in 0..a.len() {
            let (ra, rb) = (a.row(k).unwrap_or(&[]), b.row(k).unwrap_or(&[]));
            assert_eq!(ra.len(), 16);
            for (i, (x, y)) in ra.iter().zip(rb).enumerate() {
                assert!(
                    (x - y).abs() < 1e-4,
                    "row {k} band {i}: {x} at 60 Hz against {y} at 144 Hz"
                );
            }
        }
        // Non-vacuity: the rows hold different levels, so a ring that pushed
        // one row four times would fail above.
        let (r0, r1) = (a.row(0).unwrap_or(&[]), a.row(1).unwrap_or(&[]));
        assert!(r0.iter().zip(r1).any(|(x, y)| (x - y).abs() > 1e-2));

        let row_spacing = DEFAULT_ROW_SPACING;
        let (fa, fb) = (a.frac() * row_spacing, b.frac() * row_spacing);
        assert!(
            (a.frac() - 0.8).abs() < 1e-3,
            "2 s is 4.8 periods, so the rows sit 0.8 of a row back, got {}",
            a.frac()
        );
        assert!(
            (fa - fb).abs() <= row_spacing * 1e-3,
            "depth offset {fa} at 60 Hz against {fb} at 144 Hz"
        );
    }

    /// A full ring keeps its newest rows: the oldest is overwritten, and the
    /// rows read back newest first.
    #[test]
    fn a_full_ring_drops_its_oldest_row() {
        let mut landscape = Landscape::default();
        landscape.resize(4, 3, 0.1, Easing::INSTANT);
        assert_eq!(landscape.capacity(), 2);
        for level in [0.1f32, 0.2, 0.3] {
            let spectrum = [level; SPECTRUM_BINS];
            landscape.step(&spectrum, 1.0, 0.1);
        }
        assert_eq!(landscape.len(), 2);
        assert!((landscape.row(0).unwrap_or(&[])[0] - 0.3).abs() < 1e-6);
        assert!((landscape.row(1).unwrap_or(&[])[0] - 0.2).abs() < 1e-6);
        assert!(landscape.row(2).is_none());
    }

    /// A frame much longer than the ring pushes it full and no further.
    #[test]
    fn a_long_frame_pushes_at_most_the_ring() {
        let mut landscape = Landscape::default();
        landscape.resize(4, 5, 0.05, Easing::INSTANT);
        landscape.step(&[0.5; SPECTRUM_BINS], 1.0, 10.0);
        assert_eq!(landscape.len(), 4);
        assert!(landscape.frac() < 1.0);
    }

    /// Rows past what the buffer draws are held to it, and the clamp is
    /// announced with what was asked, in the buffer's own unit.
    #[test]
    fn rows_are_held_to_whole_rows_of_the_buffer() {
        assert_eq!(
            rows_clamp(126, 64, 8000, false),
            (126, None),
            "exactly at the cap"
        );
        let (rows, overflow) = rows_clamp(200, 64, 8000, false);
        assert_eq!(rows, 126, "8000 segments hold 126 rows of 63");
        let overflow = overflow.unwrap_or_else(|| panic!("a clamp that bit is announced"));
        assert_eq!(overflow.context, OverflowContext::Rows(200, 63));
        assert_eq!(overflow.cap, 8000);
        assert_eq!(overflow.dropped, 74 * 63);
        let text = overflow.to_string();
        assert!(
            text.contains("rows 200") && text.contains("keeps 126 rows"),
            "{text}"
        );
    }

    /// **A solid row costs its skirts** (ADR-0263): the same rows that fit a
    /// buffer as lines alone are held to half as many once each segment
    /// carries a skirt, and the clamp says so in the same words, counting
    /// a row at its skirted cost.
    #[test]
    fn a_skirted_row_costs_twice_its_segments() {
        assert_eq!(
            rows_clamp(63, 64, 8000, true),
            (63, None),
            "63 skirted rows of 126 fit"
        );
        let (rows, overflow) = rows_clamp(126, 64, 8000, true);
        assert_eq!(rows, 63, "8000 segments hold 63 skirted rows of 63 bands");
        let overflow = overflow.unwrap_or_else(|| panic!("a clamp that bit is announced"));
        assert_eq!(overflow.context, OverflowContext::Rows(126, 126));
        assert_eq!(overflow.dropped, 63 * 126);
        let text = overflow.to_string();
        assert!(text.contains("keeps 63 rows"), "{text}");
    }

    /// **The same seed and analysis frames give the same ring after 600
    /// frames**: two landscapes stepped through one varying sequence hold
    /// bit-identical rows and offsets.
    #[test]
    fn the_same_frames_give_the_same_ring_after_600_frames() {
        let run = || {
            let mut landscape = Landscape::default();
            landscape.resize(32, 40, 0.04, Easing::symmetric(0.08));
            for n in 0..600u64 {
                landscape.step(&spectrum_at(n), 0.8, 1.0 / 60.0);
            }
            landscape
        };
        let (a, b) = (run(), run());
        assert_eq!(a.len(), 39, "600 frames at 0.04 s a row fill a 39-row ring");
        assert_eq!(a.frac().to_bits(), b.frac().to_bits());
        for k in 0..a.len() {
            let (ra, rb) = (a.row(k).unwrap_or(&[]), b.row(k).unwrap_or(&[]));
            assert!(!ra.is_empty());
            assert!(
                ra.iter().zip(rb).all(|(x, y)| x.to_bits() == y.to_bits()),
                "row {k} differs between two runs of the same frames"
            );
        }
        assert!(
            a.live()
                .iter()
                .zip(b.live())
                .all(|(x, y)| x.to_bits() == y.to_bits()),
            "the live row differs"
        );
    }

    /// The light runs from the front edge's to `1 - fade` at the deepest row.
    #[test]
    fn fade_dims_the_deepest_row_by_its_value() {
        let g = RowGeometry::new(0.6, 0.05, 0.8, 11);
        assert!((g.light(0.0) - 1.0).abs() < 1e-6);
        assert!((g.light(10.0) - 0.2).abs() < 1e-6);
        assert!((g.light(5.0) - 0.6).abs() < 1e-6);
        assert!(
            g.z(10.0) < g.z(0.0),
            "deeper rows lie farther from the camera"
        );
    }
}
