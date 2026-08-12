//! Background pre-pass (ADR-0018): fills the whole frame with an audio-tintable
//! gradient + vignette *before* the active scene draws, so every scene composites
//! over a shared backdrop instead of clearing its own near-black. This pass
//! **owns the destination's clear**; the scenes switched from `Clear` to `Load`
//! (Plan 0018 Phase 3), so a mid-composite pass never wipes what a prior stage
//! drew.
//!
//! # It paints the chain's *destination*, not the chain's input (ADR-0055)
//!
//! The backdrop used to render into the first active post stage's offscreen, which
//! put it **inside** the texture the chain folds — so the kaleidoscope folded
//! `bg_vignette`'s radial darkening into its wedges, and the fold's falloff had no
//! backdrop to land on and faded to black instead.
//!
//! It now paints the chain's destination and the chain composites *over* it with
//! premultiplied alpha. The backdrop is therefore never folded, never blurred, and
//! never accumulated into the trails feedback — it is the plate underneath.
//! [`PostChain::begin`](super::post::PostChain::begin) clears the chain's own input
//! to transparent in its place. When no stage is active the two views are the same
//! texture, so that path is unchanged.
//!
//! # It colours through the preset's palette (ADR-0086)
//!
//! This pass used to carry its own inline copy of the iq cosine — the third copy
//! of the constant ADR-0021 ([`super::palette`]) was written to de-duplicate — so
//! `[palette]`, `saturation` and `palette_mix` stopped at the scene and never
//! reached the sky. It now samples the **same baked LUT pair every other scene
//! samples**: `bg_hue` is a coordinate in the preset's own gradient (cyclic, like
//! `color_center` / `hue_center`, because the LUT sampler repeat-addresses `u`),
//! and the two shared colour modulations move the backdrop with the figure.
//!
//! `saturation` and `palette_mix` stay in the **scenes'** vocabularies — the
//! backdrop declares neither, and [`PARAMS`] is unchanged. The renderer fans one
//! binding out to both consumers ([`ParamRoute::SceneAndBackdrop`](super::ParamRoute)),
//! so an author writes `saturation` once and the whole frame answers.
//!
//! # It paints a *segment* of that palette, along one ramp axis (ADR-0094)
//!
//! `bg_hue` alone takes **one** point of the gradient and multiplies it by a
//! fixed vertical brightness tilt. `bg_hue_span` makes the coordinate travel
//! along a screen axis instead, so the smooth fade a horizon needs falls out of
//! the `[palette]`'s own stops — and each stop's `at` position *is* the horizon's
//! vertical placement. There is no second placement mechanism.
//!
//! The swept coordinate is **not clamped**: it keeps the repeat addressing every
//! other palette coordinate in this engine uses, because two shipped presets
//! already drive `bg_hue` outside `[0, 1]` and depend on the wrap.
//!
//! **There is one brightness ramp on the frame, not two.** The fixed
//! `mix(0.72, 1.0, ndc.y)` tilt this pass used to hardcode has *retired into*
//! that ramp as `bg_shade` / `bg_shade_end`'s defaults, on the same axis as the
//! colour sweep. Keeping the tilt and multiplying an authorable ramp on top would
//! have been the cheaper identity guarantee, and it is exactly what was rejected:
//! the tilt is welded to `+y` while the ramp can point anywhere, so any angled
//! backdrop would carry a second vertical gradient no param explains.
//!
//! **One exponent shapes the ramp, and it shapes the *position* rather than
//! either channel.** `bg_ramp_gamma` eases where things sit along the axis ahead
//! of both the palette coordinate and the shade mix, so colour and brightness
//! reach their midpoints at the same height and the two halves of one ramp
//! cannot disagree. It is not redundant with `[palette]` stop placement: that
//! palette is **shared with the scene** (ADR-0086/0090), so shaping the sky's
//! falloff through stop `at` positions would re-map the figure too — and it is
//! the only shape control the brightness ramp has at all, `mix` being a straight
//! line no stop can bend.
//!
//! Every ramp param defaults to an **arithmetic identity** with the picture that
//! shipped before it, not to an approximation of it — see each one's constant.
//!
//! # It paints one soft band over that ground (ADR-0095)
//!
//! A galaxy is unresolved starlight **behind** the stars, so it belongs in this
//! pre-pass rather than over the scene: the band is drawn *additively over the
//! ground and under everything else*. It is one gaussian swell across its own
//! axis — `bg_band_angle` names the direction **across** the band, sharing
//! `bg_angle`'s convention, so `0` runs it horizontally — centred at
//! `bg_band_pos` and reaching `1/e` exactly `bg_band_width` either side of that
//! centre. That is what the half-width *means*; it is not a full width and not a
//! hard edge.
//!
//! **And the centreline bows.** `bg_band_curve` displaces it by `curve * 4t(1-t)`
//! in the *along-band* coordinate `t`, a form that is zero at both ends and
//! exactly `1` in the middle — so the param is the bow's depth in across-axis
//! units, and `0` is exactly straight rather than nearly so. The straight band
//! ADR-0095 Alternative F would have shipped is still here; it just is not the
//! only shape.
//!
//! Every axis is the **same function** ([`axis_pos`] in the shader), called with
//! a direction. One copy rather than three, precisely so
//! [ADR-0037](../../../docs/adrs/0037-internal-grid-is-a-resolution-not-a-shape.md)'s
//! trap cannot be fixed in one axis and left in the other.
//!
//! **`bg_band_amount = 0` is an identity structurally, not arithmetically**: the
//! shader takes a `select` arm, so the pre-band expression is the *untaken*
//! branch and no shipped preset or baseline can move by a rounding step.
//!
//! **And the pass now builds for a band alone.** The build condition below
//! widened from `bg_bright > 0` to `bg_bright > 0 || bg_band_amount > 0`,
//! because the sky this was written for is nearly black away from the horizon —
//! without it a galaxy over an unlit ground would silently render nothing.
//!
//! Driven by named params (`bg_hue`, `bg_bright`, `bg_vignette`, `bg_hue_span`) the renderer
//! routes here before the scene's own bindings. At the defaults (`bg_bright = 0`)
//! the backdrop is black, so a preset that binds none renders exactly as before —
//! the migration is neutral until a preset opts into a backdrop.
//!
//! **When no backdrop is bound at all (`bg_bright <= 0` *and*
//! `bg_band_amount <= 0`) the pass is a plain black clear** — no gradient
//! pipeline is drawn, and the pipeline is not even built.
//! Two reasons: it is the NFR §1 passthrough win (an invisible black gradient
//! costs nothing), and — like the reaction-diffusion / attractor scenes' lazy
//! resources — it keeps a second fullscreen fragment pipeline off the device
//! during the headless no-bg captures, where the DX12 WARP software adapter would
//! otherwise mis-render the coexisting scene pipelines.
//!
//! **That second reason used to end "a documented quirk with no validation error"
//! — it is neither undocumented nor a quirk any more.** Plan 0053 Phase 3
//! reproduced it, isolated it against a control, and fixed it: the quirk was this
//! pass's `[Uniform:FRAGMENT]` bind-group layout colliding with the fullscreen
//! scenes' (ADR-0058), and the explicit `min_binding_size` in
//! [`Resources::build`] separates them. The laziness above is still worth having
//! for its own reason, but it is no longer load-bearing against a mis-render.
//! Real hardware was never affected, which is exactly what made it expensive: the
//! whole golden suite captures on WARP.
//!
//! The **fragment field** is the one scene that still draws opaquely over the
//! backdrop, so its bg params have no visible effect. Every other scene composites
//! over it: the *sparse* scenes (lines, swarm, attractor) reveal the gradient in
//! the space between strokes and points, and reaction-diffusion reveals it in the
//! field's voids (Plan 0025 / ADR-0026 switched both fullscreen/accumulating scenes
//! from an opaque present to an alpha-blend over the backdrop).

// Hot-path panic-denial pragma (Plan 0002 Phase 2, extended to render/ by the
// hygiene guard). Runs every displayed frame.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

use crate::render::gpu;
use crate::render::palette::{self, Palette};

/// Parameter defaults — a black backdrop when nothing is bound, so the composite
/// is byte-neutral against the pre-Phase-3 per-scene clears.
const DEFAULT_HUE: f32 = 0.0;
const DEFAULT_BRIGHT: f32 = 0.0;
const DEFAULT_VIGNETTE: f32 = 0.0;
/// The ramp's palette travel (ADR-0094). `0.0` sweeps nowhere, so the pass takes
/// one sample at `bg_hue` — today's picture, as an arithmetic identity rather
/// than an approximation of it.
const DEFAULT_HUE_SPAN: f32 = 0.0;
/// The ramp's direction, in radians, `0` = bottom-to-top — the axis the retired
/// tilt already used, and `launch_angle`'s zero-is-up convention. At `0` the
/// aspect term cancels exactly (see the shader), so the default is the identity
/// rather than an approximation of it.
const DEFAULT_ANGLE: f32 = 0.0;
/// The brightness ramp's two ends, on the same axis as the colour sweep. These
/// two numbers **are** the fixed `mix(0.72, 1.0, ·)` tilt the pass used to
/// hardcode: the shader still runs that instruction with those constants, so the
/// retirement costs no pixels. A preset that binds them can now point the
/// brightness the other way, which the tilt could never do.
const DEFAULT_SHADE: f32 = 0.72;
const DEFAULT_SHADE_END: f32 = 1.0;
/// The ramp's response exponent (ADR-0094, in ADR-0092's form). `1.0` is the
/// **exact** identity — the shader's `select` takes the unexponentiated arm — so
/// the ramp is bit-for-bit linear until a preset bends it.
const DEFAULT_RAMP_GAMMA: f32 = 1.0;
/// The exponent's guard rails, and the reasoning `ink.rs` states for the same
/// pair: `pow(0, 0)` is undefined and a negative exponent sends the ramp's start
/// to infinity, so a binding that sweeps out of range is clamped rather than
/// allowed to produce a NaN frame. Both ends are far outside anything a sky
/// wants — at `0.05` the ramp's midpoint sits within a pixel of its start, at
/// `20` within a pixel of its end.
const MIN_RAMP_GAMMA: f32 = 0.05;
const MAX_RAMP_GAMMA: f32 = 20.0;
/// The two shared colour modulations, at the same defaults every scene uses —
/// `saturation` unchanged, `palette_mix` fully on palette A.
const DEFAULT_SATURATION: f32 = 1.0;
const DEFAULT_PALETTE_MIX: f32 = 0.0;

/// The band's intensity (ADR-0095). At `0.0` the band term is not added at all —
/// the shader takes a `select` arm rather than multiplying by zero — so this
/// default is an identity with the pre-band picture structurally rather than by
/// arithmetic that a rounding step could perturb.
const DEFAULT_BAND_AMOUNT: f32 = 0.0;
/// The band's direction, in radians, naming the axis **across** the band. Shares
/// [`DEFAULT_ANGLE`]'s convention exactly — same `sin`/`cos` pair, same
/// `axis_pos` — so `0` runs the band horizontally.
const DEFAULT_BAND_ANGLE: f32 = 0.0;
/// The centreline's position along that across-axis, in the same normalized
/// `0..1` the ramp's `s` uses: `0.5` is the middle of the frame.
const DEFAULT_BAND_POS: f32 = 0.5;
/// The gaussian's **`1/e` half-width**, in those same units — the envelope
/// reaches `1/e` exactly this far either side of the centre.
const DEFAULT_BAND_WIDTH: f32 = 0.15;
/// The arc: how far the centreline bows, in across-axis units, at the middle of
/// the band. `0.0` is **exactly** straight rather than nearly so — the bow term
/// is `0.0` times a finite number — so the straight band the simpler design
/// would have shipped is still here, as the default (ADR-0095 Alternative F).
const DEFAULT_BAND_CURVE: f32 = 0.0;
/// The half-width's guard rails, and the reasoning [`applied_ramp_gamma`] states
/// for its pair: the shader divides by this, so zero is a division by zero and a
/// negative value is a mirrored band no author asked for. Both ends are far
/// outside anything a sky wants, and both are stated in what they *do* rather
/// than picked round:
///
/// - at `0.001` the half-width is about **one pixel** of a 1080-row frame
///   (`1/1080 = 0.000926`), so the band is already a hairline there;
/// - at `100` the envelope is within **0.0025 %** of `1` across the whole frame
///   (the farthest a pixel can sit from a centred band is 0.5, so `z <= 0.005`),
///   i.e. a flat wash with no band in it at all.
const MIN_BAND_WIDTH: f32 = 0.001;
const MAX_BAND_WIDTH: f32 = 100.0;

const SHADER: &str = r#"
struct Bg {
    // x: hue, y: bright, z: vignette, w: aspect (from the DESTINATION SURFACE)
    v: vec4<f32>,
    // x: palette_mix (A/B crossfade), y: saturation,
    // z: bg_ramp_gamma (CPU-clamped positive), w: bg_band_amount
    c: vec4<f32>,
    // The ramp (ADR-0094). x: bg_angle, y: bg_hue_span,
    // z: bg_shade, w: bg_shade_end
    g: vec4<f32>,
    // The band (ADR-0095). x: bg_band_angle, y: bg_band_pos,
    // z: bg_band_width (CPU-clamped positive), w: bg_band_curve
    b: vec4<f32>,
}

@group(0) @binding(0) var<uniform> u: Bg;

// The preset's baked gradient, in its own bind group (group 1) — the same shape
// and the same textures the shader-coloured scenes sample (ADR-0021/0086). Two
// LUTs (A/B) for the `palette_mix` crossfade; one shared sampler.
@group(1) @binding(0) var lut_a: texture_2d<f32>;
@group(1) @binding(1) var lut_b: texture_2d<f32>;
@group(1) @binding(2) var lut_samp: sampler;

// Shared `saturation` (mirrors core/src/render/palette.rs::desaturate verbatim):
// scale chroma around Rec. 601 luma. 1.0 unchanged, 0.0 grayscale.
fn apply_saturation(c: vec3<f32>, s: f32) -> vec3<f32> {
    let luma = dot(c, vec3<f32>(0.299, 0.587, 0.114));
    return vec3<f32>(luma) + (c - vec3<f32>(luma)) * s;
}

// The normalized position along a screen axis: 0 at the frame edge the axis
// starts from, 1 at the edge it ends on.
//
// `aspect` is the **destination surface's**, and the axis is measured in
// pixel-proportional coordinates so an authored angle is the angle seen on
// screen. The normalizing denominator is the axis's own extent across that
// rectangle, so the result still spans 0..1 corner to corner at any angle — and
// it lands inside [0, 1] *by construction*, since |ndc.x| and |ndc.y| are both
// at most 1.
//
// **Two axes call this** (ADR-0095): the ramp's, and the band's — which in turn
// has an across direction and an along one. It is one function rather than
// three copies precisely so ADR-0037's trap cannot be fixed in one and left in
// the others.
fn axis_pos(ndc: vec2<f32>, dir: vec2<f32>, aspect: f32) -> f32 {
    let q = vec2<f32>(ndc.x * aspect, ndc.y);
    return 0.5 + 0.5 * dot(q, dir) / (aspect * abs(dir.x) + abs(dir.y));
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let hue = u.v.x;
    let bright = u.v.y;
    let vig_amt = u.v.z;
    let palette_mix = u.c.x;
    let saturation = u.c.y;
    let aspect = u.v.w;
    let angle = u.g.x;
    let hue_span = u.g.y;
    let shade = u.g.z;
    let shade_end = u.g.w;
    let band_amount = u.c.w;
    let band_angle = u.b.x;
    let band_pos = u.b.y;
    let band_width = u.b.z;
    let band_curve = u.b.w;

    // The ramp axis (ADR-0094): a normalized position along it, 0 at the frame
    // edge the ramp starts from and 1 at the edge it ends on. `bg_angle` is in
    // radians and `0` runs bottom-to-top, matching `launch_angle`'s
    // zero-is-up convention.
    //
    // **At `angle = 0` the aspect term cancels exactly**: `sin(0)` is 0 and
    // `cos(0)` is 1, so `dot(q, d)` is `ndc.y` and the denominator is
    // `aspect * 0 + 1`. That is why this reduces to the pre-ramp expression bit
    // for bit — and why no default-angle test can tell a right aspect from a
    // wrong one, which is what `backdrop_ramp.rs`'s negative control exists for.
    let d = vec2<f32>(sin(angle), cos(angle));
    let s = clamp(axis_pos(in.ndc, d, aspect), 0.0, 1.0);

    // The ramp is linear in screen space and light is not, so one exponent shapes
    // *where* things sit along the axis. It is applied to the position, ahead of
    // both channels, so colour and brightness stay locked as ONE ramp — the same
    // reason the fixed tilt retired into the ramp rather than sitting beside it.
    // `g > 1` holds the ramp near its start before falling away (a hot band at
    // the horizon, then a long fade); `g < 1` drops fast and leaves a dim tail.
    //
    // The `g == 1.0` branch is a correctness requirement, not an optimization,
    // and it is the form `ink.rs:135` shipped at Plan 0078: `pow(x, 1.0)` is
    // `exp2(1.0 * log2(x))` and is NOT bit-exact, so without it the *default*
    // would perturb every backdrop-binding preset and every future golden by a
    // rounding step. `g` is clamped positive on the CPU
    // ([`applied_ramp_gamma`]), so `1.0` reaches the uniform exactly and `pow`
    // never sees the undefined `0^0`.
    let g = u.c.z;
    let e = select(pow(s, g), s, g == 1.0);

    // The brightness ramp, on the **same axis as the colour sweep**. This used to
    // be a hardcoded `mix(0.72, 1.0, ·)` — a fixed 28 % tilt welded to +y, which
    // is the wrong way round for a horizon and which no param explained. Those
    // two constants are now `bg_shade` / `bg_shade_end`'s defaults, so this is
    // the identical instruction with the identical constants until a preset says
    // otherwise (ADR-0094 Alternative E: the tilt retires *into* the ramp rather
    // than sitting beside it, so an angled backdrop cannot carry a second,
    // invisible vertical gradient).
    let grad = mix(shade, shade_end, e);
    // A radial vignette that darkens the corners. Stays radial and independent of
    // the ramp — nothing here makes it directional.
    let r = length(in.ndc);
    let vig = 1.0 - vig_amt * clamp(r * r, 0.0, 1.0);

    // `bg_hue` is a *coordinate* in the preset's gradient, not an offset into a
    // private cosine. Linear-filtered and repeat-addressed, so it wraps past the
    // gradient's edge exactly as `color_center` / `hue_center` do.
    //
    // With `bg_hue_span` the pass paints a **segment** of that gradient along the
    // ramp rather than one point of it: `bg_hue` is the coordinate at the ramp's
    // start and the sweep travels `bg_hue_span` from there. The coordinate is not
    // clamped — it keeps the engine-wide repeat addressing, so a segment leaving
    // [0, 1] wraps (ADR-0094 Alternative D: two shipped presets depend on it).
    // At the `0.0` default this is `hue + 0.0 * e`, which is `hue` exactly.
    let coord = hue + hue_span * e;
    let ca = textureSample(lut_a, lut_samp, vec2<f32>(coord, 0.5)).rgb;
    let cb = textureSample(lut_b, lut_samp, vec2<f32>(coord, 0.5)).rgb;
    var tint = mix(ca, cb, clamp(palette_mix, 0.0, 1.0));
    tint = apply_saturation(tint, saturation);

    let ground = tint * bright * grad * vig;

    // The band (ADR-0095): one gaussian swell across its own axis, **added over
    // the ground**, which is what luminous unresolved starlight is. It takes its
    // across position from the same `axis_pos` the ramp does, so `bg_band_angle`
    // names the direction across the band exactly as `bg_angle` names the ramp's.
    //
    // `bg_band_width` is the **`1/e` half-width**: at `|across - centre|` equal
    // to it the exponent is exactly `-1`. It is CPU-clamped strictly positive
    // ([`applied_band_width`]), so this divide can be neither by zero nor by a
    // negative.
    let bd = vec2<f32>(sin(band_angle), cos(band_angle));
    let band_across = clamp(axis_pos(in.ndc, bd, aspect), 0.0, 1.0);

    // The **along-band** axis, perpendicular to the across one: rotating
    // `(sin a, cos a)` a quarter turn gives `(cos a, -sin a)`, so at
    // `bg_band_angle = 0` the band runs left-to-right and `t` travels with it.
    //
    // **Deliberately not clamped.** With the right normalizer `t` is inside
    // [0, 1] by construction, so a clamp could only ever *hide* a wrong one:
    // push `t` out of range and `4t(1-t)` goes negative, bowing the band the
    // wrong way at the frame edges. That is what `backdrop_ramp.rs`'s non-square
    // bow measurement catches, and it was verified to bite — dropping the aspect
    // from this denominator alone moves the edge columns 1.1 and 2.5 rows and
    // shears the arc.
    //
    // **The aspect's *value* nonetheless cancels here at the default angle**,
    // exactly as it does on the ramp: at `bg_band_angle = 0` the along direction
    // is horizontal, so the denominator is `aspect` while the numerator carries
    // `ndc.x * aspect`. The plan expected otherwise. It costs nothing — every
    // axis reads the one `u.v.w` the pass is handed, so ADR-0037's real trap
    // (taking that from the chain's internal grid) is one measurement for all
    // three, and `the_ramp_angle_takes_the_surfaces_aspect_not_the_internal_grids`
    // is it.
    let be = vec2<f32>(cos(band_angle), -sin(band_angle));
    let t = axis_pos(in.ndc, be, aspect);

    // The arc. `4t(1-t)` is zero at both ends and exactly `1` in the middle, so
    // `bg_band_curve` is *the bow's depth in across-axis units* — and `0` is
    // exactly straight on every pixel, not approximately so: the whole term is
    // `0.0 * (a finite number)`, which is `0.0`.
    let centre = band_pos + band_curve * 4.0 * t * (1.0 - t);
    let z = (band_across - centre) / band_width;
    let env = exp(-z * z);

    // `select(f, t, cond)` — so at `bg_band_amount <= 0` the value taken is the
    // untouched ground and the whole band expression is the *untaken* branch.
    // That is why the default is an identity structurally rather than
    // arithmetically: nothing here can perturb a shipped backdrop by a rounding
    // step (ADR-0095).
    let col = select(ground + tint * env * band_amount, ground, band_amount <= 0.0);
    return vec4<f32>(col, 1.0);
}
"#;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Bg {
    v: [f32; 4],
    c: [f32; 4],
    g: [f32; 4],
    b: [f32; 4],
}

/// The gradient pipeline, its uniform and its LUT pair, built lazily on the first
/// frame that actually paints a backdrop (see the module docs on the WARP quirk).
struct Resources {
    pipeline: wgpu::RenderPipeline,
    uniforms: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    /// The gradient LUTs + sampler (group 1), kept out of the uniform group so
    /// this pipeline's layout does not match a single-uniform pass's.
    lut_bind_group: wgpu::BindGroup,
    /// The 256×1 gradient LUT textures (A/B) the fragment samples at `bg_hue`.
    lut_texture_a: wgpu::Texture,
    lut_texture_b: wgpu::Texture,
}

impl Resources {
    fn build(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Self {
        let shader =
            gpu::fullscreen_shader(device, "background-shader", gpu::FULLSCREEN_VS_NDC, SHADER);
        let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("background-params"),
            size: std::mem::size_of::<Bg>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        // **The explicit `min_binding_size` is the fix for a measured WARP
        // mis-render, not tidiness (Plan 0053 Phase 3, ADR-0058). Do not drop it
        // back to `gpu::uniform`.**
        //
        // A bare `[Uniform:FRAGMENT]` here is byte-identical to
        // `fragment-field-uniform-layout` and `rd-init-layout`, and on the DX12
        // WARP software adapter that collision **renders the wrong picture**.
        // Measured at 160x100 against the same frame on the hardware adapter:
        //
        // | configuration              | hardware (mean RGB)     | WARP, bare      |
        // |----------------------------|-------------------------|-----------------|
        // | fragment field + backdrop  | 131.010 170.559 141.381 | 142.712 x3      |
        // | reaction-diffusion + bdrop | 087.612 165.165 156.168 | 087.543 064.538 |
        //
        // The fragment field came back a **flat grey** — every channel the same
        // number, its colour gone. The control is what makes this the layout
        // rather than the adapter: with `bg_bright = 0` the gradient pipeline is
        // never built (see the module docs), and there the two adapters agree to
        // **0.02 of one 8-bit level**. Adding this one field moves WARP onto the
        // hardware numbers to the same 0.02, with no shader change and no
        // visibility change.
        //
        // The size is genuinely known, so the field is honest on its own terms —
        // the same argument `scenes/emitter.rs` makes for the same fix. This is
        // the measurement that **isolates** it: the emitter changed the
        // visibility mask and the size together, so which one did the work was
        // never established. Here the size alone is sufficient.
        let bind_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("background-bind-layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: std::num::NonZeroU64::new(std::mem::size_of::<Bg>() as u64),
                },
                count: None,
            }],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("background-bind-group"),
            layout: &bind_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniforms.as_entire_binding(),
            }],
        });
        let lut_texture_a = palette::lut_texture(device, "background-lut-a");
        let lut_texture_b = palette::lut_texture(device, "background-lut-b");
        let lut_view_a = lut_texture_a.create_view(&wgpu::TextureViewDescriptor::default());
        let lut_view_b = lut_texture_b.create_view(&wgpu::TextureViewDescriptor::default());
        let lut_sampler = palette::lut_sampler(device);
        // Shape-identical to `fragment-field-lut-layout` — the ADR-0058
        // configuration where the DX12 WARP software adapter can hand a pass
        // another live pipeline's resources. **The evidence that clears the pair
        // is in the `Background` doc comment below**, per that ADR's rule that an
        // entry with no recorded measurement is not an entry.
        let lut_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("background-lut-layout"),
            entries: &[
                gpu::texture(0, true),
                gpu::texture(1, true),
                gpu::sampler(2),
            ],
        });
        let lut_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("background-lut-bind-group"),
            layout: &lut_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&lut_view_a),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&lut_view_b),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&lut_sampler),
                },
            ],
        });
        // Opaque: the backdrop establishes the frame the scene loads over.
        let pipeline = gpu::fullscreen_pipeline(
            device,
            &shader,
            &[&bind_layout, &lut_layout],
            surface_format,
            wgpu::BlendState::REPLACE,
            "background",
        );

        Self {
            pipeline,
            uniforms,
            bind_group,
            lut_bind_group,
            lut_texture_a,
            lut_texture_b,
        }
    }
}

/// The engine-owned background pass. Not a [`Scene`](super::scenes::Scene), and
/// deliberately **not** a [`PostStage`](super::post::PostStage) either: it is a
/// *pre*-pass that owns the frame clear and never folds a rendered frame down, so
/// the renderer drives it directly — routing `bg_*` params to it and painting it
/// into whatever target the chain chose — ahead of the scene and the chain
/// (ADR-0018, ADR-0031). Its GPU pipeline is built lazily on the first frame that
/// paints a visible backdrop.
///
/// # ADR-0058 evidence: `background-lut-layout` vs `fragment-field-lut-layout`
///
/// Adding the LUT group gave this pass a `[Texture, Texture, Sampler]` layout, and
/// the enumeration in `tonemap/tests.rs` shows exactly one other layout with that
/// shape — the fragment field's. ADR-0058 requires the pair be **measured** on
/// both adapters rather than assumed safe. Measured 2026-08-08 on this repo's dev
/// box (DX12: WARP vs the hardware adapter), 64x64, `bg_bright = 0.55`,
/// `bg_vignette = 0.35`, means over the whole frame, against the same probes run
/// on the parent commit:
///
/// - **The pair is not live in any shipped configuration.** The two layouts can
///   only coexist on a fragment-field preset that also lights a backdrop, and no
///   shipped fragment-field preset binds `bg_bright` — the fragment field draws
///   opaquely over the backdrop, so a lit one there is invisible by construction.
///   No golden fixture binds both either.
/// - **On hardware, the fragment-field-plus-lit-backdrop probes are byte-identical
///   before and after this change** (`208.724 138.047 72.030` and
///   `158.394 100.674 50.482`, both commits). Nothing aliased: an opaque scene
///   over a re-coloured sky is the same picture.
/// - **On WARP that configuration was already wrong before this change**, and for
///   an unrelated, documented reason — the fullscreen-scene + background-pipeline
///   coexistence in the module docs above. The parent commit renders those two
///   probes at `11.817 11.716 11.663` and `0.000 0.000 0.000` against hardware's
///   values above. It is a different flavour of wrong afterwards; it was not
///   *made* wrong here.
///
///   **That "unrelated reason" has since been identified, and it was not
///   unrelated** (Plan 0053 Phase 3). It was the *other* ADR-0058 collision this
///   pass was in — its single-uniform layout against the fullscreen scenes' —
///   and the explicit `min_binding_size` in [`Resources::build`] fixes it. A
///   fragment field over a lit backdrop now renders `130.989 170.538 141.359` on
///   WARP against `131.010 170.559 141.381` on hardware. So the bullet below
///   still stands and this one is discharged: the LUT pair was live and
///   colliding throughout, and fixing only the uniform group made the frame
///   correct — which is a stronger statement of "this pair does not alias" than
///   the original measurement could make.
/// - **Every probe whose scene is not the fragment field agrees between the two
///   adapters to under 0.15 of one 8-bit level**, before and after — swarm with no
///   palette, with a flat palette, desaturated, and over `ember` with trails.
///
/// So the new pair adds no observed aliasing, and the one configuration that could
/// exhibit it is unreachable from shipped content and already excluded from the
/// software adapter. If a fragment-field preset ever binds `bg_bright`, this is the
/// measurement to re-run.
pub struct Background {
    device: wgpu::Device,
    surface_format: wgpu::TextureFormat,
    /// Gradient pipeline, built lazily (module docs: WARP + passthrough).
    res: Option<Resources>,
    hue: f32,
    bright: f32,
    vignette: f32,
    angle: f32,
    hue_span: f32,
    shade: f32,
    shade_end: f32,
    ramp_gamma: f32,
    band_amount: f32,
    band_angle: f32,
    band_pos: f32,
    band_width: f32,
    band_curve: f32,
    /// The active preset's baked palette pair, re-uploaded to the LUT textures
    /// when `palette_dirty` (a preset switch, or a lazy rebuild), off the hot
    /// path. Held here rather than in [`Resources`] so a backdrop that has not
    /// been painted yet still remembers which gradient it belongs to.
    palette: Palette,
    palette_dirty: bool,
    /// The two shared colour modulations, fanned out from the scene's binding.
    saturation: f32,
    palette_mix: f32,
}

/// The parameter names this compositing stage consumes. Unlike a scene's
/// vocabulary these are **global** — every preset may bind them whatever its
/// system — so the loader's typo check unions them in (ADR-0020).
/// **Keep in sync with `set_param` below**; the
/// `declared_params_match_set_param` guard in `core/tests/preset.rs` fails if
/// the two drift.
pub const PARAMS: &[&str] = &[
    "bg_hue",
    "bg_bright",
    "bg_vignette",
    "bg_angle",
    "bg_hue_span",
    "bg_shade",
    "bg_shade_end",
    "bg_ramp_gamma",
    "bg_band_amount",
    "bg_band_angle",
    "bg_band_pos",
    "bg_band_width",
    "bg_band_curve",
];

/// The exponent the shader will **actually apply** for a bound `bg_ramp_gamma`:
/// a non-finite binding falls back to the identity, and a finite one is held
/// inside the positive range the arithmetic needs ([`MIN_RAMP_GAMMA`],
/// [`MAX_RAMP_GAMMA`]).
///
/// The guard lives on the CPU rather than in the shader for the two reasons
/// [`ink::applied_gamma`](super::ink) states: `1.0` stays **exactly** `1.0` on
/// the way to the uniform, which is what the shader's identity branch tests, and
/// the clamp can never be reached with a NaN, where WGSL's `clamp` is
/// implementation-defined.
fn applied_ramp_gamma(gamma: f32) -> f32 {
    if gamma.is_finite() {
        gamma.clamp(MIN_RAMP_GAMMA, MAX_RAMP_GAMMA)
    } else {
        DEFAULT_RAMP_GAMMA
    }
}

/// The half-width the shader will **actually apply** for a bound
/// `bg_band_width`, guarded exactly as [`applied_ramp_gamma`] is and for the
/// same two reasons: the shader divides by it, and a NaN must never reach
/// WGSL's `clamp`, where the result is implementation-defined.
fn applied_band_width(width: f32) -> f32 {
    if width.is_finite() {
        width.clamp(MIN_BAND_WIDTH, MAX_BAND_WIDTH)
    } else {
        DEFAULT_BAND_WIDTH
    }
}

/// The two colour modulations the backdrop **shares with the scene** rather than
/// owning (ADR-0086).
///
/// Deliberately *not* part of [`PARAMS`]: every system already declares both, and
/// claiming them here would take them off the scene — the routing is a fan-out,
/// not a transfer. [`resolve_route`](super::resolve_route) reads this list to
/// decide which of a system's own names also reach the sky, and
/// [`set_shared_colour_param`](Background::set_shared_colour_param) is the arm
/// that applies them.
pub const SHARED_COLOUR_PARAMS: &[&str] = &["saturation", "palette_mix"];

impl Background {
    /// Store the device/format for a lazy pipeline build; no GPU resources yet.
    pub fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Self {
        Self {
            device: device.clone(),
            surface_format,
            res: None,
            hue: DEFAULT_HUE,
            bright: DEFAULT_BRIGHT,
            vignette: DEFAULT_VIGNETTE,
            angle: DEFAULT_ANGLE,
            hue_span: DEFAULT_HUE_SPAN,
            shade: DEFAULT_SHADE,
            shade_end: DEFAULT_SHADE_END,
            ramp_gamma: DEFAULT_RAMP_GAMMA,
            band_amount: DEFAULT_BAND_AMOUNT,
            band_angle: DEFAULT_BAND_ANGLE,
            band_pos: DEFAULT_BAND_POS,
            band_width: DEFAULT_BAND_WIDTH,
            band_curve: DEFAULT_BAND_CURVE,
            // Seeded with the default `spectrum` (the cosine this pass used to
            // inline), so a backdrop painted before any `set_palette` call is the
            // colour it always was rather than black.
            palette: Palette::default_spectrum(),
            palette_dirty: true,
            saturation: DEFAULT_SATURATION,
            palette_mix: DEFAULT_PALETTE_MIX,
        }
    }

    /// Take the preset's baked gradient (ADR-0086). Called once per preset switch
    /// alongside the scene's own [`set_palette`](super::scenes::Scene::set_palette),
    /// with the same baked pair — one bake, both consumers, no drift. The upload
    /// is deferred to the next painted frame, so a preset with no backdrop never
    /// touches the queue.
    pub fn set_palette(&mut self, palette: &Palette) {
        self.palette = palette.clone();
        self.palette_dirty = true;
    }

    /// Drop the lazily-built gradient pipeline so the next backdrop rebuilds it.
    /// Called when the renderer rebuilds its scenes for a capture (Plan 0013): a
    /// capture stays a pure function of its inputs, and — on the WARP software
    /// adapter — a bg preset's pipeline never lingers to mis-render the *next*
    /// capture's scene (module docs).
    pub fn reset_resources(&mut self) {
        self.res = None;
    }

    /// Reset every background param to its default (called each frame before the
    /// active preset's bindings are routed, so unbound params don't leak).
    pub fn reset_params(&mut self) {
        self.hue = DEFAULT_HUE;
        self.bright = DEFAULT_BRIGHT;
        self.vignette = DEFAULT_VIGNETTE;
        self.angle = DEFAULT_ANGLE;
        self.hue_span = DEFAULT_HUE_SPAN;
        self.shade = DEFAULT_SHADE;
        self.shade_end = DEFAULT_SHADE_END;
        self.ramp_gamma = DEFAULT_RAMP_GAMMA;
        self.band_amount = DEFAULT_BAND_AMOUNT;
        self.band_angle = DEFAULT_BAND_ANGLE;
        self.band_pos = DEFAULT_BAND_POS;
        self.band_width = DEFAULT_BAND_WIDTH;
        self.band_curve = DEFAULT_BAND_CURVE;
        self.saturation = DEFAULT_SATURATION;
        self.palette_mix = DEFAULT_PALETTE_MIX;
    }

    /// Apply one named parameter, returning whether it was a background param
    /// (`bg_*`). Offered first, ahead of the post chain; the renderer falls
    /// through to the scene only when neither claims the name, so the background,
    /// post-stage and scene namespaces never collide.
    pub fn set_param(&mut self, name: &str, value: f32) -> bool {
        match name {
            "bg_hue" => self.hue = value,
            "bg_bright" => self.bright = value,
            "bg_vignette" => self.vignette = value,
            "bg_angle" => self.angle = value,
            "bg_hue_span" => self.hue_span = value,
            "bg_shade" => self.shade = value,
            "bg_shade_end" => self.shade_end = value,
            "bg_ramp_gamma" => self.ramp_gamma = value,
            "bg_band_amount" => self.band_amount = value,
            "bg_band_angle" => self.band_angle = value,
            "bg_band_pos" => self.band_pos = value,
            "bg_band_width" => self.band_width = value,
            "bg_band_curve" => self.band_curve = value,
            _ => return false,
        }
        true
    }

    /// Apply one of the [`SHARED_COLOUR_PARAMS`]. Unlike [`set_param`](Self::set_param)
    /// this claims nothing — the scene receives the same value from the same
    /// binding, and an unrecognized name is simply ignored, because the caller
    /// resolved the route at load and only calls this for the two names above.
    pub fn set_shared_colour_param(&mut self, name: &str, value: f32) {
        match name {
            "saturation" => self.saturation = value,
            "palette_mix" => self.palette_mix = value,
            _ => {}
        }
    }

    /// Own the frame clear — the first pass of the composite. With no visible
    /// backdrop at all (`bg_bright <= 0` *and* `bg_band_amount <= 0`) this is a
    /// plain black clear (no pipeline); with either, it lazily builds the
    /// gradient pipeline and paints it fullscreen.
    ///
    /// # `surface` is the **destination's** size, never an internal grid's
    ///
    /// This pass paints `destination`, which `composite_into` sizes from the
    /// surface. The `target.size` sitting on the next line there is the post
    /// chain's quantized, capped internal grid — a *resolution, not a shape*
    /// ([ADR-0037](../../../docs/adrs/0037-internal-grid-is-a-resolution-not-a-shape.md)),
    /// and at a 160x100 surface it is a square 256x256. Taking it would be that
    /// defect for the third time, and it is invisible at `bg_angle = 0` where the
    /// aspect term provably cancels — which is why `backdrop_ramp.rs` carries a
    /// negative control at a non-zero angle *with a stage active*, where the two
    /// sources disagree.
    pub fn render(
        &mut self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        surface: (u32, u32),
    ) {
        // **A band alone is enough to build this pass** (ADR-0095). The condition
        // used to read `bright <= 0` alone, which would have made a galaxy over a
        // near-black sky render nothing at all — and that is the configuration
        // the reference photograph actually is, not an edge case.
        if self.bright <= 0.0 && self.band_amount <= 0.0 {
            // Passthrough: a plain black clear establishes the frame without a
            // second fullscreen pipeline (module docs: NFR §1 + WARP).
            encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("background-clear"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            return;
        }

        // A rebuild hands back empty LUT textures, so the upload below has to run
        // again whether or not the palette itself changed.
        let fresh = self.res.is_none();
        let res = self
            .res
            .get_or_insert_with(|| Resources::build(&self.device, self.surface_format));
        if fresh || self.palette_dirty {
            palette::write_lut(queue, &res.lut_texture_a, &self.palette.lut_a_bytes());
            palette::write_lut(queue, &res.lut_texture_b, &self.palette.lut_b_bytes());
            self.palette_dirty = false;
        }
        let aspect = surface.0.max(1) as f32 / surface.1.max(1) as f32;
        queue.write_buffer(
            &res.uniforms,
            0,
            bytemuck::bytes_of(&Bg {
                v: [self.hue, self.bright, self.vignette, aspect],
                c: [
                    self.palette_mix,
                    self.saturation,
                    applied_ramp_gamma(self.ramp_gamma),
                    self.band_amount,
                ],
                g: [self.angle, self.hue_span, self.shade, self.shade_end],
                b: [
                    self.band_angle,
                    self.band_pos,
                    applied_band_width(self.band_width),
                    self.band_curve,
                ],
            }),
        );
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("background-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    // The backdrop owns the clear: establish the frame here so no
                    // scene needs to (ADR-0018).
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&res.pipeline);
        pass.set_bind_group(0, &res.bind_group, &[]);
        pass.set_bind_group(1, &res.lut_bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}
