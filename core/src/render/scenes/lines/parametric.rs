//! Parametric-curve scene: a pure `t -> (x, y)` curve resampled every frame
//! into the shared [`LineRenderer`] (ADR-0007 parametric build model). Phase 1
//! is hardcoded to one Maurer rose that gently rotates on the deterministic
//! scene clock; Phase 2 makes the curve family and every named parameter
//! preset-driven so audio can sweep it live.
//!
//! ## The colour axis: **position along the traced path** (ADR-0059)
//!
//! This scene honours `[palette]` / `[palette_b]` / `palette_mix` / `hue_spread`
//! / `saturation` through the shared [`ColorRamp`], and the axis its generator
//! makes meaningful is **how far along the walk a chord sits**: `0` at the first
//! sampled point, `1` at the last. On a Maurer rose that is the drawn-stroke
//! reading — the web is one continuous walk, so the ramp travels along it the way
//! a pen would.
//!
//! **Normalized over `samples`, not over the revealed prefix.** `draw_progress`
//! is a reveal, so a chord's place on the curve is a property of the curve; if
//! the divisor were the drawn count, a per-beat `draw_progress` would drag every
//! chord's colour with it and the figure would re-tint rather than draw itself
//! on. Revealing half the curve therefore shows the palette's first half.
//!
//! `hue_spread = 0` collapses the ramp to the single `hue` this scene has always
//! drawn, so the surface is a strict superset.

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

use super::super::{FALLBACK_DT, Phase, Scene};
use super::biarc::Piece;
use super::renderer::{ArcInstance, JOINED_A, JOINED_B, LineRenderer, SegmentInstance};
use super::{
    CapOverflow, ColorRamp, CurveFamily, GeneratorConfig, MirrorSpec, OverflowContext,
    ViewTransform, curves, replicate_mirror,
};
use crate::dsp::AnalysisFrame;
use crate::render::palette::{self, Palette};

// Parameter defaults — a calm, whole, slowly turning rose when nothing is bound.
const DEFAULT_N: f32 = 6.0;
const DEFAULT_D: f32 = 71.0;
// Shape params (ADR-0029): both no-ops by default, so an unbound rose is the
// plain `sin(n*theta)` curve — `phase` adds inside the sine, `radial_offset`
// adds to the radius.
const DEFAULT_PHASE: f32 = 0.0;
const DEFAULT_RADIAL_OFFSET: f32 = 0.0;
const DEFAULT_SAMPLES: f32 = 361.0;
const DEFAULT_THICKNESS: f32 = 2.0;
const DEFAULT_HUE: f32 = 0.6;
/// Colour surface (ADR-0021 / ADR-0059), all three at the value that reproduces
/// the single flat `hue` this scene drew before the palette reached it: no ramp
/// along the path, palette A alone, unmodified saturation.
const DEFAULT_HUE_SPREAD: f32 = 0.0;
const DEFAULT_SATURATION: f32 = 1.0;
const DEFAULT_PALETTE_MIX: f32 = 0.0;
const DEFAULT_SPIN: f32 = 0.1;
const DEFAULT_SCALE: f32 = 0.9;
const DEFAULT_BRIGHTNESS: f32 = 1.0;
/// The line renderer's **per-segment falloff** multiplier (Plan 0038 Phase 1) —
/// not a post-process bloom. `1.0` is the value every line scene passed as a
/// literal before it was bound, so the default is exactly today's look.
const DEFAULT_GLOW: f32 = 1.0;
const DEFAULT_DRAW_PROGRESS: f32 = 1.0;
// Shared view transform (ADR-0018): identity by default, so an unbound preset is
// unchanged.
const DEFAULT_ZOOM: f32 = 1.0;
const DEFAULT_PAN: f32 = 0.0;
// Geometry mirror (Phase 4): identity by default (one copy, no reflection).
const DEFAULT_MIRROR_ORDER: f32 = 1.0;
const DEFAULT_MIRROR_REFLECT: f32 = 0.0;

/// A parametric line curve (the Maurer rose), sampled per frame and driven by
/// named preset parameters over the audio analysis.
pub struct ParametricCurveScene {
    /// The single line renderer, shared with the other line scenes (ADR-0007:
    /// "one line renderer"). Only the active scene draws in a frame, so the
    /// shared pipeline + buffer are never contended.
    renderer: Rc<RefCell<LineRenderer>>,
    /// Reused draw buffer — the mirrored geometry actually rendered. Preallocated
    /// to the cap so replication never allocates on the hot path.
    segments: Vec<SegmentInstance>,
    /// Reused buffer for the single (pre-mirror) sampled curve, replicated into
    /// [`segments`](Self::segments) by [`replicate_mirror`]. Preallocated.
    single_buf: Vec<SegmentInstance>,
    /// [`segments`](Self::segments)' arc half, and its pre-mirror source — the
    /// G1 chain a **smooth** walk is fitted to (ADR-0098, Plan 0087 Phase 5).
    /// Both empty for a chord web, which is every shipped `d`, so that preset
    /// draws exactly the segment batch it always did.
    arcs: Vec<ArcInstance>,
    single_arcs: Vec<ArcInstance>,
    /// The fit's three scratch buffers: the sampled walk, the chain it fits,
    /// and each piece's place along the walk. Fields rather than locals because
    /// the fit runs every frame and must not allocate (ADR-0007's parametric
    /// build model gives it no load moment to run at).
    points: Vec<[f32; 2]>,
    pieces: Vec<Piece>,
    walk: Vec<f32>,
    /// The active tier's segment ceiling
    /// ([`TierConfig::max_segments`](crate::render::TierConfig::max_segments)),
    /// resolved once at construction (Plan 0044). A field rather than a constant
    /// so the tier can raise it; the buffers above are preallocated to it, which
    /// is what keeps the per-frame replication allocation-free.
    max_segments: usize,
    /// Set when this frame's mirror replication overflowed the segment cap
    /// (ADR-0007: never a silent cut); `None` when it fit.
    mirror_overflow: Option<CapOverflow>,
    /// Which curve family to sample, chosen at preset load via `configure`.
    family: CurveFamily,
    /// This frame's elapsed real time, stored by [`advance`](Scene::advance) and
    /// consumed by [`update`](Scene::update) — `advance` runs before this
    /// frame's parameter values land, so the rate it would integrate against is
    /// the previous frame's.
    dt: f32,
    /// The integrated rotation ([`Phase`]). **This scene does not read the
    /// shared clock at all**, and has no `set_time`: the figure's rotation was
    /// the clock's only reader here, and a rate has to be integrated rather than
    /// multiplied against elapsed time (ADR-0135).
    spin_phase: Phase,
    /// The preset's baked colour LUT (ADR-0021), sampled on the CPU per chord.
    /// Defaults to the engine cosine, which is the ramp this scene coloured
    /// through before the palette reached it.
    palette: Palette,
    n: f32,
    d: f32,
    phase: f32,
    radial_offset: f32,
    samples: f32,
    thickness: f32,
    hue: f32,
    hue_spread: f32,
    saturation: f32,
    palette_mix: f32,
    /// Hard palette bands and their contour (ADR-0078), raw as the preset
    /// bound them -- `palette::band_steps` / `band_contour` condition them on
    /// the way to the sample site.
    palette_steps: f32,
    palette_contour: f32,
    spin: f32,
    scale: f32,
    brightness: f32,
    glow: f32,
    softness: f32,
    /// Whether this figure draws through the **opacity-preserving** seam
    /// rather than the additive one, from `stroke_blend` (ADR-0138).
    ///
    /// At or above [`OPAQUE_BLEND`](super::OPAQUE_BLEND) the whole batch
    /// composites over: a stroke laid on another replaces the interior of what
    /// it covers instead of summing with it, so a quantized palette keeps its
    /// plateaus. Below it the batch is additive light. `0` is the default, so a
    /// preset that does not bind this draws exactly what it drew.
    stroke_blend: f32,
    draw_progress: f32,
    zoom: f32,
    pan_x: f32,
    pan_y: f32,
    mirror_order: f32,
    mirror_reflect: f32,
}

impl ParametricCurveScene {
    /// Build the scene over the shared line renderer, preallocating its segment
    /// buffer to the cap.
    pub fn new(renderer: Rc<RefCell<LineRenderer>>, max_segments: usize) -> Self {
        Self {
            renderer,
            segments: Vec::with_capacity(max_segments),
            single_buf: Vec::with_capacity(max_segments),
            arcs: Vec::with_capacity(max_segments),
            single_arcs: Vec::with_capacity(max_segments),
            points: Vec::with_capacity(max_segments),
            pieces: Vec::with_capacity(max_segments),
            walk: Vec::with_capacity(max_segments),
            max_segments,
            mirror_overflow: None,
            family: CurveFamily::MaurerRose,
            dt: FALLBACK_DT,
            spin_phase: Phase::default(),
            // Replaced by the preset's palette on the next switch; the default
            // is the engine cosine, so an unconfigured scene still colours.
            palette: Palette::default_spectrum(),
            n: DEFAULT_N,
            d: DEFAULT_D,
            phase: DEFAULT_PHASE,
            radial_offset: DEFAULT_RADIAL_OFFSET,
            samples: DEFAULT_SAMPLES,
            thickness: DEFAULT_THICKNESS,
            hue: DEFAULT_HUE,
            hue_spread: DEFAULT_HUE_SPREAD,
            saturation: DEFAULT_SATURATION,
            palette_mix: DEFAULT_PALETTE_MIX,
            palette_steps: palette::DEFAULT_PALETTE_STEPS,
            palette_contour: palette::DEFAULT_PALETTE_CONTOUR,
            spin: DEFAULT_SPIN,
            scale: DEFAULT_SCALE,
            brightness: DEFAULT_BRIGHTNESS,
            glow: DEFAULT_GLOW,
            softness: super::DEFAULT_SOFTNESS,
            stroke_blend: super::ADDITIVE_BLEND,
            draw_progress: DEFAULT_DRAW_PROGRESS,
            zoom: DEFAULT_ZOOM,
            pan_x: DEFAULT_PAN,
            pan_y: DEFAULT_PAN,
            mirror_order: DEFAULT_MIRROR_ORDER,
            mirror_reflect: DEFAULT_MIRROR_REFLECT,
        }
    }
}

impl ParametricCurveScene {
    /// Split the fitted chain into the two instance buffers the renderer draws,
    /// colouring each piece by **where it sits along the walk**.
    ///
    /// The walk position is what the fit reports, not the piece's index: a
    /// piece spans as many samples as the budget allowed, so the `k`th piece is
    /// not the `k`th chord and an index would run the palette at the wrong rate
    /// — visibly, wherever the fit's pieces are uneven, which is everywhere a
    /// rose's curvature changes. `samples` stays the divisor for the reason
    /// [`color_along_path`] gives: a chord's place on the curve belongs to the
    /// curve, so a `draw_progress` reveal draws the gradient on rather than
    /// re-tinting it.
    fn split_pieces(&mut self, samples: usize, ramp: ColorRamp, color: [f32; 3], width: f32) {
        self.single_buf.clear();
        self.single_arcs.clear();
        let span = samples.saturating_sub(1).max(1) as f32;
        let last = self.pieces.len().saturating_sub(1);
        for (k, piece) in self.pieces.iter().enumerate() {
            let color = self
                .walk
                .get(k)
                .map_or(color, |at| ramp.at(&self.palette, at / span));
            match *piece {
                Piece::Arc {
                    centre,
                    radius,
                    start,
                    sweep,
                } => self.single_arcs.push(ArcInstance {
                    centre,
                    radius,
                    angle_start: start,
                    angle_sweep: sweep,
                    color,
                    width,
                }),
                Piece::Line { a, b } => {
                    // A chain is a chain (ADR-0041): every piece but the walk's
                    // two ends continues a neighbour, across a corner as much
                    // as along a curve — the join is what covers the wedge
                    // between two strokes, and a corner is where there is one.
                    let mut joined = 0;
                    if k > 0 {
                        joined |= JOINED_A;
                    }
                    if k < last {
                        joined |= JOINED_B;
                    }
                    self.single_buf.push(SegmentInstance {
                        a,
                        b,
                        color,
                        width,
                        alpha: 1.0,
                        joined,
                    });
                }
            }
        }
    }
}

/// Colour each chord by **how far along the traced path it sits** (ADR-0059's
/// axis for this generator): chord `i` of a `samples`-point walk is at
/// `i / (samples - 1)`, so the ramp runs from the walk's first point to its last.
///
/// `samples` is the **full** curve's chord count, not `segs.len()`. Those differ
/// whenever `draw_progress` reveals a prefix, and the full count is the right
/// divisor: a chord's place on the curve belongs to the curve, so a per-beat
/// reveal draws the gradient on rather than re-tinting every chord it already
/// drew. A degenerate `samples` (0 or 1) leaves the whole figure at `u = 0`,
/// which is the flat `hue` — never a divide by zero.
pub(crate) fn color_along_path(
    segs: &mut [SegmentInstance],
    palette: &Palette,
    ramp: ColorRamp,
    samples: usize,
) {
    let span = samples.saturating_sub(1).max(1) as f32;
    for (i, seg) in segs.iter_mut().enumerate() {
        seg.color = ramp.at(palette, i as f32 / span);
    }
}

/// Parameter vocabulary — see [`fragment_field::PARAMS`](crate::render::scenes::fragment_field::PARAMS).
/// **Keep in sync with `set_param` below.**
pub const PARAMS: &[&str] = &[
    "n",
    "d",
    "phase",
    "radial_offset",
    "samples",
    "thickness",
    "hue",
    "hue_spread",
    "saturation",
    "palette_mix",
    "palette_steps",
    "palette_contour",
    "spin",
    "scale",
    "brightness",
    "glow",
    "softness",
    "stroke_blend",
    "draw_progress",
    "zoom",
    "pan_x",
    "pan_y",
    "mirror_order",
    "mirror_reflect",
];

impl Scene for ParametricCurveScene {
    fn name(&self) -> &'static str {
        "parametric curve"
    }

    fn advance(&mut self, dt: f32) {
        // Stored, not integrated: the `spin` this frame will use has not been
        // set yet. A non-finite or negative `dt` degrades to the capture step
        // rather than poisoning the accumulator, which is the one piece of state
        // here a bad frame could corrupt permanently.
        self.dt = if dt.is_finite() && dt > 0.0 {
            dt
        } else {
            FALLBACK_DT
        };
    }

    fn reset_params(&mut self) {
        self.n = DEFAULT_N;
        self.d = DEFAULT_D;
        self.phase = DEFAULT_PHASE;
        self.radial_offset = DEFAULT_RADIAL_OFFSET;
        self.samples = DEFAULT_SAMPLES;
        self.thickness = DEFAULT_THICKNESS;
        self.hue = DEFAULT_HUE;
        self.hue_spread = DEFAULT_HUE_SPREAD;
        self.saturation = DEFAULT_SATURATION;
        self.palette_mix = DEFAULT_PALETTE_MIX;
        self.palette_steps = palette::DEFAULT_PALETTE_STEPS;
        self.palette_contour = palette::DEFAULT_PALETTE_CONTOUR;
        self.spin = DEFAULT_SPIN;
        self.scale = DEFAULT_SCALE;
        self.brightness = DEFAULT_BRIGHTNESS;
        self.glow = DEFAULT_GLOW;
        self.softness = super::DEFAULT_SOFTNESS;
        self.stroke_blend = super::ADDITIVE_BLEND;
        self.draw_progress = DEFAULT_DRAW_PROGRESS;
        self.zoom = DEFAULT_ZOOM;
        self.pan_x = DEFAULT_PAN;
        self.pan_y = DEFAULT_PAN;
        self.mirror_order = DEFAULT_MIRROR_ORDER;
        self.mirror_reflect = DEFAULT_MIRROR_REFLECT;
    }

    fn set_param(&mut self, name: &str, value: f32) {
        match name {
            "n" => self.n = value,
            "d" => self.d = value,
            "phase" => self.phase = value,
            "radial_offset" => self.radial_offset = value,
            "samples" => self.samples = value,
            "thickness" => self.thickness = value,
            "hue" => self.hue = value,
            "hue_spread" => self.hue_spread = value,
            "saturation" => self.saturation = value,
            "palette_mix" => self.palette_mix = value,
            "palette_steps" => self.palette_steps = value,
            "palette_contour" => self.palette_contour = value,
            "spin" => self.spin = value,
            "scale" => self.scale = value,
            "brightness" => self.brightness = value,
            "glow" => self.glow = value,
            "softness" => self.softness = value,
            "stroke_blend" => self.stroke_blend = value,
            "draw_progress" => self.draw_progress = value,
            "zoom" => self.zoom = value,
            "pan_x" => self.pan_x = value,
            "pan_y" => self.pan_y = value,
            "mirror_order" => self.mirror_order = value,
            "mirror_reflect" => self.mirror_reflect = value,
            _ => {}
        }
    }

    fn set_palette(&mut self, palette: &Palette) {
        self.palette = palette.clone();
    }

    fn configure(&mut self, cfg: &GeneratorConfig) -> Option<CapOverflow> {
        // A curve preset records its family here (off the hot path). Later
        // phases' generator config variants are for the generator scenes; this
        // match gains ignore-arms for them when they land.
        match cfg {
            GeneratorConfig::Curve { family } => self.family = *family,
            // Other scenes' configs (L-system, star, particle attractor,
            // spectrum readout).
            GeneratorConfig::LSystem { .. }
            | GeneratorConfig::Star { .. }
            | GeneratorConfig::Particles { .. }
            | GeneratorConfig::Spectrum { .. }
            | GeneratorConfig::WarpMesh { .. } => {}
        }
        // No load-time truncation: the parametric sampler builds nothing here.
        // Its only cap is a per-frame `samples` clamp in `update` (see there).
        None
    }

    fn mirror_overflow(&self) -> Option<&CapOverflow> {
        self.mirror_overflow.as_ref()
    }

    fn update(&mut self, _frame: &AnalysisFrame) {
        // Per-frame defensive clamp: a huge `samples` can never overrun the
        // preallocated buffer (ADR-0007 cap is explicit). Unlike the generator
        // scenes' load-time build, `samples` is an expression evaluated every
        // frame, so there is no "load" moment to surface a truncation at, and a
        // sane curve preset (samples in the hundreds) never approaches the cap —
        // the clamp is a safety backstop, not a structural cut worth reporting.
        let samples = (self.samples.max(0.0) as usize).min(self.max_segments);
        self.spin_phase.step(self.spin, self.dt);
        let rotation = self.spin_phase.get();
        let ramp = ColorRamp {
            hue: self.hue,
            hue_spread: self.hue_spread,
            palette_mix: self.palette_mix,
            palette_steps: self.palette_steps,
            saturation: self.saturation,
            brightness: self.brightness,
        };
        // The sampler paints the whole web in the walk's starting colour; the
        // pass below walks it along the path. Keeping the sampler colour-agnostic
        // is what leaves the curve maths free of any palette knowledge.
        let color = ramp.at(&self.palette, 0.0);
        let width = super::half_width(self.thickness);

        let params = curves::RoseParams {
            n: self.n,
            d: self.d,
            phase: self.phase,
            radial_offset: self.radial_offset,
            samples,
            scale: self.scale,
            rotation,
            draw_progress: self.draw_progress,
            color,
            width,
        };

        // Sample the single curve, then replicate it under the geometry mirror
        // (Phase 4). At the default identity spec this is a 1:1 copy, so an
        // un-mirrored preset is unchanged.
        //
        // **Two primitives, one walk** (Plan 0087 Phase 5). A *smooth* Maurer
        // walk — a small angular step, where the successive points trace a rose
        // rather than web it — is fitted to a G1 arc chain and drawn without a
        // tangent break anywhere. A chord web declines the fit and takes the
        // path below, which is untouched: the chords **are** that figure, and
        // an arc through two of them would be drawing something else.
        let fitted = match self.family {
            CurveFamily::MaurerRose => curves::maurer_rose_pieces(
                params,
                &mut self.points,
                &mut self.pieces,
                &mut self.walk,
            ),
        };
        if fitted {
            self.split_pieces(samples, ramp, color, width);
        } else {
            match self.family {
                CurveFamily::MaurerRose => curves::maurer_rose(params, &mut self.single_buf),
            }
            self.single_arcs.clear();
            color_along_path(&mut self.single_buf, &self.palette, ramp, samples);
        }
        let mirror = MirrorSpec::from_params(self.mirror_order, self.mirror_reflect);
        if mirror.is_identity() {
            // Identity spec: replication would copy the whole segment set into a
            // second buffer to produce exactly what it was given. Swap instead —
            // O(1), and both buffers were preallocated to `max_segments`, so
            // neither can grow later. `maurer_rose` clears before it fills, so
            // whatever lands back in `single_buf` is overwritten next frame.
            debug_assert!(
                self.single_buf.len() <= self.max_segments,
                "the sampler already clamps to the cap, so identity cannot truncate"
            );
            std::mem::swap(&mut self.single_buf, &mut self.segments);
            std::mem::swap(&mut self.single_arcs, &mut self.arcs);
            self.mirror_overflow = None;
            return;
        }
        let dropped = replicate_mirror(
            &self.single_buf,
            mirror,
            self.max_segments,
            &mut self.segments,
        );
        // The arcs replicate under the same spec and against their own share of
        // the cap: whatever the segments left. One budget over both kinds, the
        // way `star_pattern` charges them (ADR-0098).
        let arc_cap = self.max_segments.saturating_sub(self.segments.len());
        let arc_dropped = replicate_mirror(&self.single_arcs, mirror, arc_cap, &mut self.arcs);
        let dropped = dropped + arc_dropped;
        self.mirror_overflow = (dropped > 0).then_some(CapOverflow {
            dropped,
            context: OverflowContext::Mirror(mirror.order),
            cap: self.max_segments,
        });
    }

    fn render(
        &mut self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        aspect: f32,
    ) {
        // Segments carry brightness in their colour; `glow` is the renderer's
        // separate per-segment falloff multiplier (Plan 0038 Phase 1).
        let xform = ViewTransform {
            zoom: self.zoom,
            pan: [self.pan_x, self.pan_y],
            _pad: 0.0,
        };
        let mut renderer = self.renderer.borrow_mut();
        if self.stroke_blend >= super::OPAQUE_BLEND {
            renderer.draw_opaque(
                queue,
                encoder,
                view,
                aspect,
                self.glow,
                self.softness,
                xform,
                &self.segments,
                &self.arcs,
            );
        } else {
            renderer.draw_arcs(
                queue,
                encoder,
                view,
                aspect,
                self.glow,
                self.softness,
                xform,
                &self.segments,
                &self.arcs,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::indexing_slicing)]

    use super::*;

    const SAMPLES: usize = 240;

    /// `spin` integrates rather than multiplying the clock (ADR-0135), and at a
    /// constant rate the two agree — which is what makes "no golden moves" a
    /// property of the arithmetic rather than of the tolerance. Every fixture
    /// binding this scene's `spin` binds a constant.
    #[test]
    fn a_constant_spin_integrates_to_the_multiply_it_replaced() {
        let dt = FALLBACK_DT;
        for rate in [DEFAULT_SPIN, 0.0, 0.4, -0.25] {
            let mut phase = Phase::default();
            let mut time = 0.0f32;
            for _ in 0..600 {
                phase.step(rate, dt);
                time += dt;
            }
            assert!(
                (phase.get() - rate * time).abs() < 1e-3,
                "rate {rate}: integrated {} against the multiply's {}",
                phase.get(),
                rate * time
            );
        }
    }

    /// ...and the property the multiply failed: a `spin` that MOVES advances the
    /// rotation by `spin * dt` whatever the elapsed time. Under `spin * time` the
    /// same change at t = 100 s swings the figure through fifty seconds of
    /// rotation in one frame.
    #[test]
    fn a_spin_change_bends_the_rotation_instead_of_teleporting_it() {
        let dt = FALLBACK_DT;
        let mut phase = Phase::default();
        let mut time = 0.0f32;
        for _ in 0..6_000 {
            phase.step(DEFAULT_SPIN, dt);
            time += dt;
        }
        assert!(time > 99.0, "the fixture must be far from t = 0: {time}");

        let before = phase.get();
        phase.step(1.5, dt);
        let step = phase.get() - before;
        assert!(
            (step - 1.5 * dt).abs() < 1e-4,
            "the rotation advanced {step}, not {}",
            1.5 * dt
        );
        // What the multiply would have done, on the record rather than described.
        let teleport = (1.5 - DEFAULT_SPIN) * time;
        assert!(
            teleport > 100.0,
            "the multiply's one-frame jump at this elapsed time was {teleport} rad"
        );
    }

    fn ramp(hue_spread: f32) -> ColorRamp {
        ColorRamp {
            hue: DEFAULT_HUE,
            hue_spread,
            palette_mix: DEFAULT_PALETTE_MIX,
            palette_steps: palette::DEFAULT_PALETTE_STEPS,
            saturation: DEFAULT_SATURATION,
            brightness: DEFAULT_BRIGHTNESS,
        }
    }

    fn curve(samples: usize, draw_progress: f32, hue_spread: f32) -> Vec<SegmentInstance> {
        let mut out = Vec::with_capacity(samples + 1);
        curves::maurer_rose(
            curves::RoseParams {
                n: DEFAULT_N,
                d: DEFAULT_D,
                phase: DEFAULT_PHASE,
                radial_offset: DEFAULT_RADIAL_OFFSET,
                samples,
                scale: DEFAULT_SCALE,
                rotation: 0.0,
                draw_progress,
                color: [0.0; 3],
                width: 0.01,
            },
            &mut out,
        );
        color_along_path(
            &mut out,
            &Palette::default_spectrum(),
            ramp(hue_spread),
            samples,
        );
        out
    }

    /// Plan 0054 Phase 2 done-when 2 (ADR-0059). The claim is not "colours vary"
    /// — it is that the ramp runs **along the direction of travel**, so the
    /// walk's first chord and its last carry different colours and the walk
    /// between them never doubles back on a colour it already used.
    #[test]
    fn the_spread_colours_the_curve_along_its_direction_of_travel() {
        let swept = curve(SAMPLES, 1.0, 0.5);
        assert_eq!(swept.len(), SAMPLES, "one chord per sample");
        assert_ne!(
            swept[0].color,
            swept[SAMPLES - 1].color,
            "the path's start and end must differ — that is the whole claim"
        );

        // Monotone along the walk. `hue_spread = 0.5` stays inside one traverse
        // of the palette, so the ramp is a strictly advancing sample coordinate
        // and no two chords may share a colour.
        for k in 1..swept.len() {
            assert_ne!(
                swept[k].color,
                swept[k - 1].color,
                "chord {k} repeated chord {}'s colour, so the ramp is not \
                 advancing along the path",
                k - 1
            );
        }
    }

    /// The other half of the superset claim: `hue_spread = 0` is one flat colour
    /// across the whole web — exactly what this scene drew before ADR-0059.
    #[test]
    fn zero_spread_is_one_flat_colour_along_the_whole_path() {
        let flat = curve(SAMPLES, 1.0, 0.0);
        for (k, seg) in flat.iter().enumerate() {
            assert_eq!(seg.color, flat[0].color, "chord {k} must carry the one hue");
        }
    }

    /// The divisor is the **full** curve, not the revealed prefix. A per-beat
    /// `draw_progress` therefore draws the gradient on rather than re-tinting the
    /// chords it already drew — which is what a reveal should look like, and the
    /// bug the obvious `segs.len()` divisor would have shipped.
    #[test]
    fn the_reveal_draws_the_gradient_on_rather_than_re_tinting_it() {
        let full = curve(SAMPLES, 1.0, 0.5);
        let half = curve(SAMPLES, 0.5, 0.5);
        assert!(
            !half.is_empty() && half.len() < full.len(),
            "the probe must actually reveal a prefix"
        );
        for (k, seg) in half.iter().enumerate() {
            assert_eq!(
                seg.color, full[k].color,
                "chord {k} changed colour when the reveal shortened"
            );
        }
        // ...and the revealed half really has only travelled part of the ramp.
        assert_ne!(
            half[half.len() - 1].color,
            full[full.len() - 1].color,
            "a half-drawn curve must not already show the ramp's far end"
        );
    }

    /// Total over the degenerate sample counts an expression can produce: a
    /// one-point or empty walk has no path to ramp along and must not divide by
    /// zero on the render path.
    #[test]
    fn a_degenerate_sample_count_leaves_the_figure_flat() {
        for samples in [0usize, 1, 2] {
            let out = curve(samples, 1.0, 0.9);
            for seg in &out {
                assert!(
                    seg.color.iter().all(|c| c.is_finite()),
                    "samples = {samples} produced a non-finite colour"
                );
            }
        }
    }
}
