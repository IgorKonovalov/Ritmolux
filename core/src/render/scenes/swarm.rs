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
mod tests {
    // Tests index fixed-size arrays and panic on failure; allowed over the
    // file's hot-path pragma — this is not the render path.
    #![allow(clippy::indexing_slicing, clippy::panic, clippy::expect_used)]

    use super::{
        DEFAULT_HUE, DEFAULT_HUE_CENTER, DEFAULT_HUE_SPREAD, DEPTH_PARALLAX_FAR,
        DEPTH_PARALLAX_NEAR, DEPTH_SCALE_FAR, DEPTH_SCALE_NEAR, MARGIN, SEED, Scene, SwarmScene,
        bounds, hue_coord,
    };
    use crate::render::palette::Palette;
    use crate::render::scenes::SeededRng;

    /// The particle count these tests run at — the floor tier's, which is the
    /// number the seeded-scatter assertions below were written against and the one
    /// every golden capture draws (Plan 0044).
    const FLOOR_PARTICLES: usize = crate::render::TierConfig::FLOOR.swarm_particles;

    /// Target aspects worth checking a domain against: 16:9, the 16:10 the fixed
    /// constants disagreed with, 4:3, an ultrawide, and a portrait.
    const ASPECTS: [f32; 5] = [16.0 / 9.0, 16.0 / 10.0, 4.0 / 3.0, 21.0 / 9.0, 9.0 / 16.0];

    /// The domain has the **render target's** shape, not the baked 16:9 the
    /// replaced constants encoded (ADR-0037, ADR-0044).
    ///
    /// The visible frame is `|world.y| <= 1` by `|world.x| <= aspect`, so a domain
    /// that fills it without over-filling has exactly that ratio. The old
    /// `BOUND_X = 1.8` / `BOUND_Y = 1.0` pair is 1.80 at every target size — right
    /// only at 16:9, and the reason no existing test could tell: at 16:10 it
    /// over-fills horizontally by 12 %.
    #[test]
    fn the_domain_takes_its_shape_from_the_target() {
        for aspect in ASPECTS {
            let (bx, by) = bounds(aspect);
            assert!(
                (bx / by - aspect).abs() < 1e-5,
                "domain shape {:.4} must equal the target's {aspect:.4}",
                bx / by
            );
        }

        // The pair it replaced, for the record: correct at 16:9 and wrong the
        // moment the target is anything else.
        let (old_x, old_y) = (1.8f32, 1.0f32);
        let sixteen_ten = 16.0 / 10.0;
        assert!(
            (old_x / old_y - sixteen_ten).abs() > 0.1,
            "the fixed constants must genuinely disagree with 16:10, or this guards nothing"
        );
    }

    /// **The artifact fix, as arithmetic** (backlog 0029): the toroidal wrap seam
    /// projects outside the visible frame across the whole `zoom`/`pan_*` range the
    /// swarm family works in, so no particle is guaranteed to paint on a fixed
    /// on-screen line and the feedback stage has no bar to integrate.
    ///
    /// The shader projects a particle at world `p` to
    /// `ndc = ((p.x * zoom + pan_x) / aspect, p.y * zoom + pan_y)`, so a seam at
    /// `+bound` clears the frame when `bound * zoom - |pan| > extent`. Non-vacuous
    /// by construction: at the old `MARGIN = 1` equivalent the y seam lands exactly
    /// on `ndc.y = 1` at `zoom = 1`, which is the reported defect.
    ///
    /// Asserted twice, because the margin is **proportional** and `pan_*` is in
    /// world units: the general clearance scales with each axis' half-extent, so
    /// the literal `0.16` the presets pan by buys different headroom on a wide
    /// target than on a tall one. The second block pins the family's own number
    /// against the landscape targets it will actually meet. On a portrait target
    /// the x axis is the tight one — `9:16` leaves 0.14 of pan headroom against
    /// that 0.16 — which is a property of `pan_x` being world-space, not something
    /// the domain should distort its shape to paper over.
    #[test]
    fn the_wrap_seam_projects_outside_the_visible_frame() {
        // The family's working range (Plan 0043 Phase 1).
        const ZOOMS: [f32; 4] = [1.0, 1.1, 1.2, 1.3];
        /// The range the **shipped** presets actually reach, which starts *below* 1:
        /// `swarm_drift.toml` binds `zoom = "1.04 + sin(...) * 0.05 + ..."`, so it
        /// bottoms out just under 1 and a guard starting at 1.0 leaves the shipped
        /// minimum unmeasured (Plan 0043 close review). Used for the concrete block
        /// below, not the general one — "clears by at least `headroom`" is a
        /// `zoom >= 1` property by construction, while "clears at all" is the claim
        /// that has to hold everywhere the family goes.
        const SHIPPED_ZOOMS: [f32; 5] = [0.99, 1.0, 1.1, 1.2, 1.3];
        /// The largest `pan_*` amplitude any surviving preset binds (Drift's `pan_x`).
        /// A future preset that pans further or zooms lower has to widen these two —
        /// which is why they say where they come from.
        const PAN: f32 = 0.16;
        let headroom = MARGIN - 1.0;

        // General: on any target and anywhere in the working zoom range, each seam
        // clears its frame edge by at least `headroom` of that axis' half-extent.
        for aspect in ASPECTS {
            let (bx, by) = bounds(aspect);
            for zoom in ZOOMS {
                assert!(
                    by * zoom - 1.0 >= headroom - 1e-5,
                    "y seam clears by {:.4}, want >= {headroom:.4} (aspect {aspect:.3}, \
                     zoom {zoom:.2})",
                    by * zoom - 1.0
                );
                assert!(
                    bx * zoom - aspect >= aspect * headroom - 1e-5,
                    "x seam clears by {:.4}, want >= {:.4} (aspect {aspect:.3}, zoom {zoom:.2})",
                    bx * zoom - aspect,
                    aspect * headroom
                );
            }
        }

        // Concrete: the pan the swarm presets reach, against every landscape
        // target, **at the near depth layer**. Parallax scales both the pan offset
        // and the zoom deflection, and the near layer takes the most pan — so it is
        // the one whose seam sits closest to the frame and the only one worth
        // asserting. Checking a depth-agnostic 1.0 here would pass while the layer
        // that actually binds went unmeasured.
        for aspect in [16.0 / 9.0, 16.0 / 10.0, 4.0 / 3.0, 21.0 / 9.0] {
            let (bx, by) = bounds(aspect);
            for zoom in SHIPPED_ZOOMS {
                let par = DEPTH_PARALLAX_NEAR;
                let seam_y = by * (1.0 + (zoom - 1.0) * par) - PAN * par;
                let seam_x = bx * (1.0 + (zoom - 1.0) * par) - PAN * par;
                assert!(
                    seam_y > 1.0,
                    "near-layer y seam projects to {seam_y:.3} at zoom {zoom:.2}, pan {PAN} \
                     — inside the frame"
                );
                assert!(
                    seam_x > aspect,
                    "near-layer x seam projects to {seam_x:.3} at zoom {zoom:.2}, pan {PAN} \
                     — inside the half-width {aspect:.3}"
                );
            }
        }

        // What the margin costs, stated so a change to it is deliberate: the
        // visible fraction of the domain is 1 / MARGIN^2.
        let visible = 1.0 / (MARGIN * MARGIN);
        assert!(
            (0.5..0.85).contains(&visible),
            "a margin keeping under half the particles on screen is too expensive: {visible:.3}"
        );
    }

    /// **Parallax is present, not merely a scale change** (Plan 0043 Phase 3's
    /// done-when): under the same `pan_*`, a near particle traverses the frame
    /// measurably faster than a far one.
    ///
    /// Replicates the vertex shader's projection exactly — that one expression is
    /// the whole depth transform, so asserting on it is asserting on what the GPU
    /// does. Two claims, and the second is what stops this from being a tautology
    /// about a constant: the layers separate under a pan, and they do **not**
    /// separate at the identity transform, so an unbound preset gets no parallax
    /// distortion at all.
    #[test]
    fn near_particles_traverse_the_frame_faster_than_far_ones() {
        // `misc.v`: ndc.x = (center.x * (1 + (zoom - 1) * par) + pan.x * par) / aspect
        let project = |center_x: f32, zoom: f32, pan_x: f32, par: f32, aspect: f32| {
            (center_x * (1.0 + (zoom - 1.0) * par) + pan_x * par) / aspect
        };
        let aspect = 16.0 / 9.0;
        let (near, far) = (DEPTH_PARALLAX_NEAR, DEPTH_PARALLAX_FAR);

        // Two particles at the same place, at opposite depths, under a pan sweep.
        let center_x = 0.4;
        let (pan_a, pan_b) = (0.0, 0.3);
        let travel = |par: f32| {
            (project(center_x, 1.0, pan_b, par, aspect)
                - project(center_x, 1.0, pan_a, par, aspect))
            .abs()
        };
        let (near_travel, far_travel) = (travel(near), travel(far));
        assert!(
            near_travel > far_travel * 1.5,
            "the near layer must outrun the far one: {near_travel:.4} vs {far_travel:.4} \
             (ratio {:.2})",
            near_travel / far_travel
        );

        // A zoom deflection separates them too — depth is not pan-only.
        let zoomed = |par: f32| (project(center_x, 1.3, 0.0, par, aspect)).abs();
        assert!(
            zoomed(near) > zoomed(far) * 1.05,
            "zoom must deflect the near layer further: {:.4} vs {:.4}",
            zoomed(near),
            zoomed(far)
        );

        // ...and at the identity transform every depth projects to the same place,
        // so an unbound preset is untouched by the depth axis' parallax term.
        for par in [far, 1.0, near] {
            assert!(
                (project(center_x, 1.0, 0.0, par, aspect) - center_x / aspect).abs() < 1e-6,
                "identity zoom/pan must be depth-independent"
            );
        }
    }

    /// The depth axis is **seeded**, so a capture is reproducible run-to-run
    /// (NFR §6) — and it genuinely spans the range, which is what makes the scale,
    /// fade and parallax lerps do anything.
    #[test]
    fn the_seeded_scatter_reproduces_the_same_depth_sequence() {
        let depths = || {
            let mut rng = SeededRng::new(SEED);
            (0..FLOOR_PARTICLES)
                .map(|_| SwarmScene::spawn(&mut rng).z)
                .collect::<Vec<f32>>()
        };
        let (a, b) = (depths(), depths());
        assert_eq!(a, b, "the same seed must give the same depth sequence");

        let lo = a.iter().copied().fold(f32::INFINITY, f32::min);
        let hi = a.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        let mean = a.iter().sum::<f32>() / a.len() as f32;
        assert!(
            (0.0..0.02).contains(&lo) && (0.98..=1.0).contains(&hi),
            "depth must span the full 0..1 range, got {lo:.4}..{hi:.4}"
        );
        assert!(
            (0.45..0.55).contains(&mean),
            "depth must populate the range evenly, mean was {mean:.4}"
        );
    }

    /// A resize rescales the field instead of teleporting it (ADR-0044's
    /// consequence: "the wrap must stay stable across one rather than teleporting
    /// every particle at once").
    ///
    /// Normalized storage is what buys this, and the test says so by measuring the
    /// alternative alongside: with world-space positions, shrinking the domain
    /// re-wraps everything outside the new bounds, and those particles jump by a
    /// full domain width. Normalized positions move continuously with the change.
    #[test]
    fn a_resize_rescales_the_field_rather_than_wrapping_it() {
        let (before, after) = (16.0 / 9.0, 16.0 / 10.0);
        let (bx0, by0) = bounds(before);
        let (bx1, by1) = bounds(after);

        // A fan of normalized positions spanning the torus, including both seams.
        let samples: Vec<[f32; 2]> = (0..64)
            .map(|i| {
                let u = i as f32 / 63.0 * 2.0 - 1.0;
                [u, -u]
            })
            .collect();

        let mut worst_normalized = 0.0f32;
        let mut worst_world_space = 0.0f32;
        for s in &samples {
            // What this scene does: the normalized position is untouched, so the
            // world position moves by exactly the change in the half-extents.
            let moved =
                ((s[0] * bx1 - s[0] * bx0).powi(2) + (s[1] * by1 - s[1] * by0).powi(2)).sqrt();
            worst_normalized = worst_normalized.max(moved);

            // What a world-space store would do: keep the world position and
            // re-wrap it into the new domain.
            let (mut wx, wy) = (s[0] * bx0, s[1] * by0);
            if wx > bx1 {
                wx -= 2.0 * bx1;
            } else if wx < -bx1 {
                wx += 2.0 * bx1;
            }
            let jump = ((wx - s[0] * bx0).powi(2) + (wy - s[1] * by0).powi(2)).sqrt();
            worst_world_space = worst_world_space.max(jump);
        }

        // 16:9 -> 16:10 narrows the x half-extent by ~0.22 world units; nothing
        // moves further than that, and the y axis does not move at all.
        assert!(
            worst_normalized < 0.3,
            "a resize must move particles continuously, worst was {worst_normalized:.3}"
        );
        assert!(
            worst_world_space > 2.0,
            "the world-space alternative must genuinely teleport, or this test proves \
             nothing: worst jump was {worst_world_space:.3}"
        );
    }

    /// The default hue band (`center = 0.5`, `spread = 1`, `hue = 0`) reduces to
    /// `particle_hue`, so the swarm's colour is unchanged from before Plan 0020.
    #[test]
    fn default_hue_band_is_the_prior_full_wheel() {
        for &ph in &[0.0, 0.2, 0.5, 0.73, 0.99] {
            let coord = hue_coord(DEFAULT_HUE_CENTER, DEFAULT_HUE_SPREAD, ph, DEFAULT_HUE);
            assert!(
                (coord - ph).abs() < 1e-6,
                "default band maps particle_hue to itself: {coord} vs {ph}"
            );
        }
    }

    /// A narrow `hue_spread` collapses the full particle-hue range into a tight
    /// LUT band, so the sampled colours cluster (a coherent single-family swarm)
    /// where `spread = 1` samples the whole wheel (rainbow confetti). Measured as
    /// the spread of sampled RGB — the gap the plan closes.
    #[test]
    fn narrow_spread_makes_colour_coherent() {
        let pal = Palette::default_spectrum();
        // Total variance of the sampled colours across a fan of particle hues.
        let colour_spread = |spread: f32| -> f32 {
            let hues: Vec<f32> = (0..64).map(|i| i as f32 / 64.0).collect();
            let cols: Vec<[f32; 3]> = hues
                .iter()
                .map(|&h| pal.sample(hue_coord(0.5, spread, h, 0.0), 0.0))
                .collect();
            let n = cols.len() as f32;
            let mut mean = [0.0f32; 3];
            for c in &cols {
                for k in 0..3 {
                    mean[k] += c[k] / n;
                }
            }
            let mut var = 0.0f32;
            for c in &cols {
                for k in 0..3 {
                    var += (c[k] - mean[k]).powi(2);
                }
            }
            var / n
        };
        let narrow = colour_spread(0.1);
        let full = colour_spread(1.0);
        assert!(
            narrow < full * 0.25,
            "narrow band ({narrow:.4}) is far more coherent than the full wheel ({full:.4})"
        );
    }

    // -----------------------------------------------------------------------
    // The mark silhouette (Plan 0070 Phase 1, ADR-0084)
    // -----------------------------------------------------------------------

    /// The square capture the single-mark probe below draws into. Large enough
    /// that a seven-pointed star's valleys are tens of pixels from its tips —
    /// the whole point of the count is that the profile has structure, and at
    /// `golden.rs`'s 128 there is not enough of it to bin cleanly.
    const MARK_CAPTURE: u32 = 256;

    /// The mark's half-size in world units. The frame is `|ndc| <= 1` on a square
    /// target, so this leaves a tenth of the frame outside the sprite quad.
    const MARK_HALF: f32 = 0.9;

    /// **One mark, drawn large and centred, through the real swarm pipeline** —
    /// the linear composite it wrote, RGBA, row-major.
    ///
    /// A swarm normally draws thousands of sprites and no single silhouette is
    /// legible in the sum, so this builds the scene with a pool of **one**. Two
    /// tricks make that one mark measurable, and both are arithmetic rather than
    /// tuning:
    ///
    /// - **It is centred exactly**, whatever the seeded scatter put the particle
    ///   at. The vertex shader computes
    ///   `center * (1 + (zoom - 1) * parallax) + pan * parallax`, so
    ///   `zoom = 1 - 1/parallax` with `pan = 0` collapses the position term to
    ///   zero identically. The particle's own `parallax` comes off its depth,
    ///   which the seeded draw is replayed here to read.
    /// - **It is scaled to a known size** by dividing [`MARK_HALF`] through the
    ///   per-particle size and depth scale the same replay gives.
    ///
    /// `saturation = 0` so every lit pixel is grey: the profile below thresholds
    /// on luminance, and a palette sample that happened to be dark in one channel
    /// would put notches in it that have nothing to do with the shape.
    fn capture_one_mark(shape: f32, points: f32) -> Option<Vec<f32>> {
        use crate::dsp::AnalysisFrame;
        use crate::render::context::RenderError;
        use crate::render::{COMPOSITE_FORMAT, HeadlessOptions, Renderer, capture};

        let renderer = match Renderer::new_headless(HeadlessOptions {
            width: MARK_CAPTURE,
            height: MARK_CAPTURE,
            prefer_software: true,
        }) {
            Ok(renderer) => renderer,
            Err(RenderError::RequestAdapter(_)) => {
                eprintln!("skipped: no GPU adapter on this runner (ADR-0016)");
                return None;
            }
            Err(e) => panic!("headless renderer build failed: {e}"),
        };
        let device = renderer.ctx.device.clone();
        let queue = renderer.ctx.queue.clone();

        // Replay the seeded draw the scene's own pool will make, so the depth
        // terms below are the particle's and not an assumption about it.
        let mut rng = SeededRng::new(SEED);
        let particle = SwarmScene::spawn(&mut rng);
        let parallax = DEPTH_PARALLAX_FAR + (DEPTH_PARALLAX_NEAR - DEPTH_PARALLAX_FAR) * particle.z;
        let depth_scale = DEPTH_SCALE_FAR + (DEPTH_SCALE_NEAR - DEPTH_SCALE_FAR) * particle.z;

        let mut scene = SwarmScene::new(&device, COMPOSITE_FORMAT, 1);
        for (name, value) in [
            ("force", 0.0),
            ("spin", 0.0),
            ("burst", 0.0),
            ("brightness", 1.0),
            ("saturation", 0.0),
            ("size", MARK_HALF / (particle.size * depth_scale)),
            ("zoom", 1.0 - 1.0 / parallax),
            ("pan_x", 0.0),
            ("pan_y", 0.0),
            ("shape", shape),
            ("points", points),
        ] {
            scene.set_param(name, value);
        }
        scene.set_time(0.0);
        scene.update(&AnalysisFrame::default());

        let (texture, view) =
            capture::create_target(&device, COMPOSITE_FORMAT, MARK_CAPTURE, MARK_CAPTURE);
        let (buffer, padded_bpr) =
            capture::create_linear_readback(&device, MARK_CAPTURE, MARK_CAPTURE);
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("swarm-one-mark"),
        });
        capture::record_clear(&mut encoder, &view);
        // A square target, so the shader's aspect divide is the identity and a
        // world unit is a normalized-device unit on both axes.
        scene.render(&queue, &mut encoder, &view, 1.0);
        capture::record_copy(
            &mut encoder,
            &texture,
            &buffer,
            padded_bpr,
            MARK_CAPTURE,
            MARK_CAPTURE,
        );
        queue.submit(std::iter::once(encoder.finish()));
        Some(
            capture::read_back_linear(&device, &buffer, MARK_CAPTURE, MARK_CAPTURE, padded_bpr)
                .expect("read back the one-mark composite"),
        )
    }

    /// The lit radius of a capture, per direction: for each of `rays` angles out
    /// of the frame centre, the furthest sample whose luminance still clears a
    /// fraction of the frame's brightest.
    ///
    /// Marched along the ray rather than binned by pixel angle, because binning
    /// leaves *holes*: at a hundred directions the angular width of a bin is
    /// under a pixel of arc at small radii, so a valley direction can contain no
    /// pixel centre at all and read as a lit radius of zero. Marching asks each
    /// direction directly.
    ///
    /// The falloff is radial along every ray by construction — `d` scales
    /// linearly with radius for the disc, polygon and star arms — so this profile
    /// is the silhouette's own boundary radius times a constant, and its maxima
    /// are the shape's points.
    fn lit_radius_profile(pixels: &[f32], rays: usize) -> Vec<f32> {
        let size = MARK_CAPTURE as usize;
        let centre = MARK_CAPTURE as f32 * 0.5;
        let lum = |x: f32, y: f32| -> f32 {
            // The capture is row-major top-to-bottom; the sprite's `local` frame
            // has +y up, so the row index counts down from the centre.
            let col = (centre + x).floor();
            let row = (centre - y).floor();
            if col < 0.0 || row < 0.0 || col >= size as f32 || row >= size as f32 {
                return 0.0;
            }
            let base = (row as usize * size + col as usize) * 4;
            pixels
                .get(base..base + 3)
                .map_or(0.0, |px| px[0] + px[1] + px[2])
        };
        let peak = pixels
            .chunks_exact(4)
            .map(|px| px[0] + px[1] + px[2])
            .fold(0.0f32, f32::max);
        assert!(peak > 0.0, "the one-mark capture is empty");
        // A fifth of the brightest sample: well clear of the half-float floor,
        // and — because `g = (1 - d)^2` — a contour at a fixed fraction of the
        // shape's own radius, so it has the silhouette's outline.
        let threshold = peak * 0.2;

        (0..rays)
            .map(|i| {
                let a = std::f32::consts::TAU * i as f32 / rays as f32;
                let (dx, dy) = (a.cos(), a.sin());
                let steps = (centre * 2.0) as usize;
                let mut furthest = 0.0f32;
                for s in 0..steps {
                    let r = s as f32 * 0.5;
                    if r > centre - 1.0 {
                        break;
                    }
                    if lum(dx * r, dy * r) > threshold {
                        furthest = r;
                    }
                }
                furthest
            })
            .collect()
    }

    /// How many separated angular maxima a circular profile has — the point
    /// count, counted rather than eyeballed.
    ///
    /// A **Schmitt trigger against the mark's own outer radius**, not a
    /// derivative test and not the midpoint of the profile's own range. Both of
    /// those count rasterization noise: a disc's profile here spans 62.8 to 64.0
    /// px, and the midpoint of *that* range is crossed 88 times by a circle.
    /// Here a lobe is an angular run reaching within 20 % of the furthest lit
    /// radius, separated from the next by a dip below 70 % of it — so a figure
    /// that never dips (a disc, a many-sided polygon) is one lobe, and a
    /// seven-pointed star, whose valleys sit at 45 % of its tips, is seven.
    fn angular_lobes(profile: &[f32]) -> usize {
        let hi = profile.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        let (on, off) = (hi * 0.8, hi * 0.7);
        let n = profile.len();
        // Latched state, walked twice so the wrap-around is settled before the
        // count starts.
        let mut lit = profile.iter().copied().fold(true, |acc, r| {
            if r >= on {
                true
            } else if r <= off {
                false
            } else {
                acc
            }
        });
        if profile.iter().all(|&r| r > on) {
            return 1;
        }
        let mut lobes = 0usize;
        for i in 0..n {
            let r = profile.get(i).copied().unwrap_or(0.0);
            let next = if r >= on {
                true
            } else if r <= off {
                false
            } else {
                lit
            };
            if next && !lit {
                lobes += 1;
            }
            lit = next;
        }
        lobes
    }

    /// **A seven-pointed star has exactly seven angular maxima** (Plan 0070
    /// Phase 1's second done-when), counted off a real capture of the real
    /// pipeline rather than asserted by eye.
    ///
    /// Three counts, because one would not separate "the shape is a star" from
    /// "the profile is noisy": the same probe at 5, 7 and 9 points must return 5,
    /// 7 and 9. And a disc must return **one** — a circle's lit radius is
    /// constant, so a lobe count above 1 on it would mean this whole measurement
    /// is reading rasterization noise and the star counts prove nothing.
    #[test]
    fn a_seven_pointed_star_has_seven_angular_maxima() {
        const BINS: usize = 360;
        const STAR: f32 = 3.0;
        const DISC: f32 = 0.0;

        let Some(disc) = capture_one_mark(DISC, 7.0) else {
            return;
        };
        let disc_profile = lit_radius_profile(&disc, BINS);
        let disc_lobes = angular_lobes(&disc_profile);
        let (lo, hi) = (
            disc_profile.iter().copied().fold(f32::INFINITY, f32::min),
            disc_profile
                .iter()
                .copied()
                .fold(f32::NEG_INFINITY, f32::max),
        );
        eprintln!("disc lit radius {lo:.1}..{hi:.1} px over {BINS} bins, {disc_lobes} lobe(s)");
        assert!(
            hi < lo * 1.15,
            "a disc's lit radius must be constant to within rasterization, got \
             {lo:.1}..{hi:.1} px — the profile is measuring something other than \
             the silhouette"
        );
        assert_eq!(
            disc_lobes, 1,
            "a disc has no points; a count above 1 means this measurement reads noise"
        );

        for points in [5.0f32, 7.0, 9.0] {
            let Some(star) = capture_one_mark(STAR, points) else {
                return;
            };
            let profile = lit_radius_profile(&star, BINS);
            let lobes = angular_lobes(&profile);
            let lo = profile.iter().copied().fold(f32::INFINITY, f32::min);
            let hi = profile.iter().copied().fold(f32::NEG_INFINITY, f32::max);
            eprintln!(
                "star points={points}: lit radius {lo:.1}..{hi:.1} px, {lobes} angular maxima"
            );
            assert!(
                hi > lo * 1.5,
                "a star's tips must reach well past its valleys, got {lo:.1}..{hi:.1} px"
            );
            assert_eq!(
                lobes, points as usize,
                "a {points}-pointed star must show exactly {points} angular maxima, \
                 counted {lobes}"
            );
        }
    }

    /// **The default mark is byte-identical to the one this scene drew before it
    /// had a shape at all** (Plan 0070 Phase 1's first done-when), end to end
    /// through the preset path.
    ///
    /// Exact equality, not a tolerance: the `disc` arm is `length(p)` — the same
    /// expression the fragment shader held — so binding `shape = "0"` and binding
    /// nothing must produce the same bytes on the same adapter in the same run.
    /// That is the property every untouched golden baseline rests on, asserted
    /// here rather than inferred from the goldens passing.
    ///
    /// The third capture is the non-vacuity arm: a star through the same path
    /// must genuinely move the frame, or the first assertion would also pass on a
    /// `shape` binding that reached nothing.
    #[test]
    fn a_disc_shaped_swarm_is_byte_identical_to_the_unshaped_one() {
        use crate::dsp::AnalysisFrame;
        use crate::preset::Preset;
        use crate::render::context::RenderError;
        use crate::render::{HeadlessOptions, Renderer};

        const SIZE: u32 = 128;
        const FRAMES: u32 = 30;

        let mut renderer = match Renderer::new_headless(HeadlessOptions {
            width: SIZE,
            height: SIZE,
            prefer_software: true,
        }) {
            Ok(renderer) => renderer,
            Err(RenderError::RequestAdapter(_)) => {
                eprintln!("skipped: no GPU adapter on this runner (ADR-0016)");
                return;
            }
            Err(e) => panic!("headless renderer build failed: {e}"),
        };
        let frame = AnalysisFrame::default();
        let mut capture = |name: &str, extra: &str| {
            let toml = format!(
                "system = \"swarm\"\nname = \"{name}\"\n[params]\nforce = \"0.8\"\n\
                 spin = \"0.1\"\nbrightness = \"0.9\"\nsize = \"3.0\"\n{extra}"
            );
            let preset = Preset::from_toml_str(&toml).expect("the probe preset parses");
            renderer.set_presets(vec![preset]);
            renderer
                .capture_preset(name, &frame, FRAMES)
                .expect("capture the probe preset")
        };

        let unshaped = capture("unshaped", "");
        let disc = capture("disc", "shape = \"0\"\npoints = \"7\"\n");
        let star = capture("star", "shape = \"3\"\npoints = \"7\"\n");

        assert_eq!(
            unshaped.rgba, disc.rgba,
            "an explicit `shape = disc` must render byte-identically to no shape \
             binding at all — the disc arm is the length() it replaced"
        );
        let differing = star
            .rgba
            .chunks_exact(4)
            .zip(disc.rgba.chunks_exact(4))
            .filter(|(a, b)| a[..3] != b[..3])
            .count();
        eprintln!(
            "shaped swarm: {differing} of {} pixels differ between disc and star",
            (SIZE * SIZE) as usize
        );
        assert!(
            differing * 50 > (SIZE * SIZE) as usize,
            "a star must genuinely move the frame, or the equality above is a \
             statement about a binding that reached nothing: {differing} pixels"
        );
    }

    // -----------------------------------------------------------------------
    // The sprite seam does not punch holes in the backdrop (Plan 0051 Phase 1)
    // -----------------------------------------------------------------------

    /// The lit-backdrop fixture this guard captures three ways. Its `bg_bright`
    /// and `size` lines are **stripped and rewritten** per capture — one scene at
    /// three configurations — so the numbers are read back out of the file rather
    /// than restated here, and editing the fixture moves the test with it.
    const LIT_FIXTURE: &str = include_str!("../../../tests/fixtures/swarm_lit_backdrop.toml");

    /// The square capture size. Modest, because this reads back three whole float
    /// frames; and an exact multiple of the post chain's 256 px grid step, so the
    /// trails stage runs at the target size and its present is a 1:1 sample rather
    /// than a resample that would blur the property being asserted.
    const CAPTURE_SIZE: u32 = 256;

    /// Frames per capture. `force`/`spin`/`burst` are all 0 in the fixture, so
    /// this is long enough for the seeded initial velocities to damp out and the
    /// trail history to settle onto a static field.
    const CAPTURE_FRAMES: u32 = 40;

    /// A backdrop channel this bright counts as *present* for the non-vacuity arm
    /// below — well above the half-precision floor, well below the fixture's own
    /// `bg_bright`.
    const BACKDROP_PRESENT: f32 = 0.05;

    /// The value of a top-level `key = "<number>"` line in [`LIT_FIXTURE`], or
    /// `NaN` when it is absent. Used so the fixture stays the single statement of
    /// what this test captures.
    fn fixture_value(key: &str) -> f32 {
        LIT_FIXTURE
            .lines()
            .find_map(|line| {
                let rest = line.trim_start().strip_prefix(key)?;
                let rest = rest.trim_start().strip_prefix('=')?;
                rest.trim().trim_matches('"').parse::<f32>().ok()
            })
            .unwrap_or(f32::NAN)
    }

    /// Slack for half-precision rounding, the same shape `bloom.rs`'s guard uses.
    /// The composite is `Rgba16Float`, so a value of magnitude `m` is stored to
    /// roughly `m / 1024`, and the lit capture quantizes a different sum than the
    /// backdrop-only one does.
    ///
    /// It is slack, not a tolerance: the property below is **exact** in real
    /// arithmetic. Upstream of the tonemap the composite is a plain premultiplied
    /// OVER, so where the scene wrote nothing the backdrop must arrive unchanged.
    /// Measured on this fixture, the fixed shader's worst `|L - B|` is **0.0002**
    /// and the pre-fix one's is **0.3467** — the backdrop's own brightness,
    /// discarded outright — across 9594 channels. This sits ~1700x below the
    /// defect and ~20x above the noise.
    fn half_slack(value: f32) -> f32 {
        (4.0 / 1024.0) * value.abs().max(1.0)
    }

    /// **Where the swarm drew no light, the backdrop arrives intact** — the guard
    /// the scene→chain seam shipped without.
    ///
    /// `fs_main` used to return `vec4(in.color * g, 1.0)`: colour carried the
    /// radial falloff, alpha was a literal constant. With the alpha blend at
    /// `BlendComponent::OVER` and a source alpha of exactly 1, destination alpha
    /// saturated to 1 across every sprite's **square** quad — including the four
    /// corners outside the inscribed disc, about 21 % of each sprite, where the
    /// shader wrote nothing at all. The chain's resolve computes
    /// `src.rgb + backdrop * (1 - src.a)` (ADR-0055), so those corners discarded
    /// the backdrop and rendered as black rectangular notches, dozens per frame.
    /// See `gpu::ADDITIVE_LIGHT_SATURATING_COVERAGE`.
    ///
    /// # Why this reads the linear composite and not the capture
    ///
    /// Same reason `bloom.rs`'s guard does: the capture's bytes are downstream of
    /// the tonemap, which scales all three channels off the brightest one
    /// (ADR-0046), so adding a backdrop under a stroke changes every channel by
    /// design and no byte-level tolerance separates that from the defect.
    /// Upstream of the tonemap there is no confound — it is a plain premultiplied
    /// OVER — so the bound is **0** rather than a tolerance. That readback is
    /// `pub(crate)`, which is why this test lives here and not in `core/tests/`.
    ///
    /// # Why it needed writing at all
    ///
    /// Every swarm fixture and every golden baseline runs `bg_bright = 0`, where
    /// a black backdrop times any alpha is still black. The whole regression
    /// suite was blind to this by construction, and so was the contact sheet.
    /// That is verbatim the blind spot ADR-0055's first Negative bullet names —
    /// the third instance of it, after the fold (Plan 0045 Phase 2b) and the
    /// bloom recombine (Phase 4b), each of which got a guard of this shape.
    #[test]
    fn a_lit_backdrop_survives_where_the_swarm_drew_nothing() {
        use crate::dsp::AnalysisFrame;
        use crate::preset::Preset;
        use crate::render::capture;
        use crate::render::context::RenderError;
        use crate::render::{HeadlessOptions, Renderer};

        // --- Non-vacuity, before any GPU work: the fixture must still describe
        // the configuration this guard exists for. ---
        let backdrop = fixture_value("bg_bright");
        let sprite = fixture_value("size");
        let trails = fixture_value("trails");
        assert!(
            backdrop > 0.0,
            "swarm_lit_backdrop.toml no longer ships a lit backdrop (bg_bright = \
             {backdrop}); on black this whole comparison is black against black"
        );
        assert!(
            sprite > 0.0,
            "swarm_lit_backdrop.toml no longer draws sprites (size = {sprite})"
        );
        assert!(
            trails > 0.0,
            "swarm_lit_backdrop.toml no longer binds `trails` (= {trails}), so no \
             post stage is active. With an empty chain the scene draws straight \
             onto the backdrop and its additive colour cannot remove light — the \
             defect is unrepresentable and this test proves nothing"
        );

        /// The linear composite the tonemap is about to map, at a given backdrop
        /// brightness and sprite size.
        ///
        /// Builds and drops **one** renderer per call rather than holding three:
        /// a second live device in a binary is what the software adapter falls
        /// over on, and building GPU resources mid-run shifts what the trails
        /// stage resolves to on WARP.
        fn linear_composite(bg_bright: f32, size: f32) -> Option<Vec<f32>> {
            let mut renderer = match Renderer::new_headless(HeadlessOptions {
                width: CAPTURE_SIZE,
                height: CAPTURE_SIZE,
                prefer_software: true,
            }) {
                Ok(renderer) => renderer,
                Err(RenderError::RequestAdapter(_)) => {
                    eprintln!("skipped: no GPU adapter on this runner (ADR-0016)");
                    return None;
                }
                Err(e) => panic!("headless renderer build failed: {e}"),
            };
            // Both keys live in `[params]`, which is the fixture's last table, so
            // stripping them and appending the overrides keeps them in it.
            let base: String = LIT_FIXTURE
                .lines()
                .filter(|line| {
                    let line = line.trim_start();
                    !line.starts_with("bg_bright") && !line.starts_with("size")
                })
                .collect::<Vec<_>>()
                .join("\n");
            let toml = format!("{base}\nbg_bright = \"{bg_bright}\"\nsize = \"{size}\"\n");
            let preset = Preset::from_toml_str(&toml)
                .expect("the lit-backdrop swarm fixture parses with overrides");
            let name = preset.name.clone();
            renderer.set_presets(vec![preset]);

            // Every binding is a constant, so the analysis frame only has to be
            // well-formed — the swarm's `update` ignores it entirely.
            let frame = AnalysisFrame::default();
            renderer
                .capture_preset(&name, &frame, CAPTURE_FRAMES)
                .expect("capture the lit-backdrop swarm fixture");

            let device = renderer.ctx.device.clone();
            let queue = renderer.ctx.queue.clone();
            let src = renderer
                .tonemap
                .src_texture()
                .expect("the tonemap built its input while capturing")
                .clone();
            let (buffer, padded_bpr) =
                capture::create_linear_readback(&device, CAPTURE_SIZE, CAPTURE_SIZE);
            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("swarm-backdrop-readback"),
            });
            capture::record_copy(
                &mut encoder,
                &src,
                &buffer,
                padded_bpr,
                CAPTURE_SIZE,
                CAPTURE_SIZE,
            );
            queue.submit(std::iter::once(encoder.finish()));
            Some(
                capture::read_back_linear(&device, &buffer, CAPTURE_SIZE, CAPTURE_SIZE, padded_bpr)
                    .expect("read back the linear composite"),
            )
        }

        // `L`: the frame as shipped. `K`: the same scene over a black backdrop,
        // which is what "the scene wrote no light here" is read off. `B`: the
        // backdrop with the scene contributing nothing — zero-area sprite quads
        // rasterize no fragments, so the chain resolves fully transparent and
        // this is the backdrop alone, through the same pipeline as `L`.
        let Some(lit) = linear_composite(backdrop, sprite) else {
            return;
        };
        let Some(dark) = linear_composite(0.0, sprite) else {
            return;
        };
        let Some(backdrop_only) = linear_composite(backdrop, 0.0) else {
            return;
        };
        assert_eq!(dark.len(), lit.len(), "the captures differ in size");
        assert_eq!(
            dark.len(),
            backdrop_only.len(),
            "the captures differ in size"
        );

        let total = dark.len() / 4;
        let (mut untouched, mut drawn, mut over_backdrop) = (0usize, 0usize, 0usize);
        let (mut violations, mut worst) = (0usize, 0.0f32);
        for (pixel, texel) in dark.chunks_exact(4).enumerate() {
            if texel[0] != 0.0 || texel[1] != 0.0 || texel[2] != 0.0 {
                drawn += 1;
                continue; // the scene put light here; the property says nothing
            }
            untouched += 1;
            let base = pixel * 4;
            if backdrop_only[base..base + 3]
                .iter()
                .any(|&c| c > BACKDROP_PRESENT)
            {
                over_backdrop += 1;
            }
            for channel in 0..3 {
                let l = lit[base + channel];
                let b = backdrop_only[base + channel];
                let diff = (l - b).abs();
                if diff > worst {
                    worst = diff;
                }
                if diff > half_slack(b) {
                    violations += 1;
                }
            }
        }
        eprintln!(
            "swarm lit backdrop at {CAPTURE_SIZE}x{CAPTURE_SIZE}: {untouched} of \
             {total} pixels untouched by the scene ({over_backdrop} of those over \
             a lit backdrop), {drawn} lit by it; worst |L - B| {worst:.4}"
        );

        // --- Non-vacuity: the region the property speaks about is a substantial
        // part of the frame, the scene genuinely drew into the rest, and the
        // backdrop genuinely reached the frame underneath. A fixture edit that
        // quietly empties any of the three shows up here rather than passing. ---
        assert!(
            untouched * 4 > total,
            "only {untouched} of {total} pixels are untouched by the scene — the \
             fixture has filled the frame and the property covers almost nothing"
        );
        assert!(
            drawn * 20 > total,
            "only {drawn} of {total} pixels carry any scene light — the fixture \
             has stopped drawing, so the sprite corners this guards are not in \
             the frame"
        );
        assert!(
            over_backdrop * 2 > untouched,
            "only {over_backdrop} of the {untouched} untouched pixels sit over a \
             backdrop brighter than {BACKDROP_PRESENT} — comparing black against \
             black, which any alpha would pass"
        );

        // --- The property. ---
        assert_eq!(
            violations, 0,
            "{violations} channels differ between the lit frame and the backdrop \
             alone at pixels where the scene wrote NO light (worst {worst:.4}). \
             Upstream of the tonemap this is a plain premultiplied OVER, so where \
             nothing was drawn the backdrop must arrive intact — a difference \
             here is a sprite emitting coverage it does not have, holding the \
             backdrop out of pixels it never painted"
        );
    }
}
