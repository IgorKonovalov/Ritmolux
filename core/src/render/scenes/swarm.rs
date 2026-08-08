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
use super::{FALLBACK_DT, Scene, SeededRng};
use crate::dsp::AnalysisFrame;
use crate::render::gpu;
use crate::render::palette::{self, Palette};

const SEED: u64 = 0x4C4D_565F_5357_524D; // "LMV_SWRM"

/// How far the toroidal domain extends past the visible frame (Plan 0043 Phase 1,
/// [ADR-0044](../../../../docs/adrs/0044-swarm-world-is-a-25d-torus-sized-from-the-target.md)).
///
/// The world half-extents used to be `BOUND_X = 1.8` / `BOUND_Y = 1.0` — and
/// `1.0` **is** the NDC frame edge. The wrap is toroidal, so that line was the one
/// place on screen every wrapping particle was guaranteed to paint, and the
/// feedback stage integrated it into a saturated bar across the top and bottom of
/// every swarm preset within a few hundred frames.
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

/// The scene's own WGSL. The shared mark-silhouette chunk
/// ([`marks::sdf_wgsl`]) is prepended at module creation, so `mark_distance` here
/// is the same function the emitter evaluates.
///
/// **`shape` and `points` travel vertex -> fragment as flat varyings rather than
/// being read from `misc` in the fragment stage**, and that is deliberate. The
/// fragment stage cannot see this scene's uniform without widening the bind
/// layout's visibility to `VERTEX_FRAGMENT` — which would make this descriptor
/// byte-identical to the line renderer's (`{uniform, VERTEX_FRAGMENT,
/// min_binding_size: None}`), the exact collision shape
/// [ADR-0058](../../../../docs/adrs/0058-bind-group-layout-collisions-carry-evidence.md)
/// records and the one the emitter's layout comment says not to tidy back in. A
/// flat varying carries a per-draw value with no descriptor change at all.
const SHADER: &str = r#"
struct Misc {
    // x: aspect, y: zoom, zw: pan (the shared ViewTransform, ADR-0018)
    v: vec4<f32>,
    // x: mark shape index, y: quantized point count (ADR-0084). Per draw, not
    // per instance: the branch stays uniform across a warp and `Instance` does
    // not grow.
    m: vec4<f32>,
}

@group(0) @binding(0) var<uniform> misc: Misc;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) local: vec2<f32>,
    @location(1) color: vec3<f32>,
    @location(2) @interpolate(flat) shape: f32,
    @location(3) @interpolate(flat) points: f32,
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
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    // The silhouette (ADR-0084). At the default `disc` this is `length(in.local)`
    // and nothing else, so an unshaped swarm is the arithmetic it always was; the
    // falloff below is untouched either way, so a visual change is attributable
    // to the shape alone.
    let d = mark_distance(in.local, in.shape, in.points);
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
    /// The mark silhouette and its point count, **as bound** (ADR-0084). Both
    /// are quantized on the way to the uniform rather than here, so a
    /// `[smoothing]`-eased binding still eases — it just steps at the midpoints
    /// (see [`marks::mark_points`]).
    shape: f32,
    points: f32,
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
        let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("swarm-misc"),
            size: std::mem::size_of::<Misc>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
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
        let particle_state: Vec<Particle> = (0..particles).map(|_| Self::spawn(&mut rng)).collect();

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
            shape: DEFAULT_SHAPE,
            points: DEFAULT_POINTS,
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
    // The shared mark silhouette (ADR-0084) — the same two names the emitter
    // carries; `marks::PARAMS` is the single statement of the pair.
    "shape",
    "points",
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
        self.shape = DEFAULT_SHAPE;
        self.points = DEFAULT_POINTS;
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
            "shape" => self.shape = value,
            "points" => self.points = value,
            _ => {}
        }
    }

    #[allow(
        clippy::indexing_slicing,
        reason = "pos/vel index fixed [f32; 2] and base indexes a fixed [f32; 3], all at constant offsets, always in-bounds"
    )]
    fn update(&mut self, _frame: &AnalysisFrame) {
        // Field evolves at `spin`; `force` steers, `burst` shoves outward.
        let field_t = self.time * self.spin;
        let force = self.force;
        let burst_kick = self.burst;
        // Hoisted out of the loop: one read, 10 000 uses (Plan 0043 Phase 2).
        let field_freq = self.field_freq;

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
            let base = palette::desaturate(
                self.palette.sample(coord, self.palette_mix),
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
            // atmospheric fade multiplies it rather than replacing it.
            let bright = ((0.25 + speed * 0.7) * p.bright).min(1.6) * self.brightness * depth_fade;

            *inst = Instance {
                center: [p.pos[0] * bound_x, p.pos[1] * bound_y],
                size: p.size * self.size * depth_scale,
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
            }),
        );

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("swarm-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    // Load over the engine backdrop (ADR-0018): the additive
                    // particles bloom over whatever the background pass painted,
                    // so the sparse gaps between them reveal it.
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.set_vertex_buffer(0, self.instances.slice(..));
        pass.draw(0..6, 0..self.particles.len() as u32);
    }
}

#[cfg(test)]
mod tests;
