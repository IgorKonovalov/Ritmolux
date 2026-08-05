//! Bloom (ADR-0046, Plan 0045 Phase 4): the third and last
//! [`PostStage`](super::post::PostStage) — a threshold bright-pass, a separable
//! blur pyramid, and an additive recombine, so light that is genuinely *over
//! range* spills into the pixels around it.
//!
//! # Why this could not be built before Phase 3
//!
//! A bright-pass is a **question about values above 1.0**, and until Phase 3 the
//! composite had none: every hand-off wrote through an 8-bit gate, so a stroke at
//! brightness 4 and a stroke at brightness 1 arrived here as the same white. A
//! threshold over that frame selects *area*, not *energy*, which is exactly the
//! "lifted floor makes the halo flat paint" failure design-backlog 0005 measured
//! (ADR-0046, Alternative B). Now the stage reads
//! [`COMPOSITE_FORMAT`](super::COMPOSITE_FORMAT) linear light and
//! `bloom_threshold` defaults to **1.0** — the value that means "bloom only what
//! the display could not have shown anyway".
//!
//! # Off by default, and therefore absent by default
//!
//! `bloom_amount` defaults to 0, which reports [`active`](super::post::PostStage::active)
//! `false`, which drops the stage from the frame entirely — no offscreens, no
//! pyramid, no pipelines. That is the existing skip discipline (see
//! [`trails`](super::trails) and [`kaleidoscope`](super::kaleidoscope)) and it is
//! what makes this phase's "every existing baseline is byte-identical" claim
//! structural rather than a tolerance: on a preset that does not bind
//! `bloom_amount` the GPU never hears of this module.
//!
//! # The pyramid
//!
//! With `N` = [`TierConfig::bloom_levels`](super::TierConfig::bloom_levels)
//! (clamped so no level falls under [`MIN_LEVEL_PX`]), each level is **half** the
//! previous one, and every level is blurred with a **separable** 5-tap binomial
//! kernel — horizontal, then vertical:
//!
//! ```text
//!   src ──bright──> mip0 ──H──> tmp0 ──V──> mip0 ──/2──> mip1 ──H──> tmp1 ──V──> mip1 ── …
//!                     ▲                                   ▲
//!                     └───────── += s · up(mip1) ─────────┘   (additive, coarse to fine)
//!   out = src + amount/(1 + s + s² + …) * mip0
//! ```
//!
//! Each level is **halved into** by its own reduction pass and then blurred in
//! place, horizontally and vertically, at its own resolution — so both halves of
//! the separable kernel step one texel of a source the same size as their target.
//! Folding the reduction into the horizontal pass saves a pass and costs exactly
//! that symmetry; [`DOWN_SHADER`] records what that looked like. The up-chain then
//! folds coarse levels into finer ones with the geometric weight `s`, so `mip0`
//! ends up holding `L0 + s·L1 + s²·L2 + …`.
//!
//! **`s` is where `bloom_radius` acts, and the kernel is fixed at one texel per
//! level** — [`MIN_SCATTER`] records why, and what a five-tap kernel does when a
//! radius is allowed to stretch it instead. Dividing the recombine by the same
//! geometric sum is what keeps `bloom_amount` a brightness and `bloom_radius` a
//! width, independently of each other and of the tier's level count.
//!
//! Sizes cascade from the stage's internal grid, which follows the render target
//! under the shared policy (ADR-0034). The pyramid is therefore ~2/3 of one
//! grid-sized float texture in total (two textures per level, each level a quarter
//! of the last): about 11 MB at the floor cap, against the ~66 MB per chain that
//! [`TierConfig::post_cap`](super::TierConfig::post_cap) already accounts for.
//!
//! # The bind layouts are chosen, not stylistic
//!
//! The DX12 WARP software adapter hands a pipeline whose bind-group layout matches
//! another live one *the other pass's resources* — twice-observed before this
//! stage (ADR-0021 / Plan 0020, then the tonemap in Phase 3, where the wrong
//! uniform reproduced to the byte) and a third time here, on the blur, which is
//! why it now binds none (see [`Resources`]). Every layout this stage adds is a
//! shape no other live pipeline in the engine has:
//!
//! | pass                     | layout                               |
//! |--------------------------|--------------------------------------|
//! | bright                   | `uniform, sampler, texture`          |
//! | blur H / blur V / reduce | `sampler, texture`                   |
//! | up                       | `sampler, texture, uniform`          |
//! | recombine                | `sampler, texture, texture, uniform` |
//!
//! They look arbitrary because they are — the requirement is only that they be
//! *distinct*, and the natural orderings are all taken.
//!
//! # The recombine adds light, and light is not coverage
//!
//! This stage is the only one in the chain whose output can exceed alpha 1, and
//! that is a defect rather than a feature — [`MIX_SHADER`] clamps it (Plan 0045
//! Phase 4b). The chain carries **premultiplied alpha** (ADR-0055) and the
//! recombine is the pass that blends into the chain's destination, which is where
//! the backdrop has already been painted. So the sum it writes is a blend source:
//!
//! ```text
//!   dst' = src + dst * (1 - src.a)
//! ```
//!
//! `src.rgb` above 1.0 is the whole point of the linear region — the tonemap turns
//! it back into a picture. `src.a` above 1.0 is not analogous: it drives
//! `1 - src.a` **negative**, and the backdrop is then subtracted under the frame's
//! brightest regions rather than covered by them. Anywhere the upstream frame is
//! already opaque (`base.a = 1`) a non-zero halo does exactly that, which is why
//! the symptom is a dark hole tracking the bloom.
//!
//! It shipped because every bloom fixture runs `bg_bright = 0` on purpose (a black
//! backdrop makes the baseline measure the pyramid rather than the backdrop) — and
//! on a black backdrop subtracting the backdrop and covering it are the same
//! picture. `a_backdrop_under_an_active_halo_only_ever_adds_light` below is the
//! guard that closes it, the same shape as `core/tests/kaleidoscope.rs`'s for the
//! fold; **it reads the linear composite rather than a capture**, and its docs say
//! why a display-byte version of the same assertion cannot be written.
//!
//! # Every pass is orientation-preserving
//!
//! All of them use [`gpu::FULLSCREEN_VS_UV_FLIPPED`], which despite the name is
//! the **identity** blit: clip-space y is up and a render target's y is down, so
//! the flip is what makes framebuffer row *n* read texture row *n*. Using the
//! unflipped prelude anywhere in here would mirror that level and the additive
//! up-chain would then add a halo to the wrong half of the frame.

// Hot-path panic-denial pragma (Plan 0002 Phase 2; render/ is scanned by the
// hygiene guard). The pyramid encodes 4N passes on every frame it is active.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

use super::gpu;
use super::post::{Fold, PostStage, internal_grid_size};

/// `bloom_amount` default — **off**, so an unbound preset never builds the stage.
const DEFAULT_AMOUNT: f32 = 0.0;

/// `bloom_threshold` default — 1.0, the display's own ceiling.
///
/// This is the number that makes bloom mean something specific in linear light:
/// at 1.0 the bright-pass selects exactly the light the 8-bit surface could not
/// have carried anyway, so a preset that switches bloom on gets halos around the
/// places that *were* clipping and nothing else. Lower it to bloom mid-tones too.
const DEFAULT_THRESHOLD: f32 = 1.0;

/// `bloom_radius` default — the middle of the scatter range, a halo that reads as
/// a glow around the figure rather than as a wash over the frame.
const DEFAULT_RADIUS: f32 = 1.0;

/// Ceiling on `bloom_amount`. Four times the pyramid's own energy is already a
/// blown-out look; past it the frame is halo and the tonemap's shoulder is doing
/// all the work.
const MAX_AMOUNT: f32 = 4.0;

/// Ceiling on `bloom_threshold`. Above ~8 the bright-pass selects nothing on any
/// frame the rest of the engine can produce.
const MAX_THRESHOLD: f32 = 8.0;

/// Bounds on `bloom_radius`. The whole range is usable — it is a *weighting*, not
/// a kernel width (see [`scatter`]) — so both ends are looks rather than limits.
const MIN_RADIUS: f32 = 0.0;
const MAX_RADIUS: f32 = 4.0;

/// The geometric weight the up-chain gives each coarser pyramid level, at the two
/// ends of `bloom_radius`.
///
/// **`bloom_radius` does not scale the blur kernel, and this is the second thing
/// this stage got wrong.** The obvious implementation multiplies each level's tap
/// spacing by the radius — but the kernel is five taps, so at a spacing above
/// about one texel it stops being a blur and becomes five *copies* of whatever it
/// is sampling. At `bloom_radius = 3` on a 256 px capture that rendered the
/// fixture's star as a neat column of five stars, spaced by the coarsest level's
/// tap step. It was invisible at 512 px, where the star is wide enough that the
/// copies overlap back into something smooth, and invisible in the 160x100
/// baseline, which is too small to resolve them.
///
/// So the kernel is fixed at one texel per level and the radius weights the
/// *pyramid* instead: each up-chain step multiplies by `s`, so the finest level
/// accumulates `L0 + s·L1 + s²·L2 + …`. Since level `i` is `2^i` times wider than
/// level 0, a larger `s` moves the halo's energy outward — which is what a radius
/// is supposed to do — and it does so with a *sum of Gaussians*, which cannot
/// alias whatever the level count is.
///
/// 0.95 rather than 1.0 at the top so the coarsest level never fully dominates
/// the sum; 0.25 at the bottom for a halo that hugs its source.
const MIN_SCATTER: f32 = 0.05;
const MAX_SCATTER: f32 = 1.0;

/// The up-chain's per-level weight for a given `bloom_radius`.
fn scatter(radius: f32) -> f32 {
    let t = (radius / MAX_RADIUS).clamp(0.0, 1.0);
    MIN_SCATTER + (MAX_SCATTER - MIN_SCATTER) * t
}

/// What the accumulated pyramid sums to at scatter `s` over `levels` levels —
/// `1 + s + s² + …`, the geometric series the recombine divides out.
///
/// Without it `bloom_amount` would not mean one thing: a wider halo would also be
/// a brighter one, and a preset easing `bloom_radius` on the beat would pump the
/// frame's overall exposure as a side effect.
fn pyramid_sum(s: f32, levels: usize) -> f32 {
    if s >= 1.0 {
        return levels.max(1) as f32;
    }
    ((1.0 - s.powi(levels.max(1) as i32)) / (1.0 - s)).max(1.0)
}

/// The smallest a pyramid level may get on either axis. Levels past this are
/// dropped, so a small render target quietly runs a shallower pyramid instead of
/// allocating 1x1 textures whose blur is a no-op.
const MIN_LEVEL_PX: u32 = 4;

/// How wide the bright-pass's soft knee is, as a fraction of the threshold.
///
/// A hard `max(m - t, 0)` is continuous but not smooth at `m == t`, and
/// `bloom_threshold` is an ordinary bindable that presets will drive off audio and
/// ease through `[smoothing]` — so a stroke sitting near the threshold would
/// flicker in and out as the eased value crossed it. The knee makes the transition
/// C1 at the cost of blooming a little below the nominal threshold.
const KNEE_FRACTION: f32 = 0.5;

/// Floor on that knee, so a `bloom_threshold` of 0 still has one.
const MIN_KNEE: f32 = 0.05;

/// The knee band for a given threshold.
fn knee_band(threshold: f32) -> f32 {
    (threshold * KNEE_FRACTION).max(MIN_KNEE)
}

/// The pyramid's level sizes for `grid`, deepest-allowed first level onward.
///
/// **Pure**, so the level-count policy — halving, the [`MIN_LEVEL_PX`] floor, and
/// the "always at least one level" guarantee — is unit-testable without a GPU, and
/// so the chain's compare-first rebuild keeps resting on a pure function of
/// `surface`.
fn level_sizes(grid: (u32, u32), max_levels: u32) -> Vec<(u32, u32)> {
    let mut sizes = Vec::new();
    let (mut w, mut h) = grid;
    for _ in 0..max_levels.max(1) {
        w = (w / 2).max(1);
        h = (h / 2).max(1);
        // Always take the first level: a stage that built no pyramid at all would
        // have to be a second code path through `resolve`, and a 2x2 blur is
        // cheap enough not to be worth one.
        if !sizes.is_empty() && (w < MIN_LEVEL_PX || h < MIN_LEVEL_PX) {
            break;
        }
        sizes.push((w, h));
    }
    sizes
}

/// Bright-pass body. `VsOut`/`vs_main` come from [`gpu::FULLSCREEN_VS_UV_FLIPPED`].
const BRIGHT_SHADER: &str = r#"
struct Bright { v: vec4<f32> } // x: threshold, y: knee band, z: exposure

@group(0) @binding(0) var<uniform> u: Bright;
@group(0) @binding(1) var samp: sampler;
@group(0) @binding(2) var t_src: texture_2d<f32>;

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    // Negative light is not a thing; a float target can hold it, so clamp before
    // it can invert the weight below.
    let c = max(textureSample(t_src, samp, in.uv), vec4<f32>(0.0));
    // The BRIGHTEST CHANNEL, the same measure the tonemap's roll-off uses. A luma
    // measure would under-select saturated blues (luma 0.114) whose blue channel
    // is well over range, which is most of the neon palette.
    //
    // Scaled by the frame's EXPOSURE (ADR-0080), so the comparison below is
    // against the light the display will be asked to show rather than against the
    // scene's own linear units. `bloom_threshold` then means one thing at any
    // stop; before this, a preset at exposure 0.03 put its whole figure over every
    // threshold it could ask for and 0.95 against the 8.0 ceiling rendered alike.
    // At the default stop of exactly 1.0 this multiply is the identity.
    let m = max(c.r, max(c.g, c.b)) * u.v.z;
    let over = max(m - u.v.x, 0.0);
    let band = max(u.v.y, 1e-4);
    // Smooth the first `band` of over-range so an eased `bloom_threshold` sweeping
    // past a stroke does not pop it into existence.
    let soft = smoothstep(0.0, 1.0, clamp(over / band, 0.0, 1.0));
    // `w` is a dimensionless FRACTION of `m` — `over` and `m` are both in exposed
    // units, so the stop cancels out of it. That is why what leaves this pass is
    // `c * w` and not `c * u.v.z * w`: only the *selection* moved to
    // display-referred units. The light itself stays in the scene's linear
    // currency, because the recombine adds it back onto an un-exposed frame and
    // the tonemap applies the stop to the sum, downstream, exactly as before.
    let w = clamp(over * soft / max(m, 1e-4), 0.0, 1.0);
    // COLOUR AND ALPHA together (ADR-0055): the chain carries premultiplied alpha
    // and the recombine adds this on top of the source, so the halo has to be a
    // premultiplied value or it would add light without adding any coverage.
    return c * w;
}
"#;

/// The 5-tap binomial kernel and the two directions it is applied in. Prepended
/// to both blur bodies so there is exactly one copy of the weights.
///
/// **The tap step comes from `textureDimensions`, not from a uniform.** That is
/// the fix for the defect recorded on [`Resources`]: it makes the step a property
/// of the texture being read rather than of a buffer that has to arrive at the
/// right pass, and it makes the two directions *provably* symmetric — both are one
/// texel of the same same-sized source, computed by the same line of shader.
const BLUR_PRELUDE: &str = r#"
@group(0) @binding(0) var samp: sampler;
@group(0) @binding(1) var t_src: texture_2d<f32>;

// Binomial (1, 4, 6, 4, 1) / 16 — the discrete Gaussian at this width, and the
// widest kernel five taps can carry without breaking into separate copies of the
// source. Weights sum to exactly 1, so the pass preserves energy.
const W0: f32 = 0.375;
const W1: f32 = 0.25;
const W2: f32 = 0.0625;

fn blur(uv: vec2<f32>, d: vec2<f32>) -> vec4<f32> {
    var acc = textureSample(t_src, samp, uv) * W0;
    acc = acc + (textureSample(t_src, samp, uv + d)
               + textureSample(t_src, samp, uv - d)) * W1;
    acc = acc + (textureSample(t_src, samp, uv + d * 2.0)
               + textureSample(t_src, samp, uv - d * 2.0)) * W2;
    return acc;
}

fn texel() -> vec2<f32> {
    return 1.0 / vec2<f32>(textureDimensions(t_src));
}
"#;

/// Horizontal half of the separable blur. Source and target are the same size.
const BLUR_H_SHADER: &str = r#"
@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    return blur(in.uv, vec2<f32>(texel().x, 0.0));
}
"#;

/// Vertical half of the separable blur. Source and target are the same size.
const BLUR_V_SHADER: &str = r#"
@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    return blur(in.uv, vec2<f32>(0.0, texel().y));
}
"#;

/// The 2:1 reduction between levels, kept as its own pass.
///
/// Folding it into the horizontal blur would save a pass and cost the symmetry:
/// the step would then be one texel of a source *twice* the target's width, so the
/// horizontal kernel would be half the vertical one's and every halo would come
/// out 2:1 elongated (it did — see [`Level::size`]).
///
/// A bare bilinear fetch is an exact 2x2 box average at precisely 2:1, and the
/// source has already been blurred in both directions at its own resolution, so
/// there is nothing left above the target's Nyquist for this to alias.
const DOWN_SHADER: &str = r#"
@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    return textureSample(t_src, samp, in.uv);
}
"#;

/// The coarse-to-fine additive fold, weighted by the scatter `s` that
/// `bloom_radius` sets. Bilinear upsample, no kernel of its own.
///
/// Every up pass in a frame wants the **same** `s`, so they all share one uniform
/// buffer — which is what keeps this pass out of the failure mode the blur passes
/// were in.
const UP_SHADER: &str = r#"
struct Up { v: vec4<f32> } // x: scatter

@group(0) @binding(0) var samp: sampler;
@group(0) @binding(1) var t_src: texture_2d<f32>;
@group(0) @binding(2) var<uniform> u: Up;

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    return textureSample(t_src, samp, in.uv) * u.v.x;
}
"#;

/// Recombine body: the untouched source plus the finest pyramid level, scaled.
/// Same prelude.
const MIX_SHADER: &str = r#"
struct Mix { v: vec4<f32> } // x: amount / the pyramid's geometric sum

@group(0) @binding(0) var samp: sampler;
@group(0) @binding(1) var t_src: texture_2d<f32>;
@group(0) @binding(2) var t_bloom: texture_2d<f32>;
@group(0) @binding(3) var<uniform> u: Mix;

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let base = textureSample(t_src, samp, in.uv);
    let halo = textureSample(t_bloom, samp, in.uv);
    // Additive in linear light, and above 1.0 freely — the tonemap downstream is
    // what turns the sum back into a picture. This is the whole reason the stage
    // sits inside the linear region rather than after it.
    let sum = base + halo * u.v.x;
    // COLOUR ABOVE 1 IS LIGHT; ALPHA ABOVE 1 IS A HOLE. See the module docs: this
    // draw blends OVER the backdrop, and a source alpha past 1 makes the blend's
    // `1 - src.a` factor negative, which *subtracts* the backdrop under exactly
    // the frame's brightest regions. Clamp the coverage, keep the light.
    return vec4<f32>(sum.rgb, clamp(sum.a, 0.0, 1.0));
}
"#;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct V4 {
    v: [f32; 4],
}

/// One pyramid level: its two views and the four passes that touch them.
/// The textures themselves are held by [`Resources::_pyramid`].
/// A level carries no size of its own: the tap step is derived in the shader from
/// `textureDimensions` (see [`BLUR_PRELUDE`]), and the sizes the textures were
/// built at are [`level_sizes`]'s, which is pure and tested directly.
struct Level {
    mip: wgpu::TextureView,
    tmp: wgpu::TextureView,
    /// `mip[i]` -> `tmp[i]`, horizontal.
    h_bind: wgpu::BindGroup,
    /// `tmp[i]` -> `mip[i]`, vertical.
    v_bind: wgpu::BindGroup,
    /// `mip[i-1]` -> `mip[i]`, the 2:1 reduction. `None` on level 0, which the
    /// bright-pass seeds instead.
    down_bind: Option<wgpu::BindGroup>,
    /// `mip[i+1]` -> `mip[i]`, additive. `None` on the coarsest level, which has
    /// nothing below it.
    up_bind: Option<wgpu::BindGroup>,
}

/// The bloom GPU resources, built lazily on the first active frame and rebuilt
/// only when the internal grid changes size (ADR-0030's compare-first rule).
///
/// # Three uniform buffers, and that number is the point
///
/// An earlier draft gave **every** pass its own uniform — a tap step per level per
/// direction, nineteen buffers at the rich tier — because `queue.write_buffer`
/// stages its writes ahead of the whole command buffer and one buffer rewritten
/// between passes would hand them all the last value. It rendered correctly on the
/// hardware adapter and wrongly on WARP, where every blur pass behaved as though
/// it had been handed the *vertical* pass's buffer: horizontal blur measurably
/// absent, halo smeared into a vertical column of copies. Isolated by rendering
/// the same fixture on both adapters at two sizes, and confirmed by zeroing each
/// direction's step in turn — with the vertical step at zero the horizontal one
/// changed nothing, on a square grid where the two numbers are equal.
///
/// That is the ADR-0021 / Plan 0020 hazard again, and the tonemap hit its
/// uniform-shaped variant in Phase 3. Rather than guess at which resource WARP
/// aliases, the shape here removes the surface: the blur passes have **no
/// uniform**, and the three that remain — bright, up, mix — are each shared by
/// passes that all want the same value, so there is nothing left for a mix-up to
/// get wrong.
struct Resources {
    /// The grid these were built for, so [`Bloom::begin`] can compare before
    /// rebuilding.
    size: (u32, u32),
    // The offscreen the upstream stage (or the scene) renders into.
    _src: wgpu::Texture,
    src_view: wgpu::TextureView,
    /// The pyramid's `(mip, tmp)` textures, kept alive so [`Level`]'s views stay
    /// valid; not read after construction.
    _pyramid: Vec<(wgpu::Texture, wgpu::Texture)>,
    levels: Vec<Level>,
    bright_uniform: wgpu::Buffer,
    bright_bind: wgpu::BindGroup,
    bright_pipeline: wgpu::RenderPipeline,
    blur_h_pipeline: wgpu::RenderPipeline,
    blur_v_pipeline: wgpu::RenderPipeline,
    down_pipeline: wgpu::RenderPipeline,
    /// Shared by every up pass in the frame — they all fold at the same scatter.
    up_uniform: wgpu::Buffer,
    up_pipeline: wgpu::RenderPipeline,
    mix_uniform: wgpu::Buffer,
    mix_bind: wgpu::BindGroup,
    mix_pipeline: wgpu::RenderPipeline,
}

/// A `COMPOSITE_FORMAT` colour target that can also be sampled.
fn target_texture(
    device: &wgpu::Device,
    label: &str,
    format: wgpu::TextureFormat,
    size: (u32, u32),
) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: size.0.max(1),
            height: size.1.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    })
}

/// A 16-byte uniform buffer.
fn small_uniform(device: &wgpu::Device, label: &str) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: std::mem::size_of::<V4>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

impl Resources {
    fn build(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        size: (u32, u32),
        max_levels: u32,
    ) -> Self {
        let src = target_texture(device, "bloom-src", surface_format, size);
        let src_view = src.create_view(&wgpu::TextureViewDescriptor::default());

        // Linear filtering is load-bearing: every pass in the pyramid changes
        // resolution, and the free 2x2 average on the way down is half of why a
        // 5-tap kernel is enough. Clamped, so the widest taps at the frame's edge
        // extend the border instead of wrapping a halo around to the far side.
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("bloom-sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        // --- the four layouts; see the module docs on why these orderings ---
        let bright_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("bloom-bright-layout"),
            entries: &[
                gpu::uniform(0, wgpu::ShaderStages::FRAGMENT),
                gpu::sampler(1),
                gpu::texture(2, true),
            ],
        });
        // Shared by the horizontal blur, the vertical blur and the reduction —
        // they differ only in shader, and none of them binds a uniform.
        let blur_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("bloom-blur-layout"),
            entries: &[gpu::sampler(0), gpu::texture(1, true)],
        });
        let up_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("bloom-up-layout"),
            entries: &[
                gpu::sampler(0),
                gpu::texture(1, true),
                gpu::uniform(2, wgpu::ShaderStages::FRAGMENT),
            ],
        });
        let mix_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("bloom-mix-layout"),
            entries: &[
                gpu::sampler(0),
                gpu::texture(1, true),
                gpu::texture(2, true),
                gpu::uniform(3, wgpu::ShaderStages::FRAGMENT),
            ],
        });

        // --- the pyramid's textures, before any bind group can name them ---
        let sizes = level_sizes(size, max_levels);
        let textures: Vec<(wgpu::Texture, wgpu::Texture)> = sizes
            .iter()
            .enumerate()
            .map(|(index, &level_size)| {
                (
                    target_texture(
                        device,
                        &format!("bloom-mip-{index}"),
                        surface_format,
                        level_size,
                    ),
                    target_texture(
                        device,
                        &format!("bloom-tmp-{index}"),
                        surface_format,
                        level_size,
                    ),
                )
            })
            .collect();
        let views: Vec<(wgpu::TextureView, wgpu::TextureView)> = textures
            .iter()
            .map(|(mip, tmp)| {
                (
                    mip.create_view(&wgpu::TextureViewDescriptor::default()),
                    tmp.create_view(&wgpu::TextureViewDescriptor::default()),
                )
            })
            .collect();

        let blur_bind = |label: &str, source: &wgpu::TextureView| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(label),
                layout: &blur_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Sampler(&sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(source),
                    },
                ],
            })
        };
        let up_uniform = small_uniform(device, "bloom-up-uniform");
        let up_bind = |label: &str, source: &wgpu::TextureView| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(label),
                layout: &up_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Sampler(&sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(source),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: up_uniform.as_entire_binding(),
                    },
                ],
            })
        };

        let mut levels = Vec::with_capacity(sizes.len());
        for (index, (mip, tmp)) in views.iter().enumerate() {
            levels.push(Level {
                mip: mip.clone(),
                tmp: tmp.clone(),
                h_bind: blur_bind(&format!("bloom-h-bg-{index}"), mip),
                v_bind: blur_bind(&format!("bloom-v-bg-{index}"), tmp),
                // Level 0 is seeded by the bright-pass instead.
                down_bind: index
                    .checked_sub(1)
                    .and_then(|prev| views.get(prev))
                    .map(|(prev_mip, _)| blur_bind(&format!("bloom-down-bg-{index}"), prev_mip)),
                up_bind: views
                    .get(index + 1)
                    .map(|(next_mip, _)| up_bind(&format!("bloom-up-bg-{index}"), next_mip)),
            });
        }

        // --- the bright-pass, which seeds level 0 from the stage's input ---
        let bright_uniform = small_uniform(device, "bloom-bright-uniform");
        let bright_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("bloom-bright-bg"),
            layout: &bright_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: bright_uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&src_view),
                },
            ],
        });

        // --- the recombine, which is the only pass that writes outside the stage ---
        let finest = levels
            .first()
            .map(|level| level.mip.clone())
            .unwrap_or_else(|| src_view.clone());
        let mix_uniform = small_uniform(device, "bloom-mix-uniform");
        let mix_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("bloom-mix-bg"),
            layout: &mix_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&src_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&finest),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: mix_uniform.as_entire_binding(),
                },
            ],
        });

        let bright_shader = gpu::fullscreen_shader(
            device,
            "bloom-bright-shader",
            gpu::FULLSCREEN_VS_UV_FLIPPED,
            BRIGHT_SHADER,
        );
        // The two blur halves and the reduction share a prelude — one copy of the
        // kernel and of the `textureDimensions` step — and differ only in the two
        // lines of `fs_main` that pick a direction.
        let blur_h_shader = gpu::fullscreen_shader(
            device,
            "bloom-blur-h-shader",
            gpu::FULLSCREEN_VS_UV_FLIPPED,
            &format!("{BLUR_PRELUDE}{BLUR_H_SHADER}"),
        );
        let blur_v_shader = gpu::fullscreen_shader(
            device,
            "bloom-blur-v-shader",
            gpu::FULLSCREEN_VS_UV_FLIPPED,
            &format!("{BLUR_PRELUDE}{BLUR_V_SHADER}"),
        );
        let down_shader = gpu::fullscreen_shader(
            device,
            "bloom-down-shader",
            gpu::FULLSCREEN_VS_UV_FLIPPED,
            &format!("{BLUR_PRELUDE}{DOWN_SHADER}"),
        );
        let up_shader = gpu::fullscreen_shader(
            device,
            "bloom-up-shader",
            gpu::FULLSCREEN_VS_UV_FLIPPED,
            UP_SHADER,
        );
        let mix_shader = gpu::fullscreen_shader(
            device,
            "bloom-mix-shader",
            gpu::FULLSCREEN_VS_UV_FLIPPED,
            MIX_SHADER,
        );

        let bright_pipeline = gpu::fullscreen_pipeline(
            device,
            &bright_shader,
            &[&bright_layout],
            surface_format,
            wgpu::BlendState::REPLACE,
            "bloom-bright",
        );
        let blur_h_pipeline = gpu::fullscreen_pipeline(
            device,
            &blur_h_shader,
            &[&blur_layout],
            surface_format,
            wgpu::BlendState::REPLACE,
            "bloom-blur-h",
        );
        let blur_v_pipeline = gpu::fullscreen_pipeline(
            device,
            &blur_v_shader,
            &[&blur_layout],
            surface_format,
            wgpu::BlendState::REPLACE,
            "bloom-blur-v",
        );
        let down_pipeline = gpu::fullscreen_pipeline(
            device,
            &down_shader,
            &[&blur_layout],
            surface_format,
            wgpu::BlendState::REPLACE,
            "bloom-down",
        );
        let up_pipeline = gpu::fullscreen_pipeline(
            device,
            &up_shader,
            &[&up_layout],
            surface_format,
            ADDITIVE,
            "bloom-up",
        );
        // Premultiplied-alpha OVER (ADR-0055), the same as the other two stages':
        // into a transparent-cleared intermediate it is bit-identical to REPLACE,
        // and into the chain's destination it composites over the backdrop.
        let mix_pipeline = gpu::fullscreen_pipeline(
            device,
            &mix_shader,
            &[&mix_layout],
            surface_format,
            wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING,
            "bloom-mix",
        );

        Self {
            size,
            _src: src,
            src_view,
            _pyramid: textures,
            levels,
            bright_uniform,
            bright_bind,
            bright_pipeline,
            blur_h_pipeline,
            blur_v_pipeline,
            down_pipeline,
            up_uniform,
            up_pipeline,
            mix_uniform,
            mix_bind,
            mix_pipeline,
        }
    }
}

/// `One + One` on colour and alpha — the up-chain's blend, which sums a coarse
/// level onto the finer one already in the target.
const ADDITIVE: wgpu::BlendState = wgpu::BlendState {
    color: wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::One,
        dst_factor: wgpu::BlendFactor::One,
        operation: wgpu::BlendOperation::Add,
    },
    alpha: wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::One,
        dst_factor: wgpu::BlendFactor::One,
        operation: wgpu::BlendOperation::Add,
    },
};

/// Encode one fullscreen pass into `view`.
fn blit(
    encoder: &mut wgpu::CommandEncoder,
    label: &str,
    view: &wgpu::TextureView,
    load: wgpu::LoadOp<wgpu::Color>,
    pipeline: &wgpu::RenderPipeline,
    bind: &wgpu::BindGroup,
) {
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some(label),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load,
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, bind, &[]);
    pass.draw(0..3, 0..1);
}

/// The engine bloom stage — a [`PostStage`], not a
/// [`Scene`](super::scenes::Scene): it consumes an already-rendered frame rather
/// than an `AnalysisFrame`. Last in the chain (ADR-0046), so it blooms the folded
/// image and its bright-pass sees the final HDR composite.
pub struct Bloom {
    device: wgpu::Device,
    surface_format: wgpu::TextureFormat,
    res: Option<Resources>,
    amount: f32,
    threshold: f32,
    radius: f32,
    /// The frame's evaluated `exposure`, handed down by the renderer through
    /// [`PostStage::set_exposure`] (ADR-0080). **Not a param of this stage**: it is
    /// owned, bound and applied by the tonemap, and read here only so the
    /// bright-pass can compare against the light the display will be shown.
    ///
    /// Set once per frame per side, before [`resolve`](PostStage::resolve); it is
    /// not reset by [`reset_params`](PostStage::reset_params) because it is not
    /// this stage's to default. The neutral stop is the initial value, so a caller
    /// that never sets it gets today's arithmetic exactly.
    exposure: f32,
    /// The active tier's cap on this stage's internal grid — see
    /// [`Trails::post_cap`](super::trails::Trails).
    post_cap: (u32, u32),
    /// The active tier's pyramid depth
    /// ([`TierConfig::bloom_levels`](super::TierConfig::bloom_levels)), resolved
    /// once at construction. Read only through [`level_sizes`], which is pure, so
    /// the chain's rebuild comparison keeps resting on a pure function of
    /// `surface`.
    max_levels: u32,
    /// How many times [`Resources::build`] has run — see
    /// [`Trails::builds`](super::trails::Trails).
    builds: u32,
}

/// Global parameter vocabulary — see [`background::PARAMS`](super::background::PARAMS).
/// **Keep in sync with `set_param` below.**
pub const PARAMS: &[&str] = &["bloom_amount", "bloom_threshold", "bloom_radius"];

impl Bloom {
    /// Store the device/format/tier for a lazy build; no GPU resources yet.
    pub fn new(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        post_cap: (u32, u32),
        max_levels: u32,
    ) -> Self {
        Self {
            device: device.clone(),
            surface_format,
            res: None,
            amount: DEFAULT_AMOUNT,
            threshold: DEFAULT_THRESHOLD,
            radius: DEFAULT_RADIUS,
            exposure: super::tonemap::DEFAULT_EXPOSURE,
            post_cap,
            max_levels,
            builds: 0,
        }
    }

    /// How many times this stage has built its GPU resources.
    #[cfg(test)]
    pub(crate) fn build_count(&self) -> u32 {
        self.builds
    }
}

impl PostStage for Bloom {
    fn name(&self) -> &'static str {
        "bloom"
    }

    /// Reset the bloom params to their defaults (each frame, before routing).
    fn reset_params(&mut self) {
        self.amount = DEFAULT_AMOUNT;
        self.threshold = DEFAULT_THRESHOLD;
        self.radius = DEFAULT_RADIUS;
    }

    /// Apply one named parameter, returning whether it was a `bloom_*` param.
    fn set_param(&mut self, name: &str, value: f32) -> bool {
        match name {
            "bloom_amount" => self.amount = value,
            "bloom_threshold" => self.threshold = value,
            "bloom_radius" => self.radius = value,
            _ => return false,
        }
        true
    }

    fn params(&self) -> &'static [&'static str] {
        PARAMS
    }

    /// Take the frame's evaluated stop (ADR-0080). See the field's docs for why
    /// this is not a param and not reset with them.
    fn set_exposure(&mut self, exposure: f32) {
        self.exposure = exposure;
    }

    /// Whether bloom runs this frame (a preset bound `bloom_amount > 0`).
    fn active(&self) -> bool {
        self.amount > 0.0 && self.amount.is_finite()
    }

    /// The stage-input size, following the render target under the shared policy
    /// (ADR-0034) — the same grid the other two stages run at, so a chain of all
    /// three hands off at one resolution.
    fn internal_size(&self, surface: (u32, u32)) -> (u32, u32) {
        internal_grid_size(surface, self.post_cap)
    }

    /// Build the resources if needed and return the offscreen view the upstream
    /// stage (or the scene) renders into this frame. Called when
    /// [`active`](PostStage::active).
    fn begin(
        &mut self,
        _encoder: &mut wgpu::CommandEncoder,
        surface: (u32, u32),
    ) -> Option<wgpu::TextureView> {
        // Compare-first (ADR-0030): build once, then only when the grid changes.
        // The pyramid is 2N textures, so rebuilding per frame would be the most
        // expensive version of this mistake in the engine.
        let wanted = self.internal_size(surface);
        if self.res.as_ref().is_none_or(|res| res.size != wanted) {
            self.res = Some(Resources::build(
                &self.device,
                self.surface_format,
                wanted,
                self.max_levels,
            ));
            self.builds += 1;
        }
        self.res.as_ref().map(|res| res.src_view.clone())
    }

    /// Run the pyramid and composite the result into `out` — the next active
    /// stage's input, or the chain's destination. Returns the passes encoded:
    /// `4N` for an `N`-level pyramid (bright, then reduce/H/V per level bar the
    /// first's reduce, then `N-1` folds back up, then the recombine).
    ///
    /// `surface` is unused: no pass here computes geometry, so this stage has no
    /// aspect to get wrong (ADR-0037). Every pass is a normalized blit, and the
    /// pyramid's own levels are resolutions rather than shapes.
    fn resolve(
        &mut self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        out: &wgpu::TextureView,
        _surface: (u32, u32),
        fold: Fold,
    ) -> u32 {
        let Some(res) = self.res.as_ref() else {
            return 0;
        };
        let threshold = self.threshold.clamp(0.0, MAX_THRESHOLD);
        let radius = self.radius.clamp(MIN_RADIUS, MAX_RADIUS);
        let amount = self.amount.clamp(0.0, MAX_AMOUNT);
        // Already sanitized by `Tonemap::applied_exposure` at the source; guarded
        // again here because this stage must stay correct whatever a future caller
        // hands it, and because a NaN reaching `m * u.v.z` would make the whole
        // bright-pass select nothing.
        let exposure = if self.exposure.is_finite() {
            self.exposure.max(0.0)
        } else {
            super::tonemap::DEFAULT_EXPOSURE
        };
        let s = scatter(radius);
        let norm = pyramid_sum(s, res.levels.len());

        // Three writes, to three buffers each read by passes that all want the
        // same value — see [`Resources`] for why that count is deliberate.
        queue.write_buffer(
            &res.bright_uniform,
            0,
            bytemuck::bytes_of(&V4 {
                v: [threshold, knee_band(threshold), exposure, 0.0],
            }),
        );
        queue.write_buffer(
            &res.up_uniform,
            0,
            bytemuck::bytes_of(&V4 {
                v: [s, 0.0, 0.0, 0.0],
            }),
        );
        queue.write_buffer(
            &res.mix_uniform,
            0,
            bytemuck::bytes_of(&V4 {
                v: [amount / norm, 0.0, 0.0, 0.0],
            }),
        );

        // 1. Threshold the stage's input down into the finest level.
        let Some(first) = res.levels.first() else {
            return 0;
        };
        blit(
            encoder,
            "bloom-bright-pass",
            &first.mip,
            wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
            &res.bright_pipeline,
            &res.bright_bind,
        );
        let mut passes = 1;

        // 2. Down the pyramid: halve into this level, then blur it in place,
        //    horizontally and then vertically at its own resolution. The reduction
        //    is its own pass rather than folded into the horizontal blur — see
        //    [`DOWN_SHADER`], which is where the elongated-halo defect lived.
        for level in res.levels.iter() {
            if let Some(down) = level.down_bind.as_ref() {
                blit(
                    encoder,
                    "bloom-down",
                    &level.mip,
                    wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    &res.down_pipeline,
                    down,
                );
                passes += 1;
            }
            blit(
                encoder,
                "bloom-blur-h",
                &level.tmp,
                wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                &res.blur_h_pipeline,
                &level.h_bind,
            );
            blit(
                encoder,
                "bloom-blur-v",
                &level.mip,
                wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                &res.blur_v_pipeline,
                &level.v_bind,
            );
            passes += 2;
        }

        // 3. Back up, coarse to fine, adding each level onto the one above it at
        //    the scatter weight — so the finest level ends up holding
        //    `L0 + s*L1 + s^2*L2 + …`.
        for level in res.levels.iter().rev() {
            let Some(up) = level.up_bind.as_ref() else {
                continue;
            };
            blit(
                encoder,
                "bloom-up",
                &level.mip,
                wgpu::LoadOp::Load,
                &res.up_pipeline,
                up,
            );
            passes += 1;
        }

        // 4. Source plus halo, into the chain's next target.
        blit(
            encoder,
            "bloom-mix-pass",
            out,
            fold.load_op(),
            &res.mix_pipeline,
            &res.mix_bind,
        );
        passes + 1
    }

    /// Drop the lazily-built resources, so the next active frame starts from a
    /// fresh pyramid — used on the capture scene-rebuild so a headless capture
    /// stays a pure function of its inputs (NFR §6), and so a stale bloom pipeline
    /// never lingers to mis-render the next capture's scene on the WARP adapter.
    fn reset_resources(&mut self) {
        self.res = None;
    }
}

#[cfg(test)]
mod tests {
    // Test asserts panic on failure; allowed here over the file's pragma.
    #![allow(clippy::panic, clippy::indexing_slicing)]

    use super::*;

    /// The pyramid halves each axis per level and stops at the tier's depth. This
    /// is the arithmetic the `bloom_levels` tier value buys, GPU-free.
    #[test]
    fn the_pyramid_halves_each_level_down_to_the_tier_depth() {
        let sizes = level_sizes((1024, 512), 4);
        assert_eq!(sizes, vec![(512, 256), (256, 128), (128, 64), (64, 32)]);
        // A deeper tier is genuinely deeper on a grid with room for it.
        assert!(level_sizes((1024, 512), 6).len() > level_sizes((1024, 512), 4).len());
    }

    /// A grid too small for the requested depth runs a shallower pyramid rather
    /// than allocating levels the blur cannot act on — and **always keeps one**,
    /// so `resolve` has no second code path for a stage with no pyramid.
    #[test]
    fn a_small_grid_truncates_the_pyramid_but_never_empties_it() {
        // 32x32 halves to 16, 8, 4 and would next hit 2, below the floor.
        assert_eq!(level_sizes((32, 32), 8), vec![(16, 16), (8, 8), (4, 4)]);
        // Degenerate targets still yield exactly one level.
        for grid in [(1, 1), (2, 2), (4, 1), (0, 0)] {
            let sizes = level_sizes(grid, 6);
            assert_eq!(sizes.len(), 1, "{grid:?} produced {sizes:?}");
            assert!(sizes[0].0 >= 1 && sizes[0].1 >= 1, "{sizes:?}");
        }
        // A zero depth request is still one level, not none.
        assert_eq!(level_sizes((512, 512), 0).len(), 1);
    }

    /// **The tier's effect on this stage**, GPU-free: at the display size the
    /// floor tier was written for, rich runs a genuinely deeper pyramid — and the
    /// floor runs exactly the depth its `TierConfig` asks for, so the assertion is
    /// about the tier value rather than about some clamp binding first.
    ///
    /// Also pins the depth the **golden captures** run at. `composite.rs` renders
    /// 160x100, which the post grid quantizes up to 256x256, and the floor's four
    /// levels take that to 16x16 — comfortably above [`MIN_LEVEL_PX`], so the
    /// capture exercises the full floor pyramid rather than a truncated one.
    #[test]
    fn the_rich_tier_runs_a_deeper_pyramid_than_the_floor() {
        use crate::render::TierConfig;

        let depth_at = |surface: (u32, u32), tier: &TierConfig| {
            level_sizes(
                internal_grid_size(surface, tier.post_cap),
                tier.bloom_levels,
            )
            .len()
        };
        let display = (1920, 1080);
        let floor = depth_at(display, &TierConfig::FLOOR);
        let rich = depth_at(display, &TierConfig::RICH);
        assert_eq!(
            floor,
            TierConfig::FLOOR.bloom_levels as usize,
            "the floor's depth must be its tier value, not a clamp"
        );
        assert!(
            rich > floor,
            "rich must run a deeper pyramid than the floor: {rich} vs {floor}"
        );

        // The golden capture size, whose grid is 256x256.
        assert_eq!(
            internal_grid_size((160, 100), TierConfig::FLOOR.post_cap),
            (256, 256)
        );
        assert_eq!(depth_at((160, 100), &TierConfig::FLOOR), 4);
    }

    /// `bloom_radius` moves the halo's energy outward monotonically, and stays
    /// inside the band where the up-chain's sum converges. The **pixel-level**
    /// consequence — the halo actually sits further out — is
    /// `core/tests/bloom.rs`; this pins the arithmetic behind it.
    #[test]
    fn scatter_rises_with_the_radius_and_stays_below_one() {
        let mut last = -1.0;
        for radius in [0.0f32, 0.5, 1.0, 2.0, MAX_RADIUS] {
            let s = scatter(radius);
            assert!(s > last, "scatter({radius}) = {s} did not rise");
            assert!(
                (MIN_SCATTER..=MAX_SCATTER).contains(&s),
                "scatter({radius}) = {s}"
            );
            last = s;
        }
        // Out-of-range bindings saturate rather than diverging: a preset easing
        // this off a band can overshoot, and a scatter at or above 1 would make
        // every coarse level's contribution outweigh the one it lands on.
        assert_eq!(scatter(-5.0), MIN_SCATTER);
        assert_eq!(scatter(1e9), MAX_SCATTER);
    }

    /// The recombine's divisor is the sum the up-chain actually accumulates, so
    /// `bloom_amount` is a brightness and `bloom_radius` is a width rather than
    /// the two being tangled.
    #[test]
    fn the_pyramid_sum_matches_the_series_it_normalizes() {
        // 1 + s + s^2 + s^3 at s = 0.5.
        assert!((pyramid_sum(0.5, 4) - 1.875).abs() < 1e-5);
        assert!(
            (pyramid_sum(0.0, 4) - 1.0).abs() < 1e-5,
            "no scatter, one level's worth"
        );
        // Monotone in both arguments: a wider halo and a deeper pyramid each
        // accumulate more, and each has to be divided back out.
        assert!(pyramid_sum(MAX_SCATTER, 6) > pyramid_sum(MAX_SCATTER, 4));
        assert!(pyramid_sum(MAX_SCATTER, 4) > pyramid_sum(MIN_SCATTER, 4));
        // Degenerate inputs stay finite and never divide the halo away.
        assert!(pyramid_sum(1.0, 4).is_finite() && pyramid_sum(1.0, 4) >= 1.0);
        assert!(pyramid_sum(0.5, 0) >= 1.0);
    }

    // -----------------------------------------------------------------------
    // The halo does not punch a hole in the backdrop (Plan 0045 Phase 4b)
    // -----------------------------------------------------------------------

    /// The fixture, shared with `composite.rs`'s blessed baseline and
    /// `core/tests/bloom.rs`: a small bright core on black, over range by design.
    /// Its `bg_bright` line is stripped and rewritten per capture — the whole
    /// point here is the value it does *not* ship.
    const BACKDROP_FIXTURE: &str = include_str!("../../tests/fixtures/composite_bloom.toml");

    /// The lit half of the comparison. Bright enough to be unmistakably present in
    /// every channel, dim enough not to wash out the knot the halo comes from.
    const BACKDROP_BRIGHT: f32 = 0.45;

    /// Slack for half-precision rounding. The composite is `Rgba16Float`, so a
    /// value of magnitude `m` is stored to roughly `m / 1024`, and the lit capture
    /// quantizes a *different* sum than the dark one. Four of those.
    ///
    /// It is slack, not a tolerance: the property below is exact in real
    /// arithmetic. Measured, the fixed shader's worst deficit is **0.0000** and
    /// the unclamped one's is **0.3125**, so this sits ~150x below the defect and
    /// ~all the way above the noise.
    fn half_slack(value: f32) -> f32 {
        (4.0 / 1024.0) * value.abs().max(1.0)
    }

    /// **Compositing a backdrop underneath the chain may only ADD light** — the
    /// guard the bloom stage shipped without.
    ///
    /// The recombine summed **alpha** as well as colour, and it blends
    /// `PREMULTIPLIED_ALPHA_BLENDING` into the chain's destination, where ADR-0055
    /// paints the backdrop. Where the frame was already opaque and the halo
    /// non-zero the source alpha exceeded 1, `1 - src.a` went negative, and the
    /// backdrop was *subtracted* under the frame's brightest regions. See the
    /// module docs.
    ///
    /// # Why this reads the linear composite and not the capture
    ///
    /// **A display-byte version of this assertion cannot be written.** The bytes a
    /// capture holds are downstream of the tonemap, which scales all three
    /// channels by `f(m)/m` off the brightest one (ADR-0046, hue-preserving). Add
    /// a red-dominant backdrop under a magenta stroke and `m` rises, so the scale
    /// falls, so **blue comes out darker than it did over black** — measured at up
    /// to **15 bytes on this fixture with the bloom stage switched off entirely**.
    /// That is the curve behaving as designed, it is seven times the defect's own
    /// display-space signal, and no byte-level tolerance separates them.
    ///
    /// Upstream of the tonemap there is no such confound: the composite is a plain
    /// premultiplied OVER, so the true bound is **0** rather than a tolerance, and
    /// the defect is unmasked — the same fixture reads a worst deficit of
    /// **0.3125** linear on the unclamped shader against **0.0000** on the fixed
    /// one. That readback is `pub(crate)`, which is why this test is here rather
    /// than beside the stage's other pixel properties in `core/tests/bloom.rs`.
    ///
    /// # Why it needed writing at all
    ///
    /// Every bloom fixture runs `bg_bright = 0` — for a baseline that is the right
    /// call (see the fixture's own comment), and it is also why the one stage in
    /// the chain that can exceed alpha 1 had no lit-backdrop test. On black,
    /// subtracting the backdrop and covering it are the same picture. Same shape
    /// as the guard `core/tests/kaleidoscope.rs` installed for the fold, and the
    /// blind spot ADR-0055's Negative section names outright.
    #[test]
    fn a_backdrop_under_an_active_halo_only_ever_adds_light() {
        use crate::dsp::AnalysisFrame;
        use crate::preset::Preset;
        use crate::render::capture;
        use crate::render::context::RenderError;
        use crate::render::{HeadlessOptions, Renderer};

        /// Square and modest: this reads back a whole float frame twice.
        const SIZE: u32 = 256;
        /// The fixture is frozen (`spin = 0`), so this only clears the draw-in.
        const FRAMES: u32 = 40;

        // --- Non-vacuity, before any GPU work: the fixture must still switch the
        // stage on. Edit `bloom_amount` to 0 and every assertion below would hold
        // for the trivial reason that the pyramid never ran. ---
        let binds_bloom = BACKDROP_FIXTURE.lines().any(|line| {
            let line = line.trim_start();
            line.starts_with("bloom_amount")
                && line.contains('"')
                && !line.contains("\"0\"")
                && !line.contains("\"0.0\"")
        });
        assert!(
            binds_bloom,
            "composite_bloom.toml no longer binds a non-zero `bloom_amount`, so \
             this test renders a frame with no bloom stage in it and proves nothing"
        );

        /// The linear composite the tonemap is about to map, at a given backdrop.
        ///
        /// Builds and drops **one** renderer per call rather than holding two: a
        /// second live device in a binary is what the software adapter falls over
        /// on, and the whole point of this test is a configuration that puts an
        /// extra fullscreen pipeline (the backdrop's) on the device.
        fn linear_composite(bg_bright: f32) -> Option<Vec<f32>> {
            let mut renderer = match Renderer::new_headless(HeadlessOptions {
                width: SIZE,
                height: SIZE,
                prefer_software: true,
            }) {
                Ok(renderer) => renderer,
                Err(RenderError::RequestAdapter(_)) => {
                    eprintln!("skipped: no GPU adapter on this runner (ADR-0016)");
                    return None;
                }
                Err(e) => panic!("headless renderer build failed: {e}"),
            };
            let base: String = BACKDROP_FIXTURE
                .lines()
                .filter(|line| !line.trim_start().starts_with("bg_bright"))
                .collect::<Vec<_>>()
                .join("\n");
            let toml = format!("{base}\nbg_bright = \"{bg_bright}\"\n");
            let preset =
                Preset::from_toml_str(&toml).expect("the bloom fixture parses with a backdrop");
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
            renderer
                .capture_preset(&name, &frame, FRAMES)
                .expect("capture the bloom fixture");

            // The tonemap's input still holds that frame's linear composite.
            let device = renderer.ctx.device.clone();
            let queue = renderer.ctx.queue.clone();
            let src = renderer
                .tonemap
                .src_texture()
                .expect("the tonemap built its input while capturing")
                .clone();
            let (buffer, padded_bpr) = capture::create_linear_readback(&device, SIZE, SIZE);
            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("bloom-backdrop-readback"),
            });
            capture::record_copy(&mut encoder, &src, &buffer, padded_bpr, SIZE, SIZE);
            queue.submit(std::iter::once(encoder.finish()));
            Some(
                capture::read_back_linear(&device, &buffer, SIZE, SIZE, padded_bpr)
                    .expect("read back the linear composite"),
            )
        }

        let Some(dark) = linear_composite(0.0) else {
            return;
        };
        let Some(lit) = linear_composite(BACKDROP_BRIGHT) else {
            return;
        };
        assert_eq!(dark.len(), lit.len(), "the two captures differ in size");

        // Colour only: the composite's alpha is the backdrop's own (opaque)
        // wherever the backdrop paints, so it is 1.0 in the lit capture by
        // construction and says nothing about the recombine.
        let colour = |values: &[f32]| -> Vec<f32> {
            values
                .chunks_exact(4)
                .flat_map(|texel| texel[..3].to_vec())
                .collect()
        };
        let (dark, lit) = (colour(&dark), colour(&lit));

        let (mut worst, mut violations, mut gained) = (0.0f32, 0usize, 0usize);
        for (&d, &l) in dark.iter().zip(lit.iter()) {
            let deficit = d - l;
            if deficit > worst {
                worst = deficit;
            }
            if deficit > half_slack(d) {
                violations += 1;
            }
            if l - d > half_slack(d) {
                gained += 1;
            }
        }
        eprintln!(
            "linear composite, bg_bright 0 -> {BACKDROP_BRIGHT}: {gained} channels \
             gained, {violations} lost, worst deficit {worst:.4} of {}",
            dark.len()
        );

        // --- Non-vacuity: the backdrop genuinely reached the composite. Without
        // this the comparison would pass on two identical frames. ---
        assert!(
            gained * 2 > dark.len(),
            "only {gained} of {} channels gained light when bg_bright went 0 -> \
             {BACKDROP_BRIGHT} — the backdrop is not reaching the composite, so \
             the assertion below is about nothing",
            dark.len()
        );

        // --- The property. ---
        assert_eq!(
            violations, 0,
            "{violations} channels of the linear composite are DARKER with a \
             backdrop underneath than without one (worst {worst:.4}). Upstream of \
             the tonemap this is a plain premultiplied OVER, which cannot remove \
             light — so this is the recombine driving source alpha past 1 and \
             making the blend's `1 - src.a` factor negative, subtracting the \
             backdrop under the halo instead of covering it"
        );
    }

    /// The knee is a positive band at every threshold a preset can bind, including
    /// zero — otherwise the bright-pass would divide by it.
    #[test]
    fn the_knee_band_is_always_positive() {
        for threshold in [0.0f32, 0.05, 1.0, MAX_THRESHOLD] {
            assert!(
                knee_band(threshold) >= MIN_KNEE,
                "knee_band({threshold}) = {}",
                knee_band(threshold)
            );
        }
        // And it scales with the threshold rather than staying at the floor.
        assert!(knee_band(4.0) > knee_band(1.0));
    }
}
