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
