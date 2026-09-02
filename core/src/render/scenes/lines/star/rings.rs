//! The mandala interior: rings of placed motifs (ADR-0079).
//!
//! A `[generator] rings` roster, the motion levers that breathe it, and
//! [`build_rings`], which walks the roster into the two instance buffers the
//! line renderer draws. Every copy lands through one [`Placement`] -- a
//! similarity -- whichever of the four kinds of motif it is.

// Hot-path panic-denial pragma (Plan 0002 Phase 2; render/ is scanned by the
// hygiene guard).
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

// A continuation of one module split across three files, so it needs the names
// `star/mod.rs` has in scope.
use super::*;

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
pub(super) const MAX_RING_SPREAD: f32 = 4.0;
/// The largest `ring_scale` a binding may reach. Motifs span about one unit, so
/// at `8` a single copy covers the whole frame; beyond that the ring is a blob
/// whatever its count.
pub(super) const MAX_RING_SCALE: f32 = 8.0;

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
pub(super) const RING_PHASE_STEP: f32 = 0.001;
/// The `ring_spread` hysteresis. Same arithmetic at the same outer radius:
/// `0.001 * 0.82 * 540 = 0.44 px`.
pub(super) const RING_SPREAD_STEP: f32 = 0.001;
/// The `ring_scale` hysteresis. A motif `scale` is an order of magnitude smaller
/// than a ring radius (`0.13` to `0.46` across the shipped presets), so the same
/// sub-pixel step is a looser number: `0.002 * 0.46 * 540 = 0.50 px`.
pub(super) const RING_SCALE_STEP: f32 = 0.002;

/// How many of a ring's repeated element actually get placed: copies for every
/// motif but [`Motif::Scallop`], whose `count` is a **lobe count** and has a
/// floor of [`MIN_SCALLOP_LOBES`].
///
/// One function, called by both the cap-free `wanted` fold and the placement
/// loop, because a raised count that only one of them knew about would make the
/// drop count a fiction.
pub(super) fn placed_count(ring: &RingSpec) -> u32 {
    if ring.motif.is_scallop() {
        ring.count.max(MIN_SCALLOP_LOBES)
    } else {
        ring.count.max(1)
    }
}

/// Where one copy of a motif sits: the ring's placement, as a **similarity** --
/// scale about the motif's own origin, then the radial offset, then the rotation
/// to this copy's angle.
///
/// It being a similarity is what lets the polyline path measure its miters on
/// the *unplaced* outline: a similarity preserves angles.
#[derive(Clone, Copy)]
struct Placement {
    scale: f32,
    radius: f32,
    sin: f32,
    cos: f32,
}

impl Placement {
    fn point(self, p: [f32; 2]) -> [f32; 2] {
        let x = p[0] * self.scale + self.radius;
        let y = p[1] * self.scale;
        [x * self.cos - y * self.sin, x * self.sin + y * self.cos]
    }
}

/// One ring's resolved geometry: how many copies, where the first one starts,
/// and the radius and scale with the ring motion already folded in.
#[derive(Clone, Copy)]
struct Ring {
    count: u32,
    base_phase: f32,
    radius: f32,
    scale: f32,
}

impl Ring {
    /// The ring's own configuration, moved. Computed once per ring because it is
    /// constant across every copy on it.
    fn of(spec: &RingSpec, index: usize, motion: RingMotion) -> Self {
        Self {
            count: placed_count(spec),
            base_phase: spec.phase + ring_direction(index) * motion.phase,
            radius: spec.radius * motion.spread,
            scale: spec.scale * motion.scale,
        }
    }

    /// Copy `i`'s angle, and the placement that puts a local point there.
    fn placement(self, i: u32) -> (f32, Placement) {
        let theta = TAU * i as f32 / self.count as f32 + self.base_phase;
        let (sin, cos) = theta.sin_cos();
        (
            theta,
            Placement {
                scale: self.scale,
                radius: self.radius,
                sin,
                cos,
            },
        )
    }
}

/// The two instance buffers a ring fills, and the **one** cap they share: a
/// truncation is measured across both, because both are drawn out of one budget.
struct Buffers<'a> {
    out: &'a mut Vec<SegmentInstance>,
    arcs: &'a mut Vec<ArcInstance>,
    cap: usize,
}

impl Buffers<'_> {
    fn full(&self) -> bool {
        self.out.len() + self.arcs.len() >= self.cap
    }
}

/// A unit-width, unit-colour arc instance. The four ring paths differ in the
/// geometry they compute, never in these two, which the draw layer overwrites
/// per frame anyway.
fn arc_at(centre: [f32; 2], radius: f32, angle_start: f32, angle_sweep: f32) -> ArcInstance {
    ArcInstance {
        centre,
        radius,
        angle_start,
        angle_sweep,
        color: [1.0, 1.0, 1.0],
        width: 0.01,
    }
}

/// The scalloped boundary: **one closed chain of `count` lobes**, not `count`
/// copies of a motif (ADR-0079's open question, and the form the user chose at
/// Plan 0065 Phase 2). Each lobe is an exact arc, and consecutive lobes share
/// the point where they leave the base circle, so the chain closes on itself.
///
/// Returns `false` when the cap stopped it, which ends the whole build.
fn push_scallop(ring: Ring, buf: &mut Buffers<'_>) -> bool {
    let half_span = PI / ring.count as f32;
    // `scale` is the lobe's depth here, and `radius` the circle it bulges from
    // — see `Motif::Scallop`. Both already carry the ring motion, so the
    // boundary breathes under `ring_spread` and deepens under `ring_scale` like
    // any other ring.
    let lobe = scallop_lobe(ring.radius.abs(), ring.scale, half_span);
    for i in 0..ring.count {
        if buf.full() {
            return false;
        }
        // Lobe `i` is the same arc turned into its own sector; the sectors tile
        // the circle exactly, which is what makes the chain closed and its cusps
        // evenly spaced.
        let (theta, place) = ring.placement(i);
        let (x, y) = (lobe.centre[0], lobe.centre[1]);
        buf.arcs.push(arc_at(
            [x * place.cos - y * place.sin, x * place.sin + y * place.cos],
            lobe.radius,
            lobe.start + theta,
            lobe.sweep,
        ));
    }
    true
}

/// A circular motif is **one arc per copy**, with no interior joint at any scale
/// (ADR-0098), rather than `SMOOTH_SAMPLES` segments and as many additive beads.
fn push_arc_motif(shape: ArcShape, ring: Ring, buf: &mut Buffers<'_>) -> bool {
    for i in 0..ring.count {
        if buf.full() {
            return false;
        }
        let (theta, place) = ring.placement(i);
        buf.arcs.push(arc_at(
            place.point(shape.centre),
            // `abs` for the reason `LineInstance::rotate_scale` gives: a
            // negative `scale` reflects the motif, and the reflected circle has
            // the same positive radius about the centre already reflected.
            (shape.radius * ring.scale).abs(),
            // Placement is one rotation for both the orientation and the
            // position, exactly as it is for a polyline motif — the arc carries
            // its own orientation, so the rotation reaches it as an angle rather
            // than through its endpoints.
            shape.start + theta,
            shape.sweep,
        ));
    }
    true
}

/// A fitted motif is a G1 chain of arcs (ADR-0098): the same placement, piece by
/// piece rather than copy by copy, and the two kinds land in the two buffers
/// they belong to.
fn push_chain_motif(chain: &[Piece], closed: bool, ring: Ring, buf: &mut Buffers<'_>) -> bool {
    for i in 0..ring.count {
        let (theta, place) = ring.placement(i);
        for (k, piece) in chain.iter().enumerate() {
            if buf.full() {
                return false;
            }
            match *piece {
                Piece::Arc {
                    centre,
                    radius: curvature,
                    start,
                    sweep,
                } => buf.arcs.push(arc_at(
                    place.point(centre),
                    // `abs` for the reason `LineInstance::rotate_scale` gives: a
                    // negative `scale` reflects the piece, and the reflected arc
                    // has the same positive radius about the centre `place`
                    // already reflected.
                    (curvature * ring.scale).abs(),
                    start + theta,
                    sweep,
                )),
                Piece::Line { a, b } => {
                    // A chain is a chain (ADR-0158): every piece continues its
                    // neighbour, and a closed one continues it at both ends.
                    // That is true across a corner too — the extension is what
                    // covers the wedge between two strokes, and a corner is
                    // exactly where there is one, so the length is the miter the
                    // two arms subtend.
                    let (ext_a, ext_b) =
                        Piece::chain_extensions(chain, k, PLACEHOLDER_WIDTH, closed);
                    buf.out.push(SegmentInstance {
                        a: place.point(a),
                        b: place.point(b),
                        color: [1.0, 1.0, 1.0],
                        width: PLACEHOLDER_WIDTH,
                        alpha: 1.0,
                        ext_a,
                        ext_b,
                    });
                }
            }
        }
    }
    true
}

/// Everything else: a sampled outline, placed edge by edge.
fn push_polyline(pts: &[[f32; 2]], closed: bool, ring: Ring, buf: &mut Buffers<'_>) -> bool {
    let n = pts.len();
    if n < 2 {
        return true;
    }
    let edges = if closed { n } else { n - 1 };
    for i in 0..ring.count {
        let (_, place) = ring.placement(i);
        for e in 0..edges {
            if buf.full() {
                return false;
            }
            let (Some(&a), Some(&b)) = (pts.get(e), pts.get((e + 1) % n)) else {
                continue;
            };
            // A closed outline is a closed chain, so every vertex is a joint
            // (ADR-0158); an open one is free at its two ends only. A joint
            // reaches its corner's point by the miter its two edges subtend,
            // measured on the UNPLACED outline — `Placement` is a similarity and
            // a similarity preserves angles.
            let ext_a = (closed || e > 0)
                .then(|| pts.get((e + n - 1) % n))
                .flatten()
                .map_or(0.0, |&before| {
                    miter_extension(PLACEHOLDER_WIDTH, before, a, b)
                });
            let ext_b = (closed || e + 1 < edges)
                .then(|| pts.get((e + 2) % n))
                .flatten()
                .map_or(0.0, |&after| {
                    miter_extension(PLACEHOLDER_WIDTH, a, b, after)
                });
            buf.out.push(SegmentInstance {
                a: place.point(a),
                b: place.point(b),
                color: [1.0, 1.0, 1.0],
                width: PLACEHOLDER_WIDTH,
                alpha: 1.0,
                ext_a,
                ext_b,
            });
        }
    }
    true
}

/// Build every ring's geometry into `out` and `arcs` (both cleared first) under
/// `motion`, returning how many instances the shared cap dropped.
///
/// The placement, which is the whole of ADR-0079's geometry: copy `i` of a ring
/// of `k` sits at angle `2*pi*i/k + phase`, and because each motif is authored
/// with **outward along `+x`**, that one rotation supplies both the copy's
/// position and its orientation — the motif is offset to `(radius, 0)` in the
/// ring's own frame and the whole frame is turned. [`Placement`] is that
/// rotation, and all four paths below go through it.
///
/// `motion` rides on top of that and changes no count: it adds a **signed**
/// `phase` per ring, multiplies `radius` by `spread` and `scale` by `scale`. At
/// [`RingMotion::STATIC`] every one of those is an exact IEEE identity, so the
/// static roster is reproduced rather than approximated.
///
/// Four kinds of motif, in the order the roster resolves them: a scallop is one
/// closed chain of lobes, a circular motif one exact arc per copy, a fitted one
/// a G1 chain per copy, and everything else a sampled outline.
///
/// **Truncation at `cap` is silent by construction** (ADR-0007's behaviour on the
/// turtle, kept deliberately): the caller drops the count, `presets/README.md`
/// documents what a preset over budget looks like, and nothing surfaces it. The
/// count is returned anyway because it is what makes the cap testable. The first
/// path that stops on the cap ends the build — `wanted` below is what a cap-free
/// build would have emitted, which is the only way to report the drop without
/// running the loop past the cap.
///
/// Build-time: runs from `configure`, never from `update`. Written panic-free
/// under the module's pragma all the same.
pub(crate) fn build_rings(
    rings: &[RingSpec],
    motion: RingMotion,
    cap: usize,
    out: &mut Vec<SegmentInstance>,
    arcs: &mut Vec<ArcInstance>,
) -> usize {
    out.clear();
    arcs.clear();
    let wanted = rings.iter().fold(0usize, |acc, ring| {
        acc.saturating_add(
            ring.motif
                .instances()
                .saturating_mul(placed_count(ring) as usize),
        )
    });

    let mut buf = Buffers { out, arcs, cap };
    let mut pts: Vec<[f32; 2]> = Vec::new();
    for (index, spec) in rings.iter().enumerate() {
        let ring = Ring::of(spec, index, motion);
        let motif = spec.motif;
        let within_cap = if motif.is_scallop() {
            push_scallop(ring, &mut buf)
        } else if let Some(shape) = motif.arc_shape() {
            push_arc_motif(shape, ring, &mut buf)
        } else if let Some(chain) = motif.chain() {
            push_chain_motif(chain, motif.is_closed(), ring, &mut buf)
        } else {
            motif.outline(&mut pts);
            push_polyline(&pts, motif.is_closed(), ring, &mut buf)
        };
        if !within_cap {
            break;
        }
    }
    wanted.saturating_sub(buf.out.len() + buf.arcs.len())
}
