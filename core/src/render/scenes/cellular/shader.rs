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
/// a uniform is what lost the device on WARP for the analytic field (a WARP
/// reading, unverified on lavapipe as of 2026-10-06).
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

/// The present pass's declarations and helpers: everything but its entry point.
/// [`gpu::FULLSCREEN_VS_UV_FLIPPED`](crate::render::gpu::FULLSCREEN_VS_UV_FLIPPED)
/// is prepended at construction, and [`PRESENT_MAIN`] or
/// [`ROUTE_PRESENT_MAIN`] follows.
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
    // x: palette_contour_style (integral, rounded CPU-side),
    // y: palette_contour_ink (ADR-0197), zw: unused. Its own vec4 because every
    // slot above is spent.
    e: vec4<f32>,
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

// Shared `palette_contour` (ADR-0078 / ADR-0133 / ADR-0197), copied verbatim from
// `fragment_field.rs`, whose comment carries the reasoning.
fn band_contour_ink(
    col: vec3<f32>,
    t: f32,
    steps: f32,
    amount: f32,
    style: f32,
    ink_t: f32,
    lut_a: texture_2d<f32>,
    lut_b: texture_2d<f32>,
    lut_samp: sampler,
    mix_ab: f32,
) -> vec3<f32> {
    let f = t * steps;
    let w = max(fwidth(f), 1e-5);
    if (steps < 1.5 || amount <= 0.0) {
        return col;
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
        return col;
    }
    let d = min(fract(f), 1.0 - fract(f));
    if (style < 0.5) {
        return col * (1.0 - clamp(amount, 0.0, 1.0) * (1.0 - smoothstep(0.0, w, d)));
    }
    let hard = style == 1.0 || style == 3.0;
    let cover = select(1.0 - smoothstep(0.0, w, d), f32(d < w), hard);
    let ink_lut = mix(
        textureSampleLevel(lut_a, lut_samp, vec2<f32>(ink_t, 0.5), 0.0).rgb,
        textureSampleLevel(lut_b, lut_samp, vec2<f32>(ink_t, 0.5), 0.0).rgb,
        m
    );
    let ink = select(vec3<f32>(0.0), ink_lut, style >= 2.0);
    return mix(col, ink, clamp(amount, 0.0, 1.0) * cover);
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
"#;

/// The present pass's entry point when no route is drawn: the field through the
/// palette and nothing else.
pub(super) const PRESENT_MAIN: &str = r#"
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
    col = band_contour_ink(
        col, coord, steps, pp.b.y, pp.e.x, pp.e.y, lut_a, lut_b, lut_samp, palette_mix
    );
    col = apply_saturation(col, pp.a.z);
    col = col * (max(pp.a.y, 0.0) * light);

    // Premultiplied: a cell's light is its coverage, so a dead cell leaves the
    // backdrop untouched. `occlude` (ADR-0085) scales that coverage; the
    // renderer hands 1.0 whenever a post stage owns the seam instead.
    return vec4<f32>(col, light * pp.b.z);
}
"#;

/// The present pass's entry point when the route is drawn (ADR-0266): the field
/// as [`PRESENT_MAIN`] paints it, with the route mixed over it. Follows
/// [`PRESENT_SHADER`]; the route constants
/// (`route::wgsl_consts`) come before both.
///
/// Group 1 is the route's own: the per-cell regions, the control words the GPU
/// keeps, and this pass's uniform. A cell's palette coordinate and the route's
/// both go through one `shade`, so the route reads banding, contour ink and
/// saturation exactly as the field does.
pub(super) const ROUTE_PRESENT_MAIN: &str = r#"
struct RoutePresent {
    // x: route (the overlay's strength), y: route_coord, z: route_grade,
    // w: route_reveal (seconds, >= 0, clamped CPU-side)
    a: vec4<f32>,
    // x: this frame's scene time (ms since configure), yzw: unused
    b: vec4<u32>,
}
@group(1) @binding(0) var<storage, read> route_cells: array<u32>;
@group(1) @binding(1) var<storage, read> route_control: array<u32>;
@group(1) @binding(2) var<uniform> rp: RoutePresent;

// The palette colour at `coord` (hue already added), banded, contour-inked
// and saturated as the plain present colours a cell.
fn shade(coord: f32) -> vec3<f32> {
    let steps = pp.b.x;
    let palette_mix = pp.a.w;
    let banded = band_coord(coord, steps);
    let ca = textureSampleLevel(lut_a, lut_samp, vec2<f32>(banded, 0.5), 0.0).rgb;
    let cb = textureSampleLevel(lut_b, lut_samp, vec2<f32>(banded, 0.5), 0.0).rgb;
    let col = mix(ca, cb, clamp(palette_mix, 0.0, 1.0));
    return apply_saturation(
        band_contour_ink(
            col, coord, steps, pp.b.y, pp.e.x, pp.e.y, lut_a, lut_b, lut_samp, palette_mix
        ),
        pp.a.z
    );
}

// Seconds from the control word `stamp` (ms) to this frame, never negative.
fn since(stamp: u32) -> f32 {
    return f32(select(0u, rp.b.x - stamp, rp.b.x >= stamp)) / 1000.0;
}

// What the route paints on cell `i`: x the overlay's strength, y its palette
// coordinate before hue. Zero strength where it paints nothing — a wall now,
// a cell off the committed route, or no route drawn.
//
// `t` runs from 0 at B to 1 at C. The reveal front moves from 0 to
// `1 + REVEAL_EDGE` over `route_reveal` seconds and a cell eases in across the
// edge's width behind it; at `route_reveal = 0` every cell is in at once.
// From the quiet's break the strength eases out over a quarter of the reveal.
fn route_paint(i: u32, nn: u32, open_now: bool) -> vec2<f32> {
    let shown = route_control[C_SHOWN];
    if (!open_now || shown == SHOWN_NONE) {
        return vec2<f32>(0.0);
    }
    let d = route_cells[R_ROUTE * nn + i];
    if (d == INF) {
        return vec2<f32>(0.0);
    }
    let t = f32(d) / f32(max(route_control[C_LENGTH], 1u));
    let reveal = rp.a.w;
    var appear = 1.0;
    if (reveal > 0.0) {
        let front = since(route_control[C_COMMIT_MS]) / reveal * (1.0 + REVEAL_EDGE);
        appear = smoothstep(0.0, 1.0, (front - t) / REVEAL_EDGE);
    }
    var fade = 1.0;
    if (shown == SHOWN_FADING) {
        let span = reveal * 0.25;
        fade = select(0.0, 1.0 - smoothstep(0.0, 1.0, since(route_control[C_BREAK_MS]) / span), span > 0.0);
    }
    return vec2<f32>(clamp(rp.a.x, 0.0, 1.0) * appear * fade, rp.a.y + rp.a.z * t);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let grid = pp.b.w;
    let zoom = max(pp.c.x, 1e-3);
    let pan = vec2<f32>(pp.c.y, -pp.c.z);
    let uv = (in.uv - vec2<f32>(0.5)) / zoom + vec2<f32>(0.5) + pan;
    let inside = all(uv >= vec2<f32>(0.0)) && all(uv < vec2<f32>(1.0));
    let n = i32(grid);
    let cell = clamp(vec2<i32>(floor(fract(uv) * grid)), vec2<i32>(0), vec2<i32>(n - 1));
    let texel = textureLoad(field, cell, 0);
    let p = paint(texel);
    let shown = pp.c.w > 0.5 || inside;

    let i = u32(cell.y) * u32(n) + u32(cell.x);
    let r = route_paint(i, u32(n * n), texel.x <= 0.5);
    // The route mixes over the cell's own colour by its strength, and lights
    // the cell at least that much.
    let s = select(0.0, r.x, shown);
    let light = max(select(0.0, p.light, shown), s);
    let col = mix(shade(p.coord + pp.a.x), shade(r.y + pp.a.x), s);
    return vec4<f32>(col * (max(pp.a.y, 0.0) * light), light * pp.b.z);
}
"#;

/// What every route pass shares: the per-event uniform. The route constants
/// (`route::wgsl_consts`) precede it.
pub(super) const ROUTE_COMMON: &str = r#"
// One slot per event on the frame's timeline, bound at a dynamic offset.
struct Event {
    // x: grid (cells per side), y: wrap (1 torus), z: this event's scene time
    // (ms since configure), w: work tiles per side
    a: vec4<u32>,
    // unused
    b: vec4<u32>,
}
"#;

/// The route's grid passes (ADR-0266): `compare`, run after every generation,
/// and `work`, run on every route pass the clock owes. The route constants
/// (`route::wgsl_consts`) are prepended at construction.
///
/// **One bind group shape for both**, and what each does is its entry point or
/// the action the control words name — never a field of the uniform, which is
/// the WARP hazard `StepParams` records.
///
/// `work` is dispatched **indirectly**, its workgroup count written by the
/// control pass, so an idle route dispatches nothing. Its action is read once
/// through `workgroupUniformLoad`, which is what makes the barriers inside the
/// relaxation legal under WGSL's uniformity rules.
///
/// The relaxation is **Jacobi, never in place**: a pass reads one half of the
/// distance pair and writes the other, each tile from the read half alone. So
/// a pass is a pure function of the one before it, whatever order an adapter
/// runs workgroups in.
pub(super) const ROUTE_GRID_SHADER: &str = r#"
@group(0) @binding(0) var field: texture_2d<f32>;
@group(0) @binding(1) var<storage, read_write> cells: array<u32>;
@group(0) @binding(2) var<storage, read_write> counts: array<atomic<u32>>;
@group(0) @binding(3) var<storage, read> control: array<u32>;
@group(0) @binding(4) var<uniform> ev: Event;

// The tile and its one-cell halo, as distances and as open bits.
var<workgroup> tile_d: array<u32, SPAN_CELLS>;
var<workgroup> tile_open: array<u32, SPAN_CELLS>;
var<workgroup> wg_action: u32;
// Per-workgroup partials, published by one invocation at the end.
var<workgroup> wg_count: atomic<u32>;
var<workgroup> wg_max: atomic<u32>;
var<workgroup> wg_min: atomic<u32>;

// The index of the cell at `c`: wrapped on a torus, INF past a border.
fn wrapped(c: vec2<i32>, n: i32, wrap: bool) -> u32 {
    let inside = all(c >= vec2<i32>(0)) && all(c < vec2<i32>(n));
    let w = ((c % vec2<i32>(n)) + vec2<i32>(n)) % vec2<i32>(n);
    return select(INF, u32(w.y * n + w.x), inside || wrap);
}

// How far cell `i`'s centre lies from the grid's, squared, in half-cells:
// `(2x + 1 - n)^2 + (2y + 1 - n)^2`. Exact in u32 up to MAX_GRID.
fn centre_key(i: u32, n: u32) -> u32 {
    let m = i32(n);
    let dx = 2 * i32(i % n) + 1 - m;
    let dy = 2 * i32(i / n) + 1 - m;
    return u32(dx * dx + dy * dy);
}

// Counts the cells whose open bit (dead = open) differs from the last
// generation's, and the open cells, then records this generation's mask.
@compute @workgroup_size(16, 16)
fn compare(
    @builtin(global_invocation_id) gid: vec3<u32>,
    @builtin(local_invocation_index) li: u32,
) {
    if (li == 0u) {
        atomicStore(&wg_count, 0u);
        atomicStore(&wg_max, 0u);
    }
    workgroupBarrier();
    let n = ev.a.x;
    let nn = n * n;
    if (gid.x < n && gid.y < n) {
        let i = gid.y * n + gid.x;
        let open = select(1u, 0u, textureLoad(field, vec2<i32>(gid.xy), 0).x > 0.5);
        if (open != cells[R_PREV * nn + i]) {
            atomicAdd(&wg_count, 1u);
        }
        atomicAdd(&wg_max, open);
        cells[R_PREV * nn + i] = open;
    }
    workgroupBarrier();
    if (li == 0u) {
        atomicAdd(&counts[K_MOVED], atomicLoad(&wg_count));
        atomicAdd(&counts[K_OPEN], atomicLoad(&wg_max));
    }
}

// A_SNAPSHOT: freeze the last compared mask, and find the open cell nearest
// the centre's key.
fn snapshot(i: u32, n: u32, nn: u32) {
    let open = cells[R_PREV * nn + i];
    cells[R_SNAP * nn + i] = open;
    if (open == 1u) {
        atomicMin(&wg_min, centre_key(i, n));
    }
}

// A_INDEX: the lowest index whose key equals the one the control pass named —
// the centre's distance, or the largest finite distance of the field. After
// sweep 2 it also keeps that sweep's field as d_B.
fn find_index(i: u32, n: u32, nn: u32) {
    let d = cells[(R_DIST + control[C_PARITY]) * nn + i];
    if (control[C_SEARCH] == 2u) {
        cells[R_KEPT * nn + i] = d;
    }
    if (cells[R_SNAP * nn + i] != 1u) {
        return;
    }
    var key = d;
    if (control[C_KEY_KIND] == KIND_CENTRE) {
        key = centre_key(i, n);
    }
    if (key == control[C_KEY]) {
        atomicMin(&wg_min, i);
    }
}

// A_COMMIT: an open cell is on the route when it lies on some shortest path
// from B to C, `d_B + d_C == d_B(C)`, and then holds d_B; every other cell
// holds the sentinel. Both distances are finite there, and below MAX_GRID^2,
// so the sum cannot wrap.
fn commit(i: u32, nn: u32) {
    let b = cells[R_KEPT * nn + i];
    let c = cells[(R_DIST + control[C_PARITY]) * nn + i];
    let on = cells[R_SNAP * nn + i] == 1u && b != INF && c != INF
        && b + c == control[C_PENDING_LENGTH];
    cells[R_ROUTE * nn + i] = select(INF, b, on);
}

// A_RELAX: one pass of `d(x) = min(d(x), 1 + min over open neighbours d(n))`
// over this tile, LOCAL_ITERATIONS deep in workgroup memory, the halo held at
// the read half's values.
fn relax(wg: vec2<u32>, lid: vec2<u32>, li: u32, n: u32, wrap: bool) {
    let nn = n * n;
    let parity = control[C_PARITY];
    let fresh = control[C_FRESH] != 0u;
    let source = control[C_SOURCE];
    let read = (R_DIST + parity) * nn;
    let origin = vec2<i32>(wg * TILE) - vec2<i32>(1);
    for (var k = li; k < SPAN * SPAN; k = k + TILE * TILE) {
        let c = origin + vec2<i32>(i32(k % SPAN), i32(k / SPAN));
        let j = wrapped(c, i32(n), wrap);
        var open = 0u;
        var d = INF;
        if (j != INF) {
            open = cells[R_SNAP * nn + j];
            // A fresh sweep starts from the source alone, whatever the pair
            // holds from the last one.
            d = select(cells[read + j], select(INF, 0u, j == source), fresh);
            d = select(INF, d, open == 1u);
        }
        tile_open[k] = open;
        tile_d[k] = d;
    }
    workgroupBarrier();

    let me = (lid.y + 1u) * SPAN + lid.x + 1u;
    let gid = wg * TILE + lid;
    let mine = gid.x < n && gid.y < n;
    let moves = mine && tile_open[me] == 1u;
    let start = tile_d[me];
    var d = start;
    for (var it = 0u; it < LOCAL_ITERATIONS; it = it + 1u) {
        if (moves) {
            let m = min(
                min(tile_d[me - 1u], tile_d[me + 1u]),
                min(tile_d[me - SPAN], tile_d[me + SPAN])
            );
            if (m != INF) {
                d = min(d, m + 1u);
            }
        }
        workgroupBarrier();
        tile_d[me] = d;
        workgroupBarrier();
    }
    if (mine) {
        cells[(R_DIST + 1u - parity) * nn + gid.y * n + gid.x] = d;
        if (d != start) {
            atomicStore(&wg_count, 1u);
        }
        if (d != INF) {
            atomicMax(&wg_max, d);
        }
    }
}

@compute @workgroup_size(16, 16)
fn work(
    @builtin(workgroup_id) wg: vec3<u32>,
    @builtin(local_invocation_id) lid: vec3<u32>,
    @builtin(local_invocation_index) li: u32,
) {
    if (li == 0u) {
        wg_action = control[C_ACTION];
        atomicStore(&wg_count, 0u);
        atomicStore(&wg_max, 0u);
        atomicStore(&wg_min, INF);
    }
    let action = workgroupUniformLoad(&wg_action);
    let n = ev.a.x;
    let nn = n * n;
    let gid = wg.xy * TILE + lid.xy;
    let mine = gid.x < n && gid.y < n;
    let i = gid.y * n + gid.x;
    if (action == A_RELAX) {
        relax(wg.xy, lid.xy, li, n, ev.a.y != 0u);
    } else if (action == A_SNAPSHOT && mine) {
        snapshot(i, n, nn);
    } else if (action == A_INDEX && mine) {
        find_index(i, n, nn);
    } else if (action == A_COMMIT && mine) {
        commit(i, nn);
    }
    workgroupBarrier();
    if (li == 0u) {
        if (action == A_RELAX) {
            atomicAdd(&counts[K_CHANGED], atomicLoad(&wg_count));
            atomicMax(&counts[K_MAX], atomicLoad(&wg_max));
        } else if (action == A_SNAPSHOT) {
            atomicMin(&counts[K_KEY], atomicLoad(&wg_min));
        } else if (action == A_INDEX) {
            atomicMin(&counts[K_INDEX], atomicLoad(&wg_min));
        }
    }
}
"#;

/// The route's control pass (ADR-0266): one invocation that reads what the last
/// grid pass counted and decides what the next one does. `after_generation`
/// follows a generation's compare pass and `after_work` a work pass; each is
/// handed its own event's scene time. The route constants are prepended.
///
/// It is the only thing that decides the route's state, and nothing reads the
/// state back to the CPU.
pub(super) const ROUTE_CONTROL_SHADER: &str = r#"
@group(0) @binding(0) var<uniform> ev: Event;
@group(0) @binding(1) var<storage, read> cells: array<u32>;
@group(0) @binding(2) var<storage, read_write> counts: array<u32>;
@group(0) @binding(3) var<storage, read_write> control: array<u32>;

// Name the next work pass, and dispatch it over `groups` tiles a side — zero
// for none.
fn dispatch(action: u32, groups: u32) {
    control[C_ACTION] = action;
    control[C_ARGS] = groups;
    control[C_ARGS + 1u] = groups;
    control[C_ARGS + 2u] = 1u;
}

// Clear the partials a work pass accumulates.
fn clear_counts() {
    counts[K_CHANGED] = 0u;
    counts[K_MAX] = 0u;
    counts[K_KEY] = INF;
    counts[K_INDEX] = INF;
}

// Start sweep `search` (1, 2 or 3) from `source`: the next relax pass reads a
// field holding 0 there and INF everywhere else.
fn sweep_from(source: u32, search: u32) {
    control[C_SOURCE] = source;
    control[C_FRESH] = 1u;
    control[C_SEARCH] = search;
    control[C_SWEEP_PASSES] = 0u;
    control[C_CONVERGED] = 0u;
    dispatch(A_RELAX, ev.a.w);
}

// Drop the epoch in flight, if any, where it stands. Its fields are never
// committed.
fn abandon() {
    if (control[C_SEARCH] != 0u || control[C_ACTION] != A_NONE) {
        control[C_ABANDONED] = control[C_ABANDONED] + 1u;
    }
    control[C_SEARCH] = 0u;
    clear_counts();
    dispatch(A_NONE, 0u);
}

// A fading route is cleared once its fade has run, at this event's time.
fn settle_fade() {
    if (control[C_SHOWN] == SHOWN_FADING && ev.a.z - control[C_BREAK_MS] >= ev.b.x) {
        control[C_SHOWN] = SHOWN_NONE;
    }
}

@compute @workgroup_size(1)
fn after_generation() {
    let moved = counts[K_MOVED];
    let open = counts[K_OPEN];
    control[C_LAST_MOVED] = moved;
    control[C_LAST_OPEN] = open;
    control[C_GENERATIONS] = control[C_GENERATIONS] + 1u;
    counts[K_MOVED] = 0u;
    counts[K_OPEN] = 0u;
    // Still: at most the tolerance's fraction of the open cells changed. Both
    // products stay below 2^32 up to MAX_GRID.
    if (moved * QUIET_TOLERANCE_DEN <= open * QUIET_TOLERANCE_NUM) {
        control[C_STILL_RUN] = min(control[C_STILL_RUN] + 1u, QUIET_HOLD + 1u);
        // The quiet begins on the generation the run reaches the hold, and
        // only then: a standing maze starts one epoch, not one a generation.
        if (control[C_STILL_RUN] == QUIET_HOLD) {
            clear_counts();
            control[C_SEARCH] = 0u;
            control[C_EPOCH_PASSES] = 0u;
            dispatch(A_SNAPSHOT, ev.a.w);
        }
    } else {
        control[C_STILL_RUN] = 0u;
        abandon();
        if (control[C_SHOWN] == SHOWN_ON) {
            control[C_SHOWN] = SHOWN_FADING;
            control[C_BREAK_MS] = ev.a.z;
        }
    }
    settle_fade();
}

@compute @workgroup_size(1)
fn after_work() {
    let action = control[C_ACTION];
    let nn = ev.a.x * ev.a.x;
    if (action != A_NONE) {
        control[C_EPOCH_PASSES] = control[C_EPOCH_PASSES] + 1u;
    }
    if (action == A_SNAPSHOT) {
        let key = counts[K_KEY];
        clear_counts();
        if (key == INF) {
            // No open cell: nothing to search.
            dispatch(A_NONE, 0u);
            return;
        }
        // The last route's far end, while it is still open, so a maze that
        // moves a little keeps its route in the same component; the centre
        // rule otherwise.
        let end = control[C_END];
        if (end != 0u && end <= nn && cells[R_SNAP * nn + end - 1u] == 1u) {
            sweep_from(end - 1u, 1u);
        } else {
            control[C_KEY] = key;
            control[C_KEY_KIND] = KIND_CENTRE;
            dispatch(A_INDEX, ev.a.w);
        }
    } else if (action == A_INDEX) {
        let found = counts[K_INDEX];
        clear_counts();
        let search = control[C_SEARCH];
        if (search == 0u) {
            sweep_from(found, 1u);
        } else if (search == 1u) {
            // B: sweep 2 starts there.
            sweep_from(found, 2u);
        } else {
            // C: the index pass kept d_B; sweep 3 starts at C.
            control[C_PENDING_C] = found;
            sweep_from(found, 3u);
        }
    } else if (action == A_RELAX) {
        let changed = counts[K_CHANGED];
        let far = counts[K_MAX];
        control[C_D_MAX] = far;
        control[C_LAST_CHANGED] = changed;
        clear_counts();
        control[C_FRESH] = 0u;
        control[C_PARITY] = 1u - control[C_PARITY];
        control[C_RELAX_PASSES] = control[C_RELAX_PASSES] + 1u;
        control[C_SWEEP_PASSES] = control[C_SWEEP_PASSES] + 1u;
        // A pass that changed nothing is the fixed point: the sweep has
        // converged, and the pass's largest finite distance is the field's.
        if (changed == 0u) {
            control[C_CONVERGED] = control[C_SWEEP_PASSES];
            let search = control[C_SEARCH];
            if (search == 3u) {
                dispatch(A_COMMIT, ev.a.w);
            } else {
                if (search == 2u) {
                    control[C_PENDING_LENGTH] = far;
                }
                control[C_KEY] = far;
                control[C_KEY_KIND] = KIND_FAR;
                dispatch(A_INDEX, ev.a.w);
            }
        }
    } else if (action == A_COMMIT) {
        control[C_LENGTH] = control[C_PENDING_LENGTH];
        control[C_END] = control[C_PENDING_C] + 1u;
        control[C_SHOWN] = SHOWN_ON;
        control[C_COMMIT_MS] = ev.a.z;
        control[C_COMMITS] = control[C_COMMITS] + 1u;
        control[C_LAST_EPOCH] = control[C_EPOCH_PASSES];
        control[C_SEARCH] = 0u;
        // Nothing is relaxed again until the quiet breaks and returns.
        dispatch(A_NONE, 0u);
    }
    settle_fade();
}
"#;
