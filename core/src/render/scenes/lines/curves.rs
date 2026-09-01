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

use super::biarc::{self, Piece};
use super::renderer::SegmentInstance;

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

/// The lateral budget the rose is fitted to: **one pixel at 1080p**.
///
/// Quoted directly in [`biarc::PIXEL_1080P`] and not divided by anything,
/// because unlike a motif outline this walk is sampled in the frame it is drawn
/// in — `scale` is applied inside [`maurer_rose`] itself.
const ROSE_FIT_BUDGET: f32 = biarc::PIXEL_1080P;

/// Everything [`maurer_rose`] needs, by name.
///
/// This was eleven positional `f32`s behind `#[allow(clippy::too_many_arguments)]`
/// (Plan 0031 Phase 6). Four of them — `phase`, `scale`, `radial_offset`,
/// `rotation` — are adjacent, same-typed, and easy to transpose: the call would
/// still compile and would draw a different curve. Named fields make that a typo
/// you can see. `Copy` and all-scalar, so it is free at runtime.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RoseParams {
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
}

/// Sample a Maurer rose into `out` (cleared first).
///
/// A Maurer rose walks [`samples`](RoseParams::samples) points at a fixed angular
/// step [`d`](RoseParams::d) degrees, with radius
/// `r = sin(n * theta + phase) + radial_offset`; connecting the successive chords
/// is what draws the characteristic web. With `phase` and `radial_offset` both at
/// `0.0` the formula reduces to the plain `sin(n * theta)` rose (a no-op — the
/// property that kept the golden fixture unchanged when they were added).
///
/// Allocation-free: the caller preallocates `out` with capacity `>= samples`,
/// and this pushes at most `samples` segments (never exceeding that capacity),
/// so no reallocation occurs on the hot path.
pub fn maurer_rose(p: RoseParams, out: &mut Vec<SegmentInstance>) {
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

    let mut prev = point(0);
    for k in 1..=drawn {
        let cur = point(k);
        // Chained (ADR-0158): consecutive chords share a sampled point, so every
        // interior vertex is a joint. The two ends of the walk stay free — and
        // that includes the head of a partially revealed curve, so
        // `draw_progress` never pushes the stroke past the point it actually
        // reached.
        let ext_a = if k > 1 { p.width } else { 0.0 };
        let ext_b = if k < drawn { p.width } else { 0.0 };
        out.push(SegmentInstance {
            a: prev,
            b: cur,
            color: p.color,
            width: p.width,
            alpha: 1.0,
            ext_a,
            ext_b,
        });
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
pub(crate) fn maurer_rose_pieces(
    p: RoseParams,
    points: &mut Vec<[f32; 2]>,
    pieces: &mut Vec<Piece>,
    at: &mut Vec<f32>,
) -> bool {
    points.clear();
    pieces.clear();
    at.clear();
    if p.samples == 0 {
        return true;
    }
    let (rot_sin, rot_cos) = p.rotation.sin_cos();
    let progress = p.draw_progress.clamp(0.0, 1.0);
    let drawn = (((p.samples as f32) * progress).round() as usize).min(p.samples);
    for k in 0..=drawn {
        points.push(rose_point(&p, k, rot_sin, rot_cos));
    }
    if points.len() < 2 {
        return true;
    }
    if biarc::corner_fraction(points, false) > SMOOTH_CORNER_SHARE {
        return false;
    }
    // Open, not closed: a Maurer walk ends where it ends. Even the closed-up
    // cases arrive back at their start as a matter of arithmetic rather than
    // of construction, and telling the fit otherwise would have it join two
    // ends that a `draw_progress` reveal has no reason to bring together.
    biarc::fit(points, false, ROSE_FIT_BUDGET, pieces, at);
    true
}

/// Point `k` of the walk, in the frame [`maurer_rose`] draws in.
fn rose_point(p: &RoseParams, k: usize, rot_sin: f32, rot_cos: f32) -> [f32; 2] {
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

#[cfg(test)]
mod tests;
