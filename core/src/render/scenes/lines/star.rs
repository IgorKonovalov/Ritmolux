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
    /// The ring ornament (ADR-0079), built once in `configure` and static for as
    /// long as the preset is loaded. **Empty is the signal** that this preset
    /// declared no `rings`, and every ring-aware branch below keys off it, so a
    /// rings-less preset takes exactly the path it took before Plan 0065.
    ring_segments: Vec<SegmentInstance>,
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
            // a rings-less preset never allocates either of these.
            ring_segments: Vec::new(),
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
        if !self.ring_segments.is_empty() {
            // The rings are static, but the rosette under them is not, and the
            // radial ramp is a min-max over the pair — so a rosette rebuild
            // refills both. Bounded by the cache's own hysteresis, i.e. by
            // distance travelled rather than by frame count (ADR-0060).
            if rebuilt || self.combined.len() != self.combined_len() {
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
/// **This roster is provisional** until Plan 0065 Phase 3, where the user picks
/// from rendered samples which of these ship. Dropping members there is the
/// expected outcome.
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
    /// A three-vertex wedge, apex outward.
    Triangle,
    /// An **open** circular arc bulging outward, chord tangential. The scalloped
    /// boundary as a *motif ring*: raise `scale` until neighbouring members'
    /// chord ends touch and the ring closes into a scallop (ADR-0079's Notes
    /// leave that A/B to the sample set).
    Arc,
    /// A six-pointed star polygon.
    Star,
    /// A three-lobed rose, `r = |cos(3*theta/2)|` — the densest member, and the
    /// one that reads as ornament rather than as a bead.
    Trefoil,
    /// An **open** two-segment chevron, apex outward. The cheapest motif in the
    /// roster at two segments a copy.
    Chevron,
}

impl Motif {
    /// Every motif, in roster order. The sample set renders this list.
    pub const ALL: &'static [Motif] = &[
        Motif::Circle,
        Motif::Petal,
        Motif::Teardrop,
        Motif::Diamond,
        Motif::Triangle,
        Motif::Arc,
        Motif::Star,
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
            "triangle" => Motif::Triangle,
            "arc" => Motif::Arc,
            "star" => Motif::Star,
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
            Motif::Triangle => "triangle",
            Motif::Arc => "arc",
            Motif::Star => "star",
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
            Motif::Triangle => 3,
            Motif::Arc => ARC_SAMPLES + 1,
            Motif::Star => 2 * STAR_POINTS,
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
            Motif::Triangle => {
                out.push([0.5, 0.0]);
                out.push([-0.3, 0.32]);
                out.push([-0.3, -0.32]);
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
            Motif::Star => {
                for k in 0..2 * STAR_POINTS {
                    let t = TAU * k as f32 / (2 * STAR_POINTS) as f32;
                    let r = if k % 2 == 0 { 0.5 } else { 0.22 };
                    out.push([r * t.cos(), r * t.sin()]);
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
/// Points on [`Motif::Star`].
const STAR_POINTS: usize = 6;

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

/// Place every ring's motifs into `out` (cleared first) and return how many
/// segments were dropped at `cap`.
///
/// The placement, which is the whole of ADR-0079's geometry: copy `i` of a ring
/// of `k` sits at angle `2*pi*i/k + phase`, and because each motif is authored
/// with **outward along `+x`**, that one rotation supplies both the copy's
/// position and its orientation — the motif is offset to `(radius, 0)` in the
/// ring's own frame and the whole frame is turned.
///
/// **Truncation at `cap` is silent by construction** (ADR-0007's behaviour on the
/// turtle, kept deliberately): the caller drops the count, `presets/README.md`
/// documents what a preset over budget looks like, and nothing surfaces it. The
/// count is returned anyway because it is what makes the cap testable.
///
/// Build-time: runs from `configure`, never from `update`. Written panic-free
/// under the module's pragma all the same.
pub(crate) fn build_rings(rings: &[RingSpec], cap: usize, out: &mut Vec<SegmentInstance>) -> usize {
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
    'rings: for ring in rings {
        ring.motif.outline(&mut pts);
        let n = pts.len();
        if n < 2 {
            continue;
        }
        let edges = if ring.motif.is_closed() { n } else { n - 1 };
        let count = ring.count.max(1);
        for i in 0..count {
            let theta = TAU * i as f32 / count as f32 + ring.phase;
            let (sin, cos) = theta.sin_cos();
            let place = |p: [f32; 2]| -> [f32; 2] {
                let x = p[0] * ring.scale + ring.radius;
                let y = p[1] * ring.scale;
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
                // done once here and never again — the rings do not move until
                // Plan 0065 Phase 4 gives them params. The cap truncates
                // **silently**, exactly as the turtle's has since ADR-0007:
                // nothing detects it, and `presets/README.md` documents it.
                let _dropped = build_rings(rings, self.max_segments, &mut self.ring_segments);
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

    /// The angle a variant asks for, at the golden fixture's base angle.
    const BASE: f32 = 35.0;

    fn geometry_at(variant: f32) -> Vec<SegmentInstance> {
        let mut cache = RosetteCache::default();
        cache.request(12, contact_angle_deg(BASE, variant));
        cache.segments.clone()
    }

    fn differs(a: &[SegmentInstance], b: &[SegmentInstance]) -> bool {
        a.len() != b.len()
            || a.iter().zip(b).any(|(x, y)| {
                (x.a[0] - y.a[0]).abs() > 1e-4
                    || (x.a[1] - y.a[1]).abs() > 1e-4
                    || (x.b[0] - y.b[0]).abs() > 1e-4
                    || (x.b[1] - y.b[1]).abs() > 1e-4
            })
    }

    /// **Plan 0054 Phase 3 done-when 1 (ADR-0060): intermediate `variant` values
    /// produce intermediate geometry.** Under the old `floor` into three cached
    /// rosettes the middle frame was *identical* to one of the ends, which is
    /// exactly what makes this non-vacuous — the assertion that would have failed
    /// before is the one that the halfway figure differs from **both**.
    #[test]
    fn a_half_variant_is_a_real_rosette_between_the_two_ends() {
        let lo = geometry_at(0.0);
        let mid = geometry_at(0.5);
        let hi = geometry_at(1.0);

        assert!(differs(&lo, &hi), "the two ends must differ at all");
        assert!(
            differs(&mid, &lo),
            "variant 0.5 collapsed onto variant 0 — this is the floor, back again"
        );
        assert!(
            differs(&mid, &hi),
            "variant 0.5 collapsed onto variant 1 — this is the floor, back again"
        );

        // And it really is *between* them, not merely different: the petal tip
        // radius is monotone in the contact angle, so the middle figure's inner
        // radius sits between the two ends'.
        let inner = |segs: &[SegmentInstance]| {
            segs.iter().fold(f32::INFINITY, |acc, s| {
                acc.min((s.a[0].powi(2) + s.a[1].powi(2)).sqrt())
                    .min((s.b[0].powi(2) + s.b[1].powi(2)).sqrt())
            })
        };
        let (a, b, c) = (inner(&lo), inner(&mid), inner(&hi));
        assert!(a < b && b < c, "inner radii must be ordered: {a} {b} {c}");
    }

    /// The compatibility claim ADR-0060's "the precomputed-variant vocabulary
    /// disappears" was worried about, and the reason no baseline moved: `variant`
    /// 0 / 1 / 2 still name the `-24 / 0 / +24` degree offsets the three cached
    /// rosettes held, so a preset binding integers draws what it always drew.
    #[test]
    fn the_integer_variants_are_the_angles_the_cache_used_to_hold() {
        for (variant, offset) in [(0.0f32, -24.0f32), (1.0, 0.0), (2.0, 24.0)] {
            let want = (BASE + offset).clamp(CONTACT_MIN_DEG, CONTACT_MAX_DEG);
            assert!(
                (contact_angle_deg(BASE, variant) - want).abs() < 1e-5,
                "variant {variant} must still mean {want} degrees"
            );
        }
        // Out of range clamps to the ends rather than running off, as the old
        // `min(variants - 1)` index clamp did.
        assert_eq!(contact_angle_deg(BASE, -3.0), contact_angle_deg(BASE, 0.0));
        assert_eq!(contact_angle_deg(BASE, 9.0), contact_angle_deg(BASE, 2.0));
        // Total over what an expression can actually produce.
        for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let a = contact_angle_deg(BASE, bad);
            assert!(
                a.is_finite() && (CONTACT_MIN_DEG..=CONTACT_MAX_DEG).contains(&a),
                "variant {bad} produced {a}"
            );
        }
    }

    /// **Plan 0054 Phase 3 done-when 2: a swept `variant` does not rebuild every
    /// frame.** The bound is *distance travelled / step*, not the frame count —
    /// which is the whole content of the hysteresis, and the thing that keeps
    /// ADR-0007's off-hot-path guarantee once a bound param can reach the
    /// generator.
    #[test]
    fn a_swept_variant_rebuilds_per_step_not_per_frame() {
        const FRAMES: usize = 2_000;
        let mut cache = RosetteCache::default();

        for frame in 0..FRAMES {
            let variant = 2.0 * frame as f32 / (FRAMES - 1) as f32;
            cache.request(12, contact_angle_deg(BASE, variant));
        }

        // The sweep covers the whole variant range: 2 * VARIANT_SPAN_DEG degrees
        // of contact angle. `+ 2` for the initial build and the final partial
        // step.
        let travelled = 2.0 * VARIANT_SPAN_DEG;
        let bound = (travelled / STEP_DEG) as u64 + 2;
        assert!(
            cache.rebuilds() <= bound,
            "{} rebuilds over a {travelled}-degree sweep exceeds the {bound} the \
             {STEP_DEG}-degree step allows",
            cache.rebuilds()
        );
        assert!(
            cache.rebuilds() < FRAMES as u64 / 2,
            "{} rebuilds over {FRAMES} frames is tracking the frame rate, not \
             the step",
            cache.rebuilds()
        );

        // The other half of hysteresis, and the half a quantized-bucket key
        // would get wrong: a `variant` dithering inside one step never rebuilds
        // after the first, however many frames it runs for.
        let mut steady = RosetteCache::default();
        steady.request(12, contact_angle_deg(BASE, 1.0));
        let after_first = steady.rebuilds();
        for frame in 0..600 {
            // +/- 0.002 of a variant unit is +/- 0.048 degrees, just under half
            // a step — the band a quantized-bucket key would rebuild across on
            // every crossing.
            let jitter = if frame % 2 == 0 { 0.002 } else { -0.002 };
            steady.request(12, contact_angle_deg(BASE, 1.0 + jitter));
        }
        assert_eq!(
            steady.rebuilds(),
            after_first,
            "a variant jittering inside one step must never rebuild"
        );
    }

    /// The rebuild is bounded work, and this pins the number the frame-budget
    /// measurement in the module docs rests on: a rosette is `2n` segments, and
    /// the loader's whole tiling vocabulary stops at `n = 12`. So the rebuild
    /// this scene can actually be asked for is 24 segments — three orders of
    /// magnitude under the floor tier's 20 000-segment cap.
    #[test]
    fn the_reachable_rebuild_is_two_dozen_segments() {
        let mut widest = 0usize;
        for name in ["square", "hexagon", "octagon", "dodecagon"] {
            let order = hankin::tiling_order(name).unwrap_or(0);
            let mut cache = RosetteCache::default();
            cache.request(order, contact_angle_deg(BASE, 1.0));
            assert_eq!(cache.segments.len(), 2 * order as usize, "{name}");
            widest = widest.max(cache.segments.len());
        }
        assert_eq!(widest, 24, "the loader's largest tiling is 12-fold");
        assert!(widest * 800 < crate::render::TierConfig::FLOOR.max_segments);
    }

    /// A rebuild reuses its buffers, so a sweeping `variant` allocates nothing
    /// after the first build for an order — the ADR-0007 property the cache
    /// exists to keep.
    #[test]
    fn rebuilding_reuses_the_cache_buffers() {
        let mut cache = RosetteCache::default();
        cache.request(12, contact_angle_deg(BASE, 0.0));
        let (segs, radii) = (cache.segments.capacity(), cache.radii.capacity());
        for frame in 0..200 {
            let variant = 2.0 * frame as f32 / 199.0;
            cache.request(12, contact_angle_deg(BASE, variant));
        }
        assert!(cache.rebuilds() > 1, "the sweep must actually rebuild");
        assert_eq!(cache.segments.capacity(), segs, "segments reallocated");
        assert_eq!(cache.radii.capacity(), radii, "radii reallocated");
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

    // -----------------------------------------------------------------------
    // The mandala interior (Plan 0065 Phase 1 / ADR-0079)
    // -----------------------------------------------------------------------

    const CAP: usize = crate::render::TierConfig::FLOOR.max_segments;

    fn ring(motif: Motif, count: u32, radius: f32, scale: f32) -> RingSpec {
        RingSpec {
            motif,
            count,
            radius,
            scale,
            phase: 0.0,
        }
    }

    /// The four-ring roster `presets/star_mandala.toml` ships, so the coverage
    /// claim below is measured on the figure that actually shipped rather than
    /// on a fixture invented to pass.
    fn mandala_roster() -> Vec<RingSpec> {
        vec![
            ring(Motif::Trefoil, 1, 0.00, 0.46),
            ring(Motif::Diamond, 12, 0.30, 0.20),
            ring(Motif::Petal, 18, 0.52, 0.26),
            ring(Motif::Circle, 24, 0.70, 0.13),
        ]
    }

    fn midpoint_radius(s: &SegmentInstance) -> f32 {
        let (x, y) = (0.5 * (s.a[0] + s.b[0]), 0.5 * (s.a[1] + s.b[1]));
        (x * x + y * y).sqrt()
    }

    /// **Phase 1 done-when 1, stated where it can be checked without a GPU: with
    /// `rings` absent nothing new runs.** The pixel half of the claim is the
    /// `star_pattern` golden fixture (also rings-less) still matching its
    /// committed baseline; this is the half that says *why* it must — an empty
    /// roster produces an empty ornament, and `base` then hands out the cache's
    /// own buffers rather than a copy of them.
    #[test]
    fn an_absent_roster_builds_no_ornament_at_all() {
        let mut out = vec![
            SegmentInstance {
                a: [9.0, 9.0],
                b: [9.0, 9.0],
                color: [0.0; 3],
                width: 1.0,
                joined: 0,
            };
            3
        ];
        let dropped = build_rings(&[], CAP, &mut out);
        assert_eq!(dropped, 0);
        assert!(out.is_empty(), "an empty roster clears rather than appends");
    }

    /// The roster is closed and its two name maps are inverses — the property
    /// that makes an unknown `motif = "..."` a load error rather than a silent
    /// fallback to whichever variant happened to be first.
    #[test]
    fn the_motif_roster_round_trips_and_rejects_everything_else() {
        for &m in Motif::ALL {
            assert_eq!(Motif::from_name(m.name()), Some(m), "{}", m.name());
        }
        assert_eq!(Motif::ALL.len(), 9, "the provisional roster is nine motifs");
        for bad in ["", "hexagon", "Circle", "crescent", "petal2"] {
            assert_eq!(Motif::from_name(bad), None, "'{bad}' is outside the roster");
        }
    }

    /// [`Motif::segments`] is what the budget arithmetic multiplies by `count`,
    /// so it has to agree with the outline rather than be maintained beside it.
    #[test]
    fn the_declared_segment_count_matches_the_outline() {
        let mut pts = Vec::new();
        for &m in Motif::ALL {
            m.outline(&mut pts);
            assert_eq!(pts.len(), m.vertex_count(), "{}: vertex count", m.name());
            assert!(pts.len() >= 3, "{}: a motif needs a shape", m.name());
            let expected = if m.is_closed() {
                pts.len()
            } else {
                pts.len() - 1
            };
            assert_eq!(m.segments(), expected, "{}: segment count", m.name());

            let mut out = Vec::new();
            build_rings(&[ring(m, 1, 0.5, 1.0)], CAP, &mut out);
            assert_eq!(out.len(), m.segments(), "{}: one copy", m.name());
        }
    }

    /// **The placement arithmetic, which is the whole of ADR-0079's geometry.**
    /// `count` copies land at `2*pi*i/count + phase` and at `radius` from the
    /// centre, so a ring's segment set is invariant under a `2*pi/count`
    /// rotation — the same property `hankin.rs` asserts of the rosette, and the
    /// one that lets a ring count and a fold order be chosen together.
    #[test]
    fn a_ring_places_its_copies_evenly_around_the_frame_centre() {
        for &m in Motif::ALL {
            for count in [3u32, 8, 17] {
                let radius = 0.6;
                let scale = 0.18;
                let mut out = Vec::new();
                build_rings(&[ring(m, count, radius, scale)], CAP, &mut out);
                assert_eq!(out.len(), m.segments() * count as usize);

                // Every segment sits within one motif's reach of the ring.
                for seg in &out {
                    let r = midpoint_radius(seg);
                    assert!(
                        (r - radius).abs() <= scale,
                        "{} x{count}: a segment at {r} is off the {radius} ring",
                        m.name()
                    );
                }

                // ...and the set as a whole is invariant under one sector.
                let ang = TAU / count as f32;
                let (s, c) = ang.sin_cos();
                let rot = |p: [f32; 2]| [p[0] * c - p[1] * s, p[0] * s + p[1] * c];
                for seg in &out {
                    let (ra, rb) = (rot(seg.a), rot(seg.b));
                    let matched = out.iter().any(|o| {
                        let close = |x: [f32; 2], y: [f32; 2]| {
                            (x[0] - y[0]).abs() < 1e-4 && (x[1] - y[1]).abs() < 1e-4
                        };
                        close(o.a, ra) && close(o.b, rb)
                    });
                    assert!(matched, "{} x{count}: not {count}-fold symmetric", m.name());
                }
            }
        }
    }

    /// The local convention every outline is authored to — **outward is `+x`** —
    /// is what makes one rotation supply both position and orientation. Checked
    /// on the one motif where "which way is out" is unambiguous: the teardrop's
    /// cusp must end up pointing at the frame centre, not away from it.
    #[test]
    fn a_placed_motif_keeps_its_outward_end_outward() {
        let mut pts = Vec::new();
        Motif::Teardrop.outline(&mut pts);
        // Local: the widest part of the drop sits on the +x half.
        let widest = pts
            .iter()
            .fold(f32::NEG_INFINITY, |acc, p| acc.max(p[1].abs()));
        let widest_x = pts
            .iter()
            .filter(|p| p[1].abs() > 0.5 * widest)
            .fold(f32::NEG_INFINITY, |acc, p| acc.max(p[0]));
        assert!(widest_x > 0.0, "the drop's body is on the outward side");

        // Placed at the top of a ring (phase = pi/2), the cusp is the point
        // nearest the centre and the body is further out.
        let mut out = Vec::new();
        build_rings(
            &[RingSpec {
                motif: Motif::Teardrop,
                count: 1,
                radius: 0.6,
                scale: 0.3,
                phase: std::f32::consts::FRAC_PI_2,
            }],
            CAP,
            &mut out,
        );
        let nearest = out.iter().fold(f32::INFINITY, |acc, s| {
            acc.min((s.a[0] * s.a[0] + s.a[1] * s.a[1]).sqrt())
        });
        // The cusp is at local (-0.5, 0) -> radius 0.6 - 0.3 * 0.5 = 0.45, and
        // it really is the innermost point of the placed copy.
        assert!(
            (nearest - 0.45).abs() < 1e-3,
            "the cusp should land at 0.45, got {nearest}"
        );
        // ...and it points at the centre: the copy is centred at (0, 0.6), so
        // the innermost vertex is below it.
        let inner = out
            .iter()
            .min_by(|x, y| {
                let r = |s: &SegmentInstance| s.a[0] * s.a[0] + s.a[1] * s.a[1];
                r(x).total_cmp(&r(y))
            })
            .map(|s| s.a)
            .unwrap_or([0.0; 2]);
        assert!(inner[1] < 0.6, "the cusp is on the centre side of the copy");
    }

    /// **The cap is a silent truncation, not a new failure mode** (the plan's
    /// explicit instruction, and ADR-0007's behaviour on the turtle). It also
    /// must not be a *slow* one: the build stops at the cap rather than looping
    /// over a large `count` to count drops it will never emit, which is what the
    /// arithmetic below stands in for.
    #[test]
    fn the_cap_truncates_and_the_drop_is_counted_without_being_surfaced() {
        let cap = 100usize;
        let mut out = Vec::with_capacity(cap);
        // 40 trefoils at 36 segments each: 1 440 wanted against a 100 cap.
        let dropped = build_rings(&[ring(Motif::Trefoil, 40, 0.6, 0.1)], cap, &mut out);
        assert_eq!(out.len(), cap, "the cap is filled exactly");
        assert_eq!(dropped, 40 * 36 - cap, "and the rest is counted");

        // The count is honest across several rings too — the second one is the
        // whole of what a mandala over budget loses.
        let mut out = Vec::new();
        let dropped = build_rings(
            &[
                ring(Motif::Diamond, 10, 0.4, 0.2),
                ring(Motif::Circle, 10, 0.7, 0.2),
            ],
            40,
            &mut out,
        );
        assert_eq!(out.len(), 40);
        assert_eq!(dropped, 10 * 4 + 10 * 24 - 40);

        // At the maximum a preset can declare, the whole roster still fits the
        // floor tier's cap several times over for anything but the densest
        // motif — the budget claim ADR-0079 makes, as a number.
        let widest = Motif::ALL
            .iter()
            .map(|m| m.segments())
            .max()
            .unwrap_or_default();
        assert_eq!(widest, 36, "the trefoil is the densest motif");
        assert_eq!(
            widest * MAX_RING_COUNT as usize,
            18_432,
            "one maximum ring is under the floor tier's cap on its own"
        );
        assert!(widest * MAX_RING_COUNT as usize <= CAP);
    }

    /// **Phase 1 done-when 3, as geometry rather than as an opinion.** Backlog
    /// 0007's "hollow ring" is a *radial occupancy* claim: at `star_rosette`'s
    /// 12-fold / 20-degree rosette every segment sits in one thin band near the
    /// rim and the inner 60 % of the disc holds nothing. The shipped four-ring
    /// roster occupies the interior instead, and the two numbers are measured
    /// side by side here so the pixel-level `cover` comparison has a structural
    /// counterpart that no capture size can wobble.
    #[test]
    fn four_rings_occupy_the_interior_the_bare_rosette_leaves_empty() {
        const SHELLS: usize = 10;
        let occupied = |segs: &[SegmentInstance]| -> usize {
            let mut hit = [false; SHELLS];
            for seg in segs {
                let r = midpoint_radius(seg) / 0.9;
                let k = ((r * SHELLS as f32) as usize).min(SHELLS - 1);
                if let Some(slot) = hit.get_mut(k) {
                    *slot = true;
                }
            }
            hit.iter().filter(|h| **h).count()
        };

        let bare = rosette(12, 20.0);
        let bare_shells = occupied(&bare);

        let mut rings = Vec::new();
        build_rings(&mandala_roster(), CAP, &mut rings);
        let mut combined = bare.clone();
        combined.extend(rings.iter());
        let mandala_shells = occupied(&combined);

        println!(
            "radial shells occupied (of {SHELLS}): bare rosette {bare_shells}, \
             four-ring mandala {mandala_shells}"
        );
        assert_eq!(
            bare_shells, 1,
            "the bare rosette really does live in one shell — this is the \
             'hollow ring'"
        );
        assert!(
            mandala_shells >= 6,
            "a four-ring mandala must reach the interior, got {mandala_shells}"
        );

        // And the segment count is the other half of "materially more figure",
        // comfortably inside the budget ADR-0079 quotes.
        assert_eq!(combined.len(), 1_116, "24 interlace + 1 092 ornament");
        assert!(combined.len() * 17 < CAP, "room for the interlace on top");

        // The radial colour axis was identically flat on the bare rosette (see
        // the module docs); over the combined figure it is a real range, which
        // is what makes `hue_spread` a live lever on a mandala preset.
        let mut u = Vec::new();
        normalized_radii(&combined, &mut u);
        let lo = u.iter().copied().fold(f32::INFINITY, f32::min);
        let hi = u.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        assert!(
            (lo - 0.0).abs() < 1e-5 && (hi - 1.0).abs() < 1e-5,
            "the ramp must span 0..1, got {lo}..{hi}"
        );
    }

    /// A rings-only preset (`tiling = "none"`, order 0) draws the ornament alone
    /// — the composition the reference image is, and the one the cache has to
    /// stay out of the way of. The rosette construction already returns nothing
    /// below `n = 3`; what this pins is that the *cache* then rebuilds rather
    /// than matching its own just-invalidated order-0 state and serving whatever
    /// angle it happened to hold.
    #[test]
    fn order_zero_draws_no_interlace_and_never_reuses_a_stale_rosette() {
        let mut cache = RosetteCache::default();
        cache.request(12, 20.0);
        assert_eq!(cache.segments.len(), 24);

        cache.invalidate();
        let rebuilt = cache.request(0, 20.0);
        assert!(
            rebuilt,
            "an invalidated cache must rebuild, even at order 0"
        );
        assert!(cache.segments.is_empty(), "order 0 draws no interlace");
        assert!(cache.radii.is_empty());
    }

    /// The ornament is a **pure function of its roster** — no clock, no
    /// randomness — so a mandala is the same figure on every device and in every
    /// capture (the determinism rule).
    #[test]
    fn the_ornament_is_deterministic() {
        let mut a = Vec::new();
        let mut b = Vec::new();
        build_rings(&mandala_roster(), CAP, &mut a);
        build_rings(&mandala_roster(), CAP, &mut b);
        assert_eq!(a, b, "same roster -> identical geometry");
        assert!(a.iter().all(|s| s.a[0].is_finite() && s.a[1].is_finite()));
    }

    /// A closed outline is a closed chain and every vertex is a joint
    /// (ADR-0041); an open one is free at its two ends only. Getting this wrong
    /// leaves a notch in the stroke at every motif vertex, which at the counts a
    /// mandala uses is the whole figure.
    #[test]
    fn closed_motifs_join_everywhere_and_open_ones_only_inside() {
        for &m in Motif::ALL {
            let mut out = Vec::new();
            build_rings(&[ring(m, 1, 0.5, 0.3)], CAP, &mut out);
            let flags: Vec<u32> = out.iter().map(|s| s.joined).collect();
            if m.is_closed() {
                assert!(
                    flags.iter().all(|&f| f == JOINED_A | JOINED_B),
                    "{}: a closed outline joins at every vertex, got {flags:?}",
                    m.name()
                );
                // ...and it really does close: the last segment's `b` end is the
                // first segment's `a` end.
                let start = out.first().map(|s| s.a).unwrap_or([f32::NAN; 2]);
                let end = out.last().map(|s| s.b).unwrap_or([f32::NAN; 2]);
                assert!(
                    (start[0] - end[0]).abs() < 1e-5 && (start[1] - end[1]).abs() < 1e-5,
                    "{}: the outline must close",
                    m.name()
                );
            } else {
                assert_eq!(flags.first().copied(), Some(JOINED_B), "{}", m.name());
                assert_eq!(flags.last().copied(), Some(JOINED_A), "{}", m.name());
                for (i, &f) in flags.iter().enumerate() {
                    if i > 0 && i + 1 < flags.len() {
                        assert_eq!(f, JOINED_A | JOINED_B, "{} interior {i}", m.name());
                    }
                }
            }
        }
    }
}
