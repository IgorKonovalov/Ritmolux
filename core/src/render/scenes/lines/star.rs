//! Star-pattern scene: a Hankin star rosette built from a **continuous contact
//! angle** and cached (ADR-0007 generator build model), cheap to animate. Per
//! frame the scene resolves `variant` to an angle, reuses the cached rosette
//! unless the request has moved more than one step, and applies a
//! rotate/scale/colour/draw-on transform (allocation-free).
//!
//! ## `variant` is a contact angle, not an index (ADR-0060)
//!
//! It used to `floor` into one of three precomputed rosettes, so `[smoothing]`
//! on it spent its time on fractional values a floor threw away and the change
//! read as a stutter — design-backlog 0007's "change between star rosette shapes
//! should be smooth". Now `variant` maps linearly onto a contact-angle offset:
//! `0`, `1` and `2` land on exactly the `-24 / 0 / +24` degree offsets the three
//! cached variants used to hold, so **a preset binding integers draws exactly
//! what it drew before**, while a fractional value is a real rosette in between.
//!
//! The cache stays, keyed on the built angle with **hysteresis**: a request more
//! than [`STEP_DEG`] from the built angle rebuilds, anything nearer reuses. That
//! is what keeps generator work off the hot path (ADR-0007) now that a bound
//! param can reach it.
//!
//! **The step is measured, not assumed** (ADR-0060 leaves the number to the
//! plan). At `0.1` degrees:
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
//! `hue_spread` is therefore a **no-op on a rings-less preset**, and that is
//! stated here and in `presets/README.md` rather than shipped as a lever that
//! quietly does nothing. What such a preset does gain is `[palette]` itself: the
//! rosette can finally be an ember or an ice figure instead of a point on the
//! built-in cosine.
//!
//! ## The interior: rings of motifs (ADR-0079)
//!
//! **That day arrived.** `[generator] rings` is an optional roster of concentric
//! rings — `{ motif, count, radius, scale, phase }` each — drawn through the same
//! [`LineRenderer`] alongside, or instead of, the interlace. It is the answer to
//! design-backlog 0007's hollow-ring half, and it is *placement* rather than
//! construction: copy `i` of a ring of `k` sits at `2*pi*i/k + phase`, scaled by
//! `scale`, at distance `radius`, in the same fit-normalized world the rosette
//! lands in (the rosette spans `+/- 0.9`, so a `radius` near `0.9` sits on its
//! rim and anything smaller is genuinely interior).
//!
//! Two consequences worth stating where they can be read:
//!
//! - **With `rings` absent nothing here runs at all**, and the scene draws the
//!   Hankin path it drew before, segment for segment — the rings live in their
//!   own buffer and the combined one is never even allocated.
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
//! Plan 0065 Phase 4 puts the ornament on the param surface without making the
//! roster bindable — the roster stays structural, and what moves is a
//! [`RingMotion`] applied to it: `ring_phase` turns **alternate rings in opposite
//! directions**, `ring_spread` multiplies every radius about the centre, and
//! `ring_scale` multiplies every motif's size. All three default to
//! [`RingMotion::STATIC`], which is the exact identity (`+ 0`, `* 1`, `* 1`), so a
//! preset that binds none of them draws Phase 1's figure bit for bit.
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
//! further than one step ([`RING_PHASE_STEP`] and friends) from what is held
//! rebuilds, anything nearer reuses. A preset binding none of the three
//! therefore never rebuilds after `configure`, which is the ADR-0007 property
//! that made the roster structural in the first place — but a preset that
//! *animates* a lever does re-place its ornament on most frames, and that is
//! affordable rather than free. See [`RING_PHASE_STEP`] for the measurement.

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
use std::f32::consts::TAU;
use std::rc::Rc;

use super::super::Scene;
use super::renderer::{JOINED_A, JOINED_B, LineRenderer, SegmentInstance};
use super::{
    CapOverflow, ColorRamp, GeneratorConfig, MirrorSpec, OverflowContext, ViewTransform, hankin,
    replicate_mirror, transform_cached, turtle,
};
use crate::dsp::AnalysisFrame;
use crate::render::palette::Palette;

/// Maps `thickness` to an NDC-y half-width (see the parametric scene).
const WIDTH_SCALE: f32 = 0.003;

/// How far (degrees of contact angle) `variant` reaches either side of the
/// preset's base angle — a pointier star at `0`, a blunter one at `2`. This is
/// the span the three precomputed variants used to sit at (`-24 / 0 / +24`), kept
/// so a preset binding integers is unchanged (ADR-0060).
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
    /// rings-less preset takes exactly the path it took before Plan 0065.
    ring_segments: Vec<SegmentInstance>,
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
    /// Per-segment stroke colour for the cached rosette, rebuilt each frame into
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
            built_motion: RingMotion::STATIC,
            ring_rebuilds: 0,
            combined: Vec::new(),
            combined_radii: Vec::new(),
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
        if !self.ring_segments.is_empty() {
            // Either half can move under the other — the rosette on `variant`,
            // the ornament on the three ring levers — and the radial ramp is a
            // min-max over the pair, so either rebuild refills both. Bounded by
            // the two hystereses, i.e. by distance travelled rather than by frame
            // count (ADR-0060).
            if rebuilt || rings_moved || self.combined.len() != self.combined_len() {
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
        );
        true
    }

    /// How long the combined figure is once the cap has bitten — the rosette
    /// first, then as much of the ornament as fits.
    fn combined_len(&self) -> usize {
        (self.cache.segments.len() + self.ring_segments.len()).min(self.max_segments)
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
        normalized_radii(&self.combined, &mut self.combined_radii);
    }

    /// The geometry this frame transforms, with its colour axis: the rosette
    /// alone when no `rings` were declared — which is bit-for-bit the pre-Plan
    /// 0065 path, buffer included — and the combined figure otherwise.
    fn base(&self) -> (&[SegmentInstance], &[f32]) {
        if self.ring_segments.is_empty() {
            (&self.cache.segments, &self.cache.radii)
        } else {
            (&self.combined, &self.combined_radii)
        }
    }
}

/// The contact angle (degrees) a `variant` asks for, around the preset's
/// `[generator] contact_angle_deg`.
///
/// `variant` keeps the `0..2` range it has always had and 0 / 1 / 2 land exactly
/// on the `-24 / 0 / +24` degree offsets the three precomputed variants used to
/// hold, so a preset binding integers draws exactly what it drew before
/// (ADR-0060). What is new is that everything between them is a real rosette.
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
        normalized_radii(&self.segments, &mut self.radii);
        true
    }

    /// Drop whatever is held, so the next [`request`](Self::request) rebuilds.
    /// Called when a preset switch changes the construction under the cache.
    pub(crate) fn invalidate(&mut self) {
        self.order = 0;
        // `order = 0` alone is not enough to force a rebuild any more: a
        // rings-only preset asks for order 0 (`tiling = "none"`), which would
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

// ---------------------------------------------------------------------------
// The mandala interior: rings of motifs (ADR-0079)
// ---------------------------------------------------------------------------

/// The **closed, curated** motif roster (ADR-0079): the shapes a `[generator]
/// rings` entry may repeat around a ring.
///
/// Closed on purpose, and this is the boundary the decision draws. Each motif is
/// a parametric outline sampled to segments — the same thing `parametric_curve`
/// already does, placed rather than drawn once — so making the set authorable
/// would be a drawing language rather than a parameter, with no natural stopping
/// point (ADR-0079 Alternative C). A look outside the roster routes back through
/// `architect` + `dev`.
///
/// **Local convention, and every outline below obeys it:** a motif is authored
/// about its own centre, spanning roughly one unit, with **outward** (away from
/// the frame centre) along `+x`. Placement is then one rotation for both the
/// orientation and the position — see [`build_rings`].
///
/// **The roster closed at seven on 2026-08-06** (Plan 0065 Phase 3), picked from
/// the rendered sample sheets rather than from names. Two of the nine provisional
/// members were **cut**, and the property they were cut on is worth keeping
/// because the next candidate meets it too: *does it hold its identity across the
/// whole 8-to-32 count range*.
///
/// - **`star`** is an ornament at x8 and dissolves into texture by x32.
/// - **`triangle`** duplicates [`Chevron`](Motif::Chevron)'s sawtooth role at
///   roughly twelve times the segment cost — `chevron` is 2 segments, the
///   cheapest member in the set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Motif {
    /// A closed circle — the plainest bead, and the ring that reads as a dotted
    /// orbit.
    Circle,
    /// A pointed oval (a vesica), pointed at **both** ends along the radius.
    Petal,
    /// Round at the outer end, cusped at the inner one — the classic paisley
    /// drop, and the one motif with an unambiguous "which way is out".
    Teardrop,
    /// A four-vertex rhombus, long along the radius.
    Diamond,
    /// An **open** circular arc bulging outward, chord tangential. The nearest
    /// the closed roster comes to a scalloped boundary: raise `scale` until
    /// neighbouring members' chord ends touch and the ring closes into a scallop.
    ///
    /// **It is an approximation and the user chose the real thing.** Shown side
    /// by side with a dense overlapping arc ring at Phase 3, the user picked a
    /// genuine boundary *curve primitive*, which the engine does not have —
    /// design-backlog 0071, architect then dev. Nothing here fakes one.
    Arc,
    /// A three-lobed rose, `r = |cos(3*theta/2)|` — the densest member, and the
    /// one that reads as ornament rather than as a bead.
    Trefoil,
    /// An **open** two-segment chevron, apex outward. The cheapest motif in the
    /// roster at two segments a copy.
    Chevron,
}

impl Motif {
    /// Every motif, in roster order — **the closed set**, and the list a load
    /// error names when a preset asks for something outside it.
    pub const ALL: &'static [Motif] = &[
        Motif::Circle,
        Motif::Petal,
        Motif::Teardrop,
        Motif::Diamond,
        Motif::Arc,
        Motif::Trefoil,
        Motif::Chevron,
    ];

    /// The `motif = "..."` name a preset writes. `None` for anything outside the
    /// roster — the loader turns that into a surfaced error, never a fallback.
    pub fn from_name(name: &str) -> Option<Motif> {
        Some(match name.trim() {
            "circle" => Motif::Circle,
            "petal" => Motif::Petal,
            "teardrop" => Motif::Teardrop,
            "diamond" => Motif::Diamond,
            "arc" => Motif::Arc,
            "trefoil" => Motif::Trefoil,
            "chevron" => Motif::Chevron,
            _ => return None,
        })
    }

    /// The roster name, for error messages and the sample index.
    pub fn name(self) -> &'static str {
        match self {
            Motif::Circle => "circle",
            Motif::Petal => "petal",
            Motif::Teardrop => "teardrop",
            Motif::Diamond => "diamond",
            Motif::Arc => "arc",
            Motif::Trefoil => "trefoil",
            Motif::Chevron => "chevron",
        }
    }

    /// Whether the outline closes back onto its first vertex. Open motifs
    /// ([`Arc`](Motif::Arc), [`Chevron`](Motif::Chevron)) emit one segment fewer
    /// than they have vertices and leave their two free ends unjoined.
    fn is_closed(self) -> bool {
        !matches!(self, Motif::Arc | Motif::Chevron)
    }

    /// Vertices in one copy of this motif.
    fn vertex_count(self) -> usize {
        match self {
            Motif::Circle | Motif::Petal | Motif::Teardrop => SMOOTH_SAMPLES,
            Motif::Diamond => 4,
            Motif::Arc => ARC_SAMPLES + 1,
            Motif::Trefoil => TREFOIL_SAMPLES,
            Motif::Chevron => 3,
        }
    }

    /// **Segments** one copy of this motif contributes — the number the budget
    /// arithmetic multiplies by `count`.
    pub fn segments(self) -> usize {
        let n = self.vertex_count();
        if self.is_closed() { n } else { n - 1 }
    }

    /// Write this motif's outline vertices into `out` (cleared first), in the
    /// local convention: centred on the origin, spanning roughly one unit,
    /// outward along `+x`.
    ///
    /// A pure function of the variant — no clock, no randomness (the determinism
    /// rule), so a mandala is the same figure on every device and in every
    /// capture.
    fn outline(self, out: &mut Vec<[f32; 2]>) {
        out.clear();
        match self {
            Motif::Circle => {
                for k in 0..SMOOTH_SAMPLES {
                    let t = TAU * k as f32 / SMOOTH_SAMPLES as f32;
                    out.push([0.5 * t.cos(), 0.5 * t.sin()]);
                }
            }
            // A pointed oval: the `1.6` exponent is what makes the two ends cusp
            // instead of round, and it is the whole difference from `Circle`.
            Motif::Petal => {
                for k in 0..SMOOTH_SAMPLES {
                    let t = TAU * k as f32 / SMOOTH_SAMPLES as f32;
                    let s = t.sin();
                    out.push([0.5 * t.cos(), 0.30 * s.signum() * s.abs().powf(1.6)]);
                }
            }
            // The `(1 + cos t) / 2` taper collapses the width at `t = pi`, i.e.
            // at the *inner* end, so the cusp points at the frame centre.
            Motif::Teardrop => {
                for k in 0..SMOOTH_SAMPLES {
                    let t = TAU * k as f32 / SMOOTH_SAMPLES as f32;
                    let c = t.cos();
                    out.push([0.5 * c, 0.32 * t.sin() * 0.5 * (1.0 + c)]);
                }
            }
            Motif::Diamond => {
                out.push([0.5, 0.0]);
                out.push([0.0, 0.3]);
                out.push([-0.5, 0.0]);
                out.push([0.0, -0.3]);
            }
            // Chord along `y`, bulge along `+x`, then shifted so the arc is
            // centred on the origin like every other motif — otherwise `radius`
            // would mean the chord for this one member and the centre for the
            // rest.
            Motif::Arc => {
                let bulge = 0.5 * ARC_RADIUS * (1.0 - ARC_HALF_ANGLE.cos());
                for k in 0..=ARC_SAMPLES {
                    let psi = ARC_HALF_ANGLE * (2.0 * k as f32 / ARC_SAMPLES as f32 - 1.0);
                    out.push([
                        ARC_RADIUS * (psi.cos() - ARC_HALF_ANGLE.cos()) - bulge,
                        ARC_RADIUS * psi.sin(),
                    ]);
                }
            }
            // `|cos(1.5 t)|` has three lobes over a full turn, and the sample
            // count is a multiple of six so every cusp lands exactly on a vertex
            // rather than being rounded off by the sampling.
            Motif::Trefoil => {
                for k in 0..TREFOIL_SAMPLES {
                    let t = TAU * k as f32 / TREFOIL_SAMPLES as f32;
                    let r = 0.5 * (1.5 * t).cos().abs();
                    out.push([r * t.cos(), r * t.sin()]);
                }
            }
            Motif::Chevron => {
                out.push([-0.25, 0.42]);
                out.push([0.5, 0.0]);
                out.push([-0.25, -0.42]);
            }
        }
    }
}

/// Vertices in the three smooth closed motifs. Twenty-four is the number
/// ADR-0079's budget arithmetic quotes, and it is smooth enough that a bead at a
/// shipped `scale` shows no facets.
const SMOOTH_SAMPLES: usize = 24;
/// Vertices in [`Motif::Trefoil`] — a multiple of six, so the three lobe cusps
/// fall on samples.
const TREFOIL_SAMPLES: usize = 36;
/// Segments in one [`Motif::Arc`].
const ARC_SAMPLES: usize = 12;
/// Half the angle [`Motif::Arc`] subtends at its own centre of curvature. Sixty
/// degrees gives a chord of `2 * 0.5 * sin(60 deg) = 0.866` against a `0.25`
/// bulge — a shallow scallop rather than a hook.
const ARC_HALF_ANGLE: f32 = std::f32::consts::FRAC_PI_3;
/// [`Motif::Arc`]'s radius of curvature.
const ARC_RADIUS: f32 = 0.5;

/// The largest `count` one ring may declare, enforced at load.
///
/// A ceiling rather than a raw `u32` because `count` is the one ring key that
/// multiplies work: at 512 copies even the roster's densest motif is 18 432
/// segments, which already reaches the floor tier's cap on its own, so anything
/// above this can only buy truncation. Validated at the boundary (an out-of-range
/// count is a load error) rather than clamped, because a preset asking for 4 000
/// copies has misunderstood something and should be told.
pub const MAX_RING_COUNT: u32 = 512;

/// The `scale` a ring takes when it declares none — a motif a quarter the size of
/// the fit-normalized figure, which is legible at every ring count in the roster.
pub const DEFAULT_RING_SCALE: f32 = 0.25;

/// One concentric ring of repeated motifs: the validated form of one entry in the
/// `[generator] rings` array (ADR-0079).
///
/// Every field is **structural** — read once at load, fixed for as long as the
/// preset is loaded. Plan 0065 Phase 4 adds the *bindable* levers (a global ring
/// phase, spread and scale) on top of this static configuration rather than in
/// place of it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RingSpec {
    /// Which curated outline is repeated around this ring.
    pub motif: Motif,
    /// Copies around the ring. Validated into `1..=`[`MAX_RING_COUNT`] at load.
    pub count: u32,
    /// Distance from the frame centre to each copy's own centre, in the
    /// fit-normalized world the rosette lands in — that figure spans `+/- 0.9`,
    /// so `0.9` is its rim and anything smaller is interior.
    pub radius: f32,
    /// Motif size multiplier; the outlines span roughly one unit, so this is
    /// close to the copy's diameter.
    pub scale: f32,
    /// Angular offset of copy `0`, in radians.
    pub phase: f32,
}

/// The per-frame motion applied to a validated roster (Plan 0065 Phase 4): what
/// the three bindable ring params resolve to.
///
/// Separate from [`RingSpec`] on purpose. The roster is **structural** — read
/// once at load, fixed for as long as the preset is loaded — and this is the
/// thin, three-scalar layer a bound expression may move it through, so nothing
/// bindable can change how many segments exist or which motif they are.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RingMotion {
    /// Counter-rotation, radians. Ring `i` turns by `+phase` when `i` is even and
    /// `-phase` when it is odd — see [`ring_direction`].
    pub phase: f32,
    /// Multiplies every ring's `radius`, about the frame centre.
    pub spread: f32,
    /// Multiplies every ring's motif `scale`.
    pub scale: f32,
}

impl RingMotion {
    /// The identity, and every param's default: the roster exactly as declared.
    /// `+ 0.0`, `* 1.0` and `* 1.0` are exact in IEEE, so this really is
    /// bit-for-bit the pre-Phase-4 geometry rather than approximately it.
    pub(crate) const STATIC: RingMotion = RingMotion {
        phase: 0.0,
        spread: 1.0,
        scale: 1.0,
    };

    /// Resolve the three bound params into a motion.
    ///
    /// **Total**, because all three run per frame from author expressions: a
    /// non-finite value falls back to its static component rather than reaching
    /// the placement arithmetic and writing NaN vertices into the draw buffer.
    /// `phase` wraps into one turn (it is typically `k * time`, which would
    /// otherwise lose angular precision within minutes and stop the hysteresis
    /// below resolving a step at all); `spread` and `scale` clamp to a range that
    /// keeps the figure on the same order as the frame.
    pub(crate) fn from_params(phase: f32, spread: f32, scale: f32) -> Self {
        let phase = if phase.is_finite() {
            phase.rem_euclid(TAU)
        } else {
            RingMotion::STATIC.phase
        };
        let clamp = |v: f32, hi: f32, fallback: f32| {
            if v.is_finite() {
                v.clamp(0.0, hi)
            } else {
                fallback
            }
        };
        RingMotion {
            phase,
            spread: clamp(spread, MAX_RING_SPREAD, RingMotion::STATIC.spread),
            scale: clamp(scale, MAX_RING_SCALE, RingMotion::STATIC.scale),
        }
    }

    /// Whether `want` has walked further than one step from `self` on any lever,
    /// i.e. whether the ornament has to be rebuilt.
    ///
    /// The same hysteresis habit as [`RosetteCache`] (ADR-0060), for the same
    /// reason: it is what keeps generator work off the hot path now that a bound
    /// param can reach it. The steps are chosen so one of them is sub-pixel at
    /// 1080p — see [`RING_PHASE_STEP`].
    pub(crate) fn needs_rebuild(self, want: RingMotion) -> bool {
        (want.phase - self.phase).abs() > RING_PHASE_STEP
            || (want.spread - self.spread).abs() > RING_SPREAD_STEP
            || (want.scale - self.scale).abs() > RING_SCALE_STEP
    }
}

/// The direction ring `i` turns under `ring_phase` — **the whole of
/// counter-rotation, and it costs one sign** (ADR-0079's ornamental motion).
///
/// Adjacent rings turn opposite ways, which is what makes a mandala read as one
/// figure breathing rather than as a rigid plate being spun. Indexed by position
/// in the roster, so a preset chooses which rings pair up by the order it writes
/// them in.
pub(crate) fn ring_direction(index: usize) -> f32 {
    if index.is_multiple_of(2) { 1.0 } else { -1.0 }
}

/// The largest `ring_spread` a binding may reach. The roster's radii live in the
/// fit-normalized world the rosette lands in (`+/- 0.9`), so `4` already pushes
/// every ring well off frame — past this the figure is gone and only the cost of
/// drawing it remains.
const MAX_RING_SPREAD: f32 = 4.0;
/// The largest `ring_scale` a binding may reach. Motifs span about one unit, so
/// at `8` a single copy covers the whole frame; beyond that the ring is a blob
/// whatever its count.
const MAX_RING_SCALE: f32 = 8.0;

/// The `ring_phase` hysteresis: a requested phase further than this from the
/// built one rebuilds the ornament, anything nearer reuses it.
///
/// **Sized the way [`STEP_DEG`] is — but it buys something different, and the
/// difference is stated rather than implied.** The outermost ring a shipped
/// preset places sits at radius `0.82` in a world whose half-height maps to
/// 540 px at 1080p, so one step moves a motif `0.001 * 0.82 * 540 =` **0.44 px**,
/// invisible under a stroke several pixels wide. That is what lets the step exist
/// at all.
///
/// What it does *not* do is keep an animated mandala off the rebuild path. A
/// `ring_phase` turning at any usable rate covers more than a step per frame at
/// 60 fps, so **an animated preset re-places its ornament on most frames**; what
/// never rebuilds is a preset that binds none of the three, which is every
/// rings-less preset and the static-roster default.
///
/// That is affordable rather than free, and the number is measured rather than
/// assumed: the shipped four-ring roster costs **4.9 us** per rebuild in release
/// (1 092 segments over 20 000 iterations) — 0.03 % of a 16.7 ms frame, and 0.5 %
/// for a hypothetical ornament filled to the floor tier's whole 20 000-segment
/// cap. So the hysteresis is a *saving* on slow and static levers, and the thing
/// that makes the fast case fine is the placement being O(segments) with no
/// allocation, not the step.
const RING_PHASE_STEP: f32 = 0.001;
/// The `ring_spread` hysteresis. Same arithmetic at the same outer radius:
/// `0.001 * 0.82 * 540 = 0.44 px`.
const RING_SPREAD_STEP: f32 = 0.001;
/// The `ring_scale` hysteresis. A motif `scale` is an order of magnitude smaller
/// than a ring radius (`0.13` to `0.46` across the shipped presets), so the same
/// sub-pixel step is a looser number: `0.002 * 0.46 * 540 = 0.50 px`.
const RING_SCALE_STEP: f32 = 0.002;

/// Place every ring's motifs into `out` (cleared first) under `motion`, and
/// return how many segments were dropped at `cap`.
///
/// The placement, which is the whole of ADR-0079's geometry: copy `i` of a ring
/// of `k` sits at angle `2*pi*i/k + phase`, and because each motif is authored
/// with **outward along `+x`**, that one rotation supplies both the copy's
/// position and its orientation — the motif is offset to `(radius, 0)` in the
/// ring's own frame and the whole frame is turned.
///
/// `motion` (Phase 4) rides on top of that and changes no count: it adds a
/// **signed** `phase` per ring, multiplies `radius` by `spread` and `scale` by
/// `scale`. At [`RingMotion::STATIC`] every one of those is an exact IEEE
/// identity, so the static roster is reproduced rather than approximated.
///
/// **Truncation at `cap` is silent by construction** (ADR-0007's behaviour on the
/// turtle, kept deliberately): the caller drops the count, `presets/README.md`
/// documents what a preset over budget looks like, and nothing surfaces it. The
/// count is returned anyway because it is what makes the cap testable.
///
/// Build-time: runs from `configure`, never from `update`. Written panic-free
/// under the module's pragma all the same.
pub(crate) fn build_rings(
    rings: &[RingSpec],
    motion: RingMotion,
    cap: usize,
    out: &mut Vec<SegmentInstance>,
) -> usize {
    out.clear();
    // What a cap-free build would emit — the only way to report the drop without
    // running the loop past the cap, which for a large `count` is the difference
    // between bounded and unbounded load-time work.
    let wanted = rings.iter().fold(0usize, |acc, ring| {
        acc.saturating_add(
            ring.motif
                .segments()
                .saturating_mul(ring.count.max(1) as usize),
        )
    });

    let mut pts: Vec<[f32; 2]> = Vec::new();
    'rings: for (index, ring) in rings.iter().enumerate() {
        ring.motif.outline(&mut pts);
        let n = pts.len();
        if n < 2 {
            continue;
        }
        let edges = if ring.motif.is_closed() { n } else { n - 1 };
        let count = ring.count.max(1);
        // The ring's own configuration, moved. Hoisted out of the copy loop
        // because it is constant across the ring.
        let base_phase = ring.phase + ring_direction(index) * motion.phase;
        let radius = ring.radius * motion.spread;
        let scale = ring.scale * motion.scale;
        for i in 0..count {
            let theta = TAU * i as f32 / count as f32 + base_phase;
            let (sin, cos) = theta.sin_cos();
            let place = |p: [f32; 2]| -> [f32; 2] {
                let x = p[0] * scale + radius;
                let y = p[1] * scale;
                [x * cos - y * sin, x * sin + y * cos]
            };
            for e in 0..edges {
                if out.len() >= cap {
                    break 'rings;
                }
                let (Some(&a), Some(&b)) = (pts.get(e), pts.get((e + 1) % n)) else {
                    continue;
                };
                // A closed outline is a closed chain, so every vertex is a joint
                // (ADR-0041); an open one is free at its two ends only.
                let joined = if ring.motif.is_closed() {
                    JOINED_A | JOINED_B
                } else {
                    let mut j = 0;
                    if e > 0 {
                        j |= JOINED_A;
                    }
                    if e + 1 < edges {
                        j |= JOINED_B;
                    }
                    j
                };
                out.push(SegmentInstance {
                    a: place(a),
                    b: place(b),
                    color: [1.0, 1.0, 1.0],
                    width: 0.01,
                    joined,
                });
            }
        }
    }
    wanted.saturating_sub(out.len())
}

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
        self.ring_phase = DEFAULT_RING_PHASE;
        self.ring_spread = DEFAULT_RING_SPREAD;
        self.ring_scale = DEFAULT_RING_SCALE_PARAM;
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
        // Other config variants belong to sibling line scenes and are ignored.
        match cfg {
            GeneratorConfig::Star {
                order,
                contact_angle_deg,
                rings,
            } => {
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
                );
                if self.ring_segments.is_empty() {
                    // A switch *away* from a mandala must not leave its buffers
                    // behind for `base` to pick up.
                    self.combined.clear();
                    self.combined_radii.clear();
                }
                self.refresh();
            }
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
        // `variant` is a contact angle now (ADR-0060). The cache reuses its
        // rosette unless the request has walked more than one `STEP_DEG`, which
        // is what keeps generator work off the hot path now that a bound param
        // can reach it.
        self.refresh();
        // The rosette, the ornament, or both — see [`base`](Self::base). Taken as
        // a pair of slices *before* the colour fill so the borrow of the geometry
        // and the mutable borrow of `colors` stay on disjoint fields.
        let (base, base_radii) = if self.ring_segments.is_empty() {
            (&self.cache.segments, &self.cache.radii)
        } else {
            (&self.combined, &self.combined_radii)
        };
        if base.is_empty() {
            self.draw_buf.clear();
            return;
        }

        // The radial colour ramp (ADR-0059). One sample per segment, into a
        // buffer sized at build time. The radii are build-time values because a
        // rotate plus a uniform scale leaves a normalized radius unchanged.
        let ramp = ColorRamp {
            hue: self.hue,
            hue_spread: self.hue_spread,
            palette_mix: self.palette_mix,
            saturation: self.saturation,
            brightness: self.brightness,
        };
        for (slot, &u) in self.colors.iter_mut().zip(base_radii) {
            *slot = ramp.at(&self.palette, u);
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
mod tests;
