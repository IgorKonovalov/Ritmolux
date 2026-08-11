//! The `over`-join blend pass (ADR-0090 / Plan 0076 Phase 3): where a preset's
//! second scene layer joins the composite **between the kaleidoscope and
//! bloom** — exempt from the resampling stages (its geometry is never smeared
//! by trails nor folded by the kaleidoscope), participating in the luminous
//! ones (it blooms and tonemaps with the frame), in linear light before the
//! tonemap (ADR-0046's ordering).
//!
//! # Two inputs, one junction
//!
//! Like the transition [`Blend`](super::transition), and for the same reason,
//! this is a **two-input sampled pass**, not a fixed-function composite:
//! `overlay` branches per channel on the *destination's* value, which no blend
//! factor can express, so the chain-side content must be sampled rather than
//! rendered over. That forces the junction's shape:
//!
//! - the **chain input** (`chain_input`) is where the pre-bloom composite
//!   lands — the scene draws straight into it when no earlier stage is active,
//!   and the last pre-bloom stage folds into it ([`Fold::Own`]) otherwise. It
//!   is sized to the **junction's output** (bloom's internal grid when bloom
//!   is active, the destination otherwise), so the main content takes exactly
//!   the resampling steps it would have taken with no layer — which is what
//!   keeps `mix = 0` pixel-identical to the layerless preset;
//! - the **layer input** (`layer_input`) is the layer scene's own offscreen,
//!   always **surface-sized**: the layer is rasterized crisp at the display's
//!   resolution and meets the chain at the junction, whatever grid the chain
//!   happens to run at;
//! - [`resolve`](LayerBlendPass::resolve) samples both and writes the blended
//!   result to the junction's output — bloom's input ([`Fold::Own`]) or the
//!   chain's destination ([`Fold::Over`], where the backdrop is underneath and
//!   `occlude` scales the emitted alpha exactly as every stage fold does,
//!   ADR-0085).
//!
//! The chain side is fetched with `textureLoad` (exact integer fetch — the two
//! are always the same size, and `mix = 0` must reproduce it bit-for-bit); the
//! layer side is sampled with a linear filter, since it may be minified from
//! surface resolution to a post grid.
//!
//! # The modes, in linear HDR light
//!
//! `add` / `screen` / `multiply` / `overlay`, selected by a uniform slot in
//! **one** pipeline — a selector like the feedback warp roster, deliberately,
//! because WARP's pipeline-count sensitivity is documented and four
//! permutations of one pass is exactly the shape that bites (Plan 0046's risk
//! note). All four operate **within the layer's premultiplied-alpha footprint**
//! (ADR-0056): the blended result is `mix(chain, mode(chain, layer), coverage)`
//! with `coverage = layer.a * mix`, so a darkening mode darkens only where the
//! layer has coverage, and `mix = 0` is the chain content exactly (a lerp at
//! `t = 0` is exact in floating point).
//!
//! The chain content is unbounded linear light, and the bounded-operand modes
//! are defined to respect that: `screen` lifts toward 1.0 in proportion to the
//! remaining headroom and never darkens an over-range channel; `multiply`
//! clamps the layer operand so it strictly darkens; `overlay`'s bright branch
//! grows with an over-range base rather than clipping it.
//!
//! # ADR-0058: this layout is deliberately not the transition blend's
//!
//! The bind-group layout here — uniform + two textures + a sampler — would be
//! shape-identical to `blend-bind-layout`, and the two are live together
//! whenever a dissolve crosses an `over`-join preset. WARP mis-renders
//! coexisting identical layouts (measured, Plan 0053), so this one is
//! separated by both established levers: a **wider visibility mask**
//! (`VERTEX_FRAGMENT`, the emitter fix's idiom) and an explicit
//! `min_binding_size` of its own 32-byte uniform against the transition
//! blend's 16.

// Hot-path panic-denial pragma (Plan 0002 Phase 2; render/ is scanned by the
// hygiene guard). This pass encodes on every frame of every over-join preset.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

use super::gpu;
use super::post::Fold;
use crate::preset::LayerBlend;

const SHADER: &str = r#"
struct LayerBlendU {
    // x = mix in [0,1], y = mode code, z = alpha scale (occlude at an Over
    // fold, 1.0 at an Own fold), w unused
    p: vec4<f32>,
    // reserved; the second vec4 exists so this uniform's size differs from the
    // transition blend's (ADR-0058 — see the module docs)
    q: vec4<f32>,
}

@group(0) @binding(0) var<uniform> u: LayerBlendU;
@group(0) @binding(1) var t_chain: texture_2d<f32>;
@group(0) @binding(2) var t_layer: texture_2d<f32>;
@group(0) @binding(3) var samp: sampler;

// One blend mode, on the chain's premultiplied HDR base and the layer's
// straight (unpremultiplied) colour. See the module docs for the HDR rules.
fn mode_result(base: vec3<f32>, s: vec3<f32>, straight: vec3<f32>, mode: u32) -> vec3<f32> {
    if (mode == 0u) {
        // add: linear-light addition — the engine's native idiom. Uses the
        // straight colour; the coverage lerp below turns it back into exactly
        // `base + layer_premultiplied * mix`.
        return base + straight;
    }
    if (mode == 2u) {
        // multiply: strictly darkening within the footprint.
        return base * s;
    }
    if (mode == 3u) {
        // overlay: multiply below mid-grey, screen above, per channel. An
        // over-range base lands in the bright branch, whose `1 + 2(b-1)(1-s)`
        // form grows with it rather than clipping.
        let dark = 2.0 * base * s;
        let bright = vec3<f32>(1.0) - 2.0 * (vec3<f32>(1.0) - base) * (vec3<f32>(1.0) - s);
        return select(bright, dark, base < vec3<f32>(0.5));
    }
    // screen (mode 1, and the fallback): lift toward 1.0 by the remaining
    // headroom; `max` keeps an over-range channel from being pulled down.
    return max(base, mix(base, vec3<f32>(1.0), s));
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    // The chain side is the same size as the output — exact integer fetch, so
    // `mix = 0` reproduces it bit-for-bit.
    let a = textureLoad(t_chain, vec2<i32>(in.pos.xy), 0);
    // The layer side is surface-sized and may be minified here.
    let b = textureSample(t_layer, samp, in.uv);

    let m = clamp(u.p.x, 0.0, 1.0);
    let mode = u32(u.p.y + 0.5);
    let alpha_scale = u.p.z;

    // The layer's premultiplied-alpha footprint bounds the blend (ADR-0056).
    let cov = clamp(b.a, 0.0, 1.0) * m;
    // Straight layer colour for the bounded modes; `add` uses the raw
    // premultiplied energy through the identity noted in `mode_result`.
    let straight = b.rgb / max(b.a, 1e-4);
    let s = clamp(straight, vec3<f32>(0.0), vec3<f32>(1.0));

    let blended = mix(a.rgb, mode_result(a.rgb, s, straight, mode), cov);
    // Coverage union: the layer occludes the backdrop within its own
    // footprint, exactly as its `under` twin would through the scene target.
    let alpha = a.a + cov * (1.0 - a.a);
    return vec4<f32>(blended, alpha * alpha_scale);
}
"#;

/// The mode codes the shader switches on — kept beside the WGSL they select.
fn mode_code(mode: LayerBlend) -> f32 {
    match mode {
        LayerBlend::Add => 0.0,
        LayerBlend::Screen => 1.0,
        LayerBlend::Multiply => 2.0,
        LayerBlend::Overlay => 3.0,
    }
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct LayerBlendUniform {
    p: [f32; 4],
    /// Reserved. Exists so this uniform is 32 bytes against the transition
    /// blend's 16 — half of the ADR-0058 layout separation (module docs).
    q: [f32; 4],
}

/// The pipeline and everything sized independently of the frame — built once on
/// the first over-join frame and kept, like the transition blend's.
struct Pipeline {
    uniform: wgpu::Buffer,
    sampler: wgpu::Sampler,
    bind_layout: wgpu::BindGroupLayout,
    pipeline: wgpu::RenderPipeline,
}

/// The two inputs and the bind group over them, rebuilt when either size moves.
struct Targets {
    /// The pre-bloom chain content — sized to the junction's output.
    _chain: wgpu::Texture,
    chain_view: wgpu::TextureView,
    /// The layer scene's own offscreen — surface-sized, always.
    _layer: wgpu::Texture,
    layer_view: wgpu::TextureView,
    bind_group: wgpu::BindGroup,
    chain_size: (u32, u32),
    layer_size: (u32, u32),
}

/// The `over`-join blend pass. GPU-lazy: a chain whose preset declares no
/// `over` layer never builds a pipeline or a texture here.
pub(crate) struct LayerBlendPass {
    device: wgpu::Device,
    format: wgpu::TextureFormat,
    pipeline: Option<Pipeline>,
    targets: Option<Targets>,
}

impl LayerBlendPass {
    pub(crate) fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        Self {
            device: device.clone(),
            format,
            pipeline: None,
            targets: None,
        }
    }

    /// Build (or rebuild, on a size change) the two inputs. `chain_size` is the
    /// junction output's size; `layer_size` is the surface's.
    fn ensure(&mut self, chain_size: (u32, u32), layer_size: (u32, u32)) -> Option<&Targets> {
        let chain_size = (chain_size.0.max(1), chain_size.1.max(1));
        let layer_size = (layer_size.0.max(1), layer_size.1.max(1));
        if self.pipeline.is_none() {
            self.pipeline = Some(Pipeline::build(&self.device, self.format));
        }
        let pipeline = self.pipeline.as_ref()?;
        let stale = self
            .targets
            .as_ref()
            .is_none_or(|t| t.chain_size != chain_size || t.layer_size != layer_size);
        if stale {
            self.targets = Some(Targets::build(
                &self.device,
                self.format,
                pipeline,
                chain_size,
                layer_size,
            ));
        }
        self.targets.as_ref()
    }

    /// The view the pre-bloom chain content lands in this frame. The caller
    /// clears it transparent (it persists between frames) and either renders
    /// the scene into it or folds the last pre-bloom stage into it.
    pub(crate) fn chain_input(
        &mut self,
        chain_size: (u32, u32),
        layer_size: (u32, u32),
    ) -> Option<wgpu::TextureView> {
        Some(self.ensure(chain_size, layer_size)?.chain_view.clone())
    }

    /// The view the layer scene renders into this frame — surface-sized, so
    /// the layer stays crisp whatever grid the chain runs at. The caller
    /// clears it transparent.
    pub(crate) fn layer_input(
        &mut self,
        chain_size: (u32, u32),
        layer_size: (u32, u32),
    ) -> Option<wgpu::TextureView> {
        Some(self.ensure(chain_size, layer_size)?.layer_view.clone())
    }

    /// Blend the held inputs into `out` — bloom's input ([`Fold::Own`]) or the
    /// chain's destination ([`Fold::Over`]). Returns the draw calls encoded.
    pub(crate) fn resolve(
        &mut self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        out: &wgpu::TextureView,
        mode: LayerBlend,
        mix: f32,
        fold: Fold,
    ) -> u32 {
        let (Some(pipeline), Some(targets)) = (self.pipeline.as_ref(), self.targets.as_ref())
        else {
            return 0;
        };
        let mix = if mix.is_finite() {
            mix.clamp(0.0, 1.0)
        } else {
            1.0
        };
        queue.write_buffer(
            &pipeline.uniform,
            0,
            bytemuck::bytes_of(&LayerBlendUniform {
                p: [mix, mode_code(mode), fold.alpha_scale(), 0.0],
                q: [0.0; 4],
            }),
        );
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("layer-blend-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: out,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: fold.load_op(),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&pipeline.pipeline);
        pass.set_bind_group(0, &targets.bind_group, &[]);
        pass.draw(0..3, 0..1);

        1
    }

    /// Drop the two inputs — at a preset switch away from an `over` join, so
    /// the full-frame textures live only while a preset actually uses them.
    pub(crate) fn release_targets(&mut self) {
        self.targets = None;
    }

    /// Drop everything, pipeline included — the capture rebuild path, matching
    /// the stages' `reset_resources`.
    pub(crate) fn reset_resources(&mut self) {
        self.targets = None;
        self.pipeline = None;
    }
}

impl Pipeline {
    fn build(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("layer-blend-sampler"),
            // The layer side may be minified (surface -> post grid); linear
            // keeps that resample smooth. The chain side never goes through
            // this sampler — it is fetched with `textureLoad`.
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("layer-blend-uniform"),
            size: std::mem::size_of::<LayerBlendUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let shader = gpu::fullscreen_shader(
            device,
            "layer-blend-shader",
            gpu::FULLSCREEN_VS_UV_FLIPPED,
            SHADER,
        );
        // **Deliberately not `gpu::uniform`, and deliberately not the
        // transition blend's descriptor** (ADR-0058, module docs): the wider
        // `VERTEX_FRAGMENT` mask and this uniform's own 32-byte
        // `min_binding_size` are what separate this layout from
        // `blend-bind-layout`, whose shape it would otherwise share while the
        // two are live together in a dissolve across an over-join preset.
        let bind_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("layer-blend-bind-layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: std::num::NonZeroU64::new(std::mem::size_of::<
                            LayerBlendUniform,
                        >()
                            as u64),
                    },
                    count: None,
                },
                gpu::texture(1, true),
                gpu::texture(2, true),
                gpu::sampler(3),
            ],
        });
        let pipeline = gpu::fullscreen_pipeline(
            device,
            &shader,
            &[&bind_layout],
            format,
            // The stage-fold convention (ADR-0055): over a transparent-cleared
            // Own target this reduces exactly to REPLACE; over the destination
            // it composites over the backdrop, with `occlude` already applied
            // to the emitted alpha in the shader.
            wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING,
            "layer-blend",
        );

        Self {
            uniform,
            sampler,
            bind_layout,
            pipeline,
        }
    }
}

impl Targets {
    fn build(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        pipeline: &Pipeline,
        chain_size: (u32, u32),
        layer_size: (u32, u32),
    ) -> Self {
        let make = |label: &str, (width, height): (u32, u32)| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            })
        };
        let chain = make("layer-blend-chain", chain_size);
        let layer = make("layer-blend-layer", layer_size);
        let chain_view = chain.create_view(&wgpu::TextureViewDescriptor::default());
        let layer_view = layer.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("layer-blend-bind-group"),
            layout: &pipeline.bind_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: pipeline.uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&chain_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&layer_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&pipeline.sampler),
                },
            ],
        });

        Self {
            _chain: chain,
            chain_view,
            _layer: layer,
            layer_view,
            bind_group,
            chain_size,
            layer_size,
        }
    }
}
