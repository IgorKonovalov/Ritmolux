//! Star-pattern scene: a Hankin star rosette built from a **continuous contact
//! angle** and cached (ADR-0007 generator build model), cheap to animate. Per
//! frame the scene resolves `variant` to an angle, reuses the cached rosette
//! unless the request has moved more than one step, and applies a
//! rotate/scale/colour/draw-on transform (allocation-free).
//!
//! ## `variant` is a contact angle, not an index (ADR-0060)
//!
//! `variant` maps linearly onto a contact-angle offset rather than flooring into
//! one of three precomputed rosettes. `0`, `1` and `2` land on exactly the
//! `-24 / 0 / +24` degree offsets of those three, so **a preset binding integers
//! draws the rosette it names**, while a fractional value is a real rosette in
//! between and `[smoothing]` has something to interpolate.
//!
//! The cache stays, keyed on the built angle with **hysteresis**: a request more
//! than `STEP_DEG` from the built angle rebuilds, anything nearer reuses. That
//! is what keeps generator work off the hot path (ADR-0007) now that a bound
//! param can reach it.
//!
//! **The step is measured, not assumed** (ADR-0060 leaves the number open). At
//! `0.1` degrees:
//!
//! - *Invisible in motion.* The worst case is the sharpest reachable rosette:
//!   a 12-fold star at an 11-degree contact angle moves a vertex 11.0 px per
//!   degree at 1080p, so one step is **1.14 px** there and 0.67 / 0.25 px at the
//!   20 / 55-degree angles the two shipped presets use — under a stroke that is
//!   itself several pixels of glow wide.
//! - *Cannot rebuild every frame.* The full `variant` range is 48 degrees of
//!   contact angle, i.e. 480 steps, so a sweep slower than 8 s at 60 fps rebuilds
//!   on a fraction of its frames. Both shipped presets sweep in ~45 s, which is
//!   about one rebuild every six frames.
//! - *And a rebuild fits the frame anyway.* Measured at the loader's maximum
//!   order (`n = 12`, so `2n = 24` segments): **0.34 us**, 0.002% of a 16.7 ms
//!   frame. A hypothetical rosette filled to the floor tier's whole
//!   20 000-segment cap (`n = 10 000`, unreachable from the preset surface, whose
//!   tilings stop at 12) costs 282 us — 1.7% of a frame — so even the ceiling
//!   this scene cannot reach is inside budget.
//!
//! ## The colour axis: **radius from the rosette centre** (ADR-0059)
//!
//! This scene honours `[palette]` / `[palette_b]` / `palette_mix` / `hue_spread`
//! / `saturation` through the shared `ColorRamp`, on a **normalized radius**
//! axis: a Hankin rosette is rotationally symmetric about the frame centre, so
//! radius is the only ordering the construction itself supplies.
//!
//! **On the bare rosette that ramp is identically flat — measured, not
//! estimated.** The rosette is `2n` *congruent* segments: each runs from a
//! contact point on the unit circle to a petal tip at radius
//! `sin(a) / sin(pi/n + a)`, and every one is a rotation or reflection of every
//! other about a centre that `normalize_fit` leaves at the origin (every tiling
//! order the loader accepts — 4, 6, 8, 12 — is even, so the bounding box is
//! centred). Each segment's radial interval is therefore the *same* interval, and
//! one colour per segment has nothing to distinguish. Across both shipped presets
//! and all three of their variants the spread of segment radii is **1.2e-7**,
//! which is f32 noise and not a range.
//!
//! The *figure's* radial extent is a different quantity, and it is the "hollow
//! ring" of design-backlog 0007: at `star_rosette`'s 12-fold / 20-degree rosette
//! the strokes live between radius 0.54 and 0.90, so the inner **60%** of the
//! disc is empty, and `star_lantern`'s 55-degree variant empties **87%**. That is
//! the interior question, not something a colour axis can answer.
//!
//! `hue_spread` is therefore a **no-op on a rings-less preset**, stated here and
//! in `presets/README.md` rather than shipped as a lever that quietly does
//! nothing. What such a preset does gain is `[palette]` itself.
//!
//! ## The interior: rings of motifs (ADR-0079)
//!
//! `[generator] rings` is an optional roster of concentric rings — `{ motif,
//! count, radius, scale, phase }` each — drawn through the same [`LineRenderer`]
//! alongside, or instead of, the interlace. It answers design-backlog 0007's
//! hollow-ring half, and it is *placement* rather than construction: copy `i` of
//! a ring of `k` sits at `2*pi*i/k + phase`, scaled by `scale`, at distance
//! `radius`, in the same fit-normalized world the rosette lands in (the rosette
//! spans `+/- 0.9`, so a `radius` near `0.9` sits on its rim and anything smaller
//! is genuinely interior).
//!
//! Two consequences worth stating where they can be read:
//!
//! - **With `rings` absent nothing here runs at all**, and the scene draws the
//!   Hankin path segment for segment — the rings live in their own buffer and the
//!   combined one is never even allocated.
//! - **With `rings` present the radial colour axis stops being degenerate.** The
//!   ramp is computed over the *combined* figure, which really does span radii,
//!   so `hue_spread` becomes a live lever on exactly the presets that have an
//!   interior to spread across.
//!
//! The motif roster is **closed** ([`Motif`]): a look outside it routes back
//! through `architect` + `dev` rather than being added on request (ADR-0079).
//!
//! ## The rings move: three levers, and why two of them are radial
//!
//! The roster stays structural; what moves is a `RingMotion` applied to it.
//! `ring_phase` turns **alternate rings in opposite directions**, `ring_spread`
//! multiplies every radius about the centre, and `ring_scale` multiplies every
//! motif's size. All three default to `RingMotion::STATIC`, the exact identity
//! (`+ 0`, `* 1`, `* 1`), so a preset that binds none of them draws the static
//! ornament bit for bit.
//!
//! **The radial pair is not a garnish, and this is the one design note worth
//! reading before authoring a mandala.** `core/tests/animation.rs` captures at
//! 96x96 and diffs whole frames, and a ring mandala is *more* rotationally
//! symmetric than the bare rosette design-backlog 0009 measured — an 18- and
//! 24-fold figure turned by any angle lands almost on top of itself, so **spin
//! alone reads as frozen to that gate and, at a distance, to the eye**.
//! `ring_spread` and `ring_scale` change what the figure *is* at each radius
//! rather than where it sits, so they move pixels. A shipped mandala carries its
//! animation on those and spends `ring_phase` on the counter-rotation, which is
//! the ornamental gesture rather than the liveness.
//!
//! Like the rosette, the ornament is **rebuilt under hysteresis**: a motion
//! further than one step (`RING_PHASE_STEP` and friends) from what is held
//! rebuilds, anything nearer reuses. A preset binding none of the three never
//! rebuilds after `configure` — but one that *animates* a lever re-places its
//! ornament on most frames, which is affordable rather than free. See
//! `RING_PHASE_STEP` for the measurement.

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
use std::f32::consts::{PI, TAU};
use std::rc::Rc;
use std::sync::OnceLock;

use super::super::Scene;
use super::super::common;
use super::biarc::{self, Piece};
use super::renderer::{ArcInstance, LineRenderer, SegmentInstance, StrokeMetric, miter_extension};
use super::{
    CapOverflow, ColorRamp, GeneratorConfig, MirrorSpec, OverflowContext, PLACEHOLDER_WIDTH,
    ViewTransform, hankin, replicate_mirror, transform_cached, turtle,
};
use crate::dsp::AnalysisFrame;
use crate::render::palette::Palette;

/// How far (degrees of contact angle) `variant` reaches either side of the
/// preset's base angle — a pointier star at `0`, a blunter one at `2`. This is
/// the span of the three precomputed variants (`-24 / 0 / +24`), so a preset
/// binding integers draws one of them exactly (ADR-0060).
const VARIANT_SPAN_DEG: f32 = 24.0;
/// The `variant` value that means "the preset's own `contact_angle_deg`" — the
/// middle of the range, and this scene's default.
const VARIANT_CENTER: f32 = 1.0;
/// Contact angle is clamped to this range for a sensible star.
const CONTACT_MIN_DEG: f32 = 8.0;
const CONTACT_MAX_DEG: f32 = 80.0;

/// The rebuild hysteresis: a requested contact angle further than this from the
/// built one rebuilds the rosette, anything nearer reuses it. See the module
/// docs for the measurement behind the number — it is the resolution of the
/// morph, not a shape (the ADR-0037 habit).
const STEP_DEG: f32 = 0.1;

const DEFAULT_VARIANT: f32 = 1.0;
const DEFAULT_ROTATION: f32 = 0.0;
const DEFAULT_HUE: f32 = 0.5;
/// Colour surface (ADR-0021 / ADR-0059), at the value that reproduces the single
/// flat `hue` this scene drew before the palette reached it: no ramp along the
/// ring axis. The palette-A-alone and unmodified-saturation halves of that rest
/// in `scenes::common`, which every system shares them with.
const DEFAULT_HUE_SPREAD: f32 = 0.0;
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
// Geometry mirror (Phase 4): identity by default.
const DEFAULT_MIRROR_ORDER: f32 = 1.0;
const DEFAULT_MIRROR_REFLECT: f32 = 0.0;
// The ring levers (Plan 0065 Phase 4), all at the exact identity so a preset
// that binds none of them draws the static roster it declared.
const DEFAULT_RING_PHASE: f32 = 0.0;
const DEFAULT_RING_SPREAD: f32 = 1.0;
const DEFAULT_RING_SCALE_PARAM: f32 = 1.0;

/// A generator scene drawing a Hankin star pattern.
pub struct StarPatternScene {
    /// The single line renderer, shared with the other line scenes (ADR-0007).
    renderer: Rc<RefCell<LineRenderer>>,
    /// The one cached rosette, with the contact angle it was built at
    /// (ADR-0060). Rebuilt only when `variant` walks it more than
    /// [`STEP_DEG`] away.
    cache: RosetteCache,
    /// The preset's `[generator] contact_angle_deg` and star order, from
    /// `configure`. `variant` offsets the angle around this.
    order: u32,
    base_contact_deg: f32,
    /// The validated roster this preset declared (ADR-0079) — **structural**,
    /// read once at load and never bindable. Empty for a rings-less preset.
    rings: Vec<RingSpec>,
    /// The ring ornament (ADR-0079): [`rings`](Self::rings) placed, under the
    /// motion it was last built at. **Empty is the signal** that this preset
    /// declared no `rings`, and every ring-aware branch below keys off it, so a
    /// rings-less preset takes the ring-free path end to end.
    ring_segments: Vec<SegmentInstance>,
    /// The ornament's **arcs** — the circular motifs, one instance each
    /// (ADR-0098). A ring of `circle` or `arc` puts nothing in
    /// [`ring_segments`](Self::ring_segments) and everything here, so the
    /// ornament is present when *either* is non-empty; see
    /// [`has_ornament`](Self::has_ornament).
    ring_arcs: Vec<ArcInstance>,
    /// The [`RingMotion`] [`ring_segments`](Self::ring_segments) holds, i.e. this
    /// ornament's half of the hysteresis (Phase 4). A preset binding none of the
    /// three levers never leaves [`RingMotion::STATIC`] and so never rebuilds.
    built_motion: RingMotion,
    /// How many times the ornament has been rebuilt. Not read by the render path
    /// — it is the observable the hysteresis test asserts on, exactly as
    /// [`RosetteCache::rebuilds`] is.
    ring_rebuilds: u64,
    /// The rosette and the ornament concatenated — the geometry actually
    /// transformed per frame when both exist. Allocated only when `rings` is
    /// present, and refilled when the rosette rebuilds under it (the rings do not
    /// move, but a min-max radial ramp over the pair does).
    combined: Vec<SegmentInstance>,
    /// [`normalized_radii`] over [`combined`](Self::combined) — ADR-0059's colour
    /// axis across the *whole* figure rather than across the rosette alone.
    combined_radii: Vec<f32>,
    /// The ornament's arcs, alongside [`combined`](Self::combined). The rosette
    /// is an interlace of straight chords and contributes none, so this is
    /// [`ring_arcs`](Self::ring_arcs) under the shared cap.
    combined_arcs: Vec<ArcInstance>,
    /// Their share of the same radial colour axis — normalized against the
    /// **whole** figure, both kinds together, or a mandala's circles would be
    /// coloured on a different scale from its interlace.
    combined_arc_radii: Vec<f32>,
    /// Per-segment stroke colour for the cached rosette, rebuilt each frame into
    /// a buffer sized at build time so the fill allocates nothing.
    colors: Vec<[f32; 3]>,
    /// The same, per arc.
    arc_colors: Vec<[f32; 3]>,
    /// Reused per-frame draw buffer — the mirrored geometry actually rendered.
    /// Preallocated so replication allocates nothing on the hot path.
    draw_buf: Vec<SegmentInstance>,
    /// Reused buffer for the single (pre-mirror) transformed variant, replicated
    /// into [`draw_buf`](Self::draw_buf) by [`replicate_mirror`]. Preallocated.
    single_buf: Vec<SegmentInstance>,
    /// The arc halves of [`draw_buf`](Self::draw_buf) and
    /// [`single_buf`](Self::single_buf). Sized at `configure` from the roster
    /// the preset declares rather than to `max_segments`: arcs are produced only
    /// by `build_rings`, so their count is known at load and reserving the whole
    /// segment ceiling for them would be most of a megabyte nothing uses.
    arc_draw_buf: Vec<ArcInstance>,
    single_arc_buf: Vec<ArcInstance>,
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
    /// The shared palette knobs (ADR-0021).
    colour: common::PaletteParams,
    /// The shared view transform (ADR-0018).
    pan: common::PanParams,
    hue_spread: f32,
    draw_progress: f32,
    thickness: f32,
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
    zoom: f32,
    mirror_order: f32,
    mirror_reflect: f32,
    ring_phase: f32,
    ring_spread: f32,
    ring_scale: f32,
}

impl StarPatternScene {
    /// Build the scene over the shared line renderer, preallocating the draw
    /// buffer. No pattern is built until a preset configures one.
    pub fn new(renderer: Rc<RefCell<LineRenderer>>, max_segments: usize) -> Self {
        Self {
            renderer,
            cache: RosetteCache::default(),
            order: 0,
            base_contact_deg: 0.0,
            // Sized at `configure` from the roster the preset actually declares —
            // a rings-less preset never allocates any of these.
            rings: Vec::new(),
            ring_segments: Vec::new(),
            ring_arcs: Vec::new(),
            built_motion: RingMotion::STATIC,
            ring_rebuilds: 0,
            combined: Vec::new(),
            combined_radii: Vec::new(),
            combined_arcs: Vec::new(),
            combined_arc_radii: Vec::new(),
            colors: Vec::new(),
            arc_colors: Vec::new(),
            draw_buf: Vec::with_capacity(max_segments),
            single_buf: Vec::with_capacity(max_segments),
            arc_draw_buf: Vec::new(),
            single_arc_buf: Vec::new(),
            max_segments,
            mirror_overflow: None,
            time: 0.0,
            // Replaced by the preset's palette on the next switch; the default
            // is the engine cosine, so an unconfigured scene still colours.
            palette: Palette::default_spectrum(),
            variant: DEFAULT_VARIANT,
            rotation: DEFAULT_ROTATION,
            colour: common::PaletteParams::new(DEFAULT_HUE, DEFAULT_BRIGHTNESS),
            pan: common::PanParams::default(),
            hue_spread: DEFAULT_HUE_SPREAD,
            draw_progress: DEFAULT_DRAW_PROGRESS,
            thickness: DEFAULT_THICKNESS,
            scale: DEFAULT_SCALE,
            glow: DEFAULT_GLOW,
            softness: super::DEFAULT_SOFTNESS,
            stroke_blend: super::ADDITIVE_BLEND,
            zoom: DEFAULT_ZOOM,
            mirror_order: DEFAULT_MIRROR_ORDER,
            mirror_reflect: DEFAULT_MIRROR_REFLECT,
            ring_phase: DEFAULT_RING_PHASE,
            ring_spread: DEFAULT_RING_SPREAD,
            ring_scale: DEFAULT_RING_SCALE_PARAM,
        }
    }

    /// Ask the cache for the rosette this frame's `variant` names, rebuilding
    /// only if the request has walked more than one step, and keep the combined
    /// figure and the colour buffer sized to it.
    fn refresh(&mut self) {
        let rebuilt = self.cache.request(
            self.order,
            contact_angle_deg(self.base_contact_deg, self.variant),
        );
        let rings_moved = self.refresh_rings();
        if self.has_ornament() {
            // Either half can move under the other — the rosette on `variant`,
            // the ornament on the three ring levers — and the radial ramp is a
            // min-max over the pair, so either rebuild refills both. Bounded by
            // the two hystereses, i.e. by distance travelled rather than by frame
            // count (ADR-0060).
            if rebuilt
                || rings_moved
                || self.combined.len() != self.combined_len()
                || self.combined_arcs.len() != self.combined_arc_len()
            {
                self.rebuild_combined();
            }
        }
        // A rebuild at the same order keeps the same `2n` segments, so this
        // fires on a preset switch and never on a morph.
        let wanted = self.base().0.len();
        if self.colors.len() != wanted {
            self.colors.clear();
            self.colors.resize(wanted, [0.0; 3]);
        }
        let wanted_arcs = self.base_arcs().0.len();
        if self.arc_colors.len() != wanted_arcs {
            self.arc_colors.clear();
            self.arc_colors.resize(wanted_arcs, [0.0; 3]);
        }
    }

    /// Whether this preset declared a `rings` ornament that produced anything.
    ///
    /// **Both kinds, and that is the whole reason it is a method.** Before
    /// Plan 0087 an empty `ring_segments` meant "no ornament"; a roster of
    /// nothing but `circle` rings now leaves that empty and fills
    /// [`ring_arcs`](Self::ring_arcs) instead, and reading the old signal would
    /// send such a preset down the rings-less path and draw only its interlace.
    fn has_ornament(&self) -> bool {
        !self.ring_segments.is_empty() || !self.ring_arcs.is_empty()
    }

    /// Re-place the ornament if this frame's [`RingMotion`] has walked further
    /// than one step from the one it holds. Returns `true` if it rebuilt.
    ///
    /// The rebuild reuses the buffer, which is what keeps a moving mandala
    /// allocation-free: a motion changes where segments are, never how many, so
    /// the `Vec` that `configure` grew is exactly the right size forever.
    fn refresh_rings(&mut self) -> bool {
        if self.rings.is_empty() {
            return false;
        }
        let want = RingMotion::from_params(self.ring_phase, self.ring_spread, self.ring_scale);
        if !self.built_motion.needs_rebuild(want) {
            return false;
        }
        self.built_motion = want;
        self.ring_rebuilds = self.ring_rebuilds.saturating_add(1);
        // Truncation stays silent at the cap, exactly as it is at load — a bound
        // lever cannot change the count, so a preset that fits keeps fitting.
        let _dropped = build_rings(
            &self.rings,
            want,
            self.max_segments,
            &mut self.ring_segments,
            &mut self.ring_arcs,
        );
        true
    }

    /// How long the combined figure is once the cap has bitten — the rosette
    /// first, then as much of the ornament as fits.
    fn combined_len(&self) -> usize {
        (self.cache.segments.len() + self.ring_segments.len()).min(self.max_segments)
    }

    /// The arc half of the same: whatever room the segments left, which is all
    /// of the ornament's arcs unless the cap has bitten.
    fn combined_arc_len(&self) -> usize {
        self.ring_arcs
            .len()
            .min(self.max_segments.saturating_sub(self.combined_len()))
    }

    /// Refill [`combined`](Self::combined) (and its radii) from the cached
    /// rosette and the static ornament. Capacity was reserved at `configure`, so
    /// the steady state allocates nothing.
    fn rebuild_combined(&mut self) {
        self.combined.clear();
        self.combined
            .extend(self.cache.segments.iter().take(self.max_segments));
        let room = self.max_segments.saturating_sub(self.combined.len());
        self.combined.extend(self.ring_segments.iter().take(room));
        // The arcs take what the segments left. One cap over both kinds, as
        // `Motif::instances` charges them (ADR-0098): the ceiling is a statement
        // about how much geometry a tier draws, not about one kind of it.
        self.combined_arcs.clear();
        let room = self.max_segments.saturating_sub(self.combined.len());
        self.combined_arcs.extend(self.ring_arcs.iter().take(room));
        normalized_radii(
            &self.combined,
            &self.combined_arcs,
            &mut self.combined_radii,
            &mut self.combined_arc_radii,
        );
    }

    /// The geometry this frame transforms, with its colour axis: the rosette
    /// alone when no `rings` were declared — which is bit-for-bit the pre-Plan
    /// 0065 path, buffer included — and the combined figure otherwise.
    fn base(&self) -> (&[SegmentInstance], &[f32]) {
        if !self.has_ornament() {
            (&self.cache.segments, &self.cache.radii)
        } else {
            (&self.combined, &self.combined_radii)
        }
    }

    /// [`base`](Self::base)'s arc half. The rosette has no arcs, so a rings-less
    /// preset gets two empty slices and its draw is exactly what it was.
    fn base_arcs(&self) -> (&[ArcInstance], &[f32]) {
        (&self.combined_arcs, &self.combined_arc_radii)
    }
}

/// The contact angle (degrees) a `variant` asks for, around the preset's
/// `[generator] contact_angle_deg`.
///
/// `variant` spans `0..2`, and 0 / 1 / 2 land exactly on the `-24 / 0 /
/// +24` degree offsets of the three precomputed variants, so a preset
/// binding integers draws one of them exactly (ADR-0060). Everything
/// between them is a real rosette.
///
/// **Total**, because it runs per frame from an author expression: a non-finite
/// `variant` falls back to the centre rather than reaching the construction, and
/// the result is clamped to the range that makes a sensible star.
pub(crate) fn contact_angle_deg(base_deg: f32, variant: f32) -> f32 {
    let v = if variant.is_finite() {
        variant.clamp(0.0, 2.0)
    } else {
        VARIANT_CENTER
    };
    let angle = base_deg + (v - VARIANT_CENTER) * VARIANT_SPAN_DEG;
    if angle.is_finite() {
        angle.clamp(CONTACT_MIN_DEG, CONTACT_MAX_DEG)
    } else {
        CONTACT_MIN_DEG
    }
}

/// The single cached rosette and the contact angle it was built at (ADR-0060).
///
/// The cache key is the **built angle plus a hysteresis band**, not a quantized
/// bucket: a request rebuilds when it is further than [`STEP_DEG`] from what is
/// held, and the rebuild targets the request itself. That is what bounds the
/// rebuild count of a sweep by *distance travelled / step* rather than by frame
/// count — and it is why a `variant` dithering inside one band never rebuilds at
/// all, which a bucket key would do on every crossing.
#[derive(Default)]
pub(crate) struct RosetteCache {
    /// The star order the held rosette was built for. `0` means "nothing built".
    order: u32,
    /// The contact angle (degrees) it was built at.
    built_deg: f32,
    /// The rosette, fit-normalized, positions only.
    segments: Vec<SegmentInstance>,
    /// Its per-segment normalized radius — ADR-0059's colour axis. A load-time
    /// quantity: `transform_cached`'s rotate and uniform scale leave a
    /// *normalized* radius unchanged, so nothing per frame can move it.
    radii: Vec<f32>,
    /// How many times this cache has built a rosette. Not used by the render
    /// path; it is the observable the rebuild-rate test asserts on, because
    /// "does not rebuild every frame" is otherwise invisible from outside.
    rebuilds: u64,
}

impl RosetteCache {
    /// Ensure the cache holds an `order`-fold rosette within [`STEP_DEG`] of
    /// `angle_deg`, rebuilding if not. Returns `true` if it rebuilt.
    ///
    /// A rebuild reuses the buffers, so the steady state allocates nothing; the
    /// first build for an order grows them to `2 * order` and they stay.
    pub(crate) fn request(&mut self, order: u32, angle_deg: f32) -> bool {
        let held = self.order == order && (angle_deg - self.built_deg).abs() <= STEP_DEG;
        if held {
            return false;
        }
        self.order = order;
        self.built_deg = angle_deg;
        self.rebuilds = self.rebuilds.saturating_add(1);
        hankin::star_rosette(order, angle_deg.to_radians(), &mut self.segments);
        turtle::normalize_fit(&mut self.segments, 0.9);
        normalized_radii(&self.segments, &[], &mut self.radii, &mut Vec::new());
        true
    }

    /// Drop whatever is held, so the next [`request`](Self::request) rebuilds.
    /// Called when a preset switch changes the construction under the cache.
    pub(crate) fn invalidate(&mut self) {
        self.order = 0;
        // `order = 0` alone does not force a rebuild: a rings-only preset
        // asks for order 0 (`tiling = "none"`), which would
        // match a just-invalidated cache and reuse its *empty* segment list at
        // whatever angle happened to be held. A non-finite built angle fails
        // every comparison, so the next request always rebuilds.
        self.built_deg = f32::NAN;
        self.segments.clear();
        self.radii.clear();
    }

    /// How many rosettes this cache has built. Test-only on purpose: nothing on
    /// the render path needs it, and "does not rebuild every frame" is the one
    /// claim of ADR-0060's hysteresis that is invisible from outside.
    #[cfg(test)]
    pub(crate) fn rebuilds(&self) -> u64 {
        self.rebuilds
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
pub(crate) fn normalized_radii(
    segs: &[SegmentInstance],
    arcs: &[ArcInstance],
    out: &mut Vec<f32>,
    arc_out: &mut Vec<f32>,
) {
    out.clear();
    arc_out.clear();
    let radius = |s: &SegmentInstance| -> f32 {
        let (x, y) = (0.5 * (s.a[0] + s.b[0]), 0.5 * (s.a[1] + s.b[1]));
        (x * x + y * y).sqrt()
    };
    // An arc's centre of curvature, for the same reason a segment's midpoint:
    // it is the one point that stands for the whole instance, and for a placed
    // circular motif it is exactly where the copy sits on its ring.
    let arc_radius = |a: &ArcInstance| -> f32 {
        let (x, y) = (a.centre[0], a.centre[1]);
        (x * x + y * y).sqrt()
    };
    // **One min-max over both kinds.** Normalizing each separately would give a
    // mandala's circles their own full palette sweep independent of the
    // interlace's, and ADR-0059's axis is the radius across the whole figure.
    let mut lo = f32::INFINITY;
    let mut hi = f32::NEG_INFINITY;
    for seg in segs {
        let r = radius(seg);
        lo = lo.min(r);
        hi = hi.max(r);
    }
    for arc in arcs {
        let r = arc_radius(arc);
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
    for arc in arcs {
        arc_out.push((arc_radius(arc) - lo) * scale);
    }
}

/// The smallest radial spread (in the fit-normalized world, where the figure
/// spans at most `2 * 0.9`) that counts as a range worth colouring along.
const RADIAL_FLOOR: f32 = 1e-4;

// The two halves of the mandala interior (ADR-0079), which never talk to each
// other: `motif` is pure shape arithmetic in a motif's own frame, `rings` is
// placement. What stays here is the scene, its rosette, and its `Scene` impl.
mod motif;
mod rings;

// `Motif`, `RingSpec` and the two ring bounds are named from outside the scene
// — the preset schema validates a `[generator] rings` roster against them — so
// they keep their old path rather than gaining a `motif::`/`rings::` segment.
pub use motif::{MIN_SCALLOP_LOBES, Motif};
pub use rings::{DEFAULT_RING_SCALE, MAX_RING_COUNT, RingSpec};

use motif::*;
use rings::*;

/// Parameter vocabulary — see [`fragment_field::PARAMS`](crate::render::scenes::fragment_field::PARAMS).
/// **Keep in sync with `set_param` below.**
pub const PARAMS: &[&str] = &[
    "variant",
    "rotation",
    "hue",
    "hue_spread",
    "saturation",
    "palette_mix",
    "palette_steps",
    "palette_contour",
    "draw_progress",
    "thickness",
    "scale",
    "brightness",
    "glow",
    "softness",
    "stroke_blend",
    "zoom",
    "pan_x",
    "pan_y",
    "mirror_order",
    "mirror_reflect",
    // The ring levers (Plan 0065 Phase 4). Inert on a preset that declares no
    // `[generator] rings` — there is nothing for them to move.
    "ring_phase",
    "ring_spread",
    "ring_scale",
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
        self.colour.reset();
        self.pan.reset();
        self.hue_spread = DEFAULT_HUE_SPREAD;
        self.draw_progress = DEFAULT_DRAW_PROGRESS;
        self.thickness = DEFAULT_THICKNESS;
        self.scale = DEFAULT_SCALE;
        self.glow = DEFAULT_GLOW;
        self.softness = super::DEFAULT_SOFTNESS;
        self.stroke_blend = super::ADDITIVE_BLEND;
        self.zoom = DEFAULT_ZOOM;
        self.mirror_order = DEFAULT_MIRROR_ORDER;
        self.mirror_reflect = DEFAULT_MIRROR_REFLECT;
        self.ring_phase = DEFAULT_RING_PHASE;
        self.ring_spread = DEFAULT_RING_SPREAD;
        self.ring_scale = DEFAULT_RING_SCALE_PARAM;
    }

    fn set_param(&mut self, name: &str, value: f32) {
        // The shared param blocks first, this scene's own names after
        // (`scenes::common`).
        if self.colour.set(name, value) || self.pan.set(name, value) {
            return;
        }
        match name {
            "variant" => self.variant = value,
            "rotation" => self.rotation = value,
            "hue_spread" => self.hue_spread = value,
            "draw_progress" => self.draw_progress = value,
            "thickness" => self.thickness = value,
            "scale" => self.scale = value,
            "glow" => self.glow = value,
            "softness" => self.softness = value,
            "stroke_blend" => self.stroke_blend = value,
            "zoom" => self.zoom = value,
            "mirror_order" => self.mirror_order = value,
            "mirror_reflect" => self.mirror_reflect = value,
            "ring_phase" => self.ring_phase = value,
            "ring_spread" => self.ring_spread = value,
            "ring_scale" => self.ring_scale = value,
            _ => {}
        }
    }

    fn set_palette(&mut self, palette: &Palette) {
        self.palette = palette.clone();
    }

    fn configure(&mut self, cfg: &GeneratorConfig) -> Option<CapOverflow> {
        // Record the construction and build the first rosette off the hot path.
        // Every other variant belongs to a sibling scene and is not named:
        // matching only this one is what keeps a new variant from editing four
        // scenes that do not use it, and `GeneratorConfig::element_count` is the
        // one place that still has to acknowledge every variant.
        if let GeneratorConfig::Star {
            order,
            contact_angle_deg,
            rings,
        } = cfg
        {
            self.order = *order;
            self.base_contact_deg = *contact_angle_deg;
            // The previous preset's rosette is not this preset's, whatever
            // angle it happens to sit at.
            self.cache.invalidate();
            // The ornament is placement arithmetic over a validated roster,
            // built here at the **static** motion and re-placed thereafter
            // only when a bound lever has moved a whole step (Phase 4). The
            // cap truncates **silently**, exactly as the turtle's has since
            // ADR-0007: nothing detects it, and `presets/README.md`
            // documents it.
            self.rings.clear();
            self.rings.extend_from_slice(rings);
            self.built_motion = RingMotion::STATIC;
            let _dropped = build_rings(
                &self.rings,
                RingMotion::STATIC,
                self.max_segments,
                &mut self.ring_segments,
                &mut self.ring_arcs,
            );
            // The arc buffers are sized here, from the roster the preset
            // actually declared, so the per-frame transform and mirror
            // allocate nothing — and a preset with no circular motif
            // reserves nothing at all.
            // The mirror is the multiplier, and its order is capped at
            // load (`MAX_MIRROR_ORDER`), reflection doubling it once more.
            let arc_room = self
                .ring_arcs
                .len()
                .saturating_mul(2 * super::MAX_MIRROR_ORDER as usize)
                .min(self.max_segments);
            self.single_arc_buf.reserve(self.ring_arcs.len());
            self.arc_draw_buf.reserve(arc_room);
            if !self.has_ornament() {
                // A switch *away* from a mandala must not leave its buffers
                // behind for `base` to pick up.
                self.combined.clear();
                self.combined_radii.clear();
                self.combined_arcs.clear();
                self.combined_arc_radii.clear();
            }
            self.refresh();
        }
        // A rosette is `2 * n` segments for the small regular tilings v1 allows
        // (n <= 12), far under the cap — no truncation to surface.
        None
    }

    fn mirror_overflow(&self) -> Option<&CapOverflow> {
        self.mirror_overflow.as_ref()
    }

    fn update(&mut self, _frame: &AnalysisFrame) {
        // `variant` is a contact angle now (ADR-0060). The cache reuses its
        // rosette unless the request has walked more than one `STEP_DEG`, which
        // is what keeps generator work off the hot path now that a bound param
        // can reach it.
        self.refresh();
        // The rosette, the ornament, or both — see [`base`](Self::base). Taken as
        // a pair of slices *before* the colour fill so the borrow of the geometry
        // and the mutable borrow of `colors` stay on disjoint fields.
        let (base, base_radii) = if !self.has_ornament() {
            (&self.cache.segments, &self.cache.radii)
        } else {
            (&self.combined, &self.combined_radii)
        };
        if base.is_empty() && self.combined_arcs.is_empty() {
            self.draw_buf.clear();
            self.arc_draw_buf.clear();
            return;
        }

        // The radial colour ramp (ADR-0059). One sample per segment, into a
        // buffer sized at build time. The radii are build-time values because a
        // rotate plus a uniform scale leaves a normalized radius unchanged.
        let ramp = ColorRamp {
            hue: self.colour.hue,
            hue_spread: self.hue_spread,
            palette_mix: self.colour.mix,
            palette_steps: self.colour.steps,
            saturation: self.colour.saturation,
            brightness: self.colour.brightness,
        };
        for (slot, &u) in self.colors.iter_mut().zip(base_radii) {
            *slot = ramp.at(&self.palette, u);
        }
        for (slot, &u) in self.arc_colors.iter_mut().zip(&self.combined_arc_radii) {
            *slot = ramp.at(&self.palette, u);
        }
        let inner = self.colors.first().copied().unwrap_or([1.0; 3]);

        let width = super::half_width(self.thickness);
        transform_cached(
            base,
            self.rotation,
            self.scale,
            inner,
            width,
            self.draw_progress,
            &mut self.single_buf,
        );
        // The same transform, the same reveal fraction. `draw_progress` is a
        // prefix of each kind rather than of one concatenated list: the two are
        // separate draws with separate buffers, and a fraction of each is the
        // only rule that keeps meaning when a figure is all arcs.
        transform_cached(
            &self.combined_arcs,
            self.rotation,
            self.scale,
            inner,
            width,
            self.draw_progress,
            &mut self.single_arc_buf,
        );
        for (arc, &color) in self.single_arc_buf.iter_mut().zip(&self.arc_colors) {
            arc.color = color;
        }
        // `transform_cached` keeps a prefix (the `draw_progress` reveal), so
        // segment `i` of the output is still segment `i` of the base figure —
        // which is why the rosette comes first in `combined` and the ornament
        // after it: a partial reveal draws the interlace, then the rings.
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
            std::mem::swap(&mut self.single_arc_buf, &mut self.arc_draw_buf);
            self.mirror_overflow = None;
            return;
        }
        let dropped = replicate_mirror(
            &self.single_buf,
            mirror,
            self.max_segments,
            &mut self.draw_buf,
        );
        // The arcs replicate into what the segments left of the same cap — one
        // ceiling over both kinds, as `Motif::instances` charges them.
        let arc_dropped = replicate_mirror(
            &self.single_arc_buf,
            mirror,
            self.max_segments.saturating_sub(self.draw_buf.len()),
            &mut self.arc_draw_buf,
        );
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
                &self.draw_buf,
                &self.arc_draw_buf,
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
                &self.draw_buf,
                &self.arc_draw_buf,
            );
        }
    }
}

#[cfg(test)]
mod tests;
