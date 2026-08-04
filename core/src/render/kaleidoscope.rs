//! Screen-space kaleidoscope (ADR-0018 composite stage 4, Plan 0018 Phase 7): a
//! post-pass that folds the composited frame into `N` mirrored wedges before
//! present — the general, engine-wide kaleidoscope (distinct from the line-only
//! *geometry* mirror of Phase 4, which replicates segments; this folds pixels).
//!
//! The fold is dihedral: each output pixel's angle is wrapped into one
//! `2*pi/order` wedge and mirrored within it, so the frame is invariant under a
//! `2*pi/order` rotation and carries a mirror line per wedge. `kaleido_angle`
//! rotates the whole fold, and `kaleido_center_x`/`_y` place its axis.
//!
//! # The fold folds a **disc** (ADR-0047)
//!
//! The operation is polar but the source is rectangular, so the two do not have
//! the same shape. Each output pixel keeps its radius and only changes its angle,
//! which means any pixel whose radius exceeds the source's extent *in the folded
//! direction* reconstructs a coordinate outside `[0, 1]`. That used to be handed
//! to a `ClampToEdge` sampler, which smeared the border texel radially into hard
//! streaks and chevron debris — design-backlog 0010, user-reported three times,
//! and catastrophic in a portrait window, where most of the frame is out of range
//! rather than just the corners.
//!
//! So the **sample** radius is clamped to `r_max`, the largest disc around the
//! fold axis that the source contains (the nearest of the four edges, in
//! aspect-corrected space). Nothing is ever sampled outside the source, at any
//! aspect or fold centre. Past `r_max` the result fades out over
//! [`FALLOFF_BAND`], so the boundary is a designed vignette rather than a hard
//! ring. Content outside that disc is discarded — the fold shows less of the
//! source frame than the broken version pretended to, which ADR-0047 weighed and
//! accepted for centred figures, the fold's overwhelming use.
//!
//! # ...and what happens **outside** that disc is a choice (`kaleido_edge`, ADR-0061)
//!
//! ADR-0047 picked one treatment for the region beyond `r_max`, and that region is
//! not a trim: with `r_max = 0.5` in aspect-corrected space and a corner radius of
//! `0.5 * sqrt(aspect^2 + 1)`, the corner sits at **2.04x `r_max` at 16:9** and the
//! inscribed disc covers only `pi r_max^2` of an `aspect x 1` frame — so **55.8 %
//! of the frame at 16:9 lies outside it** (the same at 9:16, by symmetry). Seen in
//! motion, one treatment could not serve both of the fold's populations: the
//! residual rays read as leftovers around a *centred figure*, and the disc *crops*
//! a border-filling *field* that used to fill the frame.
//!
//! So [`PARAMS`]'s `kaleido_edge` selects the treatment per preset, from a closed
//! roster, inside **one** pipeline — every arm is a uniform branch on how `r` maps
//! to a sample radius `rs` and an output weight `w` (`m = r / r_max`):
//!
//! | Value | Name | `rs` | `w` |
//! |---|---|---|---|
//! | 0 | `falloff` | `min(r, r_max)` | `1 - smoothstep(r_max, r_max*(1+band), r)` |
//! | 1 | `vignette` | `min(r, r_max)` | `1 - smoothstep(r_max*(1-band), r_max, r)` |
//! | 2 | `mirror` | `r_max * abs(m - 2*round(m/2))` | `1` |
//! | 3 | `tile` | `r` (**`MirrorRepeat` sampler**) | `1` |
//! | 4 | `squash` | `r_max * tanh(m)` | `1` |
//!
//! **0 is the default and is today's behaviour byte-for-byte**, so ADR-0047 is
//! supplemented rather than superseded and nothing moves until an author opts in.
//!
//! Four of the five keep `rs` inside `[0, r_max]`, so they inherit ADR-0047's real
//! guarantee unchanged — the design-backlog 0010 smear came from *reconstructing a
//! coordinate outside the source* and handing it to `ClampToEdge`, and none of them
//! does that. `tile` is the exception and the reason there is a second sampler: it
//! is the one arm whose coordinate is *meant* to leave `[0,1]`, which is safe only
//! because a `MirrorRepeat` sampler defines that read. Wired to the `ClampToEdge`
//! sampler it would be the original defect under a new name.
//!
//! **What `mirror` does at the corners is arithmetic, and it is not obvious.** At
//! 16:9 the frame corner sits at `m = 2.04`, and `abs(2.04 - 2*round(1.02)) = 0.04`
//! — so the corners sample from **0.04 `r_max`, right next to the fold axis**.
//! `mirror` brings the *centre* of the figure back into the corners; it is a
//! reflection of the disc, not a continuation outward of its rim.
//!
//! `kaleido_edge` is the stage's **second stepped param**. Like `kaleido_order` it
//! is clamped and rounded on the CPU ([`fold_edge`]), for the [`fold_order`]
//! reason: `[smoothing]` and preset dissolves both sweep a param *continuously*
//! between two settings, and a selector swept through 2.5 is not a sixth treatment
//! — it is an undefined one. Rounding in Rust keeps the shader's precondition
//! visible in Rust.
//!
//! **Identity passthrough when `kaleido_order < 2`** — every shipped preset until
//! one opts in — so the [`PostChain`](super::post::PostChain) skips this stage
//! entirely: no offscreen, no
//! pipeline, golden/determinism unchanged, the NFR §1 iGPU floor pays nothing,
//! and (like the background/trails passes) the DX12 WARP software adapter never
//! sees a coexisting fold pipeline during the no-kaleidoscope captures. When
//! active the pipeline builds lazily and is dropped on the capture scene-rebuild.
//!
//! Runs at an internal resolution that **follows the render target** (ADR-0034),
//! quantized and capped by
//! [`internal_grid_size`](super::post::internal_grid_size). It used to be a fixed
//! 1280x720 with the fold's aspect correction baked to match.
//!
//! **The fold's aspect is the render target's, never that grid's** (ADR-0037). The
//! grid is quantized to a 256 px step, so its ratio is only approximately the
//! window's, and folding about the grid's axis skewed every wedge whenever the two
//! disagreed — which is most window sizes, but not the 16:9 ones this was
//! developed at.
//!
//! On a line scene, prefer the **geometry** mirror (`mirror_order` /
//! `mirror_reflect`) over this fold when either would do: that one replicates real
//! segments *before* rasterization, so it costs nothing in resolution, while this
//! one folds finished pixels at the stage's internal grid.

// Hot-path panic-denial pragma (Plan 0002 Phase 2; render/ is scanned by the
// hygiene guard). The fold pass encodes every displayed frame it is active.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

use crate::render::gpu;

use super::post::{Fold, PostStage, internal_grid_size};

/// `kaleido_order` default — 1 = identity, so an unbound preset is unaffected.
const DEFAULT_ORDER: f32 = 1.0;
/// `kaleido_angle` default — no rotation.
const DEFAULT_ANGLE: f32 = 0.0;
/// `kaleido_center_x` / `kaleido_center_y` default — the screen centre, which is
/// where the fold axis was hardcoded before it became bindable (ADR-0047).
const DEFAULT_CENTER: f32 = 0.5;

/// How far past the inscribed disc the vignette takes to fade out, as a fraction
/// of `r_max` (ADR-0047).
///
/// The out-of-disc region is not small and its size is an aspect fact, not a
/// tuning one: the fold keeps each output pixel's radius, so what matters is the
/// ratio between the frame's corner radius and its shortest half-extent, which is
/// `sqrt(1 + (long/short)^2)` — about **2.04x `r_max` at 16:9**, and larger the
/// further the window is from square. A band of 0.35 therefore reaches the
/// backdrop well before the corners at every aspect, which is what makes the edge
/// read as a vignette rather than as a disc pasted on a rectangle.
const FALLOFF_BAND: f32 = 0.35;

/// `kaleido_edge` default — 0 = `falloff`, ADR-0047's shipped treatment, so an
/// unbound preset renders exactly as it did before ADR-0061 (module docs).
const DEFAULT_EDGE: f32 = 0.0;
/// The last value the `kaleido_edge` roster defines (4 = `squash`). Values past it
/// clamp here rather than selecting the shader's fall-through arm by accident.
const MAX_EDGE: f32 = 4.0;

/// Below this order the fold is the identity passthrough (the stage is skipped).
const MIN_ACTIVE_ORDER: f32 = 2.0;
/// Ceiling on the fold order — beyond a couple dozen wedges the fold is a blur.
const MAX_ORDER: f32 = 48.0;

/// The wedge count the shader is handed: clamped to the active range, then
/// **rounded to an integer**.
///
/// The shader wraps with `a - seg * floor(a / seg)` where `seg = 2*pi/order`, then
/// mirrors within the wedge — a function periodic in `seg`. `atan2`'s branch cut
/// lies on the **-x ray**: crossing it, `a` jumps by exactly `2*pi`, and a
/// `seg`-periodic function absorbs that jump only when `2*pi` is a whole multiple
/// of `seg` — that is, only when `order` is an integer. At any fractional order
/// the frame tears along one horizontal ray from the centre to the left edge.
///
/// Two things make that constant rather than rare. `kaleido_order` sits under
/// `[smoothing]` in nearly every shipped kaleido preset, so each ladder step eases
/// through a second or more of fractional orders and preset dissolves interpolate
/// it too; and the fold's mirror is *even*, so the jump cancels exactly at
/// `kaleido_angle = 0` and only at 0 — which 10 of the 12 shipped presets with an
/// active fold leave behind immediately, driving the angle off `time`.
///
/// Rounding **here** rather than in WGSL keeps the shader's precondition visible
/// in Rust: the uniform never carries a fractional order. The cost is that
/// `kaleido_order` becomes a **stepped** parameter (a 12.5-wedge kaleidoscope is
/// not a thing); `presets/README.md` says so beside the param.
fn fold_order(order: f32) -> f32 {
    order.clamp(MIN_ACTIVE_ORDER, MAX_ORDER).round()
}

/// One axis of the fold centre, in uv: clamped into the frame, with a non-finite
/// binding falling back to the screen centre.
///
/// Off-frame is not a useful fold: the inscribed disc is the distance to the
/// *nearest* source edge, so a centre outside `[0, 1]` has no disc at all and the
/// falloff would take the whole frame to the backdrop. Clamping keeps an
/// over-driven binding (an eased `pan`-like sweep that overshoots) at the frame
/// edge instead of blanking the picture.
fn fold_center(v: f32) -> f32 {
    if v.is_finite() {
        v.clamp(0.0, 1.0)
    } else {
        DEFAULT_CENTER
    }
}

/// The edge treatment the shader is handed: clamped into the roster, then
/// **rounded to an integer**, with a non-finite binding falling back to the
/// default.
///
/// This is [`fold_order`]'s treatment for [`fold_order`]'s reason, on a param
/// whose values are *identities* rather than a quantity. `kaleido_edge` sits in
/// the same `[smoothing]` and preset-dissolve machinery as everything else, and
/// both of those interpolate a binding **continuously** from one setting to
/// another: easing from `mirror` (2) to `squash` (4) passes through 2.5, 3.0, 3.5.
/// Rounding here means the sweep *snaps* at each midpoint — `kaleido_order`'s
/// documented cost, taken again knowingly — instead of the shader receiving a
/// value no arm defines. Doing it in Rust rather than WGSL keeps that precondition
/// visible on the CPU side, where the roster's bounds live.
///
/// Non-finite falls back to the default rather than clamping (which is what
/// `fold_order` does with an infinity): a selector has no "as far as you can go"
/// reading, so a broken binding should land on the treatment that changes nothing.
fn fold_edge(v: f32) -> f32 {
    if v.is_finite() {
        v.clamp(0.0, MAX_EDGE).round()
    } else {
        DEFAULT_EDGE
    }
}

/// The roster's radius map, normalized: given a treatment and `m = r / r_max`,
/// the **sample** radius as a fraction of `r_max`.
///
/// **The shader below is the implementation; this is its CPU mirror**, and it
/// exists so the properties that make each treatment what it claims can be
/// asserted arithmetically rather than argued — that `falloff`, `vignette`,
/// `mirror` and `squash` never reconstruct a coordinate outside the source, that
/// `mirror` leaves the disc interior alone and reflects about `r_max`, and what
/// `mirror` does at a 16:9 corner. The two are kept identical by inspection; the
/// pixel-level guards on the shader itself are Plan 0055 Phase 3's, once the live
/// A/B has said which arms survive. Weight `w` is not mirrored here: it is a plain
/// `smoothstep` on the two arms that use it and carries no such property.
#[cfg(test)]
fn edge_sample_radius(edge: f32, m: f32) -> f32 {
    // Half-step comparisons and the same arm order as the shader, so the two are
    // one function written twice rather than two functions that agree on the
    // roster's five values. `falloff` and `vignette` share the fall-through: they
    // differ only in `w`.
    if edge < 1.5 {
        m.min(1.0)
    } else if edge < 2.5 {
        (m - 2.0 * (m * 0.5).round()).abs()
    } else if edge < 3.5 {
        m
    } else {
        m.tanh()
    }
}

const SHADER: &str = r#"
struct K {
    v: vec4<f32>, // x: order, y: angle, z: aspect, w: unused
    c: vec4<f32>, // x,y: fold centre (uv), z: falloff band (fraction of r_max),
                  //   w: edge treatment (ADR-0061; integral, quantized CPU-side)
}

@group(0) @binding(0) var<uniform> u: K;
@group(0) @binding(1) var t_src: texture_2d<f32>;
@group(0) @binding(2) var samp: sampler;
// `MirrorRepeat`, and used by the `tile` arm alone — the one treatment whose
// sample coordinate is MEANT to leave [0,1]. That is safe only because this
// sampler defines the out-of-range read; wiring `tile` to `samp` above would be
// design-backlog 0010 with a new name (ADR-0061).
@group(0) @binding(3) var samp_tile: sampler;

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let order = max(u.v.x, 1.0);
    let angle = u.v.y;
    let aspect = max(u.v.z, 0.001);
    let centre = u.c.xy;

    // Centre and aspect-correct so the wedges are radially symmetric.
    var p = in.uv - centre;
    p.x = p.x * aspect;

    let r = length(p);
    let seg = 6.28318530 / order;
    var a = atan2(p.y, p.x) + angle;
    // Wrap into one wedge, then mirror within it (dihedral fold).
    //
    // PRECONDITION: `order` is integral (the CPU side rounds it — see
    // `fold_order`). `atan2` jumps by 2*pi across the -x ray, and this wrap only
    // absorbs that jump when 2*pi is a whole multiple of `seg`. A fractional
    // order tears the frame along that ray.
    a = a - seg * floor(a / seg);
    a = abs(a - seg * 0.5);

    // The largest disc centred on the fold axis that the source rectangle
    // contains, in this same aspect-corrected space: the nearest of the four
    // edges. An off-centre fold shrinks it on one side by construction.
    let r_max = max(min(min(centre.x, 1.0 - centre.x) * aspect,
                        min(centre.y, 1.0 - centre.y)), 0.001);

    let band = max(u.c.z, 0.001);
    let m = r / r_max;

    // What happens outside the disc is the preset's choice (ADR-0061). Every arm
    // is a map from `r` to a SAMPLE radius `rs` and an output weight `w`; the
    // branch is on a uniform-buffer value, so this is one pipeline, one bind
    // layout, one pass, and — for four of the five arms — one fetch.
    //
    // PRECONDITION: `u.c.w` is an integer in [0, 4] (`fold_edge` on the CPU side).
    // The comparisons are half-step so a quantized value can only land on its own
    // arm; the fall-through is `falloff`, the default and today's behaviour.
    let edge = u.c.w;
    // Fold a DISC (ADR-0047) is still what four of the five arms do. Clamping the
    // SAMPLE radius — not the output pixel's — is what keeps every reconstructed
    // coordinate inside [0,1]: beyond r_max the polar reconstruction used to land
    // outside the source and `ClampToEdge` smeared the border texel radially into
    // the streaks and chevrons of design-backlog 0010.
    var rs = min(r, r_max);
    var w = 1.0;
    if (edge < 0.5) {
        // 0 `falloff` — past the disc, fade out rather than leaving a flat ring. A
        // plain clamp does NOT leave one: the clamped sample still varies with
        // angle, so the rim replicates outward as a sunburst of rays (ADR-0047's
        // Outcome). This is what fades those rays.
        w = 1.0 - smoothstep(r_max, r_max * (1.0 + band), r);
    } else if (edge < 1.5) {
        // 1 `vignette` — the same fade moved INSIDE the disc, so the rim is gone
        // before r_max and no ray is drawn at all. Costs a rim of real content.
        w = 1.0 - smoothstep(r_max * (1.0 - band), r_max, r);
    } else if (edge < 2.5) {
        // 2 `mirror` — reflect the radius instead of clamping it: a triangle wave
        // in `m` of period 2, identity below r_max and folding back inward above
        // it. Note `round` here is WGSL's (ties to even) against Rust's (ties away
        // from zero) in `edge_sample_radius`; they differ only at odd `m`, where
        // both give exactly 1, so the two agree everywhere on this map.
        rs = r_max * abs(m - 2.0 * round(m * 0.5));
    } else if (edge < 3.5) {
        // 3 `tile` — leave the radius alone and let the MirrorRepeat sampler
        // define the read. The ONLY arm that samples outside [0,1], and the reason
        // `samp_tile` exists.
        rs = r;
    } else {
        // 4 `squash` — compress the radius asymptotically into the disc. 1:1 at
        // the fold axis (tanh'(0) = 1) and approaching r_max at the corners, so it
        // crops nothing and draws no ray, at the cost of bending geometry — and,
        // unlike `mirror`, it is NOT the identity inside the disc: tanh(m) < m
        // everywhere past the axis, so the whole interior is pulled inward.
        rs = r_max * tanh(m);
    }

    // Reconstruct the sample coordinate from the folded angle + sample radius.
    var q = vec2<f32>(cos(a), sin(a)) * rs;
    q.x = q.x / aspect;
    let s_uv = q + centre;
    // Two `textureSample` calls, one per address mode, each in UNIFORM control
    // flow — the branch is on a uniform-buffer value, which is what `textureSample`
    // requires. Only one executes per fragment.
    var col: vec4<f32>;
    if (edge > 2.5 && edge < 3.5) {
        col = textureSample(t_src, samp_tile, s_uv);
    } else {
        col = textureSample(t_src, samp, s_uv);
    }
    // `w` scales COLOUR AND ALPHA together (ADR-0055). The values are
    // premultiplied, so this fades to *transparent* and the backdrop composited
    // underneath the chain shows through. Multiplying only `.rgb` and forcing
    // alpha to 1 is what made the falloff fade to black and fight `bg_*` instead
    // of landing on it. The three fill arms leave `w = 1`, so they carry the
    // source's own alpha out to the frame edge.
    return col * w;
}
"#;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct K {
    v: [f32; 4],
    c: [f32; 4],
}

struct Resources {
    // The offscreen the scene (or the trails output) renders into.
    /// The grid these were built for, so `begin` can compare before rebuilding.
    size: (u32, u32),
    // Kept alive so `src_view` stays valid; not read after construction.
    _src: wgpu::Texture,
    src_view: wgpu::TextureView,
    uniform: wgpu::Buffer,
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
}

impl Resources {
    fn build(device: &wgpu::Device, surface_format: wgpu::TextureFormat, size: (u32, u32)) -> Self {
        let src = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("kaleido-src"),
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
        let src_view = src.create_view(&wgpu::TextureViewDescriptor::default());
        // Four of the five edge treatments clamp their sample radius to the
        // inscribed disc, so they never sample outside [0,1] and this address mode
        // is unreachable for them — it stays `ClampToEdge` as the defined fallback.
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("kaleido-sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        // The fifth — `tile` (ADR-0061) — is the one whose coordinate leaves
        // `[0,1]` on purpose, and it is safe *only* because this sampler defines
        // what a read out there means. Reflecting rather than repeating is what
        // keeps the continuation continuous at the source border instead of
        // wrapping the far edge in. Built unconditionally so the layout shape does
        // not depend on a param value; unbound presets never sample through it.
        let sampler_tile = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("kaleido-sampler-tile"),
            address_mode_u: wgpu::AddressMode::MirrorRepeat,
            address_mode_v: wgpu::AddressMode::MirrorRepeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("kaleido-uniform"),
            size: std::mem::size_of::<K>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let shader = gpu::fullscreen_shader(
            device,
            "kaleido-shader",
            gpu::FULLSCREEN_VS_UV_FLIPPED,
            SHADER,
        );
        // `[Uniform, Texture, Sampler, Sampler]` since ADR-0061 added `tile`'s
        // second address mode. That is one entry longer than the
        // `[Uniform, Texture, Sampler]` shape ADR-0058 records this layout under —
        // a shape it shared with `ink-bind-layout` — so the fold is now the more
        // distinctive of the two and that particular collision is off the list.
        let bind_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("kaleido-bind-layout"),
            entries: &[
                gpu::uniform(0, wgpu::ShaderStages::FRAGMENT),
                gpu::texture(1, true),
                gpu::sampler(2),
                gpu::sampler(3),
            ],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("kaleido-bind-group"),
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
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&sampler_tile),
                },
            ],
        });
        // Premultiplied-alpha OVER, not REPLACE (ADR-0055). Into the chain's
        // destination this composites the fold over the backdrop painted there.
        // Into an intermediate stage's input — which `Fold::Own` has just cleared
        // to transparent — it is *bit-identical* to REPLACE, since
        // `src + dst * (1 - src.a)` with `dst = 0` is `src` in every channel. One
        // pipeline covers both, so the stage's pipeline count is unchanged and the
        // WARP sensitivity documented in `post.rs` is not disturbed.
        let pipeline = gpu::fullscreen_pipeline(
            device,
            &shader,
            &[&bind_layout],
            surface_format,
            wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING,
            "kaleido",
        );

        Self {
            size,
            _src: src,
            src_view,
            uniform,
            pipeline,
            bind_group,
        }
    }
}

/// The engine screen-space kaleidoscope stage — a [`PostStage`], not a
/// [`Scene`](super::scenes::Scene): it consumes an already-rendered frame rather
/// than an `AnalysisFrame`. Driven by the `kaleido_*` named params, it folds
/// whatever the chain hands it — the trails output when that stage is active,
/// otherwise the scene alone — before the next stage or present (ADR-0018). The
/// backdrop is **not** in that input: it is composited underneath the chain
/// (ADR-0055), so the fold never folds `bg_*`.
pub struct Kaleidoscope {
    device: wgpu::Device,
    surface_format: wgpu::TextureFormat,
    res: Option<Resources>,
    order: f32,
    angle: f32,
    center_x: f32,
    center_y: f32,
    /// The out-of-disc treatment (ADR-0061), raw as the preset bound it —
    /// [`fold_edge`] quantizes it on the way to the uniform.
    edge: f32,
    /// The active tier's cap on this stage's internal grid — see
    /// [`Trails::post_cap`](super::trails::Trails).
    post_cap: (u32, u32),
    /// How many times [`Resources::build`] has run — see
    /// [`Trails::builds`](super::trails::Trails).
    builds: u32,
}

/// Global parameter vocabulary — see [`background::PARAMS`](super::background::PARAMS).
/// **Keep in sync with `set_param` below.**
pub const PARAMS: &[&str] = &[
    "kaleido_order",
    "kaleido_angle",
    "kaleido_center_x",
    "kaleido_center_y",
    "kaleido_edge",
];

impl Kaleidoscope {
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
            order: DEFAULT_ORDER,
            angle: DEFAULT_ANGLE,
            center_x: DEFAULT_CENTER,
            center_y: DEFAULT_CENTER,
            edge: DEFAULT_EDGE,
            post_cap,
            builds: 0,
        }
    }

    /// How many times this stage has built its GPU resources.
    #[cfg(test)]
    pub(crate) fn build_count(&self) -> u32 {
        self.builds
    }
}

impl PostStage for Kaleidoscope {
    fn name(&self) -> &'static str {
        "kaleidoscope"
    }

    /// Reset the fold params to their defaults (each frame, before routing).
    fn reset_params(&mut self) {
        self.order = DEFAULT_ORDER;
        self.angle = DEFAULT_ANGLE;
        self.center_x = DEFAULT_CENTER;
        self.center_y = DEFAULT_CENTER;
        self.edge = DEFAULT_EDGE;
    }

    /// Apply one named parameter, returning whether it was a `kaleido_*` param.
    fn set_param(&mut self, name: &str, value: f32) -> bool {
        match name {
            "kaleido_order" => self.order = value,
            "kaleido_angle" => self.angle = value,
            "kaleido_center_x" => self.center_x = value,
            "kaleido_center_y" => self.center_y = value,
            "kaleido_edge" => self.edge = value,
            _ => return false,
        }
        true
    }

    fn params(&self) -> &'static [&'static str] {
        PARAMS
    }

    /// Whether the fold is active this frame (order at least 2; below that it is
    /// the identity passthrough).
    fn active(&self) -> bool {
        self.order >= MIN_ACTIVE_ORDER && self.order.is_finite()
    }

    /// The fold-input size, following the render target under the shared policy
    /// (ADR-0034) — reported to a scene that sizes an internal field, as the trails
    /// stage's is. A **texel count only**: the aspect the composite renders at, and
    /// the one [`resolve`](PostStage::resolve) folds about, is the render target's
    /// (ADR-0037).
    fn internal_size(&self, surface: (u32, u32)) -> (u32, u32) {
        internal_grid_size(surface, self.post_cap)
    }

    /// Build the resources if needed and return the offscreen view the scene (or
    /// the trails output) renders into this frame. `None` only if the resources are
    /// absent (never, after the build) — the caller falls back to the surface.
    /// Called when [`active`](PostStage::active).
    fn begin(
        &mut self,
        _encoder: &mut wgpu::CommandEncoder,
        surface: (u32, u32),
    ) -> Option<wgpu::TextureView> {
        // Compare-first (ADR-0030): build once, then only when the grid changes.
        let wanted = self.internal_size(surface);
        if self.res.as_ref().is_none_or(|res| res.size != wanted) {
            self.res = Some(Resources::build(&self.device, self.surface_format, wanted));
            self.builds += 1;
        }
        self.res.as_ref().map(|res| res.src_view.clone())
    }

    /// Fold the input offscreen into `out` — the next active stage's input, or the
    /// chain's destination. Called after the scene has rendered into the
    /// [`begin`](PostStage::begin) target, when [`active`](PostStage::active).
    fn resolve(
        &mut self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        out: &wgpu::TextureView,
        surface: (u32, u32),
        fold: Fold,
    ) -> u32 {
        let Some(res) = self.res.as_ref() else {
            return 0;
        };
        let order = fold_order(self.order);
        // The **render target's** ratio, not this stage's input grid's (ADR-0037).
        // The fold happens in the destination's space and the frame it samples was
        // drawn pre-squashed at this same aspect, so both the output geometry and
        // the reconstructed sample coordinate want the shape the frame is finally
        // seen at. The grid's own aspect is a resolution artefact — quantized to a
        // 256 px step — and correcting by it skewed every wedge on any window the
        // step did not divide evenly.
        let aspect = surface.0 as f32 / surface.1.max(1) as f32;
        queue.write_buffer(
            &res.uniform,
            0,
            bytemuck::bytes_of(&K {
                v: [order, self.angle, aspect, 0.0],
                c: [
                    fold_center(self.center_x),
                    fold_center(self.center_y),
                    FALLOFF_BAND,
                    fold_edge(self.edge),
                ],
            }),
        );
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("kaleido-pass"),
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
        pass.set_pipeline(&res.pipeline);
        pass.set_bind_group(0, &res.bind_group, &[]);
        pass.draw(0..3, 0..1);

        1 // the fold pass
    }

    /// Drop the lazily-built resources — used on the capture scene-rebuild so a
    /// stale fold pipeline never lingers to mis-render the next capture's scene on
    /// the WARP adapter (module docs).
    fn reset_resources(&mut self) {
        self.res = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The uniform never carries a fractional wedge count, whatever a preset (or
    /// the smoothing that eases between two ladder steps) hands the stage. The
    /// pixel-level consequence — no tear across the -x ray — is
    /// `core/tests/kaleidoscope.rs`; this pins the arithmetic that guarantees it.
    #[test]
    fn fold_order_is_always_integral() {
        for &raw in &[2.0f32, 2.4, 6.0, 12.5, 12.4999, 13.5, 30.7, 47.999] {
            let order = fold_order(raw);
            assert_eq!(
                order,
                order.round(),
                "fold_order({raw}) = {order} is not an integer"
            );
        }
        // Nearest-integer, not truncation: an eased sweep must land on the step
        // it is closest to rather than always the one below it.
        assert_eq!(fold_order(12.4), 12.0);
        assert_eq!(fold_order(12.6), 13.0);
    }

    /// Rounding happens inside the active range, so it can never hand the shader
    /// an order the stage would have skipped (`< MIN_ACTIVE_ORDER`) or one past
    /// the blur ceiling.
    #[test]
    fn fold_order_stays_within_the_active_range() {
        assert_eq!(fold_order(0.0), MIN_ACTIVE_ORDER);
        assert_eq!(fold_order(1.9), MIN_ACTIVE_ORDER);
        assert_eq!(fold_order(1e9), MAX_ORDER);
        assert_eq!(fold_order(f32::NEG_INFINITY), MIN_ACTIVE_ORDER);
    }

    /// The fold axis stays inside the frame whatever a binding drives it to. A
    /// centre outside `[0, 1]` has no inscribed disc — `r_max` would be negative
    /// — so this is what keeps an overshooting sweep at the edge rather than
    /// blanking the picture to the backdrop.
    #[test]
    fn fold_center_stays_inside_the_frame() {
        assert_eq!(fold_center(0.5), 0.5);
        assert_eq!(fold_center(0.2), 0.2);
        assert_eq!(fold_center(-3.0), 0.0);
        assert_eq!(fold_center(1.4), 1.0);
        // Not merely finite-checked away: NaN would otherwise reach the uniform
        // and every comparison in the shader's `min` chain would go false.
        assert_eq!(fold_center(f32::NAN), DEFAULT_CENTER);
        assert_eq!(fold_center(f32::INFINITY), DEFAULT_CENTER);
    }

    // --- The edge treatment (ADR-0061) -------------------------------------
    //
    // The shader is the implementation; `edge_sample_radius` is its CPU mirror
    // and what these assert against (see its docs). Each test states the property
    // that makes a treatment *that* treatment, so a future edit to the map has to
    // break a named claim rather than merely a pixel.

    /// 16:9, the aspect the whole out-of-disc question is sized at.
    const ASPECT_16_9: f32 = 16.0 / 9.0;

    /// Where the frame's corner sits, as a multiple of `r_max`.
    ///
    /// A centred fold in the shader's aspect-corrected space has `r_max = 0.5` and
    /// a corner at `0.5 * sqrt(aspect^2 + 1)`, so the ratio is `sqrt(aspect^2 + 1)`
    /// — independent of the frame's size, and the reason this is arithmetic rather
    /// than a measurement.
    fn corner_m(aspect: f32) -> f32 {
        (aspect * aspect + 1.0).sqrt()
    }

    /// The uniform never carries a fractional treatment or one outside the roster,
    /// whatever a binding — or the `[smoothing]` easing between two settings —
    /// hands the stage. `kaleido_order`'s guard, on `kaleido_order`'s seam, for the
    /// reason in [`fold_edge`]'s docs.
    #[test]
    fn fold_edge_is_always_a_roster_value() {
        for &raw in &[-9.0f32, 0.0, 0.4, 1.0, 2.2, 3.0, 3.9, 4.0, 4.4, 400.0] {
            let e = fold_edge(raw);
            assert_eq!(e, e.round(), "fold_edge({raw}) = {e} is not an integer");
            assert!(
                (DEFAULT_EDGE..=MAX_EDGE).contains(&e),
                "fold_edge({raw}) = {e} is outside the roster [{DEFAULT_EDGE}, {MAX_EDGE}]"
            );
        }

        // Nearest-integer, not truncation: an eased sweep from one treatment to
        // another must land on the step it is closest to, so it snaps at the
        // midpoint rather than lagging a whole treatment behind.
        assert_eq!(fold_edge(2.4), 2.0);
        assert_eq!(fold_edge(2.6), 3.0);
        assert_eq!(fold_edge(0.6), 1.0);

        // Out of range clamps into the roster rather than selecting the shader's
        // fall-through arm by arithmetic accident.
        assert_eq!(fold_edge(-1.0), DEFAULT_EDGE);
        assert_eq!(fold_edge(99.0), MAX_EDGE);

        // Non-finite falls back to the DEFAULT, not to a clamp bound: a selector
        // has no "as far as you can go" reading, so a broken binding lands on the
        // treatment that changes nothing.
        assert_eq!(fold_edge(f32::NAN), DEFAULT_EDGE);
        assert_eq!(fold_edge(f32::INFINITY), DEFAULT_EDGE);
        assert_eq!(fold_edge(f32::NEG_INFINITY), DEFAULT_EDGE);
    }

    /// Every treatment but `tile` keeps its **sample** radius inside the disc, so
    /// none of them can reconstruct a coordinate outside the source — ADR-0047's
    /// real guarantee, and the mechanism behind design-backlog 0010's smear.
    ///
    /// The sweep runs well past the 16:9 corner because that is where a plausible
    /// mis-implementation shows: `mirror` written with `floor` in place of `round`
    /// is the identity on `m` in `(1, 2)` and would sail past 1 here, while reading
    /// correctly at every `m` below the rim.
    #[test]
    fn only_tile_lets_the_sample_radius_leave_the_disc() {
        let mut m = 0.0f32;
        while m <= 3.0 {
            for edge in [0.0f32, 1.0, 2.0, 4.0] {
                let rs = edge_sample_radius(edge, m);
                assert!(
                    (0.0..=1.0).contains(&rs),
                    "treatment {edge} maps m = {m} to rs = {rs} r_max, outside the source"
                );
            }
            m += 0.01;
        }

        // Non-vacuity, and the roster's one deliberate exception: `tile` is the arm
        // whose coordinate leaves [0,1], which is why it — and only it — reads
        // through the MirrorRepeat sampler.
        let corner = corner_m(ASPECT_16_9);
        assert!(
            edge_sample_radius(3.0, corner) > 1.0,
            "tile no longer leaves the disc, so the check above distinguishes nothing"
        );

        // `falloff` and `vignette` differ only in their weight — the same clamped
        // radius map underneath, which is why neither can fill a corner.
        for &m in &[0.3f32, 1.0, corner] {
            assert_eq!(
                edge_sample_radius(0.0, m),
                edge_sample_radius(1.0, m),
                "falloff and vignette must share the clamped radius map"
            );
        }
    }

    /// `mirror` leaves the disc interior exactly as today's clamp does, and
    /// **reflects** about the rim rather than stepping across it.
    ///
    /// The reflection identity `rs(1 - h) == rs(1 + h)` is the property; continuity
    /// at `r_max` follows from it, since both sides tend to 1 as `h` shrinks. That
    /// is what makes the treatment a seamless mirror instead of a visible ring. The
    /// tolerance is f32 round-off in forming `1 +/- h`, not a tuned threshold — the
    /// map itself is exact.
    #[test]
    fn mirror_is_the_identity_inside_the_disc_and_reflects_about_its_rim() {
        const ROUNDOFF: f32 = 1e-6;

        for &h in &[1e-4f32, 1e-3, 1e-2, 0.1, 0.25, 0.5, 0.9, 1.0] {
            let inside = edge_sample_radius(2.0, 1.0 - h);
            let outside = edge_sample_radius(2.0, 1.0 + h);

            // Identity below r_max: inside the disc `mirror` is today's clamp, so
            // adopting it changes nothing about what the wedges already show.
            assert!(
                (inside - (1.0 - h)).abs() <= ROUNDOFF,
                "mirror is not the identity inside the disc: m = {} maps to {inside}",
                1.0 - h
            );
            // ...and the same distance outside reads the same radius.
            assert!(
                (inside - outside).abs() <= ROUNDOFF,
                "mirror does not reflect about r_max: m = {} reads {inside} but m = {} \
                 reads {outside} — a step of {h} either side of the rim leaves a seam",
                1.0 - h,
                1.0 + h
            );
        }
    }

    /// What `mirror` does at a corner, which decides how it reads and is not
    /// obvious: it brings the **fold axis** back into the corners, not a
    /// continuation of the rim outward.
    #[test]
    fn mirror_brings_the_fold_axis_into_a_16_9_corner() {
        let corner = corner_m(ASPECT_16_9);
        // The arithmetic the module docs quote: sqrt((16/9)^2 + 1) = 2.04.
        assert!(
            (corner - 2.0397).abs() < 1e-3,
            "the 16:9 corner is at {corner} r_max, not the documented 2.04"
        );

        // Past one full reflection: the triangle wave has period 2, so a corner at
        // 2.04 lands 0.04 out from the axis rather than 0.04 in from the rim.
        let rs = edge_sample_radius(2.0, corner);
        assert!(
            (rs - (corner - 2.0)).abs() <= 1e-6,
            "mirror at the 16:9 corner reads {rs}, not the corner's distance past the \
             second fold ({})",
            corner - 2.0
        );
        assert!(
            rs < 0.05,
            "the 16:9 corner samples from {rs} r_max — the module docs' claim that \
             mirror puts the centre of the figure in the corners rests on this being \
             right next to the axis"
        );
    }

    /// `squash` is 1:1 **at the axis** and asymptotic to the rim — it never crops
    /// and never leaves the disc, at the cost of compressing the whole interior.
    ///
    /// Note this is *not* the identity below `r_max` the way `mirror` is: Plan
    /// 0055 Phase 1's done-when groups the two, but `tanh(m) < m` for every
    /// `m > 0`, so `squash` pulls the disc's interior inward everywhere. The
    /// formula is the one both the plan's roster table and ADR-0061's give; the
    /// grouping in the prose is what does not hold, and ADR-0061's Outcome is where
    /// that belongs. `mirror` alone is the "interior untouched" candidate.
    #[test]
    fn squash_is_one_to_one_at_the_axis_and_asymptotic_to_the_rim() {
        // 1:1 at the fold axis: tanh'(0) = 1, so the ratio tends to 1. At m = 1e-4
        // the series error is ~m^2/3, some four orders below this bound.
        let tiny = 1e-4f32;
        assert!(
            (edge_sample_radius(4.0, tiny) / tiny - 1.0).abs() < 1e-5,
            "squash is not 1:1 at the fold axis"
        );

        // Asymptotic, never reaching: no crop, no ray, and nothing sampled outside
        // the source however far out the pixel is. `m` runs to 8 because a fold
        // centre clamped to the frame edge shrinks `r_max` without bound, so large
        // ratios are reachable and not merely hypothetical.
        //
        // Only non-decreasing out here, deliberately: past `m ~ 7.6` consecutive
        // steps of `tanh` land within one f32 ulp of each other, so "asymptotic"
        // and "constant" stop being distinguishable in the type. That costs
        // nothing — the guarantee is that the radius stays inside the disc, and it
        // does — but asserting strict growth there would be asserting a property of
        // f32 rather than of the map.
        let mut prev = 0.0f32;
        let mut m = 0.05f32;
        while m <= 8.0 {
            let rs = edge_sample_radius(4.0, m);
            assert!(rs < 1.0, "squash reached the rim at m = {m} (rs = {rs})");
            assert!(rs >= prev, "squash went backwards at m = {m}");
            prev = rs;
            m += 0.05;
        }

        // Strictly increasing across every ratio a frame actually presents: the
        // corner sits at `sqrt(aspect^2 + 1)` — 2.04 at 16:9, 2.28 at the portrait
        // shape the disc guard captures at — so 4 is comfortably past both. This is
        // the range in which "compresses without cropping" has to mean that
        // distinct radii stay distinct, or the corners flatten into a ring.
        let mut prev = 0.0f32;
        let mut m = 0.05f32;
        while m <= 4.0 {
            let rs = edge_sample_radius(4.0, m);
            assert!(
                rs > prev,
                "squash is not strictly monotone at m = {m}, which is inside the range \
                 a real frame reaches — distinct radii must stay distinct there"
            );
            prev = rs;
            m += 0.05;
        }

        // ...and it is a compression of the interior, not the identity there.
        assert!(
            edge_sample_radius(4.0, 0.5) < 0.5,
            "squash left the disc interior untouched — it is tanh, which compresses \
             everywhere past the axis"
        );
    }
}
