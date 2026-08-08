//! The tonemap: the one pass where the composite stops being **light** and
//! becomes a **picture** (Plan 0045 Phase 3, ADR-0046).
//!
//! Everything upstream of here — the backdrop, the scene, both post stages, the
//! transition blend — runs in linear light at
//! [`COMPOSITE_FORMAT`](super::COMPOSITE_FORMAT), where an additive accumulation
//! is free to exceed 1.0 and nothing clips. This pass reads that unbounded frame,
//! applies `exposure`, folds everything into `[0, 1)`, and writes the result at
//! the **surface** format. Downstream of it the frame is display-referred, which
//! is what [`Ink`](super::ink::Ink) has always assumed it was reading (ADR-0028 /
//! ADR-0032 are unchanged by this plan — only the pass that hands ink its input
//! is new).
//!
//! # Not skippable
//!
//! Every other pass in `render/` skips when its amount param is off. This one
//! cannot: it is not a look, it is the **format boundary**. Skipping it would
//! present linear values above 1.0 into an 8-bit surface, which is the clipped
//! composite this plan exists to retire. `exposure = 1.0` (the default) still
//! runs the pass — it is a near-identity below the knee, not a no-op.
//!
//! # The curve
//!
//! Identity below a knee, a rational shoulder above it:
//!
//! ```text
//! f(x) = x                            for x <= k
//! f(x) = 1 - (1-k)^2 / (x + 1 - 2k)   for x >  k
//! ```
//!
//! The shoulder is the unique curve of the form `1 - a/(x + b)` that meets the
//! identity at `k` with a matching slope (`f(k) = k`, `f'(k) = 1`) and tends to 1
//! — so it is C1-continuous at the knee, strictly monotone, and bounded. At
//! `k = 0.6`: `f(1) = 0.800`, `f(2) = 0.911`, `f(4) = 0.958`, `f(8) = 0.980`.
//!
//! ADR-0046 requires the curve be **monotone, hue-preserving, and near-identity
//! below the mid-range**. The first and third are the curve's own shape; the
//! second is how it is applied to colour: the scale factor `f(m)/m` is computed
//! from the **brightest channel** `m` and applied to all three, so the ratios
//! between R, G and B are exactly preserved and the roll-off never rotates a hue
//! or washes a saturated core toward white. It is also gamut-safe by
//! construction — the largest channel lands on `f(m) < 1`, so no channel can
//! exceed 1 and be clipped by the 8-bit write.
//!
//! Plain Reinhard (`x / (1 + x)`) was not an option despite being the obvious
//! one: it maps 0.8 to 0.44, so every existing preset would have gone dark.
//! ADR-0046's "near-identity below the mid-range" is what rules it out, and it is
//! what keeps this plan's golden re-bless confined to the regions that were
//! actually clipping.
//!
//! # Why the output is 8-bit, not float
//!
//! Plan 0045 Phase 3's file list reads "…and ink-src to `Rgba16Float`". Taken
//! literally that needs **two** tonemap pipelines — one targeting ink's float
//! input, one targeting the surface for the (common) ink-off frame — against this
//! plan's own documented WARP pipeline-count risk. The linear region therefore
//! ends *at this pass's input*: the tonemap writes display-referred values at the
//! surface format into ink's input, or straight into the surface when ink is off.
//! One pipeline, and ink's semantics are bit-for-bit what they were.

// Hot-path panic-denial pragma (Plan 0002 Phase 2; render/ is scanned by the
// hygiene guard). This pass runs on every displayed frame.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

use super::gpu;

/// `exposure` default — a plain 1.0 stop, so an unbound preset is scaled by
/// nothing and only the curve applies.
///
/// `pub(crate)` since ADR-0080: the bloom bright-pass now thresholds against
/// *exposed* luminance, so it needs the neutral stop — the value at which its own
/// multiply is the IEEE-754 identity and every existing baseline is byte-identical
/// — and the two must not be able to drift to different numbers.
pub(crate) const DEFAULT_EXPOSURE: f32 = 1.0;

/// Where the shoulder starts. Below this the curve is **exactly** the identity,
/// which is what keeps existing sub-1.0 content where it was (ADR-0046).
///
/// **0.6, chosen by looking at the re-blessed baselines rather than at the
/// formula.** The knee sets how much of the output range is left for everything
/// above 1.0, and the display is 8-bit sRGB, so that is the number that decides
/// whether stacked light stays *modelled* or reads as a flat plateau:
///
/// | knee | f(1) | f(8) | bytes spanned by 1.0 -> 8.0 |
/// |------|------|------|------------------------------|
/// | 0.8  | 0.900| 0.995| 244 -> 254  (**10**)         |
/// | 0.6  | 0.800| 0.980| 231 -> 253  (**22**)         |
///
/// At 0.8 the whole over-range decade landed inside ten bytes and the trails
/// fixture's star came back a flat teal silhouette — correct arithmetic, no
/// interior. 0.6 more than doubles the room and the interior comes back.
///
/// It is not free: the curve stops being the identity at 0.6 rather than 0.8, so
/// a linear 0.8 mid-tone now presents at 0.733 — about eight bytes darker. That
/// is the trade, and it is deliberately paid on mid-tones (where nothing was
/// broken but the loss is a slight dimming) rather than on highlights (where the
/// loss was the picture).
pub(crate) const KNEE: f32 = 0.6;

/// The shipped curve, on the CPU — the same arithmetic as `shoulder` in
/// [`SHADER`], kept here so the properties ADR-0046 asks for are unit-testable
/// without a GPU.
///
/// Defined for `x >= 0`; the shader clamps negatives away before calling it.
#[cfg(test)]
pub(crate) fn map(x: f32) -> f32 {
    if x <= KNEE {
        return x;
    }
    let a = (1.0 - KNEE) * (1.0 - KNEE);
    1.0 - a / (x + 1.0 - 2.0 * KNEE)
}

/// Exposure + tonemap body. `VsOut`/`vs_main` come from
/// [`gpu::FULLSCREEN_VS_UV_FLIPPED`] — this pass samples what another pass
/// rendered, so it needs the render-target orientation.
const SHADER: &str = r#"
struct Ctl { v: vec4<f32> } // x: exposure, y: knee

// The uniform sits BETWEEN the texture and the sampler, and that is
// load-bearing — see `Resources::build`.
@group(0) @binding(0) var t_src: texture_2d<f32>;
@group(0) @binding(1) var<uniform> u: Ctl;
@group(0) @binding(2) var samp: sampler;

// Identity below the knee, a rational shoulder above it. C1-continuous at k and
// strictly monotone, so two values never swap order across the map.
fn shoulder(x: f32, k: f32) -> f32 {
    if (x <= k) {
        return x;
    }
    let a = (1.0 - k) * (1.0 - k);
    return 1.0 - a / (x + 1.0 - 2.0 * k);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let src = textureSample(t_src, samp, in.uv);
    // Negative light is not a thing; a float target can hold it, so clamp once
    // here rather than letting it invert the scale factor below.
    var rgb = max(src.rgb, vec3<f32>(0.0)) * max(u.v.x, 0.0);

    let k = u.v.y;
    // The BRIGHTEST channel drives the roll-off and all three scale by the same
    // factor, so hue and saturation survive it exactly (ADR-0046). Applying the
    // curve per channel instead would desaturate bright cores toward white.
    let m = max(rgb.r, max(rgb.g, rgb.b));
    if (m > k) {
        // k >= 0.6, so m > k means m is comfortably non-zero.
        rgb = rgb * (shoulder(m, k) / m);
    }

    // Alpha passes through untouched: the backdrop owns this target's clear and
    // paints it opaque (ADR-0055 put the backdrop *under* the chain), so this is
    // 1.0 on every frame — but reading it rather than writing a literal keeps
    // the pass honest if that ever stops being true.
    return vec4<f32>(rgb, src.a);
}
"#;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Ctl {
    v: [f32; 4],
}

/// The tonemap's GPU resources, built lazily on the first frame and rebuilt only
/// on a surface-size change (ADR-0030's compare-first rule).
struct Resources {
    /// The surface-sized **linear** offscreen everything upstream renders into.
    /// Kept alive so `src_view` stays valid; not read after construction.
    _src: wgpu::Texture,
    src_view: wgpu::TextureView,
    uniform: wgpu::Buffer,
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    width: u32,
    height: u32,
}

impl Resources {
    fn build(
        device: &wgpu::Device,
        target_format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> Self {
        let src = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("tonemap-src"),
            size: wgpu::Extent3d {
                width: width.max(1),
                height: height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            // The linear-light currency, not the surface's — this texture is the
            // last thing in the frame that holds values above 1.0.
            format: super::COMPOSITE_FORMAT,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC
                // COPY_DST is for the tests alone: it is what lets a known
                // linear frame be written straight in, so the shipped WGSL curve
                // can be checked against this module's CPU mirror. A usage flag,
                // no runtime cost.
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let src_view = src.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("tonemap-sampler"),
            // 1:1 same-size map, so nearest is exact; clamp at the edges.
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("tonemap-uniform"),
            size: std::mem::size_of::<Ctl>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let shader = gpu::fullscreen_shader(
            device,
            "tonemap-shader",
            gpu::FULLSCREEN_VS_UV_FLIPPED,
            SHADER,
        );
        // **The binding order here is deliberate, not stylistic (ADR-0021 /
        // Plan 0020).** On the DX12 WARP software adapter a pipeline whose
        // bind-group layout matches another live one is handed the *other* pass's
        // resources, and this pass is uniquely exposed: it runs on every frame
        // alongside whatever the preset has switched on.
        //
        // It is not a subtle corruption and it was not hypothetical. The natural
        // layout — `uniform, texture, sampler` — is byte-identical to `ink`'s and
        // to the fold's, and with it WARP fed this shader the **kaleidoscope's**
        // uniform: `exposure` became `kaleido_order` (6.0) and `knee` became
        // `kaleido_angle` (0.0), which reproduces the wrong pixels exactly. Moving
        // the uniform into its own group made group 0 `[uniform]` — identical to
        // the **backdrop's** — and WARP then fed it `bg_hue` as the exposure and
        // `bg_bright` as the knee, again to the byte. The hardware adapter
        // rendered both configurations correctly, which is precisely what makes
        // this class of defect expensive: the whole golden suite captures on WARP.
        //
        // So: one group, with the uniform **between** the texture and the sampler
        // — `[texture, uniform, sampler]`, which
        // `the_tonemap_layout_is_a_shape_no_other_layout_in_core_has` proves is
        // held by nothing else in `core/src`.
        //
        // **Plan 0045 Phase 4b moved it there, because the shipped ordering's own
        // justification was false.** Phase 3 wrote `[texture, sampler, uniform]`
        // and claimed it was unique; `attractor-decay`
        // (`scenes/particles/mod.rs`) is byte-identical to it, from the same three
        // helpers. No mis-render was ever observed from that collision — the
        // WARP-blessed `attractor.png` and a hardware render of the same preset
        // agree (mean luma 51.84 vs 56.29, lit-pixel counts within 0.1 %), so the
        // curve was not being fed the decay pass's uniform — but a false comment
        // on a hazard surface is worse than none, and the enumeration is cheap.
        let bind_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("tonemap-bind-layout"),
            entries: &[
                gpu::texture(0, true),
                gpu::uniform(1, wgpu::ShaderStages::FRAGMENT),
                gpu::sampler(2),
            ],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("tonemap-bind-group"),
            layout: &bind_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&src_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: uniform.as_entire_binding(),
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
            target_format,
            wgpu::BlendState::REPLACE,
            "tonemap",
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

/// Global parameter vocabulary — see [`background::PARAMS`](super::background::PARAMS).
/// **Keep in sync with `set_param` below.**
pub const PARAMS: &[&str] = &["exposure"];

/// The engine-wide exposure + tonemap pass. Neither a
/// [`Scene`](super::scenes::Scene) nor a [`PostStage`](super::post::PostStage):
/// it consumes an already-rendered frame, it is engine-wide rather than
/// per-preset, and unlike every other pass it never skips (module docs).
pub(crate) struct Tonemap {
    device: wgpu::Device,
    /// The **display-referred** format this pass writes: ink's input when ink is
    /// active, the surface otherwise — the same format either way.
    target_format: wgpu::TextureFormat,
    res: Option<Resources>,
    exposure: f32,
}

impl Tonemap {
    /// Store the device/format for a lazy build; no GPU resources yet.
    pub(crate) fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        Self {
            device: device.clone(),
            target_format,
            res: None,
            exposure: DEFAULT_EXPOSURE,
        }
    }

    /// Reset `exposure` to its default (each frame, before routing).
    pub(crate) fn reset_params(&mut self) {
        self.exposure = DEFAULT_EXPOSURE;
    }

    /// Apply one named parameter, returning whether it was `exposure`.
    pub(crate) fn set_param(&mut self, name: &str, value: f32) -> bool {
        if name == "exposure" {
            self.exposure = value;
            true
        } else {
            false
        }
    }

    /// This frame's evaluated exposure — held for the **outgoing** preset at a
    /// dissolve's start, to be handed back to
    /// [`crossfade_from`](Self::crossfade_from).
    pub(crate) fn exposure(&self) -> f32 {
        self.exposure
    }

    /// The stop this pass will **actually apply** this frame: the bound value with
    /// the two guards [`resolve`](Self::resolve) used to apply inline — negatives
    /// floored (a negative stop would invert the frame) and a non-finite binding
    /// replaced by the default.
    ///
    /// Extracted so the bloom stage can threshold against the same number rather
    /// than a second transcription of it (ADR-0080). It is a **one-way read**: the
    /// stage takes the value and has no way to change it, which is what keeps the
    /// composite's fixed-order property (ADR-0018) intact — the tonemap still owns
    /// `exposure` and still applies it, downstream, exactly as before.
    ///
    /// Distinct from [`exposure`](Self::exposure), which returns the raw bound
    /// value because that is what a dissolve holds and hands back to
    /// [`crossfade_from`](Self::crossfade_from).
    pub(crate) fn applied_exposure(&self) -> f32 {
        if self.exposure.is_finite() {
            self.exposure.max(0.0)
        } else {
            DEFAULT_EXPOSURE
        }
    }

    /// Declare that this frame is `t` of the way through a dissolve out of an
    /// `from` exposure, and interpolate to this frame's own (the incoming
    /// preset's, already routed).
    ///
    /// One engine-wide pass over a *blended* frame cannot show two presets' stops
    /// at once, so it shows the mix — exactly ink's answer to the same problem
    /// (ADR-0032). Without this an `exposure`-binding preset would pop one stop
    /// on the frame the roster flips.
    pub(crate) fn crossfade_from(&mut self, from: f32, t: f32) {
        let t = t.clamp(0.0, 1.0);
        self.exposure = from + (self.exposure - from) * t;
    }

    /// Build the surface-sized resources if needed (or rebuild on a size change)
    /// and return the **linear** offscreen view everything upstream renders into
    /// this frame — the chain's destination, or the transition blend's output
    /// while a dissolve runs. `None` only if the resources are absent (never,
    /// after the build); the caller then falls back to its own view, which
    /// degrades to the pre-Plan-0045 clipped composite rather than to no frame.
    pub(crate) fn begin(&mut self, surface: (u32, u32)) -> Option<wgpu::TextureView> {
        let (width, height) = (surface.0.max(1), surface.1.max(1));
        let stale = self
            .res
            .as_ref()
            .is_none_or(|res| res.width != width || res.height != height);
        if stale {
            self.res = Some(Resources::build(
                &self.device,
                self.target_format,
                width,
                height,
            ));
        }
        self.res.as_ref().map(|res| res.src_view.clone())
    }

    /// Map the linear input into `out` — ink's input when ink is active, the
    /// surface otherwise. Called after everything upstream has rendered into the
    /// [`begin`](Self::begin) target. Returns the draw calls encoded.
    pub(crate) fn resolve(
        &mut self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        out: &wgpu::TextureView,
    ) -> u32 {
        let Some(res) = self.res.as_ref() else {
            return 0;
        };
        let exposure = self.applied_exposure();
        queue.write_buffer(
            &res.uniform,
            0,
            bytemuck::bytes_of(&Ctl {
                v: [exposure, KNEE, 0.0, 0.0],
            }),
        );
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("tonemap-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: out,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    // Fully covered by the fullscreen draw below.
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

        1 // the tonemap pass
    }

    /// The linear input texture, for a test that wants to read the composite
    /// **before** it is mapped — the only place an over-1.0 accumulation is
    /// observable (Plan 0045 Phase 3's first done-when).
    #[cfg(test)]
    pub(crate) fn src_texture(&self) -> Option<&wgpu::Texture> {
        self.res.as_ref().map(|res| &res._src)
    }

    /// Drop the lazily-built resources — used on the capture scene-rebuild so a
    /// headless capture stays a pure function of its inputs (NFR §6).
    pub(crate) fn reset_resources(&mut self) {
        self.res = None;
    }
}

#[cfg(test)]
mod tests;
