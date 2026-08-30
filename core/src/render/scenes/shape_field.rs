//! **The mark roster, drawn at frame scale as a distance field**
//! (ADR-0105, Plan 0091).
//!
//! Every other scene here hands the palette a *level* — a noise field, a
//! chemical concentration, a particle's depth. This one hands it a **distance**,
//! and that single substitution is the whole scene:
//!
//! ```text
//! palette coordinate = mark_distance(p) * color_span + color_center
//! ```
//!
//! (that is `coord_mode`'s default; the second coordinate is below.)
//!
//! Because a band of the palette coordinate is now a band of constant distance,
//! turning `palette_steps` up produces **concentric offset contours of the
//! chosen shape** — not concentric circles, and not an outline sampled to
//! straight segments. It is per pixel, resolution-independent, and there is no
//! geometry to facet (which is the entire argument of ADR-0105 against routing
//! this through the line renderer). `palette_contour` then draws thin outlines
//! at those band boundaries, and this is the third scene that param does
//! anything in.
//!
//! # Two coordinates now, and the second is the one the references asked for
//! (ADR-0111, Plan 0098)
//!
//! An offset family is an **erosion**, and erosion rounds a reflex corner while
//! keeping convex ones sharp — so a nested heart keeps its bottom point and
//! loses its top notch as the contours move inward, and no amount of tuning
//! reaches the construction two batches of user reference images have asked for.
//! `coord_mode = "1"` hands the palette
//!
//! ```text
//! s = length(p) / r_boundary(theta)
//! ```
//!
//! instead — `0` at the centre and exactly `1` on the outline, the same contract,
//! but its level sets are **scaled copies** of the outline. The ring count is
//! then `palette_steps` alone and the innermost figure is a scaled copy at any
//! count, so notch sharpness stops trading against ring count.
//!
//! **The distance stays the default and stays bit-identical**, which is what
//! keeps every shipped preset and every golden baseline on the arithmetic it has
//! today. The two are not interchangeable settings of one knob: `color_span`
//! means a different thing under each, because the exterior is divided by the
//! shape's inradius under one and grows linearly in `r` under the other.
//!
//! # It shares the shape vocabulary rather than restating it
//!
//! The silhouettes come from [`marks`](super::marks) — the same WGSL chunk
//! `swarm` and `emitter` splice in, and the same CPU-side quantizers for the
//! `shape` selector and the `points` count. So a mark a particle can wear and a
//! figure this scene can be cannot drift apart, and the roster stays closed at
//! five names (ADR-0084's consequence, restated in ADR-0105).
//!
//! What *is* new is that this scene reads the field **outside** the silhouette,
//! where the particle path never looked. Plan 0091 Phase 2 measured that region
//! and repaired the two arms that were wrong out there; see `marks.rs`'s own
//! header for what it found and what it deliberately left approximate.
//!
//! # The aspect comes from the render target (ADR-0037)
//!
//! There is no internal grid here to take an aspect from by accident, which
//! removes the usual mechanism for that bug but not the obligation. The figure
//! is drawn in a square-unit space built by stretching NDC x by the **render
//! target's** aspect, so a disc is round at every window shape. `tests` renders
//! at 2:1 and 1:2 and measures the figure's own width against its height,
//! because both 1920x1080 and this box's 2048x1152 quantize to exactly 16:9 —
//! where no test can tell a right aspect source from a wrong one.

// Hot-path panic-denial pragma, as everywhere under `scenes/`.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

use crate::render::gpu;

use super::Scene;
use super::marks;
use crate::dsp::AnalysisFrame;
use crate::render::palette::{self, Palette};

/// `scale` default — the figure's outline sits at 0.6 of the frame's short
/// half-axis, which leaves room for several contour bands around it before they
/// leave the frame. The whole point of this scene is what happens *outside* the
/// silhouette, so a figure filling the frame would be the wrong default.
const DEFAULT_SCALE: f32 = 0.6;
/// Smallest `scale` the shader is handed. Not zero: at zero the figure has no
/// size and every pixel is infinitely far outside it in units of nothing, so
/// the coordinate degenerates rather than fading out.
const MIN_SCALE: f32 = 0.01;
/// Largest `scale`. Past this the figure is far outside the frame and the whole
/// screen is one interior band — reachable, but it is the end of the useful
/// range rather than an arbitrary cap.
const MAX_SCALE: f32 = 20.0;

/// Shared view transform (ADR-0018): `pan_*` moves the figure's centre.
const DEFAULT_PAN: f32 = 0.0;

/// `rotation` default — **0, and an exact arithmetic identity**: the shader
/// tests for it and skips the rotation entirely, so every shipped preset and
/// every golden baseline stays on the arithmetic it has today.
///
/// Radians, matching `lines/star.rs` and `lines/lsystem.rs` — the two other
/// figure-drawing scenes that carry this name. Unclamped for the same reason
/// they are: an angle wraps, so there is no end of the useful range to hold it
/// inside; a non-finite binding falls back to the identity because `cos(NaN)`
/// would take the whole frame with it.
const DEFAULT_ROTATION: f32 = 0.0;

/// `gamma` default — **the identity**, and it is exactly `1.0` on the way to the
/// uniform because the shader's identity branch tests for it (`pow(x, 1.0)` is
/// not bit-exact, ADR-0092's care).
const DEFAULT_GAMMA: f32 = 1.0;
/// The range `gamma` is held in. Same shape and the same reasoning as
/// `ink_gamma` and `bg_ramp_gamma`: positive on both sides, wide enough that the
/// clamp is the end of the useful range rather than a limit an author meets.
const MIN_GAMMA: f32 = 0.05;
const MAX_GAMMA: f32 = 20.0;

/// The `coord_mode` roster, in the order the numeric parameter selects them.
///
/// `0` hands the palette the normalized **distance** to the figure, whose level
/// sets are offset curves; `1` hands it `r / r_boundary(theta)`, whose level
/// sets are **scaled copies** of the outline
/// (ADR-0111). Both are `0` at the figure's centre and exactly `1` on its outline; what
/// differs is the shape of everything in between.
pub(crate) const COORD_MODES: [&str; 2] = ["distance", "radius"];

/// `coord_mode` default — **0, the distance**, and that is an obligation rather
/// than a preference: it is the arithmetic every shipped preset and every golden
/// baseline has today.
const DEFAULT_COORD_MODE: f32 = 0.0;
const MIN_COORD_MODE: f32 = 0.0;
const MAX_COORD_MODE: f32 = COORD_MODES.len() as f32 - 1.0;

/// Shared palette colour knobs (ADR-0021). `color_span = 0.6` puts the
/// silhouette's interior (`d` in `0..1`) across the gradient's first 60 %, so
/// the exterior contours have somewhere to go.
const DEFAULT_COLOR_SPAN: f32 = 0.6;
const DEFAULT_COLOR_CENTER: f32 = 0.0;
const DEFAULT_SATURATION: f32 = 1.0;
/// `palette_mix` default — 0 = palette A only.
const DEFAULT_PALETTE_MIX: f32 = 0.0;

const SHADER: &str = r#"
struct Params {
    // x: aspect (from the RENDER TARGET), y: shape index (quantized CPU-side),
    // z: points (quantized CPU-side), w: scale
    a: vec4<f32>,
    // xy: pan (the shared ViewTransform, ADR-0018), z: color_span,
    // w: color_center
    b: vec4<f32>,
    // x: saturation, y: palette_mix, z: palette_steps (integral, quantized
    // CPU-side), w: palette_contour
    c: vec4<f32>,
    // x: occlude (ADR-0085), y: gamma (the response exponent on the distance,
    // exactly 1.0 for the identity), z: coord_mode (quantized CPU-side; 0 = the
    // distance, 1 = the scaled-copy radius), w: rotation in radians, exactly 0.0
    // for the identity.
    d: vec4<f32>,
    // xyz: the star arm's shape params (valley, curve, jitter), conditioned
    // CPU-side. Inert on every other silhouette.
    e: vec4<f32>,
}

// **One bind group, sampler first and uniform last — and that arrangement is
// what buys this pipeline a layout shape nothing else holds** (ADR-0058: two
// byte-identical layouts alias on the DX12 WARP adapter, and the whole golden
// suite runs there, so a collision is blessed rather than caught).
//
// It is deliberately not `fragment_field`'s two-group split, because that split
// has no free shape left for a tenth scene. A lone uniform group can vary only
// by visibility and by whether it declares a `min_binding_size`, and all four
// combinations are taken: `[Uniform:FRAGMENT]` by the fragment field, the RD
// init and the test disc; `+size` by the backdrop; `VERTEX_FRAGMENT` by the
// line renderer; and `VERTEX_FRAGMENT+size` by the emitter. Merging the groups
// is what keeps this unique WITHOUT padding a layout with a binding the shader
// does not use, which is the cure ADR-0058's Alternative A refuses.
//
// Pick another free shape rather than tidying this back into two groups.
@group(0) @binding(0) var lut_samp: sampler;
@group(0) @binding(1) var lut_a: texture_2d<f32>;
@group(0) @binding(2) var lut_b: texture_2d<f32>;
@group(0) @binding(3) var<uniform> params: Params;

// Shared `saturation` (mirrors core/src/render/palette.rs::desaturate verbatim).
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
    let aspect = params.a.x;
    let shape = params.a.y;
    let points = params.a.z;
    let scale = params.a.w;
    let pan = params.b.xy;
    let color_span = params.b.z;
    let color_center = params.b.w;
    let saturation = params.c.x;
    let palette_mix = params.c.y;
    let palette_steps = params.c.z;
    let palette_contour = params.c.w;
    let gamma = params.d.y;
    let coord_mode = params.d.z;
    let rotation = params.d.w;
    let star = params.e.xyz;

    // Square units, from the RENDER TARGET's aspect (ADR-0037): stretching x
    // makes one unit of `uv` the same length on both axes, so the figure below
    // is the shape it claims to be and not the window's shape.
    var uv = in.ndc;
    uv.x = uv.x * aspect;

    // The figure's own frame: `pan` moves its centre, `scale` sets its size.
    //
    // **`rotation` is applied AFTER the pan, and that is a choice.** Turning the
    // sample point before subtracting `pan` would swing the figure around the
    // frame's centre — an orbit — and turning it after swings it about its own.
    // Both are defensible and they look completely different; this scene draws
    // ONE figure, and a figure that spins in place is what `rotation` means on
    // `lines/star.rs` and `lines/lsystem.rs` too.
    //
    // It is done in `uv`, which is already SQUARE units (ADR-0037): x has been
    // stretched by the render target's aspect, so one unit is the same length on
    // both axes and this is a rotation. In raw NDC the same two lines would
    // SHEAR — invisible at 16:9, where the stretch is nearly 1, and obvious at
    // 2:1. `tests` renders a square at 2:1 and turns it a quarter turn.
    //
    // A branch rather than an unconditional multiply, so 0 is an exact identity
    // and no shipped preset moves through `cos`/`sin` (ADR-0092's care, the same
    // reason `gamma` has one).
    var q = uv - pan;
    if (rotation != 0.0) {
        let cr = cos(rotation);
        let sr = sin(rotation);
        // The INVERSE rotation on the sample point, so a positive `rotation`
        // turns the figure counter-clockwise on screen rather than the frame.
        q = vec2<f32>(cr * q.x + sr * q.y, cr * q.y - sr * q.x);
    }
    let p = q / scale;

    // THE substitution this scene exists for: the palette coordinate is a
    // FIGURE coordinate rather than a level. Both modes are 0 at the figure's
    // centre and exactly 1 on its outline, and both grow outward — what differs
    // is what a band of the coordinate is a band OF.
    //
    // An `if` rather than a `select`, and that is not style: `select` evaluates
    // both arms, and the second arm here is a whole second shape evaluation. The
    // mode is a per-draw uniform, so this branch is uniform across a warp and
    // the hardware takes one arm rather than both.
    var d: f32;
    if (coord_mode < 0.5) {
        // Mode 0 — a band of the coordinate is a band of constant DISTANCE,
        // which is the definition of an offset curve (ADR-0105). This is the
        // default and it is bit-for-bit the arithmetic that shipped.
        d = mark_distance(p, shape, points, star);
    } else {
        // Mode 1 — a band of the coordinate is a band of constant SCALING, so
        // its level sets are scaled copies of the outline (ADR-0111). On a
        // polygon that keeps the corners the offsets round off; on a heart it
        // keeps the notch, which is the construction the reference images are.
        d = length(p) / max(mark_boundary_radius(p, shape, points, star), 1e-6);
    }
    // The response exponent, applied to the distance BEFORE it becomes a palette
    // coordinate — so it reshapes where the contours sit rather than which
    // colours they take. Above 1 the bands crowd toward the centre, which is what
    // the reference images do and what a raw (evenly spaced) distance cannot.
    // `select` rather than a branch, and the identity is exact: `pow(x, 1.0)` is
    // not bit-exact, so an unbound preset must not go through it (ADR-0092).
    let shaped = select(pow(d, gamma), d, gamma == 1.0);
    let coord = shaped * color_span + color_center;

    // Hard bands, then the contour drawn from the SAME coordinate (ADR-0078).
    let banded = band_coord(coord, palette_steps);
    let ca = textureSample(lut_a, lut_samp, vec2<f32>(banded, 0.5)).rgb;
    let cb = textureSample(lut_b, lut_samp, vec2<f32>(banded, 0.5)).rgb;
    var col = mix(ca, cb, clamp(palette_mix, 0.0, 1.0));
    col = col * band_contour(
        coord, palette_steps, palette_contour, lut_a, lut_b, lut_samp, palette_mix
    );
    col = apply_saturation(col, saturation);

    // Alpha: this field covers every pixel, which is the coverage it honestly
    // has (ADR-0056). `occlude` scales how much of that the backdrop underneath
    // resolves against (ADR-0085). Reached only when no post stage is active;
    // the chain owns the seam otherwise and the renderer hands a literal 1.0.
    return vec4<f32>(col, params.d.x);
}
"#;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Params {
    a: [f32; 4],
    b: [f32; 4],
    c: [f32; 4],
    d: [f32; 4],
    e: [f32; 4],
}

/// A fullscreen signed-distance figure from the shared mark roster, coloured
/// through the shared palette.
pub struct ShapeFieldScene {
    pipeline: wgpu::RenderPipeline,
    uniforms: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    /// The 256x1 gradient LUT pair (A/B) the fragment samples + crossfades for
    /// colour (ADR-0021), with the baked palette awaiting upload.
    luts: palette::LutPair,
    /// The silhouette and its point count, raw as the preset bound them —
    /// `marks::mark_shape` / `mark_points` quantize on the way to the uniform,
    /// which is where a selector's precondition belongs (the `kaleido_edge`
    /// precedent).
    shape: f32,
    points: f32,
    /// The `star` arm's three shape params, raw as the preset bound them
    /// (Plan 0091 Phase 5). `marks::star_*` condition them on the way to the
    /// uniform. Inert on every other silhouette, and nothing warns —
    /// `presets/README.md` carries that.
    star_valley: f32,
    star_curve: f32,
    star_jitter: f32,
    scale: f32,
    pan_x: f32,
    pan_y: f32,
    color_span: f32,
    color_center: f32,
    saturation: f32,
    palette_mix: f32,
    palette_steps: f32,
    palette_contour: f32,
    /// The response exponent on the distance, raw as the preset bound it;
    /// [`applied_gamma`] conditions it on the way to the uniform.
    gamma: f32,
    /// Which coordinate the palette is handed, raw as the preset bound it;
    /// [`applied_coord_mode`] quantizes it on the way to the uniform, which is
    /// where a selector's precondition belongs.
    coord_mode: f32,
    /// The figure's own turn, in radians, raw as the preset bound it. Applied
    /// about the figure's centre rather than the frame's — see the shader.
    rotation: f32,
    /// How much of this field's (total) coverage the backdrop resolves against
    /// (ADR-0085). Set by the renderer every frame — **not** a named param, so
    /// it is not reset by `reset_params`.
    occlude: f32,
}

impl ShapeFieldScene {
    /// Build the scene's pipeline and uniform buffer on `device`.
    pub fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Self {
        // The mark roster's chunk is prepended, exactly as the two particle
        // scenes do it: it declares no bindings and no entry points, so
        // splicing it in changes nothing about this pipeline's layout.
        let source = format!("{}{SHADER}", marks::sdf_wgsl());
        let shader = gpu::fullscreen_shader(
            device,
            "shape-field-shader",
            gpu::FULLSCREEN_VS_NDC,
            &source,
        );
        let uniforms =
            gpu::uniform_buffer(device, "shape-field-params", std::mem::size_of::<Params>());
        // Seeded with the default `spectrum`; the renderer calls `set_palette`
        // before the first frame and `render` uploads it, so the textures are
        // valid even if `set_palette` were never called.
        let luts = palette::LutPair::new(device, "shape-field");
        // One group, sampler first and uniform last — see the WGSL's note for
        // why this shape and not `fragment_field`'s two-group split. The uniform
        // entry is a full literal rather than `gpu::uniform` because that helper
        // passes `min_binding_size: None`, and declaring one is half of what
        // makes this shape unique.
        let bind_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("shape-field-bind-layout"),
            entries: &[
                gpu::sampler(0),
                gpu::texture(1, true),
                gpu::texture(2, true),
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(
                            std::mem::size_of::<Params>() as u64
                        ),
                    },
                    count: None,
                },
            ],
        });
        // This layout binds the sampler first and the two textures after it, so
        // the pair's role-ordered array is destructured into binding order here.
        let [lut_a, lut_b, lut_sampler] = luts.bind_entries(1, 2, 0);
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("shape-field-bind-group"),
            layout: &bind_layout,
            entries: &[
                lut_sampler,
                lut_a,
                lut_b,
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: uniforms.as_entire_binding(),
                },
            ],
        });
        let pipeline = gpu::fullscreen_pipeline(
            device,
            &shader,
            &[&bind_layout],
            surface_format,
            wgpu::BlendState::REPLACE,
            "shape-field",
        );

        Self {
            pipeline,
            uniforms,
            bind_group,
            luts,
            shape: marks::DEFAULT_SHAPE,
            points: marks::DEFAULT_POINTS,
            star_valley: marks::DEFAULT_STAR_VALLEY,
            star_curve: marks::DEFAULT_STAR_CURVE,
            star_jitter: marks::DEFAULT_STAR_JITTER,
            scale: DEFAULT_SCALE,
            pan_x: DEFAULT_PAN,
            pan_y: DEFAULT_PAN,
            color_span: DEFAULT_COLOR_SPAN,
            color_center: DEFAULT_COLOR_CENTER,
            saturation: DEFAULT_SATURATION,
            palette_mix: DEFAULT_PALETTE_MIX,
            palette_steps: palette::DEFAULT_PALETTE_STEPS,
            palette_contour: palette::DEFAULT_PALETTE_CONTOUR,
            gamma: DEFAULT_GAMMA,
            coord_mode: DEFAULT_COORD_MODE,
            rotation: DEFAULT_ROTATION,
            occlude: crate::render::post::DEFAULT_OCCLUDE,
        }
    }
}

/// The `scale` the shader is handed: held inside the range the arithmetic needs,
/// with a non-finite binding falling back to the default.
///
/// The clamp is CPU-side for the reason `background::applied_ramp_gamma` states:
/// it can never be reached with a NaN, where WGSL's `clamp` is
/// implementation-defined, and the default stays **exactly** the default on the
/// way to the uniform.
fn applied_scale(scale: f32) -> f32 {
    if scale.is_finite() {
        scale.clamp(MIN_SCALE, MAX_SCALE)
    } else {
        DEFAULT_SCALE
    }
}

/// The `rotation` the shader is handed: passed through, with a non-finite
/// binding falling back to the identity.
///
/// No clamp, because an angle wraps and there is no end of the range to hold it
/// inside — the same treatment `lines/star.rs` gives the name. The finiteness
/// guard is not decoration: `cos(NaN)` is `NaN`, and one `NaN` in the figure's
/// own frame takes every pixel of the frame with it.
fn applied_rotation(rotation: f32) -> f32 {
    if rotation.is_finite() {
        rotation
    } else {
        DEFAULT_ROTATION
    }
}

/// The `coord_mode` the shader is handed: clamped into the roster, then
/// **rounded to an integer**, with a non-finite binding falling back to the
/// default — and **forced back to the distance on a `ring`**.
///
/// The quantizing half is `marks::mark_shape`'s treatment for
/// `marks::mark_shape`'s reason, and the `kaleido_edge` precedent behind both. A
/// mode's values are **identities** rather than a quantity: `[smoothing]` and
/// preset dissolves interpolate a binding continuously from one setting to
/// another, so easing the distance to the radius passes through 0.4 and 0.6, and
/// there is nothing halfway between an offset curve and a scaled copy for the
/// shader to draw there.
///
/// # The `ring` fallback, and why it is not silent
///
/// An annulus's centre is in its hole, so `r / r_boundary` has no single value
/// there — the one behavioural choice ADR-0111 leaves open. Plan 0098
/// Phase 4 rendered the three defensible answers before picking, and what
/// settled it is that defining the boundary as the outer rim produces a figure
/// **byte-identical to a `disc`**: the coordinate collapses to `length(p)` and
/// the hole stops existing, so a preset naming one roster entry would be shown
/// another. That is the negative ADR-0111 records, reached in practice.
///
/// So the combination is refused rather than approximated, and the refusal is
/// **announced**: `Preset::from_toml_str` warns at load when a preset rests on
/// it (ADR-0020's shape, the `thickness` dead-zone precedent). The silent
/// fallback was the third candidate and it is the one this rejects — it renders
/// the same pixels as this does and costs an author the afternoon.
fn applied_coord_mode(mode: f32, shape: f32) -> f32 {
    if shape == marks::RING_SHAPE {
        return DEFAULT_COORD_MODE;
    }
    if mode.is_finite() {
        mode.clamp(MIN_COORD_MODE, MAX_COORD_MODE).round()
    } else {
        DEFAULT_COORD_MODE
    }
}

/// The exponent the shader will **actually apply** for a bound `gamma`: a
/// non-finite binding falls back to the identity, and a finite one is held
/// inside the positive range ([`MIN_GAMMA`], [`MAX_GAMMA`]).
///
/// CPU-side for `ink::applied_gamma`'s two reasons: `1.0` stays **exactly**
/// `1.0` on the way to the uniform, which is what the shader's identity branch
/// tests, and the clamp can never be reached with a NaN, where WGSL's `clamp` is
/// implementation-defined.
fn applied_gamma(gamma: f32) -> f32 {
    if gamma.is_finite() {
        gamma.clamp(MIN_GAMMA, MAX_GAMMA)
    } else {
        DEFAULT_GAMMA
    }
}

/// The palette coordinate this scene hands the LUT, as a CPU mirror of the
/// shader's two lines — so the exponent's properties are testable without a GPU
/// (the arrangement `ink::key` and `tonemap::map` both use).
#[cfg(test)]
pub(crate) fn coord(distance: f32, gamma: f32, color_span: f32, color_center: f32) -> f32 {
    let g = applied_gamma(gamma);
    let shaped = if g == 1.0 { distance } else { distance.powf(g) };
    shaped * color_span + color_center
}

/// The parameter names this scene consumes — the vocabulary a preset binding is
/// checked against at load (ADR-0020). **Keep in sync with `set_param` below**;
/// `declared_params_match_set_param` in `core/tests/preset.rs` fails if the two
/// drift.
pub const PARAMS: &[&str] = &[
    "shape",
    "points",
    "star_valley",
    "star_curve",
    "star_jitter",
    "scale",
    "pan_x",
    "pan_y",
    "color_span",
    "color_center",
    "saturation",
    "palette_mix",
    "palette_steps",
    "palette_contour",
    "gamma",
    "coord_mode",
    "rotation",
];

impl Scene for ShapeFieldScene {
    fn name(&self) -> &'static str {
        "shape field"
    }

    fn set_occlude(&mut self, occlude: f32) {
        self.occlude = occlude;
    }

    fn set_palette(&mut self, palette: &Palette) {
        self.luts.set(palette);
    }

    fn reset_params(&mut self) {
        self.shape = marks::DEFAULT_SHAPE;
        self.points = marks::DEFAULT_POINTS;
        self.star_valley = marks::DEFAULT_STAR_VALLEY;
        self.star_curve = marks::DEFAULT_STAR_CURVE;
        self.star_jitter = marks::DEFAULT_STAR_JITTER;
        self.scale = DEFAULT_SCALE;
        self.pan_x = DEFAULT_PAN;
        self.pan_y = DEFAULT_PAN;
        self.color_span = DEFAULT_COLOR_SPAN;
        self.color_center = DEFAULT_COLOR_CENTER;
        self.saturation = DEFAULT_SATURATION;
        self.palette_mix = DEFAULT_PALETTE_MIX;
        self.palette_steps = palette::DEFAULT_PALETTE_STEPS;
        self.palette_contour = palette::DEFAULT_PALETTE_CONTOUR;
        self.gamma = DEFAULT_GAMMA;
        self.coord_mode = DEFAULT_COORD_MODE;
        self.rotation = DEFAULT_ROTATION;
    }

    fn set_param(&mut self, name: &str, value: f32) {
        match name {
            "shape" => self.shape = value,
            "points" => self.points = value,
            "star_valley" => self.star_valley = value,
            "star_curve" => self.star_curve = value,
            "star_jitter" => self.star_jitter = value,
            "scale" => self.scale = value,
            "pan_x" => self.pan_x = value,
            "pan_y" => self.pan_y = value,
            "color_span" => self.color_span = value,
            "color_center" => self.color_center = value,
            "saturation" => self.saturation = value,
            "palette_mix" => self.palette_mix = value,
            "palette_steps" => self.palette_steps = value,
            "palette_contour" => self.palette_contour = value,
            "gamma" => self.gamma = value,
            "coord_mode" => self.coord_mode = value,
            "rotation" => self.rotation = value,
            _ => {}
        }
    }

    fn update(&mut self, _frame: &AnalysisFrame) {
        // Fully parameter-driven; the analysis reaches this scene only through
        // the preset expressions bound to its parameters.
    }

    fn render(
        &mut self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        aspect: f32,
    ) {
        // Quantized once, because `applied_coord_mode` has to see the same value
        // the shader will: the `ring` refusal is a fact about the SELECTED arm,
        // not about the raw binding.
        let shape = marks::mark_shape(self.shape);
        self.luts.flush(queue);

        let params = Params {
            // `aspect` is the argument the chain hands down for the target this
            // scene is drawing into — never a size this scene chose (ADR-0037).
            a: [
                aspect.max(0.1),
                shape,
                marks::mark_points(self.points),
                applied_scale(self.scale),
            ],
            b: [self.pan_x, self.pan_y, self.color_span, self.color_center],
            c: [
                self.saturation,
                self.palette_mix,
                palette::band_steps(self.palette_steps),
                palette::band_contour(self.palette_contour),
            ],
            d: [
                self.occlude,
                applied_gamma(self.gamma),
                applied_coord_mode(self.coord_mode, shape),
                applied_rotation(self.rotation),
            ],
            e: [
                marks::star_valley(self.star_valley),
                marks::star_curve(self.star_curve),
                marks::star_jitter(self.star_jitter),
                0.0,
            ],
        };
        queue.write_buffer(&self.uniforms, 0, bytemuck::bytes_of(&params));

        let mut pass = gpu::color_pass(encoder, "shape-field-pass", view, wgpu::LoadOp::Load);
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}

#[cfg(test)]
mod tests;
