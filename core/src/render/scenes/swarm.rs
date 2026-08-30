//! Particle-swarm scene: ~10k CPU-simulated particles drifting through a flow
//! field, drawn as instanced additive sprites (the starfield's approach,
//! scaled up). One of the two preset-driven systems (ADR-0002 layers 1-2).
//!
//! Its behavior is a set of named parameters — `force`, `spin`, `burst`, `hue`,
//! `brightness`, `size` — that a preset binds to expressions over the audio
//! analysis (Plan 0003 Phase 5). All per-particle math is CPU-side; no compute
//! shader. Motion is deterministic; the only randomness is the seeded initial
//! scatter (NFR 6).

// Hot-path panic-denial pragma (Plan 0002 Phase 2, extended to scenes by Plan
// 0003 Phase 0). Runs every displayed frame.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

use super::marks;
use super::{FALLBACK_DT, Phase, Scene, SeededRng};
use crate::dsp::AnalysisFrame;
use crate::render::gpu;
use crate::render::palette::{self, Palette};

const SEED: u64 = 0x4C4D_565F_5357_524D; // "LMV_SWRM"

/// How far the toroidal domain extends past the visible frame (Plan 0043 Phase 1,
/// ADR-0044).
///
/// Half-extents of `BOUND_X = 1.8` / `BOUND_Y = 1.0` put the wrap seam
/// on the NDC frame edge, which `1.0` **is**. The wrap is toroidal, so
/// that line is the one place on screen every wrapping particle is
/// guaranteed to paint, and the feedback stage integrates it into a
/// saturated bar across the top and bottom of every swarm preset within
/// a few hundred frames.
///
/// The bounds now follow the render target (below) and carry this margin so the
/// seam sits *outside* the frame. Chosen by measurement, not by rounding: the
/// family works at `zoom` 1.0–1.3 with `pan_*` to about 0.16, and a particle at
/// world `y = BOUND_Y` lands on the frame edge when `BOUND_Y * zoom - |pan_y| ==
/// 1`. At the worst case in that range (`zoom = 1.0`) the seam clears the frame
/// for `|pan| <= MARGIN - 1`, so 1.25 buys 0.25 of pan headroom on both axes —
/// comfortably past what the family uses, and it also puts the *domain rectangle*
/// off-screen down to `zoom = 0.8`, which is the inset-edge wall that pinned the
/// family at or above 1.0.
///
/// The cost is visible density: the visible fraction of the domain is `1 /
/// MARGIN^2`, so a quarter of the 10 000 particles are off-screen at any moment.
/// That is the tradeoff Phase 4's re-authoring absorbs.
const MARGIN: f32 = 1.25;
/// Domain aspect before the first [`Scene::render`] hands one over. Only reached
/// on the very first `update` of a fresh scene; because positions are stored
/// normalized (see [`Particle::pos`]) an aspect change rescales the field rather
/// than teleporting it, so this fallback is continuous with whatever follows.
const FALLBACK_ASPECT: f32 = 16.0 / 9.0;

/// Velocity retained per frame (the rest is re-steered by the flow field).
const DAMPING: f32 = 0.86;

// --- The depth axis (Plan 0043 Phase 3, ADR-0044) -------------------------------
//
// Each particle carries a `z` in `0..1` — 0 far, 1 near — seeded with the rest of
// the scatter. It drives four things and **never** a sort: the scene blends
// additively, and addition is commutative, so draw order is irrelevant. That one
// fact is what makes a depth axis nearly free here; the per-frame sort a 3D
// particle system normally pays buys occlusion an additive scene does not have.
//
// It is an honest fake. There is no occlusion and no perspective divide — two
// particles at different depths that overlap simply sum — so the illusion flattens
// as density rises. That is the known limit of the 2.5D choice, not a defect.
/// Sprite scale at `z = 0` and `z = 1`. The mean is ~1, so the family's `size`
/// bindings keep roughly their old meaning.
const DEPTH_SCALE_FAR: f32 = 0.55;
const DEPTH_SCALE_NEAR: f32 = 1.50;
/// Atmospheric fade: brightness multiplier at `z = 0` and `z = 1`. Distance
/// washing out contrast is the oldest depth cue there is, and it is what keeps a
/// far particle from reading as merely a small near one.
const DEPTH_FADE_FAR: f32 = 0.45;
const DEPTH_FADE_NEAR: f32 = 1.05;
/// Parallax strength against the shared view transform at `z = 0` and `z = 1`.
///
/// A near particle traverses the frame ~1.9x faster than a far one under the same
/// `pan_*`, which is the difference between a depth axis and a sprite sheet at two
/// scales. Both ends are deliberately kept near 1 rather than spread wide: the
/// near layer is the binding case for the [`MARGIN`] seam clearance (it is the one
/// pan pushes furthest toward the frame), and at `zoom = 1` with the family's
/// `pan` of 0.16 this still leaves the seam off-screen.
const DEPTH_PARALLAX_FAR: f32 = 0.65;
const DEPTH_PARALLAX_NEAR: f32 = 1.25;
/// Phase offset, in radians, applied to the flow-field sample per unit of `z`.
///
/// **This is the term that makes it read as volume.** Without it every depth layer
/// rides identical streamlines and the result is one flock drawn at several sizes;
/// with it the near and far layers follow genuinely different currents, so they
/// cross and separate the way real depth does. Sized as a large fraction of the
/// field's `TAU` period — enough to decorrelate the layers, short of wrapping them
/// back onto each other.
const DEPTH_FIELD_OFFSET: f32 = 2.6;

/// Parameter defaults — a calm idle drift when nothing is bound.
const DEFAULT_FORCE: f32 = 1.4;
const DEFAULT_SPIN: f32 = 0.3;
const DEFAULT_BURST: f32 = 0.0;
const DEFAULT_HUE: f32 = 0.0;
const DEFAULT_BRIGHTNESS: f32 = 0.8;
const DEFAULT_SIZE: f32 = 1.0;
/// Spatial frequency of the flow field — how many vortices fit across the world,
/// and so how many distinct streams a frame can hold (Plan 0043 Phase 2).
///
/// Was a bare `const FIELD_FREQ`; it is now the bindable `field_freq`, and this
/// default is **exactly** the constant it replaced, so a preset that does not bind
/// it renders unchanged.
///
/// It is this scene's first structural lever. Low values give a few broad
/// currents that many particles share — which is where the family's apparent
/// flocking comes from, since neighbours on one streamline travel together — and
/// high values give many tight swirls. `spin` says how fast the field is rewritten;
/// this says how finely it is divided.
const DEFAULT_FIELD_FREQ: f32 = 2.3;
// Per-mark individuation (Plan 0077 Phase 2, backlog 0068). Both default OFF —
// unlike the emitter's spreads, which default non-zero, the swarm's scatter
// already ships a seeded per-particle size and brightness, so these *widen*
// what is there and their defaults must leave every shipped capture
// byte-identical.
const DEFAULT_TWINKLE: f32 = 0.0;
const DEFAULT_SIZE_SPREAD: f32 = 0.0;
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
/// Fraction of the domain's normalized half-extent one `reseed` kick spans per
/// axis (Plan 0077 Phase 3, ADR-0066 semantics): the kick disturbs the
/// population **where it is**, sized from the swarm's own domain the way
/// `AttractorFamily::jitter_extent` derives from `seed_box` — *not* a respawn
/// into a uniform box, which is the artifact class ADR-0066 removed and
/// backlog 0064 caught returning once already. Positions are normalized, so a
/// fraction here is domain-relative on any target and any aspect.
///
/// The value is the attractor's measured `JITTER_FRACTION`, adopted as the
/// starting magnitude for the same figure-relative kick. ADR-0066 records the
/// magnitude as the lever if the disturbance reads too subtle; returning to a
/// box re-fill is not.
const RESEED_KICK: f32 = 0.06;
// Shared palette color knobs (ADR-0021). Each particle's hue occupies the band
// `hue_center + (particle_hue - 0.5) * hue_spread`; the defaults (`center = 0.5`,
// `spread = 1`) reproduce the prior full-wheel look (`particle_hue`), and
// `saturation = 1` leaves color untouched — so an unbound swarm is unchanged.
const DEFAULT_HUE_SPREAD: f32 = 1.0;
const DEFAULT_HUE_CENTER: f32 = 0.5;
const DEFAULT_SATURATION: f32 = 1.0;
/// `palette_mix` default — 0 = palette A only (the crossfade is a no-op unless a
/// preset declares `[palette_b]` and binds `palette_mix`).
const DEFAULT_PALETTE_MIX: f32 = 0.0;
// Shared view transform (ADR-0018): identity by default, so an unbound preset is
// unchanged. `zoom` multiplies particle positions about the frame centre; `pan_*`
// offset them — matching the line scenes' semantics (zoom > 1 = zoomed in).
const DEFAULT_ZOOM: f32 = 1.0;
const DEFAULT_PAN: f32 = 0.0;
// The mark silhouette (ADR-0084). `disc` is exactly the arithmetic the sprite
// drew before the roster existed, so an unbound swarm is unchanged.
const DEFAULT_SHAPE: f32 = marks::DEFAULT_SHAPE;
const DEFAULT_POINTS: f32 = marks::DEFAULT_POINTS;
/// The `star` arm's three shape params (Plan 0091 Phase 5), aliased beside the
/// other two mark defaults so this scene states its whole vocabulary locally.
const DEFAULT_STAR_VALLEY: f32 = marks::DEFAULT_STAR_VALLEY;
const DEFAULT_STAR_CURVE: f32 = marks::DEFAULT_STAR_CURVE;
const DEFAULT_STAR_JITTER: f32 = marks::DEFAULT_STAR_JITTER;

/// The scene's own WGSL. The shared mark-silhouette chunk
/// ([`marks::sdf_wgsl`]) is prepended at module creation, so `mark_distance` here
/// is the same function the emitter evaluates.
///
/// **`shape` and `points` travel vertex -> fragment as flat varyings rather than
/// being read from `misc` in the fragment stage**, and that is deliberate. The
/// fragment stage cannot see this scene's uniform without widening the bind
/// layout's visibility to `VERTEX_FRAGMENT` — which would make this descriptor
/// byte-identical to the line renderer's (`{uniform, VERTEX_FRAGMENT,
/// min_binding_size: None}`), the exact collision shape ADR-0058 records and the
/// one the emitter's layout comment says not to tidy back in. A flat varying
/// carries a per-draw value with no descriptor change at all.
const SHADER: &str = r#"
struct Misc {
    // x: aspect, y: zoom, zw: pan (the shared ViewTransform, ADR-0018)
    v: vec4<f32>,
    // x: mark shape index, y: quantized point count (ADR-0084). Per draw, not
    // per instance: the branch stays uniform across a warp and `Instance` does
    // not grow.
    m: vec4<f32>,
    // xyz: the star arm's shape params (valley, curve, jitter), conditioned
    // CPU-side (Plan 0091 Phase 5). Per draw, like `m`. Inert on every other
    // shape, and at their defaults the arm takes its original closed form.
    s: vec4<f32>,
}

@group(0) @binding(0) var<uniform> misc: Misc;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) local: vec2<f32>,
    @location(1) color: vec3<f32>,
    @location(2) @interpolate(flat) shape: f32,
    @location(3) @interpolate(flat) points: f32,
    @location(4) @interpolate(flat) star: vec3<f32>,
}

@vertex
fn vs_main(
    @builtin(vertex_index) vi: u32,
    @location(0) center: vec2<f32>,
    @location(1) size: f32,
    @location(2) color: vec3<f32>,
    @location(3) parallax: f32,
) -> VsOut {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(0.0, 1.0),
        vec2<f32>(0.0, 1.0), vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0),
    );
    let c = corners[vi] * 2.0 - vec2<f32>(1.0, 1.0);
    // Shared ViewTransform (ADR-0018): zoom about the frame centre, then pan the
    // particle position; the sprite quad (c * size) keeps its on-screen size.
    //
    // Depth parallax (Plan 0043 Phase 3): `parallax` is the per-particle strength
    // the CPU derived from `z`, so a near particle takes more of the pan and more
    // of the zoom deflection than a far one and the layers slide across each other
    // as the camera moves. At the identity transform (zoom 1, pan 0) this reduces
    // to `center` for every depth, so an unbound preset is untouched.
    let zoom = misc.v.y;
    let pan = misc.v.zw;
    let center_v = center * (1.0 + (zoom - 1.0) * parallax) + pan * parallax;
    let world = center_v + c * size;
    var out: VsOut;
    out.pos = vec4<f32>(world.x / misc.v.x, world.y, 0.0, 1.0);
    out.local = c;
    out.color = color;
    out.shape = misc.m.x;
    out.points = misc.m.y;
    out.star = misc.s.xyz;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    // The silhouette (ADR-0084). At the default `disc` this is `length(in.local)`
    // and nothing else, so an unshaped swarm is the arithmetic it always was; the
    // falloff below is untouched either way, so a visual change is attributable
    // to the shape alone.
    let d = mark_distance(in.local, in.shape, in.points, in.star);
    let falloff = max(0.0, 1.0 - d);
    let g = falloff * falloff;
    // Premultiplied: colour AND alpha carry the same coverage `g`, so the four
    // corners outside the inscribed disc write nothing at all rather than
    // opaque black (ADR-0056). See `gpu::ADDITIVE_LIGHT_SATURATING_COVERAGE`.
    return vec4<f32>(in.color * g, g);
}
"#;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Instance {
    center: [f32; 2],
    size: f32,
    color: [f32; 3],
    /// The particle's depth parallax strength, resolved from its `z` on the CPU so
    /// the shader needs no depth constants (Plan 0043 Phase 3).
    parallax: f32,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Misc {
    v: [f32; 4],
    /// `[shape, points, 0, 0]` — the mark silhouette, quantized on the way in
    /// (ADR-0084). Padded to a second `vec4` because that is WGSL's uniform
    /// layout rule, not because three slots are wanted.
    m: [f32; 4],
    /// `[star_valley, star_curve, star_jitter, 0]` — the star arm's three shape
    /// params, conditioned on the way in (Plan 0091 Phase 5). A third `vec4`
    /// rather than a wider `m` because WGSL's uniform layout rule packs in
    /// 16-byte rows; the layout SHAPE is unchanged, so ADR-0058 is untouched.
    s: [f32; 4],
}

struct Particle {
    /// Position on the torus in **normalized** domain coordinates, each axis in
    /// `[-1, 1)`; world position is this times the current half-extents (Plan 0043
    /// Phase 1).
    ///
    /// Normalized rather than world-space for one reason: the half-extents now
    /// follow the render target, so they change on a resize, and a world-space
    /// store would have to either re-wrap (teleporting every particle that fell
    /// outside the new domain, all in one frame) or rescale every position by
    /// hand. Here the resize *is* the rescale — each particle keeps its place on
    /// the torus and the field stretches with the frame, which is the
    /// discontinuity-free resize ADR-0044 requires. It also keeps the seeded
    /// scatter aspect-independent, so the same seed gives the same field at any
    /// target size (NFR §6).
    pos: [f32; 2],
    /// Velocity in **world** units per second — the flow field and the burst are
    /// screen-space forces, so they must not change magnitude with the domain.
    vel: [f32; 2],
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
    /// Depth: 0 = far, 1 = near (Plan 0043 Phase 3). Drives sprite scale, an
    /// atmospheric brightness fade, a parallax offset against the shared view
    /// transform, and which current the particle rides — **never** sorting, since
    /// the scene blends additively (ADR-0044).
    ///
    /// Fixed for the particle's life, like `hue` and `bright`: it comes off the
    /// seeded scatter, so the same seed gives the same depth sequence every run
    /// (NFR §6).
    z: f32,
    /// Per-particle palette offset and brightness, from the seeded scatter.
    hue: f32,
    bright: f32,
    size: f32,
}

/// ~10k-particle CPU flow-field swarm, driven by named preset parameters.
pub struct SwarmScene {
    pipeline: wgpu::RenderPipeline,
    instances: wgpu::Buffer,
    uniforms: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    particles: Vec<Particle>,
    instance_data: Vec<Instance>,
    /// Shared scene clock (seconds), set by the renderer each frame.
    time: f32,
    /// The **render target's** aspect, recorded by `render` for the next `update`
    /// to size the toroidal domain from (Plan 0043 Phase 1).
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
    /// positions are normalized, so a change rescales the field continuously.
    aspect: f32,
    /// Real elapsed seconds for this frame's integration (Plan 0014 Phase 2),
    /// injected via `advance` so the swarm moves at the same wall-clock rate on
    /// any refresh. Seeded to the fallback step for the first frame before any
    /// `advance` call.
    dt: f32,
    force: f32,
    spin: f32,
    /// The curl-noise field's own clock, integrated at `spin` ([`Phase`]).
    ///
    /// **Not `time * spin`** (ADR-0135). `spin` is the one rate here two shipped
    /// worlds bind to a band, and under the multiply a binding that moved
    /// rescaled every second already elapsed: at t = 100 s a 0.04 swing advanced
    /// this clock by 4 s in a single frame against a nominal 0.019 s, and the
    /// field re-rolled rather than flowing on. The particles steer by the field,
    /// so it reads as the flow changing its mind, not as a teleport.
    field_phase: Phase,
    burst: f32,
    hue: f32,
    brightness: f32,
    size: f32,
    field_freq: f32,
    zoom: f32,
    pan_x: f32,
    pan_y: f32,
    /// The active baked palette (ADR-0021), sampled per particle on the CPU. Set
    /// by `set_palette` on a preset switch; default `spectrum` reproduces the
    /// prior cosine.
    palette: Palette,
    /// Per-particle hue band + shared desaturation (ADR-0021).
    hue_spread: f32,
    hue_center: f32,
    saturation: f32,
    /// A/B palette crossfade position (Plan 0020 Phase 4); 0 = palette A.
    palette_mix: f32,
    /// Hard palette bands and their contour (ADR-0078), raw as the preset
    /// bound them -- `palette::band_steps` / `band_contour` condition them on
    /// the way to the sample site.
    palette_steps: f32,
    palette_contour: f32,
    /// The mark silhouette and its point count, **as bound** (ADR-0084). Both
    /// are quantized on the way to the uniform rather than here, so a
    /// `[smoothing]`-eased binding still eases — it just steps at the midpoints
    /// (see [`marks::mark_points`]).
    shape: f32,
    points: f32,
    /// The `star` arm's three shape params, raw as the preset bound them
    /// (Plan 0091 Phase 5). `marks::star_*` condition them on the way to the
    /// uniform. Inert on every other silhouette, and nothing warns —
    /// `presets/README.md` carries that.
    star_valley: f32,
    star_curve: f32,
    star_jitter: f32,
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
}

impl SwarmScene {
    /// Build the pipeline, buffers, and seeded particle set on `device`.
    /// `particles` is the active tier's
    /// [`swarm_particles`](crate::render::TierConfig::swarm_particles). The count
    /// is fixed for the life of the scene — the instance buffer and the CPU
    /// mirror are both sized to it here, so the per-frame path never allocates —
    /// and a tier change rebuilds the scene rather than resizing it.
    pub fn new(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        particles: usize,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("swarm-shader"),
            // The shared silhouette chunk first, then this scene's own source —
            // one `mark_distance`, two scenes (ADR-0084).
            source: wgpu::ShaderSource::Wgsl(format!("{}{SHADER}", marks::sdf_wgsl()).into()),
        });
        let instances = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("swarm-instances"),
            size: (particles * std::mem::size_of::<Instance>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let uniforms = gpu::uniform_buffer(device, "swarm-misc", std::mem::size_of::<Misc>());
        let bind_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("swarm-bind-layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("swarm-bind-group"),
            layout: &bind_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniforms.as_entire_binding(),
            }],
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
                    array_stride: std::mem::size_of::<Instance>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &wgpu::vertex_attr_array![
                        0 => Float32x2,
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
                    format: surface_format,
                    // Additive light, saturating coverage (ADR-0056) — shared
                    // with the line renderer so the two cannot drift.
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

        let mut rng = SeededRng::new(SEED);
        // The individuation draws come off the particle's index through `unit`,
        // NOT off `rng`: an extra `SeededRng` draw per particle would shift the
        // stream for every draw after it and re-scatter the whole field, and
        // the defaults' byte-identity claim (Plan 0077 Phase 2) rests on the
        // existing scatter being untouched.
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
            pipeline,
            instances,
            uniforms,
            bind_group,
            particles: particle_state,
            instance_data: vec![
                Instance {
                    center: [0.0, 0.0],
                    size: 0.0,
                    color: [0.0, 0.0, 0.0],
                    parallax: 1.0,
                };
                particles
            ],
            time: 0.0,
            aspect: FALLBACK_ASPECT,
            dt: FALLBACK_DT,
            force: DEFAULT_FORCE,
            spin: DEFAULT_SPIN,
            field_phase: Phase::default(),
            burst: DEFAULT_BURST,
            hue: DEFAULT_HUE,
            brightness: DEFAULT_BRIGHTNESS,
            size: DEFAULT_SIZE,
            field_freq: DEFAULT_FIELD_FREQ,
            zoom: DEFAULT_ZOOM,
            pan_x: DEFAULT_PAN,
            pan_y: DEFAULT_PAN,
            palette: Palette::default_spectrum(),
            hue_spread: DEFAULT_HUE_SPREAD,
            hue_center: DEFAULT_HUE_CENTER,
            saturation: DEFAULT_SATURATION,
            palette_mix: DEFAULT_PALETTE_MIX,
            palette_steps: palette::DEFAULT_PALETTE_STEPS,
            palette_contour: palette::DEFAULT_PALETTE_CONTOUR,
            shape: DEFAULT_SHAPE,
            points: DEFAULT_POINTS,
            star_valley: DEFAULT_STAR_VALLEY,
            star_curve: DEFAULT_STAR_CURVE,
            star_jitter: DEFAULT_STAR_JITTER,
            twinkle: DEFAULT_TWINKLE,
            size_spread: DEFAULT_SIZE_SPREAD,
            reseed: 0.0,
            prev_reseed: 0.0,
            reseed_count: 0,
        }
    }

    /// A particle scattered across the field with a random heading and tint.
    ///
    /// The scatter is in **normalized** domain coordinates, so it does not depend
    /// on the render target — the same seed gives the same field at any size
    /// (NFR §6).
    #[allow(
        clippy::indexing_slicing,
        reason = "pos/vel index a fixed [f32; 2] at constant 0/1, always in-bounds"
    )]
    fn spawn(rng: &mut SeededRng) -> Particle {
        let angle = rng.range(0.0, std::f32::consts::TAU);
        Particle {
            pos: [rng.range(-1.0, 1.0), rng.range(-1.0, 1.0)],
            vel: [angle.cos() * 0.2, angle.sin() * 0.2],
            z: rng.next_f32(),
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
}

/// The toroidal world half-extents for a render target of this aspect (Plan 0043
/// Phase 1).
///
/// The visible frame is `|world.y| <= 1` and `|world.x| <= aspect` — the shader
/// divides x by the aspect on its way to NDC — so this is the visible rectangle
/// scaled by [`MARGIN`], which is what puts the wrap seam off-screen. At
/// `MARGIN = 1` and 16:9 it returns `(1.78, 1.0)`, i.e. the constants it replaces.
fn bounds(aspect: f32) -> (f32, f32) {
    (aspect * MARGIN, MARGIN)
}

/// The LUT sample coordinate for one particle (ADR-0021): its per-particle hue
/// occupies the band `hue_center + (particle_hue - 0.5) * hue_spread`, plus the
/// shared `hue` rotation. Defaults (`center = 0.5`, `spread = 1`, `hue = 0`)
/// reduce to `particle_hue`, reproducing the prior full-wheel look.
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
pub const PARAMS: &[&str] = &[
    "force",
    "spin",
    "burst",
    "hue",
    "brightness",
    "size",
    "field_freq",
    "zoom",
    "pan_x",
    "pan_y",
    "hue_spread",
    "hue_center",
    "saturation",
    "palette_mix",
    "palette_steps",
    "palette_contour",
    // Per-mark individuation (Plan 0077 Phase 2) — the emitter's names, with
    // the emitter's semantics.
    "twinkle",
    "size_spread",
    // The percussive accent (Plan 0077 Phase 3) — the attractor's name, with
    // ADR-0066's disturbance semantics.
    "reseed",
    // The shared mark silhouette (ADR-0084) — the same two names the emitter
    // carries; `marks::PARAMS` is the single statement of the pair.
    "shape",
    "points",
    "star_valley",
    "star_curve",
    "star_jitter",
];

impl Scene for SwarmScene {
    fn name(&self) -> &'static str {
        "swarm"
    }

    fn advance(&mut self, dt: f32) {
        // A non-finite or negative `dt` degrades to the capture step rather than
        // poisoning `field_phase`, which is the one piece of state here that a
        // single bad frame could corrupt permanently — the damping `powf` below
        // only loses the frame it is given.
        self.dt = if dt.is_finite() && dt > 0.0 {
            dt
        } else {
            FALLBACK_DT
        };
    }

    fn set_time(&mut self, time: f32) {
        self.time = time;
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
        self.hue = DEFAULT_HUE;
        self.brightness = DEFAULT_BRIGHTNESS;
        self.size = DEFAULT_SIZE;
        self.field_freq = DEFAULT_FIELD_FREQ;
        self.zoom = DEFAULT_ZOOM;
        self.pan_x = DEFAULT_PAN;
        self.pan_y = DEFAULT_PAN;
        self.hue_spread = DEFAULT_HUE_SPREAD;
        self.hue_center = DEFAULT_HUE_CENTER;
        self.saturation = DEFAULT_SATURATION;
        self.palette_mix = DEFAULT_PALETTE_MIX;
        self.palette_steps = palette::DEFAULT_PALETTE_STEPS;
        self.palette_contour = palette::DEFAULT_PALETTE_CONTOUR;
        self.shape = DEFAULT_SHAPE;
        self.points = DEFAULT_POINTS;
        self.star_valley = DEFAULT_STAR_VALLEY;
        self.star_curve = DEFAULT_STAR_CURVE;
        self.star_jitter = DEFAULT_STAR_JITTER;
        self.twinkle = DEFAULT_TWINKLE;
        self.size_spread = DEFAULT_SIZE_SPREAD;
        // `prev_reseed` is deliberately NOT reset: this runs every frame
        // before the bindings are routed, and resetting the previous level
        // would turn a held gate into an edge per frame — a continuous
        // disturbance in place of a percussive one (measured while building
        // this: the population never re-gathered at all). The attractor's
        // reset_params makes the same omission for the same reason.
        self.reseed = 0.0;
    }

    fn set_param(&mut self, name: &str, value: f32) {
        match name {
            "force" => self.force = value,
            "spin" => self.spin = value,
            "burst" => self.burst = value,
            "hue" => self.hue = value,
            "brightness" => self.brightness = value,
            "size" => self.size = value,
            "field_freq" => self.field_freq = value,
            "zoom" => self.zoom = value,
            "pan_x" => self.pan_x = value,
            "pan_y" => self.pan_y = value,
            "hue_spread" => self.hue_spread = value,
            "hue_center" => self.hue_center = value,
            "saturation" => self.saturation = value,
            "palette_mix" => self.palette_mix = value,
            "palette_steps" => self.palette_steps = value,
            "palette_contour" => self.palette_contour = value,
            "shape" => self.shape = value,
            "points" => self.points = value,
            "star_valley" => self.star_valley = value,
            "star_curve" => self.star_curve = value,
            "star_jitter" => self.star_jitter = value,
            "twinkle" => self.twinkle = value,
            "size_spread" => self.size_spread = value,
            "reseed" => self.reseed = value,
            _ => {}
        }
    }

    #[allow(
        clippy::indexing_slicing,
        reason = "pos/vel index fixed [f32; 2] and base indexes a fixed [f32; 3], all at constant offsets, always in-bounds"
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
        // Hoisted out of the loop: one read, 10 000 uses (Plan 0043 Phase 2).
        let field_freq = self.field_freq;
        // The individuation pair, resolved at draw like the emitter's: an
        // eased width moves the whole population continuously instead of only
        // particles spawned since the change (Plan 0077 Phase 2).
        let twinkle = self.twinkle;
        let size_spread = self.size_spread;
        let time = self.time;

        // Frame-rate-independent integration (Plan 0014 Phase 2): scale the
        // acceleration/advection by real `dt`, and raise the per-frame damping to
        // the `dt`-relative power so the velocity decays at the same wall-clock
        // rate regardless of refresh (one `powf` per frame, not per particle).
        // At `dt == FALLBACK_DT` (1/60) this reduces to the former fixed step, so
        // the look is unchanged live and byte-identical under fixed-`dt` capture.
        let dt = self.dt;
        let damp = DAMPING.powf(dt * 60.0);

        // The domain follows the render target (Plan 0043 Phase 1). Computed once
        // per frame, outside the loop; positions are normalized, so a change in
        // these rescales the whole field at once instead of wrapping particles
        // individually — no resize teleport (ADR-0044).
        let (bound_x, bound_y) = bounds(self.aspect);

        for (p, inst) in self.particles.iter_mut().zip(self.instance_data.iter_mut()) {
            // Normalized torus position -> world, which is what the field, the
            // burst and the sprite all work in.
            let world = [p.pos[0] * bound_x, p.pos[1] * bound_y];

            // Scalar potential -> flow direction (cheap curl-ish field), sampled at
            // a depth-dependent phase so each layer rides its own currents rather
            // than the same streamlines at several sizes (Plan 0043 Phase 3). The
            // two axes take different offsets, so layers decorrelate in both.
            let zo = p.z * DEPTH_FIELD_OFFSET;
            let a = (world[0] * field_freq + field_t + zo).sin()
                + (world[1] * field_freq - field_t * 0.8 - zo * 0.7).cos();
            let dir = [a.cos(), a.sin()];

            p.vel[0] = p.vel[0] * damp + dir[0] * force * dt;
            p.vel[1] = p.vel[1] * damp + dir[1] * force * dt;

            // Beat burst pushes particles radially outward from center.
            if burst_kick > 0.0 {
                let r = (world[0] * world[0] + world[1] * world[1]).sqrt().max(1e-3);
                p.vel[0] += world[0] / r * burst_kick * dt;
                p.vel[1] += world[1] / r * burst_kick * dt;
            }

            // Integrate a world-space velocity into a normalized position.
            p.pos[0] += p.vel[0] * dt / bound_x;
            p.pos[1] += p.vel[1] * dt / bound_y;

            // Toroidal wrap keeps the field populated (no respawns/hitches). In
            // normalized space the seam is at +/-1 whatever the target is, and it
            // is `MARGIN` past the visible frame — which is what stopped it from
            // burning a bright bar into the feedback stage (ADR-0044).
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

            let speed = (p.vel[0] * p.vel[0] + p.vel[1] * p.vel[1]).sqrt();
            // Colour through the shared LUT (ADR-0021): the per-particle hue is
            // mapped into the `hue_spread`/`hue_center` band, then desaturated by
            // the shared `saturation`. Defaults reproduce the prior full-wheel look.
            let coord = hue_coord(self.hue_center, self.hue_spread, p.hue, self.hue);
            // Hard bands on the palette coordinate (ADR-0078), the canonical
            // `palette::band_coord` called rather than copied. `palette_steps <= 1`
            // returns it untouched, so an unbound preset is byte-unchanged.
            let base = palette::desaturate(
                self.palette.sample(
                    palette::band_coord(coord, self.palette_steps),
                    self.palette_mix,
                ),
                self.saturation,
            );
            // Depth, resolved into the three visual terms it drives (Plan 0043
            // Phase 3). Three `mul_add`-shaped lerps on a value that never changes
            // — the whole per-particle cost of the depth axis.
            let depth_scale = DEPTH_SCALE_FAR + (DEPTH_SCALE_NEAR - DEPTH_SCALE_FAR) * p.z;
            let depth_fade = DEPTH_FADE_FAR + (DEPTH_FADE_NEAR - DEPTH_FADE_FAR) * p.z;
            let parallax = DEPTH_PARALLAX_FAR + (DEPTH_PARALLAX_NEAR - DEPTH_PARALLAX_FAR) * p.z;

            // The speed cue predates depth and still earns its place: on a coherent
            // field the fast channels read brighter than slack water. The
            // atmospheric fade multiplies it rather than replacing it. The
            // twinkle factor is exactly 1.0 when `twinkle` is unbound, and the
            // size factor exactly 1.0 at zero spread — multiplying by either is
            // bit-exact, which is what keeps the shipped captures byte-identical
            // (Plan 0077 Phase 2).
            let bright = ((0.25 + speed * 0.7) * p.bright).min(1.6)
                * self.brightness
                * depth_fade
                * twinkle_factor(p.twinkle_freq, p.twinkle_phase, time, twinkle);

            *inst = Instance {
                center: [p.pos[0] * bound_x, p.pos[1] * bound_y],
                size: p.size * self.size * depth_scale * size_factor(p.size_unit, size_spread),
                color: [base[0] * bright, base[1] * bright, base[2] * bright],
                parallax,
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
        // The domain the *next* `update` wraps against (Plan 0043 Phase 1). This
        // argument is the render target's aspect — the only correct source for a
        // shape (ADR-0037); see the field's docs for why `set_target_size` is not.
        self.aspect = aspect.max(0.1);
        queue.write_buffer(
            &self.instances,
            0,
            bytemuck::cast_slice(&self.instance_data),
        );
        queue.write_buffer(
            &self.uniforms,
            0,
            bytemuck::bytes_of(&Misc {
                v: [self.aspect, self.zoom, self.pan_x, self.pan_y],
                // Quantized here, on the way into the uniform, so the shader's
                // precondition stays visible on the CPU side: the roster's
                // bounds and the integer point count live in `marks`, and no
                // fractional value ever reaches an angular fold (ADR-0084).
                m: [
                    marks::mark_shape(self.shape),
                    marks::mark_points(self.points),
                    0.0,
                    0.0,
                ],
                s: [
                    marks::star_valley(self.star_valley),
                    marks::star_curve(self.star_curve),
                    marks::star_jitter(self.star_jitter),
                    0.0,
                ],
            }),
        );

        // Load over the engine backdrop (ADR-0018): the additive particles
        // bloom over whatever the background pass painted, so the sparse gaps
        // between them reveal it.
        let mut pass = gpu::color_pass(encoder, "swarm-pass", view, wgpu::LoadOp::Load);
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.set_vertex_buffer(0, self.instances.slice(..));
        pass.draw(0..6, 0..self.particles.len() as u32);
    }
}

#[cfg(test)]
mod tests;
