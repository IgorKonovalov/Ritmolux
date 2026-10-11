//! The voxel system's two stages: the step pass, a compute dispatch that seeds
//! or advances the 3D state one generation, and the march, a fullscreen pass
//! that walks each pixel's ray through the cube under emission-absorption, and
//! under `solid` stops at the first live cell and lights its face — and,
//! between them, the brick pass and the light field's splat and blur.

// Hot-path panic-denial pragma (Plan 0002 Phase 2; render/ is scanned by the
// hygiene guard). Only constants live here, compiled once at construction.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

/// The step pass. A `const MODE: u32` naming the pass (0 step, 1 seed, 2
/// stamp), `const
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
    // x: the seed ball's radius squared, in half-cells^2, y: the reseed ball's
    // seed; zw unused
    c: vec4<u32>,
    // xyz: the reseed ball's centre (cells), w: its radius squared (cells^2)
    d: vec4<u32>,
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

// The workgroup's tile: its 4^3 cells and a one-cell halo, 6^3 in all, each
// cell's live bit loaded from the texture once and read from here by every
// neighbour that counts it — 216 loads a workgroup where counting from the
// texture is 64 x 26.
const TILE: u32 = 6u;
var<workgroup> tile: array<u32, 216>;

// The tile index of local offset `l` from the tile's corner.
fn tile_at(l: vec3<u32>) -> u32 {
    return tile[(l.z * TILE + l.y) * TILE + l.x];
}

// The live cells among the 26 sharing a face, an edge or a corner with the
// cell at local position `lid`, read from the tile.
fn moore_count(lid: vec3<u32>) -> u32 {
    var count = 0u;
    for (var dz = 0u; dz < 3u; dz = dz + 1u) {
        for (var dy = 0u; dy < 3u; dy = dy + 1u) {
            for (var dx = 0u; dx < 3u; dx = dx + 1u) {
                count = count + tile_at(lid + vec3<u32>(dx, dy, dz));
            }
        }
    }
    return count - tile_at(lid + vec3<u32>(1u));
}

// The live cells among the 6 sharing a face with the cell at `lid`.
fn von_neumann_count(lid: vec3<u32>) -> u32 {
    let c = lid + vec3<u32>(1u);
    return tile_at(c + vec3<u32>(1u, 0u, 0u))
        + tile_at(c - vec3<u32>(1u, 0u, 0u))
        + tile_at(c + vec3<u32>(0u, 1u, 0u))
        + tile_at(c - vec3<u32>(0u, 1u, 0u))
        + tile_at(c + vec3<u32>(0u, 0u, 1u))
        + tile_at(c - vec3<u32>(0u, 0u, 1u));
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
fn main(
    @builtin(global_invocation_id) gid: vec3<u32>,
    @builtin(workgroup_id) wg: vec3<u32>,
    @builtin(local_invocation_id) lid: vec3<u32>,
    @builtin(local_invocation_index) li: u32,
) {
    let n = i32(params.a.x);
    let wrap = params.a.y != 0u;
    // Only a generation counts neighbours, so only it fills the tile. `MODE` is
    // a constant, so the barrier is in uniform control flow, and it comes
    // before any invocation past the grid returns.
    if (MODE == 0u) {
        let origin = vec3<i32>(wg * 4u) - vec3<i32>(1);
        for (var k = li; k < TILE * TILE * TILE; k = k + 64u) {
            let off = vec3<i32>(i32(k % TILE), i32((k / TILE) % TILE), i32(k / (TILE * TILE)));
            tile[k] = live_at(origin + off, n, wrap);
        }
        workgroupBarrier();
    }
    if (any(gid >= vec3<u32>(params.a.x))) {
        return;
    }
    let c = vec3<i32>(gid);
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
        case 2u: {
            // A ball of fresh seeded cells, measured across the seam on a torus
            // so a ball near a face wraps rather than being cut. A ball is not a
            // generation: a cell it leaves alone keeps its state and its age.
            var d = abs(c - vec3<i32>(params.d.xyz));
            if (wrap) {
                d = min(d, vec3<i32>(n) - d);
            }
            age = here >> 8u;
            if (u32(d.x * d.x + d.y * d.y + d.z * d.z) <= params.d.w) {
                let h = cell_hash3(c, params.c.y);
                next = select(0u, 1u, (h >> 8u) < params.a.w);
            }
            age = select(age, 0u, next != s);
        }
        default: {
            let states = max(params.b.z, 2u);
            let count = select(moore_count(lid), von_neumann_count(lid), params.b.w == 1u);
            if (s == 0u) {
                next = (params.b.x >> count) & 1u;
            } else if (s == 1u) {
                let stays = ((params.b.y >> count) & 1u) == 1u;
                next = select(select(0u, 2u, states > 2u), 1u, stays);
            } else {
                // A decay stage advances, and falls to dead past the last —
                // which is also where a stage left by a rule with more states
                // goes when the rule changes.
                next = select(s + 1u, 0u, s + 1u >= states);
            }
            age = select(age, 0u, next != s);
        }
    }
    textureStore(dst, c, vec4<u32>(pack(next, age), 0u, 0u, 0u));
}
"#;

/// The brick pass. `const BRICK: u32` is prepended at construction. One
/// workgroup a brick, each invocation scanning one column of it; the brick is
/// marked 1 when any of its cells is not dead. The grid's size is the
/// texture's, so a brick past a grid that is not a multiple of `BRICK` scans
/// only the cells that exist.
pub(super) const BRICK_SHADER: &str = r#"
@group(0) @binding(0) var cells: texture_3d<u32>;
@group(0) @binding(1) var bricks: texture_storage_3d<r32uint, write>;

var<workgroup> occupied: atomic<u32>;

@compute @workgroup_size(8, 8, 1)
fn main(
    @builtin(workgroup_id) wg: vec3<u32>,
    @builtin(local_invocation_id) lid: vec3<u32>,
    @builtin(local_invocation_index) li: u32,
) {
    if (li == 0u) {
        atomicStore(&occupied, 0u);
    }
    workgroupBarrier();
    let n = textureDimensions(cells).x;
    let base = wg * BRICK;
    var found = 0u;
    for (var z = 0u; z < BRICK; z = z + 1u) {
        let c = base + vec3<u32>(lid.xy, z);
        if (all(c < vec3<u32>(n))) {
            found = found | select(0u, 1u, (textureLoad(cells, vec3<i32>(c), 0).x & 0xFFu) != 0u);
        }
    }
    if (found != 0u) {
        atomicOr(&occupied, 1u);
    }
    workgroupBarrier();
    if (li == 0u) {
        textureStore(bricks, vec3<i32>(wg), vec4<u32>(atomicLoad(&occupied), 0u, 0u, 0u));
    }
}
"#;

/// The light field's splat (ADR-0272). `const LIGHT_CELL: u32` and `const
/// AGE_SPAN: f32` are prepended at construction. One invocation a texel, each
/// reducing the `LIGHT_CELL`³ block of cells it covers: `r` the live fraction,
/// `g` the live cells' ages (as the march colours a live cell) summed over the
/// same count, so `g / r` is their mean and a blur of both stays a weighted
/// mean. Decay stages emit nothing. A block past a grid that is not a multiple
/// of `LIGHT_CELL` counts only the cells that exist, over the whole block.
pub(super) const LIGHT_SPLAT_SHADER: &str = r#"
@group(0) @binding(0) var cells: texture_3d<u32>;
@group(0) @binding(1) var splat: texture_storage_3d<rgba16float, write>;

@compute @workgroup_size(4, 4, 4)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (any(gid >= textureDimensions(splat))) {
        return;
    }
    let n = textureDimensions(cells).x;
    let base = gid * LIGHT_CELL;
    var live = 0.0;
    var aged = 0.0;
    for (var z = 0u; z < LIGHT_CELL; z = z + 1u) {
        for (var y = 0u; y < LIGHT_CELL; y = y + 1u) {
            for (var x = 0u; x < LIGHT_CELL; x = x + 1u) {
                let c = base + vec3<u32>(x, y, z);
                if (all(c < vec3<u32>(n))) {
                    let texel = textureLoad(cells, vec3<i32>(c), 0).x;
                    if ((texel & 0xFFu) == 1u) {
                        live = live + 1.0;
                        aged = aged + min(f32(texel >> 8u) / AGE_SPAN, 1.0);
                    }
                }
            }
        }
    }
    let k = 1.0 / f32(LIGHT_CELL * LIGHT_CELL * LIGHT_CELL);
    textureStore(splat, vec3<i32>(gid), vec4<f32>(live * k, aged * k, 0.0, 0.0));
}
"#;

/// The light field's blur: each texel the 3³ texels about it, weighted by a
/// 1-2-1 tent on each axis (64 in all). A texel past the field's faces is
/// empty, so the light fades toward the cube's faces rather than piling up.
pub(super) const LIGHT_BLUR_SHADER: &str = r#"
@group(0) @binding(0) var src: texture_3d<f32>;
@group(0) @binding(1) var dst: texture_storage_3d<rgba16float, write>;

fn tent(d: i32) -> f32 {
    return select(1.0, 2.0, d == 0);
}

@compute @workgroup_size(4, 4, 4)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let side = vec3<i32>(textureDimensions(src));
    let at = vec3<i32>(gid);
    if (any(at >= side)) {
        return;
    }
    var sum = vec2<f32>(0.0);
    for (var dz = -1; dz <= 1; dz = dz + 1) {
        for (var dy = -1; dy <= 1; dy = dy + 1) {
            for (var dx = -1; dx <= 1; dx = dx + 1) {
                let c = at + vec3<i32>(dx, dy, dz);
                if (all(c >= vec3<i32>(0)) && all(c < side)) {
                    let w = tent(dx) * tent(dy) * tent(dz);
                    sum = sum + w * textureLoad(src, c, 0).xy;
                }
            }
        }
    }
    textureStore(dst, at, vec4<f32>(sum / 64.0, 0.0, 0.0));
}
"#;

/// The present: the march's target stretched over the render target,
/// filtered. [`gpu::FULLSCREEN_VS_UV_FLIPPED`](crate::render::gpu::FULLSCREEN_VS_UV_FLIPPED)
/// is prepended at construction.
pub(super) const PRESENT_SHADER: &str = r#"
@group(0) @binding(0) var samp: sampler;
@group(0) @binding(1) var marched: texture_2d<f32>;

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    return textureSampleLevel(marched, samp, in.uv, 0.0);
}
"#;

/// The march. [`gpu::FULLSCREEN_VS_NDC`](crate::render::gpu::FULLSCREEN_VS_NDC),
/// `const MAX_MARCH: i32` — the most cells a ray through the largest grid
/// crosses — `const AGE_SPAN: f32` and `const BRICK: i32` are prepended at
/// construction.
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
/// in world units, so the cube is 2 across whatever the grid, and the walk
/// stops once `T` is under one 8-bit step. The output is premultiplied,
/// `(C, (1 - T) * occlude)` (ADR-0201): at `density = 0`, `T` stays 1 and the
/// volume adds to the backdrop without covering it.
///
/// Under `solid` (`light.w`), the first live cell ends the walk: its entry
/// face — the axis of the last crossing, facing against the step — is shaded
/// `albedo * (ambient * occ + key * max(0, n . l))`, added under the light in
/// front of it, and `T` drops to 0, so the pixel's coverage is `occlude`.
/// `occ` is the face's voxel ambient occlusion at the hit point, mixed in by
/// `ao`; at `ao = 0` it is not evaluated. With `bounce.x > 0` the face adds
/// the light field sampled half a cell in front of the hit point, coloured at
/// the field's mean age; at 0 the field is not fetched.
///
/// A cell the section plane hides (`cut_away`) is dead to the walk and to
/// `solid_at`, so the occlusion reads it as open. The brick jump does not know
/// the plane: a brick is jumped only on its own empty flag, and a cut brick is
/// walked cell by cell.
///
/// **A crossing is computed from its boundary's index, never accumulated.** The
/// parameter at which the ray crosses an axis's next boundary is
/// `(boundary - o) * inv_dg`, evaluated afresh each step. That is what lets the walk
/// jump a whole empty brick: the exit face's crossing, and every crossing on
/// the other two axes before it, are the same expressions stepping would have
/// evaluated, compared in the step's own order (the earlier parameter first, a
/// tie to x, then y, then z), so the jump lands on exactly the cell and the
/// parameter stepping reaches, and moves no pixel.
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
    // x: age_tint, y: hue_spread, z: fog (0..1), w: shell_gain (>= 0)
    c: vec4<f32>,
    // x: the cube's nearest view depth, y: its depth span, z: shells (0 for
    // none), w: 1 to jump empty bricks
    d: vec4<f32>,
    // xyz: the unit world direction toward the key light, w: 1 under solid
    light: vec4<f32>,
    // x: key, y: ambient, z: ao (0..1); w: 1 when the section plane is on
    lit: vec4<f32>,
    // The section plane: xyz its unit normal toward the hidden side, in the
    // cube's frame; w its offset. Read only while lit.w is 1
    cut: vec4<f32>,
    // x: bounce, 0 to skip the light field; yzw unused
    bounce: vec4<f32>,
    // The shells' levels, four to a vec4, bass first
    shells: array<vec4<f32>, 4>,
}
@group(0) @binding(0) var<uniform> mp: March;
@group(0) @binding(1) var field: texture_3d<u32>;
@group(0) @binding(2) var bricks: texture_3d<u32>;
@group(0) @binding(3) var lut_a: texture_2d<f32>;
@group(0) @binding(4) var lut_b: texture_2d<f32>;
@group(0) @binding(5) var lut_samp: sampler;
// r: intensity, g: intensity-weighted age; a one-texel stand-in while
// bounce.x is 0
@group(0) @binding(6) var light_field: texture_3d<f32>;
@group(0) @binding(7) var light_samp: sampler;

// Under this transmittance nothing further along the ray reaches one 8-bit
// step of the frame.
const OPAQUE: f32 = 1.0 / 255.0;

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

// True when the section plane hides the cell at `c`: its centre, in the cube's
// [-1, 1]^3 frame, lies beyond the plane. Always false while the plane is off.
fn cut_away(c: vec3<i32>) -> bool {
    let centre = (vec3<f32>(c) + vec3<f32>(0.5)) / mp.eye.w * 2.0 - vec3<f32>(1.0);
    return mp.lit.w > 0.5 && dot(centre, mp.cut.xyz) > mp.cut.w;
}

// 1 when the cell at `c` is live, 0 for any other state, off the grid, and
// for a cell the section plane hides.
fn solid_at(c: vec3<i32>, ni: i32) -> f32 {
    if (any(c < vec3<i32>(0)) || any(c >= vec3<i32>(ni)) || cut_away(c)) {
        return 0.0;
    }
    return select(0.0, 1.0, (textureLoad(field, c, 0).x & 0xFFu) == 1u);
}

// How closed one corner of a face is, 0 open to 1 shut, from the two cells
// beside it and the one diagonal to it in the layer in front of the face: two
// sides shut it whatever the diagonal, otherwise each closes a third.
fn corner_closed(side_a: f32, side_b: f32, diagonal: f32) -> f32 {
    if (side_a > 0.5 && side_b > 0.5) {
        return 1.0;
    }
    return (side_a + side_b + diagonal) / 3.0;
}

// The unit vector along `axis` (0, 1 or 2). This shader reaches a vector's
// component by `select`, never by a runtime index: FXC, the DX12 compiler,
// unrolls a loop whose body indexes a vector at run time, and a loop it
// cannot unroll fails the pipeline.
fn axis_unit(axis: i32) -> vec3<i32> {
    return vec3<i32>(vec3<bool>(axis == 0, axis == 1, axis == 2));
}

// `v[axis]`, without the runtime index (`axis_unit`).
fn pick_i(v: vec3<i32>, axis: i32) -> i32 {
    return select(select(v.z, v.y, axis == 1), v.x, axis == 0);
}

// `v[axis]`, without the runtime index (`axis_unit`).
fn pick_f(v: vec3<f32>, axis: i32) -> f32 {
    return select(select(v.z, v.y, axis == 1), v.x, axis == 0);
}

// One axis of a brick jump: the cell index `c` stepped by `stride` past every
// boundary the ray crosses before the exit parameter `t_e`, at most `BRICK`
// of them. A boundary exactly at `t_e` is crossed only by an axis ordered
// before the exit axis (`before_exit`), which is the step's own tie order.
fn skip_axis(c: i32, ahead: i32, o: f32, inv_dg: f32, stride: i32, t_e: f32, before_exit: bool) -> i32 {
    var at = c;
    for (var k = 0; k < BRICK; k = k + 1) {
        let t_b = (f32(at + ahead) - o) * inv_dg;
        if (!(t_b < t_e || (t_b == t_e && before_exit))) {
            break;
        }
        at = at + stride;
    }
    return at;
}

// The open fraction of the ambient light reaching point `p` (cell units) on
// the face of `cell` whose outward normal is `facing` (+1 or -1) along `axis`:
// the four corners' closure, interpolated bilinearly across the face.
fn face_open(cell: vec3<i32>, axis: i32, facing: i32, p: vec3<f32>, ni: i32) -> f32 {
    let front = cell + axis_unit(axis) * facing;
    let ua = (axis + 1) % 3;
    let va = (axis + 2) % 3;
    let eu = axis_unit(ua);
    let ev = axis_unit(va);
    let u_lo = solid_at(front - eu, ni);
    let u_hi = solid_at(front + eu, ni);
    let v_lo = solid_at(front - ev, ni);
    let v_hi = solid_at(front + ev, ni);
    let c00 = corner_closed(u_lo, v_lo, solid_at(front - eu - ev, ni));
    let c10 = corner_closed(u_hi, v_lo, solid_at(front + eu - ev, ni));
    let c01 = corner_closed(u_lo, v_hi, solid_at(front - eu + ev, ni));
    let c11 = corner_closed(u_hi, v_hi, solid_at(front + eu + ev, ni));
    let fu = clamp(pick_f(p, ua) - f32(pick_i(cell, ua)), 0.0, 1.0);
    let fv = clamp(pick_f(p, va) - f32(pick_i(cell, va)), 0.0, 1.0);
    return 1.0 - mix(mix(c00, c10, fu), mix(c01, c11, fu), fv);
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
    // The axis of the face the ray last crossed into a cell through: first
    // the cube's own entry face, then each step's. A solid hit's normal.
    var axis = 2;
    if (t_near.x >= t_near.y && t_near.x >= t_near.z) {
        axis = 0;
    } else if (t_near.y >= t_near.z) {
        axis = 1;
    }
    let solid = mp.light.w > 0.5;

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
    // Every crossing below multiplies by this one reciprocal, so the step and
    // the brick jump evaluate the same expression.
    let inv_dg = vec3<f32>(1.0) / dg;
    let forward_axis = dg >= vec3<f32>(0.0);
    let stride = select(vec3<i32>(-1), vec3<i32>(1), forward_axis);
    // The boundary a cell's crossing on each axis is at: its far side going
    // up an axis, its near side going down.
    let ahead = select(vec3<i32>(0), vec3<i32>(1), forward_axis);
    let skip = mp.d.w > 0.5;
    var brick = vec3<i32>(-1);
    var brick_full = true;

    let forward = mp.r3.xyz;
    var light = vec3<f32>(0.0);
    var transmit = 1.0;
    for (var i = 0; i < MAX_MARCH; i = i + 1) {
        let t_cross = (vec3<f32>(cell + ahead) - o) * inv_dg;

        if (skip) {
            let here = cell / BRICK;
            if (any(here != brick)) {
                brick = here;
                brick_full = textureLoad(bricks, here, 0).x != 0u;
            }
            if (!brick_full) {
                // Every cell of this brick is dead: jump to the first face the
                // ray leaves it through.
                let lo = here * BRICK;
                let face = select(lo, min(lo + vec3<i32>(BRICK), vec3<i32>(ni)), forward_axis);
                let t_face = (vec3<f32>(face) - o) * inv_dg;
                var e = 2;
                if (t_face.x <= t_face.y && t_face.x <= t_face.z) {
                    e = 0;
                } else if (t_face.y <= t_face.z) {
                    e = 1;
                }
                // `t_face[e]`: `e` is the first axis holding the minimum.
                let t_e = min(min(t_face.x, t_face.y), t_face.z);
                if (t_e >= t_exit) {
                    break;
                }
                // The other two axes step past every boundary they cross before
                // the exit, in the step's own order; the exit axis crosses its
                // face.
                let passed = vec3<i32>(
                    skip_axis(cell.x, ahead.x, o.x, inv_dg.x, stride.x, t_e, 0 < e),
                    skip_axis(cell.y, ahead.y, o.y, inv_dg.y, stride.y, t_e, 1 < e),
                    skip_axis(cell.z, ahead.z, o.z, inv_dg.z, stride.z, t_e, 2 < e)
                );
                let crossed = select(face - vec3<i32>(1), face, forward_axis);
                cell = select(passed, crossed, axis_unit(e) != vec3<i32>(0));
                t = t_e;
                axis = e;
                if (any(cell < vec3<i32>(0)) || any(cell >= vec3<i32>(ni))) {
                    break;
                }
                continue;
            }
        }

        let t_next = min(min(t_cross.x, t_cross.y), t_cross.z);
        let t_end = min(t_next, t_exit);
        let len = max(t_end - t, 0.0);

        let texel = textureLoad(field, cell, 0).x;
        // A cell the section plane hides reads as dead, under either present.
        let state = select(texel & 0xFFu, 0u, cut_away(cell));
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
            // The shell this cell lies in lights it by its band's level: bass
            // at the centre, treble at the faces, a corner in the last shell.
            // `1 + gain * level` is exactly 1 at gain 0.
            var shell = 1.0;
            if (mp.d.z > 0.5) {
                let i = min(u32(min(length(centre), 1.0) * mp.d.z), u32(mp.d.z) - 1u);
                let row = mp.shells[i / 4u];
                let k = i % 4u;
                let level = select(select(select(row.w, row.z, k == 2u), row.y, k == 1u), row.x, k == 0u);
                shell = 1.0 + mp.c.w * level;
            }
            if (solid && state == 1u) {
                // The first live cell ends the ray: its entry face, lit, under
                // the light gathered in front of it, covering the pixel.
                let facing = -pick_i(stride, axis);
                let normal = vec3<f32>(axis_unit(axis) * facing);
                let lambert = max(dot(normal, mp.light.xyz), 0.0);
                // At `ao = 0` the ambient term is left exactly as it is.
                var ambient = mp.lit.y;
                if (mp.lit.z > 0.0) {
                    let open = face_open(cell, axis, facing, o + dg * t, ni);
                    ambient = ambient * mix(1.0, open, mp.lit.z);
                }
                let lighting = ambient + mp.lit.x * lambert;
                light = light + transmit * shade(coord) * (max(mp.a.y, 0.0) * fog * shell * lighting);
                if (mp.bounce.x > 0.0) {
                    // The neighbours' light half a cell in front of the face,
                    // in field texels of LIGHT_CELL cells each.
                    let at = o + dg * t + normal * 0.5;
                    let uvw = at / (LIGHT_CELL * vec3<f32>(textureDimensions(light_field)));
                    let lf = textureSampleLevel(light_field, light_samp, uvw, 0.0).xy;
                    let lit_age = select(0.0, lf.y / lf.x, lf.x > 1e-6);
                    let lit_coord = mp.a.x + mp.c.y * (radius - 0.5) + mp.c.x * lit_age;
                    light = light + transmit * shade(lit_coord) * (mp.bounce.x * lf.x * max(mp.a.y, 0.0) * fog * shell);
                }
                transmit = 0.0;
                break;
            }
            light = light + transmit * shade(coord) * (max(mp.a.y, 0.0) * glow * EMISSION * fog * shell * len);
            transmit = transmit * exp(-mp.b.z * glow * len);
            if (transmit < OPAQUE) {
                break;
            }
        }

        t = t_end;
        if (t_next >= t_exit) {
            break;
        }
        if (t_cross.x <= t_cross.y && t_cross.x <= t_cross.z) {
            cell.x = cell.x + stride.x;
            axis = 0;
        } else if (t_cross.y <= t_cross.z) {
            cell.y = cell.y + stride.y;
            axis = 1;
        } else {
            cell.z = cell.z + stride.z;
            axis = 2;
        }
        if (any(cell < vec3<i32>(0)) || any(cell >= vec3<i32>(ni))) {
            break;
        }
    }
    return vec4<f32>(light, (1.0 - transmit) * mp.b.y);
}
"#;
