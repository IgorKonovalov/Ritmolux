//! A CPU statement of the step shader: the seeding hash, the reseed disc and
//! each family's rule, written out independently so a GPU field can be held to
//! it cell for cell. Test-only.
//!
//! Every cell is a whole number here — `0` or `1`, or a `cyclic` colour index —
//! which is exactly what the shader writes into the state channel, so the
//! comparison is equality rather than a tolerance.
#![allow(clippy::indexing_slicing)]

use super::Stamp;

/// The lowbias32 mixer `gpu::HASH_WGSL` defines, bit for bit.
pub(super) fn mix32(v: u32) -> u32 {
    let mut h = v;
    h ^= h >> 16;
    h = h.wrapping_mul(0x7FEB_352D);
    h ^= h >> 15;
    h = h.wrapping_mul(0x846C_A68B);
    h ^= h >> 16;
    h
}

/// The shader's `seeded`: 1 when the cell's hash under `seed` falls below
/// `threshold`.
pub(super) fn seeded(x: u32, y: u32, seed: u32, threshold: u32) -> u8 {
    let h = mix32(x ^ mix32(y ^ mix32(seed)));
    u8::from((h >> 8) < threshold)
}

/// A whole `n` x `n` field seeded under `seed`, row-major from the top-left.
pub(super) fn seed_field(n: u32, seed: u32, threshold: u32) -> Vec<u8> {
    (0..n * n)
        .map(|i| seeded(i % n, i / n, seed, threshold))
        .collect()
}

/// The cell at `(x, y)`, wrapped on a torus and dead past a border otherwise.
fn at(cells: &[u8], n: u32, wrap: bool, x: i64, y: i64) -> u8 {
    let n = i64::from(n);
    if !wrap && (x < 0 || y < 0 || x >= n || y >= n) {
        return 0;
    }
    let (x, y) = (x.rem_euclid(n), y.rem_euclid(n));
    cells[(y * n + x) as usize]
}

/// One `life_like` generation: bit `k` of `birth` (dead cell) or `survive`
/// (live cell) decides a cell with `k` live neighbours among its eight.
pub(super) fn life_step(cells: &[u8], n: u32, wrap: bool, birth: u32, survive: u32) -> Vec<u8> {
    (0..n * n)
        .map(|i| {
            let (x, y) = (i64::from(i % n), i64::from(i / n));
            let mut count = 0u32;
            for dy in -1..=1 {
                for dx in -1..=1 {
                    if dx != 0 || dy != 0 {
                        count += u32::from(at(cells, n, wrap, x + dx, y + dy));
                    }
                }
            }
            let mask = if cells[i as usize] == 1 {
                survive
            } else {
                birth
            };
            u8::from((mask >> count) & 1 == 1)
        })
        .collect()
}

/// One `larger_than_life` generation, summed the unseparated way on purpose —
/// every cell of the box read directly — so the GPU's row-and-column sum is held
/// to an independent statement of it. `birth` and `survive` are the inclusive
/// whole-count intervals the shader receives.
pub(super) fn ltl_step(
    cells: &[u8],
    n: u32,
    wrap: bool,
    radius: u32,
    birth: [u32; 2],
    survive: [u32; 2],
) -> Vec<u8> {
    let r = i64::from(radius);
    (0..n * n)
        .map(|i| {
            let (x, y) = (i64::from(i % n), i64::from(i / n));
            let mut count = 0u32;
            for dy in -r..=r {
                for dx in -r..=r {
                    if dx != 0 || dy != 0 {
                        count += u32::from(at(cells, n, wrap, x + dx, y + dy));
                    }
                }
            }
            let [lo, hi] = if cells[i as usize] == 1 {
                survive
            } else {
                birth
            };
            u8::from(count >= lo && count <= hi)
        })
        .collect()
}

/// A `cyclic` seed: every cell one of `states` colours, from the same hash.
pub(super) fn seed_colours(n: u32, seed: u32, states: u32) -> Vec<u8> {
    (0..n * n)
        .map(|i| {
            let h = mix32((i % n) ^ mix32((i / n) ^ mix32(seed)));
            ((h >> 8) % states) as u8
        })
        .collect()
}

/// One `cyclic` generation: a cell advances to the next colour round the cycle
/// when at least `threshold` of its eight neighbours hold it. A neighbour past a
/// dead border counts for nothing.
pub(super) fn cyclic_step(
    cells: &[u8],
    n: u32,
    wrap: bool,
    states: u32,
    threshold: u32,
) -> Vec<u8> {
    let ni = i64::from(n);
    (0..n * n)
        .map(|i| {
            let (x, y) = (i64::from(i % n), i64::from(i / n));
            let s = u32::from(cells[i as usize]) % states;
            let next = (s + 1) % states;
            let mut count = 0;
            for dy in -1..=1i64 {
                for dx in -1..=1i64 {
                    if dx == 0 && dy == 0 {
                        continue;
                    }
                    let (px, py) = (x + dx, y + dy);
                    if !wrap && (px < 0 || py < 0 || px >= ni || py >= ni) {
                        continue;
                    }
                    let j = py.rem_euclid(ni) * ni + px.rem_euclid(ni);
                    if u32::from(cells[j as usize]) % states == next {
                        count += 1;
                    }
                }
            }
            (if count >= threshold { next } else { s }) as u8
        })
        .collect()
}

/// The topological defects of a `cyclic` field on a torus: every 2x2 plaquette
/// whose colour steps, taken the short way round the cycle, wind a full turn.
/// A spiral's core is one; noise is full of them. Meaningful for an odd
/// `states`, where the short way round is never a tie.
pub(super) fn defects(cells: &[u8], n: u32, states: u32) -> Vec<(u32, u32)> {
    let s = states as i32;
    let step = |a: u8, b: u8| -> i32 {
        let d = (i32::from(b) - i32::from(a)).rem_euclid(s);
        if d > s / 2 { d - s } else { d }
    };
    let at = |x: u32, y: u32| cells[((y % n) * n + (x % n)) as usize];
    let mut out = Vec::new();
    for y in 0..n {
        for x in 0..n {
            let (a, b, c, d) = (at(x, y), at(x + 1, y), at(x + 1, y + 1), at(x, y + 1));
            if step(a, b) + step(b, c) + step(c, d) + step(d, a) != 0 {
                out.push((x, y));
            }
        }
    }
    out
}

/// A field with its age channel: per cell, the state and the generations since
/// it last changed, saturating at [`AGE_CAP`](super::AGE_CAP).
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Aged {
    pub(super) state: Vec<u8>,
    pub(super) age: Vec<f32>,
}

impl Aged {
    /// A seeded field: live cells at age 0, dead ones at the cap — a seed has
    /// no history, so nothing starts with a wake.
    pub(super) fn seeded(state: Vec<u8>) -> Self {
        let age = state
            .iter()
            .map(|s| if *s == 1 { 0.0 } else { super::AGE_CAP })
            .collect();
        Self { state, age }
    }

    /// Advance to `next`: a cell that changed restarts at 0, one that did not
    /// counts on to the cap. `aged` is false for a disc, which is not a
    /// generation and leaves an unchanged cell's age where it was.
    pub(super) fn then(&self, next: Vec<u8>, aged: bool) -> Self {
        let age = self
            .state
            .iter()
            .zip(&next)
            .zip(&self.age)
            .map(|((was, now), age)| {
                if was != now {
                    0.0
                } else if aged {
                    (age + 1.0).min(super::AGE_CAP)
                } else {
                    *age
                }
            })
            .collect();
        Self { state: next, age }
    }
}

/// The field after one reseed disc: every cell within the disc — measured
/// across the seam on a torus — reseeded under the stamp's own seed.
pub(super) fn stamp(cells: &[u8], n: u32, wrap: bool, s: Stamp, threshold: u32) -> Vec<u8> {
    (0..n * n)
        .map(|i| {
            let (x, y) = (i % n, i / n);
            let mut dx = x.abs_diff(s.centre[0]);
            let mut dy = y.abs_diff(s.centre[1]);
            if wrap {
                dx = dx.min(n - dx);
                dy = dy.min(n - dy);
            }
            if dx * dx + dy * dy <= s.radius_sq {
                seeded(x, y, s.seed, threshold)
            } else {
                cells[i as usize]
            }
        })
        .collect()
}
