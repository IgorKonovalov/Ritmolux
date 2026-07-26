//! Spectrum scene (Plan 0034 / ADR-0036): the analysis frame's log-spaced band
//! array, drawn as N elements.
//!
//! This is a **fourth consumer of the existing line idiom**, not a fifth render
//! idiom: N bars, an N-point polyline and a radial ring of N spokes are all
//! segment lists, and they go out through the same shared
//! [`LineRenderer`](super::LineRenderer) the three other line scenes draw
//! through (ADR-0007). Nothing new is uploaded, no new pipeline is built, and
//! the `Scene` trait is untouched — `update` already receives the whole
//! [`AnalysisFrame`], bands included.
//!
//! The per-frame work is a chain of small pure steps — [`downsample`], the
//! per-element ease, then one of the three [`SpectrumLayout`] builders — all
//! free functions over preallocated buffers. They are separate from the scene so
//! the claims that matter (low elements track low frequencies; the 64 → N
//! reduction loses nothing) are testable without a GPU.
//!
//! **The composite vocabulary this scene honors**, since a silent no-op is the
//! failure mode the shared-surface work exists to avoid:
//!
//! - `zoom` / `pan_x` / `pan_y` — the shared view transform (ADR-0018), applied
//!   by the renderer exactly as for every other line scene.
//! - `mirror_order` / `mirror_reflect` — the geometry mirror (Plan 0018 Phase
//!   4). It replicates real geometry *before* rasterization and costs this scene
//!   nothing, so refusing it would be a no-op the author could not see. On the
//!   radial ring it is nearly the identity (the ring is already rotationally
//!   symmetric, the same near-no-op the Hankin star has); on bars and polyline
//!   it is genuinely transformative, because those figures are not centred.
//! - `[palette]` / `[palette_b]` / `palette_mix` / `hue` / `hue_spread` /
//!   `saturation` — the colour surface (ADR-0021), sampled on the CPU. This is
//!   the **first line scene to honor the palette**; the others still colour from
//!   the built-in cosine. It matters here because colouring elements along their
//!   own axis is what turns a frequency readout into a look. The default
//!   `spectrum` palette is that same cosine, so an author who sets no `[palette]`
//!   sees the engine's usual colour language.
//! - `thickness` / `brightness` / `scale` / `base` — ordinary stroke styling.
//!
//! `radius` is **layout-specific**: it is the ring's inner radius and has no
//! meaning for bars or the polyline. That is stated in `presets/README.md` and
//! `docs/presets.md` rather than left for an author to discover.

// Hot-path panic-denial pragma (Plan 0002 Phase 2, extended to scenes by Plan
// 0003 Phase 0). `update`/`render` run every displayed frame.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

use std::cell::RefCell;
use std::rc::Rc;

use super::super::Scene;
use super::renderer::{LineRenderer, SegmentInstance};
use super::{
    CapOverflow, GeneratorConfig, MAX_SEGMENTS, MirrorSpec, OverflowContext, ViewTransform,
    replicate_mirror,
};
use crate::dsp::AnalysisFrame;
use crate::preset::Easing;
use crate::render::palette::{Palette, desaturate};

/// Maps `thickness` to an NDC-y half-width — the same scale the other line
/// scenes use, so a `thickness` that reads well on the rose reads the same here.
const WIDTH_SCALE: f32 = 0.003;

/// Largest element count a `[spectrum]` table may ask for — the band count
/// itself, because above it the 64 → N reduction stops being a partition of the
/// array. The loader validates against this, and the render layer sizes its
/// per-element scratch to it (Plan 0034 Phase 4), so the two cannot disagree.
pub const MAX_ELEMENTS: usize = crate::dsp::SPECTRUM_BINS;

/// World-space half-width the readout spans. The renderer divides x by the
/// target aspect, so this is a **world** extent, not a screen one — the scene
/// never sees an aspect and so cannot take one from the wrong place (ADR-0037).
const SPAN_X: f32 = 1.0;

/// World-space y the bars and the polyline rest on.
const BASELINE_Y: f32 = -0.85;

// Parameter defaults — a legible, calm readout when a preset binds nothing.
const DEFAULT_THICKNESS: f32 = 6.0;
const DEFAULT_HUE: f32 = 0.55;
const DEFAULT_HUE_SPREAD: f32 = 0.0;
const DEFAULT_SATURATION: f32 = 1.0;
const DEFAULT_PALETTE_MIX: f32 = 0.0;
const DEFAULT_BRIGHTNESS: f32 = 1.0;
const DEFAULT_SCALE: f32 = 1.2;
/// Minimum element length, in world units. Non-zero on purpose: a spectrum
/// readout at rest is a comb, not an empty frame, so the figure stays on screen
/// (and legible) through a silence instead of vanishing.
const DEFAULT_BASE: f32 = 0.06;
/// Inner radius of the radial ring (ignored by the other two layouts).
const DEFAULT_RADIUS: f32 = 0.35;
const DEFAULT_ROTATION: f32 = 0.0;
// Shared view transform (ADR-0018): identity by default.
const DEFAULT_ZOOM: f32 = 1.0;
const DEFAULT_PAN: f32 = 0.0;
// Geometry mirror (Plan 0018 Phase 4): identity by default.
const DEFAULT_MIRROR_ORDER: f32 = 1.0;
const DEFAULT_MIRROR_REFLECT: f32 = 0.0;

/// Parameter vocabulary — see [`fragment_field::PARAMS`](crate::render::scenes::fragment_field::PARAMS).
/// **Keep in sync with `set_param` below.**
pub const PARAMS: &[&str] = &[
    "base",
    "scale",
    "radius",
    "rotation",
    "thickness",
    "hue",
    "hue_spread",
    "saturation",
    "palette_mix",
    "brightness",
    "zoom",
    "pan_x",
    "pan_y",
    "mirror_order",
    "mirror_reflect",
];

/// Which figure the elements form. Selected once at preset load through the
/// `[spectrum]` table; an unknown name is a surfaced load error (ADR-0007).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SpectrumLayout {
    /// Upright bars standing on a common baseline — the classic readout.
    #[default]
    Bars,
    /// A single continuous line through one point per element: the same data as
    /// a contour rather than a comb.
    Polyline,
    /// Spokes radiating outward from a ring, one per element, with the frequency
    /// axis wrapped around the circle.
    RadialRing,
}

impl SpectrumLayout {
    /// The accepted `[spectrum] layout` names, in the order the error message
    /// lists them. The single source for both parsing and the message.
    pub const NAMES: [&'static str; 3] = ["bars", "polyline", "radial_ring"];

    /// Parse a `[spectrum] layout` name, or `None` if unknown.
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "bars" => SpectrumLayout::Bars,
            "polyline" => SpectrumLayout::Polyline,
            "radial_ring" => SpectrumLayout::RadialRing,
            _ => return None,
        })
    }
}

/// Where the figure sits — the two scalars that belong to the **whole** readout
/// rather than to an element, so they stay off the per-element arrays.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Placement {
    /// Inner radius — [`SpectrumLayout::RadialRing`] only.
    pub radius: f32,
    /// Whole-figure rotation in radians, about the world origin.
    pub rotation: f32,
}

/// The parameters this scene accepts as a **per-element series** (Plan 0034
/// Phase 4) — the ones whose effect is genuinely per element. Everything else in
/// [`PARAMS`] describes the whole figure (`radius`, `rotation`, the view
/// transform, the mirror, `hue_spread`, `palette_mix`, `saturation`), so a
/// series aimed at one of those degrades to its `index = 0` value, which is what
/// the trait default does.
///
/// **Index order matches the `SERIES_*` constants below** — the two are read
/// together by `set_param_series` and `update`.
const SERIES_PARAMS: [&str; 5] = ["base", "scale", "thickness", "brightness", "hue"];
const SERIES_BASE: usize = 0;
const SERIES_SCALE: usize = 1;
const SERIES_THICKNESS: usize = 2;
const SERIES_BRIGHTNESS: usize = 3;
const SERIES_HUE: usize = 4;

/// Reduce the engine's band array to `levels.len()` elements by **averaging each
/// element's own contiguous slice of bands**.
///
/// Element `i` covers `[i * bands / n, (i + 1) * bands / n)`. That is a genuine
/// partition — contiguous, non-overlapping, and complete — so no band is dropped
/// or double-counted at any element count up to the band count (which is why the
/// loader caps the count there). It is also deterministic: integer arithmetic on
/// lengths, no clock and no rounding mode to disagree about.
pub(crate) fn downsample(spectrum: &[f32], levels: &mut [f32]) {
    let bands = spectrum.len();
    let n = levels.len();
    if bands == 0 || n == 0 {
        levels.fill(0.0);
        return;
    }
    for (i, level) in levels.iter_mut().enumerate() {
        let lo = i * bands / n;
        // `hi` is the next element's `lo`, which makes the ranges abut exactly.
        // The `max` only bites when n > bands, where a strict partition is not
        // available; the loader caps the count so that never happens.
        let hi = (((i + 1) * bands / n).min(bands)).max(lo + 1);
        let slice = spectrum.get(lo..hi).unwrap_or(&[]);
        *level = if slice.is_empty() {
            0.0
        } else {
            slice.iter().sum::<f32>() / slice.len() as f32
        };
    }
}

/// The world-space length an element reaches, `base + scale * level`, floored at
/// zero so a degenerate level can never invert the element.
pub(crate) fn element_length(level: f32, base: f32, scale: f32) -> f32 {
    (base + scale * level).max(0.0)
}

/// Build the segment list for `layout` into `out` (cleared first).
/// Allocation-free into a preallocated buffer; the per-frame half of the scene.
///
/// `lengths`, `widths` and `colors` are read **positionally, per element**, which
/// is what lets a per-element binding vary any of them across the figure (Plan
/// 0034 Phase 4). A short list falls back to a sane constant rather than
/// panicking — which cannot happen, since all three are sized together at load,
/// but keeps the hot path total.
pub(crate) fn build(
    layout: SpectrumLayout,
    lengths: &[f32],
    widths: &[f32],
    colors: &[[f32; 3]],
    place: Placement,
    out: &mut Vec<SegmentInstance>,
) {
    out.clear();
    if lengths.is_empty() {
        return;
    }
    let (sin, cos) = place.rotation.sin_cos();
    // The whole-figure rotation is applied here, at the point where world-space
    // endpoints are emitted, so it composes with every layout identically.
    let turn = |p: [f32; 2]| -> [f32; 2] { [p[0] * cos - p[1] * sin, p[0] * sin + p[1] * cos] };
    let color_of = |i: usize| colors.get(i).copied().unwrap_or([1.0, 1.0, 1.0]);
    let width_of = |i: usize| widths.get(i).copied().unwrap_or(0.01);

    match layout {
        SpectrumLayout::Bars => {
            let step = 2.0 * SPAN_X / lengths.len() as f32;
            for (i, &length) in lengths.iter().enumerate() {
                let x = -SPAN_X + step * (i as f32 + 0.5);
                out.push(SegmentInstance {
                    a: turn([x, BASELINE_Y]),
                    b: turn([x, BASELINE_Y + length]),
                    color: color_of(i),
                    width: width_of(i),
                });
            }
        }
        SpectrumLayout::Polyline => {
            // One point per element, spanning edge to edge, joined by n-1
            // segments. A single element has no segment to draw, which is why
            // the loader's minimum count is 2.
            let span = lengths.len().saturating_sub(1);
            if span == 0 {
                return;
            }
            let step = 2.0 * SPAN_X / span as f32;
            let point = |i: usize, length: f32| -> [f32; 2] {
                turn([-SPAN_X + step * i as f32, BASELINE_Y + length])
            };
            let mut prev = point(0, lengths.first().copied().unwrap_or(0.0));
            for (i, &length) in lengths.iter().enumerate().skip(1) {
                let next = point(i, length);
                out.push(SegmentInstance {
                    a: prev,
                    b: next,
                    color: color_of(i),
                    width: width_of(i),
                });
                prev = next;
            }
        }
        SpectrumLayout::RadialRing => {
            // The frequency axis wrapped around the circle: element 0 points
            // along +x and the rest follow counter-clockwise, each spoke running
            // outward from the ring.
            let n = lengths.len() as f32;
            let inner = place.radius.max(0.0);
            for (i, &length) in lengths.iter().enumerate() {
                let angle = place.rotation + std::f32::consts::TAU * i as f32 / n;
                let (s, c) = angle.sin_cos();
                let outer = inner + length;
                out.push(SegmentInstance {
                    a: [c * inner, s * inner],
                    b: [c * outer, s * outer],
                    color: color_of(i),
                    width: width_of(i),
                });
            }
        }
    }
}

/// The spectrum readout: N elements driven by the analysis frame's band array,
/// drawn through the shared line renderer.
pub struct SpectrumScene {
    /// The single line renderer, shared with the other line scenes (ADR-0007:
    /// "one line renderer"). Only the active scene draws in a frame.
    renderer: Rc<RefCell<LineRenderer>>,
    /// The drawn geometry, after the mirror. Preallocated to the cap.
    segments: Vec<SegmentInstance>,
    /// The single (pre-mirror) figure, replicated into
    /// [`segments`](Self::segments). Preallocated to the cap.
    single_buf: Vec<SegmentInstance>,
    /// Set when this frame's mirror replication overflowed the cap.
    mirror_overflow: Option<CapOverflow>,
    /// This frame's downsampled band levels, before easing.
    raw_levels: Vec<f32>,
    /// The **held** (eased) levels actually drawn — the per-element envelope
    /// state. Sized at load beside [`raw_levels`](Self::raw_levels).
    levels: Vec<f32>,
    /// Per-element stroke colour, rebuilt each frame into a buffer sized at load.
    colors: Vec<[f32; 3]>,
    /// Per-element world-space length, rebuilt each frame. Sized at load.
    lengths: Vec<f32>,
    /// Per-element stroke half-width, rebuilt each frame. Sized at load.
    widths: Vec<f32>,
    /// Per-element binding overrides (Plan 0034 Phase 4), one row per
    /// [`SERIES_PARAMS`] entry, each sized at load.
    series: [Vec<f32>; SERIES_PARAMS.len()],
    /// Which rows this frame's bindings actually wrote. Cleared in
    /// `reset_params` alongside the scalars, so a series never outlives the frame
    /// that produced it — the same lifetime rule every other param follows.
    series_active: [bool; SERIES_PARAMS.len()],
    /// The figure, from `[spectrum] layout`.
    layout: SpectrumLayout,
    /// Per-element easing, from `[spectrum] smoothing`.
    easing: Easing,
    /// Real elapsed seconds for this frame, injected through
    /// [`advance`](Scene::advance) — what makes the easing frame-rate
    /// independent (ADR-0019).
    dt: f32,
    /// The preset's baked colour LUT (ADR-0021), sampled on the CPU per element.
    palette: Palette,
    thickness: f32,
    hue: f32,
    hue_spread: f32,
    saturation: f32,
    palette_mix: f32,
    brightness: f32,
    scale: f32,
    base: f32,
    radius: f32,
    rotation: f32,
    zoom: f32,
    pan_x: f32,
    pan_y: f32,
    mirror_order: f32,
    mirror_reflect: f32,
}

impl SpectrumScene {
    /// Build the scene over the shared line renderer, preallocating its segment
    /// buffers to the cap. The element buffers are sized by `configure`, which
    /// the renderer runs on every preset switch.
    pub fn new(renderer: Rc<RefCell<LineRenderer>>) -> Self {
        Self {
            renderer,
            segments: Vec::with_capacity(MAX_SEGMENTS),
            single_buf: Vec::with_capacity(MAX_SEGMENTS),
            mirror_overflow: None,
            raw_levels: Vec::new(),
            levels: Vec::new(),
            colors: Vec::new(),
            lengths: Vec::new(),
            widths: Vec::new(),
            series: Default::default(),
            series_active: [false; SERIES_PARAMS.len()],
            layout: SpectrumLayout::default(),
            easing: Easing::INSTANT,
            dt: 0.0,
            // Replaced by the preset's palette on the next switch; the default
            // is the engine cosine, so an unconfigured scene still colours.
            palette: Palette::default_spectrum(),
            thickness: DEFAULT_THICKNESS,
            hue: DEFAULT_HUE,
            hue_spread: DEFAULT_HUE_SPREAD,
            saturation: DEFAULT_SATURATION,
            palette_mix: DEFAULT_PALETTE_MIX,
            brightness: DEFAULT_BRIGHTNESS,
            scale: DEFAULT_SCALE,
            base: DEFAULT_BASE,
            radius: DEFAULT_RADIUS,
            rotation: DEFAULT_ROTATION,
            zoom: DEFAULT_ZOOM,
            pan_x: DEFAULT_PAN,
            pan_y: DEFAULT_PAN,
            mirror_order: DEFAULT_MIRROR_ORDER,
            mirror_reflect: DEFAULT_MIRROR_REFLECT,
        }
    }

    /// Resize the per-element buffers and clear the envelope state. Off the hot
    /// path (preset load only) — the scratch every frame writes into is sized
    /// here, never per frame.
    fn resize(&mut self, elements: usize) {
        self.raw_levels.clear();
        self.raw_levels.resize(elements, 0.0);
        // Cleared rather than resized in place: a preset switch must not show
        // the previous preset's envelope decaying under the new one.
        self.levels.clear();
        self.levels.resize(elements, 0.0);
        self.colors.clear();
        self.colors.resize(elements, [0.0; 3]);
        self.lengths.clear();
        self.lengths.resize(elements, 0.0);
        self.widths.clear();
        self.widths.resize(elements, 0.0);
        for row in &mut self.series {
            row.clear();
            row.resize(elements, 0.0);
        }
        self.series_active = [false; SERIES_PARAMS.len()];
    }

    /// Element `i`'s value for series row `row`, or `fallback` when no binding
    /// drove that row this frame. Total — an out-of-range row or element reads
    /// the fallback rather than panicking on the hot path.
    fn series_value(&self, row: usize, i: usize, fallback: f32) -> f32 {
        if !self.series_active.get(row).copied().unwrap_or(false) {
            return fallback;
        }
        self.series
            .get(row)
            .and_then(|values| values.get(i))
            .copied()
            .unwrap_or(fallback)
    }
}

impl Scene for SpectrumScene {
    fn name(&self) -> &'static str {
        "spectrum"
    }

    fn advance(&mut self, dt: f32) {
        self.dt = dt;
    }

    fn reset_params(&mut self) {
        self.thickness = DEFAULT_THICKNESS;
        self.hue = DEFAULT_HUE;
        self.hue_spread = DEFAULT_HUE_SPREAD;
        self.saturation = DEFAULT_SATURATION;
        self.palette_mix = DEFAULT_PALETTE_MIX;
        self.brightness = DEFAULT_BRIGHTNESS;
        self.scale = DEFAULT_SCALE;
        self.base = DEFAULT_BASE;
        self.radius = DEFAULT_RADIUS;
        self.rotation = DEFAULT_ROTATION;
        self.zoom = DEFAULT_ZOOM;
        self.pan_x = DEFAULT_PAN;
        self.pan_y = DEFAULT_PAN;
        self.mirror_order = DEFAULT_MIRROR_ORDER;
        self.mirror_reflect = DEFAULT_MIRROR_REFLECT;
        // A per-element series lives exactly one frame, like every scalar above:
        // the rows keep their storage (sized at load) but stop being read until
        // a binding writes them again.
        self.series_active = [false; SERIES_PARAMS.len()];
    }

    fn set_param(&mut self, name: &str, value: f32) {
        match name {
            "base" => self.base = value,
            "scale" => self.scale = value,
            "radius" => self.radius = value,
            "rotation" => self.rotation = value,
            "thickness" => self.thickness = value,
            "hue" => self.hue = value,
            "hue_spread" => self.hue_spread = value,
            "saturation" => self.saturation = value,
            "palette_mix" => self.palette_mix = value,
            "brightness" => self.brightness = value,
            "zoom" => self.zoom = value,
            "pan_x" => self.pan_x = value,
            "pan_y" => self.pan_y = value,
            "mirror_order" => self.mirror_order = value,
            "mirror_reflect" => self.mirror_reflect = value,
            _ => {}
        }
    }

    fn set_param_series(&mut self, name: &str, values: &[f32]) {
        let Some(row) = SERIES_PARAMS.iter().position(|&p| p == name) else {
            // Not a per-element parameter on this scene (the whole-figure ones:
            // radius, rotation, the view transform, the mirror, hue_spread,
            // palette_mix, saturation). Fall back to the trait's rule — the
            // element-0 value — rather than dropping the binding.
            if let Some(&first) = values.first() {
                self.set_param(name, first);
            }
            return;
        };
        let (Some(dst), Some(active)) = (self.series.get_mut(row), self.series_active.get_mut(row))
        else {
            return; // unreachable: `position` returned an in-range row
        };
        // Copy rather than borrow: the caller's slice is the renderer's scratch,
        // reused by the next binding. `n` is the overlap, so a scratch sized for
        // a different element count can neither overrun nor leave stale values
        // beyond it (the rest of the row keeps whatever it held; only the first
        // `n` are read, because `update` walks `lengths`, which is `n` long).
        let n = dst.len().min(values.len());
        if let (Some(dst), Some(src)) = (dst.get_mut(..n), values.get(..n)) {
            dst.copy_from_slice(src);
        }
        *active = n > 0;
    }

    fn set_palette(&mut self, palette: &Palette) {
        self.palette = palette.clone();
    }

    fn configure(&mut self, cfg: &GeneratorConfig) -> Option<CapOverflow> {
        // Exhaustive with no wildcard, like the sibling line scenes: a new config
        // variant has to be acknowledged here rather than silently ignored.
        match cfg {
            GeneratorConfig::Spectrum {
                elements,
                layout,
                easing,
            } => {
                self.layout = *layout;
                self.easing = *easing;
                self.resize(*elements);
            }
            // Other scenes' configs (curve, L-system, star, particle attractor).
            GeneratorConfig::Curve { .. }
            | GeneratorConfig::LSystem { .. }
            | GeneratorConfig::Star { .. }
            | GeneratorConfig::Particles { .. } => {}
        }
        // Nothing is built here — the element count is validated at load and is
        // orders of magnitude under the segment cap — so nothing truncates.
        None
    }

    fn mirror_overflow(&self) -> Option<&CapOverflow> {
        self.mirror_overflow.as_ref()
    }

    fn update(&mut self, frame: &AnalysisFrame) {
        downsample(&frame.spectrum, &mut self.raw_levels);
        // Per-element temporal easing on the injected real `dt`, through the same
        // `Easing` the `[smoothing]` table uses (ADR-0035) — so "0.2 seconds"
        // means the same thing here as on a binding, at any frame rate. The
        // default is `INSTANT`, which passes the raw level straight through.
        for (held, &raw) in self.levels.iter_mut().zip(self.raw_levels.iter()) {
            *held = self.easing.step(*held, raw, self.dt);
        }

        // Per-element geometry and colour. Each scalar below reads its own series
        // row when a binding drove one this frame (Plan 0034 Phase 4) and the
        // whole-figure param otherwise — so `thickness = "0.01 + bin(index) * 5"`
        // varies the stroke across the figure while `thickness = "2"` does not,
        // with no branch in the preset and no second code path here.
        let count = self.levels.len();
        let span = count.max(1) as f32;
        for i in 0..count {
            let level = self.levels.get(i).copied().unwrap_or(0.0);
            let base = self.series_value(SERIES_BASE, i, self.base);
            let scale = self.series_value(SERIES_SCALE, i, self.scale);
            let thickness = self.series_value(SERIES_THICKNESS, i, self.thickness);
            let brightness = self.series_value(SERIES_BRIGHTNESS, i, self.brightness);
            // `hue_spread` walks the palette along the axis on top of whatever
            // `hue` is — at the default spread of 0 the figure is one hue, at 1
            // it spans the palette from the lowest element to the highest.
            let hue =
                self.series_value(SERIES_HUE, i, self.hue) + self.hue_spread * (i as f32 / span);
            let rgb = desaturate(self.palette.sample(hue, self.palette_mix), self.saturation);

            if let Some(slot) = self.lengths.get_mut(i) {
                *slot = element_length(level, base, scale);
            }
            if let Some(slot) = self.widths.get_mut(i) {
                *slot = (thickness * WIDTH_SCALE).max(0.0005);
            }
            if let Some(slot) = self.colors.get_mut(i) {
                *slot = [
                    rgb[0] * brightness,
                    rgb[1] * brightness,
                    rgb[2] * brightness,
                ];
            }
        }

        let place = Placement {
            radius: self.radius,
            rotation: self.rotation,
        };
        build(
            self.layout,
            &self.lengths,
            &self.widths,
            &self.colors,
            place,
            &mut self.single_buf,
        );

        let mirror = MirrorSpec::from_params(self.mirror_order, self.mirror_reflect);
        if mirror.is_identity() {
            // Identity replication would copy the whole set to produce exactly
            // what it was given; swap instead (Plan 0031 Phase 4). Both buffers
            // are preallocated to the cap, so neither can grow later.
            std::mem::swap(&mut self.single_buf, &mut self.segments);
            self.mirror_overflow = None;
            return;
        }
        let dropped = replicate_mirror(&self.single_buf, mirror, MAX_SEGMENTS, &mut self.segments);
        self.mirror_overflow = (dropped > 0).then_some(CapOverflow {
            dropped,
            context: OverflowContext::Mirror(mirror.order),
        });
    }

    fn render(
        &mut self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        aspect: f32,
    ) {
        let xform = ViewTransform {
            zoom: self.zoom,
            pan: [self.pan_x, self.pan_y],
            _pad: 0.0,
        };
        self.renderer
            .borrow_mut()
            .draw(queue, encoder, view, aspect, 1.0, xform, &self.segments);
    }
}

#[cfg(test)]
mod tests {
    // Test asserts index the produced Vec; allowed here over the file's
    // hot-path pragma since test code is not the render path.
    #![allow(clippy::indexing_slicing)]

    use super::*;
    use crate::dsp::SPECTRUM_BINS;

    fn white(n: usize) -> Vec<[f32; 3]> {
        vec![[1.0, 1.0, 1.0]; n]
    }

    fn even_widths(n: usize) -> Vec<f32> {
        vec![0.01; n]
    }

    /// Build one figure from raw per-element lengths, at the origin, unturned.
    fn figure(layout: SpectrumLayout, lengths: &[f32]) -> Vec<SegmentInstance> {
        let mut out = Vec::new();
        build(
            layout,
            lengths,
            &even_widths(lengths.len()),
            &white(lengths.len()),
            Placement::default(),
            &mut out,
        );
        out
    }

    /// The element lengths a stimulus produces, low index first — the whole
    /// chain from band array to drawn geometry, at `base = 0`, `scale = 1`.
    fn bar_lengths(spectrum: &[f32; SPECTRUM_BINS], elements: usize) -> Vec<f32> {
        let mut levels = vec![0.0; elements];
        downsample(spectrum, &mut levels);
        let lengths: Vec<f32> = levels
            .iter()
            .map(|&level| element_length(level, 0.0, 1.0))
            .collect();
        figure(SpectrumLayout::Bars, &lengths)
            .iter()
            .map(|s| s.b[1] - s.a[1])
            .collect()
    }

    /// A band array with energy `1.0` in `lit` bands starting at `from`.
    fn banded(from: usize, lit: usize) -> [f32; SPECTRUM_BINS] {
        let mut spectrum = [0.0; SPECTRUM_BINS];
        for band in spectrum.iter_mut().skip(from).take(lit) {
            *band = 1.0;
        }
        spectrum
    }

    /// Plan 0034 Phase 2 done-when 3, and the claim that separates a working
    /// spectrum from N bars of noise: **energy lands on the elements that own
    /// its frequencies.** Bass raises the low-index bars and leaves the high
    /// ones flat; treble does exactly the reverse.
    #[test]
    fn energy_raises_the_elements_that_own_its_frequencies() {
        let n = 24;
        let quarter = n / 4;

        let low = bar_lengths(&banded(0, SPECTRUM_BINS / 4), n);
        let high = bar_lengths(&banded(3 * SPECTRUM_BINS / 4, SPECTRUM_BINS / 4), n);

        for i in 0..quarter {
            assert!(
                low[i] > 0.5,
                "bass stimulus must raise low element {i}, got {}",
                low[i]
            );
            assert_eq!(
                high[i], 0.0,
                "treble stimulus must leave low element {i} flat"
            );
        }
        for i in (n - quarter)..n {
            assert!(
                high[i] > 0.5,
                "treble stimulus must raise high element {i}, got {}",
                high[i]
            );
            assert_eq!(
                low[i], 0.0,
                "bass stimulus must leave high element {i} flat"
            );
        }
    }

    /// Plan 0034 Phase 3 done-when 2. The downsample is a genuine **partition**
    /// of the band array at every element count the loader admits: contiguous,
    /// non-overlapping, complete. Proved by conservation — the element means,
    /// weighted by how many bands each covers, must sum to the band total, which
    /// is only possible if every band was counted exactly once.
    #[test]
    fn every_band_is_counted_exactly_once_at_every_element_count() {
        // A distinct value per band, so a dropped or double-counted band shifts
        // the total rather than cancelling out.
        let mut spectrum = [0.0f32; SPECTRUM_BINS];
        for (i, band) in spectrum.iter_mut().enumerate() {
            *band = (i + 1) as f32;
        }
        let total: f32 = spectrum.iter().sum();

        // Every count the loader admits, walked end to end at the small sizes
        // and sampled at the awkward ones (7, 24, 30, 31 do not divide 64).
        for n in 2..=SPECTRUM_BINS {
            let mut levels = vec![0.0; n];
            downsample(&spectrum, &mut levels);

            let mut covered = 0usize;
            let mut weighted = 0.0f32;
            for (i, &level) in levels.iter().enumerate() {
                let lo = i * SPECTRUM_BINS / n;
                let hi = (i + 1) * SPECTRUM_BINS / n;
                assert_eq!(
                    lo,
                    covered,
                    "element {i} of {n} must start where element {} ended",
                    i.saturating_sub(1)
                );
                assert!(hi > lo, "element {i} of {n} must cover at least one band");
                covered = hi;
                weighted += level * (hi - lo) as f32;
            }
            assert_eq!(covered, SPECTRUM_BINS, "{n} elements must cover every band");
            assert!(
                (weighted - total).abs() < 1e-2,
                "{n} elements lose or duplicate energy: {weighted} vs {total}"
            );
        }
    }

    /// The bars layout is what a spectrum readout should be: one segment per
    /// element, evenly spaced left to right, standing on a common baseline.
    #[test]
    fn bars_are_evenly_spaced_upright_segments_on_one_baseline() {
        let levels = [0.0f32, 0.5, 1.0, 0.25];
        let plain: Vec<f32> = levels
            .iter()
            .map(|&level| element_length(level, 0.0, 1.0))
            .collect();
        let out = figure(SpectrumLayout::Bars, &plain);
        assert_eq!(out.len(), levels.len(), "one segment per element");

        let spacing = out[1].a[0] - out[0].a[0];
        for (i, seg) in out.iter().enumerate() {
            assert_eq!(seg.a[0], seg.b[0], "element {i} is upright");
            assert_eq!(seg.a[1], BASELINE_Y, "element {i} stands on the baseline");
            assert!(
                seg.a[0].abs() <= SPAN_X,
                "element {i} stays inside the span"
            );
            if i > 0 {
                assert!(
                    ((out[i].a[0] - out[i - 1].a[0]) - spacing).abs() < 1e-6,
                    "elements are evenly spaced"
                );
            }
        }
        // Length tracks the level, and `base` lifts every element off the floor.
        // Compared approximately: a length is a difference of two world-space
        // f32 coordinates, so it carries the baseline's rounding.
        let length = |seg: &SegmentInstance| seg.b[1] - seg.a[1];
        assert!(length(&out[0]).abs() < 1e-6, "a silent element is flat");
        assert!(
            (length(&out[2]) - 1.0).abs() < 1e-6,
            "a full element is full"
        );

        let lifted_lengths: Vec<f32> = levels
            .iter()
            .map(|&level| element_length(level, 0.1, 1.0))
            .collect();
        let lifted = figure(SpectrumLayout::Bars, &lifted_lengths);
        assert!(
            (length(&lifted[0]) - 0.1).abs() < 1e-6,
            "base is the resting length, so silence still draws a comb"
        );
        // And a level can never pull an element below its baseline, whatever a
        // degenerate `scale` does.
        assert_eq!(element_length(1.0, 0.0, -5.0), 0.0, "length floors at zero");
    }

    /// Plan 0034 Phase 3 done-when 1: each layout draws the *same data* as a
    /// different figure, and each is structurally what its name claims.
    #[test]
    fn each_layout_draws_the_same_levels_as_its_own_figure() {
        let levels = [0.1f32, 0.4, 0.9, 0.3, 0.6];

        // Polyline: a connected chain, so consecutive segments share an endpoint
        // and there is one fewer segment than elements.
        let out = figure(SpectrumLayout::Polyline, &levels);
        assert_eq!(out.len(), levels.len() - 1, "n-1 segments join n points");
        for i in 1..out.len() {
            assert_eq!(
                out[i - 1].b,
                out[i].a,
                "segment {i} continues from the previous one"
            );
        }
        // The chain spans the full width, edge to edge.
        assert!(
            (out[0].a[0] + SPAN_X).abs() < 1e-6,
            "starts at the left edge"
        );
        assert!(
            (out[out.len() - 1].b[0] - SPAN_X).abs() < 1e-6,
            "ends at the right edge"
        );

        // Radial ring: one spoke per element, each pointing away from the origin
        // and starting on a circle of the configured inner radius.
        let mut out = Vec::new();
        build(
            SpectrumLayout::RadialRing,
            &levels,
            &even_widths(levels.len()),
            &white(levels.len()),
            Placement {
                radius: 0.4,
                rotation: 0.0,
            },
            &mut out,
        );
        assert_eq!(out.len(), levels.len(), "one spoke per element");
        for (i, seg) in out.iter().enumerate() {
            let inner = (seg.a[0] * seg.a[0] + seg.a[1] * seg.a[1]).sqrt();
            let outer = (seg.b[0] * seg.b[0] + seg.b[1] * seg.b[1]).sqrt();
            assert!(
                (inner - 0.4).abs() < 1e-5,
                "spoke {i} starts on the ring, got {inner}"
            );
            assert!(
                (outer - (0.4 + levels[i])).abs() < 1e-5,
                "spoke {i} reaches its own level outward"
            );
        }
        // The spokes are distinct directions covering the circle once.
        let angle = |seg: &SegmentInstance| seg.a[1].atan2(seg.a[0]);
        let expected = std::f32::consts::TAU / levels.len() as f32;
        for i in 1..out.len() {
            // Wrapped, because `atan2` returns -pi..pi and the walk crosses it.
            let step = (angle(&out[i]) - angle(&out[i - 1])).rem_euclid(std::f32::consts::TAU);
            assert!(
                (step - expected).abs() < 1e-5,
                "spoke {i} sits one even step around the circle, got {step}"
            );
        }
    }

    /// `rotation` turns the whole figure about the origin — the same angle for
    /// every layout, so it is never a silent no-op on one of them.
    #[test]
    fn rotation_turns_every_layout_by_the_same_angle() {
        let levels = [0.2f32, 0.7, 0.4, 0.5];
        let quarter = std::f32::consts::FRAC_PI_2;

        for layout in [
            SpectrumLayout::Bars,
            SpectrumLayout::Polyline,
            SpectrumLayout::RadialRing,
        ] {
            let plain = figure(layout, &levels);
            let mut turned = Vec::new();
            build(
                layout,
                &levels,
                &even_widths(levels.len()),
                &white(levels.len()),
                Placement {
                    radius: 0.0,
                    rotation: quarter,
                },
                &mut turned,
            );
            assert_eq!(plain.len(), turned.len(), "{layout:?} keeps its segments");
            for (a, b) in plain.iter().zip(&turned) {
                // A quarter turn maps (x, y) to (-y, x).
                assert!(
                    (b.a[0] + a.a[1]).abs() < 1e-5 && (b.a[1] - a.a[0]).abs() < 1e-5,
                    "{layout:?} start point is not rotated a quarter turn"
                );
                assert!(
                    (b.b[0] + a.b[1]).abs() < 1e-5 && (b.b[1] - a.b[0]).abs() < 1e-5,
                    "{layout:?} end point is not rotated a quarter turn"
                );
            }
        }
    }

    /// Plan 0034 Phase 3 done-when 3. Per-element easing is expressed in seconds
    /// and is **frame-rate independent**: reaching the same wall-clock time
    /// through many small steps or a few large ones lands in the same place, so a
    /// preset looks the same at 60 and 144 Hz (ADR-0019).
    #[test]
    fn per_element_easing_is_frame_rate_independent() {
        let easing = Easing::symmetric(0.25);
        let settle = |steps: u32, dt: f32| {
            let mut held = 0.0f32;
            for _ in 0..steps {
                held = easing.step(held, 1.0, dt);
            }
            held
        };
        // One second of wall clock at 60, 144 and 30 fps.
        let at_60 = settle(60, 1.0 / 60.0);
        let at_144 = settle(144, 1.0 / 144.0);
        let at_30 = settle(30, 1.0 / 30.0);
        assert!(
            (at_60 - at_144).abs() < 1e-3 && (at_60 - at_30).abs() < 1e-3,
            "one second of easing must land in the same place: 60 {at_60}, 144 {at_144}, 30 {at_30}"
        );
        // And it is genuinely easing rather than snapping or stalling.
        assert!(
            at_60 > 0.9 && at_60 < 1.0,
            "a 0.25 s constant is most of the way there after 1 s, got {at_60}"
        );
        // `INSTANT` (the default) passes the raw value straight through.
        assert_eq!(Easing::INSTANT.step(0.0, 0.6, 1.0 / 60.0), 0.6);
    }

    /// A degenerate element count must not panic or produce garbage geometry —
    /// this runs per frame.
    #[test]
    fn a_degenerate_element_count_is_inert() {
        let mut none: Vec<f32> = Vec::new();
        downsample(&[0.5; SPECTRUM_BINS], &mut none);
        for layout in [
            SpectrumLayout::Bars,
            SpectrumLayout::Polyline,
            SpectrumLayout::RadialRing,
        ] {
            let mut out = vec![SegmentInstance {
                a: [9.0, 9.0],
                b: [9.0, 9.0],
                color: [1.0, 1.0, 1.0],
                width: 1.0,
            }];
            build(layout, &none, &[], &[], Placement::default(), &mut out);
            assert!(out.is_empty(), "{layout:?}: no elements, no segments");

            // Short width/colour lists fall back rather than panicking — the hot
            // path must stay total even if the buffers ever disagree.
            build(
                layout,
                &[0.3, 0.4],
                &[],
                &[],
                Placement::default(),
                &mut out,
            );
            assert!(
                out.iter().all(|s| s.width > 0.0),
                "{layout:?}: a missing width falls back to a drawable one"
            );
        }
        // A single element has no polyline to draw and must not underflow.
        assert!(
            figure(SpectrumLayout::Polyline, &[0.5]).is_empty(),
            "one point is not a line"
        );

        // An empty spectrum leaves every element at zero rather than reading
        // stale values.
        let mut levels = vec![7.0; 4];
        downsample(&[], &mut levels);
        assert_eq!(levels, vec![0.0; 4]);
    }
}
