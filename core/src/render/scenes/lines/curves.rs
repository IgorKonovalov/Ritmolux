//! Parametric curve samplers: pure `t -> (x, y)` functions written straight
//! into a preallocated segment buffer. Cheap enough to resample every frame
//! (ADR-0007 parametric build model), so continuous audio can sweep the shape
//! live. Deterministic: no wall-clock, no randomness — the same parameters
//! always yield the same segments (NFR 6).

// Hot-path panic-denial pragma: the sampler runs every displayed frame.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

use super::CurveFamily;
use super::biarc::{self, Piece};
use super::renderer::{SegmentInstance, miter_extension};

/// The largest share of a Maurer walk's vertices that may be **corners** before
/// `maurer_rose_pieces` declines to fit it — *is this a curve at all?*
///
/// **The two families a Maurer walk holds are not near each other on this
/// number, which is the whole reason a threshold can exist.** At the shipped
/// chord-web steps (`d = 29` to `71`) more than 85 % of the walk's vertices
/// turn past `biarc::CORNER_TURN`; at `d = 2`, the smooth rose, under 15 % do —
/// and those few are the genuine cusps where the radius crosses zero and the
/// figure passes through the origin.
///
/// **A per-corner rule alone would not do**, and that is a measurement rather
/// than a worry: a `d = 29` web is ~90 % corners, so the fit would turn its
/// remaining tenth into arcs and redraw a figure whose chords *are* the figure.
/// The decision has to be about the walk as a whole.
pub const SMOOTH_CORNER_SHARE: f32 = 0.25;

/// The lateral budget every family is fitted to: **one pixel at 1080p**.
///
/// Quoted directly in [`biarc::PIXEL_1080P`] and not divided by anything,
/// because unlike a motif outline a walk is sampled in the frame it is drawn
/// in — `scale` is applied inside the sampler itself.
const FIT_BUDGET: f32 = biarc::PIXEL_1080P;

/// How near a walk's last point must come to its first for the walk to be
/// **closed** — drawn as one loop with a G1 joint where it meets itself, rather
/// than as an open trace whose two free ends happen to touch.
///
/// About a twentieth of a pixel at 1080p. A figure that closes by construction
/// (a Lissajous with whole frequencies) comes back to within f32 rounding of its
/// start — a few `1e-6` — so this sits two orders above that and two below
/// anything a viewer could read as a gap.
const CLOSE_TOLERANCE: f32 = 1.0e-4;

/// Everything a family's walk needs, by name.
///
/// This was eleven positional `f32`s behind `#[allow(clippy::too_many_arguments)]`
/// (Plan 0031 Phase 6). Four of them — `phase`, `scale`, `radial_offset`,
/// `rotation` — are adjacent, same-typed, and easy to transpose: the call would
/// still compile and would draw a different curve. Named fields make that a typo
/// you can see. `Copy` and all-scalar, so it is free at runtime.
///
/// `n`, `d` and `phase` carry a **family-specific** meaning; the field docs
/// give the rose's, and each family's sampler states its own.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CurveParams {
    /// Petal frequency: the `n` in `sin(n * theta)`.
    pub n: f32,
    /// Angular step between successive sampled points, in **degrees** — the
    /// Maurer parameter that turns a smooth rose into its chord web.
    pub d: f32,
    /// Phase (radians) **inside** the sine, so advancing it reshapes the petal
    /// structure. Distinct from [`rotation`](Self::rotation), which spins the
    /// finished figure in screen space. `0.0` is the plain rose.
    pub phase: f32,
    /// Constant added to the radius, opening the rose off the origin into
    /// spiral / annular / rosette forms. A nonzero value makes `r` exceed
    /// `[-1, 1]` (intended — the renderer clips). `0.0` is the plain rose.
    pub radial_offset: f32,
    /// How many points to walk; the chord count when fully drawn.
    pub samples: usize,
    /// Uniform scale applied after the rotation.
    pub scale: f32,
    /// Screen-space rotation of the finished figure, in radians.
    pub rotation: f32,
    /// Reveal fraction in `0..=1` (line-draw-on); `1.0` draws the whole curve.
    pub draw_progress: f32,
    /// One RGB colour for every segment.
    pub color: [f32; 3],
    /// Per-segment line width.
    pub width: f32,
    /// The levers beyond `n`, `d` and `phase` that only some families read.
    pub levers: Levers,
}

/// The family-specific levers: each is read by one family and is **inert** on
/// every other, so none of them can move a rose.
///
/// Grouped apart from [`CurveParams`]' shared fields so a caller that does not
/// draw the family reading one can take [`Levers::default`] and say so.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Levers {
    /// Hypotrochoid: the pen's distance from the rolling circle's centre, in
    /// rolling radii — `1` traces the cusped cycloid, below it the curtate
    /// form, above it the prolate form with loops.
    pub pen: f32,
    /// Superformula: the symmetry number `m`, a whole count of lobes.
    pub sym: f32,
    /// Superformula: `n1`, the outer exponent — low draws a pointed star,
    /// high rounds the figure toward a circle.
    pub sharpness: f32,
    /// Superformula: `n2`, and `n3` before `d` skews it — how the lobes swell.
    pub lobe: f32,
    /// Harmonograph: the pendulums' damping, per radian of `t` — the
    /// amplitude at `t` is `exp(-decay * t)`.
    pub decay: f32,
}

impl Default for Levers {
    /// Each lever at the default its `ParamSpec` declares.
    fn default() -> Self {
        use super::parametric::PARAMS;
        use crate::render::scenes::default_of;
        Self {
            pen: default_of(PARAMS, "pen"),
            sym: default_of(PARAMS, "sym"),
            sharpness: default_of(PARAMS, "sharpness"),
            lobe: default_of(PARAMS, "lobe"),
            decay: default_of(PARAMS, "decay"),
        }
    }
}

/// How many turns of `t` a harmonograph trace runs — the length of the
/// pendulums' swing the figure records.
///
/// A trace that closes (`decay = 0`, whole frequencies) retraces its one figure
/// this many times; a damped one spends them spiralling inward. Four is enough
/// turns for a moderate `decay` to shrink the figure to a fraction of its first
/// swing without leaving a `samples`-point walk too coarse to fit.
pub const HARMONOGRAPH_TURNS: f32 = 4.0;

/// The largest `decay` the harmonograph honours. Past it the whole trace after
/// its first few samples is inside a pixel of the centre.
const MAX_DECAY: f32 = 16.0;

/// The shortest chord a walk may hold and still be handed to the fit — a
/// sixty-fourth of a pixel at 1080p.
///
/// Below it two samples are one point to anything that draws them, and the
/// tangent the fit takes between them is `f32` rounding rather than the
/// curve's. A damped harmonograph's inner turns shrink toward the centre until
/// its samples land this close, which is where its verdict declines the fit.
const MIN_CHORD: f32 = biarc::PIXEL_1080P / 64.0;

/// The smallest radius ratio the hypotrochoid rolls at, whatever `n` asks for.
///
/// The walk runs `d / |n|` turns of the fixed circle, so `|n|` near zero asks
/// for an unbounded walk over a finite `samples`. A quarter keeps the longest
/// walk at `4 d` turns.
const MIN_RATIO: f32 = 0.25;

/// The largest cusp count a hypotrochoid walks, whatever `d` asks for — a
/// ceiling on the walk's length, not on the figure: past a few hundred the
/// cusps are finer than any `samples` resolves.
const MAX_CUSPS: f32 = 1024.0;

/// The largest `pen` the sampler honours; past it the loops dwarf the rolling
/// circle and the figure is a ring of near-circles.
const MAX_PEN: f32 = 16.0;

/// Sample a Maurer rose into `out` (cleared first).
///
/// A Maurer rose walks [`samples`](CurveParams::samples) points at a fixed angular
/// step [`d`](CurveParams::d) degrees, with radius
/// `r = sin(n * theta + phase) + radial_offset`; connecting the successive chords
/// is what draws the characteristic web. With `phase` and `radial_offset` both at
/// `0.0` the formula reduces to the plain `sin(n * theta)` rose (a no-op — the
/// property that kept the golden fixture unchanged when they were added).
///
/// Allocation-free: the caller preallocates `out` with capacity `>= samples`,
/// and this pushes at most `samples` segments (never exceeding that capacity),
/// so no reallocation occurs on the hot path.
pub fn maurer_rose(p: CurveParams, out: &mut Vec<SegmentInstance>) {
    out.clear();
    if p.samples == 0 {
        return;
    }

    let (rot_sin, rot_cos) = p.rotation.sin_cos();
    // How many of the `samples` chords to draw (line-draw-on).
    let progress = p.draw_progress.clamp(0.0, 1.0);
    let drawn = ((p.samples as f32) * progress).round() as usize;
    let drawn = drawn.min(p.samples);

    // The same walk [`maurer_rose_pieces`] samples, term for term — one
    // function, so the polyline path and the fitted one cannot draw two
    // different roses from one set of parameters.
    let point = |k: usize| rose_point(&p, k, rot_sin, rot_cos);

    // A three-point window over the walk, so each joint's interior angle is in
    // hand when the segment carrying it is pushed and `point` is evaluated once
    // per sample rather than three times.
    let (mut back, mut prev) = (point(0), point(0));
    for k in 1..=drawn {
        let cur = point(k);
        // Chained (ADR-0158): consecutive chords share a sampled point, so every
        // interior vertex is a joint, and each end reaches its corner's point by
        // the miter the two arms subtend. The two ends of the walk stay free —
        // and that includes the head of a partially revealed curve, so
        // `draw_progress` never pushes the stroke past the point it actually
        // reached.
        let ext_a = if k > 1 {
            miter_extension(p.width, back, prev, cur)
        } else {
            0.0
        };
        let ext_b = if k < drawn {
            miter_extension(p.width, prev, cur, point(k + 1))
        } else {
            0.0
        };
        out.push(SegmentInstance {
            a: prev,
            b: cur,
            color: p.color,
            width: p.width,
            alpha: 1.0,
            ext_a,
            ext_b,
        });
        back = prev;
        prev = cur;
    }
}

/// Walk a Maurer rose into `points`, and fit it to a **G1 arc chain** in
/// `pieces` (with each piece's place along the walk in `at`) — when the walk is
/// a curve at all.
///
/// Returns **`false` for a chord web**, having filled only `points`: at a large
/// angular step the successive chords *are* the figure, every vertex is a
/// corner, and there is no curve to draw. The caller falls back to
/// [`maurer_rose`], which is why a `d = 43` preset renders exactly what it
/// rendered before this existed, chord for chord.
///
/// The two are one sampler with one parameter between them, so the decision
/// cannot be made at load — only from the walk in hand. A `d` bound to an
/// expression may therefore cross the threshold mid-show; the two renderings
/// converge as it approaches, because a walk that is nearly all corners fits
/// with pieces that are nearly its own chords.
///
/// Allocation-free into preallocated buffers, because this runs **every frame**
/// (ADR-0007's parametric build model gives it no load moment to run at).
///
/// The scene reaches this through [`fit_walk`], which every family shares; this
/// is the rose's arm of it by name, for the tests that pin the rose's verdict.
#[cfg(test)]
pub(crate) fn maurer_rose_pieces(
    p: CurveParams,
    points: &mut Vec<[f32; 2]>,
    pieces: &mut Vec<Piece>,
    at: &mut Vec<f32>,
) -> bool {
    fit_walk(CurveFamily::MaurerRose, p, points, pieces, at).fitted
}

/// What one family contributes to `parametric_curve`: **a walk and a fit
/// verdict**, plus the polyline drawn when the verdict declines.
///
/// The scene owns the buffers, the mirror stage and the colour ramp; a family
/// owns only these three. Adding a family is a [`CurveFamily`] variant and an
/// arm in [`arm`] — the scene never names one.
#[derive(Clone, Copy)]
pub(crate) struct FamilyArm {
    /// Fill `points` (cleared first) with the walk, in the frame the scene draws
    /// in, and say whether it **closes** — its last sample is its first again,
    /// so the fit joins the two ends as an ordinary G1 joint. A closed walk
    /// leaves that repeated sample out.
    pub sample: fn(&CurveParams, &mut Vec<[f32; 2]>) -> bool,
    /// Whether the walk in hand is a **curve** the arc fit should take, or a
    /// figure whose chords are the figure.
    pub fits: fn(&CurveParams, &[[f32; 2]]) -> bool,
    /// The chords drawn when [`fits`](Self::fits) declines: the walk in hand,
    /// whether it closes, and the output buffer (cleared first).
    pub polyline: fn(&CurveParams, &[[f32; 2]], bool, &mut Vec<SegmentInstance>),
}

/// The arm `family` draws through.
pub(crate) fn arm(family: CurveFamily) -> FamilyArm {
    match family {
        CurveFamily::MaurerRose => FamilyArm {
            sample: rose_walk,
            fits: |_, points| biarc::corner_fraction(points, false) <= SMOOTH_CORNER_SHARE,
            // The rose's chord web resamples through `maurer_rose` rather than
            // chaining the walk in hand: that sampler is the one every web
            // golden was blessed against, joints and all.
            polyline: |p, _, _, out| maurer_rose(*p, out),
        },
        CurveFamily::Lissajous => FamilyArm {
            sample: |p, points| {
                periodic_walk(p, std::f32::consts::TAU, |t| lissajous_point(p, t), points)
            },
            // A curve by construction: `sin` against `sin` is smooth everywhere
            // it is not stationary, and the fit keeps any genuine corner a
            // degenerate phase produces.
            fits: |_, _| true,
            polyline: polyline_of,
        },
        CurveFamily::Hypotrochoid => FamilyArm {
            sample: |p, points| {
                let roll = Roll::of(p);
                periodic_walk(p, roll.period, |t| roll.point(p, t), points)
            },
            // A curve at every `pen`: the cusps `pen = 1` traces are genuine
            // corners of the figure, and the fit breaks its chain at them
            // rather than being declined by them.
            fits: |_, _| true,
            polyline: polyline_of,
        },
        CurveFamily::Superformula => FamilyArm {
            sample: |p, points| {
                let shape = Gielis::of(p);
                let peak = shape.peak(p.samples);
                periodic_walk(p, std::f32::consts::TAU, |t| shape.point(t, peak), points)
            },
            // A curve with genuine corners — a star's tips at a low
            // `sharpness` — which the fit breaks its chain at.
            fits: |_, _| true,
            polyline: polyline_of,
        },
        CurveFamily::Harmonograph => FamilyArm {
            sample: |p, points| {
                let period = std::f32::consts::TAU * HARMONOGRAPH_TURNS;
                periodic_walk(p, period, |t| harmonograph_point(p, t), points)
            },
            // **Read off the walk, not the family.** At `decay = 0` the trace
            // is a Lissajous figure and a curve; at a hard `decay` its later
            // turns collapse onto the centre faster than the samples follow
            // them, and a walk holding two samples the fit cannot tell apart —
            // or one that is mostly corners — is drawn as its chords instead.
            fits: |_, points| {
                shortest_chord(points) >= MIN_CHORD
                    && biarc::corner_fraction(points, false) <= SMOOTH_CORNER_SHARE
            },
            polyline: polyline_of,
        },
    }
}

/// A fitted walk's outcome: whether the arc chain was built, and whether the
/// walk closes on itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Fit {
    /// `true`: `pieces` holds the G1 chain. `false`: the verdict declined and
    /// the caller draws [`FamilyArm::polyline`] over the walk in `points`.
    pub fitted: bool,
    /// The walk comes back to its start, so the chain wraps and neither end is
    /// free.
    pub closed: bool,
}

/// Walk `family` into `points` and, when its verdict takes the walk, fit it to
/// a **G1 arc chain** in `pieces` (with each piece's place along the walk in
/// `at`).
///
/// An empty or one-point walk is reported as fitted with nothing in it — there
/// is no chord for a polyline to draw either.
///
/// Allocation-free into preallocated buffers, because this runs every frame.
pub(crate) fn fit_walk(
    family: CurveFamily,
    p: CurveParams,
    points: &mut Vec<[f32; 2]>,
    pieces: &mut Vec<Piece>,
    at: &mut Vec<f32>,
) -> Fit {
    let arm = arm(family);
    pieces.clear();
    at.clear();
    let closed = (arm.sample)(&p, points);
    if points.len() < 2 {
        return Fit {
            fitted: true,
            closed,
        };
    }
    if !(arm.fits)(&p, points) {
        return Fit {
            fitted: false,
            closed,
        };
    }
    biarc::fit(points, closed, FIT_BUDGET, pieces, at);
    Fit {
        fitted: true,
        closed,
    }
}

/// How many of the `samples` chords a `draw_progress` reveal draws.
fn drawn(p: &CurveParams) -> usize {
    let progress = p.draw_progress.clamp(0.0, 1.0);
    (((p.samples as f32) * progress).round() as usize).min(p.samples)
}

/// The Maurer walk: `drawn + 1` points at the rose's fixed angular step.
///
/// Open, never closed: a Maurer walk ends where it ends. Even the closed-up
/// cases arrive back at their start as a matter of arithmetic rather than of
/// construction, and telling the fit otherwise would have it join two ends that
/// a `draw_progress` reveal has no reason to bring together.
fn rose_walk(p: &CurveParams, points: &mut Vec<[f32; 2]>) -> bool {
    points.clear();
    if p.samples == 0 {
        return false;
    }
    let (rot_sin, rot_cos) = p.rotation.sin_cos();
    for k in 0..=drawn(p) {
        points.push(rose_point(p, k, rot_sin, rot_cos));
    }
    false
}

/// Point `k` of the walk, in the frame [`maurer_rose`] draws in.
fn rose_point(p: &CurveParams, k: usize, rot_sin: f32, rot_cos: f32) -> [f32; 2] {
    let theta = (k as f32 * p.d).to_radians();
    let r = (p.n * theta + p.phase).sin() + p.radial_offset;
    let (ts, tc) = theta.sin_cos();
    let x = r * tc;
    let y = r * ts;
    [
        (x * rot_cos - y * rot_sin) * p.scale,
        (x * rot_sin + y * rot_cos) * p.scale,
    ]
}

/// Walk a figure given as `point(p, t)` over `t` in `[0, period]`: `samples`
/// equal steps, of which a `draw_progress` reveal takes the first `drawn`.
///
/// **Closed exactly when the whole trace is drawn and it is periodic over
/// `period`** — decided from the figure, not from the family, because whether a
/// Lissajous closes depends on whether `n` and `d` are whole, and both are
/// expressions. A partial reveal is always open, so its drawing head stays a
/// free end.
///
/// Periodic means the trace **carries on the way it began**: its end is its
/// start, *and* one step past the end is its second sample. The first alone is
/// not enough, and not by accident — every Lissajous-type trace at `phase = 0`
/// starts at the origin, and a damped harmonograph ends there too, arriving
/// along a spiral a twelfth the size of the swing it left on. Joining those two
/// ends would draw a closing joint the figure does not have.
///
/// `point` returns the figure in its own unit frame; the rotation and `scale`
/// are applied here, once, for every family. It is a closure so a family can
/// resolve its construction once per walk rather than once per sample.
fn periodic_walk(
    p: &CurveParams,
    period: f32,
    point: impl Fn(f32) -> [f32; 2],
    points: &mut Vec<[f32; 2]>,
) -> bool {
    points.clear();
    if p.samples == 0 {
        return false;
    }
    let (rot_sin, rot_cos) = p.rotation.sin_cos();
    let place = |t: f32| {
        let [x, y] = point(t);
        [
            (x * rot_cos - y * rot_sin) * p.scale,
            (x * rot_sin + y * rot_cos) * p.scale,
        ]
    };
    let drawn = drawn(p);
    let step = period / p.samples as f32;
    for k in 0..=drawn {
        points.push(place(step * k as f32));
    }
    let returns =
        |a: Option<&[f32; 2]>, b: [f32; 2]| a.is_some_and(|&a| dist(a, b) <= CLOSE_TOLERANCE);
    let closes = drawn == p.samples
        && points.len() > 3
        && points
            .last()
            .is_some_and(|&end| returns(points.first(), end))
        && returns(points.get(1), place(step * (p.samples + 1) as f32));
    if closes {
        // The repeated start is the wrap itself; a closed fit supplies it.
        points.pop();
    }
    closes
}

/// The Lissajous figure in its unit square: `x = sin(n t + phase)`,
/// `y = sin(d t)`, so `n` and `d` are the two frequencies and `phase` — a
/// fraction of a turn, as the parameter reference states it — the offset
/// between them. Whole `n` and `d` close over one `TAU` of `t`.
fn lissajous_point(p: &CurveParams, t: f32) -> [f32; 2] {
    let phase = finite_or_zero(p.phase) * std::f32::consts::TAU;
    [
        (finite_or_zero(p.n) * t + phase).sin(),
        (finite_or_zero(p.d) * t).sin(),
    ]
}

/// The rolling construction a hypotrochoid walk is drawn from, resolved once
/// from the parameters: a fixed circle of radius `1`, a rolling circle of
/// radius `rolling`, and a pen `pen` from the rolling circle's centre.
#[derive(Clone, Copy)]
struct Roll {
    /// The rolling circle's radius, `1 / |n|`.
    rolling: f32,
    /// `true` when the circle rolls **outside** the fixed one — a negative `n`.
    outside: bool,
    /// The pen's distance from the rolling circle's centre, in world units of
    /// the construction (`pen` rolling radii).
    pen: f32,
    /// How far `t` runs: `d` cusps, one every `TAU / |n|` of the fixed circle.
    period: f32,
    /// The largest distance the pen can reach from the centre, `|1 -+ r| + pen`
    /// — what the figure is divided by to land in the unit disc.
    extent: f32,
}

impl Roll {
    fn of(p: &CurveParams) -> Self {
        let n = finite_or_zero(p.n);
        let ratio = n.abs().max(MIN_RATIO);
        let rolling = 1.0 / ratio;
        let outside = n < 0.0;
        let pen = if p.levers.pen.is_finite() {
            p.levers.pen.clamp(0.0, MAX_PEN)
        } else {
            1.0
        } * rolling;
        let cusps = finite_or_zero(p.d).clamp(1.0, MAX_CUSPS);
        let centre = if outside {
            1.0 + rolling
        } else {
            1.0 - rolling
        };
        Self {
            rolling,
            outside,
            pen,
            period: std::f32::consts::TAU * cusps / ratio,
            // A pen at the rolling circle's centre on a circle rolling its own
            // radius inside is a figure of one point; the floor keeps that a
            // point rather than a division by zero.
            extent: (centre.abs() + pen).max(f32::EPSILON),
        }
    }
}

impl Roll {
    /// The hypotrochoid (or, for a negative `n`, the epitrochoid) in the unit
    /// disc: the rolling circle's centre runs round a circle of radius `1 -+ r`
    /// while the pen turns about it `(1 -+ r) / r` times as fast. `phase`, a
    /// fraction of a turn, sets where on the rolling circle the pen starts.
    ///
    /// At `t = 0` and `phase = 0` the pen sits on the positive x-axis, so with
    /// `pen = 1` point `0` is a cusp: the outermost point of a hypocycloid and
    /// the innermost of an epicycloid.
    fn point(self, p: &CurveParams, t: f32) -> [f32; 2] {
        let start = finite_or_zero(p.phase) * std::f32::consts::TAU;
        let (x, y) = if self.outside {
            let centre = 1.0 + self.rolling;
            let spin = centre / self.rolling * t + start;
            (
                centre * t.cos() - self.pen * spin.cos(),
                centre * t.sin() - self.pen * spin.sin(),
            )
        } else {
            let centre = 1.0 - self.rolling;
            let spin = centre / self.rolling * t + start;
            (
                centre * t.cos() + self.pen * spin.cos(),
                centre * t.sin() - self.pen * spin.sin(),
            )
        };
        [x / self.extent, y / self.extent]
    }
}

/// Gielis' superformula, resolved once from the parameters:
/// `r(theta) = (|cos(m theta / 4)|^n2 + |sin(m theta / 4)|^n3)^(-1 / n1)`, with
/// `m = sym`, `n1 = sharpness`, `n2 = lobe` and `n3 = lobe * d`.
///
/// # Why the radius is carried as a logarithm
///
/// `n1` near zero raises the sum to a huge negative power: at `sharpness =
/// 0.05` a sum of `1e-3` is `r = 1e60`, past `f32`'s range, and the vertex is
/// `inf`. So the sampler works in `ln r = -ln(sum) / n1`, which is bounded —
/// the sum lies in `[f32::MIN_POSITIVE, 2]` and `n1` is floored — and divides
/// the figure by its own largest radius by **subtracting** the peak logarithm
/// before exponentiating. Every vertex is then `exp(<= 0)` from the centre:
/// finite and inside the unit disc, at every parameter, as a property of the
/// arithmetic rather than of a clamp on the output.
#[derive(Clone, Copy)]
struct Gielis {
    /// `m / 4`, with `m` the whole symmetry number.
    quarter_sym: f32,
    /// `n1`, floored above zero.
    sharpness: f32,
    /// `n2` and `n3`, the two exponents `lobe` binds and `d` skews apart.
    cos_power: f32,
    sin_power: f32,
}

/// The largest symmetry number the superformula honours — the ceiling on a
/// figure whose lobes a `samples`-point walk can still resolve.
const MAX_SYM: f32 = 64.0;

/// The smallest `sharpness` the superformula honours. The radius's logarithm
/// is `-ln(sum) / n1`, so this floor is what bounds it: at `0.05` it is at most
/// `1750` in magnitude, comfortably finite before the peak is subtracted.
const MIN_SHARPNESS: f32 = 0.05;

/// The largest exponent `lobe` (or `lobe * d`) reaches. Past it `|cos|^n` is
/// a spike narrower than any walk samples, and `0.7^64` is already `1e-10`.
const MAX_LOBE: f32 = 64.0;

/// The largest `d` skew the superformula honours, as a ratio `n3 / n2`.
const MAX_SKEW: f32 = 16.0;

impl Gielis {
    fn of(p: &CurveParams) -> Self {
        let sym = finite_or_zero(p.levers.sym).round().clamp(0.0, MAX_SYM);
        let sharpness = if p.levers.sharpness.is_finite() {
            p.levers.sharpness.max(MIN_SHARPNESS)
        } else {
            1.0
        };
        let lobe = finite_or_zero(p.levers.lobe).clamp(0.0, MAX_LOBE);
        let skew = finite_or_zero(p.d).clamp(0.0, MAX_SKEW);
        Self {
            quarter_sym: sym * 0.25,
            sharpness,
            cos_power: lobe,
            sin_power: (lobe * skew).min(MAX_LOBE),
        }
    }

    /// `ln r(theta)`, finite for every `theta` — see the type's docs.
    fn ln_radius(self, theta: f32) -> f32 {
        let (sin, cos) = (self.quarter_sym * theta).sin_cos();
        let sum = cos.abs().powf(self.cos_power) + sin.abs().powf(self.sin_power);
        -sum.max(f32::MIN_POSITIVE).ln() / self.sharpness
    }

    /// The largest `ln r` over the **whole** trace, so the figure's size is a
    /// property of the figure: a `draw_progress` reveal draws part of it at the
    /// size it will be, rather than rescaling each frame to what is drawn.
    fn peak(self, samples: usize) -> f32 {
        let step = std::f32::consts::TAU / samples.max(1) as f32;
        (0..=samples)
            .map(|k| self.ln_radius(step * k as f32))
            .fold(f32::MIN, f32::max)
    }

    /// The point at `theta`, divided by the peak radius: `exp(ln r - peak)`,
    /// which is at most `1`.
    fn point(self, theta: f32, peak: f32) -> [f32; 2] {
        let r = (self.ln_radius(theta) - peak).min(0.0).exp();
        let (sin, cos) = theta.sin_cos();
        [r * cos, r * sin]
    }
}

/// The harmonograph in its unit square: two damped pendulums,
/// `x = exp(-decay t) sin(n t + phase)` and `y = exp(-decay t) sin(d t)`, over
/// [`HARMONOGRAPH_TURNS`] turns of `t`. At `decay = 0` this is exactly
/// [`lissajous_point`].
fn harmonograph_point(p: &CurveParams, t: f32) -> [f32; 2] {
    let decay = finite_or_zero(p.levers.decay).clamp(0.0, MAX_DECAY);
    let amplitude = (-decay * t).exp();
    let [x, y] = lissajous_point(p, t);
    [amplitude * x, amplitude * y]
}

/// The shortest chord between consecutive points of `points`, or infinity for
/// a walk with fewer than two.
fn shortest_chord(points: &[[f32; 2]]) -> f32 {
    points
        .windows(2)
        .map(|pair| match pair {
            [a, b] => dist(*a, *b),
            _ => f32::INFINITY,
        })
        .fold(f32::INFINITY, f32::min)
}

/// The walk in hand as chained chords (ADR-0158): every interior vertex is a
/// joint and so, for a closed walk, is the wrap. An open walk's two ends are
/// free, which keeps a `draw_progress` head from pushing the stroke past the
/// point it reached.
fn polyline_of(p: &CurveParams, points: &[[f32; 2]], closed: bool, out: &mut Vec<SegmentInstance>) {
    out.clear();
    let n = points.len();
    if n < 2 {
        return;
    }
    let chords = if closed { n } else { n - 1 };
    let at = |k: usize| points.get(k % n).copied().unwrap_or([0.0, 0.0]);
    for k in 0..chords {
        let (a, b) = (at(k), at(k + 1));
        let ext_a = if closed || k > 0 {
            miter_extension(p.width, at(k + n - 1), a, b)
        } else {
            0.0
        };
        let ext_b = if closed || k + 1 < chords {
            miter_extension(p.width, a, b, at(k + 2))
        } else {
            0.0
        };
        out.push(SegmentInstance {
            a,
            b,
            color: p.color,
            width: p.width,
            alpha: 1.0,
            ext_a,
            ext_b,
        });
    }
}

/// `v`, or `0` when an expression produced a non-finite value — so a `NaN`
/// parameter draws a degenerate figure rather than a `NaN` vertex.
fn finite_or_zero(v: f32) -> f32 {
    if v.is_finite() { v } else { 0.0 }
}

fn dist(a: [f32; 2], b: [f32; 2]) -> f32 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    (dx * dx + dy * dy).sqrt()
}

#[cfg(test)]
mod tests;
