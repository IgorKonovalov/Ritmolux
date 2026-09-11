//! The escape-time arm's arithmetic on the CPU: a line-for-line port of
//! `cpow` and `escape_time` in `shader.rs`, in `f32`, for the tests to read the
//! shader's numerics through.
//!
//! **The WGSL is the implementation; this is the instrument.** An edit to either
//! function in the shader moves this one with it, and
//! `the_gpu_draws_the_set_the_mirror_computes` renders the two side by side so
//! a mirror that has drifted from the shader fails rather than vouching for it.
// Test-only arithmetic; indexing a fixed-size array is not a hot-path hazard.
#![allow(clippy::indexing_slicing)]

use super::{C_LIMIT, ESCAPE_RADIUS_RANGE, MAX_POWER, MIN_POWER, applied_iterations, bounded};

/// The shader's `ITERATIONS_PER_PALETTE`.
pub(super) const ITERATIONS_PER_PALETTE: f32 = 32.0;

/// The escape-time uniform as the scene packs it — raw parameter values in,
/// with the same CPU-side clamps `render` applies.
#[derive(Clone, Copy, Debug)]
pub(super) struct Escape {
    pub c: [f32; 2],
    pub iterations: f32,
    pub radius: f32,
    pub power: f32,
    pub interior: f32,
    pub mandelbrot: bool,
}

impl Escape {
    /// The values `render` would hand the shader for these raw parameters.
    pub(super) fn packed(self) -> Self {
        Self {
            c: [
                bounded(self.c[0], -C_LIMIT, C_LIMIT, super::DEFAULT_C_RE),
                bounded(self.c[1], -C_LIMIT, C_LIMIT, super::DEFAULT_C_IM),
            ],
            iterations: applied_iterations(self.iterations, super::MAX_ITERATIONS),
            radius: bounded(
                self.radius,
                ESCAPE_RADIUS_RANGE[0],
                ESCAPE_RADIUS_RANGE[1],
                super::DEFAULT_ESCAPE_RADIUS,
            ),
            power: bounded(self.power, MIN_POWER, MAX_POWER, super::DEFAULT_POWER),
            interior: bounded(self.interior, 0.0, 1.0, super::DEFAULT_INTERIOR),
            mandelbrot: self.mandelbrot,
        }
    }
}

/// What the arm hands the colour stage for one point, plus the two counts the
/// tests reason about.
#[derive(Clone, Copy, Debug)]
pub(super) struct Out {
    pub coord: f32,
    pub light: f32,
    pub escaped: bool,
    /// The integer step count `n` the orbit escaped at (or the budget).
    pub steps: u32,
    /// The smooth count `nu` (0 for a point that never escaped).
    pub nu: f32,
}

/// The shader's `cpow`.
fn cpow(z: [f32; 2], power: f32) -> [f32; 2] {
    let mut out = z;
    if power.fract() == 0.0 {
        let mut i = 2.0f32;
        while i <= 8.0 {
            if i <= power {
                out = [out[0] * z[0] - out[1] * z[1], out[0] * z[1] + out[1] * z[0]];
            }
            i += 1.0;
        }
    } else {
        let r2 = z[0] * z[0] + z[1] * z[1];
        let theta = z[1].atan2(z[0]);
        let rp = (0.5 * power * r2.max(1e-30).ln()).exp();
        out = if r2 < 1e-30 {
            [0.0, 0.0]
        } else {
            [rp * (power * theta).cos(), rp * (power * theta).sin()]
        };
    }
    out
}

/// The shader's `escape_time`, at field coordinate `p`, for an already
/// [`packed`](Escape::packed) uniform.
pub(super) fn escape(p: [f32; 2], e: &Escape) -> Out {
    let iterations = e.iterations as u32;
    let q = [p[0].clamp(-64.0, 64.0), p[1].clamp(-64.0, 64.0)];
    let mut z = if e.mandelbrot { [0.0, 0.0] } else { q };
    let c = if e.mandelbrot { q } else { e.c };
    let r2 = e.radius * e.radius;

    let mut n = 0u32;
    let mut escaped = false;
    while n < iterations {
        let w = cpow(z, e.power);
        z = [w[0] + c[0], w[1] + c[1]];
        n += 1;
        if z[0] * z[0] + z[1] * z[1] > r2 {
            escaped = true;
            break;
        }
    }

    let dot = z[0] * z[0] + z[1] * z[1];
    if escaped {
        let log_mod = 0.5 * dot.min(1e37).ln();
        let nu = n as f32 + 1.0 - (log_mod / e.radius.ln()).max(1.0).ln() / e.power.ln();
        Out {
            coord: nu.max(0.0) / ITERATIONS_PER_PALETTE,
            light: 1.0,
            escaped,
            steps: n,
            nu,
        }
    } else {
        Out {
            coord: 0.25 * dot.sqrt().min(2.0),
            light: e.interior.clamp(0.0, 1.0),
            escaped,
            steps: n,
            nu: 0.0,
        }
    }
}
