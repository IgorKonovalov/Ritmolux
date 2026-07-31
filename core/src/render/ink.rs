//! Final-stage duotone "ink" tone-remap (ADR-0028, Plan 0027 Phase 1): the
//! **terminal engine post-pass** of the ADR-0018 fixed composite. It reads the
//! fully composited frame's per-pixel luminance as an *ink density* `d` and
//! outputs `mix(paper, ink, d)` between two preset-configurable colors — so a
//! bright additive stroke (density high) becomes ink and the dark base (density
//! low) becomes paper. That is a *darkening* step the additive scene pipelines
//! can't express, which is what "black on white" needs: it is the inverse of the
//! compositing model, not a color choice reachable by any scene param.
//!
//! Default `paper = white`, `ink = black`, so turning the stage on with the
//! default colors is a pure black-on-white invert; colored poles (cream + indigo,
//! sepia, ...) are the same operation with different `paper_*`/`ink_*`. The stage
//! is driven by the `ink_*` named params, offered to it by the renderer after the
//! background and the chain, so there is **no `Scene`-trait change and the C ABI
//! is untouched**.
//!
//! # Why it is not a [`PostStage`](super::post::PostStage)
//!
//! Ink is **engine-wide**, not per-preset (ADR-0032): trails and kaleidoscope are
//! a look a preset composes, and both sides of a cross-preset dissolve legitimately
//! have their own — but there is exactly **one** finished frame to remap. So ink
//! sits outside the [`PostChain`](super::post::PostChain), driven directly by the
//! renderer, symmetric with [`Background`](super::background::Background) as the
//! pre-pass. That placement is what lets Plan 0023's two-input transition blend run
//! *before* it without widening the one-input `PostStage` trait, and it makes
//! ADR-0028's "ink remaps the blended frame" ordering structural rather than a rule
//! the chain has to honor. It still runs before the text/overlay passes, so the HUD
//! is never inverted.
//!
//! **Passthrough and unbuilt when `ink_amount <= 0`** — every shipped preset until
//! one opts in — so the renderer skips the pass entirely and the chain folds
//! straight to the surface: no offscreen, no pipeline, golden/determinism
//! unchanged, the NFR §1 iGPU floor pays nothing,
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

use crate::render::gpu;

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
    // Two pole pairs and a crossfade between them: `from` is the outgoing
    // preset's while a dissolve runs, and equals `to` otherwise (ADR-0032).
    paper_from: vec4<f32>, // x hue, y sat, z bright, w unused
    ink_from: vec4<f32>,
    paper_to: vec4<f32>,
    ink_to: vec4<f32>,
    ctl: vec4<f32>,        // x amount, y dissolve progress, rest unused
}

@group(0) @binding(0) var<uniform> u: Ink;
@group(0) @binding(1) var t_src: texture_2d<f32>;
@group(0) @binding(2) var samp: sampler;

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

    // Crossfade the two presets' poles in **RGB**, after the HSV conversion.
    // Interpolating hue/sat instead would sweep an unrelated arc of the wheel —
    // white (sat 0) to blue passes through yellow-green, a tone neither preset
    // configures. This is still a param crossfade feeding **one** remap of the
    // blended frame: the non-linearity ADR-0032 rejects is remapping each side
    // and mixing the *results*, which is a different thing.
    // Outside a dissolve `from == to` and `t == 0`, so both mixes return the
    // first operand exactly and the output is bit-identical to a single pole pair.
    let t = clamp(u.ctl.y, 0.0, 1.0);
    let paper = mix(hsv2rgb(u.paper_from.xyz), hsv2rgb(u.paper_to.xyz), t);
    let ink = mix(hsv2rgb(u.ink_from.xyz), hsv2rgb(u.ink_to.xyz), t);
    let remapped = mix(paper, ink, d);

    // `amount` gates the remap against the untouched frame (0 = passthrough, 1 =
    // full remap), so a preset can breathe between glow and ink on the beat. The
    // stage is skipped entirely at amount <= 0, so this only blends for amount > 0.
    let amount = clamp(u.ctl.x, 0.0, 1.0);
    return vec4<f32>(mix(src, remapped, amount), 1.0);
}
"#;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct InkUniform {
    paper_from: [f32; 4],
    ink_from: [f32; 4],
    paper_to: [f32; 4],
    ink_to: [f32; 4],
    ctl: [f32; 4],
}

struct Resources {
    /// The surface-sized offscreen the **tonemap** writes into (ADR-0046) — so
    /// this holds display-referred pixels at the surface format, which is exactly
    /// what ink has always assumed it was remapping. Kept alive so `src_view`
    /// stays valid; not read after construction.
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
        let shader =
            gpu::fullscreen_shader(device, "ink-shader", gpu::FULLSCREEN_VS_UV_FLIPPED, SHADER);
        let bind_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ink-bind-layout"),
            entries: &[
                gpu::uniform(0, wgpu::ShaderStages::FRAGMENT),
                gpu::texture(1, true),
                gpu::sampler(2),
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
        let pipeline = gpu::fullscreen_pipeline(
            device,
            &shader,
            &[&bind_layout],
            surface_format,
            wgpu::BlendState::REPLACE,
            "ink",
        );

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

/// The engine terminal duotone tone-remap — neither a
/// [`Scene`](super::scenes::Scene) nor a [`PostStage`](super::post::PostStage): it
/// consumes an already-rendered frame rather than an `AnalysisFrame`, and it is
/// engine-wide rather than per-preset (module docs, ADR-0032). Driven by the
/// `ink_*` named params, it remaps the finished frame before present (ADR-0028).
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
    /// The **outgoing** preset's params while a dissolve runs, plus that
    /// dissolve's progress — the crossfade state (ADR-0032). `None` on every
    /// ordinary frame, where the shader sees one pole pair and `t = 0`.
    crossfade: Option<(InkParams, f32)>,
}

/// One preset's evaluated remap params, held so a cross-preset dissolve can
/// crossfade ink (ADR-0032).
///
/// Ink is a single **engine-wide** pass, but a dissolve has two presets each
/// binding their own `ink_*`/`paper_*`. Holding the outgoing side's values at the
/// dissolve's start and interpolating toward the incoming side's by the same `t`
/// that drives the blend makes `t = 0` exactly the outgoing look and `t = 1`
/// exactly the incoming one, with no snap at either endpoint.
///
/// This is a crossfade of the **poles feeding one remap**, not of two remapped
/// frames: `mix(paper, ink, luminance)` is non-linear in the frame, so remapping
/// each side and blending the results would show a tone neither preset configures
/// (ADR-0032 Alternative B, rejected). The poles themselves interpolate in RGB, in
/// the shader, after the HSV conversion — see the fragment stage for why.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InkParams {
    amount: f32,
    paper_hue: f32,
    paper_sat: f32,
    paper_bright: f32,
    ink_hue: f32,
    ink_sat: f32,
    ink_bright: f32,
}

impl InkParams {
    /// This pair packed for the shader: `(paper hsv, ink hsv)`.
    fn poles(&self) -> ([f32; 4], [f32; 4]) {
        (
            [self.paper_hue, self.paper_sat, self.paper_bright, 0.0],
            [self.ink_hue, self.ink_sat, self.ink_bright, 0.0],
        )
    }
}

impl Default for InkParams {
    /// The remap's own defaults — `amount = 0` (off), white paper, black ink. What
    /// a preset that binds no `ink_*` hands to a dissolve, so fading *into* a
    /// non-ink preset fades the remap out rather than cutting it.
    fn default() -> Self {
        Self {
            amount: DEFAULT_AMOUNT,
            paper_hue: DEFAULT_PAPER_HUE,
            paper_sat: DEFAULT_PAPER_SAT,
            paper_bright: DEFAULT_PAPER_BRIGHT,
            ink_hue: DEFAULT_INK_HUE,
            ink_sat: DEFAULT_INK_SAT,
            ink_bright: DEFAULT_INK_BRIGHT,
        }
    }
}

/// Linear interpolation, with `t` clamped to `[0, 1]` so an out-of-range progress
/// can never extrapolate a param past either preset's value.
fn lerp(from: f32, to: f32, t: f32) -> f32 {
    from + (to - from) * t.clamp(0.0, 1.0)
}

/// Global parameter vocabulary — see [`background::PARAMS`](super::background::PARAMS).
/// **Keep in sync with `set_param` below.**
pub const PARAMS: &[&str] = &[
    "ink_amount",
    "paper_hue",
    "paper_sat",
    "paper_bright",
    "ink_hue",
    "ink_sat",
    "ink_bright",
];

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
            crossfade: None,
        }
    }

    /// Reset the remap params to their defaults (each frame, before routing). The
    /// crossfade state goes with them: the renderer re-declares it after routing,
    /// for exactly the frames a dissolve is running.
    pub fn reset_params(&mut self) {
        self.amount = DEFAULT_AMOUNT;
        self.paper_hue = DEFAULT_PAPER_HUE;
        self.paper_sat = DEFAULT_PAPER_SAT;
        self.paper_bright = DEFAULT_PAPER_BRIGHT;
        self.ink_hue = DEFAULT_INK_HUE;
        self.ink_sat = DEFAULT_INK_SAT;
        self.ink_bright = DEFAULT_INK_BRIGHT;
        self.crossfade = None;
    }

    /// Apply one named parameter, returning whether it was an `ink_*`/`paper_*`
    /// param. The renderer offers a name to the background, then the chain, then
    /// here, and falls through to the scene only when all three return `false` —
    /// the namespaces are disjoint, so no param reaches more than one owner.
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

    /// Whether the remap is active this frame (`ink_amount > 0`; at or below zero
    /// it is the identity passthrough and the pass is skipped entirely).
    pub fn active(&self) -> bool {
        self.amount > 0.0 && self.amount.is_finite()
    }

    /// This frame's evaluated params — held for the **outgoing** preset at a
    /// dissolve's start, to be handed back to [`crossfade_from`](Self::crossfade_from).
    pub fn params(&self) -> InkParams {
        InkParams {
            amount: self.amount,
            paper_hue: self.paper_hue,
            paper_sat: self.paper_sat,
            paper_bright: self.paper_bright,
            ink_hue: self.ink_hue,
            ink_sat: self.ink_sat,
            ink_bright: self.ink_bright,
        }
    }

    /// Declare that this frame is `t` of the way through a dissolve out of `from`
    /// (the outgoing preset's params, held at the dissolve's start). This frame's
    /// own params — the incoming preset's, already evaluated and routed — are the
    /// other end.
    ///
    /// Call **after** routing and **before** [`active`](Self::active): `amount`
    /// interpolates here, so a dissolve out of an ink-on preset into an ink-off one
    /// keeps the pass running at a fading `ink_amount` rather than cutting it off
    /// at the first frame. The **poles** are handed to the shader as two pairs and
    /// interpolated there, in RGB — see the fragment stage.
    pub fn crossfade_from(&mut self, from: &InkParams, t: f32) {
        let t = t.clamp(0.0, 1.0);
        self.amount = lerp(from.amount, self.amount, t);
        self.crossfade = Some((*from, t));
    }

    /// Build the surface-sized resources if needed (or rebuild on a size change)
    /// and return the offscreen view **everything upstream renders into** this
    /// frame — the chain's destination, or the transition blend's output while a
    /// dissolve runs. The remap is a 1:1 per-pixel operation, so this input is
    /// always sized from the surface rather than a fixed internal grid (module
    /// docs). `None` only if the resources are absent (never, after the build);
    /// the caller then falls back to the surface. Call when
    /// [`active`](Self::active).
    pub fn begin(&mut self, surface: (u32, u32)) -> Option<wgpu::TextureView> {
        let (width, height) = (surface.0.max(1), surface.1.max(1));
        let stale = self
            .res
            .as_ref()
            .is_none_or(|res| res.width != width || res.height != height);
        if stale {
            self.res = Some(Resources::build(
                &self.device,
                self.surface_format,
                width,
                height,
            ));
        }
        self.res.as_ref().map(|res| res.src_view.clone())
    }

    /// Remap the input offscreen into `out`. Ink is the terminal pass (ADR-0028 /
    /// ADR-0032), so `out` is the surface. Called after everything upstream has
    /// rendered into the [`begin`](Self::begin) target, when
    /// [`active`](Self::active).
    pub fn resolve(
        &mut self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        out: &wgpu::TextureView,
    ) -> u32 {
        let Some(res) = self.res.as_ref() else {
            return 0;
        };
        // Outside a dissolve both pole pairs are this frame's and `t` is 0, so the
        // shader's `mix` returns the first operand exactly — bit-identical to a
        // single-pair uniform, which is why no golden re-blesses.
        let (paper_to, ink_to) = self.params().poles();
        let (from, t) = self.crossfade.unwrap_or((self.params(), 0.0));
        let (paper_from, ink_from) = from.poles();
        queue.write_buffer(
            &res.uniform,
            0,
            bytemuck::bytes_of(&InkUniform {
                paper_from,
                ink_from,
                paper_to,
                ink_to,
                ctl: [self.amount, t, 0.0, 0.0],
            }),
        );
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("ink-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: out,
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

        1 // the remap pass
    }

    /// Drop the lazily-built resources — used on the capture scene-rebuild so a
    /// stale remap pipeline never lingers to mis-render the next capture's scene on
    /// the WARP adapter (module docs).
    pub fn reset_resources(&mut self) {
        self.res = None;
    }
}
