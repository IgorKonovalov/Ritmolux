//! Particle-swarm scene: ~10k CPU-simulated particles drifting through a flow
//! field in a perspective volume, drawn as instanced additive sprites projected
//! through the shared camera (ADR-0259). One of the two preset-driven systems
//! (ADR-0002 layers 1-2).
//!
//! Its behavior is a set of named parameters — `force`, `spin`, `burst`, `hue`,
//! `brightness`, `size` — that a preset binds to expressions over the audio
//! analysis (Plan 0003 Phase 5), plus a subset of the shared camera block. All
//! per-particle math is CPU-side; no compute shader. Motion is deterministic;
//! the only randomness is the seeded initial scatter (NFR 6).
//!
//! # The world is a torus in frustum coordinates
//!
//! A particle holds `(u, v, z)`: `z` is a view depth inside the slab
//! [`Z_NEAR`]`..`[`Z_FAR`], and `u`, `v` in `[-1, 1)` are its place across the
//! frustum's cross-section **at its own depth**, times [`MARGIN`]. Its world
//! position is `(u * hx(z), v * hy(z))` with `hx`, `hy` the rest frustum's
//! half-extents at `z` scaled by the margin ([`half_extent`]). The torus wraps
//! in `u` and `v`, so the seam sits the same margin outside the frame at every
//! depth — a box of fixed world bounds would show its seam at one end of the
//! slab or waste its population at the other (ADR-0259).
//!
//! The flow is evaluated in world space and a world velocity is converted back
//! by the half-extent at the particle's depth, so a far particle crosses the
//! screen more slowly than a near one under the same current: motion parallax
//! out of the simulation itself. `z` rides a slow component of the same field,
//! wraps across the slab, and a particle's light fades to zero in a band at
//! either slab bound, so the depth wrap never pops.
//!
//! # The camera is a subset of the shared block
//!
//! The eye orbits the slab's centre at [`PIVOT`] and looks down `-z`. `yaw` and
//! `pitch` are held to a sway the margin covers ([`sway_bound`]), and there is
//! no `distance`: the slab is defined relative to the camera, so an orbit would
//! show the edge of the world. `zoom` divides the field of view and `pan_*`
//! shift after the projection (ADR-0257); neither re-maps the simulation, and a
//! `zoom` below the margin shows the seam.

// Hot-path panic-denial pragma (Plan 0002 Phase 2, extended to scenes by Plan
// 0003 Phase 0). Runs every displayed frame.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

use super::common;
use super::marks;
use super::{FALLBACK_DT, Phase, Scene, SeededRng};
use crate::dsp::AnalysisFrame;
use crate::render::camera::{self, CameraFrame, CameraParams, CameraUniform};
use crate::render::gpu;
use crate::render::palette::{self, Palette};
use crate::render::scenes::{ParamGroup, ParamKind, ParamSpec, default_of};

// The ASCII bytes of "LMV_SWRM" read as a number. Re-spelling them to
// match a renamed prefix changes every particle's start state and moves
// this scene's goldens, so the value is opaque and stays as it is.
const SEED: u64 = 0x4C4D_565F_5357_524D;

/// How far the torus extends past the visible frame, as a multiple of the rest
/// frustum's half-extent at every depth (ADR-0044, ADR-0259).
///
/// At `1.0` the wrap seam would sit on the frame edge, the one line on screen
/// every wrapping particle is guaranteed to paint, and the feedback stage
/// integrates that into a saturated bar within a few hundred frames. At 1.25 the
/// seam projects to normalized-device `±1.25` at rest, which leaves a quarter of
/// the frame's half-extent for `pan_*` and the camera sway to share
/// ([`sway_bound`]).
///
/// The cost is visible density: the visible fraction of the cross-section is
/// `1 / MARGIN^2`, so a third of the population is off-screen at any moment.
const MARGIN: f32 = 1.25;
/// Domain aspect before the first [`Scene::render`] hands one over. Only reached
/// on the very first `update` of a fresh scene; because positions are stored in
/// frustum coordinates an aspect change rescales the field rather than
/// teleporting it, so this fallback is continuous with whatever follows.
const FALLBACK_ASPECT: f32 = 16.0 / 9.0;

/// Velocity retained per frame (the rest is re-steered by the flow field).
const DAMPING: f32 = 0.86;

// --- The slab ----------------------------------------------------------------
//
// Depth is a view depth in camera units, and never a sort: the scene blends
// additively, and addition is commutative, so draw order is irrelevant and no
// depth buffer is needed (ADR-0044, ADR-0259).

/// The nearest view depth a particle reaches.
const Z_NEAR: f32 = 1.4;
/// The farthest. `Z_FAR / Z_NEAR` is the near-to-far ratio of on-screen sprite
/// size and of screen speed under the same world current.
const Z_FAR: f32 = 3.8;
/// The slab's depth.
const Z_SPAN: f32 = Z_FAR - Z_NEAR;
/// The eye's distance from the orbit target, which is the slab's centre: a sway
/// turns about the middle of the volume, so the near and far layers slide past
/// each other in opposite directions.
const PIVOT: f32 = 0.5 * (Z_NEAR + Z_FAR);
/// The view depth at which a sprite's `size` is stated: nearer draws larger and
/// farther smaller, by `SIZE_DEPTH / depth`. It is where `ln(Z_FAR / Z_NEAR) /
/// Z_SPAN`, the slab's mean of `1 / depth`, puts the mean sprite at its stated
/// size, and where the rest frame's half-height is one world unit to within
/// 2 %, so `field_freq` keeps the scale it is documented at.
const SIZE_DEPTH: f32 = 2.4;
/// The field of view the torus is sized against: the shared camera's resting
/// `fov`. A wider bound `fov` shows more of the margin, and past
/// `2 * atan(MARGIN * tan(REST_FOV / 2))` the seam.
const REST_FOV: f32 = camera::FOV.default;
/// The share of a slab bound's depth over which a particle's light fades to
/// zero before it wraps to the other bound.
const FADE_BAND: f32 = 0.12;
/// The flow's phase per unit of world depth, in radians. **This is the term
/// that makes it read as volume**: layers at different depths ride genuinely
/// different currents, so they cross and separate the way real depth does.
/// Sized so the whole slab spans 2.6 radians, a large fraction of the field's
/// `TAU` period — enough to decorrelate the layers, short of wrapping them back
/// onto each other.
const DEPTH_FIELD_FREQ: f32 = 2.6 / Z_SPAN;
/// The depth current's share of `force`. Small, so a particle takes tens of
/// seconds to cross the slab and depth reads as layering, not as rushing.
const Z_FLOW: f32 = 0.3;
/// The share of [`sway_bound`]'s first-order bound the sway may take: the rest
/// absorbs the second-order terms that model leaves out.
const SWAY_SHARE: f32 = 0.8;
/// The light of a particle at the near slab bound, under `depth_fade`. With the
/// default fade it falls to 0.45 at the far bound: the ramp ADR-0044 tuned.
const DEPTH_LIGHT_NEAR: f32 = 1.05;

/// Parameter defaults — a calm idle drift when nothing is bound.
const DEFAULT_FORCE: f32 = default_of(PARAMS, "force");
const DEFAULT_SPIN: f32 = default_of(PARAMS, "spin");
const DEFAULT_BURST: f32 = default_of(PARAMS, "burst");
const DEFAULT_HUE: f32 = 0.0;
const DEFAULT_BRIGHTNESS: f32 = 0.8;
const DEFAULT_SIZE: f32 = default_of(PARAMS, "size");
/// Spatial frequency of the flow field — how many vortices fit across a world
/// unit, and so how many distinct streams a frame can hold (Plan 0043 Phase 2).
///
/// It is this scene's first structural lever. Low values give a few broad
/// currents that many particles share — which is where the family's apparent
/// flocking comes from, since neighbours on one streamline travel together — and
/// high values give many tight swirls. `spin` says how fast the field is rewritten;
/// this says how finely it is divided.
const DEFAULT_FIELD_FREQ: f32 = default_of(PARAMS, "field_freq");
// Per-mark individuation (Plan 0077 Phase 2, backlog 0068). Both default OFF —
// unlike the emitter's spreads, which default non-zero, the swarm's scatter
// already ships a seeded per-particle size and brightness, so these *widen*
// what is there and their defaults leave every capture byte-identical.
const DEFAULT_TWINKLE: f32 = default_of(PARAMS, "twinkle");
const DEFAULT_SIZE_SPREAD: f32 = default_of(PARAMS, "size_spread");
/// `depth_fade` at rest: the share of [`DEPTH_LIGHT_NEAR`] a particle loses
/// from the near slab bound to the far one, chosen so the far bound keeps 0.45.
const DEFAULT_DEPTH_FADE: f32 = default_of(PARAMS, "depth_fade");
/// The per-particle twinkle rate band, Hz — the emitter's values
/// (`emitter.rs`), kept equal so `twinkle` means one thing across the two
/// particle scenes. The spread across particles is the point, not the values:
/// a field of oscillators sharing one rate flashes as one sheet however their
/// phases scatter, so the **rate is drawn per particle as well as the phase**
/// — that is what keeps the whole-frame mean steady while every member of it
/// swings (backlog 0068's measurement).
const TWINKLE_FREQ_LO: f32 = 0.35;
const TWINKLE_FREQ_HI: f32 = 1.6;
/// `reseed` rises past this to disturb the population once — edge-triggered,
/// the attractor's constant and its reason (`particles/mod.rs`): a sustained
/// beat flag must not disturb every frame.
const RESEED_THRESHOLD: f32 = 0.5;
/// Fraction of the cross-section's normalized half-extent one `reseed` kick
/// spans per axis (Plan 0077 Phase 3, ADR-0066 semantics): the kick disturbs the
/// population **where it is**, sized from the swarm's own domain the way
/// `AttractorFamily::jitter_extent` derives from `seed_box` — *not* a respawn
/// into a uniform box, which is the artifact class ADR-0066 removed and
/// backlog 0064 caught returning once already. Positions are frustum
/// coordinates, so a fraction here is domain-relative on any target, at any
/// depth.
///
/// The value is the attractor's measured `JITTER_FRACTION`, adopted as the
/// starting magnitude for the same figure-relative kick. ADR-0066 records the
/// magnitude as the lever if the disturbance reads too subtle; returning to a
/// box re-fill is not.
const RESEED_KICK: f32 = 0.06;
// Shared palette color knobs (ADR-0021). Each particle's hue occupies the band
// `hue_center + (particle_hue - 0.5) * hue_spread`; the defaults (`center = 0.5`,
// `spread = 1`) reproduce the full-wheel look (`particle_hue`), and
// `saturation = 1` leaves color untouched — so an unbound swarm is unchanged.
const DEFAULT_HUE_SPREAD: f32 = default_of(PARAMS, "hue_spread");
const DEFAULT_HUE_CENTER: f32 = default_of(PARAMS, "hue_center");
// Shared view transform (ADR-0018): identity by default. `zoom` divides the
// camera's field of view and `pan_*` shift after the projection (ADR-0257).
const DEFAULT_ZOOM: f32 = 1.0;
// The mark silhouette (ADR-0084). `disc` is exactly the arithmetic the sprite
// drew before the roster existed, so an unbound swarm is unchanged.
const DEFAULT_SHAPE: f32 = marks::DEFAULT_SHAPE;
const DEFAULT_POINTS: f32 = marks::DEFAULT_POINTS;
/// The `star` arm's three shape params (Plan 0091 Phase 5), aliased beside the
/// other two mark defaults so this scene states its whole vocabulary locally.
const DEFAULT_STAR_VALLEY: f32 = marks::DEFAULT_STAR_VALLEY;
const DEFAULT_STAR_CURVE: f32 = marks::DEFAULT_STAR_CURVE;
const DEFAULT_STAR_JITTER: f32 = marks::DEFAULT_STAR_JITTER;
const DEFAULT_STAR_SEED: f32 = marks::DEFAULT_STAR_SEED;
const DEFAULT_STAR_WOBBLE: f32 = marks::DEFAULT_STAR_WOBBLE;
const DEFAULT_STAR_WOBBLE_FREQ: f32 = marks::DEFAULT_STAR_WOBBLE_FREQ;

/// The scene's own WGSL. The shared camera ([`camera::CAMERA_WGSL`]) and the
/// shared mark-silhouette chunk ([`marks::sdf_wgsl`]) are prepended at module
/// creation, so `project()` here is the function every 3D pipeline projects
/// through and `mark_distance` is the same function the emitter evaluates.
///
/// **`shape` and `points` travel vertex -> fragment as flat varyings rather than
/// being read from `misc` in the fragment stage.** The fragment stage cannot see
/// this scene's uniforms without widening the bind layout's visibility, and a
/// flat varying carries a per-draw value with no descriptor change at all
/// (ADR-0058).
const SHADER: &str = r#"
struct Misc {
    // x: the render target's aspect, y: the view depth a sprite's size is
    // stated at, zw: unused.
    v: vec4<f32>,
    // x: mark shape position, y: quantized point count (ADR-0084), z: the star
    // arm's arrangement seed, w: its edge-wobble amplitude. Per draw, not per
    // instance: the branch stays uniform across a warp and the instance does
    // not grow.
    m: vec4<f32>,
    // xyz: the star arm's shape params (valley, curve, jitter), w: the edge
    // wobble's frequency — all conditioned CPU-side (Plan 0091 Phase 5). Per
    // draw, like `m`. Inert on every other shape, and at their defaults the arm
    // takes its original closed form.
    s: vec4<f32>,
}

@group(0) @binding(0) var<uniform> misc: Misc;
@group(0) @binding(1) var<uniform> cam: Camera;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) local: vec2<f32>,
    @location(1) color: vec3<f32>,
    @location(2) @interpolate(flat) shape: f32,
    @location(3) @interpolate(flat) points: f32,
    @location(4) @interpolate(flat) star: vec3<f32>,
    @location(5) @interpolate(flat) rough: vec3<f32>,
    @location(6) @interpolate(flat) presence: f32,
    // How far the drawn quad reaches past the sharp sprite: `(r + coc) / r`,
    // exactly 1 where the sprite is in focus.
    @location(7) @interpolate(flat) reach: f32,
}

@vertex
fn vs_main(
    @builtin(vertex_index) vi: u32,
    @location(0) center: vec3<f32>,
    @location(1) radius: f32,
    @location(2) color: vec3<f32>,
    @location(3) presence: f32,
) -> VsOut {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(0.0, 1.0),
        vec2<f32>(0.0, 1.0), vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0),
    );
    let c = corners[vi] * 2.0 - vec2<f32>(1.0, 1.0);
    // Every particle sits inside the slab, far in front of the near plane, so
    // the divide by `w` is safe without a CPU clip.
    let clip = project(cam, center);
    let depth = clip.w;
    // `radius` is the sprite's half-extent in normalized-device height at the
    // reference depth; perspective carries it to this one. The x half-extent
    // is divided by the target's aspect so the sprite is round on screen.
    let r = radius * misc.v.y / depth;
    // Depth of field (ADR-0257): the circle of confusion at this depth, in
    // pixels of the texture drawn into, grows the sprite by `(r + coc) / r` and
    // dims its light by the area factor, so a blurred sprite spreads its light
    // over the larger disc rather than adding to it. At `coc = 0` both factors
    // are exactly 1 and the sprite is the sharp one, bit for bit.
    let r_px = r * 0.5 * cam.viewport.y;
    let blur = coc(cam, depth);
    let reach = select(1.0, (r_px + blur) / r_px, r_px > 0.0);
    let keep = 1.0 / (reach * reach);
    let ndc = clip.xy / depth + c * (r * reach) * vec2<f32>(1.0 / misc.v.x, 1.0);
    var out: VsOut;
    out.pos = vec4<f32>(ndc, 0.0, 1.0);
    // The silhouette is sampled out to the grown radius, in the sharp sprite's
    // own units, so its shape is unchanged and only its edge widens.
    out.local = c * reach;
    out.reach = reach;
    out.color = color * keep;
    out.shape = misc.m.x;
    out.points = misc.m.y;
    out.star = misc.s.xyz;
    out.rough = vec3<f32>(misc.m.z, misc.m.w, misc.s.w);
    out.presence = presence;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    // The silhouette (ADR-0084). At the default `disc` this is `length(in.local)`
    // and nothing else; the falloff below is untouched either way, so a visual
    // change is attributable to the shape alone.
    let d = mark_distance(in.local, in.shape, in.points, in.star, in.rough);
    // The falloff stretched to reach zero at the grown radius rather than the
    // sharp one: `d` scales with radius on every arm, so its iso-lines keep the
    // silhouette's shape and a blurred polygon reads as a soft polygon. At
    // `reach = 1` this is `1 - d`.
    let falloff = max(0.0, 1.0 - d / in.reach);
    let g = falloff * falloff;
    // Premultiplied: colour AND alpha carry the same coverage `g`, so the four
    // corners outside the inscribed disc write nothing at all rather than
    // opaque black (ADR-0056). See `gpu::ADDITIVE_LIGHT_SATURATING_COVERAGE`.
    // The colour already carries `presence`; the coverage takes it here.
    return vec4<f32>(in.color * g, g * in.presence);
}
"#;

/// One sprite as the GPU reads it.
///
/// **Field order is shader-location order**: `vertex_attr_array!` assigns
/// locations by declaration order and offsets by field order, so a field
/// inserted anywhere but the end re-points every attribute after it.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
struct SwarmInstance {
    /// Centre, world space.
    center: [f32; 3],
    /// Half-extent in normalized-device height at [`SIZE_DEPTH`].
    radius: f32,
    /// Premultiplied light (ADR-0056): the colour already scaled by brightness
    /// and by `presence`.
    color: [f32; 3],
    /// How present the particle is, `0..1`: its [`slab_fade`]. It scales the
    /// sprite's coverage as the CPU scaled its light, so a particle fading out
    /// at a slab bound stops holding the backdrop out as it stops lighting it.
    presence: f32,
}

/// The per-draw uniform at binding 0: `v` is `[aspect, SIZE_DEPTH, 0, 0]`, `m`
/// is `[shape, points, star_seed, star_wobble]` (ADR-0084) and `s` is
/// `[star_valley, star_curve, star_jitter, star_wobble_freq]`, quantized and
/// conditioned on the way in.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct SwarmUniform {
    v: [f32; 4],
    m: [f32; 4],
    s: [f32; 4],
}

/// The swarm's sprite pipeline: its instance buffer, its two uniforms and the
/// draw.
///
/// **The layout is this scene's own shape** — the silhouette uniform with no
/// declared size, then the camera with its size — which no other layout in
/// `core/src` shares (ADR-0058).
struct Sprites {
    pipeline: wgpu::RenderPipeline,
    instances: wgpu::Buffer,
    misc: wgpu::Buffer,
    camera: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

impl Sprites {
    fn new(device: &wgpu::Device, capacity: usize, target_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("swarm-shader"),
            // The shared camera, then the shared silhouette chunk, then this
            // scene's own source — one `project()` for every 3D pipeline, one
            // `mark_distance` for both mark scenes (ADR-0257, ADR-0084).
            source: wgpu::ShaderSource::Wgsl(
                format!("{}\n{}{SHADER}", camera::CAMERA_WGSL, marks::sdf_wgsl()).into(),
            ),
        });
        let bind_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("swarm-bind-layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(
                            std::mem::size_of::<CameraUniform>() as u64,
                        ),
                    },
                    count: None,
                },
            ],
        });
        let instances = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("swarm-instances"),
            size: (capacity.max(1) * std::mem::size_of::<SwarmInstance>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let misc = gpu::uniform_buffer(device, "swarm-misc", std::mem::size_of::<SwarmUniform>());
        let camera =
            gpu::uniform_buffer(device, "swarm-camera", std::mem::size_of::<CameraUniform>());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("swarm-bind-group"),
            layout: &bind_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: misc.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: camera.as_entire_binding(),
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("swarm-pipeline-layout"),
            bind_group_layouts: &[Some(&bind_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("swarm-pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<SwarmInstance>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &wgpu::vertex_attr_array![
                        0 => Float32x3,
                        1 => Float32,
                        2 => Float32x3,
                        3 => Float32,
                    ],
                })],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    // The seam every additive mark draws through (ADR-0056).
                    blend: Some(gpu::ADDITIVE_LIGHT_SATURATING_COVERAGE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        Self {
            pipeline,
            instances,
            misc,
            camera,
            bind_group,
        }
    }

    /// Write this frame's uniforms and sprites and encode the draw, **loading**
    /// over what is already in `view`. The pass is begun even when there is
    /// nothing to draw, so an idle frame encodes the same load/store pair.
    fn draw(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        misc: &SwarmUniform,
        camera: &CameraUniform,
        sprites: &[SwarmInstance],
    ) {
        let mut pass = gpu::color_pass(encoder, "swarm-pass", view, wgpu::LoadOp::Load);
        if sprites.is_empty() {
            return;
        }
        queue.write_buffer(&self.instances, 0, bytemuck::cast_slice(sprites));
        queue.write_buffer(&self.misc, 0, bytemuck::bytes_of(misc));
        queue.write_buffer(&self.camera, 0, bytemuck::bytes_of(camera));
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.set_vertex_buffer(0, self.instances.slice(..));
        pass.draw(0..6, 0..sprites.len() as u32);
    }
}

struct Particle {
    /// Position across the frustum's cross-section **at the particle's own
    /// depth**, each axis in `[-1, 1)`; world position is this times
    /// [`half_extent`] at `z`.
    ///
    /// Frustum coordinates rather than world space for two reasons. The
    /// half-extents follow the render target, so a resize rescales the field
    /// continuously instead of re-wrapping it (ADR-0044); and the torus wraps
    /// at `±1` here whatever the depth, which is what keeps the seam outside
    /// the frame at every depth (ADR-0259). It also keeps the seeded scatter
    /// aspect-independent, so the same seed gives the same field at any target
    /// size (NFR §6).
    pos: [f32; 2],
    /// View depth at rest, in `Z_NEAR..Z_FAR`.
    z: f32,
    /// Velocity in **world** units per second — the flow field and the burst are
    /// world-space forces, so they must not change magnitude with the domain.
    vel: [f32; 3],
    /// Per-particle twinkle oscillator (Plan 0077 Phase 2): rate in Hz from
    /// the `TWINKLE_FREQ_LO..HI` band and phase in cycles, both off the
    /// particle's stable identity through [`unit`]. Fixed for the particle's
    /// life; resolved into a brightness factor at draw only when `twinkle`
    /// is bound.
    twinkle_freq: f32,
    twinkle_phase: f32,
    /// The particle's unit draw for `size_spread`, resolved at draw time so an
    /// eased spread moves the whole population continuously (the emitter's
    /// reasoning for draw-time resolution, verbatim).
    size_unit: f32,
    /// Per-particle palette offset and brightness, from the seeded scatter.
    hue: f32,
    bright: f32,
    size: f32,
}

/// ~10k-particle CPU flow-field swarm, driven by named preset parameters.
pub struct SwarmScene {
    sprites: Sprites,
    particles: Vec<Particle>,
    /// This frame's sprites, rebuilt in place every `update`, so the per-frame
    /// path never allocates.
    instance_data: Vec<SwarmInstance>,
    /// Shared scene clock (seconds), set by the renderer each frame.
    time: f32,
    /// The **render target's** aspect, recorded by `render` for the next `update`
    /// to size the frustum torus from.
    ///
    /// Read off `render`'s argument and deliberately **not** off
    /// [`Scene::set_target_size`](super::Scene::set_target_size), which carries the
    /// post chain's internal grid — a quantized *resolution*, not a shape, whose
    /// aspect is only approximately the target's (ADR-0037). Every swarm preset
    /// composes `trails`, so that grid is exactly the quantized case; taking a
    /// domain shape from it is the defect ADR-0037 was written for.
    ///
    /// One frame behind by construction: `update` runs before `render` in a frame,
    /// so the domain follows the target with a single frame of lag. Harmless —
    /// positions are frustum coordinates, so a change rescales the field
    /// continuously.
    aspect: f32,
    /// The pixel size of the texture this scene draws into, for the camera
    /// uniform's viewport. A resolution only: no shape is taken from it.
    target: (u32, u32),
    /// Real elapsed seconds for this frame's integration (Plan 0014 Phase 2),
    /// injected via `advance` so the swarm moves at the same wall-clock rate on
    /// any refresh. Seeded to the fallback step for the first frame before any
    /// `advance` call.
    dt: f32,
    force: f32,
    spin: f32,
    /// The curl-noise field's own clock, integrated at `spin` ([`Phase`]).
    ///
    /// **Not `time * spin`** (ADR-0135). `spin` is a rate shipped worlds bind to
    /// a band, and under the multiply a binding that moved rescaled every second
    /// already elapsed: at t = 100 s a 0.04 swing advanced this clock by 4 s in a
    /// single frame against a nominal 0.019 s, and the field re-rolled rather
    /// than flowing on.
    field_phase: Phase,
    burst: f32,
    /// The shared palette knobs (ADR-0021).
    colour: common::PaletteParams,
    /// The shared view transform (ADR-0018).
    pan: common::PanParams,
    /// The camera subset this scene splices: `yaw`, `pitch`, `fov`, `focus`
    /// and `aperture`, raw as bound. `distance`, `fog` and `solid` stay at
    /// their resting values and are replaced when the frame is built.
    camera: CameraParams,
    size: f32,
    field_freq: f32,
    zoom: f32,
    depth_fade: f32,
    /// The active baked palette (ADR-0021), sampled per particle on the CPU. Set
    /// by `set_palette` on a preset switch; default `spectrum`.
    palette: Palette,
    /// Per-particle hue band + shared desaturation (ADR-0021).
    hue_spread: f32,
    hue_center: f32,
    /// The mark silhouette and its point count, **as bound** (ADR-0084). Both
    /// are conditioned on the way to the uniform rather than here, and the two
    /// conditionings differ: `points` is quantized, so a `[smoothing]`-eased
    /// binding steps at the midpoints (see [`marks::mark_points`]), while
    /// `shape` is only clamped, so an eased binding travels between two arms
    /// (ADR-0226, [`marks::mark_shape`]).
    shape: f32,
    points: f32,
    /// The `star` arm's three shape params, raw as the preset bound them
    /// (Plan 0091 Phase 5). `marks::star_*` condition them on the way to the
    /// uniform. Inert on every other silhouette, and nothing warns —
    /// `presets/README.md` carries that.
    star_valley: f32,
    star_curve: f32,
    star_jitter: f32,
    star_seed: f32,
    star_wobble: f32,
    star_wobble_freq: f32,
    /// Per-mark individuation (Plan 0077 Phase 2): the twinkle depth and the
    /// size-spread width, both resolved per particle at draw.
    twinkle: f32,
    size_spread: f32,
    /// This frame's `reseed` level (bound to a beat/onset expression); its
    /// rising edge past [`RESEED_THRESHOLD`] disturbs the population once
    /// (Plan 0077 Phase 3, ADR-0066 semantics).
    reseed: f32,
    /// Previous frame's `reseed`, for rising-edge detection.
    prev_reseed: f32,
    /// How many reseeds have fired. Salts the per-particle kick draw so
    /// successive reseeds scatter differently (the attractor's convention).
    reseed_count: u32,
    /// The tier's cap on a sprite's circle of confusion, in pixels.
    max_coc: f32,
    /// This frame's blur clamp, when `aperture` is past `max_coc`, for the
    /// renderer to announce (ADR-0007: a cap is never silent).
    clamp: Option<super::CapOverflow>,
}

impl SwarmScene {
    /// Build the pipeline, buffers, and seeded particle set on `device`.
    /// `particles` is the active tier's
    /// [`swarm_particles`](crate::render::TierConfig::swarm_particles). The count
    /// is fixed for the life of the scene — the instance buffer and the CPU
    /// mirror are both sized to it here, so the per-frame path never allocates —
    /// and a tier change rebuilds the scene rather than resizing it. Blur is
    /// held to `max_coc` pixels, the tier's cap.
    pub fn new(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        particles: usize,
        max_coc: f32,
    ) -> Self {
        let sprites = Sprites::new(device, particles, surface_format);

        let mut rng = SeededRng::new(SEED);
        // The individuation draws come off the particle's index through `unit`,
        // NOT off `rng`: an extra `SeededRng` draw per particle would shift the
        // stream for every draw after it and re-scatter the whole field.
        let particle_state: Vec<Particle> = (0..particles)
            .map(|i| {
                let mut p = Self::spawn(&mut rng);
                let seed = i as u32;
                p.twinkle_freq = TWINKLE_FREQ_LO
                    + unit(seed, channel::TWINKLE_FREQ) * (TWINKLE_FREQ_HI - TWINKLE_FREQ_LO);
                p.twinkle_phase = unit(seed, channel::TWINKLE_PHASE);
                p.size_unit = unit(seed, channel::SIZE);
                p
            })
            .collect();

        Self {
            sprites,
            particles: particle_state,
            instance_data: vec![
                SwarmInstance {
                    center: [0.0, 0.0, 0.0],
                    radius: 0.0,
                    color: [0.0, 0.0, 0.0],
                    presence: 0.0,
                };
                particles
            ],
            time: 0.0,
            aspect: FALLBACK_ASPECT,
            target: (1, 1),
            dt: FALLBACK_DT,
            force: DEFAULT_FORCE,
            spin: DEFAULT_SPIN,
            field_phase: Phase::default(),
            burst: DEFAULT_BURST,
            colour: common::PaletteParams::new(DEFAULT_HUE, DEFAULT_BRIGHTNESS),
            pan: common::PanParams::default(),
            camera: rest_camera(),
            size: DEFAULT_SIZE,
            field_freq: DEFAULT_FIELD_FREQ,
            zoom: DEFAULT_ZOOM,
            depth_fade: DEFAULT_DEPTH_FADE,
            palette: Palette::default_spectrum(),
            hue_spread: DEFAULT_HUE_SPREAD,
            hue_center: DEFAULT_HUE_CENTER,
            shape: DEFAULT_SHAPE,
            points: DEFAULT_POINTS,
            star_valley: DEFAULT_STAR_VALLEY,
            star_curve: DEFAULT_STAR_CURVE,
            star_jitter: DEFAULT_STAR_JITTER,
            star_seed: DEFAULT_STAR_SEED,
            star_wobble: DEFAULT_STAR_WOBBLE,
            star_wobble_freq: DEFAULT_STAR_WOBBLE_FREQ,
            twinkle: DEFAULT_TWINKLE,
            size_spread: DEFAULT_SIZE_SPREAD,
            reseed: 0.0,
            prev_reseed: 0.0,
            reseed_count: 0,
            max_coc,
            clamp: None,
        }
    }

    /// A particle scattered across the slab with a random heading and tint.
    ///
    /// The scatter is in **frustum** coordinates, so it does not depend on the
    /// render target — the same seed gives the same field at any size (NFR §6).
    fn spawn(rng: &mut SeededRng) -> Particle {
        let angle = rng.range(0.0, std::f32::consts::TAU);
        Particle {
            pos: [rng.range(-1.0, 1.0), rng.range(-1.0, 1.0)],
            vel: [angle.cos() * 0.2, angle.sin() * 0.2, 0.0],
            z: Z_NEAR + rng.next_f32() * Z_SPAN,
            hue: rng.next_f32(),
            bright: rng.range(0.5, 1.0),
            size: rng.range(0.004, 0.011),
            // Neutral; `new` overwrites all three from the particle's index.
            // Deliberately not drawn from `rng` — see the comment there.
            twinkle_freq: 0.0,
            twinkle_phase: 0.0,
            size_unit: 0.5,
        }
    }

    /// This frame's camera onto a render target of `aspect`: the bound subset
    /// with `yaw` and `pitch` held to [`sway_bound`], the eye at [`PIVOT`], and
    /// the slab as the volume `focus` is stated across.
    fn camera_frame(&self, aspect: f32, max_coc: f32) -> CameraFrame {
        let pan = [self.pan.x, self.pan.y];
        let [yaw_max, pitch_max] = sway_bound(self.camera.fov, self.zoom, aspect, pan);
        let finite = |v: f32| if v.is_finite() { v } else { 0.0 };
        let params = CameraParams {
            yaw: finite(self.camera.yaw).clamp(-yaw_max, yaw_max),
            pitch: finite(self.camera.pitch).clamp(-pitch_max, pitch_max),
            ..rest_camera_with(self.camera)
        };
        params.frame(aspect, self.zoom, pan, self.target, 0.5 * Z_SPAN, max_coc)
    }
}

/// The shared camera block at this scene's rest: eye on the slab's axis with no
/// sway, at [`PIVOT`], every other value at the shared default.
fn rest_camera() -> CameraParams {
    CameraParams {
        yaw: 0.0,
        pitch: 0.0,
        ..rest_camera_with(CameraParams::default())
    }
}

/// `bound` with the values this scene does not offer put back: `distance` at
/// [`PIVOT`], and neither `fog` nor `solid`.
fn rest_camera_with(bound: CameraParams) -> CameraParams {
    CameraParams {
        distance: PIVOT,
        fog: 0.0,
        solid: 0.0,
        ..bound
    }
}

/// The torus's half-extents at view depth `z` for a render target of `aspect`:
/// the rest frustum's cross-section at that depth, times [`MARGIN`].
///
/// The visible frame at rest is exactly `[MARGIN; 2]` smaller on each axis, so
/// the seam sits outside it by the same share at every depth.
fn half_extent(z: f32, aspect: f32) -> (f32, f32) {
    let h = MARGIN * z * (0.5 * REST_FOV).tan();
    (h * aspect, h)
}

/// The largest `|yaw|` and `|pitch|`, in radians, that keep the wrap seam
/// outside the frame under a bound `fov`, `zoom`, target `aspect` and `pan`.
///
/// At rest the seam projects to normalized-device `MARGIN * tan(REST_FOV / 2)
/// / tan(fov / 2)` on both axes; what is left past the frame edge and the pan
/// is that axis's headroom `h`. Turning the eye by `θ` about [`PIVOT`] moves a
/// seam point at depth `z` along the turn's own axis by `θ * (1 + s^2 - PIVOT /
/// z)` in tangent units, to first order, where `s` is the seam's rest tangent
/// on that axis — the turn itself, its foreshortening, and the eye's own
/// displacement. Alone, an axis may turn by `h` over the worst of that drift
/// across the slab.
///
/// The two turns also couple: a yaw `θ` pushes one side of the frame deeper,
/// which divides the other axis's tangents by up to `1 + θ * s_x` at the seam's
/// corner, and a pitch likewise. So both alone-bounds are scaled by one shared
/// `c`, the largest for which `seam / (1 + c * a) >= 1 + pan + c * h` holds on
/// both axes, with `a` the other turn's coupling at its alone-bound; and then by
/// [`SWAY_SHARE`] for the terms this first-order model leaves out. Zero where
/// either axis has no headroom, so a pan or a `fov` that already shows the seam
/// is not made worse by a sway.
fn sway_bound(fov: f32, zoom: f32, aspect: f32, pan: [f32; 2]) -> [f32; 2] {
    let finite = |v: f32, fallback: f32| if v.is_finite() { v } else { fallback };
    let zoom = finite(zoom, 1.0).max(1e-3);
    let fov = (finite(fov, REST_FOV) / zoom).clamp(camera::MIN_FOV, camera::MAX_FOV);
    let aspect = finite(aspect, 1.0).max(0.1);
    let rest = (0.5 * REST_FOV).tan();
    let open = (0.5 * fov).tan();
    let seam = MARGIN * rest / open;
    // Each axis's pan in normalized-device units, the frame edge's tangent and
    // the seam's rest tangent.
    let (pan_x, pan_y) = (
        finite(pan[0], 0.0).abs() / aspect,
        finite(pan[1], 0.0).abs(),
    );
    let (frame_x, frame_y) = (open * aspect, open);
    let (slope_x, slope_y) = (MARGIN * rest * aspect, MARGIN * rest);
    let (h_x, h_y) = (seam - 1.0 - pan_x, seam - 1.0 - pan_y);
    if h_x <= 0.0 || h_y <= 0.0 {
        return [0.0, 0.0];
    }
    let drift = |slope: f32| {
        [Z_NEAR, Z_FAR]
            .into_iter()
            .map(|z| (1.0 + slope * slope - PIVOT / z).abs())
            .fold(f32::EPSILON, f32::max)
    };
    let (alone_yaw, alone_pitch) = (
        h_x * frame_x / drift(slope_x),
        h_y * frame_y / drift(slope_y),
    );
    // The largest `c` in `[0, 1]` with `c^2 h a + c (h + a (1 + pan)) - h <= 0`:
    // the coupled condition above, rearranged.
    let share = |h: f32, a: f32, pan: f32| {
        if a <= f32::EPSILON {
            return 1.0;
        }
        let b = h + a * (1.0 + pan);
        ((-b + (b * b + 4.0 * h * h * a).sqrt()) / (2.0 * h * a)).clamp(0.0, 1.0)
    };
    let c = share(h_x, alone_pitch * slope_y, pan_x).min(share(h_y, alone_yaw * slope_x, pan_y));
    [SWAY_SHARE * c * alone_yaw, SWAY_SHARE * c * alone_pitch]
}

/// The light a particle keeps at view depth `z` under `depth_fade`:
/// [`DEPTH_LIGHT_NEAR`] at the near slab bound, falling linearly by the
/// `depth_fade` share of it to the far bound. Clamped at zero, since a bound
/// fade past 1 would otherwise subtract light.
fn depth_light(z: f32, depth_fade: f32) -> f32 {
    let across = ((z - Z_NEAR) / Z_SPAN).clamp(0.0, 1.0);
    (DEPTH_LIGHT_NEAR * (1.0 - depth_fade * across)).max(0.0)
}

/// How much of its light a particle at view depth `z` shows as it nears a slab
/// bound: 1 in the slab's interior, easing to 0 across [`FADE_BAND`] of the
/// slab at either bound, so a particle is dark at the moment it wraps in depth.
fn slab_fade(z: f32) -> f32 {
    let band = FADE_BAND * Z_SPAN;
    let e = ((z - Z_NEAR).min(Z_FAR - z) / band).clamp(0.0, 1.0);
    e * e * (3.0 - 2.0 * e)
}

/// The LUT sample coordinate for one particle (ADR-0021): its per-particle hue
/// occupies the band `hue_center + (particle_hue - 0.5) * hue_spread`, plus the
/// shared `hue` rotation. Defaults (`center = 0.5`, `spread = 1`, `hue = 0`)
/// reduce to `particle_hue`, the full-wheel look.
fn hue_coord(hue_center: f32, hue_spread: f32, particle_hue: f32, hue: f32) -> f32 {
    hue_center + (particle_hue - 0.5) * hue_spread + hue
}

/// The individuation contract, mirrored from the emitter (`emitter.rs`'s
/// `unit`, which is private to that scene): a per-particle quantity is a pure
/// function of `(seed, channel)` — splitmix64's finalizer applied as a hash.
/// The swarm's `seed` is the particle's index in the seeded pool, which is
/// stable for the scene's life, and the hash runs at construction only.
fn unit(seed: u32, k: u32) -> f32 {
    let mut z = ((seed as u64) << 32 | k as u64).wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z >> 40) as f32 / (1u64 << 24) as f32
}

/// Seed channels, named so a later quantity cannot silently reuse one and
/// correlate itself with an existing draw (the emitter's convention).
mod channel {
    pub(super) const TWINKLE_FREQ: u32 = 0;
    pub(super) const TWINKLE_PHASE: u32 = 1;
    pub(super) const SIZE: u32 = 2;
    pub(super) const RESEED_X: u32 = 3;
    pub(super) const RESEED_Y: u32 = 4;
}

/// The particle's brightness multiplier under `twinkle` — the emitter's
/// semantics on the emitter's frequency band, over the pre-resolved
/// per-particle rate and phase. Exactly `1.0` at `twinkle <= 0`, which is what
/// makes the defaults' byte-identity falsifiable in both directions; clamped
/// at zero because `twinkle` is a preset expression and may exceed 1, and a
/// negative multiplier would subtract light rather than removing it.
fn twinkle_factor(freq: f32, phase: f32, time: f32, twinkle: f32) -> f32 {
    if twinkle <= 0.0 {
        return 1.0;
    }
    let wave = (std::f32::consts::TAU * (freq * time + phase)).sin();
    (1.0 + twinkle * wave).max(0.0)
}

/// The particle's size multiplier within `size_spread` — the emitter's
/// `size_factor`, over the pre-resolved unit draw. Exactly `1.0` at zero
/// spread (the default), on top of the scatter's own seeded base size.
fn size_factor(size_unit: f32, size_spread: f32) -> f32 {
    (1.0 + (size_unit - 0.5) * size_spread).max(0.0)
}

/// Parameter vocabulary — see [`fragment_field::PARAMS`](super::fragment_field::PARAMS).
/// **Keep in sync with `set_param` below.**
pub const PARAMS: &[ParamSpec] = &[
    ParamSpec {
        name: "force",
        default: 1.4,
        range: Some([0.0, 4.0]),
        doc: "How hard the flow field pushes each particle, so higher is faster and straighter.",
        kind: ParamKind::Modal,
        group: ParamGroup::Motion,
        main: true,
    },
    ParamSpec {
        name: "spin",
        default: 0.3,
        range: Some([-2.0, 2.0]),
        doc: "Rotational bias added to the flow, curling the paths into vortices.",
        kind: ParamKind::Modal,
        group: ParamGroup::Motion,
        main: true,
    },
    ParamSpec {
        name: "burst",
        default: 0.0,
        range: Some([0.0, 2.0]),
        doc: "An outward impulse from the centre, for a beat to throw the swarm apart.",
        kind: ParamKind::Modal,
        group: ParamGroup::Motion,
        main: false,
    },
    crate::render::scenes::common::hue(DEFAULT_HUE),
    crate::render::scenes::common::brightness(DEFAULT_BRIGHTNESS),
    ParamSpec {
        name: "size",
        default: 1.0,
        range: Some([0.0, 4.0]),
        doc: "Size of each particle's mark at the middle of the swarm's depth; nearer marks draw larger and farther ones smaller.",
        kind: ParamKind::Modal,
        group: ParamGroup::Shape,
        main: true,
    },
    ParamSpec {
        name: "field_freq",
        default: 2.3,
        range: Some([0.5, 8.0]),
        doc: "Spatial frequency of the flow field; higher makes smaller, busier eddies.",
        kind: ParamKind::Modal,
        group: ParamGroup::Shape,
        main: false,
    },
    crate::render::scenes::common::zoom(DEFAULT_ZOOM),
    crate::render::scenes::common::PAN_X,
    crate::render::scenes::common::PAN_Y,
    ParamSpec {
        name: "hue_spread",
        default: 1.0,
        range: Some([0.0, 1.0]),
        doc: "How far across the palette the particle band reaches.",
        kind: ParamKind::Modal,
        group: ParamGroup::Colour,
        main: false,
    },
    ParamSpec {
        name: "hue_center",
        default: 0.5,
        range: Some([0.0, 1.0]),
        doc: "Where that band sits along the palette.",
        kind: ParamKind::Modal,
        group: ParamGroup::Colour,
        main: false,
    },
    crate::render::scenes::common::SATURATION,
    crate::render::scenes::common::PALETTE_MIX,
    crate::render::scenes::common::PALETTE_STEPS,
    crate::render::scenes::common::PALETTE_CONTOUR,
    ParamSpec {
        name: "twinkle",
        default: 0.0,
        range: Some([0.0, 1.0]),
        doc: "Per-particle brightness flicker, seeded so it is reproducible.",
        kind: ParamKind::Modal,
        group: ParamGroup::Light,
        main: false,
    },
    ParamSpec {
        name: "size_spread",
        default: 0.0,
        range: Some([0.0, 1.0]),
        doc: "How much particle sizes vary about `size`; 0 makes them uniform.",
        kind: ParamKind::Modal,
        group: ParamGroup::Shape,
        main: false,
    },
    ParamSpec {
        name: "reseed",
        default: 0.0,
        range: Some([0.0, 1.0]),
        doc: "Crossing zero throws every particle back to a fresh start position.",
        kind: ParamKind::Modal,
        group: ParamGroup::Motion,
        main: false,
    },
    ParamSpec {
        name: "depth_fade",
        default: 1.0 - 0.45 / DEPTH_LIGHT_NEAR,
        range: Some([0.0, 1.0]),
        doc: "How much light a particle loses from the front of the swarm to the back; 0 lights every depth alike.",
        kind: ParamKind::Modal,
        group: ParamGroup::Light,
        main: false,
    },
    ParamSpec {
        name: "yaw",
        default: 0.0,
        range: Some([-0.2, 0.2]),
        doc: "Turns the camera sideways within the swarm, in radians; held to the sway the margin past the frame covers, so bind it to a slow wave.",
        ..camera::YAW
    },
    ParamSpec {
        name: "pitch",
        default: 0.0,
        range: Some([-0.2, 0.2]),
        doc: "Tilts the camera up or down within the swarm, in radians; held to the sway the margin past the frame covers.",
        ..camera::PITCH
    },
    ParamSpec {
        range: Some([0.2, 0.9]),
        doc: "The camera's vertical field of view in radians; zoom divides it. Wider than the resting 0.8 uses up the margin past the frame, and much wider shows the wrap seam.",
        ..camera::FOV
    },
    camera::FOCUS,
    camera::APERTURE,
    crate::render::scenes::marks::SHAPE,
    crate::render::scenes::marks::POINTS,
    crate::render::scenes::marks::STAR_VALLEY,
    crate::render::scenes::marks::STAR_CURVE,
    crate::render::scenes::marks::STAR_JITTER,
    crate::render::scenes::marks::STAR_SEED,
    crate::render::scenes::marks::STAR_WOBBLE,
    crate::render::scenes::marks::STAR_WOBBLE_FREQ,
];

impl Scene for SwarmScene {
    fn name(&self) -> &'static str {
        "swarm"
    }

    fn advance(&mut self, dt: f32) {
        self.dt = dt;
    }

    fn set_time(&mut self, time: f32) {
        self.time = time;
    }

    fn set_target_size(&mut self, width: u32, height: u32) {
        self.target = (width, height);
    }

    fn mirror_overflow(&self) -> Option<&super::CapOverflow> {
        self.clamp.as_ref()
    }

    fn set_palette(&mut self, palette: &Palette) {
        // CPU-sampled per particle in `update`; a cheap array copy, off the hot
        // path (once per preset switch).
        self.palette = palette.clone();
    }

    fn reset_params(&mut self) {
        self.force = DEFAULT_FORCE;
        self.spin = DEFAULT_SPIN;
        self.burst = DEFAULT_BURST;
        self.colour.reset();
        self.pan.reset();
        self.camera = rest_camera();
        self.size = DEFAULT_SIZE;
        self.field_freq = DEFAULT_FIELD_FREQ;
        self.zoom = DEFAULT_ZOOM;
        self.depth_fade = DEFAULT_DEPTH_FADE;
        self.hue_spread = DEFAULT_HUE_SPREAD;
        self.hue_center = DEFAULT_HUE_CENTER;
        self.shape = DEFAULT_SHAPE;
        self.points = DEFAULT_POINTS;
        self.star_valley = DEFAULT_STAR_VALLEY;
        self.star_curve = DEFAULT_STAR_CURVE;
        self.star_jitter = DEFAULT_STAR_JITTER;
        self.star_seed = DEFAULT_STAR_SEED;
        self.star_wobble = DEFAULT_STAR_WOBBLE;
        self.star_wobble_freq = DEFAULT_STAR_WOBBLE_FREQ;
        self.twinkle = DEFAULT_TWINKLE;
        self.size_spread = DEFAULT_SIZE_SPREAD;
        // `prev_reseed` is deliberately NOT reset: this runs every frame
        // before the bindings are routed, and resetting the previous level
        // would turn a held gate into an edge per frame — a continuous
        // disturbance in place of a percussive one. The attractor's
        // reset_params makes the same omission for the same reason.
        self.reseed = 0.0;
    }

    fn set_param(&mut self, name: &str, value: f32) {
        // The shared param blocks first, this scene's own names after
        // (`scenes::common`). The camera is a subset of the shared block, so
        // its five names are matched here rather than delegated: the block's
        // `distance`, `fog` and `solid` are not this scene's.
        if self.colour.set(name, value) || self.pan.set(name, value) {
            return;
        }
        match name {
            "yaw" => self.camera.yaw = value,
            "pitch" => self.camera.pitch = value,
            "fov" => self.camera.fov = value,
            "focus" => self.camera.focus = value,
            "aperture" => self.camera.aperture = value,
            "force" => self.force = value,
            "spin" => self.spin = value,
            "burst" => self.burst = value,
            "size" => self.size = value,
            "field_freq" => self.field_freq = value,
            "zoom" => self.zoom = value,
            "depth_fade" => self.depth_fade = value,
            "hue_spread" => self.hue_spread = value,
            "hue_center" => self.hue_center = value,
            "shape" => self.shape = value,
            "points" => self.points = value,
            "star_valley" => self.star_valley = value,
            "star_curve" => self.star_curve = value,
            "star_jitter" => self.star_jitter = value,
            "star_seed" => self.star_seed = value,
            "star_wobble" => self.star_wobble = value,
            "star_wobble_freq" => self.star_wobble_freq = value,
            "twinkle" => self.twinkle = value,
            "size_spread" => self.size_spread = value,
            "reseed" => self.reseed = value,
            _ => {}
        }
    }

    #[allow(
        clippy::indexing_slicing,
        reason = "pos/vel/base index fixed-size arrays at constant offsets, always in-bounds"
    )]
    fn update(&mut self, _frame: &AnalysisFrame) {
        // Rising-edge detect on `reseed` (Plan 0077 Phase 3): **disturb** the
        // existing population where it is, by a seeded, domain-relative kick —
        // ADR-0066's semantics, not the box respawn it removed. The kick is a
        // pure function of (particle index, reseed ordinal), so a capture
        // remains reproducible; unbound, `reseed` and `prev_reseed` sit at 0
        // and this path never touches a position.
        if self.reseed >= RESEED_THRESHOLD && self.prev_reseed < RESEED_THRESHOLD {
            self.reseed_count = self.reseed_count.wrapping_add(1);
            let salt = self.reseed_count.wrapping_mul(0x9E37_79B9);
            for (i, p) in self.particles.iter_mut().enumerate() {
                let seed = (i as u32).wrapping_add(salt);
                p.pos[0] += (unit(seed, channel::RESEED_X) - 0.5) * 2.0 * RESEED_KICK;
                p.pos[1] += (unit(seed, channel::RESEED_Y) - 0.5) * 2.0 * RESEED_KICK;
                // No wrap here: a kick of ±RESEED_KICK cannot overshoot the ±1
                // seam by more than itself, and the integration loop below
                // wraps every position this same frame.
            }
        }
        self.prev_reseed = self.reseed;

        // Field evolves at `spin`; `force` steers, `burst` shoves outward. The
        // clock integrates here, after this frame's `set_param` calls have
        // landed, so it advances at *this* frame's rate.
        self.field_phase.step(self.spin, self.dt);
        let field_t = self.field_phase.get();
        let force = self.force;
        let burst_kick = self.burst;
        let field_freq = self.field_freq;
        // The individuation pair, resolved at draw like the emitter's: an
        // eased width moves the whole population continuously instead of only
        // particles spawned since the change (Plan 0077 Phase 2).
        let twinkle = self.twinkle;
        let size_spread = self.size_spread;
        let depth_fade = self.depth_fade;
        let time = self.time;

        // Frame-rate-independent integration (Plan 0014 Phase 2): scale the
        // acceleration/advection by real `dt`, and raise the per-frame damping to
        // the `dt`-relative power so the velocity decays at the same wall-clock
        // rate regardless of refresh (one `powf` per frame, not per particle).
        let dt = self.dt;
        let damp = DAMPING.powf(dt * 60.0);

        // The cross-section follows the render target. Its half-extents are
        // linear in depth, so the per-depth scale is one multiply per particle
        // off these two per-frame constants.
        let (unit_x, unit_y) = half_extent(1.0, self.aspect);

        for (p, inst) in self.particles.iter_mut().zip(self.instance_data.iter_mut()) {
            let (hx, hy) = (unit_x * p.z, unit_y * p.z);
            // Frustum coordinates -> world, which is what the field, the burst
            // and the sprite all work in.
            let world = [p.pos[0] * hx, p.pos[1] * hy];

            // Scalar potential -> flow direction (cheap curl-ish field), with
            // depth in its phase so each layer rides its own currents; the two
            // axes take different offsets, so layers decorrelate in both.
            let zo = p.z * DEPTH_FIELD_FREQ;
            let a = (world[0] * field_freq + field_t + zo).sin()
                + (world[1] * field_freq - field_t * 0.8 - zo * 0.7).cos();
            let dir = [a.cos(), a.sin()];
            // The slow depth current, off the same field at half its
            // frequency, so neighbours on a streamline drift in depth together.
            let lift = ((world[0] - world[1]) * field_freq * 0.5 + field_t * 0.6 + zo * 1.3).sin();

            p.vel[0] = p.vel[0] * damp + dir[0] * force * dt;
            p.vel[1] = p.vel[1] * damp + dir[1] * force * dt;
            p.vel[2] = p.vel[2] * damp + lift * force * Z_FLOW * dt;

            // Beat burst pushes particles radially outward from the view axis.
            if burst_kick > 0.0 {
                let r = (world[0] * world[0] + world[1] * world[1]).sqrt().max(1e-3);
                p.vel[0] += world[0] / r * burst_kick * dt;
                p.vel[1] += world[1] / r * burst_kick * dt;
            }

            // A world velocity becomes a frustum-coordinate one through the
            // half-extent at this depth: the same current moves a far particle
            // across less of the frame.
            p.pos[0] += p.vel[0] * dt / hx;
            p.pos[1] += p.vel[1] * dt / hy;
            p.z += p.vel[2] * dt;

            // Toroidal wrap keeps the field populated (no respawns/hitches). The
            // seam is at ±1 whatever the target and the depth, `MARGIN` past
            // the visible frame (ADR-0044, ADR-0259).
            if p.pos[0] > 1.0 {
                p.pos[0] -= 2.0;
            } else if p.pos[0] < -1.0 {
                p.pos[0] += 2.0;
            }
            if p.pos[1] > 1.0 {
                p.pos[1] -= 2.0;
            } else if p.pos[1] < -1.0 {
                p.pos[1] += 2.0;
            }
            // The depth wrap jumps a particle across the slab; `slab_fade` has
            // it dark on both sides of the jump.
            if p.z > Z_FAR {
                p.z -= Z_SPAN;
            } else if p.z < Z_NEAR {
                p.z += Z_SPAN;
            }

            let speed = (p.vel[0] * p.vel[0] + p.vel[1] * p.vel[1]).sqrt();
            // Colour through the shared LUT (ADR-0021): the per-particle hue is
            // mapped into the `hue_spread`/`hue_center` band, then desaturated by
            // the shared `saturation`.
            let coord = hue_coord(self.hue_center, self.hue_spread, p.hue, self.colour.hue);
            // Hard bands on the palette coordinate (ADR-0078), the canonical
            // `palette::band_coord` called rather than copied. `palette_steps <= 1`
            // returns it untouched.
            let base = palette::desaturate(
                self.palette.sample(
                    palette::band_coord(coord, self.colour.steps),
                    self.colour.mix,
                ),
                self.colour.saturation,
            );

            // The speed cue: on a coherent field the fast channels read brighter
            // than slack water. Depth multiplies it rather than replacing it.
            // The twinkle factor is exactly 1.0 when `twinkle` is unbound, and
            // the size factor exactly 1.0 at zero spread, so multiplying by
            // either is bit-exact.
            let presence = slab_fade(p.z);
            let bright = ((0.25 + speed * 0.7) * p.bright).min(1.6)
                * self.colour.brightness
                * depth_light(p.z, depth_fade)
                * presence
                * twinkle_factor(p.twinkle_freq, p.twinkle_phase, time, twinkle);

            let (hx, hy) = (unit_x * p.z, unit_y * p.z);
            *inst = SwarmInstance {
                // The eye sits at `+PIVOT` on world z looking down `-z`, so a
                // view depth `z` at rest is world `PIVOT - z`.
                center: [p.pos[0] * hx, p.pos[1] * hy, PIVOT - p.z],
                radius: p.size * self.size * size_factor(p.size_unit, size_spread),
                color: [base[0] * bright, base[1] * bright, base[2] * bright],
                presence,
            };
        }
    }

    fn render(
        &mut self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        aspect: f32,
    ) {
        // The domain the *next* `update` wraps against, and this frame's
        // projection. This argument is the render target's aspect — the only
        // correct source for a shape (ADR-0037); see the field's docs for why
        // `set_target_size` is not.
        self.aspect = aspect.max(0.1);
        let frame = self.camera_frame(self.aspect, self.max_coc);
        self.clamp = frame.blur;
        let misc = SwarmUniform {
            v: [self.aspect, SIZE_DEPTH, 0.0, 0.0],
            // Quantized here, on the way into the uniform, so the shader's
            // precondition stays visible on the CPU side: the roster's
            // bounds and the integer point count live in `marks`, and no
            // fractional value ever reaches an angular fold (ADR-0084).
            m: [
                marks::mark_shape(self.shape),
                marks::mark_points(self.points),
                marks::star_seed(self.star_seed),
                marks::star_wobble(self.star_wobble),
            ],
            s: [
                marks::star_valley(self.star_valley),
                marks::star_curve(self.star_curve),
                marks::star_jitter(self.star_jitter),
                marks::star_wobble_freq(self.star_wobble_freq),
            ],
        };

        // Load over the engine backdrop (ADR-0018): the additive particles
        // bloom over whatever the background pass painted, so the sparse gaps
        // between them reveal it.
        self.sprites.draw(
            queue,
            encoder,
            view,
            &misc,
            &frame.uniform,
            &self.instance_data,
        );
    }
}

#[cfg(test)]
mod tests;
