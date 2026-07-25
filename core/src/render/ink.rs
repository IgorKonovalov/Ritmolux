//! Final-stage duotone "ink" tone-remap (ADR-0028, Plan 0027 Phase 1): the last
//! stage of the ADR-0018 fixed composite. It reads the fully composited frame's
//! per-pixel luminance as an *ink density* `d` and outputs `mix(paper, ink, d)`
//! between two preset-configurable colors — so a bright additive stroke (density
//! high) becomes ink and the dark base (density low) becomes paper. That is a
//! *darkening* step the additive scene pipelines can't express, which is what
//! "black on white" needs: it is the inverse of the compositing model, not a
//! color choice reachable by any scene param.
//!
//! Default `paper = white`, `ink = black`, so turning the stage on with the
//! default colors is a pure black-on-white invert; colored poles (cream + indigo,
//! sepia, ...) are the same operation with different `paper_*`/`ink_*`. The stage
//! is driven by the `ink_*` named params the renderer routes here exactly as it
//! routes `bg_*`/`kaleido_*`, so there is **no `Scene`-trait change and the C ABI
//! is untouched**. It sits last (after the scene, trails, kaleidoscope, and — when
//! Plan 0024 lands — the transition blend), before the text/overlay passes, so the
//! HUD is never inverted.
//!
//! **Passthrough and unbuilt when `ink_amount <= 0`** — every shipped preset until
//! one opts in — so the renderer skips this stage entirely: no offscreen, no
//! pipeline, golden/determinism unchanged, the NFR §1 iGPU floor pays nothing,
//! and (like the background/trails/kaleidoscope passes) the DX12 WARP software
//! adapter never sees a coexisting remap pipeline during the no-ink captures. When
//! active the pipeline builds lazily and is dropped on the capture scene-rebuild.
//!
//! Unlike the trails/kaleidoscope stages (fixed 16:9 internal resolution), the ink
//! offscreen is **surface-sized** — the remap is a 1:1 per-pixel operation, so its
//! input must match the surface to avoid a resample blur on the final present. The
//! offscreen is rebuilt lazily whenever the surface size changes.

// Hot-path panic-denial pragma (Plan 0002 Phase 2; render/ is scanned by the
// hygiene guard). The remap pass encodes every displayed frame it is active.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

/// `ink_amount` default — 0 = off (passthrough), so an unbound preset is
/// unaffected and the stage is never built.
const DEFAULT_AMOUNT: f32 = 0.0;
/// Paper (density 0) defaults — a neutral true white (`sat = 0`, `bright = 1`).
const DEFAULT_PAPER_HUE: f32 = 0.0;
const DEFAULT_PAPER_SAT: f32 = 0.0;
const DEFAULT_PAPER_BRIGHT: f32 = 1.0;
/// Ink (density 1) defaults — a neutral true black (`sat = 0`, `bright = 0`).
const DEFAULT_INK_HUE: f32 = 0.0;
const DEFAULT_INK_SAT: f32 = 0.0;
const DEFAULT_INK_BRIGHT: f32 = 0.0;

const SHADER: &str = r#"
struct Ink {
    paper: vec4<f32>,  // x hue, y sat, z bright, w unused
    ink: vec4<f32>,    // x hue, y sat, z bright, w unused
    amount: vec4<f32>, // x amount, rest unused
}

@group(0) @binding(0) var<uniform> u: Ink;
@group(0) @binding(1) var t_src: texture_2d<f32>;
@group(0) @binding(2) var samp: sampler;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VsOut {
    var pts = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0), vec2<f32>(3.0, -1.0), vec2<f32>(-1.0, 3.0),
    );
    let p = pts[vi];
    var out: VsOut;
    out.pos = vec4<f32>(p, 0.0, 1.0);
    out.uv = vec2<f32>(0.5 * p.x + 0.5, 0.5 - 0.5 * p.y);
    return out;
}

// Standard HSV->RGB (iq form). `fract` normalizes an arbitrary hue into [0,1),
// so a bound `ink_hue` can sweep freely. No CPU color math per frame (ADR-0028).
fn hsv2rgb(c: vec3<f32>) -> vec3<f32> {
    let h = fract(c.x);
    let rgb = clamp(
        abs(((h * 6.0 + vec3<f32>(0.0, 4.0, 2.0)) % vec3<f32>(6.0)) - vec3<f32>(3.0)) - vec3<f32>(1.0),
        vec3<f32>(0.0),
        vec3<f32>(1.0),
    );
    return c.z * mix(vec3<f32>(1.0), rgb, c.y);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let src = textureSample(t_src, samp, in.uv).rgb;

    // Ink density = perceptual luminance (Rec. 709): a bright additive stroke
    // keys toward the ink pole, the dark base toward paper.
    let d = clamp(dot(src, vec3<f32>(0.2126, 0.7152, 0.0722)), 0.0, 1.0);

    let paper = hsv2rgb(u.paper.xyz);
    let ink = hsv2rgb(u.ink.xyz);
    let remapped = mix(paper, ink, d);

    // `amount` gates the remap against the untouched frame (0 = passthrough, 1 =
    // full remap), so a preset can breathe between glow and ink on the beat. The
    // stage is skipped entirely at amount <= 0, so this only blends for amount > 0.
    let amount = clamp(u.amount.x, 0.0, 1.0);
    return vec4<f32>(mix(src, remapped, amount), 1.0);
}
"#;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct InkUniform {
    paper: [f32; 4],
    ink: [f32; 4],
    amount: [f32; 4],
}

struct Resources {
    /// The surface-sized offscreen the composite (background + scene [+ post
    /// stages]) renders into. Kept alive so `src_view` stays valid; not read after
    /// construction.
    _src: wgpu::Texture,
    src_view: wgpu::TextureView,
    uniform: wgpu::Buffer,
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    /// The surface size these resources were built for; a change rebuilds them so
    /// the remap always samples a surface-resolution input.
    width: u32,
    height: u32,
}

impl Resources {
    fn build(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> Self {
        let src = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("ink-src"),
            size: wgpu::Extent3d {
                width: width.max(1),
                height: height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: surface_format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let src_view = src.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("ink-sampler"),
            // 1:1 same-size remap, so nearest is exact; clamp at the edges.
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ink-uniform"),
            size: std::mem::size_of::<InkUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("ink-shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let bind_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ink-bind-layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ink-bind-group"),
            layout: &bind_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&src_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("ink-pipeline-layout"),
            bind_group_layouts: &[Some(&bind_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("ink-pipeline"),
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
            _src: src,
            src_view,
            uniform,
            pipeline,
            bind_group,
            width: width.max(1),
            height: height.max(1),
        }
    }
}

/// The engine final-stage duotone tone-remap. Not a [`Scene`](super::scenes::Scene):
/// it is driven by the `ink_*` named params the renderer routes to it, and it
/// remaps the composited frame before present (ADR-0028).
pub struct Ink {
    device: wgpu::Device,
    surface_format: wgpu::TextureFormat,
    res: Option<Resources>,
    amount: f32,
    paper_hue: f32,
    paper_sat: f32,
    paper_bright: f32,
    ink_hue: f32,
    ink_sat: f32,
    ink_bright: f32,
}

impl Ink {
    /// Store the device/format for a lazy build; no GPU resources yet.
    pub fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Self {
        Self {
            device: device.clone(),
            surface_format,
            res: None,
            amount: DEFAULT_AMOUNT,
            paper_hue: DEFAULT_PAPER_HUE,
            paper_sat: DEFAULT_PAPER_SAT,
            paper_bright: DEFAULT_PAPER_BRIGHT,
            ink_hue: DEFAULT_INK_HUE,
            ink_sat: DEFAULT_INK_SAT,
            ink_bright: DEFAULT_INK_BRIGHT,
        }
    }

    /// Reset the remap params to their defaults (each frame, before routing).
    pub fn reset_params(&mut self) {
        self.amount = DEFAULT_AMOUNT;
        self.paper_hue = DEFAULT_PAPER_HUE;
        self.paper_sat = DEFAULT_PAPER_SAT;
        self.paper_bright = DEFAULT_PAPER_BRIGHT;
        self.ink_hue = DEFAULT_INK_HUE;
        self.ink_sat = DEFAULT_INK_SAT;
        self.ink_bright = DEFAULT_INK_BRIGHT;
    }

    /// Apply one named parameter, returning whether it was an `ink_*`/`paper_*`
    /// param. The renderer routes to the scene only when this returns `false`, so
    /// the namespaces never collide.
    pub fn set_param(&mut self, name: &str, value: f32) -> bool {
        match name {
            "ink_amount" => self.amount = value,
            "paper_hue" => self.paper_hue = value,
            "paper_sat" => self.paper_sat = value,
            "paper_bright" => self.paper_bright = value,
            "ink_hue" => self.ink_hue = value,
            "ink_sat" => self.ink_sat = value,
            "ink_bright" => self.ink_bright = value,
            _ => return false,
        }
        true
    }

    /// Drop the lazily-built resources — used on the capture scene-rebuild so a
    /// stale remap pipeline never lingers to mis-render the next capture's scene on
    /// the WARP adapter (module docs).
    pub fn reset_resources(&mut self) {
        self.res = None;
    }

    /// Whether the remap is active this frame (`ink_amount > 0`; at or below zero
    /// it is the identity passthrough and the stage is skipped).
    pub fn active(&self) -> bool {
        self.amount > 0.0 && self.amount.is_finite()
    }

    /// Build the surface-sized resources if needed (or rebuild on a size change)
    /// and return the offscreen view the composite renders into this frame. `None`
    /// only if the resources are absent (never, after the build). Called when
    /// [`active`](Self::active).
    pub fn begin(&mut self, width: u32, height: u32) -> Option<&wgpu::TextureView> {
        let stale = self
            .res
            .as_ref()
            .is_none_or(|res| res.width != width.max(1) || res.height != height.max(1));
        if stale {
            self.res = Some(Resources::build(
                &self.device,
                self.surface_format,
                width,
                height,
            ));
        }
        self.res.as_ref().map(|res| &res.src_view)
    }

    /// Remap the input offscreen into `surface_view`. Called after the composite
    /// has rendered into the [`begin`](Self::begin) target, when
    /// [`active`](Self::active).
    pub fn resolve(
        &mut self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        surface_view: &wgpu::TextureView,
    ) {
        let Some(res) = self.res.as_ref() else {
            return;
        };
        queue.write_buffer(
            &res.uniform,
            0,
            bytemuck::bytes_of(&InkUniform {
                paper: [self.paper_hue, self.paper_sat, self.paper_bright, 0.0],
                ink: [self.ink_hue, self.ink_sat, self.ink_bright, 0.0],
                amount: [self.amount, 0.0, 0.0, 0.0],
            }),
        );
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("ink-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: surface_view,
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
        pass.set_pipeline(&res.pipeline);
        pass.set_bind_group(0, &res.bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}
