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
mod tests {
    //! The curve's three ADR-0046 properties, GPU-free, plus the two pixel-level
    //! claims Plan 0045 Phase 3 owes: that the composite really carries values
    //! above 1.0 into this pass, and that this pass separates them instead of
    //! clipping them together.

    // Test asserts index, expect and panic freely; this is not the render path.
    #![allow(clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

    use super::{KNEE, Tonemap, map};
    use crate::dsp::AnalysisFrame;
    use crate::preset::Preset;
    use crate::render::capture;
    use crate::render::context::RenderError;
    use crate::render::{HeadlessOptions, Renderer};

    /// **Near-identity below the mid-range** (ADR-0046). A frame whose values are
    /// all at or below the knee comes back unchanged to well within a byte.
    ///
    /// This is the property that rules plain Reinhard out — it maps 0.8 to 0.44,
    /// which would have darkened every shipped preset — and it is what confines
    /// this plan's golden re-bless to the regions that were actually clipping.
    #[test]
    fn the_curve_is_near_identity_below_the_mid_range() {
        // A byte is 1/255 ~ 0.0039; hold the curve an order of magnitude inside
        // that so "unchanged" is a pixel claim, not an arithmetic one.
        const TOL: f32 = 1.0e-4;
        let mut x = 0.0f32;
        while x <= KNEE {
            assert!(
                (map(x) - x).abs() < TOL,
                "f({x}) = {} drifts from identity below the knee",
                map(x)
            );
            x += 0.01;
        }
        assert!(
            (map(KNEE) - KNEE).abs() < TOL,
            "the knee itself is identity"
        );
    }

    /// **Monotone, and bounded below 1** (ADR-0046). A saturating ramp maps in
    /// strictly increasing order — so two values never swap places — and never
    /// reaches 1.0, so the 8-bit write below this pass has somewhere to put the
    /// decade above 1.0 instead of flattening all of it onto one value.
    ///
    /// **Bounded below 1 is not "never 255".** The write is sRGB-encoded and then
    /// rounded to a byte, and rounding is not injective: `f(x) < 1` for every
    /// finite `x`, but `f(x)` still crosses the last byte's midpoint at a linear
    /// input of about **36** at [`KNEE`] `= 0.6`. A frame carrying that much light
    /// presents 255 legitimately. What the curve buys is the *separation* asserted
    /// below — 2.0 and 4.0 landing on different bytes, where the 8-bit chain gave
    /// both the same white.
    #[test]
    fn a_saturating_ramp_maps_monotonically_and_never_reaches_clip() {
        let mut previous = map(0.0);
        let mut x = 0.01f32;
        while x <= 64.0 {
            let y = map(x);
            assert!(
                y > previous,
                "f is not strictly increasing at {x}: {y} <= {previous}"
            );
            assert!(y < 1.0, "f({x}) = {y} reached the clip");
            previous = y;
            x *= 1.05;
        }
        // The shoulder's whole point: an accumulation that used to clip to flat
        // white is now separable — 2.0 and 4.0 land on different bytes.
        let two = (map(2.0) * 255.0).round();
        let four = (map(4.0) * 255.0).round();
        assert!(
            four > two,
            "2.0 and 4.0 must not land on the same byte: {two} vs {four}"
        );
    }

    /// **Hue-preserving** (ADR-0046): the roll-off scales all three channels by
    /// one factor, so the ratios between them — and therefore the hue and the
    /// saturation — are exactly what came in. A per-channel curve would fail
    /// this by washing the core toward white.
    #[test]
    fn the_roll_off_preserves_channel_ratios() {
        // A saturated over-range colour: 4.0 of red against 1.0 of green.
        let rgb = [4.0f32, 1.0, 0.25];
        let m = rgb[0];
        let scale = map(m) / m;
        let out = rgb.map(|c| c * scale);

        assert!(out.iter().all(|&c| c < 1.0), "gamut-safe: {out:?}");
        for pair in [(0usize, 1usize), (1, 2)] {
            let before = rgb[pair.0] / rgb[pair.1];
            let after = out[pair.0] / out[pair.1];
            assert!(
                (before - after).abs() < 1.0e-5,
                "channel ratio {before} became {after} — the map rotated the hue"
            );
        }
    }

    // -----------------------------------------------------------------------
    // The pixel-level claims, on a real composite (needs a GPU adapter)
    // -----------------------------------------------------------------------

    /// The fixture both GPU assertions run on: a dense additive rose whose
    /// strokes cross each other everywhere. Shared with
    /// `core/tests/composite.rs`, which pins the same figure to a baseline — one
    /// definition, two guards.
    const OVERLAP_FIXTURE: &str = include_str!("../../tests/fixtures/composite_overlap.toml");

    /// Small enough to read back twice cheaply; large enough that the rose's
    /// crossings cover many pixels.
    const WIDTH: u32 = 160;
    const HEIGHT: u32 = 100;
    /// Frames warmed before the capture. The figure is static (`spin = 0`), so
    /// this only has to get past the lazy resource builds.
    const FRAMES: u32 = 4;

    /// Rec.709 relative luminance — the ordering the "brighter than" claims are
    /// made in, so a hue difference between two pixels cannot decide them.
    fn luma(r: f32, g: f32, b: f32) -> f32 {
        0.2126 * r + 0.7152 * g + 0.0722 * b
    }

    /// A headless renderer on the software adapter, or `None` (a logged skip) on
    /// a runner with no GPU — macOS has no software Metal fallback (ADR-0016).
    fn headless() -> Option<Renderer> {
        match Renderer::new_headless(HeadlessOptions {
            width: WIDTH,
            height: HEIGHT,
            prefer_software: true,
        }) {
            Ok(r) => Some(r),
            Err(RenderError::RequestAdapter(_)) => {
                eprintln!("skipped: no GPU adapter on this runner (ADR-0016)");
                None
            }
            Err(e) => panic!("headless renderer build failed: {e}"),
        }
    }

    /// **Plan 0045 Phase 3's two done-when claims, on one frame.**
    ///
    /// 1. *The composite carries float linear values from scene to blend.* The
    ///    tonemap's input is read back **before** the map runs — the only place
    ///    an over-1.0 accumulation is observable — and the additive rose's
    ///    crossings are found above 1.0 there. On the pre-Plan-0045 8-bit chain
    ///    this readback could not exceed 1.0 by construction.
    ///
    /// 2. *Two overlapping full-brightness strokes no longer clip to flat
    ///    white.* The same frame's 8-bit surface is compared at two pixels the
    ///    **linear** buffer identifies rather than at hard-coded coordinates: the
    ///    brightest (a crossing, above 1.0) and one sitting at a single stroke's
    ///    ~1.0. The crossing must come out strictly brighter, and below clip.
    ///
    /// The two halves belong in one test because the second's honesty depends on
    /// the first: without the linear buffer to locate them, "the overlap region"
    /// and "a single stroke" would be coordinates someone guessed.
    #[test]
    fn stacked_light_survives_the_composite_and_separates_after_the_map() {
        let Some(mut renderer) = headless() else {
            return;
        };
        let preset = Preset::from_toml_str(OVERLAP_FIXTURE).expect("the overlap fixture parses");
        let name = preset.name.clone();
        renderer.set_presets(vec![preset]);

        let frame = AnalysisFrame {
            bass: 0.6,
            mid: 0.5,
            treb: 0.6,
            onset: 0.4,
            bar: 0.25,
            ..Default::default()
        };
        // The 8-bit surface of the *last* frame this renders — and the tonemap's
        // input still holds that same frame's linear composite afterwards.
        let displayed = renderer
            .capture_preset(&name, &frame, FRAMES)
            .expect("capture the overlap fixture");

        // --- 1. the linear composite, before the map ---
        let device = renderer.ctx.device.clone();
        let queue = renderer.ctx.queue.clone();
        let src = renderer
            .tonemap
            .src_texture()
            .expect("the tonemap built its input while capturing")
            .clone();
        let (buffer, padded_bpr) = capture::create_linear_readback(&device, WIDTH, HEIGHT);
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("tonemap-src-readback"),
        });
        capture::record_copy(&mut encoder, &src, &buffer, padded_bpr, WIDTH, HEIGHT);
        queue.submit(std::iter::once(encoder.finish()));
        let linear = capture::read_back_linear(&device, &buffer, WIDTH, HEIGHT, padded_bpr)
            .expect("read back the linear composite");

        let mut peak = (0usize, f32::MIN);
        let mut single = None;
        for (index, texel) in linear.chunks_exact(4).enumerate() {
            let l = luma(texel[0], texel[1], texel[2]);
            if l > peak.1 {
                peak = (index, l);
            }
            // A pixel a lone full-brightness stroke covers: at or just below the
            // 1.0 a single stroke reaches, and above the knee so the map has
            // actually done something to it.
            if (KNEE..=1.0).contains(&l) && single.is_none_or(|(_, best)| l > best) {
                single = Some((index, l));
            }
        }
        assert!(
            peak.1 > 1.0,
            "the composite clipped: its brightest linear luminance is {} — an \
             additive crossing must exceed 1.0 where the 8-bit chain could not",
            peak.1
        );
        let Some((single_index, single_luma)) = single else {
            panic!("no single-stroke pixel between the knee and 1.0 to compare against");
        };

        // --- 2. the same two pixels on the 8-bit surface, after the map ---
        let byte_luma = |index: usize| {
            let px = &displayed.rgba[index * 4..index * 4 + 4];
            luma(px[0] as f32, px[1] as f32, px[2] as f32)
        };
        let crossing = byte_luma(peak.0);
        let stroke = byte_luma(single_index);
        assert!(
            crossing > stroke,
            "the crossing (linear {:.3}) came out no brighter than the single \
             stroke (linear {single_luma:.3}) after the map: {crossing:.1} vs \
             {stroke:.1} — that is the flat-white clip this plan removes",
            peak.1
        );
        let clipped = displayed
            .rgba
            .chunks_exact(4)
            .flat_map(|px| px.iter().take(3))
            .filter(|&&c| c == u8::MAX)
            .count();
        assert_eq!(
            clipped, 0,
            "{clipped} channels of the overlap fixture reached 255. This is a \
             claim about **this fixture**, not about the curve: bounded below 1 \
             does not make a 255 byte unreachable (the sRGB write rounds, and a \
             linear ~36 crosses the last byte's midpoint at KNEE = 0.6). What is \
             true here is that this rose's crossings peak far below that and used \
             to clip anyway on the 8-bit chain"
        );
    }

    /// **The shipped shader implements the curve this module documents** — the
    /// one claim the three GPU-free tests above cannot make, since they exercise
    /// the CPU mirror and the frame path only ever runs the WGSL.
    ///
    /// A known linear frame is written straight into the tonemap's input (this is
    /// what its `COPY_DST` usage is for) and the mapped result compared against
    /// [`map`] within a byte. Values are chosen either side of the knee, so both
    /// the identity branch and the shoulder are covered.
    #[test]
    fn the_shader_implements_the_documented_curve() {
        use crate::render::context::RenderContext;

        const SIZE: u32 = 8;
        let ctx = match RenderContext::new_headless(SIZE, SIZE, true) {
            Ok(ctx) => ctx,
            Err(RenderError::RequestAdapter(_)) => {
                eprintln!("skipped: no GPU adapter on this runner (ADR-0016)");
                return;
            }
            Err(e) => panic!("headless context build failed: {e}"),
        };

        // f32 -> IEEE-754 binary16, for the ordinary magnitudes used below.
        fn to_half(x: f32) -> u16 {
            let bits = x.to_bits();
            let sign = ((bits >> 16) & 0x8000) as u16;
            let exponent = ((bits >> 23) & 0xff) as i32 - 127 + 15;
            let mantissa = ((bits & 0x7f_ffff) >> 13) as u16;
            sign | ((exponent as u16) << 10) | mantissa
        }

        for value in [0.25f32, 0.5, 0.8, 1.0, 2.0, 4.0] {
            let mut tonemap = Tonemap::new(&ctx.device, ctx.surface_format());
            let _ = tonemap.begin((SIZE, SIZE));
            let texture = tonemap
                .src_texture()
                .expect("the tonemap built its input")
                .clone();

            // A flat grey frame at `value`, so the max channel *is* `value` and
            // the hue-preserving scale reduces to the curve itself.
            let (half, opaque) = (to_half(value), to_half(1.0));
            let mut texels = Vec::new();
            for _ in 0..(SIZE * SIZE) {
                for channel in [half, half, half, opaque] {
                    texels.extend_from_slice(&channel.to_le_bytes());
                }
            }
            ctx.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &texels,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(SIZE * 8),
                    rows_per_image: Some(SIZE),
                },
                wgpu::Extent3d {
                    width: SIZE,
                    height: SIZE,
                    depth_or_array_layers: 1,
                },
            );

            let (target, view) =
                capture::create_target(&ctx.device, ctx.surface_format(), SIZE, SIZE);
            let (buffer, padded_bpr) = capture::create_readback(&ctx.device, SIZE, SIZE);
            let mut encoder = ctx
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("tonemap-curve"),
                });
            tonemap.resolve(&ctx.queue, &mut encoder, &view);
            capture::record_copy(&mut encoder, &target, &buffer, padded_bpr, SIZE, SIZE);
            ctx.queue.submit(std::iter::once(encoder.finish()));
            let image = capture::read_back(&ctx.device, &buffer, SIZE, SIZE, padded_bpr)
                .expect("read back the mapped frame");

            // The surface is sRGB, so the byte is the encoded form of what the
            // shader wrote — encode the expectation the same way rather than
            // decoding the measurement.
            let expected = encode_srgb(map(value)) * 255.0;
            let actual = image.rgba[0] as f32;
            assert!(
                (expected - actual).abs() <= 1.0,
                "the shader mapped {value} to byte {actual}, the documented curve \
                 says {expected:.1}"
            );
        }
    }

    /// Linear -> sRGB, the transfer function the 8-bit surface applies on write.
    fn encode_srgb(x: f32) -> f32 {
        if x <= 0.003_130_8 {
            12.92 * x
        } else {
            1.055 * x.powf(1.0 / 2.4) - 0.055
        }
    }

    // -----------------------------------------------------------------------
    // The bind-group layout enumeration (Plan 0045 Phase 4b)
    // -----------------------------------------------------------------------

    /// What one binding contributes to a layout's *shape*. Two layouts collide —
    /// in the sense the DX12 WARP aliasing hazard cares about — when their kinds
    /// match in order.
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    enum Kind {
        Texture,
        Sampler,
        Uniform,
        Storage,
    }

    /// Every spelling of an entry this repository uses, **longest first**. The
    /// scan takes the longest match at each byte, so `BufferBindingType::Uniform`
    /// is never read as the `BindingType::…` substring it contains.
    ///
    /// A new spelling belongs here. Leaving it out does not weaken the guard
    /// silently: the per-layout entry count below is derived independently, and a
    /// marker the scan missed makes the two disagree and fails the test.
    const MARKERS: &[(&str, Kind)] = &[
        ("BufferBindingType::Uniform", Kind::Uniform),
        ("BufferBindingType::Storage", Kind::Storage),
        ("BindingType::Sampler", Kind::Sampler),
        ("BindingType::Texture", Kind::Texture),
        ("lut_vertex_texture(", Kind::Texture),
        ("storage_entry(", Kind::Storage),
        ("gpu::texture(", Kind::Texture),
        ("gpu::sampler(", Kind::Sampler),
        ("gpu::uniform(", Kind::Uniform),
    ];

    fn rs_files(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        for entry in std::fs::read_dir(dir).expect("read a core/src directory") {
            let path = entry.expect("a directory entry").path();
            if path.is_dir() {
                rs_files(&path, out);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                out.push(path);
            }
        }
    }

    /// The text from `text[0]` up to the delimiter that closes an already-open
    /// `open`, ignoring every other character.
    fn balanced(text: &str, open: u8, close: u8) -> &str {
        let mut depth = 1i32;
        for (index, byte) in text.bytes().enumerate() {
            if byte == open {
                depth += 1;
            } else if byte == close {
                depth -= 1;
                if depth == 0 {
                    return &text[..index];
                }
            }
        }
        panic!("a bind-group-layout descriptor never closes");
    }

    /// How many entries a slice body holds, counted from its **top-level commas**
    /// — independent of [`MARKERS`], which is what makes the two a cross-check.
    fn entry_count(body: &str) -> usize {
        let (mut depth, mut count, mut filled) = (0i32, 0usize, false);
        for byte in body.bytes() {
            match byte {
                b'(' | b'[' | b'{' => depth += 1,
                b')' | b']' | b'}' => depth -= 1,
                b',' if depth == 0 => {
                    if filled {
                        count += 1;
                    }
                    filled = false;
                    continue;
                }
                _ => {}
            }
            if !byte.is_ascii_whitespace() {
                filled = true;
            }
        }
        if filled {
            count += 1;
        }
        count
    }

    /// `(label, kinds)` for every `create_bind_group_layout` call in one file.
    fn layouts_in(text: &str, file: &str) -> Vec<(String, Vec<Kind>)> {
        // Split so the constant does not match **itself** — this scan reads its
        // own file, and an anchor spelled whole here would open a "descriptor"
        // that runs to the end of the module.
        const CALL: &str = concat!(
            "create_bind_group_layout",
            "(&wgpu::BindGroupLayoutDescriptor {"
        );
        const ENTRIES: &str = "entries: &[";
        const LABELLED: &str = "label: Some(\"";

        let mut found = Vec::new();
        let mut cursor = 0usize;
        while let Some(hit) = text[cursor..].find(CALL) {
            cursor += hit + CALL.len();
            // Bound everything to this descriptor's own braces, so a call with a
            // computed label cannot borrow the next call's literal one.
            let desc = balanced(&text[cursor..], b'{', b'}');
            let label = match desc.find(LABELLED) {
                Some(at) => {
                    let from = at + LABELLED.len();
                    let end = desc[from..].find('"').expect("the label string closes");
                    desc[from..from + end].to_string()
                }
                // `lines/renderer.rs` formats its label per scene.
                None => format!("{file} (computed label)"),
            };
            let entries_at = desc.find(ENTRIES).expect("a layout declares entries") + ENTRIES.len();
            let body = balanced(&desc[entries_at..], b'[', b']');

            let mut kinds = Vec::new();
            let mut index = 0usize;
            while index < body.len() {
                let matched = MARKERS
                    .iter()
                    .find(|(marker, _)| body.as_bytes()[index..].starts_with(marker.as_bytes()));
                match matched {
                    Some((marker, kind)) => {
                        kinds.push(*kind);
                        index += marker.len();
                    }
                    None => index += 1,
                }
            }
            assert_eq!(
                kinds.len(),
                entry_count(body),
                "{file}: `{label}` declares {} entries but the scan recognized {} \
                 of them. Teach `MARKERS` the spelling this layout uses — an \
                 unrecognized entry would make the uniqueness check below blind \
                 to a real collision.",
                entry_count(body),
                kinds.len(),
            );
            found.push((label, kinds));
        }
        found
    }

    /// **The tonemap's bind-group layout is a shape nothing else in `core/src`
    /// has** — by enumerating every layout in the crate, not by asserting it in a
    /// comment (Plan 0045 Phase 4b).
    ///
    /// The comment is exactly what went wrong. Phase 3 shipped
    /// `[texture, sampler, uniform]` with a note saying no other live pipeline had
    /// that shape; `attractor-decay` had had it all along, built from the same
    /// three helpers. Nothing could catch that, because the claim was prose on a
    /// hazard surface (ADR-0021 / Plan 0020: WARP hands a pipeline whose layout
    /// matches another live one *the other pass's* resources).
    ///
    /// Only the tonemap is asserted on. Several older layouts genuinely do
    /// collide — `ink` with the fold, `trails` with the blend, four separate
    /// single-uniform groups — and those pairs are load-bearing history rather
    /// than this phase's business; they are printed so the picture is visible.
    #[test]
    fn the_tonemap_layout_is_a_shape_no_other_layout_in_core_has() {
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut files = Vec::new();
        rs_files(&src, &mut files);
        files.sort();

        let mut all: Vec<(String, Vec<Kind>)> = Vec::new();
        for file in &files {
            let text = std::fs::read_to_string(file).expect("read a core source file");
            let name = file
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("?")
                .to_string();
            all.extend(layouts_in(&text, &name));
        }

        for (label, kinds) in &all {
            eprintln!("{label:<34} {kinds:?}");
        }
        // The scan is the whole evidence, so a scan that found (nearly) nothing
        // must not read as a pass.
        assert!(
            all.len() >= 20,
            "only {} bind-group layouts found across core/src — the scan is not \
             seeing the crate",
            all.len()
        );

        let mine = all
            .iter()
            .find(|(label, _)| label == "tonemap-bind-layout")
            .expect("the tonemap's own layout is in the enumeration");
        let sharers: Vec<&str> = all
            .iter()
            .filter(|(label, kinds)| kinds == &mine.1 && label != &mine.0)
            .map(|(label, _)| label.as_str())
            .collect();
        assert!(
            sharers.is_empty(),
            "`tonemap-bind-layout` is {:?}, and so is {sharers:?}. This pass runs \
             on every frame beside whatever the preset switched on, so it is the \
             most exposed pipeline in the engine to the WARP identical-layout \
             aliasing hazard. Move it to a shape this enumeration shows is free — \
             and fix the comment in `Resources::build`, which is the thing that \
             was wrong last time.",
            mine.1
        );
    }
}
