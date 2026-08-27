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
//! **Off by default (passthrough).** When `trails <= 0` the
//! [`PostChain`](super::post::PostChain) skips this stage entirely: no
//! offscreen target, no pipelines, so the NFR §1 iGPU floor pays nothing, and
//! (like the background pass) the DX12 WARP software adapter never sees a
//! coexisting feedback pipeline during the no-trails captures. When active,
//! the pipelines build lazily and the accumulation is **reset on the capture
//! scene-rebuild**, so a headless capture stays a pure function of its inputs
//! (NFR §6).
//!
//! The `surface_format` this stage is built with is the **composite's** format
//! (`Rgba16Float`, ADR-0046), not the surface's: the max-decay already ran in
//! linear light — an 8-bit *sRGB* target blends linearly — so the arithmetic is
//! the same, and a bright head does **not** clip at 1.0 on its way into the
//! accumulation, which is `Rgba16Float` either way.
//!
//! The composite runs at an internal resolution that **follows the render target**
//! (ADR-0034), quantized to a 256 px step and capped — see
//! [`internal_grid_size`](super::post::internal_grid_size), rather than a fixed
//! 1280x720, which on anything above 720p upscales the whole frame: line geometry
//! rasterized at full resolution is thrown away and comes back soft, and the
//! preset lane's only recourse is to drop `trails` from every line preset to get
//! its sharpness back. Following the target is what lets those presets keep both.
//!
//! # The past moves: the accumulation is read through a transform (ADR-0048)
//!
//! The feedback pass does **not** sample `prev` at the identical uv. It samples
//! it through an **inverse per-frame affine** — `fb_zoom`, `fb_rotate`, `fb_dx`,
//! `fb_dy` about the bindable centre `fb_center_x`/`fb_center_y` — so a zoom is a
//! tunnel, a rotation a spiral, a translation a directional smear. Inverse,
//! because a destination pixel asks "where was I last frame"; the forward motion
//! the eye sees is the one the params name.
//!
//! Three properties hold that up, and each is load-bearing:
//!
//! - **Every rate is per-second, on the injected real `dt`** (ADR-0019): `fb_zoom`
//!   is a factor per second applied as `zoom^dt`, `fb_rotate` is rad/s, `fb_dx`/
//!   `fb_dy` are units/s. The same edit normalizes `fade` to `fade^(dt / (1/60))`,
//!   the form the attractor's trail uses. Applied once per *frame* instead, a
//!   trail is a third as long at 144 Hz as at 48 Hz. At the capture `dt` the
//!   exponent is exactly `1.0` and the factor is exactly `fade`, so no golden
//!   depends on the rate.
//! - **The transform is aspect-corrected from the RENDER TARGET** (ADR-0037). The
//!   accumulation grid is quantized to a 256 px step, so its own aspect is not the
//!   target's; a rotation computed in grid-uv space would shear. The centred
//!   coordinate has its `x` scaled by `surface.0 / surface.1` — the value
//!   [`resolve`](PostStage::resolve) is handed, never anything derived from
//!   [`Resources::size`] — rotated there, and scaled back on the way out.
//! - **Identity is bit-exact, not approximately so.** Round-tripping a uv through
//!   `* aspect` and `/ aspect` is not the identity in `f32`, so the shader
//!   `select`s between the transformed uv and `in.uv` on a CPU-computed flag that
//!   is `0` whenever every `fb_*` sits at its default. An unbound preset therefore
//!   samples the literal `in.uv` it always did.
//!
//! **Off-frame reads are transparent, not clamped** (the edge policy ADR-0048
//! leaves open). A zoom-out, a pan, or a rotation about an off-centre
//! `fb_center_*` all reach outside the accumulation, and the two candidates read
//! very differently: `ClampToEdge` re-deposits the border texel every frame, so
//! the edge row smears inward and compounds into a permanent bar of colour — the
//! same defect ADR-0047 had to clamp out of the kaleidoscope's fold. Evaluated at
//! a **portrait** target (ADR-0047's lesson: a non-16:9 shape is where an edge
//! policy shows), the clamp streaks the two long edges hardest, precisely where a
//! tunnel wants empty space to travel into. Sampling outside the unit square
//! therefore contributes **nothing** — the past ends at the frame's edge, and what
//! lies beyond it is the backdrop showing through, which is what makes a zoom-out
//! read as depth. It is a shader test rather than `AddressMode::ClampToBorder`
//! because that address mode is an optional wgpu feature and this must work on
//! every adapter we ship.

// Hot-path panic-denial pragma (Plan 0002 Phase 2; render/ is scanned by the
// hygiene guard). The trails stage encodes its passes every displayed frame.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

use super::feedback::{self, Deposit, FeedbackConfig, PingPongField, Transform};
use super::gpu;
use super::post::{Fold, PostStage, internal_grid_size};

/// `trails` param default — off, so an unbound preset pays nothing.
const DEFAULT_TRAILS: f32 = 0.0;

/// Hard ceiling on the decay factor: `1.0` would never fade (an ever-brightening
/// smear), so keep it strictly below.
const MAX_FADE: f32 = 0.98;

/// The reference frame duration the `fade` exponent is expressed against: `fade`
/// is retention **per 1/60 s**, so the per-frame factor is `fade^(dt / this)`.
///
/// This is the attractor trail's long-standing form
/// ([`particles::encode`](super::scenes::particles)), adopted here rather than
/// invented: one vocabulary, two buffers (ADR-0048). Written as a division by
/// [`FALLBACK_DT`](super::scenes::FALLBACK_DT) rather than a multiplication by 60
/// so the exponent at the capture `dt` is `x / x` — **exactly** `1.0` in IEEE,
/// never `1.0000001`.
const FADE_REFERENCE_DT: f32 = super::scenes::FALLBACK_DT;

/// Fade-and-accumulate body. `VsOut`/`vs_main` come from
/// [`gpu::FULLSCREEN_VS_UV_FLIPPED`] — this pass samples what another pass
/// rendered, so it needs the Y flip.
const TRAILS_SHADER: &str = r#"
struct Feedback {
    // x: decay factor, y: occlude (present pass only), z: transform-active flag,
    // w: 1 = additive deposit
    v:  vec4<f32>,
    // x: cos(theta), y: sin(theta), z: 1/scale, w: render-target aspect
    xf: vec4<f32>,
    // x,y: this frame's translation in isotropic units, z,w: the centre in uv
    tr: vec4<f32>,
    // x: warp kind selector, y: this frame's warp strength
    wp: vec4<f32>,
}

@group(0) @binding(0) var<uniform> u: Feedback;
@group(0) @binding(1) var t_composited: texture_2d<f32>;
@group(0) @binding(2) var t_accum: texture_2d<f32>;
@group(0) @binding(3) var samp: sampler;

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let cur = textureSample(t_composited, samp, in.uv);

    // `select`, not `if`: the identity path must sample the LITERAL `in.uv`, and
    // `(x * aspect) / aspect` is not `x` in f32. Both arms are pure arithmetic on
    // uniforms, so evaluating the unused one costs a few ALU and no divergence —
    // which also keeps `textureSample` out of non-uniform control flow.
    let moved = u.v.z != 0.0;
    let suv = select(in.uv, lmv_source_uv(in.uv, u.xf, u.tr, u.wp), moved);
    let inside = select(1.0, lmv_inside(suv), moved);
    let faded = textureSample(t_accum, samp, suv) * inside * u.v.x;

    // The deposit (ADR-0048). `max` is the default and the arm that ran before the
    // choice existed — the current frame at full brightness, the past fading by
    // `fade`. `add` sums instead, so overlapping echoes brighten; its geometric
    // series is bounded by 1/(1 - fade) and rolls off through the tonemap, which
    // is only true because the composite runs in linear light above 1.0 (ADR-0046).
    //
    // On ALL FOUR channels (ADR-0055). Alpha is coverage and it decays on the same
    // schedule as colour, so a trail releases the backdrop at the rate it dims.
    // Forcing alpha to 1 here held `bg_*` out of every pixel the trail had ever
    // touched, permanently.
    return select(max(cur, faded), cur + faded, u.v.w != 0.0);
}
"#;

/// Present body; same Y-flipped prelude as [`TRAILS_SHADER`].
///
/// It binds the **same buffer** the feedback pass reads, and declares only the
/// first `vec4` of it. That is legal and deliberate: a uniform binding may be
/// larger than the struct a shader declares over it, so ADR-0048's transform
/// terms cost this pass nothing and its bind-group layout — whose *shape* is the
/// WARP-sensitive part (see below) — is untouched by them.
const PRESENT_SHADER: &str = r#"
struct Fade { v: vec4<f32> } // x: fade factor (unread here), y: occlude

// SAMPLER, UNIFORM, TEXTURE — a deliberate order, and the last permutation of
// the three this crate had free. Two bind-group layouts with the same shape
// mis-render when they coexist on the DX12 WARP software adapter (ADR-0058,
// which is where that hazard is recorded), and `occlude` needed a uniform here
// where there had been none.
// Every other arrangement was taken: `[texture, sampler, uniform]` is
// `attractor-decay`, `[uniform, texture, sampler]` is `ink`, `[texture, uniform,
// sampler]` is `tonemap`, `[sampler, texture, uniform]` is `bloom-up`, `[uniform,
// sampler, texture]` is `bloom-bright`. A SECOND GROUP holding the uniform alone
// was tried first and is exactly what does not work: `[uniform]` is
// `background-bind-layout`'s shape, and on WARP this pass read the backdrop's
// buffer instead of this one — `occlude` measurably did nothing there while
// working on the hardware adapter. Pinned by
// `the_two_present_layouts_added_for_occlude_are_shapes_nothing_else_has`.
@group(0) @binding(0) var samp: sampler;
@group(0) @binding(1) var<uniform> u: Fade;
@group(0) @binding(2) var t_accum: texture_2d<f32>;

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    // Alpha travels with the colour (ADR-0055) — the accumulation is premultiplied
    // and the chain composites over the backdrop downstream.
    let c = textureSample(t_accum, samp, in.uv);
    // …scaled by how much of that coverage the backdrop resolves against
    // (ADR-0085). `1.0` at the fold into a scratch offscreen and by default, where
    // this multiply is exact and the frame is what it always was.
    return vec4<f32>(c.rgb, c.a * u.v.y);
}
"#;

/// The one uniform both passes read — see [`TRAILS_SHADER`] for the component
/// map. One buffer, one write per frame, two shaders taking different parts of it.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Feedback {
    v: [f32; 4],
    xf: [f32; 4],
    tr: [f32; 4],
    wp: [f32; 4],
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
    feedback_uniform: wgpu::Buffer,
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
        let feedback_uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("trails-feedback"),
            size: std::mem::size_of::<Feedback>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // ADR-0048's transform, concatenated in rather than written here: this
        // is the same WGSL the attractor's trail blit compiles, so the two sinks
        // cannot drift apart on what `fb_rotate` means.
        let trails_shader = gpu::fullscreen_shader(
            device,
            "trails-shader",
            gpu::FULLSCREEN_VS_UV_FLIPPED,
            &format!("{}{TRAILS_SHADER}", feedback::TRANSFORM_WGSL),
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
                        resource: feedback_uniform.as_entire_binding(),
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
        // Sampler, uniform, texture — the order is the shape, and the shape is the
        // point. See `PRESENT_SHADER`. The uniform is the SAME buffer the feedback
        // pass reads for `fade`, so one write per frame feeds both passes and the
        // two shaders take different components of it.
        let present_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("trails-present-bind-layout"),
            entries: &[
                gpu::sampler(0),
                gpu::uniform(1, wgpu::ShaderStages::FRAGMENT),
                gpu::texture(2, true),
            ],
        });
        let make_present_bg = |accum_view: &wgpu::TextureView, label: &str| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(label),
                layout: &present_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Sampler(&sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: feedback_uniform.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(accum_view),
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
            feedback_uniform,
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
    /// This frame's `fb_*` transform (ADR-0048), reset to
    /// [`Transform::IDENTITY`] each frame with every other param.
    transform: Transform,
    /// The active preset's `[feedback]` table — the warp kind and the deposit
    /// blend. **Structural**, so unlike [`transform`](Self::transform) it is set
    /// once at preset load and is *not* reset per frame.
    config: FeedbackConfig,
    /// The real elapsed seconds of the frame being resolved, injected by
    /// [`PostChain::set_dt`](super::post::PostChain::set_dt).
    ///
    /// Every rate this stage applies — the `fade` decay and all of `fb_*` — is
    /// per-second and scaled by it, so the look is the same at 48 and 144 Hz
    /// (ADR-0019). Defaulted to the capture step so a stage driven by a test that
    /// never injects one behaves exactly as the pre-ADR-0048 stage did.
    dt: f32,
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
///
/// `trails` is the amount; the `fb_*` six are ADR-0048's transform on what the
/// accumulation already holds. They are declared here — on the stage that owns the
/// buffer — and every one of them defaults to the identity, so a preset binding
/// none of them renders exactly what it rendered before they existed.
pub const PARAMS: &[&str] = &[
    "trails",
    "fb_zoom",
    "fb_rotate",
    "fb_dx",
    "fb_dy",
    "fb_center_x",
    "fb_center_y",
    "fb_warp",
];

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
            transform: Transform::IDENTITY,
            config: FeedbackConfig::default(),
            dt: FADE_REFERENCE_DT,
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

    /// Reset the `trails` amount and the `fb_*` transform to their defaults (each
    /// frame, before the active preset's bindings are routed).
    fn reset_params(&mut self) {
        self.amount = DEFAULT_TRAILS;
        self.transform = Transform::IDENTITY;
    }

    /// Apply one named parameter, returning whether this stage owned the name.
    fn set_param(&mut self, name: &str, value: f32) -> bool {
        if name == "trails" {
            self.amount = value;
            return true;
        }
        // The seven `fb_*` names are ADR-0048's shared vocabulary — delegated
        // rather than re-matched here, so this sink and the attractor's cannot
        // disagree about what `fb_dx` means.
        self.transform.set_param(name, value)
    }

    /// Take this frame's real elapsed seconds — the base every per-second rate
    /// here is scaled by (ADR-0019/ADR-0048). Non-finite or negative values are
    /// dropped rather than stored: a `NaN` would poison the decay exponent, and
    /// the previous frame's step is a far better guess than a broken one.
    fn set_dt(&mut self, dt: f32) {
        if dt.is_finite() && dt >= 0.0 {
            self.dt = dt;
        }
    }

    /// Take the active preset's `[feedback]` table (ADR-0048) — the warp kind and
    /// the deposit blend. **Once at preset load, off the hot path**, like
    /// `Scene::configure`: both are shader-path choices rather than quantities,
    /// and a per-frame one would hard-cut the look.
    fn set_feedback(&mut self, cfg: FeedbackConfig) {
        self.config = cfg;
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
    /// `surface` is **the render target's size, and the only aspect this stage may
    /// take** (ADR-0037). Both blits are still normalized stretches with no
    /// geometry of their own, but ADR-0048's transform rotates in a centred
    /// coordinate, and a rotation is only a rotation in isotropic units: reading
    /// the shape off [`Resources::size`] — a 256 px-quantized grid whose aspect is
    /// *not* the target's — would shear the accumulation on any target the policy
    /// does not return aspect-exact. That is the mistake ADR-0037 exists for.
    fn resolve(
        &mut self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        out: &wgpu::TextureView,
        surface: (u32, u32),
        fold: Fold,
    ) -> u32 {
        let dt = self.dt;
        let transform = self.transform;
        let aspect = surface.0 as f32 / surface.1.max(1) as f32;
        let Some(res) = self.res.as_mut() else {
            return 0;
        };
        let fade = self.amount.clamp(0.0, MAX_FADE);
        // Retention per 1/60 s, raised to the `dt`-relative power, so a trail runs
        // the same wall-clock length at any refresh (ADR-0048 — the form the
        // attractor's trail has always used). The `== 1.0` arm is not an
        // optimization: `powf` is not required to return `x` for an exponent of
        // exactly one, and the whole golden suite rests on this factor being
        // *bit*-identical to `fade` at the capture step.
        let exponent = dt / FADE_REFERENCE_DT;
        let decay = if exponent == 1.0 {
            fade
        } else {
            fade.powf(exponent)
        };
        let moved = !transform.is_identity(self.config.warp);
        let [xf, tr, wp] = transform.pack(dt, aspect, self.config.warp);
        let additive = self.config.blend == Deposit::Add;
        queue.write_buffer(
            &res.feedback_uniform,
            0,
            bytemuck::bytes_of(&Feedback {
                // One write, two passes: `decay` for the feedback below, `occlude`
                // for the present at the bottom (ADR-0085). The present applies it
                // unconditionally — `Fold::Own` is a literal 1.0.
                v: [
                    decay,
                    fold.alpha_scale(),
                    f32::from(u8::from(moved)),
                    f32::from(u8::from(additive)),
                ],
                xf,
                tr,
                wp,
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
