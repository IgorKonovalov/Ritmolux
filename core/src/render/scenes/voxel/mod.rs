//! The voxel system (ADR-0268): a 3D cellular automaton in a cube of cells,
//! drawn as a glowing, absorbing volume seen through the shared orbit camera.
//!
//! # The state
//!
//! A ping-pong pair of 3D textures of side `grid`, one `u32` a cell packing its
//! state and its age (`shader::STEP_SHADER`). One generation is one compute
//! dispatch that counts each cell's live neighbours and applies the rule
//! ([`rules`]). The pair is built when a preset is configured, never on a frame.
//!
//! `[voxel] grid` is content, not a resolution: a structure is a fixed number of
//! cells, so the grid decides how large every structure looks. A grid past the
//! scene's cap is clamped and announced from `configure` as a
//! [`CapOverflow`](super::CapOverflow), never silently reduced.
//!
//! # Generations, not frames
//!
//! `step_rate` is in generations per second, integrated over the injected `dt`
//! by the shared [`GenerationClock`](common::GenerationClock), so the automaton
//! advances by the same count in the same wall time at any refresh rate.
//!
//! # Every cell is drawn from the seed
//!
//! The field is seeded from an integer hash of each cell's coordinates and a
//! seed derived from the preset's pinned salt (ADR-0051) — never from a clock —
//! and each step is integer counting, so a volume is a pure function of its
//! config and the sequence of bound values and `dt`s it was driven with.
//!
//! # The present
//!
//! A fullscreen march: each pixel's ray is walked cell by cell through the cube
//! `[-1, 1]^3` under emission-absorption (`shader::MARCH_SHADER`). The ray is
//! recovered from the camera's world-to-clip rows, whose aspect is the render
//! target's (ADR-0037). `focus` and `aperture` are inert here: the circle of
//! confusion is per primitive (ADR-0257), and a march has no primitives.

// Hot-path panic-denial pragma (Plan 0002 Phase 2, extended to scenes by Plan
// 0003 Phase 0). Encodes its passes every displayed frame.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

pub mod rules;
mod shader;

use super::common::{self, PaletteParams, PanParams};
use super::lines::GeneratorConfig;
use super::{ParamGroup, ParamKind, ParamSpec, Scene, default_of};
use crate::dsp::AnalysisFrame;
use crate::render::camera::{self, CameraParams};
use crate::render::gpu;
use crate::render::palette::{self, Palette};
pub use rules::{DEFAULT_RULE, Neighbourhood, RosterRule, Rule, RuleList};

/// The grid an absent `[voxel] grid` means.
pub const DEFAULT_GRID: u32 = 64;
/// The smallest grid the loader accepts. Below it a seeded ball is a handful of
/// cells and every rule's structure is cut by the faces.
pub const MIN_GRID: u32 = 16;
/// The largest grid the loader accepts and any tier runs: 2.1 M cells, two
/// 4-byte texels each across the ping-pong pair, 16.8 MB.
pub const MAX_GRID: u32 = 128;

/// The fastest `step_rate` the clock integrates, in generations per second.
/// Well past the declared range's top, so it bounds a runaway binding rather
/// than an author.
pub const MAX_STEP_RATE: f32 = 120.0;

/// Where the age field stops counting: generations since a cell last changed
/// state, saturating here.
const AGE_CAP: u32 = 1023;
/// The generations over which a live cell's colour slides the whole of
/// `age_tint` along the palette.
const AGE_SPAN: f32 = 48.0;

/// The texel every cell is stored in: the state in the low byte and the age
/// above it, an integer format a compute pass can write.
const STATE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R32Uint;

/// Each side of the step pass's workgroup, in cells — `@workgroup_size(4, 4,
/// 4)` in `shader::STEP_SHADER`.
const WORKGROUP: u32 = 4;

/// The radius of the sphere bounding the cube `[-1, 1]^3`: the volume the
/// camera's `fog` is measured across.
const CUBE_RADIUS: f32 = 1.732_050_8;

/// Mixed into the preset's salt before it seeds the field, so a salt of `0`
/// still hashes to a field rather than to the hash's fixed point. The ASCII
/// bytes of "VOXL"; opaque, since changing it moves every seeded field.
const FIELD_SEED_MIX: u32 = 0x564F_584C;

/// `[voxel]` — the structural configuration, fixed while the preset is loaded
/// and delivered through `Scene::configure`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VoxelConfig {
    /// Cells per side of the cube, validated at load into
    /// [`MIN_GRID`]`..=`[`MAX_GRID`].
    pub grid: u32,
    /// The rules a preset lists, compiled.
    pub rules: RuleList,
    /// The seeded ball's radius, as a fraction of the cube's half-side.
    pub seed_radius: f32,
    /// The probability a cell inside the seeded ball is live.
    pub seed_fill: f32,
    /// `true`: the cube is a 3-torus. `false`: every cell past a face is dead.
    pub wrap: bool,
    /// The preset's pinned salt (ADR-0051), which every seeded cell is drawn
    /// from.
    pub salt: u32,
}

impl Default for VoxelConfig {
    fn default() -> Self {
        let (seed_radius, seed_fill) = rules::DEFAULT_RULE.seed();
        Self {
            grid: DEFAULT_GRID,
            rules: RuleList::default(),
            seed_radius,
            seed_fill,
            // Dead outside: a structure crossing a face of a torus reappears on
            // the opposite one, which an orbiting camera reads as a cut.
            wrap: false,
            salt: 0,
        }
    }
}

const DEFAULT_STEP_RATE: f32 = default_of(PARAMS, "step_rate");
const DEFAULT_DENSITY: f32 = default_of(PARAMS, "density");
const DEFAULT_TRAIL: f32 = default_of(PARAMS, "trail");
const DEFAULT_AGE_TINT: f32 = default_of(PARAMS, "age_tint");
const DEFAULT_HUE: f32 = default_of(PARAMS, "hue");
const DEFAULT_HUE_SPREAD: f32 = default_of(PARAMS, "hue_spread");
const DEFAULT_BRIGHTNESS: f32 = default_of(PARAMS, "brightness");
const DEFAULT_ZOOM: f32 = default_of(PARAMS, "zoom");

// The shared camera block (ADR-0258), re-declared with this system's own doc
// lines where they differ: the editor schema keys a row by the whole
// declaration. Default, range and kind stay the shared block's.
const FOCUS: ParamSpec = ParamSpec {
    doc: "Inert on voxel: depth of field blurs each drawn primitive, and the march draws none.",
    ..camera::FOCUS
};
const APERTURE: ParamSpec = ParamSpec {
    doc: "Inert on voxel: depth of field blurs each drawn primitive, and the march draws none.",
    ..camera::APERTURE
};
const FOG: ParamSpec = ParamSpec {
    doc: "Fades the volume toward black with depth: at 1 the cube's farthest point is black and its nearest keeps its light. 0 is off.",
    ..camera::FOG
};

/// The parameter names this scene consumes — the vocabulary a preset binding is
/// checked against at load (ADR-0020). **Keep in sync with `set_param` below**;
/// `declared_params_match_set_param` in `core/tests/suite/preset.rs` fails if the
/// two drift.
pub const PARAMS: &[ParamSpec] = &[
    ParamSpec {
        name: "step_rate",
        default: 8.0,
        range: Some([0.0, 30.0]),
        doc: "How many generations the automaton runs per second, whatever the frame rate; 0 \
              freezes it.",
        kind: ParamKind::Modal,
        group: ParamGroup::Motion,
        main: true,
    },
    ParamSpec {
        name: "density",
        default: 1.0,
        range: Some([0.0, 8.0]),
        doc: "How strongly the volume absorbs the light behind it, per unit of the cube's \
              width; 0 is pure additive glow with no front or back.",
        kind: ParamKind::Modal,
        group: ParamGroup::Light,
        main: true,
    },
    ParamSpec {
        name: "trail",
        default: 0.6,
        range: Some([0.0, 1.0]),
        doc: "How much of its light a decaying cell keeps at each stage of its decay; 0 draws \
              only the live cells.",
        kind: ParamKind::Modal,
        group: ParamGroup::Light,
        main: false,
    },
    ParamSpec {
        name: "age_tint",
        default: 0.3,
        range: Some([0.0, 1.0]),
        doc: "How far along the palette a cell's colour travels as it ages; a decaying cell \
              takes the far end.",
        kind: ParamKind::Modal,
        group: ParamGroup::Colour,
        main: false,
    },
    common::hue(0.0),
    ParamSpec {
        name: "hue_spread",
        default: 0.3,
        range: Some([0.0, 1.0]),
        doc: "How far along the palette the colour travels from the cube's centre to its \
              corners.",
        kind: ParamKind::Modal,
        group: ParamGroup::Colour,
        main: false,
    },
    common::brightness(1.0),
    camera::YAW,
    camera::PITCH,
    camera::DISTANCE,
    camera::FOV,
    FOCUS,
    APERTURE,
    FOG,
    common::SATURATION,
    common::PALETTE_MIX,
    common::PALETTE_STEPS,
    common::zoom(1.0),
    common::PAN_X,
    common::PAN_Y,
];

/// The seed the whole field is hashed from, for a preset whose salt is `salt`.
pub(crate) fn field_seed(salt: u32) -> u32 {
    salt ^ FIELD_SEED_MIX
}

/// A fill probability as the step shader compares it: the fraction of the
/// 24-bit hash range under which a cell is seeded live.
pub(crate) fn live_threshold(fill: f32) -> u32 {
    (fill.clamp(0.0, 1.0) * 16_777_216.0) as u32
}

/// The seeded ball's radius squared in half-cells², as the step shader compares
/// a cell's offset from the centre: `seed_radius` of the half-side is
/// `seed_radius * grid` half-cells.
pub(crate) fn seed_radius_sq(seed_radius: f32, grid: u32) -> u32 {
    let r = seed_radius.clamp(0.0, 2.0) * grid as f32;
    (r * r) as u32
}

/// A bound `step_rate` as the clock integrates it: clamped into
/// `0..=`[`MAX_STEP_RATE`], the declared default where it is not finite.
pub(crate) fn applied_step_rate(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, MAX_STEP_RATE)
    } else {
        DEFAULT_STEP_RATE
    }
}

/// `value` where it is finite, `fallback` where it is not.
fn finite_or(value: f32, fallback: f32) -> f32 {
    if value.is_finite() { value } else { fallback }
}

/// `config` with its grid held to `cap`, and the clamp to report if that moved
/// it. A grid is structural, so this runs once, at `configure`.
pub(crate) fn grid_clamp(
    config: VoxelConfig,
    cap: u32,
) -> (VoxelConfig, Option<super::CapOverflow>) {
    if config.grid <= cap {
        return (config, None);
    }
    let overflow = super::CapOverflow {
        dropped: (config.grid - cap) as usize,
        context: super::OverflowContext::Grid(config.grid),
        cap: cap as usize,
    };
    (
        VoxelConfig {
            grid: cap,
            ..config
        },
        Some(overflow),
    )
}

/// The three non-zero rows of `view`'s world-to-clip matrix — screen x, screen
/// y and view depth — each as `[x, y, z, translation]`. The matrix is stored
/// column-major, and its third row is zero (no depth attachment, ADR-0257).
fn rows(view: &camera::CameraView) -> [[f32; 4]; 3] {
    let m = view.view_proj;
    let row = |r: usize| {
        let at = |c: usize| m.get(c).and_then(|col| col.get(r)).copied().unwrap_or(0.0);
        [at(0), at(1), at(2), at(3)]
    };
    [row(0), row(1), row(3)]
}

/// The eye: the one point every non-zero row of `view` maps to zero, solved by
/// Cramer's rule. The three rows are the camera's right, up and view axes,
/// scaled and sheared by the lens and the pan, so they are never coplanar.
fn eye(view: &camera::CameraView) -> [f32; 3] {
    let [r0, r1, r3] = rows(view);
    let (a, b, c) = (xyz(r0), xyz(r1), xyz(r3));
    let rhs = [-r0[3], -r1[3], -r3[3]];
    let det = dot(a, cross(b, c));
    if det.abs() < 1e-12 {
        return [0.0, 0.0, view.distance];
    }
    // Column `i` of the system replaced by the right-hand side.
    let solve = |i: usize| {
        let swap = |row: [f32; 3], k: usize| {
            let mut r = row;
            if let Some(slot) = r.get_mut(i) {
                *slot = rhs.get(k).copied().unwrap_or(0.0);
            }
            r
        };
        dot(swap(a, 0), cross(swap(b, 1), swap(c, 2))) / det
    };
    [solve(0), solve(1), solve(2)]
}

fn xyz(v: [f32; 4]) -> [f32; 3] {
    [v[0], v[1], v[2]]
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// The one uniform the seed and step passes read, written once a frame.
///
/// **One buffer for both, and that is load-bearing**, for `cellular`'s reason:
/// which pass is running is a constant compiled into its pipeline, never a
/// field here, so every pass is bound to identical resources and an adapter
/// cannot hand one pass another's uniform (ADR-0058).
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct StepParams {
    /// x: grid, y: wrap (1 torus), z: the field's seed, w: live threshold.
    a: [u32; 4],
    /// x: birth mask, y: survive mask, z: states, w: neighbourhood.
    b: [u32; 4],
    /// x: the seed ball's radius squared in half-cells²; yzw unused.
    c: [u32; 4],
}

/// The march's uniform, written once a frame.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct MarchParams {
    r0: [f32; 4],
    r1: [f32; 4],
    r3: [f32; 4],
    /// xyz: the eye; w: grid.
    eye: [f32; 4],
    /// x: hue, y: brightness, z: saturation, w: palette_mix.
    a: [f32; 4],
    /// x: palette_steps, y: occlude, z: density, w: trail.
    b: [f32; 4],
    /// x: age_tint, y: hue_spread, z: fog, w: unused.
    c: [f32; 4],
    /// x: the cube's nearest view depth, y: its depth span; zw unused.
    d: [f32; 4],
}

/// What one pass of the step shader does: its `MODE` constant, compiled in.
#[derive(Clone, Copy)]
enum StepPass {
    /// Advance every cell one generation by the rule.
    Step,
    /// Hash every cell from the field's seed.
    Seed,
}

impl StepPass {
    fn mode(self) -> u32 {
        match self {
            StepPass::Step => 0,
            StepPass::Seed => 1,
        }
    }

    fn label(self) -> &'static str {
        match self {
            StepPass::Step => "voxel-step",
            StepPass::Seed => "voxel-seed",
        }
    }

    /// This pass's shader: its constants, the hash and the body. Built at
    /// construction only.
    fn source(self) -> String {
        format!(
            "const MODE: u32 = {}u;\nconst AGE_CAP: u32 = {}u;\n{}{}{}",
            self.mode(),
            AGE_CAP,
            gpu::HASH_WGSL,
            common::CELL_HASH_WGSL,
            shader::STEP_SHADER
        )
    }
}

/// One of the pair of state textures, with the two views the passes take of it.
struct StateTexture {
    /// Held beside its views; read back by the probes.
    #[cfg_attr(not(test), allow(dead_code))]
    texture: wgpu::Texture,
    /// Sampled by `textureLoad`, as the step's source and the march's field.
    read: wgpu::TextureView,
    /// Written by the step pass.
    write: wgpu::TextureView,
}

impl StateTexture {
    fn new(device: &wgpu::Device, grid: u32, label: &str) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: grid,
                height: grid,
                depth_or_array_layers: grid,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D3,
            format: STATE_FORMAT,
            // `COPY_SRC` only so a probe can read the cells back.
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let read = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let write = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Self {
            texture,
            read,
            write,
        }
    }
}

/// The GPU-side state, built when a preset is configured and rebuilt when one
/// asks for a different grid.
struct Resources {
    /// The grid these textures were built for.
    grid: u32,
    /// The pair, kept so the bind groups' views stay valid.
    #[cfg_attr(not(test), allow(dead_code))]
    a: StateTexture,
    #[cfg_attr(not(test), allow(dead_code))]
    b: StateTexture,
    /// Whether `a` holds the current generation.
    reading_a: bool,
    seed_pipeline: wgpu::ComputePipeline,
    step_pipeline: wgpu::ComputePipeline,
    step_uniform: wgpu::Buffer,
    /// Read A write B, and read B write A.
    step_from_a: wgpu::BindGroup,
    step_from_b: wgpu::BindGroup,
    march_pipeline: wgpu::RenderPipeline,
    march_uniform: wgpu::Buffer,
    march_from_a: wgpu::BindGroup,
    march_from_b: wgpu::BindGroup,
    /// The shared gradient LUT pair (ADR-0021). A fresh pair is dirty, so a
    /// (re)build uploads on its first frame.
    luts: palette::LutPair,
}

impl Resources {
    fn build(device: &wgpu::Device, surface_format: wgpu::TextureFormat, grid: u32) -> Self {
        let a = StateTexture::new(device, grid, "voxel-state-a");
        let b = StateTexture::new(device, grid, "voxel-state-b");
        let step_uniform = gpu::uniform_buffer(
            device,
            "voxel-step-params",
            std::mem::size_of::<StepParams>(),
        );
        let march_uniform = gpu::uniform_buffer(
            device,
            "voxel-march-params",
            std::mem::size_of::<MarchParams>(),
        );

        // `[Texture, StorageTexture, Uniform]`, all compute-visible: the source
        // generation, the one being written, the uniform.
        let step_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("voxel-step-layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Uint,
                        view_dimension: wgpu::TextureViewDimension::D3,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: STATE_FORMAT,
                        view_dimension: wgpu::TextureViewDimension::D3,
                    },
                    count: None,
                },
                gpu::uniform(2, wgpu::ShaderStages::COMPUTE),
            ],
        });
        let step_bind = |src: &StateTexture, dst: &StateTexture| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("voxel-step-bg"),
                layout: &step_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&src.read),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&dst.write),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: step_uniform.as_entire_binding(),
                    },
                ],
            })
        };
        let step_from_a = step_bind(&a, &b);
        let step_from_b = step_bind(&b, &a);
        let step_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("voxel-step-pipeline-layout"),
            bind_group_layouts: &[Some(&step_layout)],
            immediate_size: 0,
        });
        let compute = |pass: StepPass| {
            let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some(pass.label()),
                source: wgpu::ShaderSource::Wgsl(pass.source().into()),
            });
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(pass.label()),
                layout: Some(&step_pipeline_layout),
                module: &module,
                entry_point: Some("main"),
                compilation_options: Default::default(),
                cache: None,
            })
        };
        let seed_pipeline = compute(StepPass::Seed);
        let step_pipeline = compute(StepPass::Step);

        let luts = palette::LutPair::new(device, "voxel");
        // `[Uniform+size, Texture, Texture, Texture, Sampler]`: the uniform
        // first and sized, so the shape is not `cellular-present-layout`'s
        // field-LUTs-sampler-uniform (ADR-0058).
        let march_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("voxel-march-layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(
                            std::mem::size_of::<MarchParams>() as u64
                        ),
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Uint,
                        view_dimension: wgpu::TextureViewDimension::D3,
                        multisampled: false,
                    },
                    count: None,
                },
                gpu::texture(2, true),
                gpu::texture(3, true),
                gpu::sampler(4),
            ],
        });
        let march_bind = |field: &StateTexture| {
            let [lut_a, lut_b, lut_sampler] = luts.bind_entries(2, 3, 4);
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("voxel-march-bg"),
                layout: &march_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: march_uniform.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&field.read),
                    },
                    lut_a,
                    lut_b,
                    lut_sampler,
                ],
            })
        };
        let march_from_a = march_bind(&a);
        let march_from_b = march_bind(&b);
        let march_shader = gpu::fullscreen_shader(
            device,
            "voxel-march",
            gpu::FULLSCREEN_VS_NDC,
            &format!(
                "const MAX_MARCH: i32 = {};\nconst AGE_SPAN: f32 = {:?};\n{}",
                3 * MAX_GRID,
                AGE_SPAN,
                shader::MARCH_SHADER
            ),
        );
        let march_pipeline = gpu::fullscreen_pipeline(
            device,
            &march_shader,
            &[&march_layout],
            surface_format,
            // Premultiplied OVER the backdrop (ADR-0201): the march writes its
            // light and the coverage its absorption computed.
            wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING,
            "voxel-march",
        );

        Self {
            grid,
            a,
            b,
            reading_a: true,
            seed_pipeline,
            step_pipeline,
            step_uniform,
            step_from_a,
            step_from_b,
            march_pipeline,
            march_uniform,
            march_from_a,
            march_from_b,
            luts,
        }
    }

    /// The texture holding the current generation.
    #[cfg(test)]
    fn current(&self) -> &wgpu::Texture {
        if self.reading_a {
            &self.a.texture
        } else {
            &self.b.texture
        }
    }

    /// Encode one `pass` of the step shader: read the current generation,
    /// write the other texture, swap. Its own compute pass, so the write is
    /// complete before the next pass samples it.
    fn encode_step(&mut self, encoder: &mut wgpu::CommandEncoder, pass: StepPass) {
        let pipeline = match pass {
            StepPass::Step => &self.step_pipeline,
            StepPass::Seed => &self.seed_pipeline,
        };
        let groups = self.grid.div_ceil(WORKGROUP);
        {
            let mut cpass = gpu::compute_pass(encoder, pass.label());
            cpass.set_pipeline(pipeline);
            cpass.set_bind_group(
                0,
                if self.reading_a {
                    &self.step_from_a
                } else {
                    &self.step_from_b
                },
                &[],
            );
            cpass.dispatch_workgroups(groups, groups, groups);
        }
        self.reading_a = !self.reading_a;
    }
}

/// A 3D automaton in a cube, marched as an emitting, absorbing volume.
pub struct VoxelScene {
    /// Cloned device handle that builds [`Resources`] at `configure`.
    device: wgpu::Device,
    surface_format: wgpu::TextureFormat,
    res: Option<Resources>,
    /// The `[voxel]` table of the preset last configured.
    config: VoxelConfig,
    /// The largest grid a preset runs on.
    grid_cap: u32,
    /// Whether the next `render` seeds the field before stepping it — set by
    /// every `configure`.
    needs_seed: bool,
    clock: common::GenerationClock,
    /// This frame's `dt`, stored by `advance` and integrated in `update`, where
    /// this frame's `step_rate` has landed.
    dt: f32,
    /// Generations `update` scheduled for the next `render` to encode.
    pending_generations: u32,
    step_rate: f32,
    density: f32,
    trail: f32,
    age_tint: f32,
    hue_spread: f32,
    zoom: f32,
    colour: PaletteParams,
    pan: PanParams,
    camera: CameraParams,
    /// How much of this scene's coverage the backdrop resolves against
    /// (ADR-0085), set by the renderer every frame.
    occlude: f32,
    /// The active baked palette, held because `set_palette` can arrive before
    /// the resources exist.
    palette: Palette,
}

impl VoxelScene {
    /// The CPU-side state, holding a preset's grid to `grid_cap`. GPU resources
    /// are built at the first `configure`.
    pub fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat, grid_cap: u32) -> Self {
        Self {
            device: device.clone(),
            surface_format,
            res: None,
            config: VoxelConfig::default(),
            grid_cap: grid_cap.clamp(MIN_GRID, MAX_GRID),
            needs_seed: true,
            clock: common::GenerationClock::default(),
            dt: 0.0,
            pending_generations: 0,
            step_rate: DEFAULT_STEP_RATE,
            density: DEFAULT_DENSITY,
            trail: DEFAULT_TRAIL,
            age_tint: DEFAULT_AGE_TINT,
            hue_spread: DEFAULT_HUE_SPREAD,
            zoom: DEFAULT_ZOOM,
            colour: PaletteParams::new(DEFAULT_HUE, DEFAULT_BRIGHTNESS),
            pan: PanParams::default(),
            camera: CameraParams::default(),
            occlude: crate::render::post::DEFAULT_OCCLUDE,
            palette: Palette::default_spectrum(),
        }
    }

    /// This frame's step uniform.
    fn step_params(&self) -> StepParams {
        let rule = self.config.rules.first();
        StepParams {
            a: [
                self.config.grid,
                u32::from(self.config.wrap),
                field_seed(self.config.salt),
                live_threshold(self.config.seed_fill),
            ],
            b: [
                rule.birth,
                rule.survive,
                rule.states,
                rule.neighbourhood.index(),
            ],
            c: [
                seed_radius_sq(self.config.seed_radius, self.config.grid),
                0,
                0,
                0,
            ],
        }
    }

    /// This frame's march uniform, for a render target of `aspect`.
    fn march_params(&self, aspect: f32) -> MarchParams {
        let zoom = if self.zoom.is_finite() && self.zoom > 1e-3 {
            self.zoom
        } else {
            DEFAULT_ZOOM
        };
        let view = self.camera.camera().view(
            aspect,
            zoom,
            [finite_or(self.pan.x, 0.0), finite_or(self.pan.y, 0.0)],
        );
        let [r0, r1, r3] = rows(&view);
        let [ex, ey, ez] = eye(&view);
        MarchParams {
            r0,
            r1,
            r3,
            eye: [ex, ey, ez, self.config.grid as f32],
            a: [
                self.colour.hue,
                self.colour.brightness,
                self.colour.saturation,
                self.colour.mix,
            ],
            b: [
                palette::band_steps(self.colour.steps),
                self.occlude,
                finite_or(self.density, DEFAULT_DENSITY).max(0.0),
                finite_or(self.trail, DEFAULT_TRAIL).clamp(0.0, 1.0),
            ],
            c: [
                finite_or(self.age_tint, DEFAULT_AGE_TINT),
                finite_or(self.hue_spread, DEFAULT_HUE_SPREAD),
                finite_or(self.camera.fog, 0.0).clamp(0.0, 1.0),
                0.0,
            ],
            d: [view.distance - CUBE_RADIUS, 2.0 * CUBE_RADIUS, 0.0, 0.0],
        }
    }
}

impl Scene for VoxelScene {
    fn name(&self) -> &'static str {
        "voxel"
    }

    fn advance(&mut self, dt: f32) {
        self.dt = dt;
    }

    fn set_occlude(&mut self, occlude: f32) {
        self.occlude = occlude;
    }

    fn set_palette(&mut self, palette: &Palette) {
        self.palette = palette.clone();
        if let Some(res) = self.res.as_mut() {
            res.luts.set(palette);
        }
    }

    fn configure(&mut self, cfg: &GeneratorConfig) -> Option<super::CapOverflow> {
        let GeneratorConfig::Voxel(config) = cfg else {
            return None;
        };
        // A switch starts the incoming preset from its own seed. The grid is
        // held to the cap here, once, and the clamp is returned for the
        // renderer to announce with the preset. The state pair is built here,
        // off the frame, and only when the grid changes.
        let (config, overflow) = grid_clamp(*config, self.grid_cap);
        self.config = config;
        if self.res.as_ref().is_none_or(|res| res.grid != config.grid) {
            let mut built = Resources::build(&self.device, self.surface_format, config.grid);
            built.luts.set(&self.palette);
            self.res = Some(built);
        }
        self.needs_seed = true;
        self.clock = common::GenerationClock::default();
        self.pending_generations = 0;
        overflow
    }

    fn reset_params(&mut self) {
        self.step_rate = DEFAULT_STEP_RATE;
        self.density = DEFAULT_DENSITY;
        self.trail = DEFAULT_TRAIL;
        self.age_tint = DEFAULT_AGE_TINT;
        self.hue_spread = DEFAULT_HUE_SPREAD;
        self.zoom = DEFAULT_ZOOM;
        self.colour.reset();
        self.pan.reset();
        self.camera.reset();
    }

    fn set_param(&mut self, name: &str, value: f32) {
        // The shared param blocks first, this scene's own names after
        // (`scenes::common`).
        if self.colour.set(name, value) || self.pan.set(name, value) {
            return;
        }
        match name {
            // The camera block's names, matched here rather than delegated to
            // `CameraParams::set`: the march declares all of it but `solid`,
            // which sorts strokes and a march has none, and a delegation would
            // answer that undeclared name too. `focus` and `aperture` are
            // stored and never read.
            "yaw" => self.camera.yaw = value,
            "pitch" => self.camera.pitch = value,
            "distance" => self.camera.distance = value,
            "fov" => self.camera.fov = value,
            "focus" => self.camera.focus = value,
            "aperture" => self.camera.aperture = value,
            "fog" => self.camera.fog = value,
            "step_rate" => self.step_rate = value,
            "density" => self.density = value,
            "trail" => self.trail = value,
            "age_tint" => self.age_tint = value,
            "hue_spread" => self.hue_spread = value,
            "zoom" => self.zoom = value,
            _ => {}
        }
    }

    fn update(&mut self, _frame: &AnalysisFrame) {
        self.pending_generations = self
            .clock
            .advance(applied_step_rate(self.step_rate), self.dt);
    }

    fn render(
        &mut self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        aspect: f32,
    ) {
        // The aspect is the render target's, handed in here (ADR-0037).
        let march = self.march_params(aspect);
        let step = self.step_params();
        let seed = std::mem::replace(&mut self.needs_seed, false);
        let generations = std::mem::take(&mut self.pending_generations);
        let Some(res) = self.res.as_mut() else {
            return;
        };
        res.luts.flush(queue);
        queue.write_buffer(&res.march_uniform, 0, bytemuck::bytes_of(&march));
        // One write serves every pass below: the seed, then the generations.
        if seed || generations > 0 {
            queue.write_buffer(&res.step_uniform, 0, bytemuck::bytes_of(&step));
        }
        if seed {
            res.encode_step(encoder, StepPass::Seed);
        }
        for _ in 0..generations {
            res.encode_step(encoder, StepPass::Step);
        }

        // Load over the engine backdrop (ADR-0018): a ray through empty cells
        // writes no light and no coverage.
        let mut pass = gpu::color_pass(encoder, "voxel-march-pass", view, wgpu::LoadOp::Load);
        pass.set_pipeline(&res.march_pipeline);
        pass.set_bind_group(
            0,
            if res.reading_a {
                &res.march_from_a
            } else {
                &res.march_from_b
            },
            &[],
        );
        pass.draw(0..3, 0..1);
    }
}

#[cfg(test)]
mod tests;
