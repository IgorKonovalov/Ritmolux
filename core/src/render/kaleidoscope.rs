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
//! | Value | Name | `rs` | `w` | Reads through |
//! |---|---|---|---|---|
//! | 0 | `falloff` | `min(r, r_max)` | `1 - smoothstep(r_max, r_max*(1+band), r)` | `ClampToEdge` |
//! | **1** | **`tile`** (default) | `r` | `1` | **`MirrorRepeat`** |
//! | 2 | `squash` | `r_max * tanh(m)` | `1` | `ClampToEdge` |
//!
//! Plan 0055 shipped five candidates and its Phase 2 A/B — in the running app, in
//! motion, over a lit backdrop, on a centred figure and a border-filling field, at
//! two aspects — deleted two of them. `vignette` (the fade moved inside the disc)
//! and `mirror` (the radius reflected as a triangle wave) won on neither scene and
//! are gone from the shader rather than left dead. The three that remain keep their
//! **relative order** from that roster, which is why the numbering has a gap in its
//! history but not in its values.
//!
//! **The default is `tile` (1), not `falloff` (0)** — the one place this stage's
//! numbering is deliberately not "0 is the default". Two separate facts, kept
//! separate on purpose: `0 = falloff` preserves the "0 is what ADR-0047 shipped"
//! association that preset comments and this file's own history carry, and the A/B
//! then chose a *different* member of the roster as the resting behaviour. So a
//! preset that binds no `kaleido_edge` **fills its frame** rather than cropping to
//! a disc, and every fold-bearing golden baseline moved once, by hand, when that
//! landed.
//!
//! `falloff` and `squash` keep `rs` inside `[0, r_max]`, so they inherit ADR-0047's
//! real guarantee unchanged — the design-backlog 0010 smear came from
//! *reconstructing a coordinate outside the source* and handing it to
//! `ClampToEdge`, and neither does that. **`tile` is the exception, and it is now
//! the default**, which is the single most important thing to know about this
//! stage: its coordinate is *meant* to leave `[0,1]`, and that is safe **only**
//! because a `MirrorRepeat` sampler defines the read. Wired to the `ClampToEdge`
//! sampler it is design-backlog 0010 under a new name, unguarded by the disc
//! assertion (which `tile` is supposed to break) — see
//! `core/tests/kaleidoscope.rs`, where the guard that does catch it is the
//! ray-variance property.
//!
//! `squash` is **not** the identity inside the disc the way a clamp is: `tanh(m) <
//! m` for every `m > 0`, so it compresses the whole interior, 1:1 only in the limit
//! at the fold axis. That is the cost of its filling the frame without a crop or a
//! ray, and it is why a preset picks between it and `tile` by eye.
//!
//! `kaleido_edge` is the stage's **second stepped param**. Like `kaleido_order` it
//! is clamped and rounded on the CPU ([`fold_edge`]), for the [`fold_order`]
//! reason: `[smoothing]` and preset dissolves both sweep a param *continuously*
//! between two settings, and a selector swept through 1.5 is not a fourth treatment
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

/// The first value the `kaleido_edge` roster defines (0 = `falloff`).
const MIN_EDGE: f32 = 0.0;
/// The last value the `kaleido_edge` roster defines (2 = `squash`). Values past it
/// clamp here rather than selecting the shader's fall-through arm by accident.
const MAX_EDGE: f32 = 2.0;
/// `kaleido_edge` default — **1 = `tile`**, the treatment Plan 0055's live A/B
/// chose as the resting behaviour.
///
/// Deliberately not [`MIN_EDGE`]. `0 = falloff` keeps the roster's numbering tied
/// to ADR-0047's shipped treatment, which is what this file's history and the
/// preset comments refer to; which member of the roster is the *default* is a
/// separate question and the A/B answered it differently. Reordering the roster to
/// force the default to 0 would trade a readable history for a tidier constant.
const DEFAULT_EDGE: f32 = 1.0;

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
/// another: easing from `falloff` (0) to `squash` (2) passes through 0.5, 1.0,
/// 1.5, so it visits `tile` on the way. Rounding here means the sweep *snaps* at
/// each midpoint — `kaleido_order`'s documented cost, taken again knowingly —
/// instead of the shader receiving a value no arm defines. Doing it in Rust rather
/// than WGSL keeps that precondition visible on the CPU side, where the roster's
/// bounds live.
///
/// Non-finite falls back to the default rather than clamping (which is what
/// `fold_order` does with an infinity): a selector has no "as far as you can go"
/// reading, so a broken binding should land on the resting treatment. Note that
/// since the default is not the low bound, a clamp and the fallback are genuinely
/// different answers here — an under-driven binding lands on `falloff` while a
/// broken one lands on `tile`.
fn fold_edge(v: f32) -> f32 {
    if v.is_finite() {
        v.clamp(MIN_EDGE, MAX_EDGE).round()
    } else {
        DEFAULT_EDGE
    }
}

/// The roster's radius map, normalized: given a treatment and `m = r / r_max`,
/// the **sample** radius as a fraction of `r_max`.
///
/// **The shader below is the implementation; this is its CPU mirror**, and it
/// exists so the properties that make each treatment what it claims can be
/// asserted arithmetically rather than argued — that `falloff` and `squash` never
/// reconstruct a coordinate outside the source, and that `tile` is the one arm
/// that does. The two are kept identical by inspection. The *pixel-level* guards
/// on the shader itself live in `core/tests/kaleidoscope.rs`, which is where
/// `tile`'s real safety property is asserted: this function cannot see which
/// sampler an arm reads through, and for `tile` the sampler is the whole
/// guarantee.
///
/// Weight `w` is not mirrored here: it is a plain `smoothstep` on the one arm that
/// uses it and carries no such property.
#[cfg(test)]
fn edge_sample_radius(edge: f32, m: f32) -> f32 {
    // Half-step comparisons and the same arm order as the shader, so the two are
    // one function written twice rather than two functions that agree on the
    // roster's three values.
    if edge < 0.5 {
        m.min(1.0)
    } else if edge < 1.5 {
        m
    } else {
        m.tanh()
    }
}

const SHADER: &str = r#"
struct K {
    v: vec4<f32>, // x: order, y: angle, z: aspect, w: occlude (ADR-0085)
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
    // layout, one pass, one fetch.
    //
    // PRECONDITION: `u.c.w` is an integer in [0, 2] (`fold_edge` on the CPU side).
    // The comparisons are half-step so a quantized value can only land on its own
    // arm; the fall-through is `squash`. Note the DEFAULT is 1 (`tile`), which is
    // not the fall-through and not the first arm — see `DEFAULT_EDGE`.
    let edge = u.c.w;
    var rs = min(r, r_max);
    var w = 1.0;
    if (edge < 0.5) {
        // 0 `falloff` — ADR-0047's treatment. Clamping the SAMPLE radius, not the
        // output pixel's, is what keeps every reconstructed coordinate inside
        // [0,1]: beyond r_max the polar reconstruction used to land outside the
        // source and `ClampToEdge` smeared the border texel radially into the
        // streaks and chevrons of design-backlog 0010. Past the disc it fades out
        // rather than leaving a flat ring — a plain clamp does NOT leave one, since
        // the clamped sample still varies with angle, so the rim replicates outward
        // as a sunburst of rays (ADR-0047's Outcome). This is what fades those.
        w = 1.0 - smoothstep(r_max, r_max * (1.0 + band), r);
    } else if (edge < 1.5) {
        // 1 `tile`, THE DEFAULT — leave the radius alone and let the MirrorRepeat
        // sampler define the read. The only arm that samples outside [0,1]; safe
        // only because of that sampler, and the original defect if ever wired to
        // `samp`.
        rs = r;
    } else {
        // 2 `squash` — compress the radius asymptotically into the disc. 1:1 at
        // the fold axis (tanh'(0) = 1) and approaching r_max at the corners, so it
        // crops nothing and draws no ray, at the cost of bending geometry. NOT the
        // identity inside the disc: tanh(m) < m everywhere past the axis, so the
        // whole interior is pulled inward.
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
    if (edge > 0.5 && edge < 1.5) {
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
    //
    // Alpha is scaled once more by `occlude` — how much of the fold's coverage the
    // backdrop underneath resolves against (ADR-0085). 1.0 folding into a scratch
    // offscreen and by default, where the multiply is exact.
    let out = col * w;
    return vec4<f32>(out.rgb, out.a * u.v.w);
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
        // `falloff` and `squash` keep their sample radius inside the inscribed
        // disc, so they never sample outside [0,1] and this address mode is
        // unreachable for them — it stays `ClampToEdge` as the defined fallback.
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("kaleido-sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        // The third — `tile` (ADR-0061) — is the one whose coordinate leaves
        // `[0,1]` on purpose, and it is safe *only* because this sampler defines
        // what a read out there means. Reflecting rather than repeating is what
        // keeps the continuation continuous at the source border instead of
        // wrapping the far edge in. Built unconditionally so the layout shape does
        // not depend on a param value.
        //
        // Since Plan 0055 Phase 3 this is the sampler the DEFAULT reads through, so
        // it is on the path of every fold-bearing preset rather than an opt-in.
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
        // second address mode. Re-derived after Plan 0055 Phase 3 deleted `mirror`
        // and `vignette` rather than carried over: neither of those needed a
        // sampler, `tile` survived, so the shape is unchanged from Phase 1.
        //
        // That is one entry longer than the `[Uniform, Texture, Sampler]` shape
        // ADR-0058 records this layout under — a shape it shared with
        // `ink-bind-layout` — so the fold is now the more distinctive of the two
        // and that particular collision is off the list.
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
                v: [order, self.angle, aspect, fold.alpha_scale()],
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
        for &raw in &[-9.0f32, 0.0, 0.4, 1.0, 1.6, 2.0, 2.4, 400.0] {
            let e = fold_edge(raw);
            assert_eq!(e, e.round(), "fold_edge({raw}) = {e} is not an integer");
            assert!(
                (MIN_EDGE..=MAX_EDGE).contains(&e),
                "fold_edge({raw}) = {e} is outside the roster [{MIN_EDGE}, {MAX_EDGE}]"
            );
        }

        // Nearest-integer, not truncation: an eased sweep from one treatment to
        // another must land on the step it is closest to, so it snaps at the
        // midpoint rather than lagging a whole treatment behind.
        assert_eq!(fold_edge(0.4), MIN_EDGE);
        assert_eq!(fold_edge(0.6), 1.0);
        assert_eq!(fold_edge(1.4), 1.0);
        assert_eq!(fold_edge(1.6), MAX_EDGE);

        // Out of range clamps to the nearer BOUND, which is not the default —
        // the distinction that only exists because the default is the roster's
        // middle value. An under-driven binding lands on `falloff`, not on `tile`.
        assert_eq!(fold_edge(-1.0), MIN_EDGE);
        assert_eq!(fold_edge(99.0), MAX_EDGE);
        assert_ne!(
            fold_edge(-1.0),
            DEFAULT_EDGE,
            "clamping and the non-finite fallback must stay distinguishable"
        );

        // Non-finite falls back to the DEFAULT, not to a clamp bound: a selector
        // has no "as far as you can go" reading, so a broken binding lands on the
        // resting treatment.
        assert_eq!(fold_edge(f32::NAN), DEFAULT_EDGE);
        assert_eq!(fold_edge(f32::INFINITY), DEFAULT_EDGE);
        assert_eq!(fold_edge(f32::NEG_INFINITY), DEFAULT_EDGE);
    }

    /// `falloff` and `squash` keep their **sample** radius inside the disc, so
    /// neither can reconstruct a coordinate outside the source — ADR-0047's real
    /// guarantee, and the mechanism behind design-backlog 0010's smear. `tile` is
    /// the deliberate exception, and the reason it needs its own sampler.
    ///
    /// The sweep runs well past the 16:9 corner because a mis-implementation that
    /// only escapes far out would read correctly at every `m` below the rim.
    #[test]
    fn only_tile_lets_the_sample_radius_leave_the_disc() {
        let mut m = 0.0f32;
        while m <= 3.0 {
            for edge in [MIN_EDGE, MAX_EDGE] {
                let rs = edge_sample_radius(edge, m);
                assert!(
                    (0.0..=1.0).contains(&rs),
                    "treatment {edge} maps m = {m} to rs = {rs} r_max, outside the source"
                );
            }
            m += 0.01;
        }

        // Non-vacuity, and the roster's one deliberate exception — which is also
        // the DEFAULT, so this is the arm every unbound fold-bearing preset takes.
        // Its safety is the MirrorRepeat sampler, which this function cannot see;
        // the guard that can is the ray-variance property in
        // `core/tests/kaleidoscope.rs`.
        let corner = corner_m(ASPECT_16_9);
        assert!(
            edge_sample_radius(DEFAULT_EDGE, corner) > 1.0,
            "tile no longer leaves the disc, so the check above distinguishes nothing"
        );
    }

    /// `squash` is 1:1 **at the axis** and asymptotic to the rim — it never crops
    /// and never leaves the disc, at the cost of compressing the whole interior.
    ///
    /// Note this is *not* the identity below `r_max` the way a clamp is: Plan 0055
    /// Phase 1's done-when grouped `squash` with the (since-deleted) `mirror` as
    /// leaving the disc interior untouched, but `tanh(m) < m` for every `m > 0`, so
    /// `squash` pulls the whole interior inward. The formula is the one both the
    /// plan's roster table and ADR-0061's give; the grouping in the prose is what
    /// does not hold, and ADR-0061's Outcome carries the correction.
    #[test]
    fn squash_is_one_to_one_at_the_axis_and_asymptotic_to_the_rim() {
        // 1:1 at the fold axis: tanh'(0) = 1, so the ratio tends to 1. At m = 1e-4
        // the series error is ~m^2/3, some four orders below this bound.
        let tiny = 1e-4f32;
        assert!(
            (edge_sample_radius(MAX_EDGE, tiny) / tiny - 1.0).abs() < 1e-5,
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
            let rs = edge_sample_radius(MAX_EDGE, m);
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
            let rs = edge_sample_radius(MAX_EDGE, m);
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
            edge_sample_radius(MAX_EDGE, 0.5) < 0.5,
            "squash left the disc interior untouched — it is tanh, which compresses \
             everywhere past the axis"
        );
    }
}
