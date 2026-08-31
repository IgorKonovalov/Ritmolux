//! Biarc fitting: a sampled outline in, a **G1-continuous chain of circular
//! arcs** out (ADR-0098).
//!
//! This is the half of ADR-0098 that makes the cheap primitive enough. A
//! sampled polyline shows its joints because it is only **C0** — the tangent
//! jumps at every vertex, and the eye reads a tangent discontinuity as a corner
//! however fine the sampling. A chain built here is **G1**: consecutive pieces
//! share both an endpoint and a tangent direction there, by construction, so
//! the same handful of pieces that read as a faceted polygon read as a drawn
//! curve. The approximation error shows up as a curve slightly in the wrong
//! *place* rather than as a visible vertex.
//!
//! **A corner in the source outline stays a corner.** The fit breaks its chain
//! wherever consecutive chords turn by more than `CORNER_TURN`, because a
//! trefoil's three cusps and a diamond's four vertices are the figure, not
//! sampling artefacts, and a run that is all corners comes back as the polyline
//! it was given.
//!
//! **That is not enough to leave a Maurer chord web alone, and the measurement
//! says so.** A `d = 29` walk is about 90 % corners — but the other 10 % are
//! runs of two and three chords that the fit happily replaces with arcs, which
//! would redraw a figure whose chords *are* the figure. So the decision of
//! whether a walk is a curve at all is the **caller's**, taken from
//! `corner_fraction` before the fit is ever called; see
//! `curves::maurer_rose_pieces`.
//!
//! Pure: no clock, no randomness, no global state, so the same outline always
//! yields the same chain (the determinism rule). Allocation-free into a
//! caller-preallocated `out`, because `parametric_curve` resamples every frame.

// Hot-path panic-denial pragma. The fit is build-time for the motif roster and
// **per frame** for `parametric_curve`, whose build model is a resample every
// frame (ADR-0007) and which therefore has no load moment to run it at.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

use std::f32::consts::{PI, TAU};

/// The largest angle, in radians, by which a fitted piece's tangent may differ
/// from the outline's at any sample the piece spans.
///
/// **The G1 property costs nothing and is not what this bounds.** Consecutive
/// pieces share a tangent by construction whatever this number is; what a
/// tangent error does instead is let a piece lean off the curve *between* the
/// samples it interpolates.
///
/// **Derived to land on the same order as the caller's lateral budget, so
/// neither criterion is silently dominated.** A tangent error `e` held across a
/// piece of length `L` displaces the drawn curve from the authored one by about
/// `e * L / 4`. The fit's longest piece on the roster is about a fifth of a
/// motif's own unit span, so `0.05` rad — 2.9 degrees — works out at `2.5e-3`
/// units against the `4.0e-3` `star.rs` passes. Both bite: `petal`'s measured
/// tangent error sits on this budget, `teardrop`'s and `trefoil`'s sit on the
/// lateral one.
///
/// **A tangent budget alone would not bound the piece count, and this is the
/// one place that has to be said out loud.** Two of the three fitted motifs
/// carry a point of *unbounded curvature* — `petal`'s and `teardrop`'s tips,
/// where the outline's `1.6` exponent makes `y ~ |x|^0.8` and the tangent turns
/// arbitrarily fast through vertical. No circular arc tracks that, so a
/// tangent-only criterion subdivides without limit toward the tip and buys
/// nothing: the whole region where it is failing is `7.6e-6` units wide, five
/// hundred times narrower than a pixel. The lateral budget is what stops it.
pub(crate) const TANGENT_BUDGET: f32 = 0.05;

/// One pixel at 1080p, in the renderer's world-y units — the unit every
/// caller's lateral budget is quoted in.
///
/// The renderer maps world y `[-1, 1]` onto the target's height, so 1080 rows
/// make one unit 540 px. A caller that fits in the frame it draws in passes a
/// multiple of this directly ([`curves`](super::curves)); a caller that fits in
/// a **local** frame later scaled down — a motif outline, authored spanning one
/// unit and placed at a ring `scale` — divides by the largest scale it will be
/// drawn at, because that is where its error is largest
/// ([`star`](super::star)).
pub(crate) const PIXEL_1080P: f32 = 1.0 / 540.0;

/// The chord-to-chord turn above which a vertex is a **corner of the figure**
/// rather than a sample of a curve, and the chain breaks there.
///
/// Sixty degrees, and the gap it sits in is wide at both ends. Below it: a
/// smooth outline resampled at [`FIT_SAMPLES`] turns about 1.4 degrees per
/// chord, and even `petal`'s tip — the sharpest feature in the roster that is
/// not a corner — turns 27 degrees across the two chords that straddle it.
/// Above it: `trefoil`'s three cusps run into the origin and come back out
/// along the same ray, a 180-degree turn, and a Maurer chord web at `d = 29` or
/// more turns past 60 degrees at every single vertex.
pub(crate) const CORNER_TURN: f32 = PI / 3.0;

/// How far one piece may turn, in radians.
///
/// The biarc construction reads the turn between its two end tangents through
/// an angle wrapped into `(-PI, PI]`, so a span that genuinely turns further is
/// ambiguous — it would be fitted as the short way round. Nine tenths of half a
/// turn keeps the construction clear of that wrap with room to spare, and costs
/// nothing real: a closed outline needs at least three pieces to come back to
/// itself either way.
const MAX_PIECE_TURN: f32 = 0.9 * PI;

/// How near a single arc's far tangent must come to the span's before the pair
/// collapses to that one arc.
///
/// **This is an equality test, not a budget, and the difference is the whole
/// G1 property.** Collapsing at [`TANGENT_BUDGET`] would leave the next piece
/// starting up to 2.9 degrees off where this one ended — a tangent
/// discontinuity at every joint, which is precisely the defect ADR-0098 exists
/// to remove, arrived at by way of an optimization. At `1e-4` rad the collapse
/// only happens where the biarc it replaces would have been that same arc
/// twice, so it costs an instance and changes no geometry.
const G1_TOLERANCE: f32 = 1e-4;

/// The largest radius a fitted arc may carry before the piece is emitted as a
/// straight line instead.
///
/// The fragment shades an arc by `abs(length(p - c) - r)`, and in `f32` that
/// difference of two large nearly-equal numbers loses exactly the precision the
/// stroke needs: one ulp at magnitude `R` is `R * 2^-23`, so at `R = 64` the
/// distance resolves to `7.6e-6` — a two-hundredth of a pixel at 1080p — and at
/// `R = 1e6` it resolves to `0.06`, twenty stroke widths. A piece flat enough
/// to want a radius past this deviates from its own chord by less than `L^2 /
/// (8R)`, which for the longest piece the fit emits is under half a pixel — so
/// the straight line it becomes is not an approximation anyone can see, and a
/// straight line is the right primitive for a straight run.
const MAX_RADIUS: f32 = 64.0;

/// Samples the motif roster's outlines are re-drawn at for fitting.
///
/// The fit sees only samples, so the sampling has to be finer than the features
/// it is meant to resolve — `Motif::outline`'s 24 is the *reference polyline*'s
/// resolution and would cap a piece boundary to a 15-degree grid. 256 is dense
/// enough that the chord-to-chord turn on a smooth motif is about 1.4 degrees,
/// well under [`CORNER_TURN`], and it costs one build-time pass over an array.
pub(crate) const FIT_SAMPLES: usize = 256;

/// One piece of a fitted chain, in the fit's own frame.
///
/// Two variants rather than one because a straight run is not a curve: a
/// distance field is strictly more expensive for a line than a quad is, and an
/// arc's radius for a flat piece is exactly the regime [`MAX_RADIUS`] rules
/// out. The caller turns these into the two instance kinds `LineRenderer`
/// already draws.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Piece {
    /// A circular arc: centre of curvature, radius, start angle and **signed**
    /// sweep, exactly the quantities `ArcInstance` carries.
    Arc {
        centre: [f32; 2],
        radius: f32,
        start: f32,
        sweep: f32,
    },
    /// A straight run from `a` to `b`.
    Line { a: [f32; 2], b: [f32; 2] },
}

impl Piece {
    /// Where the piece begins.
    pub(crate) fn start_point(self) -> [f32; 2] {
        match self {
            Piece::Arc {
                centre,
                radius,
                start,
                ..
            } => on_circle(centre, radius, start),
            Piece::Line { a, .. } => a,
        }
    }

    /// Where the piece ends.
    pub(crate) fn end_point(self) -> [f32; 2] {
        match self {
            Piece::Arc {
                centre,
                radius,
                start,
                sweep,
            } => on_circle(centre, radius, start + sweep),
            Piece::Line { b, .. } => b,
        }
    }

    /// The unit direction of travel where the piece begins — the incoming half
    /// of the G1 property.
    pub(crate) fn start_tangent(self) -> [f32; 2] {
        match self {
            Piece::Arc { start, sweep, .. } => arc_tangent(start, sweep),
            Piece::Line { a, b } => normalize([b[0] - a[0], b[1] - a[1]]),
        }
    }

    /// The unit direction of travel where the piece ends — the outgoing half.
    pub(crate) fn end_tangent(self) -> [f32; 2] {
        match self {
            Piece::Arc { start, sweep, .. } => arc_tangent(start + sweep, sweep),
            Piece::Line { a, b } => normalize([b[0] - a[0], b[1] - a[1]]),
        }
    }

    /// How far `p` is from this piece, and the piece's unit tangent at the
    /// nearest point on it — the two quantities the two budgets are read
    /// against.
    ///
    /// Outside an arc's angular span the nearer endpoint stands in, which is
    /// the same convention the arc fragment shades by, so the fit is judging
    /// the shape the GPU will actually draw.
    fn measure(self, p: [f32; 2]) -> (f32, [f32; 2]) {
        match self {
            Piece::Arc {
                centre,
                radius,
                start,
                sweep,
            } => {
                let v = [p[0] - centre[0], p[1] - centre[1]];
                let len = norm(v);
                let angle = v[1].atan2(v[0]);
                // How far into the sweep `p` sits, measured the way the sweep
                // runs. Inside the span the arc itself is nearest; outside it,
                // one of the two ends is.
                let along = (sweep.signum() * (angle - start)).rem_euclid(TAU);
                if along <= sweep.abs() {
                    let here = start + sweep.signum() * along;
                    ((len - radius).abs(), arc_tangent(here, sweep))
                } else {
                    let (a, b) = (self.start_point(), self.end_point());
                    let (da, db) = (dist(p, a), dist(p, b));
                    if da <= db {
                        (da, self.start_tangent())
                    } else {
                        (db, self.end_tangent())
                    }
                }
            }
            Piece::Line { a, b } => {
                let d = [b[0] - a[0], b[1] - a[1]];
                let len2 = dot(d, d);
                let t = if len2 > f32::EPSILON {
                    (dot([p[0] - a[0], p[1] - a[1]], d) / len2).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let foot = [a[0] + t * d[0], a[1] + t * d[1]];
                (dist(p, foot), self.start_tangent())
            }
        }
    }
}

/// What one [`fit`] cost and how well it did — the numbers reported
/// against the segment counts they replace, and the ones a test reads
/// instead of re-deriving the fit's internals.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct FitStats {
    /// [`Piece::Arc`]s emitted.
    pub arcs: usize,
    /// [`Piece::Line`]s emitted.
    pub lines: usize,
    /// The largest tangent error, in radians, at any spanned sample.
    pub max_tangent_err: f32,
    /// The largest distance, in the fit's frame, from any spanned sample to the
    /// piece that spans it.
    pub max_deviation: f32,
    /// Chain breaks the fit made because the outline turned past
    /// [`CORNER_TURN`] — corners of the figure, where G1 does not hold and is
    /// not wanted.
    pub corners: usize,
}

/// Fit a G1 chain of circular arcs to `points`, into `out` (cleared first), and
/// each piece's position along the walk into `at` (likewise) as a **fractional
/// sample index**.
///
/// `closed` says whether the last point joins back to the first; a closed
/// outline with no corner anywhere comes back to its start tangentially, so the
/// closing joint is G1 like every other one.
///
/// The chain **interpolates**: every piece boundary sits exactly on one of the
/// input samples, with the outline's own tangent there. So the fit can only be
/// wrong *between* samples, which is what the two budgets bound, and it can
/// never drift away from the figure.
///
/// **`at` is not bookkeeping.** A piece spans as many samples as the budgets
/// allow, so the `k`th piece is not the `k`th chord and anything that colours a
/// figure along its own path — `parametric_curve`'s ramp, which runs
/// `0..1` across the walk — reads a fitted chain by this and not by index. A
/// caller that has no such axis passes a scratch buffer and ignores it.
pub(crate) fn fit(
    points: &[[f32; 2]],
    closed: bool,
    lateral: f32,
    out: &mut Vec<Piece>,
    at: &mut Vec<f32>,
) -> FitStats {
    out.clear();
    at.clear();
    let mut stats = FitStats::default();
    let n = points.len();
    if n < 2 {
        return stats;
    }
    let f = Fitter {
        points,
        closed,
        lateral,
    };
    // Vertices run `0..=chords`, and for a closed outline the last one is the
    // first one again — which is what lets the closing joint be an ordinary
    // interior joint rather than a special case.
    let chords = if closed { n } else { n - 1 };

    // Corner **vertices**, counted directly rather than tallied as the run loop
    // breaks: a closed outline whose vertex 0 is a corner has a corner at the
    // joint where the chain wraps, and the loop starting there never breaks on
    // it. That is a real tangent discontinuity in the drawn figure — a square
    // has four, not three.
    stats.corners = (0..chords).filter(|&k| f.is_corner(k)).count();

    let mut run_start = 0usize;
    while run_start < chords {
        let run_end = f.next_break(run_start, chords);
        let mut i = run_start;
        while i < run_end {
            let j = f.longest_span(i, run_end);
            let (first, second) = f.piece_pair(i, j);
            // The joint of a biarc sits somewhere inside the span; half way
            // along it is close enough for a colour axis and needs no arc-length
            // integral to say so.
            let midway = 0.5 * (i + j) as f32;
            for (piece, walk) in [(Some(first), i as f32), (second, midway)]
                .into_iter()
                .filter_map(|(piece, walk)| piece.map(|piece| (piece, walk)))
            {
                match piece {
                    Piece::Arc { .. } => stats.arcs += 1,
                    Piece::Line { .. } => stats.lines += 1,
                }
                out.push(piece);
                at.push(walk);
            }
            let (tangent_err, deviation) = f.worst(i, j, first, second);
            stats.max_tangent_err = stats.max_tangent_err.max(tangent_err);
            stats.max_deviation = stats.max_deviation.max(deviation);
            i = j;
        }
        run_start = run_end;
    }
    stats
}

/// The share of `points`' vertices at which the walk turns past
/// [`CORNER_TURN`] — **is this a curve at all?**, as one number in `0..=1`.
///
/// A caller reads it before fitting, because the fit itself answers the
/// question the expensive way: a walk that is all corners breaks into
/// one-chord runs and comes back as the polyline it was given, having done
/// `O(n)` work to change nothing. `parametric_curve` samples a **Maurer walk**,
/// which is a chord web at a large angular step and a smooth rose at a small
/// one, and the two are the same code with one parameter between them — so the
/// decision cannot be made at load, only from the geometry in hand.
pub(crate) fn corner_fraction(points: &[[f32; 2]], closed: bool) -> f32 {
    let f = Fitter {
        points,
        closed,
        // Unused: nothing here fits anything, it only counts turns.
        lateral: 0.0,
    };
    let chords = f.chords();
    if chords < 2 {
        return 0.0;
    }
    let corners = (0..chords).filter(|&k| f.is_corner(k)).count();
    corners as f32 / chords as f32
}

/// The fit's working state: the samples and whether they close. Every method is
/// a pure function of those two, which is what makes the whole fit one.
struct Fitter<'a> {
    points: &'a [[f32; 2]],
    closed: bool,
    /// The lateral budget, in the input's own units — see [`fit`].
    lateral: f32,
}

impl Fitter<'_> {
    /// How many chords the outline has — one per sample when it closes, one
    /// fewer when it does not.
    fn chords(&self) -> usize {
        if self.closed {
            self.points.len()
        } else {
            self.points.len().saturating_sub(1)
        }
    }

    /// Vertex `k`, wrapping for a closed outline so vertex `chords` is vertex
    /// `0` again.
    fn at(&self, k: usize) -> [f32; 2] {
        let n = self.points.len().max(1);
        self.points.get(k % n).copied().unwrap_or([0.0, 0.0])
    }

    /// The chord leaving vertex `k`, as a unit direction.
    fn chord(&self, k: usize) -> [f32; 2] {
        let (a, b) = (self.at(k), self.at(k + 1));
        normalize([b[0] - a[0], b[1] - a[1]])
    }

    /// Whether vertex `k` is a corner of the figure — the chords either side of
    /// it turn past [`CORNER_TURN`]. Only an interior vertex can be one; an
    /// open outline's two ends have nothing to turn against.
    fn is_corner(&self, k: usize) -> bool {
        let chords = self.chords();
        if chords == 0 || (!self.closed && (k == 0 || k >= chords)) {
            return false;
        }
        let prev = if k == 0 { chords - 1 } else { k - 1 };
        turn(self.chord(prev), self.chord(k)).abs() > CORNER_TURN
    }

    /// The first vertex after `from` at which the chain must break: a corner,
    /// or the end of the outline.
    fn next_break(&self, from: usize, chords: usize) -> usize {
        ((from + 1)..chords)
            .find(|&k| self.is_corner(k))
            .unwrap_or(chords)
    }

    /// The unit tangent a piece **leaves** vertex `k` along.
    ///
    /// A corner and an open outline's first vertex have no incoming chord to
    /// average with, so the outgoing chord is the tangent there; everywhere
    /// else it is the central difference, which is the same value
    /// [`tangent_in`](Self::tangent_in) returns — and that equality is the G1
    /// property at every interior joint.
    fn tangent_out(&self, k: usize) -> [f32; 2] {
        if self.is_corner(k) || (!self.closed && k == 0) {
            self.chord(k)
        } else {
            self.central(k)
        }
    }

    /// The unit tangent a piece **arrives** at vertex `k` along.
    fn tangent_in(&self, k: usize) -> [f32; 2] {
        if self.is_corner(k) || (!self.closed && k >= self.chords()) {
            self.chord(k.saturating_sub(1))
        } else {
            self.central(k)
        }
    }

    /// The central-difference tangent at vertex `k`: the direction from its
    /// predecessor to its successor, which is second-order accurate on a
    /// uniformly sampled curve and needs no derivative from the caller.
    fn central(&self, k: usize) -> [f32; 2] {
        let chords = self.chords().max(1);
        let prev = if k == 0 { chords - 1 } else { k - 1 };
        let (a, b) = (self.at(prev), self.at(k + 1));
        normalize([b[0] - a[0], b[1] - a[1]])
    }

    /// The largest `j` such that one piece pair spans `i..j` inside the run
    /// ending at `run_end` and stays inside both budgets.
    ///
    /// Doubling, then a bisection — `O(span log span)` work per piece rather
    /// than the `O(span^2)` a linear walk with a full recheck would cost, which
    /// is what makes the fit affordable on `parametric_curve`'s per-frame path.
    /// A one-chord span is accepted unconditionally: it has no interior sample
    /// to be wrong about, and there is nothing shorter to fall back to.
    fn longest_span(&self, i: usize, run_end: usize) -> usize {
        let max = run_end - i;
        if max <= 1 {
            return i + 1;
        }
        let mut lo = 1usize;
        let mut hi = 2usize;
        loop {
            if hi >= max {
                if self.spans(i, i + max) {
                    return i + max;
                }
                hi = max;
                break;
            }
            if self.spans(i, i + hi) {
                lo = hi;
                hi = hi.saturating_mul(2);
            } else {
                break;
            }
        }
        while hi > lo + 1 {
            let mid = lo + (hi - lo) / 2;
            if self.spans(i, i + mid) {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        i + lo
    }

    /// Whether one piece pair may span `i..j`: the outline must not turn
    /// further than [`MAX_PIECE_TURN`] over it, and every sample strictly
    /// inside must sit within both budgets of the fit.
    fn spans(&self, i: usize, j: usize) -> bool {
        let mut turned = 0.0f32;
        for k in i..j.saturating_sub(1) {
            turned += turn(self.chord(k), self.chord(k + 1)).abs();
            if turned > MAX_PIECE_TURN {
                return false;
            }
        }
        let (first, second) = self.piece_pair(i, j);
        let (tangent_err, deviation) = self.worst(i, j, first, second);
        tangent_err <= TANGENT_BUDGET && deviation <= self.lateral
    }

    /// The worst tangent error and lateral deviation the fit of `i..j` shows at
    /// the samples strictly inside it. `(0, 0)` for a one-chord span, which has
    /// none.
    fn worst(&self, i: usize, j: usize, first: Piece, second: Option<Piece>) -> (f32, f32) {
        let mut worst_tangent = 0.0f32;
        let mut worst_deviation = 0.0f32;
        for k in (i + 1)..j {
            let p = self.at(k);
            // The nearer of the pair stands for the fit at this sample: a biarc
            // is two arcs meeting at one point, so whichever is closer is the
            // one the sample is being drawn by.
            let (mut deviation, mut tangent) = first.measure(p);
            if let Some(second) = second {
                let (d2, t2) = second.measure(p);
                if d2 < deviation {
                    deviation = d2;
                    tangent = t2;
                }
            }
            worst_deviation = worst_deviation.max(deviation);
            worst_tangent = worst_tangent.max(turn(self.central(k), tangent).abs());
        }
        (worst_tangent, worst_deviation)
    }

    /// The one or two pieces that carry the span `i..j`, interpolating both
    /// endpoints and both tangents exactly.
    ///
    /// **A single arc first**, and that is not an optimization detail: a span
    /// that one arc already fits tangentially is one instance rather than two,
    /// and the whole circular family reaches the GPU that way. Only when one
    /// arc arrives pointing the wrong way does the span cost a biarc.
    fn piece_pair(&self, i: usize, j: usize) -> (Piece, Option<Piece>) {
        let (p0, p1) = (self.at(i), self.at(j));
        let t0 = self.tangent_out(i);
        let t1 = self.tangent_in(j);

        let single = arc_or_line(p0, t0, p1);
        if turn(single.end_tangent(), t1).abs() <= G1_TOLERANCE {
            return (single, None);
        }
        biarc(p0, t0, p1, t1)
    }
}

/// The two arcs of a biarc through `(p0, t0)` and `(p1, t1)`.
///
/// **The construction, in one line of geometry.** An arc from `P` to `Q` leaves
/// `P` and arrives at `Q` along directions that are mirror images in the chord
/// `PQ`, so writing the chord angles of the two halves as `alpha` and `beta`,
/// tangent continuity at the joint is exactly `alpha - beta = (theta0 -
/// theta1) / 2`. Every joint satisfying that lies on one circle through `P0`
/// and `P1` (the inscribed-angle theorem); this picks the member equidistant
/// from both, which is the symmetric choice and the one that degenerates
/// gracefully — equal end tangents put the joint at the midpoint.
///
/// The turn is read through [`turn`], which wraps into `(-PI, PI]`, so the
/// offset angle is at most a quarter turn and the `tan` below is bounded by 1.
/// That is why the caller caps a span's turn at [`MAX_PIECE_TURN`] rather than
/// trusting the formula past half a turn.
fn biarc(p0: [f32; 2], t0: [f32; 2], p1: [f32; 2], t1: [f32; 2]) -> (Piece, Option<Piece>) {
    let v = [p1[0] - p0[0], p1[1] - p0[1]];
    let len = norm(v);
    if len <= f32::EPSILON {
        return (Piece::Line { a: p0, b: p1 }, None);
    }
    let vhat = [v[0] / len, v[1] / len];
    let nhat = [-vhat[1], vhat[0]];
    let offset = 0.5 * len * (0.25 * turn(t1, t0)).tan();
    let joint = [
        0.5 * (p0[0] + p1[0]) + offset * nhat[0],
        0.5 * (p0[1] + p1[1]) + offset * nhat[1],
    ];
    let first = arc_or_line(p0, t0, joint);
    // The second half is built **backwards from `p1`** so its far tangent is
    // `t1` exactly rather than approximately, then flipped. Building it forward
    // from the joint would need the joint tangent, which is the one quantity
    // the construction derives rather than knows.
    let second = reverse(arc_or_line(p1, [-t1[0], -t1[1]], joint));
    (first, Some(second))
}

/// The arc leaving `p` along the unit direction `t` and ending at `q` — or the
/// straight line, when the circle through them would be flatter than
/// [`MAX_RADIUS`] can be shaded at.
fn arc_or_line(p: [f32; 2], t: [f32; 2], q: [f32; 2]) -> Piece {
    let line = Piece::Line { a: p, b: q };
    // The left normal of the direction of travel: the centre lies along it, and
    // its sign is what says which way the arc bends.
    let nhat = [-t[1], t[0]];
    let c = [q[0] - p[0], q[1] - p[1]];
    let chord2 = dot(c, c);
    if chord2 <= f32::EPSILON {
        return line;
    }
    let denom = 2.0 * dot(nhat, c);
    // `|chord2 / denom| > MAX_RADIUS`, without the division — a flat span makes
    // `denom` vanish and the quotient is exactly the radius.
    if denom.abs() * MAX_RADIUS <= chord2 {
        return line;
    }
    let signed = chord2 / denom;
    let centre = [p[0] + signed * nhat[0], p[1] + signed * nhat[1]];
    let radius = signed.abs();
    if !centre[0].is_finite() || !centre[1].is_finite() || !radius.is_finite() {
        return line;
    }
    let start = (p[1] - centre[1]).atan2(p[0] - centre[0]);
    let end = (q[1] - centre[1]).atan2(q[0] - centre[0]);
    // A centre to the **left** of the direction of travel is a counter-clockwise
    // arc, which is the whole of the sweep's sign.
    let sweep = if signed > 0.0 {
        (end - start).rem_euclid(TAU)
    } else {
        -((start - end).rem_euclid(TAU))
    };
    Piece::Arc {
        centre,
        radius,
        start,
        sweep,
    }
}

/// The same piece traversed the other way — same points, same picture, opposite
/// direction of travel.
fn reverse(piece: Piece) -> Piece {
    match piece {
        Piece::Arc {
            centre,
            radius,
            start,
            sweep,
        } => Piece::Arc {
            centre,
            radius,
            start: start + sweep,
            sweep: -sweep,
        },
        Piece::Line { a, b } => Piece::Line { a: b, b: a },
    }
}

/// A point on a circle at `angle`.
fn on_circle(centre: [f32; 2], radius: f32, angle: f32) -> [f32; 2] {
    let (sin, cos) = angle.sin_cos();
    [centre[0] + radius * cos, centre[1] + radius * sin]
}

/// The unit direction of travel at `angle` on an arc sweeping `sweep` — the
/// radius turned a quarter turn, the way the sweep runs.
fn arc_tangent(angle: f32, sweep: f32) -> [f32; 2] {
    let (sin, cos) = angle.sin_cos();
    let s = if sweep < 0.0 { -1.0 } else { 1.0 };
    [-s * sin, s * cos]
}

/// The signed angle from unit direction `a` to unit direction `b`, wrapped into
/// `(-PI, PI]`.
fn turn(a: [f32; 2], b: [f32; 2]) -> f32 {
    let cross = a[0] * b[1] - a[1] * b[0];
    let along = dot(a, b);
    cross.atan2(along)
}

fn dot(a: [f32; 2], b: [f32; 2]) -> f32 {
    a[0] * b[0] + a[1] * b[1]
}

fn norm(v: [f32; 2]) -> f32 {
    dot(v, v).sqrt()
}

fn dist(a: [f32; 2], b: [f32; 2]) -> f32 {
    norm([b[0] - a[0], b[1] - a[1]])
}

/// `v` scaled to unit length; `+x` for a degenerate input, so a repeated sample
/// yields a direction rather than a NaN that would poison the whole chain.
fn normalize(v: [f32; 2]) -> [f32; 2] {
    let len = norm(v);
    if len > f32::EPSILON {
        [v[0] / len, v[1] / len]
    } else {
        [1.0, 0.0]
    }
}

#[cfg(test)]
mod tests;
