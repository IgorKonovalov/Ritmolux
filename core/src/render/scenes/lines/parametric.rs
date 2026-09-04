//! Parametric-curve scene: a pure `t -> (x, y)` curve resampled every frame
//! into the shared [`LineRenderer`] (ADR-0007 parametric build model). Phase 1
//! is hardcoded to one Maurer rose that gently rotates on the deterministic
//! scene clock; Phase 2 makes the curve family and every named parameter
//! preset-driven so audio can sweep it live.
//!
//! ## The colour axis: **position along the traced path** (ADR-0059)
//!
//! This scene honours `[palette]` / `[palette_b]` / `palette_mix` / `hue_spread`
//! / `saturation` through the shared `ColorRamp`, and the axis its generator
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

use super::super::common;
use super::super::{FALLBACK_DT, Phase, Scene};
use super::biarc::Piece;
use super::renderer::{ArcInstance, LineRenderer, SegmentInstance, StrokeMetric};
use super::{
    CapOverflow, ColorRamp, CurveFamily, GeneratorConfig, MirrorSpec, OverflowContext,
    ViewTransform, curves, replicate_mirror,
};
use crate::dsp::AnalysisFrame;
use crate::render::palette::Palette;

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
/// Colour surface (ADR-0021 / ADR-0059), at the value that reproduces the single
/// flat `hue` this scene drew before the palette reached it: no ramp along the path.
/// The palette-A-alone and unmodified-saturation halves of that rest in
/// `scenes::common`, which every system shares them with.
const DEFAULT_HUE_SPREAD: f32 = 0.0;
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
    /// The shared palette knobs (ADR-0021).
    colour: common::PaletteParams,
    /// The shared view transform (ADR-0018).
    pan: common::PanParams,
    hue_spread: f32,
    spin: f32,
    scale: f32,
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
            // The four fit buffers reserve nothing here. Every preset in the
            // shipped library is a chord web, `maurer_rose_pieces` declines the
            // fit before it fills any of them, and at Rich's `max_segments`
            // preallocating all four costs 96 B x 60,000 = 5,760,000 B that is
            // never written. They are reserved on the first frame that actually
            // takes the fitted path — see `reserve_fit_buffers`.
            arcs: Vec::new(),
            single_arcs: Vec::new(),
            pieces: Vec::new(),
            walk: Vec::new(),
            // `points` is the exception and stays preallocated: the walk is
            // written into it on **every** frame, fitted or not.
            //
            // `max_segments + 1`, not `max_segments`. `maurer_rose_pieces`
            // pushes `drawn + 1` points for `drawn` chords — the walk has one
            // more point than it has segments — and `drawn` reaches
            // `max_segments` when a preset binds `samples` at the cap. One short
            // is a reallocation inside a path whose own doc says it is
            // allocation-free.
            points: Vec::with_capacity(max_segments + 1),
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
            colour: common::PaletteParams::new(DEFAULT_HUE, DEFAULT_BRIGHTNESS),
            pan: common::PanParams::default(),
            hue_spread: DEFAULT_HUE_SPREAD,
            spin: DEFAULT_SPIN,
            scale: DEFAULT_SCALE,
            glow: DEFAULT_GLOW,
            softness: super::DEFAULT_SOFTNESS,
            stroke_blend: super::ADDITIVE_BLEND,
            draw_progress: DEFAULT_DRAW_PROGRESS,
            zoom: DEFAULT_ZOOM,
            mirror_order: DEFAULT_MIRROR_ORDER,
            mirror_reflect: DEFAULT_MIRROR_REFLECT,
        }
    }
}

impl ParametricCurveScene {
    /// Give the four fit buffers their steady-state capacity, on the first frame
    /// that actually fits a curve.
    ///
    /// **Why not at load, the way `star.rs` sizes its arc buffers.** A star's
    /// roster is structural: the preset declares its circular motifs, so the
    /// count is known at `configure`. Whether a Maurer walk fits is not declared
    /// — it is read off the walk, per frame, and `d` is an expression that can
    /// cross `curves::SMOOTH_CORNER_SHARE` mid-show.
    /// [`curves::maurer_rose_pieces`] states it: the decision cannot be made at
    /// load, only from the walk in hand.
    ///
    /// So the shape is lazy rather than eager. A chord-web preset — every one in
    /// the shipped library — never reaches here and commits nothing. A preset
    /// that fits pays **one** growth on its first fitted frame and is
    /// allocation-free from the second, which is the property the per-frame path
    /// documents. `reserve_exact`, because these settle at a known ceiling and
    /// have no reason to carry a doubling's slack.
    fn reserve_fit_buffers(&mut self) {
        let cap = self.max_segments;
        if self.pieces.capacity() < cap {
            let extra = cap.saturating_sub(self.pieces.len());
            self.pieces.reserve_exact(extra);
        }
        if self.walk.capacity() < cap {
            let extra = cap.saturating_sub(self.walk.len());
            self.walk.reserve_exact(extra);
        }
        if self.single_arcs.capacity() < cap {
            let extra = cap.saturating_sub(self.single_arcs.len());
            self.single_arcs.reserve_exact(extra);
        }
        if self.arcs.capacity() < cap {
            let extra = cap.saturating_sub(self.arcs.len());
            self.arcs.reserve_exact(extra);
        }
    }

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
                    // A chain is a chain (ADR-0158): every piece but the walk's
                    // two ends continues a neighbour, across a corner as much
                    // as along a curve — the extension is what covers the wedge
                    // between two strokes, and a corner is where there is one.
                    //
                    // The walk is open, so its two outer ends are free.
                    let (ext_a, ext_b) = Piece::chain_extensions(&self.pieces, k, width, false);
                    self.single_buf.push(SegmentInstance {
                        a,
                        b,
                        color,
                        width,
                        alpha: 1.0,
                        ext_a,
                        ext_b,
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
        self.colour.reset();
        self.pan.reset();
        self.hue_spread = DEFAULT_HUE_SPREAD;
        self.spin = DEFAULT_SPIN;
        self.scale = DEFAULT_SCALE;
        self.glow = DEFAULT_GLOW;
        self.softness = super::DEFAULT_SOFTNESS;
        self.stroke_blend = super::ADDITIVE_BLEND;
        self.draw_progress = DEFAULT_DRAW_PROGRESS;
        self.zoom = DEFAULT_ZOOM;
        self.mirror_order = DEFAULT_MIRROR_ORDER;
        self.mirror_reflect = DEFAULT_MIRROR_REFLECT;
    }

    fn set_param(&mut self, name: &str, value: f32) {
        // The shared param blocks first, this scene's own names after
        // (`scenes::common`).
        if self.colour.set(name, value) || self.pan.set(name, value) {
            return;
        }
        match name {
            "n" => self.n = value,
            "d" => self.d = value,
            "phase" => self.phase = value,
            "radial_offset" => self.radial_offset = value,
            "samples" => self.samples = value,
            "thickness" => self.thickness = value,
            "hue_spread" => self.hue_spread = value,
            "spin" => self.spin = value,
            "scale" => self.scale = value,
            "glow" => self.glow = value,
            "softness" => self.softness = value,
            "stroke_blend" => self.stroke_blend = value,
            "draw_progress" => self.draw_progress = value,
            "zoom" => self.zoom = value,
            "mirror_order" => self.mirror_order = value,
            "mirror_reflect" => self.mirror_reflect = value,
            _ => {}
        }
    }

    fn set_palette(&mut self, palette: &Palette) {
        self.palette = palette.clone();
    }

    fn configure(&mut self, cfg: &GeneratorConfig) -> Option<CapOverflow> {
        // A curve preset records its family here (off the hot path). Every other
        // variant belongs to a sibling scene and is not named: matching only
        // this one is what keeps a new variant from editing four scenes that do
        // not use it, and `GeneratorConfig::element_count` is the one place that
        // still has to acknowledge every variant.
        if let GeneratorConfig::Curve { family } = cfg {
            self.family = *family;
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
            hue: self.colour.hue,
            hue_spread: self.hue_spread,
            palette_mix: self.colour.mix,
            palette_steps: self.colour.steps,
            saturation: self.colour.saturation,
            brightness: self.colour.brightness,
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
            self.reserve_fit_buffers();
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
            pan: [self.pan.x, self.pan.y],
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
                StrokeMetric::World,
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
                StrokeMetric::World,
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

    /// The two allocation claims behind this scene's buffer sizing, asserted on
    /// the sampler rather than on the struct — `Vec::new().capacity() == 0` is a
    /// tautology, and what actually matters is what the walk writes.
    ///
    /// **One: a chord web fills none of the fit buffers.** `pieces` and `walk`
    /// are written only when `maurer_rose_pieces` fits the walk to an arc chain,
    /// and every `d` in the shipped library webs. Preallocating them — and the
    /// two arc buffers they feed — to `max_segments` committed Rust heap that is
    /// never written: 96 B x `max_segments`, which at Rich's 60,000 is
    /// 5,760,000 B on top of the buffers that are used.
    ///
    /// **Two: `points` needs `drawn + 1`.** A polyline has one more point than it
    /// has chords, and `drawn` reaches `max_segments` when a preset binds
    /// `samples` at the per-frame clamp. At a capacity of exactly `max_segments`
    /// the last push reallocates, inside a path whose own doc block calls itself
    /// allocation-free.
    #[test]
    fn the_walk_writes_one_more_point_than_it_has_chords_and_a_web_fits_nothing() {
        let web = curves::RoseParams {
            n: 6.0,
            // A shipped-shape chord web: `maurer_rose_pieces` declines this.
            d: 71.0,
            phase: 0.0,
            radial_offset: 0.0,
            samples: SAMPLES,
            scale: 0.9,
            rotation: 0.0,
            draw_progress: 1.0,
            color: [1.0, 1.0, 1.0],
            width: 0.01,
        };

        let mut points = Vec::with_capacity(SAMPLES + 1);
        let mut pieces = Vec::new();
        let mut walk = Vec::new();

        let fitted = curves::maurer_rose_pieces(web, &mut points, &mut pieces, &mut walk);

        assert!(!fitted, "d = 71 is a chord web and declines the fit");
        assert!(
            pieces.is_empty() && walk.is_empty(),
            "a declined fit writes neither buffer, so reserving for them commits \
             heap nothing ever touches"
        );
        assert_eq!(
            pieces.capacity(),
            0,
            "and it does not even grow them: reserving nothing costs nothing"
        );
        assert_eq!(walk.capacity(), 0);

        // The walk itself is always written, fit or no fit, and it is one longer
        // than the chord count.
        assert_eq!(
            points.len(),
            SAMPLES + 1,
            "the walk has one more point than it has chords"
        );
        assert_eq!(
            points.capacity(),
            SAMPLES + 1,
            "so a capacity of `samples` exactly would have reallocated on the \
             final push"
        );
    }

    /// **A fitted chain's `Line` pieces reach their corners, and its two outer
    /// ends stay free** (ADR-0158) — this scene's own joint rule, on
    /// `curve_ionwake`'s rose, which is the figure the fitted path exists for.
    ///
    /// # Why the tangent and not a third point
    ///
    /// A `Line` piece's neighbour in a fitted chain is usually an **arc**, which
    /// has no third vertex to take a direction from — its direction at the joint
    /// is its tangent there. So the rule is stated on tangents, and this asserts
    /// it against `acos` of the same two tangents, which is the other route to
    /// the interior angle.
    ///
    /// # The G1 half is the load-bearing one
    ///
    /// Wherever the fit kept the chain tangent-continuous the two tangents are
    /// equal and the miter is exactly the flat half-width — so a fitted rose
    /// strokes its smooth runs at exactly the length it always did, and only the
    /// breaks the fit made at real corners move. Both halves are asserted:
    /// vacuity here would be a chain with no corner in it at all.
    #[test]
    fn a_fitted_chains_line_pieces_reach_their_corners_and_its_ends_stay_free() {
        use crate::render::scenes::lines::MITER_SLACK;
        use std::f32::consts::PI;

        const W: f32 = 0.01;

        let rose = curves::RoseParams {
            n: 5.0,
            // `curve_ionwake`'s rose: a curve, so `maurer_rose_pieces` takes it.
            d: 2.0,
            phase: 0.0,
            radial_offset: 0.0,
            samples: SAMPLES,
            scale: 0.9,
            rotation: 0.0,
            draw_progress: 1.0,
            color: [1.0, 1.0, 1.0],
            width: W,
        };
        let (mut points, mut pieces, mut at) = (Vec::new(), Vec::new(), Vec::new());
        assert!(
            curves::maurer_rose_pieces(rose, &mut points, &mut pieces, &mut at),
            "a d = 2 rose must be fitted, or this fixture tests nothing"
        );

        // The two outer ends of an open chain are free.
        let last = pieces.len() - 1;
        assert_eq!(
            Piece::chain_extensions(&pieces, 0, W, false).0,
            0.0,
            "the walk's first end has no neighbour to join"
        );
        assert_eq!(
            Piece::chain_extensions(&pieces, last, W, false).1,
            0.0,
            "nor its last"
        );

        let mut straight = 0usize;
        let mut cornered = 0usize;
        for k in 0..pieces.len() {
            let (ext_a, ext_b) = Piece::chain_extensions(&pieces, k, W, false);
            for (side, got, incoming, outgoing) in [
                (
                    "a",
                    ext_a,
                    k.checked_sub(1).map(|j| pieces[j].end_tangent()),
                    Some(pieces[k].start_tangent()),
                ),
                (
                    "b",
                    ext_b,
                    Some(pieces[k].end_tangent()),
                    pieces.get(k + 1).map(|p| p.start_tangent()),
                ),
            ] {
                let (Some(d1), Some(d2)) = (incoming, outgoing) else {
                    continue; // a free end, asserted above
                };
                // The interior angle by `acos` of the turn, where the producer
                // takes a square root of the half-angle identity.
                let turn = (d1[0] * d2[0] + d1[1] * d2[1]).clamp(-1.0, 1.0).acos();
                let want = W / ((PI - turn) * 0.5).sin();
                assert!(
                    (got - want).abs() <= want * MITER_SLACK,
                    "piece {k}'s `{side}` joint carries {got} against the {want} \
                     its {}-degree turn asks for",
                    turn.to_degrees()
                );
                if turn < 1e-4 {
                    straight += 1;
                    assert!(
                        (got - W).abs() <= W * MITER_SLACK,
                        "piece {k}'s `{side}` joint is G1, so its miter must be \
                         exactly the flat half-width {W}, got {got}"
                    );
                } else {
                    cornered += 1;
                }
            }
        }
        assert!(
            straight > 0 && cornered > 0,
            "this chain holds {straight} tangent-continuous joints and \
             {cornered} corners — it must hold some of each, or one of the two \
             halves above was never exercised"
        );
    }

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
            palette_mix: common::DEFAULT_PALETTE_MIX,
            palette_steps: crate::render::palette::DEFAULT_PALETTE_STEPS,
            saturation: common::DEFAULT_SATURATION,
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
