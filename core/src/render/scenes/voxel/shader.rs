//! The voxel system's two stages: the step pass, a compute dispatch that seeds
//! or advances the 3D state one generation, and the march, a fullscreen pass
//! that walks each pixel's ray through the cube under emission-absorption.

// Hot-path panic-denial pragma (Plan 0002 Phase 2; render/ is scanned by the
// hygiene guard). Only constants live here, compiled once at construction.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

/// The step pass. A `const MODE: u32` naming the pass (0 step, 1 seed), `const
/// AGE_CAP: u32`, [`gpu::HASH_WGSL`](crate::render::gpu::HASH_WGSL) and the
/// shared [`CELL_HASH_WGSL`](crate::render::scenes::common::CELL_HASH_WGSL) are
/// prepended at construction.
///
/// A texel packs a cell as `state | age << 8`: the state in the low byte — 0
/// dead, 1 live, 2 and up the decay stages — and above it the generations since
/// the cell last changed state, saturating at `AGE_CAP`. Everything here is
/// integer arithmetic, so a generation is the same on every adapter.
pub(super) const STEP_SHADER: &str = r#"
struct Step {
    // x: grid (cells per side), y: wrap (1 torus), z: the field's seed,
    // w: live threshold against the hash's top 24 bits
    a: vec4<u32>,
    // x: birth mask, y: survive mask (bit k: k live neighbours),
    // z: states (>= 2), w: neighbourhood (0 Moore, 1 von Neumann)
    b: vec4<u32>,
    // x: the seed ball's radius squared, in half-cells^2; yzw unused
    c: vec4<u32>,
}
@group(0) @binding(0) var src: texture_3d<u32>;
@group(0) @binding(1) var dst: texture_storage_3d<r32uint, write>;
@group(0) @binding(2) var<uniform> params: Step;

const STATE_MASK: u32 = 0xFFu;

fn pack(state: u32, age: u32) -> u32 {
    return state | (min(age, AGE_CAP) << 8u);
}

// 1 when the cell at `c` is live. Off the grid it is the wrapped cell on a
// torus and dead otherwise. Always loads an in-range texel and selects, so the
// function has one exit and no branch.
fn live_at(c: vec3<i32>, n: i32, wrap: bool) -> u32 {
    let size = vec3<i32>(n);
    let w = ((c % size) + size) % size;
    let inside = all(c >= vec3<i32>(0)) && all(c < size);
    let s = textureLoad(src, w, 0).x & STATE_MASK;
    return select(0u, 1u, s == 1u && (wrap || inside));
}

// The live cells among the 26 sharing a face, an edge or a corner with `c`.
fn moore_count(c: vec3<i32>, n: i32, wrap: bool) -> u32 {
    var count = 0u;
    for (var dz = -1; dz <= 1; dz = dz + 1) {
        for (var dy = -1; dy <= 1; dy = dy + 1) {
            for (var dx = -1; dx <= 1; dx = dx + 1) {
                if (dx != 0 || dy != 0 || dz != 0) {
                    count = count + live_at(c + vec3<i32>(dx, dy, dz), n, wrap);
                }
            }
        }
    }
    return count;
}

// The live cells among the 6 sharing a face with `c`.
fn von_neumann_count(c: vec3<i32>, n: i32, wrap: bool) -> u32 {
    return live_at(c + vec3<i32>(1, 0, 0), n, wrap)
        + live_at(c - vec3<i32>(1, 0, 0), n, wrap)
        + live_at(c + vec3<i32>(0, 1, 0), n, wrap)
        + live_at(c - vec3<i32>(0, 1, 0), n, wrap)
        + live_at(c + vec3<i32>(0, 0, 1), n, wrap)
        + live_at(c - vec3<i32>(0, 0, 1), n, wrap);
}

// A seeded cell: live inside the ball about the grid's centre when its hash
// falls below the fill threshold. The ball is measured in half-cells from the
// centre, `2c + 1 - n` an axis, so it is symmetric on an even grid.
fn seeded(c: vec3<i32>, n: i32) -> u32 {
    let d = 2 * c + vec3<i32>(1 - n);
    let r2 = u32(d.x * d.x + d.y * d.y + d.z * d.z);
    let h = cell_hash3(c, params.a.z);
    return select(0u, 1u, r2 <= params.c.x && (h >> 8u) < params.a.w);
}

@compute @workgroup_size(4, 4, 4)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let n = i32(params.a.x);
    if (any(gid >= vec3<u32>(params.a.x))) {
        return;
    }
    let c = vec3<i32>(gid);
    let wrap = params.a.y != 0u;
    let here = textureLoad(src, c, 0).x;
    let s = here & STATE_MASK;
    var age = (here >> 8u) + 1u;
    var next = s;
    switch MODE {
        case 1u: {
            // A seed has no history: its dead cells start long ago and its live
            // ones now.
            next = seeded(c, n);
            age = select(AGE_CAP, 0u, next == 1u);
        }
        default: {
            let states = max(params.b.z, 2u);
            let count = select(
                moore_count(c, n, wrap),
                von_neumann_count(c, n, wrap),
                params.b.w == 1u
            );
            if (s == 0u) {
                next = (params.b.x >> count) & 1u;
            } else if (s == 1u) {
                let stays = ((params.b.y >> count) & 1u) == 1u;
                next = select(select(0u, 2u, states > 2u), 1u, stays);
            } else {
                // A decay stage advances, and falls to dead past the last.
                next = select(s + 1u, 0u, s + 1u >= states);
            }
            age = select(age, 0u, next != s);
        }
    }
    textureStore(dst, c, vec4<u32>(pack(next, age), 0u, 0u, 0u));
}
"#;

/// The march. [`gpu::FULLSCREEN_VS_NDC`](crate::render::gpu::FULLSCREEN_VS_NDC),
/// `const MAX_MARCH: i32` — the most cells a ray through the largest grid
/// crosses — and `const AGE_SPAN: f32` are prepended at construction.
///
/// Each pixel's ray leaves the eye through the camera's view of that pixel and
/// is walked through the cube `[-1, 1]^3` one cell at a time by an exact grid
/// traversal (Amanatides-Woo): every cell it crosses is visited once, with the
/// length of the ray inside it. Light accumulates front to back:
///
/// ```text
/// C += T * emission * len
/// T *= exp(-density * glow * len)
/// ```
///
/// in world units, so the cube is 2 across whatever the grid. The output is
/// premultiplied, `(C, (1 - T) * occlude)` (ADR-0201): at `density = 0`, `T`
/// stays 1 and the volume adds to the backdrop without covering it.
pub(super) const MARCH_SHADER: &str = r#"
struct March {
    // xyz: the world-to-clip matrix's screen-x row, w: its translation
    r0: vec4<f32>,
    // the screen-y row, likewise
    r1: vec4<f32>,
    // the view-depth row: xyz the unit view axis, w its translation
    r3: vec4<f32>,
    // xyz: the eye, in world units; w: grid (cells per side)
    eye: vec4<f32>,
    // x: hue, y: brightness, z: saturation, w: palette_mix
    a: vec4<f32>,
    // x: palette_steps (integral, quantized CPU-side), y: occlude (ADR-0085),
    // z: density (per world unit, >= 0), w: trail (0..1)
    b: vec4<f32>,
    // x: age_tint, y: hue_spread, z: fog (0..1), w: unused
    c: vec4<f32>,
    // x: the cube's nearest view depth, y: its depth span; zw unused
    d: vec4<f32>,
}
@group(0) @binding(0) var<uniform> mp: March;
@group(0) @binding(1) var field: texture_3d<u32>;
@group(0) @binding(2) var lut_a: texture_2d<f32>;
@group(0) @binding(3) var lut_b: texture_2d<f32>;
@group(0) @binding(4) var lut_samp: sampler;

// The light a cell emits per world unit before brightness, at full glow.
const EMISSION: f32 = 2.0;

// Shared `saturation` (mirrors core/src/render/palette.rs::desaturate verbatim).
fn apply_saturation(c: vec3<f32>, s: f32) -> vec3<f32> {
    let luma = dot(c, vec3<f32>(0.299, 0.587, 0.114));
    return vec3<f32>(luma) + (c - vec3<f32>(luma)) * s;
}

// Shared `palette_steps` (mirrors core/src/render/palette.rs::band_coord
// verbatim, ADR-0078).
fn band_coord(t: f32, steps: f32) -> f32 {
    if (steps < 1.5) {
        return t;
    }
    return (floor(t * steps) + 0.5) / steps;
}

// The palette colour at `coord`: banded, crossfaded A to B, and saturated.
fn shade(coord: f32) -> vec3<f32> {
    let banded = band_coord(coord, mp.b.x);
    let ca = textureSampleLevel(lut_a, lut_samp, vec2<f32>(banded, 0.5), 0.0).rgb;
    let cb = textureSampleLevel(lut_b, lut_samp, vec2<f32>(banded, 0.5), 0.0).rgb;
    return apply_saturation(mix(ca, cb, clamp(mp.a.w, 0.0, 1.0)), mp.a.z);
}

// How much of a live cell's light a cell in `state` gives: 1 live, `trail` to
// the power of the stage for a decay stage, 0 dead. The same weight absorbs,
// so a stage that has faded from sight no longer occludes either.
fn glow_of(state: u32) -> f32 {
    if (state == 1u) {
        return 1.0;
    }
    if (state >= 2u) {
        return exp2(f32(state - 1u) * log2(max(mp.b.w, 1e-6)));
    }
    return 0.0;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    // The ray through this pixel: the direction both screen rows map to this
    // pixel's NDC, i.e. the cross product of the two planes they define,
    // pointing along the view axis.
    let pa = mp.r0.xyz - in.ndc.x * mp.r3.xyz;
    let pb = mp.r1.xyz - in.ndc.y * mp.r3.xyz;
    var dir = normalize(cross(pa, pb));
    if (dot(dir, mp.r3.xyz) < 0.0) {
        dir = -dir;
    }
    let eye = mp.eye.xyz;

    // The slab test against [-1, 1]^3. A component near zero is nudged off it,
    // so the reciprocal is large and finite rather than a division by zero.
    let tiny = abs(dir) < vec3<f32>(1e-6);
    let d = select(dir, select(vec3<f32>(-1e-6), vec3<f32>(1e-6), dir >= vec3<f32>(0.0)), tiny);
    let t_lo = (vec3<f32>(-1.0) - eye) / d;
    let t_hi = (vec3<f32>(1.0) - eye) / d;
    let t_near = min(t_lo, t_hi);
    let t_far = max(t_lo, t_hi);
    let t_exit = min(min(t_far.x, t_far.y), t_far.z);
    var t = max(max(max(t_near.x, t_near.y), t_near.z), 0.0);
    if (t_exit <= t) {
        return vec4<f32>(0.0);
    }

    // The same ray in cell units: the cube's corner at 0, its far corner at n.
    let n = mp.eye.w;
    let o = (eye + vec3<f32>(1.0)) * (0.5 * n);
    let dg = d * (0.5 * n);
    let ni = i32(n);
    var cell = clamp(
        vec3<i32>(floor(o + dg * (t + 1e-5))),
        vec3<i32>(0),
        vec3<i32>(ni - 1)
    );
    let stride = select(vec3<i32>(-1), vec3<i32>(1), dg >= vec3<f32>(0.0));
    let ahead = select(vec3<f32>(0.0), vec3<f32>(1.0), dg >= vec3<f32>(0.0));
    var t_cross = (vec3<f32>(cell) + ahead - o) / dg;
    let t_step = abs(vec3<f32>(1.0) / dg);

    let forward = mp.r3.xyz;
    var light = vec3<f32>(0.0);
    var transmit = 1.0;
    for (var i = 0; i < MAX_MARCH; i = i + 1) {
        let t_next = min(min(t_cross.x, t_cross.y), t_cross.z);
        let t_end = min(t_next, t_exit);
        let len = max(t_end - t, 0.0);

        let texel = textureLoad(field, cell, 0).x;
        let state = texel & 0xFFu;
        let glow = glow_of(state);
        if (glow > 0.0) {
            let centre = (vec3<f32>(cell) + vec3<f32>(0.5)) / n * 2.0 - vec3<f32>(1.0);
            let radius = length(centre) / sqrt(3.0);
            let age = select(1.0, min(f32(texel >> 8u) / AGE_SPAN, 1.0), state == 1u);
            let coord = mp.a.x + mp.c.y * (radius - 0.5) + mp.c.x * age;
            // Fog fades a cell by where it lies in the cube's depth (ADR-0263),
            // and is exactly 1 at fog 0.
            let depth = 0.5 * (t + t_end) * dot(dir, forward);
            let depth01 = select(0.0, clamp((depth - mp.d.x) / mp.d.y, 0.0, 1.0), mp.d.y > 0.0);
            let fog = clamp(1.0 - mp.c.z * depth01, 0.0, 1.0);
            light = light + transmit * shade(coord) * (max(mp.a.y, 0.0) * glow * EMISSION * fog * len);
            transmit = transmit * exp(-mp.b.z * glow * len);
        }

        t = t_end;
        if (t_next >= t_exit) {
            break;
        }
        if (t_cross.x <= t_cross.y && t_cross.x <= t_cross.z) {
            cell.x = cell.x + stride.x;
            t_cross.x = t_cross.x + t_step.x;
        } else if (t_cross.y <= t_cross.z) {
            cell.y = cell.y + stride.y;
            t_cross.y = t_cross.y + t_step.y;
        } else {
            cell.z = cell.z + stride.z;
            t_cross.z = t_cross.z + t_step.z;
        }
        if (any(cell < vec3<i32>(0)) || any(cell >= vec3<i32>(ni))) {
            break;
        }
    }
    return vec4<f32>(light, (1.0 - transmit) * mp.b.y);
}
"#;
