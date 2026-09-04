//! The warp mesh's four WGSL programs and the POD blocks that feed them.
//!
//! Text and layout only: the shader source, the four uniform structs the CPU
//! fills each frame, and the vertex the warp pipeline reads. The `wgpu` objects
//! built from them are in [`resources`](super::resources); the scene that drives
//! them is in `mod.rs`.
//!
//! Not to be confused with [`shader`](super::shader), which is the runtime for a
//! **converted** preset's own WGSL (ADR-0113). Nothing here is generated.

// No Rust code path here beyond four POD derives, but the hygiene guard greps
// every file under render/ for the sentinel, so the pragma is carried.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

/// The warp pass: one draw over the mesh, resampling the past.
///
/// The vertex stage does the transform rather than the CPU, deliberately. The CPU
/// hands over the nine *outputs* — which is what the per-vertex program produced —
/// and the shader turns them into a source uv. That keeps the per-vertex
/// evaluation on the render thread down to the expressions themselves (which is
/// what the tier capacity is measured against), and it puts the four warp
/// sinusoids on the GPU where they cost nothing.
pub(super) const WARP_SHADER: &str = r#"
struct Warp {
    // x: render-target aspect, y: dt (s), z: scene time (s), w: warp_scale
    misc: vec4<f32>,
    // x: decay^dt, y: WARP PHASE (not the rate - integrated CPU-side per
    //    ADR-0132; see the note at its read below), z: wrap (0/1),
    // w: darken_center amount
    misc2: vec4<f32>,
    // x: feedback quantize steps (0 = off, negative = ADR-0118 Alternative D),
    // yzw: unused
    misc3: vec4<f32>,
}
@group(0) @binding(0) var<uniform> wu: Warp;
@group(0) @binding(1) var past: texture_2d<f32>;
@group(0) @binding(2) var past_samp: sampler;

// A per-frame scale raised to `dt`, with its SIGN carried around the outside.
//
// A scale is a rate, so it is raised to `dt` (ADR-0019) - but `pow()` of a
// negative base is undefined, and a negative scale is MilkDrop's standard mirror
// idiom (3.5 % of the corpus writes one). Flooring the value itself, as this
// stage did before, replaced every mirror with a near-zero positive scale;
// flooring the MAGNITUDE keeps the guard `pow` needs and lets the flip through.
// A zero stays on the positive arm, so the expression is bit-identical to the
// old one for every `v >= 0` - nothing shipping a positive scale moves.
//
// Applying the flip on every frame is safe under any frame rate, which is the
// non-obvious half: a mirror composed with itself is the identity, so the
// symmetric fixed point a preset converges to is the same at 30 Hz as at 144 Hz,
// even though the intermediate frames differ. A signed rate does not interpolate
// the way a positive one does, and it does not have to.
fn signed_rate(v: f32, dt: f32) -> f32 {
    let m = pow(max(abs(v), 1e-4), dt);
    return select(m, -m, v < 0.0);
}

struct VsIn {
    @location(0) clip: vec2<f32>,
    // zoom, rot, cx, cy
    @location(1) t0: vec4<f32>,
    // dx, dy, sx, sy
    @location(2) t1: vec4<f32>,
    // warp, unused...
    @location(3) t2: vec4<f32>,
}

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) src: vec2<f32>,
    // The vertex's own destination uv, for the fragment stage's centre darkening.
    @location(1) uv: vec2<f32>,
}

@vertex
fn vs_main(in: VsIn) -> VsOut {
    let aspect = wu.misc.x;
    let dt     = wu.misc.y;
    let time   = wu.misc.z;
    let wscale = max(wu.misc.w, 1e-3);
    // The warp's phase, ALREADY INTEGRATED (ADR-0132). This slot used to carry
    // `warp_speed` and this stage computed `time * wspeed` — which meant that
    // the first preset to bind the rate to audio would find that a swing from
    // 1.0 to 1.5 at t = 100 s moves the phase fifty seconds in one frame. The
    // picture did not speed up; it jumped. Nothing caught it because no shipped
    // preset binds `warp_speed`.
    let wphase = wu.misc2.y;

    // This vertex's own destination uv (texture space, y down).
    let uv = vec2<f32>(in.clip.x * 0.5 + 0.5, 0.5 - in.clip.y * 0.5);

    // Rates are per second (ADR-0019): a factor is raised to dt, an amount is
    // multiplied by it, so two half-length frames compose to one full-length one.
    let zoom = signed_rate(in.t0.x, dt);
    let rot  = in.t0.y * dt;
    let ctr  = in.t0.zw;
    let d    = in.t1.xy * dt;
    let sx   = signed_rate(in.t1.z, dt);
    let sy   = signed_rate(in.t1.w, dt);
    let warp = in.t2.x * dt;

    // **The stage order is MilkDrop's**, and it matters: `zoom` is about the
    // FRAME CENTRE while `sx`/`sy` and `rot` are about the per-vertex `cx`/`cy`,
    // and a preset with an off-centre `cx` renders a different picture if the two
    // are collapsed into one origin. The whole point of a converted preset is
    // that it moves the way its author saw, so the reference's order is the one
    // to keep — see `milk::MilkRuntime` for the rest of the mapping.
    //
    // The INVERSE of the motion the outputs name throughout: a destination vertex
    // asks where its content came from, so a `zoom` above 1 shrinks the source
    // window and the past appears to grow.

    // 1. zoom, about the frame centre. Aspect-corrected on the way in and out
    //    (ADR-0037: the RENDER TARGET's aspect, never the mesh's), so a zoom is
    //    isotropic rather than a squash on a wide display.
    var p = uv - vec2<f32>(0.5, 0.5);
    p.x = p.x * aspect;
    p = p / zoom;
    p.x = p.x / aspect;
    p = p + vec2<f32>(0.5, 0.5);

    // 2. stretch, about the per-vertex centre.
    p = (p - ctr) / vec2<f32>(sx, sy) + ctr;

    // 3. the procedural warp, MilkDrop's four sinusoids: a wobble whose own
    //    frequencies drift, so it never settles into a visible standing pattern.
    //    Applied in uv, where the reference applies it.
    let wt = wphase;
    let f0 = 11.68 + 4.0 * cos(wt * 1.413 + 10.0);
    let f1 =  8.77 + 3.0 * cos(wt * 1.113 +  7.0);
    let f2 = 10.54 + 3.0 * cos(wt * 1.233 +  3.0);
    let f3 = 11.49 + 4.0 * cos(wt * 0.933 +  5.0);
    let inv = 1.0 / wscale;
    let wx = uv.x * 2.0 - 1.0;
    let wy = 1.0 - uv.y * 2.0;
    p.x = p.x + warp * 0.0035 * sin(wt * 0.333 + inv * (wx * f0 - wy * f3));
    p.y = p.y + warp * 0.0035 * cos(wt * 0.375 - inv * (wx * f2 + wy * f1));
    p.x = p.x + warp * 0.0035 * cos(wt * 0.753 - inv * (wx * f1 - wy * f2));
    p.y = p.y + warp * 0.0035 * sin(wt * 0.825 + inv * (wx * f0 + wy * f3));

    // 4. rotate, about the per-vertex centre, aspect-corrected so it is a
    //    rotation rather than a shear.
    var q = p - ctr;
    q.x = q.x * aspect;
    let c = cos(rot);
    let s = sin(rot);
    q = vec2<f32>(q.x * c - q.y * s, q.x * s + q.y * c);
    q.x = q.x / aspect;

    // 5. translate, in uv — `dx`/`dy` are uv per second, which is the source
    //    vocabulary's own unit.
    var out: VsOut;
    out.pos = vec4<f32>(in.clip, 0.0, 1.0);
    out.src = q + ctr - d;
    out.uv = uv;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let wrap = wu.misc2.z;
    // `wrap` — MilkDrop's `bTexWrap`, and **58 % of the corpus sets it**
    // (6 014 of 10 347, measured 2026-08-16). With it on the past is toroidal, so
    // a zoom that pulls content off one edge brings it back on the other; that is
    // what most of the classic tunnels are made of, and clamping instead smears
    // the border row outward.
    //
    // Off, the transparent-border edge policy applies (ADR-0048): an off-field
    // read contributes NOTHING. Clamping would re-deposit the border texel every
    // frame until the edge became a permanent bar of colour.
    let wrapped = fract(in.src);
    let src = select(in.src, wrapped, wrap > 0.5);
    let inside = select(
        f32(all(in.src >= vec2<f32>(0.0)) && all(in.src <= vec2<f32>(1.0))),
        1.0,
        wrap > 0.5
    );
    let past_c = textureSampleLevel(past, past_samp, src, 0.0);

    // `darken_center` — MilkDrop draws a small dark blob at the middle of every
    // frame, which is what stops a zooming feedback loop saturating there. This
    // is the same gesture as a multiply rather than as a drawn quad: same effect,
    // one fewer pass. Its radius is in frame-heights, so it is round on any
    // display.
    var centred = in.uv - vec2<f32>(0.5, 0.5);
    centred.x = centred.x * wu.misc.x;
    let dc = 1.0 - wu.misc2.w * (1.0 - smoothstep(0.0, 0.32, length(centred)));

    let faded = past_c * (wu.misc2.x * inside * dc);
    // The 8-bit floor an MD1-era preset's feedback field had (ADR-0118). A
    // bundle with no custom warp shader takes THIS path, so the quantizer has to
    // reach it too or a whole era of the corpus washes out unfixed. Off — every
    // native `warp_mesh` preset — `rlx_quantize` returns its argument, so this
    // line is an exact identity.
    return vec4<f32>(rlx_quantize(faded.rgb, wu.misc3.x), faded.a);
}
"#;

/// The deposit pass: this frame's light, laid onto the warped past.
pub(super) const DEPOSIT_SHADER: &str = r#"
struct Deposit {
    // x: aspect, y: amount * dt, z: ring radius, w: ring width
    a: vec4<f32>,
    // xy: centre (uv), z: arms, w: twist
    b: vec4<f32>,
    // x: spin phase (rad), y: hue + color_center, z: color_span, w: saturation
    c: vec4<f32>,
    // x: palette_mix, y: palette_steps, z: palette_contour, w: unused
    d: vec4<f32>,
}
@group(0) @binding(0) var lut_a: texture_2d<f32>;
@group(0) @binding(1) var lut_b: texture_2d<f32>;
@group(0) @binding(2) var lut_samp: sampler;
@group(0) @binding(3) var<uniform> dp: Deposit;

// Shared `saturation` (mirrors core/src/render/palette.rs::desaturate verbatim).
fn apply_saturation(col: vec3<f32>, s: f32) -> vec3<f32> {
    let luma = dot(col, vec3<f32>(0.299, 0.587, 0.114));
    return vec3<f32>(luma) + (col - vec3<f32>(luma)) * s;
}

// Shared `palette_steps` (mirrors core/src/render/palette.rs::band_coord).
fn band_coord(t: f32, steps: f32) -> f32 {
    if (steps < 1.5) {
        return t;
    }
    return (floor(t * steps) + 0.5) / steps;
}

// Shared `palette_contour` (ADR-0078 / ADR-0133; the WGSL is the implementation,
// copied verbatim at each fragment-stage site — palette.rs has no CPU
// counterpart to be canonical, since `fwidth` exists only here).
//
// Darkens within one PIXEL of a band edge, so the line has the same weight where
// the field is shallow and where it is steep — AND ONLY WHERE THE INK ACTUALLY
// CHANGES (ADR-0133). It samples the two band centres either side of the nearest
// edge and returns unchanged when they resolve to the same colour within half a
// code value, which is below the LUT's own 8-bit quantization. On a smooth
// palette two distinct centres always differ by at least one code value, so
// every edge draws exactly as it did at any `palette_steps`; inside a plateau
// the LUT is literally constant and the samples are bit-equal, so the line
// vanishes there and survives at the run boundaries. One rule, both behaviours,
// no new parameter.
//
// The two LUTs, the sampler and `palette_mix` are EXPLICIT parameters rather
// than module-scope globals this happens to find: all four sites name them the
// same today, so implicit capture would compile — and would silently bind the
// shared function to whatever a future site called its textures.
//
// `textureSampleLevel`, not `textureSample`: the LUT has one mip, and an
// explicit LOD keeps these reads free of the uniformity requirement that a
// sample after a conditional return would otherwise carry.
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

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let aspect = dp.a.x;
    // Frame-height units about the deposit centre — the target's aspect, never
    // the mesh grid's (ADR-0037), so the ring is round on any display.
    var p = in.uv - dp.b.xy;
    p.x = p.x * aspect;
    let r = length(p);
    // +y up, so a positive `deposit_spin` turns the arms the way it looks.
    let ang = atan2(-p.y, p.x);

    // A gaussian ring: `deposit_radius = 0` degenerates to a blob at the centre.
    let width = max(dp.a.w, 1e-3);
    let dr = (r - dp.a.z) / width;
    var g = exp(-0.5 * dr * dr);

    // Angular arms. Below half an arm the modulation is off entirely rather than
    // a degenerate single lobe.
    let arms = dp.b.z;
    if (arms >= 0.5) {
        let phase = arms * (ang + dp.b.w * r) + dp.c.x;
        g = g * (0.5 + 0.5 * cos(phase));
    }

    let amount = max(dp.a.y, 0.0) * g;

    // Palette by angle, so the deposit lays down colour the warp can drag into
    // structure rather than one flat tone.
    let coord = dp.c.y + dp.c.z * (ang / 6.2831853);
    let banded = band_coord(coord, dp.d.y);
    let ca = textureSample(lut_a, lut_samp, vec2<f32>(banded, 0.5)).rgb;
    let cb = textureSample(lut_b, lut_samp, vec2<f32>(banded, 0.5)).rgb;
    let mixed = mix(ca, cb, clamp(dp.d.x, 0.0, 1.0)) * band_contour(coord, dp.d.y, dp.d.z, lut_a, lut_b, lut_samp, dp.d.x);
    let col = apply_saturation(mixed, dp.c.w);

    // Additive light with saturating coverage (ADR-0056): premultiplied colour,
    // and an alpha equal to the coverage this fragment actually has.
    return vec4<f32>(col * amount, clamp(amount, 0.0, 1.0));
}
"#;

/// The filled-shape pass: a plain triangle list in the line renderer's own world
/// space, drawn into the field beside the line batch.
///
/// A second pipeline rather than a fan of wide line segments, because a custom
/// shape is a **filled** polygon with a centre-to-edge colour ramp and the line
/// primitive's across-the-stroke falloff would render it as a star. 63 % of the
/// corpus enables at least one custom shape, which is what buys the pipeline.
pub(super) const SHAPE_SHADER: &str = r#"
struct ShapeU {
    // x: aspect, yzw: unused
    v: vec4<f32>,
}
@group(0) @binding(0) var<uniform> su: ShapeU;

struct VsIn {
    @location(0) pos: vec2<f32>,
    @location(1) color: vec3<f32>,
    @location(2) alpha: f32,
}

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) color: vec3<f32>,
    @location(1) alpha: f32,
}

@vertex
fn vs_main(in: VsIn) -> VsOut {
    // World space, with x divided by the aspect — the same convention the shared
    // line renderer uses, so a shape and its own outline land on each other.
    let aspect = max(su.v.x, 0.1);
    var out: VsOut;
    out.pos = vec4<f32>(in.pos.x / aspect, in.pos.y, 0.0, 1.0);
    out.color = in.color;
    out.alpha = in.alpha;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    // Premultiplied light and its own coverage (ADR-0056): the colour arrives
    // already scaled by the alpha, and the alpha is what the fill holds out.
    return vec4<f32>(in.color, clamp(in.alpha, 0.0, 1.0));
}
"#;

/// The present pass: the field, over the backdrop.
pub(super) const PRESENT_SHADER: &str = r#"
struct Present {
    // x: brightness, y: occlude, z: gamma, w: unused
    a: vec4<f32>,
    // The four MilkDrop composite remaps, each 0 or 1:
    // x: brighten, y: darken, z: solarize, w: invert
    b: vec4<f32>,
    // The video echo: x: alpha, y: zoom, z: flip x (0/1), w: flip y (0/1).
    // The orientation arrives already decoded into two flags — see `render`.
    c: vec4<f32>,
}
@group(0) @binding(0) var<uniform> pp: Present;
@group(0) @binding(1) var field: texture_2d<f32>;
@group(0) @binding(2) var field_samp: sampler;

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    var c = textureSampleLevel(field, field_samp, in.uv, 0.0);

    // **The video echo**: MilkDrop's composite draws the finished frame twice,
    // the second copy zoomed about the centre and flipped, at `fVideoEchoAlpha`.
    // It samples the SAME texture this pass is already reading — the frame as it
    // stands after the warp, the deposit and the draw layer — so the echo is a
    // display-time composite and **does not accumulate**: the field the next
    // frame warps is untouched by it. That is the plan's open question, and it
    // is what makes the stage cheap: one extra sample, no second target.
    //
    // **The second copy is BLENDED TOWARD, not added** — ADR-0119. Plan 0109
    // Phase 3 summed it, on three supports, and the Phase 5 look gate took two
    // of them away. It read *Songflower (Moss Posy)* at the authored
    // `alpha = 1.000` against the same conversion at `0.000` and found the sum
    // turning a crisp two-axis lattice into a pale one: the preset drives
    // `echo_zoom` to ~1.75-2.0, so what was being added is a large soft
    // magnified duplicate. And both families of bars turned out to be present
    // with the echo OFF, which is the observation Plan 0108 said needed the
    // echo to explain. The third support, ADR-0056, is about the seam between
    // two *producers*; an echo is not a second producer, it is the same light
    // sampled twice.
    //
    // A `mix` cannot wash the picture out at any alpha, because a convex
    // combination is bounded by its inputs — which is the property a composite
    // of a frame with a copy of itself has to have. At `alpha = 1` the frame
    // becomes the transformed copy alone, and that is what makes this testable
    // exactly rather than by symmetry: the echo writes nothing back, so the
    // field evolves identically either way, and the echoed frame IS the mirror
    // of the un-echoed one frame for frame.
    //
    // The zoom and the flip are both about the frame centre, so they commute and
    // the order below is free. No aspect enters (ADR-0037): a uniform scale in
    // uv is a uniform scale of the picture whatever shape the target is, which
    // is exactly what MilkDrop's scaled quad does.
    if (pp.c.x > 0.0) {
        var euv = (in.uv - vec2<f32>(0.5)) / max(pp.c.y, 1e-3) + vec2<f32>(0.5);
        euv.x = select(euv.x, 1.0 - euv.x, pp.c.z > 0.5);
        euv.y = select(euv.y, 1.0 - euv.y, pp.c.w > 0.5);
        c = mix(c, textureSampleLevel(field, field_samp, euv, 0.0), pp.c.x);
    }
    // The branch is on a uniform and it is there for exactness rather than for
    // speed: `mix(c, echo, 0)` is `c` in arithmetic but not for a non-finite
    // `echo`, and an echo-less preset must render the bytes it rendered before
    // this stage existed.

    // The field already holds premultiplied colour and coverage (the deposit
    // writes them that way and the warp scales both together), so `brightness`
    // and `gamma` scale the light and `occlude` scales only how much backdrop is
    // held out (ADR-0085).
    var col = c.rgb * max(pp.a.x, 0.0) * max(pp.a.z, 0.0);

    // MilkDrop's four composite remaps, in its own order. Each is a flag in the
    // source format and stays one here, so a preset that binds none of them pays
    // four `select`s on a uniform branch and nothing else.
    //
    // **They operate on LINEAR light here and operated on 8-bit display-referred
    // pixels in MilkDrop** (ADR-0046 is the whole reason this plan is
    // interesting), so they are the same gesture rather than the same arithmetic:
    // `brighten`'s square root lifts the shadows either way, but by a different
    // amount. Stated rather than discovered.
    col = select(col, sqrt(max(col, vec3<f32>(0.0))), pp.b.x > 0.5);
    col = select(col, col * col, pp.b.y > 0.5);
    col = select(col, col * (vec3<f32>(1.0) - col) * 4.0, pp.b.z > 0.5);
    col = select(col, max(vec3<f32>(1.0) - col, vec3<f32>(0.0)), pp.b.w > 0.5);

    return vec4<f32>(col, clamp(c.a, 0.0, 1.0) * pp.a.y);
}
"#;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct WarpUniform {
    pub(super) misc: [f32; 4],
    pub(super) misc2: [f32; 4],
    pub(super) misc3: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct DepositUniform {
    pub(super) a: [f32; 4],
    pub(super) b: [f32; 4],
    pub(super) c: [f32; 4],
    pub(super) d: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct ShapeUniform {
    pub(super) v: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct PresentUniform {
    pub(super) a: [f32; 4],
    pub(super) b: [f32; 4],
    pub(super) c: [f32; 4],
}

/// One mesh vertex, as the warp pipeline reads it.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct Vertex {
    pub(super) clip: [f32; 2],
    pub(super) t0: [f32; 4],
    pub(super) t1: [f32; 4],
    pub(super) t2: [f32; 4],
}

/// [`Vertex`]'s attribute layout — one definition, because the built-in warp
/// pipeline and a converted preset's custom one (Plan 0100 Phase 6) must read
/// the same buffer, and two inline copies would drift exactly the way
/// `SegmentInstance::alpha`'s hazard note warns about.
pub(super) const VERTEX_ATTRS: [wgpu::VertexAttribute; 4] = wgpu::vertex_attr_array![
    0 => Float32x2,
    1 => Float32x4,
    2 => Float32x4,
    3 => Float32x4,
];
