//! Star-pattern scene: a Hankin star rosette built and cached at preset load
//! (ADR-0007 generator build model), cheap to animate. `configure` precomputes
//! a small set of **contact-angle variants** off the hot path; per frame the
//! scene picks the active variant and applies a rotate/scale/colour/draw-on
//! transform (allocation-free). A beat can swap the active variant for a
//! structural accent; continuous params drive rotation, hue, and draw-on.
//!
//! ## The colour axis: **radius from the rosette centre** (ADR-0059)
//!
//! This scene honours `[palette]` / `[palette_b]` / `palette_mix` / `hue_spread`
//! / `saturation` through the shared [`ColorRamp`]. Its declared axis is
//! **normalized radius**: a Hankin rosette is rotationally symmetric about the
//! frame centre, so a path-position axis would paint an arbitrary seam across a
//! figure with no beginning, and radius is the only ordering the construction
//! itself supplies.
//!
//! **And on the current construction that ramp is identically flat — measured,
//! not estimated.** The rosette is `2n` *congruent* segments: each runs from a
//! contact point on the unit circle to a petal tip at radius
//! `sin(a) / sin(pi/n + a)`, and every one of them is a rotation or reflection of
//! every other about a centre that `normalize_fit` leaves at the origin (every
//! tiling order the loader accepts — 4, 6, 8, 12 — is even, so the figure's
//! bounding box is centred). So each segment's radial interval is the *same*
//! interval, and one colour per segment has nothing to distinguish. Measured
//! across both shipped presets and all three of their variants, the spread of
//! segment radii is **1.2e-7**, which is f32 noise and not a range.
//!
//! The *figure's* radial extent is a different quantity, and it is the one
//! design-backlog 0007 reported as a "hollow ring": at `star_rosette`'s
//! 12-fold / 20-degree rosette the strokes live between radius 0.54 and 0.90, so
//! the inner **60%** of the disc is empty, and `star_lantern`'s 55-degree variant
//! empties **87%** of it. That is real, and it is the interior question — not
//! something a colour axis can answer.
//!
//! `hue_spread` is therefore a **no-op on this scene until its interior is
//! redesigned**, and that is stated here and in `presets/README.md` rather than
//! shipped as a lever that quietly does nothing. What the scene does gain today
//! is `[palette]` itself: the rosette can finally be an ember or an ice figure
//! instead of a point on the built-in cosine. The interior question — more
//! tilings, an off-centre construction, drawing the underlying tiling grid — is
//! the open half of design-backlog 0007, and the ramp comes alive on its own the
//! day a construction puts segments at different radii.

// Hot-path panic-denial pragma: `update`/`render` run every displayed frame.
// `configure` (the Hankin construction) is build-time but colocated, so it
// obeys the same panic-free bar.
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
    CapOverflow, ColorRamp, GeneratorConfig, MirrorSpec, OverflowContext, ViewTransform, hankin,
    replicate_mirror, transform_cached, turtle,
};
use crate::dsp::AnalysisFrame;
use crate::render::palette::Palette;

/// Maps `thickness` to an NDC-y half-width (see the parametric scene).
const WIDTH_SCALE: f32 = 0.003;

/// Contact-angle offsets (degrees) for the precomputed variants a beat swaps
/// between — a pointier and a blunter star around the preset's base angle.
const VARIANT_OFFSETS_DEG: [f32; 3] = [-24.0, 0.0, 24.0];
/// Contact angle is clamped to this range for a sensible star.
const CONTACT_MIN_DEG: f32 = 8.0;
const CONTACT_MAX_DEG: f32 = 80.0;

const DEFAULT_VARIANT: f32 = 1.0;
const DEFAULT_ROTATION: f32 = 0.0;
const DEFAULT_HUE: f32 = 0.5;
/// Colour surface (ADR-0021 / ADR-0059), all three at the value that reproduces
/// the single flat `hue` this scene drew before the palette reached it.
const DEFAULT_HUE_SPREAD: f32 = 0.0;
const DEFAULT_SATURATION: f32 = 1.0;
const DEFAULT_PALETTE_MIX: f32 = 0.0;
const DEFAULT_DRAW_PROGRESS: f32 = 1.0;
const DEFAULT_THICKNESS: f32 = 2.0;
const DEFAULT_SCALE: f32 = 1.0;
const DEFAULT_BRIGHTNESS: f32 = 1.0;
/// The line renderer's **per-segment falloff** multiplier (Plan 0038 Phase 1) —
/// not a post-process bloom. `1.0` is the value this scene passed as a literal
/// before it was bound, so the default is exactly today's look.
const DEFAULT_GLOW: f32 = 1.0;
// Shared view transform (ADR-0018): identity by default.
const DEFAULT_ZOOM: f32 = 1.0;
const DEFAULT_PAN: f32 = 0.0;
// Geometry mirror (Phase 4): identity by default.
const DEFAULT_MIRROR_ORDER: f32 = 1.0;
const DEFAULT_MIRROR_REFLECT: f32 = 0.0;

/// A generator scene drawing a Hankin star pattern.
pub struct StarPatternScene {
    /// The single line renderer, shared with the other line scenes (ADR-0007).
    renderer: Rc<RefCell<LineRenderer>>,
    /// One cached rosette per contact-angle variant, built in `configure`.
    cached: Vec<Vec<SegmentInstance>>,
    /// Each cached rosette's per-segment **normalized radius** (ADR-0059's
    /// colour axis), index-aligned with [`cached`](Self::cached). A load-time
    /// quantity: `transform_cached`'s rotate and uniform scale leave a
    /// normalized radius unchanged, so nothing per frame can move it.
    cached_radii: Vec<Vec<f32>>,
    /// Per-segment stroke colour for the active rosette, rebuilt each frame into
    /// a buffer sized at build time so the fill allocates nothing.
    colors: Vec<[f32; 3]>,
    /// Reused per-frame draw buffer — the mirrored geometry actually rendered.
    /// Preallocated so replication allocates nothing on the hot path.
    draw_buf: Vec<SegmentInstance>,
    /// Reused buffer for the single (pre-mirror) transformed variant, replicated
    /// into [`draw_buf`](Self::draw_buf) by [`replicate_mirror`]. Preallocated.
    single_buf: Vec<SegmentInstance>,
    /// The active tier's segment ceiling
    /// ([`TierConfig::max_segments`](crate::render::TierConfig::max_segments)),
    /// resolved once at construction (Plan 0044). A field rather than a constant
    /// so the tier can raise it; the buffers above are preallocated to it, which
    /// is what keeps the per-frame replication allocation-free.
    max_segments: usize,
    /// Set when this frame's mirror replication overflowed the cap (Phase 4).
    mirror_overflow: Option<CapOverflow>,
    /// Shared scene clock (seconds).
    time: f32,
    /// The preset's baked colour LUT (ADR-0021), sampled on the CPU per segment.
    palette: Palette,
    variant: f32,
    rotation: f32,
    hue: f32,
    hue_spread: f32,
    saturation: f32,
    palette_mix: f32,
    draw_progress: f32,
    thickness: f32,
    scale: f32,
    brightness: f32,
    glow: f32,
    zoom: f32,
    pan_x: f32,
    pan_y: f32,
    mirror_order: f32,
    mirror_reflect: f32,
}

impl StarPatternScene {
    /// Build the scene over the shared line renderer, preallocating the draw
    /// buffer. No pattern is built until a preset configures one.
    pub fn new(renderer: Rc<RefCell<LineRenderer>>, max_segments: usize) -> Self {
        Self {
            renderer,
            cached: Vec::new(),
            cached_radii: Vec::new(),
            colors: Vec::new(),
            draw_buf: Vec::with_capacity(max_segments),
            single_buf: Vec::with_capacity(max_segments),
            max_segments,
            mirror_overflow: None,
            time: 0.0,
            // Replaced by the preset's palette on the next switch; the default
            // is the engine cosine, so an unconfigured scene still colours.
            palette: Palette::default_spectrum(),
            variant: DEFAULT_VARIANT,
            rotation: DEFAULT_ROTATION,
            hue: DEFAULT_HUE,
            hue_spread: DEFAULT_HUE_SPREAD,
            saturation: DEFAULT_SATURATION,
            palette_mix: DEFAULT_PALETTE_MIX,
            draw_progress: DEFAULT_DRAW_PROGRESS,
            thickness: DEFAULT_THICKNESS,
            scale: DEFAULT_SCALE,
            brightness: DEFAULT_BRIGHTNESS,
            glow: DEFAULT_GLOW,
            zoom: DEFAULT_ZOOM,
            pan_x: DEFAULT_PAN,
            pan_y: DEFAULT_PAN,
            mirror_order: DEFAULT_MIRROR_ORDER,
            mirror_reflect: DEFAULT_MIRROR_REFLECT,
        }
    }

    /// Build + cache one rosette per contact-angle variant, with its radial
    /// colour axis. Off the hot path.
    fn build(&mut self, n: u32, contact_angle_deg: f32) {
        self.cached.clear();
        self.cached_radii.clear();
        let mut widest = 0usize;
        for offset in VARIANT_OFFSETS_DEG {
            let ang = (contact_angle_deg + offset)
                .clamp(CONTACT_MIN_DEG, CONTACT_MAX_DEG)
                .to_radians();
            let mut segs = Vec::new();
            hankin::star_rosette(n, ang, &mut segs);
            turtle::normalize_fit(&mut segs, 0.9);
            let mut radii = Vec::new();
            normalized_radii(&segs, &mut radii);
            widest = widest.max(segs.len());
            self.cached.push(segs);
            self.cached_radii.push(radii);
        }
        self.colors.clear();
        self.colors.resize(widest, [0.0; 3]);
    }
}

/// Each segment's **normalized radius** from the rosette centre, min-max mapped
/// onto `0..1` across the figure, into `out` (cleared first).
///
/// A segment's representative radius is its midpoint's distance from the origin
/// — the rosette is already centred there by `normalize_fit`. Min-max rather than
/// "radius over the outer extent" so that `u = 0` is the innermost segment and
/// `u = 1` the outermost, matching what `hue_spread` means on every other line
/// scene: the palette travels across the figure, not across the empty disc
/// around it.
///
/// **When the figure has no radial spread at all — which is every Hankin rosette
/// the current construction produces, see the module docs — every segment gets
/// `u = 0`.** That makes `hue_spread` exactly the identity there rather than a
/// hidden constant hue shift, which is the honest degenerate answer: no range,
/// no ramp.
pub(crate) fn normalized_radii(segs: &[SegmentInstance], out: &mut Vec<f32>) {
    out.clear();
    let radius = |s: &SegmentInstance| -> f32 {
        let (x, y) = (0.5 * (s.a[0] + s.b[0]), 0.5 * (s.a[1] + s.b[1]));
        (x * x + y * y).sqrt()
    };
    let mut lo = f32::INFINITY;
    let mut hi = f32::NEG_INFINITY;
    for seg in segs {
        let r = radius(seg);
        lo = lo.min(r);
        hi = hi.max(r);
    }
    let span = hi - lo;
    // `RADIAL_FLOOR` is a *spread*, not a radius: below it the figure has no
    // radial ordering to colour along, so the ramp collapses instead of
    // amplifying float noise into a full palette sweep.
    let scale = if span.is_finite() && span > RADIAL_FLOOR {
        1.0 / span
    } else {
        0.0
    };
    for seg in segs {
        out.push((radius(seg) - lo) * scale);
    }
}

/// The smallest radial spread (in the fit-normalized world, where the figure
/// spans at most `2 * 0.9`) that counts as a range worth colouring along.
const RADIAL_FLOOR: f32 = 1e-4;

/// Parameter vocabulary — see [`fragment_field::PARAMS`](crate::render::scenes::fragment_field::PARAMS).
/// **Keep in sync with `set_param` below.**
pub const PARAMS: &[&str] = &[
    "variant",
    "rotation",
    "hue",
    "hue_spread",
    "saturation",
    "palette_mix",
    "draw_progress",
    "thickness",
    "scale",
    "brightness",
    "glow",
    "zoom",
    "pan_x",
    "pan_y",
    "mirror_order",
    "mirror_reflect",
];

impl Scene for StarPatternScene {
    fn name(&self) -> &'static str {
        "star pattern"
    }

    fn set_time(&mut self, time: f32) {
        self.time = time;
    }

    fn reset_params(&mut self) {
        self.variant = DEFAULT_VARIANT;
        self.rotation = DEFAULT_ROTATION;
        self.hue = DEFAULT_HUE;
        self.hue_spread = DEFAULT_HUE_SPREAD;
        self.saturation = DEFAULT_SATURATION;
        self.palette_mix = DEFAULT_PALETTE_MIX;
        self.draw_progress = DEFAULT_DRAW_PROGRESS;
        self.thickness = DEFAULT_THICKNESS;
        self.scale = DEFAULT_SCALE;
        self.brightness = DEFAULT_BRIGHTNESS;
        self.glow = DEFAULT_GLOW;
        self.zoom = DEFAULT_ZOOM;
        self.pan_x = DEFAULT_PAN;
        self.pan_y = DEFAULT_PAN;
        self.mirror_order = DEFAULT_MIRROR_ORDER;
        self.mirror_reflect = DEFAULT_MIRROR_REFLECT;
    }

    fn set_param(&mut self, name: &str, value: f32) {
        match name {
            "variant" => self.variant = value,
            "rotation" => self.rotation = value,
            "hue" => self.hue = value,
            "hue_spread" => self.hue_spread = value,
            "saturation" => self.saturation = value,
            "palette_mix" => self.palette_mix = value,
            "draw_progress" => self.draw_progress = value,
            "thickness" => self.thickness = value,
            "scale" => self.scale = value,
            "brightness" => self.brightness = value,
            "glow" => self.glow = value,
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
        // Build + cache the star variants off the hot path. Other config
        // variants belong to sibling line scenes and are ignored.
        match cfg {
            GeneratorConfig::Star {
                order,
                contact_angle_deg,
            } => self.build(*order, *contact_angle_deg),
            GeneratorConfig::Curve { .. }
            | GeneratorConfig::LSystem { .. }
            | GeneratorConfig::Particles { .. }
            | GeneratorConfig::Spectrum { .. } => {}
        }
        // A rosette is `2 * n` segments for the small regular tilings v1 allows
        // (n <= 12), far under the cap — no truncation to surface.
        None
    }

    fn mirror_overflow(&self) -> Option<&CapOverflow> {
        self.mirror_overflow.as_ref()
    }

    fn update(&mut self, _frame: &AnalysisFrame) {
        let variants = self.cached.len();
        if variants == 0 {
            self.draw_buf.clear();
            return;
        }
        let idx = (self.variant.max(0.0) as usize).min(variants.saturating_sub(1));
        let Some(base) = self.cached.get(idx) else {
            self.draw_buf.clear();
            return;
        };

        // The radial colour ramp (ADR-0059). One sample per segment, into a
        // buffer sized at build time. The radii are load-time values because a
        // rotate plus a uniform scale leaves a normalized radius unchanged.
        let ramp = ColorRamp {
            hue: self.hue,
            hue_spread: self.hue_spread,
            palette_mix: self.palette_mix,
            saturation: self.saturation,
            brightness: self.brightness,
        };
        if let Some(radii) = self.cached_radii.get(idx) {
            for (slot, &u) in self.colors.iter_mut().zip(radii) {
                *slot = ramp.at(&self.palette, u);
            }
        }
        let inner = self.colors.first().copied().unwrap_or([1.0; 3]);

        let width = (self.thickness * WIDTH_SCALE).max(0.0005);
        transform_cached(
            base,
            self.rotation,
            self.scale,
            inner,
            width,
            self.draw_progress,
            &mut self.single_buf,
        );
        // `transform_cached` keeps a prefix (the `draw_progress` reveal), so
        // segment `i` of the output is still segment `i` of the cached rosette.
        for (seg, &color) in self.single_buf.iter_mut().zip(&self.colors) {
            seg.color = color;
        }
        // Replicate the single transformed variant under the geometry mirror
        // (Phase 4). At the default identity spec, skip it: replication would copy
        // the whole segment set into a second buffer to produce exactly what it
        // was given, so swap instead — O(1), and both buffers were preallocated to
        // `max_segments`, so neither can grow later. `transform_cached` clears
        // before it fills, so whatever lands back in `single_buf` is overwritten.
        let mirror = MirrorSpec::from_params(self.mirror_order, self.mirror_reflect);
        if mirror.is_identity() {
            debug_assert!(
                self.single_buf.len() <= self.max_segments,
                "the cached variant is capped at load, so identity cannot truncate"
            );
            std::mem::swap(&mut self.single_buf, &mut self.draw_buf);
            self.mirror_overflow = None;
            return;
        }
        let dropped = replicate_mirror(
            &self.single_buf,
            mirror,
            self.max_segments,
            &mut self.draw_buf,
        );
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
        let xform = ViewTransform {
            zoom: self.zoom,
            pan: [self.pan_x, self.pan_y],
            _pad: 0.0,
        };
        self.renderer.borrow_mut().draw(
            queue,
            encoder,
            view,
            aspect,
            self.glow,
            xform,
            &self.draw_buf,
        );
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::indexing_slicing)]

    use super::*;

    /// Build one rosette exactly as `build` does.
    fn rosette(n: u32, contact_deg: f32) -> Vec<SegmentInstance> {
        let mut segs = Vec::new();
        hankin::star_rosette(n, contact_deg.to_radians(), &mut segs);
        turtle::normalize_fit(&mut segs, 0.9);
        segs
    }

    fn radius(s: &SegmentInstance) -> f32 {
        let (x, y) = (0.5 * (s.a[0] + s.b[0]), 0.5 * (s.a[1] + s.b[1]));
        (x * x + y * y).sqrt()
    }

    /// **Plan 0054 Phase 2's honesty clause, as a measurement rather than a
    /// claim.** ADR-0059 gives this scene a radial colour axis and its own
    /// Consequences call that axis "the weakest of the four"; the number is
    /// worse than "weak" and it is pinned here so nobody has to re-derive it.
    ///
    /// A Hankin rosette is `2n` **congruent** segments about a centre
    /// `normalize_fit` leaves at the origin (every accepted tiling order is
    /// even, so the bounding box is centred). Each therefore occupies the *same*
    /// radial interval, and one colour per segment has nothing to tell them
    /// apart — the spread across segments is zero at every order and every
    /// contact angle, so `hue_spread` is exactly the identity on this scene.
    ///
    /// **If this test starts failing, the interior work landed and the ramp came
    /// alive.** That is the good outcome; re-point the assertion then.
    #[test]
    fn the_radial_axis_has_no_spread_on_the_current_rosette() {
        for n in [4u32, 6, 8, 12] {
            for contact in [CONTACT_MIN_DEG, 12.0, 20.0, 30.0, 54.0, CONTACT_MAX_DEG] {
                let segs = rosette(n, contact);
                assert_eq!(segs.len(), 2 * n as usize, "{n}-fold at {contact} deg");

                let radii: Vec<f32> = segs.iter().map(radius).collect();
                let lo = radii.iter().copied().fold(f32::INFINITY, f32::min);
                let hi = radii.iter().copied().fold(f32::NEG_INFINITY, f32::max);
                assert!(
                    hi - lo < RADIAL_FLOOR,
                    "{n}-fold at {contact} deg: segment radii span {lo}..{hi}, so \
                     the radial axis is no longer flat and this measurement is stale"
                );

                // ...and the normalization reports that as "no ramp" rather than
                // amplifying the float noise into a palette sweep.
                let mut u = Vec::new();
                normalized_radii(&segs, &mut u);
                assert_eq!(u.len(), segs.len());
                assert!(
                    u.iter().all(|&x| x == 0.0),
                    "{n}-fold at {contact} deg: a figure with no radial spread \
                     must collapse to u = 0, not to noise"
                );
            }
        }
    }

    /// The figure's own radial extent — the "hollow ring" measurement — is a
    /// different quantity from the per-segment spread above, and it is the one
    /// design-backlog 0007 reported. The rosette's vertices span
    /// `sin(a) / sin(pi/n + a)` to `1` before the fit; everything inside is
    /// empty. Recorded so the interior work has a starting number.
    #[test]
    fn the_rosette_leaves_its_interior_empty() {
        let n = 12u32;
        for contact in [12.0f32, 20.0, 30.0] {
            let segs = rosette(n, contact);
            let vertex = |p: [f32; 2]| (p[0] * p[0] + p[1] * p[1]).sqrt();
            let mut lo = f32::INFINITY;
            let mut hi = f32::NEG_INFINITY;
            for seg in &segs {
                for p in [seg.a, seg.b] {
                    lo = lo.min(vertex(p));
                    hi = hi.max(vertex(p));
                }
            }
            let a = contact.to_radians();
            let predicted = a.sin() / (std::f32::consts::PI / n as f32 + a).sin();
            assert!(
                (lo / hi - predicted).abs() < 1e-3,
                "{n}-fold at {contact} deg: inner radius fraction {} should be \
                 sin(a)/sin(pi/n + a) = {predicted}",
                lo / hi
            );
            assert!(
                lo / hi > 0.4,
                "the interior is empty from the centre out to {}",
                lo / hi
            );
        }
    }

    /// The degenerate guard is a *spread* floor, not a radius floor: a figure
    /// that genuinely does span radii still ramps across its full range.
    #[test]
    fn a_figure_with_radial_spread_still_ramps_across_it() {
        let seg = |a: [f32; 2], b: [f32; 2]| SegmentInstance {
            a,
            b,
            color: [0.0; 3],
            width: 0.01,
            joined: 0,
        };
        // Three concentric chords at radii 0.2, 0.5 and 0.9.
        let segs = vec![
            seg([0.2, 0.0], [0.2, 0.0]),
            seg([0.5, 0.0], [0.5, 0.0]),
            seg([0.9, 0.0], [0.9, 0.0]),
        ];
        let mut u = Vec::new();
        normalized_radii(&segs, &mut u);
        assert!((u[0] - 0.0).abs() < 1e-5, "innermost is 0, got {}", u[0]);
        assert!((u[2] - 1.0).abs() < 1e-5, "outermost is 1, got {}", u[2]);
        assert!(
            u[1] > u[0] && u[1] < u[2],
            "the middle chord lands between, got {}",
            u[1]
        );
    }
}
