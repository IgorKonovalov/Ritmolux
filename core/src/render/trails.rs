//! Feedback trails (ADR-0018 composite stage 3, Plan 0018 Phase 6): route the
//! composited frame (background + scene) through a fade-and-accumulate feedback
//! so moving shapes leave light trails. Reuses Plan 0014's
//! [`PingPongField`](super::feedback::PingPongField) for the accumulation — no
//! second feedback mechanism.
//!
//! The blend is a **max-decay**: `accum = max(current, fade * previous)`. The
//! current frame shows at full brightness while past frames fade at `fade` per
//! frame, so a bright additive stroke leaves a crisp head and a fading tail; a
//! static (dark) backdrop is stable (its own max), so nothing blows up. `fade`
//! comes from the `trails` named param (0 = off).
//!
//! **Off by default (passthrough).** When `trails <= 0` — every shipped preset
//! until one opts in — the [`PostChain`](super::post::PostChain) skips this stage
//! entirely: no offscreen
//! target, no pipelines, so golden/determinism are unchanged, the NFR §1 iGPU
//! floor pays nothing, and (like the background pass) the DX12 WARP software
//! adapter never sees a coexisting feedback pipeline during the no-trails
//! captures. When active, the pipelines build lazily and the accumulation is
//! **reset on the capture scene-rebuild**, so a headless capture stays a pure
//! function of its inputs (NFR §6).
//!
//! The `surface_format` this stage is built with is the **composite's** format
//! (`Rgba16Float`, ADR-0046), not the surface's: the max-decay already ran in
//! linear light — an 8-bit *sRGB* target blends linearly — so the arithmetic is
//! unchanged, but a bright head no longer clips at 1.0 on its way into the
//! accumulation, which was always `Rgba16Float` anyway.
//!
//! The composite runs at an internal resolution that **follows the render target**
//! (ADR-0034), quantized to a 256 px step and capped — see
//! [`internal_grid_size`](super::post::internal_grid_size). It used to be a fixed
//! 1280x720, which on anything above 720p upscaled the whole frame: line geometry
//! rasterized at full resolution was thrown away and came back soft, and the
//! preset lane's only recourse was to drop `trails` from every line preset to get
//! its sharpness back. Following the target is what lets those presets keep both.

// Hot-path panic-denial pragma (Plan 0002 Phase 2; render/ is scanned by the
// hygiene guard). The trails stage encodes its passes every displayed frame.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

use super::feedback::PingPongField;
use super::gpu;
use super::post::{Fold, PostStage, internal_grid_size};

/// `trails` param default — off, so an unbound preset pays nothing.
const DEFAULT_TRAILS: f32 = 0.0;

/// Hard ceiling on the decay factor: `1.0` would never fade (an ever-brightening
/// smear), so keep it strictly below.
const MAX_FADE: f32 = 0.98;

/// Fade-and-accumulate body. `VsOut`/`vs_main` come from
/// [`gpu::FULLSCREEN_VS_UV_FLIPPED`] — this pass samples what another pass
/// rendered, so it needs the Y flip.
const TRAILS_SHADER: &str = r#"
struct Fade { v: vec4<f32> } // x: fade factor

@group(0) @binding(0) var<uniform> u: Fade;
@group(0) @binding(1) var t_composited: texture_2d<f32>;
@group(0) @binding(2) var t_accum: texture_2d<f32>;
@group(0) @binding(3) var samp: sampler;

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let cur = textureSample(t_composited, samp, in.uv);
    let prev = textureSample(t_accum, samp, in.uv);
    // Max-decay: the current frame at full brightness, the past fading by `fade`.
    //
    // On ALL FOUR channels (ADR-0055). Alpha is coverage and it decays on the same
    // schedule as colour, so a trail releases the backdrop at the rate it dims.
    // Forcing alpha to 1 here held `bg_*` out of every pixel the trail had ever
    // touched, permanently.
    return max(cur, prev * u.v.x);
}
"#;

/// Present body; same Y-flipped prelude as [`TRAILS_SHADER`].
const PRESENT_SHADER: &str = r#"
@group(0) @binding(0) var t_accum: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    // Alpha travels with the colour (ADR-0055) — the accumulation is premultiplied
    // and the chain composites over the backdrop downstream.
    return textureSample(t_accum, samp, in.uv);
}
"#;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Fade {
    v: [f32; 4],
}

/// The trails GPU resources, built lazily on the first active frame and rebuilt
/// only when the internal grid changes size (ADR-0030's compare-first rule).
struct Resources {
    /// The grid these were built for, so [`Trails::begin`] can compare before
    /// rebuilding rather than reallocating every frame.
    size: (u32, u32),
    // The offscreen the composite (background + scene) renders into each frame.
    // Kept alive so `composited_view` stays valid; not read after construction.
    _composited: wgpu::Texture,
    composited_view: wgpu::TextureView,
    accum: PingPongField,
    fade_uniform: wgpu::Buffer,
    trails_pipeline: wgpu::RenderPipeline,
    // One bind group per accumulation read-side (composited + accum read + fade + sampler).
    trails_bg_a: wgpu::BindGroup,
    trails_bg_b: wgpu::BindGroup,
    present_pipeline: wgpu::RenderPipeline,
    present_bg_a: wgpu::BindGroup,
    present_bg_b: wgpu::BindGroup,
}

impl Resources {
    fn build(device: &wgpu::Device, surface_format: wgpu::TextureFormat, size: (u32, u32)) -> Self {
        let composited = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("trails-composited"),
            size: wgpu::Extent3d {
                width: size.0,
                height: size.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: surface_format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let composited_view = composited.create_view(&wgpu::TextureViewDescriptor::default());
        let accum = PingPongField::new(device, size.0, size.1);

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("trails-sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let fade_uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("trails-fade"),
            size: std::mem::size_of::<Fade>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let trails_shader = gpu::fullscreen_shader(
            device,
            "trails-shader",
            gpu::FULLSCREEN_VS_UV_FLIPPED,
            TRAILS_SHADER,
        );
        let trails_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("trails-bind-layout"),
            entries: &[
                gpu::uniform(0, wgpu::ShaderStages::FRAGMENT),
                gpu::texture(1, true),
                gpu::texture(2, true),
                gpu::sampler(3),
            ],
        });
        let make_trails_bg = |accum_view: &wgpu::TextureView, label: &str| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(label),
                layout: &trails_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: fade_uniform.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&composited_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(accum_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::Sampler(&sampler),
                    },
                ],
            })
        };
        let trails_bg_a = make_trails_bg(accum.view_a(), "trails-bg-a");
        let trails_bg_b = make_trails_bg(accum.view_b(), "trails-bg-b");
        let trails_pipeline = gpu::fullscreen_pipeline(
            device,
            &trails_shader,
            &[&trails_layout],
            PingPongField::FORMAT,
            wgpu::BlendState::REPLACE,
            "trails",
        );

        let present_shader = gpu::fullscreen_shader(
            device,
            "trails-present-shader",
            gpu::FULLSCREEN_VS_UV_FLIPPED,
            PRESENT_SHADER,
        );
        let present_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("trails-present-bind-layout"),
            entries: &[gpu::texture(0, true), gpu::sampler(1)],
        });
        let make_present_bg = |accum_view: &wgpu::TextureView, label: &str| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(label),
                layout: &present_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(accum_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&sampler),
                    },
                ],
            })
        };
        let present_bg_a = make_present_bg(accum.view_a(), "trails-present-bg-a");
        let present_bg_b = make_present_bg(accum.view_b(), "trails-present-bg-b");
        // Premultiplied-alpha OVER (ADR-0055) — see the note on the fold's
        // pipeline: into a transparent-cleared intermediate this is bit-identical
        // to REPLACE, and into the chain's destination it composites the trail over
        // the backdrop. The feedback pipeline above stays REPLACE: it overwrites the
        // accumulation wholesale rather than compositing onto it.
        let present_pipeline = gpu::fullscreen_pipeline(
            device,
            &present_shader,
            &[&present_layout],
            surface_format,
            wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING,
            "trails-present",
        );

        Self {
            size,
            _composited: composited,
            composited_view,
            accum,
            fade_uniform,
            trails_pipeline,
            trails_bg_a,
            trails_bg_b,
            present_pipeline,
            present_bg_a,
            present_bg_b,
        }
    }

    /// Clear both accumulation textures so the first feedback frame reads a
    /// defined (black) trail rather than undefined texels.
    fn clear_accum(&self, encoder: &mut wgpu::CommandEncoder) {
        for view in [self.accum.view_a(), self.accum.view_b()] {
            encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("trails-clear"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        // TRANSPARENT, not BLACK (ADR-0055). The accumulation is
                        // premultiplied and the feedback pass takes `max(cur, prev
                        // * fade)` on all four channels — a fresh field cleared
                        // opaque would start every pixel at alpha 1 and hold the
                        // backdrop out of the whole frame until it decayed away.
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
        }
    }
}

/// The engine feedback-trails stage — a [`PostStage`], not a
/// [`Scene`](super::scenes::Scene): it consumes an already-rendered frame rather
/// than an `AnalysisFrame`. Driven by the `trails` named param, it wraps what the
/// chain hands it (background + scene) in a fade-and-accumulate feedback
/// (ADR-0018). First in the chain, so it folds into whichever stage is active
/// after it — or straight into the surface when none is.
pub struct Trails {
    device: wgpu::Device,
    surface_format: wgpu::TextureFormat,
    res: Option<Resources>,
    amount: f32,
    /// The active tier's cap on this stage's internal grid
    /// ([`TierConfig::post_cap`](super::TierConfig::post_cap)), resolved once at
    /// construction. A field rather than a constant so the tier can raise it, and
    /// read only through [`internal_size`](PostStage::internal_size) so that stays
    /// a pure function of `surface` — which is what the chain's rebuild comparison
    /// rests on.
    post_cap: (u32, u32),
    /// How many times [`Resources::build`] has run on this stage. Diagnostic, and
    /// what pins ADR-0030's compare-first obligation in a test: rebuilding every
    /// frame would be correct-looking and would also clear the trail history every
    /// frame, which no pixel assertion would obviously catch.
    builds: u32,
}

/// Global parameter vocabulary — see [`background::PARAMS`](super::background::PARAMS).
/// **Keep in sync with `set_param` below.**
pub const PARAMS: &[&str] = &["trails"];

impl Trails {
    /// Store the device/format for a lazy build; no GPU resources yet.
    pub fn new(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        post_cap: (u32, u32),
    ) -> Self {
        Self {
            device: device.clone(),
            surface_format,
            res: None,
            amount: DEFAULT_TRAILS,
            post_cap,
            builds: 0,
        }
    }

    /// How many times this stage has built its GPU resources. See [`Self::builds`].
    #[cfg(test)]
    pub(crate) fn build_count(&self) -> u32 {
        self.builds
    }
}

impl PostStage for Trails {
    fn name(&self) -> &'static str {
        "trails"
    }

    /// Reset the `trails` amount to its default (each frame, before the active
    /// preset's bindings are routed).
    fn reset_params(&mut self) {
        self.amount = DEFAULT_TRAILS;
    }

    /// Apply one named parameter, returning whether it was the `trails` param.
    fn set_param(&mut self, name: &str, value: f32) -> bool {
        if name == "trails" {
            self.amount = value;
            true
        } else {
            false
        }
    }

    fn params(&self) -> &'static [&'static str] {
        PARAMS
    }

    /// Whether trails are active this frame (a preset bound `trails > 0`).
    fn active(&self) -> bool {
        self.amount > 0.0 && self.amount.is_finite()
    }

    /// The accumulation size, following the render target under the shared policy
    /// (ADR-0034). The chain reports this — not the surface — as the target size to
    /// a scene that sizes an internal field
    /// ([`Scene::set_target_size`](super::scenes::Scene::set_target_size)), so the
    /// scene does not supersample into an offscreen smaller than the window (Plan
    /// 0027 Phase 2).
    ///
    /// A **texel count only** — the scene's aspect comes from the render target
    /// (ADR-0037), because the present below is a plain normalized stretch that
    /// undoes whatever shape this grid happens to have.
    fn internal_size(&self, surface: (u32, u32)) -> (u32, u32) {
        internal_grid_size(surface, self.post_cap)
    }

    /// Build the resources if needed (clearing the fresh accumulation) and return
    /// the offscreen view the background + scene render into this frame. Returns
    /// `None` only if the resources are absent (never, after the build above) —
    /// the caller falls back to the surface view. Called when
    /// [`active`](PostStage::active).
    fn begin(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        surface: (u32, u32),
    ) -> Option<wgpu::TextureView> {
        // Compare-first (ADR-0030): build on the first active frame, and again only
        // when the grid actually changes size. Rebuilding unconditionally would
        // reallocate a `Rgba16Float` texture pair every frame *and* clear the trail
        // history every frame, which reads as "trails stopped working" rather than
        // as a performance bug.
        //
        // The flip side, accepted: a resize that crosses a 256 px step does blink
        // the accumulated trail away, because the field it lived in is gone.
        let wanted = self.internal_size(surface);
        if self.res.as_ref().is_none_or(|res| res.size != wanted) {
            let res = Resources::build(&self.device, self.surface_format, wanted);
            res.clear_accum(encoder);
            self.res = Some(res);
            self.builds += 1;
        }
        self.res.as_ref().map(|res| res.composited_view.clone())
    }

    /// Fold this frame's composited target into the accumulation (max-decay) and
    /// present the result to `out` — the next active stage's input, or the surface.
    /// Called after the scene has rendered into the [`begin`](PostStage::begin)
    /// target, when [`active`](PostStage::active). Returns the two passes it
    /// encodes (feedback + present).
    ///
    /// `surface` is unused: neither pass computes geometry, so this stage has no
    /// aspect to get wrong (ADR-0037). Both are normalized fullscreen blits, and
    /// the present's stretch back to the target is precisely what cancels the
    /// grid's shape out of the picture.
    fn resolve(
        &mut self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        out: &wgpu::TextureView,
        _surface: (u32, u32),
        fold: Fold,
    ) -> u32 {
        let Some(res) = self.res.as_mut() else {
            return 0;
        };
        let fade = self.amount.clamp(0.0, MAX_FADE);
        queue.write_buffer(
            &res.fade_uniform,
            0,
            bytemuck::bytes_of(&Fade {
                v: [fade, 0.0, 0.0, 0.0],
            }),
        );

        // Feedback pass: write the max-decay into the write side.
        let (trails_bg, write_view) = if res.accum.reading_a() {
            (&res.trails_bg_a, res.accum.view_b())
        } else {
            (&res.trails_bg_b, res.accum.view_a())
        };
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("trails-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: write_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        // Fully covered by the draw below; transparent for
                        // consistency with the accumulation's premultiplied model.
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&res.trails_pipeline);
            pass.set_bind_group(0, trails_bg, &[]);
            pass.draw(0..3, 0..1);
        }
        res.accum.swap();

        // Present the freshly-written accumulation to the surface.
        let present_bg = if res.accum.reading_a() {
            &res.present_bg_a
        } else {
            &res.present_bg_b
        };
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("trails-present-pass"),
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
        pass.set_pipeline(&res.present_pipeline);
        pass.set_bind_group(0, present_bg, &[]);
        pass.draw(0..3, 0..1);

        2 // the feedback pass + the present pass
    }

    /// Drop the lazily-built resources so the accumulation restarts cleared — used
    /// on the capture scene-rebuild so a capture stays a pure function of its
    /// inputs, and so a stale trails pipeline never lingers to mis-render the next
    /// capture's scene on the WARP adapter (module docs).
    fn reset_resources(&mut self) {
        self.res = None;
    }
}
