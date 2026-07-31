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

const SHADER: &str = r#"
struct K {
    v: vec4<f32>, // x: order, y: angle, z: aspect, w: unused
    c: vec4<f32>, // x,y: fold centre (uv), z: falloff band (fraction of r_max)
}

@group(0) @binding(0) var<uniform> u: K;
@group(0) @binding(1) var t_src: texture_2d<f32>;
@group(0) @binding(2) var samp: sampler;

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

    // Fold a DISC (ADR-0047). Clamping the SAMPLE radius — not the output pixel's
    // — is what keeps every reconstructed coordinate inside [0,1]: beyond r_max the
    // polar reconstruction used to land outside the source and `ClampToEdge`
    // smeared the border texel radially into the streaks and chevrons of
    // design-backlog 0010.
    let rs = min(r, r_max);
    // ...and past the disc, fade out rather than leaving a flat ring. A plain
    // clamp does NOT leave one: the clamped sample still varies with angle, so the
    // rim replicates outward as a sunburst of rays (ADR-0047's Outcome). This is
    // what fades those rays.
    let band = max(u.c.z, 0.001);
    let w = 1.0 - smoothstep(r_max, r_max * (1.0 + band), r);

    // Reconstruct the sample coordinate from the folded angle + sample radius.
    var q = vec2<f32>(cos(a), sin(a)) * rs;
    q.x = q.x / aspect;
    let s_uv = q + centre;
    // `w` scales COLOUR AND ALPHA together (ADR-0055). The values are
    // premultiplied, so this fades to *transparent* and the backdrop composited
    // underneath the chain shows through. Multiplying only `.rgb` and forcing
    // alpha to 1 is what made the falloff fade to black and fight `bg_*` instead
    // of landing on it.
    return textureSample(t_src, samp, s_uv) * w;
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
        // The fold clamps its sample radius to the inscribed disc, so it never
        // samples outside [0,1] and this address mode is unreachable — it stays
        // `ClampToEdge` as the defined fallback.
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("kaleido-sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
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
        let bind_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("kaleido-bind-layout"),
            entries: &[
                gpu::uniform(0, wgpu::ShaderStages::FRAGMENT),
                gpu::texture(1, true),
                gpu::sampler(2),
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
    }

    /// Apply one named parameter, returning whether it was a `kaleido_*` param.
    fn set_param(&mut self, name: &str, value: f32) -> bool {
        match name {
            "kaleido_order" => self.order = value,
            "kaleido_angle" => self.angle = value,
            "kaleido_center_x" => self.center_x = value,
            "kaleido_center_y" => self.center_y = value,
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
                    0.0,
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
}
