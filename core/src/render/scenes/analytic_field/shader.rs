//! The analytic field's fragment stage: one fullscreen pass, the family chosen
//! by an integer the CPU writes into the uniform rather than by a shader
//! permutation.

// Hot-path panic-denial pragma (Plan 0002 Phase 2; render/ is scanned by the
// hygiene guard). Only a constant lives here, compiled once at construction.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

/// The fragment half. [`gpu::FULLSCREEN_VS_NDC`](crate::render::gpu::FULLSCREEN_VS_NDC)
/// is prepended at construction, which is where `VsOut` comes from.
pub(super) const SHADER: &str = r#"
struct Params {
    // x: aspect (width / height of the RENDER TARGET, ADR-0037), y: zoom,
    // zw: pan (field-space offset, ADR-0018)
    a: vec4<f32>,
    // x: hue, y: color_span, z: color_center, w: saturation
    b: vec4<f32>,
    // x: palette_mix (A/B crossfade), y: occlude (ADR-0085),
    // z: palette_steps (integral, quantized CPU-side), w: palette_contour (ADR-0078)
    c: vec4<f32>,
    // x: brightness, y: family index, zw: mode_n, mode_m (whole numbers, >= 1,
    // clamped and rounded CPU-side)
    d: vec4<f32>,
    // x: line_width (plate units), y: plate_mix, z: one pixel of the target in
    // field units, w: unused
    e: vec4<f32>,
    // x, y: c (the Julia constant), z: iterations (whole, >= 1, capped
    // CPU-side), w: escape_radius (clamped CPU-side into [2, 256])
    f: vec4<f32>,
    // x: power (clamped CPU-side into [1.1, 8]), y: interior light,
    // z: map (0 julia, 1 mandelbrot), w: unused
    g: vec4<f32>,
}

// One group, the LUTs before the uniform. The order is what keeps this layout a
// shape no other layout in the crate has (ADR-0058): `fragment_field` splits the
// same four bindings over two groups, and no free single-uniform shape was left
// to give this scene a group of its own.
@group(0) @binding(0) var lut_a: texture_2d<f32>;
@group(0) @binding(1) var lut_b: texture_2d<f32>;
@group(0) @binding(2) var lut_samp: sampler;
@group(0) @binding(3) var<uniform> params: Params;

const PI: f32 = 3.14159265358979;

// Shared `saturation` (mirrors core/src/render/palette.rs::desaturate verbatim):
// scale chroma around Rec. 601 luma. 1.0 unchanged, 0.0 grayscale.
fn apply_saturation(c: vec3<f32>, s: f32) -> vec3<f32> {
    let luma = dot(c, vec3<f32>(0.299, 0.587, 0.114));
    return vec3<f32>(luma) + (c - vec3<f32>(luma)) * s;
}

// Shared `palette_steps` (mirrors core/src/render/palette.rs::band_coord
// verbatim, ADR-0078): snap the palette coordinate to a band centre before the
// LUT read. Below 1.5 steps it is the exact identity, not a one-band degenerate.
fn band_coord(t: f32, steps: f32) -> f32 {
    if (steps < 1.5) {
        return t;
    }
    return (floor(t * steps) + 0.5) / steps;
}

// Shared `palette_contour` (ADR-0078 / ADR-0133), copied verbatim from
// `fragment_field.rs`, whose comment carries the reasoning: a one-pixel line at
// each band edge, drawn only where the ink actually changes.
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

// What one family hands the colour stage: where on the palette this pixel
// reads (before `color_span` / `color_center` / `hue`), and how much light it
// emits (before `brightness`).
struct Sample {
    coord: f32,
    light: f32,
}

// The Chladni plate: the zero set of
//     cos(n pi x) cos(m pi y) - cos(m pi x) cos(n pi y)
// over the unit plate, which spans [-1, 1] on the target's short axis at zoom 1.
//
// The band is lit by DISTANCE to the nodal set, |f| / |grad f|, rather than by
// |f| itself: |f| rises steeply where the modes are dense and shallowly where
// they are not, so a threshold on it draws lines whose width wanders across the
// plate. The first-order distance is uniform, which is what reads as sand. Where
// two nodal lines cross the gradient vanishes too and the ratio stays finite
// (both go to zero, |f| one order faster); the floor only guards the exact
// zero.
fn chladni(p: vec2<f32>) -> Sample {
    let n = params.d.z;
    let m = params.d.w;
    let q = (p + vec2<f32>(1.0)) * 0.5;
    let nx = n * PI * q.x;
    let ny = n * PI * q.y;
    let mx = m * PI * q.x;
    let my = m * PI * q.y;
    let f = cos(nx) * cos(my) - cos(mx) * cos(ny);
    // d/dq, halved for d/dp because q = (p + 1) / 2.
    let gx = 0.5 * PI * (-n * sin(nx) * cos(my) + m * sin(mx) * cos(ny));
    let gy = 0.5 * PI * (-m * cos(nx) * sin(my) + n * cos(mx) * sin(ny));
    let dist = abs(f) / max(length(vec2<f32>(gx, gy)), 1e-4);

    let half_width = 0.5 * max(params.e.x, 0.0);
    let aa = max(params.e.z, 1e-6);
    let line = 1.0 - smoothstep(half_width - 0.5 * aa, half_width + 0.5 * aa, dist);
    // The raw signed field, in [0, 1]: |f| <= 2 on the whole plane.
    let signed = clamp(0.5 + 0.25 * f, 0.0, 1.0);
    let mix_raw = clamp(params.e.y, 0.0, 1.0);

    var s: Sample;
    s.coord = mix(line, signed, mix_raw);
    s.light = mix(line, 1.0, mix_raw);
    return s;
}

// How many iterations of smooth count one traversal of the palette spans at
// `color_span = 1`. A FIXED scale, not `iterations`: a pixel that escapes at
// step 7 keeps its colour whatever the budget is, so raising or capping the
// budget changes only the pixels that escape past it. The LUT is
// repeat-addressed, so deeper counts cycle the palette.
const ITERATIONS_PER_PALETTE: f32 = 32.0;

// z^power. A whole power multiplies exactly, which keeps z^2 + c the textbook
// map to the last bit; a fractional one goes through the polar form, whose
// angle `atan2` cuts along the negative real axis — the seam a fractional
// power shows. The origin is its own image, and is selected over whatever
// `atan2(0, 0)` makes of it.
//
// ONE EXIT, AND A CONSTANT LOOP BOUND. An early `return` out of the whole-power
// branch, or a trip count read from `power`, compiles and validates and then
// loses the device at the first draw on the DX12 software adapter — even on a
// frame whose family never calls this. The multiply loop runs to the largest
// power `MAX_POWER` allows and skips the steps past `power`.
fn cpow(z: vec2<f32>, power: f32) -> vec2<f32> {
    var out = z;
    if (fract(power) == 0.0) {
        for (var i = 2.0; i <= 8.0; i = i + 1.0) {
            if (i <= power) {
                out = vec2<f32>(out.x * z.x - out.y * z.y, out.x * z.y + out.y * z.x);
            }
        }
    } else {
        let r2 = dot(z, z);
        let theta = atan2(z.y, z.x);
        let rp = exp(0.5 * power * log(max(r2, 1e-30)));
        let polar = rp * vec2<f32>(cos(power * theta), sin(power * theta));
        out = select(polar, vec2<f32>(0.0), r2 < 1e-30);
    }
    return out;
}

// Escape time: z -> z^power + c, iterated until |z| passes the escape radius
// or the budget runs out. `map` chooses whether c is the constant (Julia, the
// orbit starting at the pixel) or the pixel (Mandelbrot, the orbit starting at
// zero).
//
// The palette coordinate is the SMOOTH count
//     nu = n + 1 - log_p( ln|z_n| / ln R )
// which runs continuously across the step where the integer count n jumps:
// |z_n| just past R gives n + 1, and |z_n| near R^p gives n — the value the
// neighbour that escaped one step earlier reached. The integer count bands the
// palette into contour steps; this is what makes the boundary read as a glow.
//
// Every term is kept finite by construction: the pixel is clamped before it
// seeds the orbit, the radius and power are clamped CPU-side, `|z|^2` is
// clamped below f32's ceiling before its log, and an escaped |z| exceeds R so
// the inner log's argument is at least 1.
fn escape_time(p: vec2<f32>) -> Sample {
    let iterations = u32(params.f.z);
    let radius = params.f.w;
    let power = params.g.x;
    let mandelbrot = params.g.z > 0.5;
    let q = clamp(p, vec2<f32>(-64.0), vec2<f32>(64.0));
    var z = select(q, vec2<f32>(0.0), mandelbrot);
    let c = select(params.f.xy, q, mandelbrot);
    let r2 = radius * radius;

    var n = 0u;
    var escaped = false;
    loop {
        if (n >= iterations) {
            break;
        }
        z = cpow(z, power) + c;
        n = n + 1u;
        if (dot(z, z) > r2) {
            escaped = true;
            break;
        }
    }

    var s: Sample;
    if (escaped) {
        let log_mod = 0.5 * log(min(dot(z, z), 1e37));
        let nu = f32(n) + 1.0 - log(max(log_mod / log(radius), 1.0)) / log(power);
        s.coord = max(nu, 0.0) / ITERATIONS_PER_PALETTE;
        s.light = 1.0;
    } else {
        // The set itself: coloured by how far out its orbit ended, which is
        // bounded for a point that never escaped.
        s.coord = 0.25 * min(length(z), 2.0);
        s.light = clamp(params.g.y, 0.0, 1.0);
    }
    return s;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let aspect = params.a.x;
    let zoom = max(params.a.y, 1e-3);
    let pan = params.a.zw;

    // The field's own coordinates: the short axis spans [-1, 1] at zoom 1 and
    // one field unit is the same number of pixels on both axes, because the
    // aspect is the render target's (ADR-0037). `zoom` above 1 magnifies.
    var uv = in.ndc;
    uv.x = uv.x * aspect;
    let p = uv / zoom + pan;

    let family = u32(params.d.y + 0.5);
    var s: Sample;
    switch family {
        case 1u: {
            s = escape_time(p);
        }
        default: {
            s = chladni(p);
        }
    }

    let hue = params.b.x;
    let color_span = params.b.y;
    let color_center = params.b.z;
    let saturation = params.b.w;
    let palette_mix = params.c.x;
    let palette_steps = params.c.z;
    let palette_contour = params.c.w;

    let coord = s.coord * color_span + color_center + hue;
    let banded = band_coord(coord, palette_steps);
    // `textureSampleLevel`: the family branch above is on a uniform, but the
    // LUT has one mip and an explicit level keeps this read free of any
    // uniformity requirement a later non-uniform branch would bring.
    let ca = textureSampleLevel(lut_a, lut_samp, vec2<f32>(banded, 0.5), 0.0).rgb;
    let cb = textureSampleLevel(lut_b, lut_samp, vec2<f32>(banded, 0.5), 0.0).rgb;
    var col = mix(ca, cb, clamp(palette_mix, 0.0, 1.0));
    col = col * band_contour(
        coord, palette_steps, palette_contour, lut_a, lut_b, lut_samp, palette_mix
    );
    col = apply_saturation(col, saturation);
    col = col * (max(params.d.x, 0.0) * s.light);

    // Alpha is `occlude` across every pixel, as `fragment_field` emits it: the
    // field covers the whole frame, and `occlude` scales how much of that the
    // backdrop resolves against (ADR-0085). The renderer hands a literal 1.0
    // whenever a post stage owns the seam instead.
    return vec4<f32>(col, params.c.y);
}
"#;
