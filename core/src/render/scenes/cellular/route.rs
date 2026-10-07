//! The maze route (ADR-0266): breadth-first distance fields over the open cells
//! of the grid, relaxed on the GPU over many passes against a frozen snapshot.
//!
//! # The graph
//!
//! An open cell is a dead cell of the state channel; a live cell is a wall. Open
//! cells join their four orthogonal neighbours, across the seam when the grid
//! wraps. `cyclic` has no dead state, so the route is never encoded there.
//!
//! # Who decides what
//!
//! The CPU encodes passes and hands each one its event's scene time; it never
//! reads the route's state back. One small storage buffer of control words is
//! the state, and the one-invocation control pass is the only writer of it:
//!
//! - after every generation, a `compare` pass counts the cells whose open bit
//!   changed and records the mask, and the control pass reads the count;
//! - on every route pass the [`RouteClock`] owes, a `work` pass runs whatever
//!   action the control words name — freeze a snapshot, find a cell by key, or
//!   relax the distance field once — dispatched **indirectly** over the count
//!   of tiles the control pass wrote, so an idle route dispatches nothing; and
//!   the control pass reads what it counted and names the next action.
//!
//! # The timeline
//!
//! Generations and route passes are owed by two clocks over the same injected
//! `dt`, and a frame encodes both **in the order of their times**: a
//! [`Timeline`] steps the two schedules and yields the earlier event each time,
//! a generation first on a tie, so the interleaving — and with it every field
//! the route reaches — is the same at any frame rate. It is generated, never
//! collected, so the render path allocates nothing for it.
//!
//! # What lives where on the GPU
//!
//! `cells` holds six regions of one `u32` a cell: the last compared mask, the
//! snapshot, the ping-pong pair of distances, the kept field and the route.
//! `counts` holds the atomics a grid pass accumulates and the control pass
//! clears. `control` holds the words below, the first three being the work
//! pass's indirect dispatch.

// Hot-path panic-denial pragma (Plan 0002 Phase 2): the encode below runs every
// displayed frame the route is on.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

use super::{MAX_GENERATIONS_PER_FRAME, ReadPair, WHOLE_SLACK, shader};
use crate::render::feedback::PingPongField;
use crate::render::gpu;

/// Relax passes owed per second of injected `dt`.
pub(crate) const ROUTE_RATE: f32 = 240.0;

/// The most route passes one frame encodes. Past it a backlog is dropped, as
/// [`MAX_GENERATIONS_PER_FRAME`] drops generations, so a stall slows the search
/// rather than queueing passes without bound.
pub(crate) const MAX_ROUTE_PASSES_PER_FRAME: u32 = 64;

/// The sentinel distance: a wall, or an open cell the sweep has not reached.
pub(crate) const INF: u32 = u32::MAX;

/// Cells a side of one relax workgroup's tile.
pub(crate) const TILE: u32 = 16;

/// How many times a relax pass applies the rule inside its tile before writing
/// it out. The halo is held at the read half's values throughout, so a pass is
/// still a pure function of the one before it; more iterations only carry the
/// front further along a corridor that winds inside one tile.
pub(crate) const LOCAL_ITERATIONS: u32 = 32;

/// The words of the control buffer.
pub(crate) const C_ARGS: u32 = 0;
pub(crate) const C_ACTION: u32 = 3;
pub(crate) const C_SEARCH: u32 = 4;
pub(crate) const C_PARITY: u32 = 5;
pub(crate) const C_FRESH: u32 = 6;
pub(crate) const C_SOURCE: u32 = 7;
pub(crate) const C_KEY: u32 = 8;
pub(crate) const C_KEY_KIND: u32 = 9;
pub(crate) const C_D_MAX: u32 = 10;
pub(crate) const C_SWEEP_PASSES: u32 = 11;
pub(crate) const C_CONVERGED: u32 = 12;
pub(crate) const C_RELAX_PASSES: u32 = 13;
pub(crate) const C_GENERATIONS: u32 = 14;
pub(crate) const C_LAST_MOVED: u32 = 15;
pub(crate) const C_LAST_OPEN: u32 = 16;
pub(crate) const C_LAST_CHANGED: u32 = 17;
/// How many words the control buffer holds.
pub(crate) const CONTROL_WORDS: u32 = 32;

/// The counters a grid pass accumulates.
pub(crate) const K_MOVED: u32 = 0;
pub(crate) const K_OPEN: u32 = 1;
pub(crate) const K_CHANGED: u32 = 2;
pub(crate) const K_MAX: u32 = 3;
pub(crate) const K_KEY: u32 = 4;
pub(crate) const K_INDEX: u32 = 5;
/// How many words the counts buffer holds.
pub(crate) const COUNT_WORDS: u32 = 8;

/// What the next work pass does.
pub(crate) const A_NONE: u32 = 0;
pub(crate) const A_SNAPSHOT: u32 = 1;
pub(crate) const A_INDEX: u32 = 2;
pub(crate) const A_RELAX: u32 = 3;

/// Which key an index pass matches: the distance to the grid's centre.
pub(crate) const KIND_CENTRE: u32 = 0;
/// Which key an index pass matches: the field's largest finite distance.
pub(crate) const KIND_FAR: u32 = 1;

/// The regions of the `cells` buffer, in units of the grid's cell count.
pub(crate) const R_PREV: u32 = 0;
pub(crate) const R_SNAP: u32 = 1;
pub(crate) const R_DIST: u32 = 2;
pub(crate) const R_KEPT: u32 = 4;
pub(crate) const R_ROUTE: u32 = 5;
/// How many `u32` a cell holds across the regions.
pub(crate) const REGIONS: u32 = 6;

/// Every constant the route's WGSL reads, by the name it reads it under.
const WGSL_CONSTS: &[(&str, u32)] = &[
    ("INF", INF),
    ("TILE", TILE),
    ("SPAN", TILE + 2),
    ("SPAN_CELLS", (TILE + 2) * (TILE + 2)),
    ("LOCAL_ITERATIONS", LOCAL_ITERATIONS),
    ("C_ARGS", C_ARGS),
    ("C_ACTION", C_ACTION),
    ("C_SEARCH", C_SEARCH),
    ("C_PARITY", C_PARITY),
    ("C_FRESH", C_FRESH),
    ("C_SOURCE", C_SOURCE),
    ("C_KEY", C_KEY),
    ("C_KEY_KIND", C_KEY_KIND),
    ("C_D_MAX", C_D_MAX),
    ("C_SWEEP_PASSES", C_SWEEP_PASSES),
    ("C_CONVERGED", C_CONVERGED),
    ("C_RELAX_PASSES", C_RELAX_PASSES),
    ("C_GENERATIONS", C_GENERATIONS),
    ("C_LAST_MOVED", C_LAST_MOVED),
    ("C_LAST_OPEN", C_LAST_OPEN),
    ("C_LAST_CHANGED", C_LAST_CHANGED),
    ("K_MOVED", K_MOVED),
    ("K_OPEN", K_OPEN),
    ("K_CHANGED", K_CHANGED),
    ("K_MAX", K_MAX),
    ("K_KEY", K_KEY),
    ("K_INDEX", K_INDEX),
    ("A_NONE", A_NONE),
    ("A_SNAPSHOT", A_SNAPSHOT),
    ("A_INDEX", A_INDEX),
    ("A_RELAX", A_RELAX),
    ("KIND_CENTRE", KIND_CENTRE),
    ("KIND_FAR", KIND_FAR),
    ("R_PREV", R_PREV),
    ("R_SNAP", R_SNAP),
    ("R_DIST", R_DIST),
    ("R_KEPT", R_KEPT),
    ("R_ROUTE", R_ROUTE),
];

/// The route constants as WGSL `const` declarations, one source of truth for
/// both sides. Built at construction only.
pub(crate) fn wgsl_consts() -> String {
    WGSL_CONSTS
        .iter()
        .map(|(name, value)| format!("const {name}: u32 = {value}u;\n"))
        .collect()
}

/// The full source of the route's grid passes.
pub(crate) fn grid_source() -> String {
    format!(
        "{}{}{}",
        wgsl_consts(),
        shader::ROUTE_COMMON,
        shader::ROUTE_GRID_SHADER
    )
}

/// The full source of the route's control pass.
pub(crate) fn control_source() -> String {
    format!(
        "{}{}{}",
        wgsl_consts(),
        shader::ROUTE_COMMON,
        shader::ROUTE_CONTROL_SHADER
    )
}

/// The full source of the present pass that draws the route, after the vertex
/// prelude.
pub(crate) fn present_source() -> String {
    format!(
        "{}{}{}",
        wgsl_consts(),
        shader::PRESENT_SHADER,
        shader::ROUTE_PRESENT_MAIN
    )
}

/// Work tiles a side for a grid of `grid` cells.
pub(crate) fn tiles(grid: u32) -> u32 {
    grid.div_ceil(TILE)
}

/// One frame of one clock: what it owed before the frame, the rate it ran at,
/// and how many whole events it owes in the frame.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct Ticks {
    pub(crate) owed_before: f64,
    pub(crate) rate: f64,
    pub(crate) count: u32,
}

impl Ticks {
    /// Seconds into a frame of `dt` at which the `j`th event (from 1) falls:
    /// where the owed count crosses `j`. Held inside `[0, dt]`, which only binds
    /// on the slack that counts a generation a few ulps early and on a stall's
    /// capped backlog.
    pub(crate) fn offset(&self, j: u32, dt: f64) -> f64 {
        if self.rate <= 0.0 {
            return dt;
        }
        ((f64::from(j) - self.owed_before) / self.rate).clamp(0.0, dt)
    }
}

/// Integrates the route's pass rate over injected `dt` into whole passes, as
/// [`GenerationClock`](super::GenerationClock) integrates generations: the
/// fraction carries, and a backlog past [`MAX_ROUTE_PASSES_PER_FRAME`] is
/// dropped rather than carried.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct RouteClock {
    owed: f64,
}

impl RouteClock {
    /// Add `dt` seconds at `rate` passes per second and return this frame's
    /// schedule. A rate that is not finite or not positive owes nothing.
    pub(crate) fn advance(&mut self, rate: f32, dt: f32) -> Ticks {
        let rate = if rate.is_finite() {
            f64::from(rate.max(0.0))
        } else {
            0.0
        };
        let owed_before = self.owed;
        self.owed += rate * f64::from(dt);
        let whole = (self.owed + WHOLE_SLACK).floor();
        let cap = f64::from(MAX_ROUTE_PASSES_PER_FRAME);
        let count = if whole > cap {
            self.owed = 0.0;
            MAX_ROUTE_PASSES_PER_FRAME
        } else {
            self.owed = (self.owed - whole).max(0.0);
            whole as u32
        };
        Ticks {
            owed_before,
            rate,
            count,
        }
    }

    /// Forget whatever is owed: the route is off, and owes no passes for the
    /// time it was off.
    pub(crate) fn reset(&mut self) {
        self.owed = 0.0;
    }
}

/// One event on a frame's timeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RouteEvent {
    /// A generation, which a compare pass and a control pass follow.
    Generation,
    /// A route pass: one work pass and a control pass.
    Pass,
}

/// A frame's generations and route passes in time order, each with its scene
/// time in milliseconds since configure.
///
/// Times are compared in whole microseconds, so two schedules reaching the same
/// instant through different sums of `dt` still tie, and a tie goes to the
/// generation.
pub(crate) struct Timeline {
    pub(crate) generations: Ticks,
    pub(crate) passes: Ticks,
    /// Scene time at the frame's start, in seconds since configure.
    pub(crate) start: f64,
    pub(crate) dt: f64,
    pub(crate) next_generation: u32,
    pub(crate) next_pass: u32,
}

impl Timeline {
    pub(crate) fn new(generations: Ticks, passes: Ticks, start: f64, dt: f64) -> Self {
        Self {
            generations,
            passes,
            start,
            dt,
            next_generation: 0,
            next_pass: 0,
        }
    }

    fn micros(&self, ticks: &Ticks, j: u32) -> u64 {
        ((self.start + ticks.offset(j, self.dt)) * 1.0e6)
            .round()
            .max(0.0) as u64
    }
}

/// Milliseconds as the GPU stamps them, saturating past `u32::MAX` (49 days).
fn millis(micros: u64) -> u32 {
    u32::try_from(micros / 1000).unwrap_or(u32::MAX)
}

impl Iterator for Timeline {
    type Item = (RouteEvent, u32);

    fn next(&mut self) -> Option<Self::Item> {
        let generation = (self.next_generation < self.generations.count)
            .then(|| self.micros(&self.generations, self.next_generation + 1));
        let pass = (self.next_pass < self.passes.count)
            .then(|| self.micros(&self.passes, self.next_pass + 1));
        match (generation, pass) {
            (Some(g), Some(p)) if p < g => {
                self.next_pass += 1;
                Some((RouteEvent::Pass, millis(p)))
            }
            (Some(g), _) => {
                self.next_generation += 1;
                Some((RouteEvent::Generation, millis(g)))
            }
            (None, Some(p)) => {
                self.next_pass += 1;
                Some((RouteEvent::Pass, millis(p)))
            }
            (None, None) => None,
        }
    }
}

/// One event's uniform slot.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct EventParams {
    /// x: grid (cells per side), y: wrap (1 torus), z: scene time (ms since
    /// configure), w: work tiles a side.
    a: [u32; 4],
    /// Unused.
    b: [u32; 4],
}

/// The present pass's route uniform.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct RoutePresent {
    /// x: route, y: route_coord, z: route_grade, w: unused.
    pub(crate) a: [f32; 4],
}

/// Event slots a frame can fill: every generation and every route pass.
const EVENT_SLOTS: u32 = MAX_GENERATIONS_PER_FRAME + MAX_ROUTE_PASSES_PER_FRAME;

/// A zeroed control buffer: idle, nothing dispatched, the pair's first half.
const CONTROL_ZERO: [u32; CONTROL_WORDS as usize] = [0; CONTROL_WORDS as usize];
/// A zeroed counts buffer.
const COUNTS_ZERO: [u32; COUNT_WORDS as usize] = [0; COUNT_WORDS as usize];

/// The route's GPU side, built on the first frame the route is on and rebuilt
/// with the grid. Never per frame.
pub(crate) struct RouteResources {
    /// The grid these buffers were sized for.
    pub(crate) grid: u32,
    #[cfg_attr(
        not(test),
        allow(dead_code, reason = "read back by the tests; the bind groups hold it")
    )]
    pub(crate) cells: wgpu::Buffer,
    pub(crate) counts: wgpu::Buffer,
    pub(crate) control: wgpu::Buffer,
    events: wgpu::Buffer,
    /// Bytes between two event slots: the adapter's dynamic-offset alignment.
    stride: u32,
    /// The frame's event slots, written into here and uploaded once. Sized at
    /// build, so a frame allocates nothing.
    staging: Vec<u8>,
    /// Slots this frame has filled.
    used: u32,
    tiles: u32,
    compare_pipeline: wgpu::ComputePipeline,
    work_pipeline: wgpu::ComputePipeline,
    after_generation_pipeline: wgpu::ComputePipeline,
    after_work_pipeline: wgpu::ComputePipeline,
    /// The grid passes' bind group reading each texture of the field's pair.
    grid_bg: ReadPair,
    control_bg: wgpu::BindGroup,
    pub(crate) present_pipeline: wgpu::RenderPipeline,
    pub(crate) present_bg: wgpu::BindGroup,
    pub(crate) present_uniform: wgpu::Buffer,
}

/// A `u32` storage buffer of `words`, readable back by a probe.
fn word_buffer(
    device: &wgpu::Device,
    label: &str,
    words: u64,
    extra: wgpu::BufferUsages,
) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: words * 4,
        usage: wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_DST
            | wgpu::BufferUsages::COPY_SRC
            | extra,
        mapped_at_creation: false,
    })
}

fn compute_pipeline(
    device: &wgpu::Device,
    label: &str,
    layout: &wgpu::BindGroupLayout,
    module: &wgpu::ShaderModule,
    entry: &str,
) -> wgpu::ComputePipeline {
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(label),
        bind_group_layouts: &[Some(layout)],
        immediate_size: 0,
    });
    device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some(label),
        layout: Some(&pipeline_layout),
        module,
        entry_point: Some(entry),
        compilation_options: Default::default(),
        cache: None,
    })
}

impl RouteResources {
    /// Build every buffer, pipeline and bind group for `grid` over `field`.
    /// `present_layout` is the scene's own present group, which the route's
    /// present pipeline binds at group 0 beside its own at group 1.
    pub(crate) fn build(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        grid: u32,
        field: &PingPongField,
        present_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let cell_count = u64::from(grid) * u64::from(grid);
        let cells = word_buffer(
            device,
            "cellular-route-cells",
            cell_count * u64::from(REGIONS),
            wgpu::BufferUsages::empty(),
        );
        let counts = word_buffer(
            device,
            "cellular-route-counts",
            u64::from(COUNT_WORDS),
            wgpu::BufferUsages::empty(),
        );
        // `INDIRECT` because its first three words are the work pass's
        // dispatch. The work pass binds it read-only, and a read-only storage
        // binding and an indirect read may share one dispatch.
        let control = word_buffer(
            device,
            "cellular-route-control",
            u64::from(CONTROL_WORDS),
            wgpu::BufferUsages::INDIRECT,
        );
        let align = device.limits().min_uniform_buffer_offset_alignment.max(1);
        let stride = size_of::<EventParams>().next_multiple_of(align as usize) as u32;
        let events = gpu::uniform_buffer(
            device,
            "cellular-route-events",
            (stride * EVENT_SLOTS) as usize,
        );
        let event_size = wgpu::BufferSize::new(size_of::<EventParams>() as u64);

        // `[Texture, Storage, Storage, Storage, Uniform+size]`, all COMPUTE: the
        // field, `cells`, `counts`, `control` read-only, and the event slot.
        let grid_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("cellular-route-grid-layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: event_size,
                    },
                    count: None,
                },
            ],
        });
        // `[Uniform+size, Storage, Storage, Storage]`, all COMPUTE: the event
        // slot first, then `cells` read-only, `counts` and `control`. The
        // uniform leads so the shape differs from the grid layout's tail.
        let control_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("cellular-route-control-layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: event_size,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        // `[Storage, Storage, Uniform]`, all FRAGMENT: `cells` and `control`
        // read-only, and the present's route uniform.
        let present_route_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("cellular-route-present-layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    gpu::uniform(2, wgpu::ShaderStages::FRAGMENT),
                ],
            });

        let event_binding = wgpu::BindingResource::Buffer(wgpu::BufferBinding {
            buffer: &events,
            offset: 0,
            size: event_size,
        });
        let grid_bind_group = |view: &wgpu::TextureView| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("cellular-route-grid-bg"),
                layout: &grid_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: cells.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: counts.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: control.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: event_binding.clone(),
                    },
                ],
            })
        };
        let grid_bg = ReadPair {
            a: grid_bind_group(field.view_a()),
            b: grid_bind_group(field.view_b()),
        };
        let control_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("cellular-route-control-bg"),
            layout: &control_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: event_binding.clone(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: cells.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: counts.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: control.as_entire_binding(),
                },
            ],
        });

        let grid_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("cellular-route-grid"),
            source: wgpu::ShaderSource::Wgsl(grid_source().into()),
        });
        let control_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("cellular-route-control"),
            source: wgpu::ShaderSource::Wgsl(control_source().into()),
        });
        let compare_pipeline = compute_pipeline(
            device,
            "cellular-route-compare",
            &grid_layout,
            &grid_module,
            "compare",
        );
        let work_pipeline = compute_pipeline(
            device,
            "cellular-route-relax",
            &grid_layout,
            &grid_module,
            "work",
        );
        let after_generation_pipeline = compute_pipeline(
            device,
            "cellular-route-control-generation",
            &control_layout,
            &control_module,
            "after_generation",
        );
        let after_work_pipeline = compute_pipeline(
            device,
            "cellular-route-control-work",
            &control_layout,
            &control_module,
            "after_work",
        );

        let present_uniform = gpu::uniform_buffer(
            device,
            "cellular-route-present-params",
            size_of::<RoutePresent>(),
        );
        let present_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("cellular-route-present-bg"),
            layout: &present_route_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: cells.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: control.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: present_uniform.as_entire_binding(),
                },
            ],
        });
        let present_shader = gpu::fullscreen_shader(
            device,
            "cellular-route-present-shader",
            gpu::FULLSCREEN_VS_UV_FLIPPED,
            &present_source(),
        );
        let present_pipeline = gpu::fullscreen_pipeline(
            device,
            &present_shader,
            &[present_layout, &present_route_layout],
            surface_format,
            wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING,
            "cellular-route-present",
        );

        Self {
            grid,
            cells,
            counts,
            control,
            events,
            stride,
            staging: vec![0; (stride * EVENT_SLOTS) as usize],
            used: 0,
            tiles: tiles(grid),
            compare_pipeline,
            work_pipeline,
            after_generation_pipeline,
            after_work_pipeline,
            grid_bg,
            control_bg,
            present_pipeline,
            present_bg,
            present_uniform,
        }
    }

    /// Return the route to idle: no action, nothing dispatched, no counts.
    /// The per-cell regions are left as they are; nothing reads them before a
    /// new snapshot is taken.
    pub(crate) fn reset(&self, queue: &wgpu::Queue) {
        queue.write_buffer(&self.control, 0, bytemuck::cast_slice(&CONTROL_ZERO));
        queue.write_buffer(&self.counts, 0, bytemuck::cast_slice(&COUNTS_ZERO));
    }

    /// Start a frame's timeline: no slot filled.
    pub(crate) fn begin_frame(&mut self) {
        self.used = 0;
    }

    /// Fill the next event slot and return its dynamic offset, or `None` once
    /// every slot is spent — which the two per-frame caps keep from happening.
    fn slot(&mut self, wrap: bool, ms: u32) -> Option<u32> {
        if self.used >= EVENT_SLOTS {
            return None;
        }
        let offset = self.used * self.stride;
        let params = EventParams {
            a: [self.grid, u32::from(wrap), ms, self.tiles],
            b: [0; 4],
        };
        let bytes = bytemuck::bytes_of(&params);
        let at = offset as usize;
        self.staging
            .get_mut(at..at + bytes.len())?
            .copy_from_slice(bytes);
        self.used += 1;
        Some(offset)
    }

    /// Encode what follows a generation: the compare pass over the field's
    /// current texture, then the control pass, both at scene time `ms`.
    pub(crate) fn encode_generation(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        field: &PingPongField,
        wrap: bool,
        ms: u32,
    ) {
        let Some(offset) = self.slot(wrap, ms) else {
            return;
        };
        {
            let mut pass = gpu::compute_pass(encoder, "cellular-route-compare");
            pass.set_pipeline(&self.compare_pipeline);
            pass.set_bind_group(0, self.grid_bg.for_field(field), &[offset]);
            pass.dispatch_workgroups(self.tiles, self.tiles, 1);
        }
        let mut pass = gpu::compute_pass(encoder, "cellular-route-control");
        pass.set_pipeline(&self.after_generation_pipeline);
        pass.set_bind_group(0, &self.control_bg, &[offset]);
        pass.dispatch_workgroups(1, 1, 1);
    }

    /// Encode one route pass: the work pass the control words name, over as
    /// many tiles as they say, then the control pass, both at scene time `ms`.
    pub(crate) fn encode_pass(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        field: &PingPongField,
        wrap: bool,
        ms: u32,
    ) {
        let Some(offset) = self.slot(wrap, ms) else {
            return;
        };
        {
            let mut pass = gpu::compute_pass(encoder, "cellular-route-relax");
            pass.set_pipeline(&self.work_pipeline);
            pass.set_bind_group(0, self.grid_bg.for_field(field), &[offset]);
            pass.dispatch_workgroups_indirect(&self.control, u64::from(C_ARGS) * 4);
        }
        let mut pass = gpu::compute_pass(encoder, "cellular-route-control");
        pass.set_pipeline(&self.after_work_pipeline);
        pass.set_bind_group(0, &self.control_bg, &[offset]);
        pass.dispatch_workgroups(1, 1, 1);
    }

    /// Upload the frame's filled slots, before the encoder is submitted.
    pub(crate) fn finish_frame(&self, queue: &wgpu::Queue) {
        let bytes = (self.used * self.stride) as usize;
        if let Some(filled) = self.staging.get(..bytes)
            && !filled.is_empty()
        {
            queue.write_buffer(&self.events, 0, filled);
        }
    }
}
