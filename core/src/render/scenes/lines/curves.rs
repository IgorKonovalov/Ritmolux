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

use super::renderer::{JOINED_A, JOINED_B, SegmentInstance};

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

    let point = |k: usize| -> [f32; 2] {
        let theta = (k as f32 * p.d).to_radians();
        let r = (p.n * theta + p.phase).sin() + p.radial_offset;
        let (ts, tc) = theta.sin_cos();
        // Base point in the unit disc, then rotate and scale.
        let x = r * tc;
        let y = r * ts;
        [
            (x * rot_cos - y * rot_sin) * p.scale,
            (x * rot_sin + y * rot_cos) * p.scale,
        ]
    };

    let mut prev = point(0);
    for k in 1..=drawn {
        let cur = point(k);
        // Chained (ADR-0041): consecutive chords share a sampled point, so every
        // interior vertex is a joint. The two ends of the walk stay free — and
        // that includes the head of a partially revealed curve, so
        // `draw_progress` never pushes the stroke half a width past the point it
        // actually reached.
        let mut joined = 0;
        if k > 1 {
            joined |= JOINED_A;
        }
        if k < drawn {
            joined |= JOINED_B;
        }
        out.push(SegmentInstance {
            a: prev,
            b: cur,
            color: p.color,
            width: p.width,
            joined,
        });
        prev = cur;
    }
}

#[cfg(test)]
mod tests;
