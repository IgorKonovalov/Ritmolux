//! The cellular system's two fragment stages: the step pass, which seeds,
//! stamps and advances the field one generation per draw, and the present pass,
//! which paints it through the palette.

// Hot-path panic-denial pragma (Plan 0002 Phase 2; render/ is scanned by the
// hygiene guard). Only constants live here, compiled once at construction.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

/// The step pass. [`gpu::FULLSCREEN_VS_UV_FLIPPED`](crate::render::gpu::FULLSCREEN_VS_UV_FLIPPED),
/// a `const MODE: u32` naming the pass (0 step, 1 seed, 2 stamp), `const
/// AGE_CAP: f32` and [`gpu::HASH_WGSL`](crate::render::gpu::HASH_WGSL) are
/// prepended at construction, which is where `VsOut`, `MODE`, `AGE_CAP` and
/// `mix32` come from. The render target is the grid itself, so a fragment's
/// position is its cell.
///
/// Everything here is integer arithmetic over texel values that are whole
/// numbers a half float holds exactly — a state of 0 or 1, an age up to
/// `AGE_CAP` — so a generation is the same on every adapter.
pub(super) const STEP_SHADER: &str = r#"
struct Step {
    // x: family (0 life_like), y: wrap (1 torus), z: grid (cells per side),
    // w: live threshold against the hash's top 24 bits
    a: vec4<u32>,
    // x: birth mask, y: survive mask (bit k: k live neighbours), zw: unused
    b: vec4<u32>,
    // x: the field's seed, y: the stamp's seed, z: stamp radius squared
    // (cells^2), w: unused
    c: vec4<u32>,
    // xy: stamp centre (cells), zw: unused
    d: vec4<u32>,
}
@group(0) @binding(0) var field: texture_2d<f32>;
@group(0) @binding(1) var<uniform> params: Step;

// The field's uniform is shared by all three passes; which pass this is lives
// in `MODE` alone. See `StepParams` for why that split is load-bearing.

// A seeded cell: live when the hash of its coordinates, under `seed`, falls
// below the threshold. Every input is a u32, so the field is the same bit for
// bit on every adapter.
fn seeded(c: vec2<i32>, seed: u32) -> f32 {
    let h = mix32(u32(c.x) ^ mix32(u32(c.y) ^ mix32(seed)));
    return select(0.0, 1.0, (h >> 8u) < params.a.w);
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
            let count = moore_count(c, n, wrap);
            let mask = select(params.b.x, params.b.y, state > 0.5);
            state = f32((mask >> count) & 1u);
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
    // x: trail (generations, >= 0, clamped CPU-side), y: age_tint, zw: unused
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
fn paint(texel: vec4<f32>) -> Paint {
    let trail = pp.d.x;
    let fade = clamp(1.0 - (texel.y + 1.0) / (trail + 1.0), 0.0, 1.0);
    let live = texel.x > 0.5;
    var p: Paint;
    p.coord = select(pp.d.y * (1.0 - fade), 0.0, live);
    p.light = select(fade, 1.0, live);
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
