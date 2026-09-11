//! The cellular system's fragment stages: the step pass, which seeds, stamps
//! and advances the field one generation per draw; the row pass, the first half
//! of a `larger_than_life` neighbourhood sum; and the present pass, which paints
//! the field through the palette.

// Hot-path panic-denial pragma (Plan 0002 Phase 2; render/ is scanned by the
// hygiene guard). Only constants live here, compiled once at construction.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

/// What the step and row passes share: the uniform's shape and the helpers that
/// read the field. Both passes declare their own bindings named `field` and
/// `params`, which WGSL resolves wherever they are declared in the module.
///
/// A `const MAX_RADIUS: i32` is prepended with it at construction.
pub(super) const STEP_COMMON: &str = r#"
struct Step {
    // x: family (0 life_like, 1 larger_than_life, 2 cyclic), y: wrap (1 torus),
    // z: grid (cells per side), w: live threshold against the hash's top 24 bits
    a: vec4<u32>,
    // x: birth mask, y: survive mask (bit k: k live neighbours),
    // z: radius (cells, 1..=MAX_RADIUS, capped CPU-side),
    // w: cyclic's states (>= 2, clamped CPU-side)
    b: vec4<u32>,
    // x: the field's seed, y: the stamp's seed, z: stamp radius squared
    // (cells^2), w: unused
    c: vec4<u32>,
    // xy: stamp centre (cells), z: cyclic's threshold (1..=8), w: unused
    d: vec4<u32>,
    // larger_than_life's intervals as whole neighbour counts, inclusive:
    // x..y births a dead cell, z..w keeps a live one
    e: vec4<u32>,
}

// 1 when the cell at `c` is live. Off the grid it is the wrapped cell on a
// torus and dead otherwise. Always loads an in-range texel and selects, so the
// function has one exit and no branch.
fn live(c: vec2<i32>, n: i32, wrap: bool) -> u32 {
    let size = vec2<i32>(n);
    let w = ((c % size) + size) % size;
    let inside = all(c >= vec2<i32>(0)) && all(c < size);
    let v = textureLoad(field, w, 0).x;
    return select(0u, 1u, v > 0.5 && (wrap || inside));
}
"#;

/// The row pass of `larger_than_life`: each cell's live count along its own
/// row, `radius` either side and itself included, written into the row
/// texture the step pass sums down its column. [`STEP_COMMON`] and the vertex
/// prelude are prepended.
///
/// **This is the separation**: a box of side `2r + 1` costs `2r + 1` reads
/// here and `2r + 1` there, where summing it in one pass costs `(2r + 1)^2` —
/// 42 reads a cell against 441 at radius 10. The loop runs to the constant
/// `MAX_RADIUS` and skips the steps past `radius`, since a trip count read from
/// a uniform is what lost the device on WARP for the analytic field.
pub(super) const ROWS_SHADER: &str = r#"
@group(0) @binding(0) var field: texture_2d<f32>;
@group(0) @binding(1) var<uniform> params: Step;

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let n = i32(params.a.z);
    let wrap = params.a.y != 0u;
    let r = i32(params.b.z);
    let c = vec2<i32>(in.pos.xy);
    var sum = 0u;
    for (var dx = -MAX_RADIUS; dx <= MAX_RADIUS; dx = dx + 1) {
        if (abs(dx) <= r) {
            sum = sum + live(c + vec2<i32>(dx, 0), n, wrap);
        }
    }
    return vec4<f32>(f32(sum), 0.0, 0.0, 1.0);
}
"#;

/// The step pass. [`gpu::FULLSCREEN_VS_UV_FLIPPED`](crate::render::gpu::FULLSCREEN_VS_UV_FLIPPED),
/// a `const MODE: u32` naming the pass (0 step, 1 seed, 2 stamp), `const
/// AGE_CAP: f32`, `const MAX_RADIUS: i32`, [`gpu::HASH_WGSL`](crate::render::gpu::HASH_WGSL)
/// and [`STEP_COMMON`] are prepended at construction. The render target is the
/// grid itself, so a fragment's position is its cell.
///
/// Everything here is integer arithmetic over texel values that are whole
/// numbers a half float holds exactly — a state of 0 or 1 or a colour index
/// below `MAX_STATES`, an age up to `AGE_CAP`, a row count up to
/// `2 * MAX_RADIUS + 1` — so a generation is the same on every adapter.
pub(super) const STEP_SHADER: &str = r#"
@group(0) @binding(0) var field: texture_2d<f32>;
// The row pass's counts; read by `larger_than_life` only, bound for every
// family so the three passes share one layout.
@group(0) @binding(1) var rows: texture_2d<f32>;
@group(0) @binding(2) var<uniform> params: Step;

// The field's uniform is shared by all three passes; which pass this is lives
// in `MODE` alone. See `StepParams` for why that split is load-bearing.

// A seeded cell, from the hash of its coordinates under `seed`: live when the
// hash falls below the threshold, or — for `cyclic` — one of its `states`
// colours, uniformly. Every input is a u32, so the field is the same bit for
// bit on every adapter.
fn seeded(c: vec2<i32>, seed: u32) -> f32 {
    let h = mix32(u32(c.x) ^ mix32(u32(c.y) ^ mix32(seed)));
    let binary = select(0.0, 1.0, (h >> 8u) < params.a.w);
    let colour = f32((h >> 8u) % max(params.b.w, 1u));
    return select(binary, colour, params.a.x == 2u);
}

// The state of the cell at `c` as a colour index in `0..states`, and whether
// it counts: off the grid it is the wrapped cell on a torus and counts for
// nothing otherwise. A state left over from a larger `states` is read modulo
// the current one, so a held change of `states` never strands a cell outside
// the cycle.
fn colour_at(c: vec2<i32>, n: i32, wrap: bool, states: u32) -> vec2<u32> {
    let size = vec2<i32>(n);
    let w = ((c % size) + size) % size;
    let inside = all(c >= vec2<i32>(0)) && all(c < size);
    let s = u32(textureLoad(field, w, 0).x + 0.5) % states;
    return vec2<u32>(s, select(0u, 1u, wrap || inside));
}

// How many of `c`'s eight neighbours hold the colour `next`.
fn cyclic_count(c: vec2<i32>, n: i32, wrap: bool, states: u32, next: u32) -> u32 {
    var count = 0u;
    for (var dy = -1; dy <= 1; dy = dy + 1) {
        for (var dx = -1; dx <= 1; dx = dx + 1) {
            if (dx != 0 || dy != 0) {
                let v = colour_at(c + vec2<i32>(dx, dy), n, wrap, states);
                count = count + select(0u, 1u, v.y == 1u && v.x == next);
            }
        }
    }
    return count;
}

// The live cells in the box of side `2r + 1` about `c`, `c` itself excluded:
// the row pass's counts summed down the column. A row past a dead border
// contributes nothing; on a torus it is the wrapped row, whose count the row
// pass already wrapped.
fn box_count(c: vec2<i32>, n: i32, wrap: bool, r: i32, self_live: u32) -> u32 {
    let size = vec2<i32>(n);
    var sum = 0u;
    for (var dy = -MAX_RADIUS; dy <= MAX_RADIUS; dy = dy + 1) {
        let p = c + vec2<i32>(0, dy);
        let w = ((p % size) + size) % size;
        let inside = p.y >= 0 && p.y < n;
        let row = u32(textureLoad(rows, w, 0).x + 0.5);
        sum = sum + select(0u, row, abs(dy) <= r && (wrap || inside));
    }
    return sum - self_live;
}

// The live neighbours of `c` among its eight.
fn moore_count(c: vec2<i32>, n: i32, wrap: bool) -> u32 {
    var count = 0u;
    for (var dy = -1; dy <= 1; dy = dy + 1) {
        for (var dx = -1; dx <= 1; dx = dx + 1) {
            if (dx != 0 || dy != 0) {
                count = count + live(c + vec2<i32>(dx, dy), n, wrap);
            }
        }
    }
    return count;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let n = i32(params.a.z);
    let wrap = params.a.y != 0u;
    let c = vec2<i32>(in.pos.xy);
    let here = textureLoad(field, c, 0);
    var state = here.x;

    // The age channel: generations since this cell last changed state. A step
    // or a disc that changes the cell restarts it at 0; one that leaves it
    // counts on, saturating at AGE_CAP, which reads as "long ago" and keeps the
    // count exact in a half float. A seed has no history, so its dead cells
    // start at AGE_CAP — no wake from a cell that never lived — and its live
    // ones at 0.
    var age = min(here.y + 1.0, AGE_CAP);

    switch MODE {
        case 1u: {
            state = seeded(c, params.c.x);
            age = select(AGE_CAP, 0.0, state > 0.5);
        }
        case 2u: {
            // A disc of fresh seeded cells, measured across the seam on a
            // torus so a disc near an edge wraps rather than being cut.
            var d = abs(c - vec2<i32>(params.d.xy));
            if (wrap) {
                d = min(d, vec2<i32>(n) - d);
            }
            // A disc is not a generation: a cell it leaves alone keeps its age.
            age = here.y;
            if (u32(d.x * d.x + d.y * d.y) <= params.c.z) {
                state = seeded(c, params.c.y);
            }
            age = select(age, 0.0, state != here.x);
        }
        default: {
            let alive = state > 0.5;
            switch params.a.x {
                case 2u: {
                    // cyclic: a cell advances to the next colour round the
                    // cycle when at least `threshold` of its eight neighbours
                    // already hold it, and is written back inside the cycle
                    // either way.
                    let states = max(params.b.w, 1u);
                    let s = u32(state + 0.5) % states;
                    let next = (s + 1u) % states;
                    let advance = cyclic_count(c, n, wrap, states, next) >= params.d.z;
                    state = f32(select(s, next, advance));
                }
                case 1u: {
                    // larger_than_life: an inclusive interval of whole counts
                    // over the box, one for a dead cell and one for a live one.
                    let count = box_count(c, n, wrap, i32(params.b.z), select(0u, 1u, alive));
                    let lo = select(params.e.x, params.e.z, alive);
                    let hi = select(params.e.y, params.e.w, alive);
                    state = select(0.0, 1.0, count >= lo && count <= hi);
                }
                default: {
                    let count = moore_count(c, n, wrap);
                    let mask = select(params.b.x, params.b.y, alive);
                    state = f32((mask >> count) & 1u);
                }
            }
            age = select(age, 0.0, state != here.x);
        }
    }
    return vec4<f32>(state, age, 0.0, 1.0);
}
"#;

/// The present pass. [`gpu::FULLSCREEN_VS_UV_FLIPPED`](crate::render::gpu::FULLSCREEN_VS_UV_FLIPPED)
/// is prepended at construction.
pub(super) const PRESENT_SHADER: &str = r#"
struct Present {
    // x: hue, y: brightness, z: saturation, w: palette_mix
    a: vec4<f32>,
    // x: palette_steps (integral, quantized CPU-side), y: palette_contour
    // (ADR-0078), z: occlude (ADR-0085), w: grid (cells per side)
    b: vec4<f32>,
    // x: zoom, yz: pan (grid-space view transform, ADR-0018), w: wrap (1 torus)
    c: vec4<f32>,
    // x: trail (generations, >= 0, clamped CPU-side), y: age_tint,
    // z: 1 for the cyclic family, w: cyclic's states (>= 2)
    d: vec4<f32>,
}
@group(0) @binding(0) var field: texture_2d<f32>;
@group(0) @binding(1) var lut_a: texture_2d<f32>;
@group(0) @binding(2) var lut_b: texture_2d<f32>;
@group(0) @binding(3) var lut_samp: sampler;
@group(0) @binding(4) var<uniform> pp: Present;

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

// Shared `palette_contour` (ADR-0078 / ADR-0133), copied verbatim from
// `fragment_field.rs`, whose comment carries the reasoning.
fn band_contour(
    t: f32,
    steps: f32,
    amount: f32,
    lut_a: texture_2d<f32>,
    lut_b: texture_2d<f32>,
    lut_samp: sampler,
    mix_ab: f32,
) -> f32 {
    let f = t * steps;
    let w = max(fwidth(f), 1e-5);
    if (steps < 1.5 || amount <= 0.0) {
        return 1.0;
    }
    let n = round(f);
    let m = clamp(mix_ab, 0.0, 1.0);
    let lo = mix(
        textureSampleLevel(lut_a, lut_samp, vec2<f32>((n - 0.5) / steps, 0.5), 0.0).rgb,
        textureSampleLevel(lut_b, lut_samp, vec2<f32>((n - 0.5) / steps, 0.5), 0.0).rgb,
        m
    );
    let hi = mix(
        textureSampleLevel(lut_a, lut_samp, vec2<f32>((n + 0.5) / steps, 0.5), 0.0).rgb,
        textureSampleLevel(lut_b, lut_samp, vec2<f32>((n + 0.5) / steps, 0.5), 0.0).rgb,
        m
    );
    if (all(abs(hi - lo) < vec3<f32>(0.5 / 255.0))) {
        return 1.0;
    }
    let d = min(fract(f), 1.0 - fract(f));
    return 1.0 - clamp(amount, 0.0, 1.0) * (1.0 - smoothstep(0.0, w, d));
}

// What one cell hands the colour stage: where on the palette it reads (before
// `hue`), and how much light it emits (before `brightness`).
struct Paint {
    coord: f32,
    light: f32,
}

// A live cell lights at the palette's origin. A dead one is its wake: it
// fades as `1 - (age + 1) / (trail + 1)`, so a cell that died this generation
// (age 0) is already dimmer than a live one and the wake is gone after `trail`
// generations — and at `trail = 0` every dead cell emits nothing, which is the
// binary field exactly. As it fades it slides `age_tint` of the way along the
// palette, so history reads as colour as well as light. A live cell's
// coordinate never moves with `age_tint`.
//
// A `cyclic` cell has no dead state: every colour is lit, and its coordinate is
// its state index over `states` with no remap — the cycle is laid round the
// palette once, so a spiral's arms are the palette in order. `trail` and
// `age_tint` are inert there.
fn paint(texel: vec4<f32>) -> Paint {
    let trail = pp.d.x;
    let fade = clamp(1.0 - (texel.y + 1.0) / (trail + 1.0), 0.0, 1.0);
    let live = texel.x > 0.5;
    var p: Paint;
    p.coord = select(pp.d.y * (1.0 - fade), 0.0, live);
    p.light = select(fade, 1.0, live);
    if (pp.d.z > 0.5) {
        let states = max(pp.d.w, 1.0);
        p.coord = (floor(texel.x + 0.5) % states) / states;
        p.light = 1.0;
    }
    return p;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let grid = pp.b.w;
    let zoom = max(pp.c.x, 1e-3);
    // `pan.y` negated because `in.uv` is Y-flipped, so a positive `pan_y`
    // moves the grid the way it moves every other scene.
    let pan = vec2<f32>(pp.c.y, -pp.c.z);
    // A plain normalized stretch of the grid over the target (ADR-0037: no
    // screen-destined geometry here, so no aspect to get wrong); `zoom` above 1
    // magnifies about the centre.
    let uv = (in.uv - vec2<f32>(0.5)) / zoom + vec2<f32>(0.5) + pan;
    let inside = all(uv >= vec2<f32>(0.0)) && all(uv < vec2<f32>(1.0));
    // `fract` first, so the cell index is in range whatever the view transform
    // did, and the torus tiles seamlessly past the frame.
    let n = i32(grid);
    let cell = clamp(vec2<i32>(floor(fract(uv) * grid)), vec2<i32>(0), vec2<i32>(n - 1));
    let p = paint(textureLoad(field, cell, 0));
    let light = select(0.0, p.light, pp.c.w > 0.5 || inside);

    let coord = p.coord + pp.a.x;
    let steps = pp.b.x;
    let palette_mix = pp.a.w;
    let banded = band_coord(coord, steps);
    let ca = textureSampleLevel(lut_a, lut_samp, vec2<f32>(banded, 0.5), 0.0).rgb;
    let cb = textureSampleLevel(lut_b, lut_samp, vec2<f32>(banded, 0.5), 0.0).rgb;
    var col = mix(ca, cb, clamp(palette_mix, 0.0, 1.0));
    col = col * band_contour(coord, steps, pp.b.y, lut_a, lut_b, lut_samp, palette_mix);
    col = apply_saturation(col, pp.a.z);
    col = col * (max(pp.a.y, 0.0) * light);

    // Premultiplied: a cell's light is its coverage, so a dead cell leaves the
    // backdrop untouched. `occlude` (ADR-0085) scales that coverage; the
    // renderer hands 1.0 whenever a post stage owns the seam instead.
    return vec4<f32>(col, light * pp.b.z);
}
"#;
