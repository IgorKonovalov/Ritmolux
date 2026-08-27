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
//! Because a band of the palette coordinate is now a band of constant distance,
//! turning `palette_steps` up produces **concentric offset contours of the
//! chosen shape** — not concentric circles, and not an outline sampled to
//! straight segments. It is per pixel, resolution-independent, and there is no
//! geometry to facet (which is the entire argument of ADR-0105 against routing
//! this through the line renderer). `palette_contour` then draws thin outlines
//! at those band boundaries, and this is the third scene that param does
//! anything in.
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

/// `gamma` default — **the identity**, and it is exactly `1.0` on the way to the
/// uniform because the shader's identity branch tests for it (`pow(x, 1.0)` is
/// not bit-exact, ADR-0092's care).
const DEFAULT_GAMMA: f32 = 1.0;
/// The range `gamma` is held in. Same shape and the same reasoning as
/// `ink_gamma` and `bg_ramp_gamma`: positive on both sides, wide enough that the
/// clamp is the end of the useful range rather than a limit an author meets.
const MIN_GAMMA: f32 = 0.05;
const MAX_GAMMA: f32 = 20.0;

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
    // exactly 1.0 for the identity), zw: reserved.
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
    let star = params.e.xyz;

    // Square units, from the RENDER TARGET's aspect (ADR-0037): stretching x
    // makes one unit of `uv` the same length on both axes, so the figure below
    // is the shape it claims to be and not the window's shape.
    var uv = in.ndc;
    uv.x = uv.x * aspect;

    // The figure's own frame: `pan` moves its centre, `scale` sets its size.
    let p = (uv - pan) / scale;

    // THE substitution this scene exists for: the palette coordinate is a
    // DISTANCE. `mark_distance` is 0 at the figure's deepest interior point,
    // exactly 1 on its outline, and grows outward — so a band of coordinate is
    // a band of constant distance, which is the definition of an offset curve.
    let d = mark_distance(p, shape, points, star);
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
    lut_texture_a: wgpu::Texture,
    lut_texture_b: wgpu::Texture,
    palette: Palette,
    palette_dirty: bool,
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
        let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("shape-field-params"),
            size: std::mem::size_of::<Params>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let lut_texture_a = palette::lut_texture(device, "shape-field-lut-a");
        let lut_texture_b = palette::lut_texture(device, "shape-field-lut-b");
        let lut_view_a = lut_texture_a.create_view(&wgpu::TextureViewDescriptor::default());
        let lut_view_b = lut_texture_b.create_view(&wgpu::TextureViewDescriptor::default());
        let lut_sampler = palette::lut_sampler(device);
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
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("shape-field-bind-group"),
            layout: &bind_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Sampler(&lut_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&lut_view_a),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&lut_view_b),
                },
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
            lut_texture_a,
            lut_texture_b,
            palette: Palette::default_spectrum(),
            palette_dirty: true,
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
];

impl Scene for ShapeFieldScene {
    fn name(&self) -> &'static str {
        "shape field"
    }

    fn set_occlude(&mut self, occlude: f32) {
        self.occlude = occlude;
    }

    fn set_palette(&mut self, palette: &Palette) {
        self.palette = palette.clone();
        self.palette_dirty = true;
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
        if self.palette_dirty {
            palette::write_lut(queue, &self.lut_texture_a, &self.palette.lut_a_bytes());
            palette::write_lut(queue, &self.lut_texture_b, &self.palette.lut_b_bytes());
            self.palette_dirty = false;
        }

        let params = Params {
            // `aspect` is the argument the chain hands down for the target this
            // scene is drawing into — never a size this scene chose (ADR-0037).
            a: [
                aspect.max(0.1),
                marks::mark_shape(self.shape),
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
            d: [self.occlude, applied_gamma(self.gamma), 0.0, 0.0],
            e: [
                marks::star_valley(self.star_valley),
                marks::star_curve(self.star_curve),
                marks::star_jitter(self.star_jitter),
                0.0,
            ],
        };
        queue.write_buffer(&self.uniforms, 0, bytemuck::bytes_of(&params));

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("shape-field-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}

#[cfg(test)]
mod tests;
