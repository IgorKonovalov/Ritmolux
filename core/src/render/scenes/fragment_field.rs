//! Fragment-field scene: a fullscreen Shadertoy-style domain-warped field,
//! colored through the shared palette LUT (ADR-0021). The first "generative-art"-
//! tier built-in and one of the two preset-driven systems (ADR-0002 layers 1-2).
//!
//! Its look is a set of named parameters — `warp`, `hue`, `zoom`, `glow`,
//! `flash`, plus the shared color knobs `color_span`/`color_center`/`saturation`
//! (Plan 0020) — that a preset binds to expressions over the audio analysis (Plan
//! 0003 Phase 5). With no preset the parameter defaults render a gentle idle
//! field. The scene reads no audio directly; all reactivity flows through the
//! parameter values.
//!
//! Color: the field level indexes a 256-entry gradient LUT (the preset's
//! `[palette]`, default `spectrum` = the exact prior cosine) instead of a
//! hardcoded `palette()`. `color_span` sets how much of the gradient the field
//! spans (replacing the old fixed `field*0.6`), `color_center`/`hue` slide the
//! window, and `saturation` desaturates toward luma — all bindable.

// Hot-path panic-denial pragma (Plan 0002 Phase 2, extended to scenes by Plan
// 0003 Phase 0). Runs every displayed frame.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

use super::Scene;
use crate::dsp::AnalysisFrame;
use crate::render::palette::{self, Palette};

/// Parameter defaults — a calm idle field when nothing is bound.
const DEFAULT_WARP: f32 = 0.4;
const DEFAULT_HUE: f32 = 0.0;
const DEFAULT_ZOOM: f32 = 1.0;
const DEFAULT_GLOW: f32 = 0.7;
const DEFAULT_FLASH: f32 = 0.0;
// Shared view transform (ADR-0018): `pan_*` offset the sampled field window. The
// field's existing `zoom` already scales the sample coordinates (its view-zoom in
// field space), so Phase 2 completes the ViewTransform here by adding pan.
const DEFAULT_PAN: f32 = 0.0;
// Shared palette color knobs (ADR-0021). `color_span` = 0.6 + `color_center` = 0
// + `saturation` = 1 reproduce the prior look exactly (the old `field*0.6` sample
// with no desaturation).
const DEFAULT_COLOR_SPAN: f32 = 0.6;
const DEFAULT_COLOR_CENTER: f32 = 0.0;
const DEFAULT_SATURATION: f32 = 1.0;
/// `palette_mix` default — 0 = palette A only (a no-op unless a preset declares
/// `[palette_b]` and binds `palette_mix`).
const DEFAULT_PALETTE_MIX: f32 = 0.0;

const SHADER: &str = r#"
struct Params {
    // x: time (s), y: aspect, z: warp, w: hue
    a: vec4<f32>,
    // x: zoom, y: glow, z: flash, w: color_span
    b: vec4<f32>,
    // xy: pan (field-space offset, ADR-0018), z: color_center, w: saturation
    c: vec4<f32>,
    // x: palette_mix (A/B crossfade), yzw: unused
    d: vec4<f32>,
}

@group(0) @binding(0) var<uniform> params: Params;
// The gradient LUTs sit in their own bind group (group 1), so this pipeline's
// layout stays distinct from the screen-space kaleidoscope's single 3-entry
// [uniform, texture, sampler] group — two byte-identical layouts mis-render when
// they coexist on the DX12 WARP software adapter (the same quirk the shared line
// renderer and the lazy feedback scenes work around). Two LUTs (A/B) for the
// `palette_mix` crossfade; one shared sampler.
@group(1) @binding(0) var lut_a: texture_2d<f32>;
@group(1) @binding(1) var lut_b: texture_2d<f32>;
@group(1) @binding(2) var lut_samp: sampler;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) ndc: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VsOut {
    // Single oversized triangle covers the viewport (no vertex buffer).
    var pts = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0), vec2<f32>(3.0, -1.0), vec2<f32>(-1.0, 3.0),
    );
    var out: VsOut;
    out.pos = vec4<f32>(pts[vi], 0.0, 1.0);
    out.ndc = pts[vi];
    return out;
}

// Shared `saturation` (mirrors core/src/render/palette.rs::desaturate verbatim):
// scale chroma around Rec. 601 luma. 1.0 unchanged, 0.0 grayscale.
fn apply_saturation(c: vec3<f32>, s: f32) -> vec3<f32> {
    let luma = dot(c, vec3<f32>(0.299, 0.587, 0.114));
    return vec3<f32>(luma) + (c - vec3<f32>(luma)) * s;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let t = params.a.x;
    let aspect = params.a.y;
    let warp = params.a.z;
    let hue = params.a.w;
    let zoom = params.b.x;
    let glow = params.b.y;
    let flash = params.b.z;
    let color_span = params.b.w;
    let pan = params.c.xy;
    let color_center = params.c.z;
    let saturation = params.c.w;
    let palette_mix = params.d.x;

    var uv = in.ndc;
    uv.x = uv.x * aspect;

    // Iterated sine-fold domain warp, scaled by zoom and folded by warp; `pan`
    // slides the sampled field window (the shared ViewTransform, ADR-0018). The
    // vignette below stays screen-anchored (uses unshifted `uv`).
    var p = uv * zoom + pan;
    for (var i = 0; i < 5; i = i + 1) {
        let fi = f32(i);
        p = p + warp * vec2<f32>(
            sin(p.y * 1.5 + t * 0.7 + fi),
            cos(p.x * 1.5 - t * 0.6 + fi)
        ) / (fi + 1.0);
    }

    let field = 0.5 + 0.5 * sin(p.x + p.y + t * 0.5);
    // Field level indexes the gradient LUT: `color_span` sets the spanned range
    // (was a fixed 0.6), `color_center`/`hue` slide the window. Linear-filtered,
    // repeat-addressed (a hue rotation wraps like the cosine wheel).
    let coord = field * color_span + color_center + hue;
    // Sample both palettes and crossfade by `palette_mix` (0 = A, 1 = B). When a
    // preset declares no [palette_b] the two LUTs are identical, so mix is a no-op.
    let ca = textureSample(lut_a, lut_samp, vec2<f32>(coord, 0.5)).rgb;
    let cb = textureSample(lut_b, lut_samp, vec2<f32>(coord, 0.5)).rgb;
    var col = mix(ca, cb, clamp(palette_mix, 0.0, 1.0));
    col = apply_saturation(col, saturation);

    let r = length(uv);
    col = col * (glow * (1.0 - 0.25 * r));
    col = col + vec3<f32>(flash * 0.12);

    return vec4<f32>(col, 1.0);
}
"#;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Params {
    a: [f32; 4],
    b: [f32; 4],
    c: [f32; 4],
    d: [f32; 4],
}

/// Fullscreen domain-warped fragment field, driven by named preset parameters.
pub struct FragmentFieldScene {
    pipeline: wgpu::RenderPipeline,
    uniforms: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    /// The LUT textures + sampler bind group (group 1), kept distinct from the
    /// uniform group so the pipeline layout does not match the kaleidoscope's.
    lut_bind_group: wgpu::BindGroup,
    /// The 256×1 gradient LUT textures (A/B) the fragment samples + crossfades
    /// for color (ADR-0021).
    lut_texture_a: wgpu::Texture,
    lut_texture_b: wgpu::Texture,
    /// The active baked palette pair, re-uploaded to `lut_texture_a`/`_b` when
    /// `palette_dirty` (set by `set_palette` on a preset switch), off the hot path.
    palette: Palette,
    palette_dirty: bool,
    /// Shared scene clock (seconds), set by the renderer each frame.
    time: f32,
    warp: f32,
    hue: f32,
    zoom: f32,
    glow: f32,
    flash: f32,
    pan_x: f32,
    pan_y: f32,
    color_span: f32,
    color_center: f32,
    saturation: f32,
    /// A/B palette crossfade position (Plan 0020 Phase 4); 0 = palette A.
    palette_mix: f32,
}

impl FragmentFieldScene {
    /// Build the scene's pipeline and uniform buffer on `device`.
    pub fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("fragment-field-shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("fragment-field-params"),
            size: std::mem::size_of::<Params>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let lut_texture_a = palette::lut_texture(device, "fragment-field-lut-a");
        let lut_texture_b = palette::lut_texture(device, "fragment-field-lut-b");
        let lut_view_a = lut_texture_a.create_view(&wgpu::TextureViewDescriptor::default());
        let lut_view_b = lut_texture_b.create_view(&wgpu::TextureViewDescriptor::default());
        let lut_sampler = palette::lut_sampler(device);
        let uniform_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("fragment-field-uniform-layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        // The LUT texture + sampler live in their own group (group 1) — see the
        // WGSL note: keeping this pipeline's layout distinct from the
        // kaleidoscope's avoids the DX12 WARP identical-layout mis-render.
        let lut_texture_entry = |binding: u32| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let lut_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("fragment-field-lut-layout"),
            entries: &[
                lut_texture_entry(0),
                lut_texture_entry(1),
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("fragment-field-uniform-bg"),
            layout: &uniform_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniforms.as_entire_binding(),
            }],
        });
        let lut_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("fragment-field-lut-bg"),
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
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("fragment-field-pipeline-layout"),
            bind_group_layouts: &[Some(&uniform_layout), Some(&lut_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("fragment-field-pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        Self {
            pipeline,
            uniforms,
            bind_group,
            lut_bind_group,
            lut_texture_a,
            lut_texture_b,
            // Seed with the default `spectrum` (the prior cosine); the renderer
            // calls `set_palette` before the first frame with the active preset's
            // palette, and `render` uploads it. Seeding here keeps the texture
            // valid even if `set_palette` were never called.
            palette: Palette::default_spectrum(),
            palette_dirty: true,
            time: 0.0,
            warp: DEFAULT_WARP,
            hue: DEFAULT_HUE,
            zoom: DEFAULT_ZOOM,
            glow: DEFAULT_GLOW,
            flash: DEFAULT_FLASH,
            pan_x: DEFAULT_PAN,
            pan_y: DEFAULT_PAN,
            color_span: DEFAULT_COLOR_SPAN,
            color_center: DEFAULT_COLOR_CENTER,
            saturation: DEFAULT_SATURATION,
            palette_mix: DEFAULT_PALETTE_MIX,
        }
    }
}

/// The parameter names this scene consumes — the vocabulary a preset binding
/// is checked against at load, so a typo is warned about instead of silently
/// doing nothing (ADR-0020). **Keep in sync with `set_param` below**; the
/// `declared_params_match_set_param` guard in `core/tests/preset.rs` fails if
/// the two drift.
pub const PARAMS: &[&str] = &[
    "warp",
    "hue",
    "zoom",
    "glow",
    "flash",
    "pan_x",
    "pan_y",
    "color_span",
    "color_center",
    "saturation",
    "palette_mix",
];

impl Scene for FragmentFieldScene {
    fn name(&self) -> &'static str {
        "fragment field"
    }

    fn set_time(&mut self, time: f32) {
        self.time = time;
    }

    fn set_palette(&mut self, palette: &Palette) {
        // Store the baked LUT; `render` uploads it (deferred so scenes with lazy
        // GPU resources share this seam). Cheap array copy, off the hot path.
        self.palette = *palette;
        self.palette_dirty = true;
    }

    fn reset_params(&mut self) {
        self.warp = DEFAULT_WARP;
        self.hue = DEFAULT_HUE;
        self.zoom = DEFAULT_ZOOM;
        self.glow = DEFAULT_GLOW;
        self.flash = DEFAULT_FLASH;
        self.pan_x = DEFAULT_PAN;
        self.pan_y = DEFAULT_PAN;
        self.color_span = DEFAULT_COLOR_SPAN;
        self.color_center = DEFAULT_COLOR_CENTER;
        self.saturation = DEFAULT_SATURATION;
        self.palette_mix = DEFAULT_PALETTE_MIX;
    }

    fn set_param(&mut self, name: &str, value: f32) {
        match name {
            "warp" => self.warp = value,
            "hue" => self.hue = value,
            "zoom" => self.zoom = value,
            "glow" => self.glow = value,
            "flash" => self.flash = value,
            "pan_x" => self.pan_x = value,
            "pan_y" => self.pan_y = value,
            "color_span" => self.color_span = value,
            "color_center" => self.color_center = value,
            "saturation" => self.saturation = value,
            "palette_mix" => self.palette_mix = value,
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
        // Upload the active palette LUTs (A + B) if a preset switch changed them
        // (off the hot path — once per switch, not per frame).
        if self.palette_dirty {
            palette::write_lut(queue, &self.lut_texture_a, &self.palette.lut_a_bytes());
            palette::write_lut(queue, &self.lut_texture_b, &self.palette.lut_b_bytes());
            self.palette_dirty = false;
        }

        let params = Params {
            a: [self.time, aspect.max(0.1), self.warp, self.hue],
            b: [self.zoom, self.glow, self.flash, self.color_span],
            c: [self.pan_x, self.pan_y, self.color_center, self.saturation],
            d: [self.palette_mix, 0.0, 0.0, 0.0],
        };
        queue.write_buffer(&self.uniforms, 0, bytemuck::bytes_of(&params));

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("fragment-field-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    // Load over the engine backdrop (ADR-0018); this fullscreen
                    // field is opaque, so it covers the backdrop as before.
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
        pass.set_bind_group(1, &self.lut_bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}
