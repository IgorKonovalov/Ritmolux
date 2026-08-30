//! The attractor's GPU resources: the three uniform blocks, the three resource
//! holders, and their bind-group helpers (Plan 0061 Phase 6).
//!
//! This is the `wgpu` half of `particles/` — buffers, layouts, pipelines and the
//! bind groups that wire them together. The scene that drives them, its `Scene`
//! impl and the `encode_*` passes stay in `mod.rs`; the ODE math it draws is in
//! [`family`](super::family), which imports no `wgpu` at all.

// Hot-path panic-denial pragma (Plan 0002 Phase 2; render/ is scanned by the
// hygiene guard).
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

// A continuation of one module split across four files, so it needs the names
// `particles/mod.rs` has in scope.
use super::*;

/// Compute step uniform (per frame): the attractor coefficients, the fixed
/// sub-step `dt`, the selected family, and the active particle count.
///
/// The same layout drives the one-shot **jitter** dispatch (ADR-0066), where
/// `family` is [`JITTER_MODE`], `coeffs.xyz` is the kick's half-extent and `salt`
/// is the reseed counter. One struct and one pipeline rather than a second of
/// each: the jitter reads and writes the same storage buffer through the same
/// bind-group layout, so only the uniform's contents differ.
///
/// **192 bytes**, for every family including the four that ignore the trailing
/// fields — negligible in
/// bandwidth, and noted because it is a struct four families share. ADR-0075
/// predicted 144 for the Plan 0062 shape; the extra 16 is the alignment padding
/// [`step_index`](Self::step_index) forces, because the scalar block ahead of the
/// `vec4` table has to round up to a multiple of 16 and it was already exactly
/// full. **The bind-group layout gains no binding at either step**, so the
/// collision surface ADR-0058 reasons about does not change shape.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct StepUniform {
    pub(super) coeffs: [f32; 4],
    pub(super) dt: f32,
    pub(super) family: u32,
    pub(super) count: u32,
    /// Which reseed this is, for the jitter dispatch. Zero (and unread) on a
    /// stepping dispatch — it was the struct's explicit padding word.
    pub(super) salt: u32,
    /// The monotonic fixed-step counter the IFS draws its map choice from
    /// (ADR-0075). Zero (and unread) on every other family, and on the jitter
    /// dispatch — which keeps its own `salt` rather than sharing this.
    pub(super) step_index: u32,
    /// The reciprocal of the fixed-point set's floored diameter
    /// ([`ifs::skeleton_scale`], ADR-0088) — the scale the step shader's IFS arm
    /// normalises a raw nearest-point distance by. Zero (and unread) on the four
    /// map families and on the jitter dispatch, exactly as the affine table is.
    ///
    /// **It costs no bytes.** It takes the first of the three explicit padding
    /// words the `vec4` table's alignment had already paid for, so the struct
    /// stays 192 and the bind-group layout gains no binding.
    pub(super) root_recip: f32,
    /// The rest of that padding. Explicit, because the `vec4` table below is
    /// 16-byte aligned and the scalars above are five words. `bytemuck::Pod`
    /// requires no implicit padding, so these words must be named.
    pub(super) _pad: [u32; 2],
    /// The IFS's resolved affine table — [`IfsPacked`] laid out flat. Zeroed for
    /// the four map families, which never read it.
    pub(super) linear: [[f32; 4]; ifs::MAPS],
    pub(super) translate: [[f32; 4]; 2],
    pub(super) cumulative_p: [f32; 4],
    /// The four respawn targets (ADR-0087), two `(x, y)` per row exactly as
    /// `translate` is packed.
    pub(super) fixed: [[f32; 4]; 2],
}

impl StepUniform {
    /// The IFS half of the uniform, as the four map families and the jitter
    /// dispatch write it: all zeros, and unread.
    pub(super) const NO_IFS: IfsPacked = IfsPacked::ZERO;

    /// Assemble one slot. The IFS payload is spread across three fields, so a
    /// constructor is what keeps the three call sites from disagreeing about it.
    pub(super) fn new(
        coeffs: [f32; 4],
        family: u32,
        count: u32,
        salt: u32,
        step_index: u32,
        packed: IfsPacked,
    ) -> Self {
        Self {
            coeffs,
            dt: FIXED_STEP,
            family,
            count,
            salt,
            step_index,
            root_recip: packed.root_recip,
            _pad: [0; 2],
            linear: packed.linear,
            translate: packed.translate,
            cumulative_p: packed.cumulative_p,
            fixed: packed.fixed,
        }
    }
}

/// Draw uniform (per frame). `v`: x aspect, y point half-size, z hue offset, w
/// spin. `w`: x world scale, y projection dim (2 or 3), z z-centre (3D),
/// w [`deposit_scale`] (ADR-0065) times [`brightness_factor`] (ADR-0080).
/// `u`: x hue_spread, y hue_center, z palette_mix, w saturation (ADR-0021).
/// `x`: x zoom, yz pan (view transform, ADR-0018), w the streak flag
/// (ADR-0069) — non-zero exactly when [`AttractorFamily::is_continuous`].
/// `bh`/`bv`: the 3D projection basis's two axis selectors (ADR-0068) — the axis
/// the spin rotates `x` against, and the vertical. Read only on the 3D branch.
/// `d`: x `perspective`, y `depth_fade`, z `depth_hue`, w the family's
/// **inverse** depth half-extent (ADR-0076) — `0` for a 2D family, which is what
/// makes every depth cue the identity there without a shader branch.
/// `ctr`: xyz the world centre subtracted before projection (Plan 0062), w unused.
/// `ch`: the two per-particle colour channels at two routes each — x `map_tint`,
/// y `map_hue` (ADR-0087), z `root_tint`, w `root_hue` (ADR-0088). All four
/// default to `0`, which is the arithmetic identity on every route.
///
/// **The row swapped rather than grew** at Plan 0074 Phase 3: `age_tint` and
/// `age_hue` held z and w until the age channel was retired. The two halves are
/// *not* the same shape — `map_*` is centred, `root_*` is anchored at `0` — for
/// the reason in ADR-0088's Anchoring section.
///
/// `em`: the emergence ramp (ADR-0087) — x the per-step brightness increment, y
/// the floor. `(1/emergence, 0)` on the IFS and `(0, 1)` everywhere else,
/// because every other family's `age` is identically zero and a bare `age·rate`
/// would black them out rather than leave them alone. **z and w are free** since
/// the retirement: z carried `1/churn_max_lifetime()`, which only the age colour
/// channel read.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct DrawUniform {
    pub(super) v: [f32; 4],
    pub(super) w: [f32; 4],
    pub(super) u: [f32; 4],
    pub(super) x: [f32; 4],
    pub(super) bh: [f32; 4],
    pub(super) bv: [f32; 4],
    pub(super) d: [f32; 4],
    pub(super) ctr: [f32; 4],
    pub(super) ch: [f32; 4],
    pub(super) em: [f32; 4],
}

/// Decay uniform (per frame): x is the per-frame trail retention factor.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct DecayUniform {
    pub(super) k: [f32; 4],
    /// The ADR-0048 feedback transform, exactly as
    /// [`feedback::Transform::pack`](crate::render::feedback::Transform::pack)
    /// returns it. Read by the decay pass; the present pass declares only `k` over
    /// the same buffer, which is legal — a uniform binding may be wider than the
    /// struct a shader lays over it — and deliberate, because the present's
    /// bind-group layout *shape* is what the WARP adapter is sensitive to.
    pub(super) xf: [f32; 4],
    pub(super) tr: [f32; 4],
    pub(super) wp: [f32; 4],
}

/// The GPU-side state, built lazily on first render (see the module docs), split
/// along the axis that actually varies (Plan 0029 Phase 1): everything in
/// [`PipelineResources`] is built once and survives every size change; only
/// [`FieldResources`] is rebuilt when the accumulation grid changes.
pub(super) struct Resources {
    pub(super) pipelines: PipelineResources,
    pub(super) grid: FieldResources,
}

/// The grid-**independent** GPU state: the four shader modules, every pipeline,
/// the particle storage buffer, the uniform buffers, and the LUT textures. None
/// of it references the accumulation field, so a size change must not touch it —
/// recompiling four WGSL modules and rebuilding four pipelines inside `render` is
/// a multi-hundred-millisecond stall, and the standalone forwards every
/// `WindowEvent::Resized`, so a live drag paid it per frame (Plan 0029 Phase 1).
pub(super) struct PipelineResources {
    pub(super) compute_pipeline: wgpu::ComputePipeline,
    pub(super) draw_pipeline: wgpu::RenderPipeline,
    pub(super) decay_pipeline: wgpu::RenderPipeline,
    pub(super) present_pipeline: wgpu::RenderPipeline,
    pub(super) particles: wgpu::Buffer,
    pub(super) step_uniform: wgpu::Buffer,
    /// Byte stride between two slots of `step_uniform`, rounded up to the
    /// adapter's dynamic-offset alignment.
    ///
    /// Separate slots rather than one written repeatedly, because a frame encodes
    /// `pending_steps` step dispatches against one binding: folding the jitter into
    /// the step slot would apply it once per sub-step, making the disturbance a
    /// function of the frame's timing and breaking determinism. [`STEP_SLOTS`] has
    /// the same argument for the sub-steps themselves.
    pub(super) step_stride: u32,
    pub(super) draw_uniform: wgpu::Buffer,
    pub(super) decay_uniform: wgpu::Buffer,
    pub(super) compute_bg: wgpu::BindGroup,
    pub(super) draw_bg: wgpu::BindGroup,
    /// The shared gradient LUT textures (A/B) the draw vertex shader samples +
    /// crossfades (ADR-0021); uploaded from the scene's baked palette on the first
    /// frame after a build and on a preset switch. They outlive a grid change, so
    /// a resize does not re-upload the palette.
    pub(super) lut_texture_a: wgpu::Texture,
    pub(super) lut_texture_b: wgpu::Texture,
    /// Kept so a grid change can rebuild [`FieldResources`]' four bind groups
    /// without recreating a layout, a sampler, or any pipeline.
    pub(super) decay_layout: wgpu::BindGroupLayout,
    pub(super) present_layout: wgpu::BindGroupLayout,
    pub(super) field_sampler: wgpu::Sampler,
    /// How many particles the buffer above holds — the active tier's
    /// [`attractor_particles`](crate::render::TierConfig::attractor_particles),
    /// fixed for the life of these resources.
    ///
    /// Since ADR-0069 the dispatch, the instance draw and the step uniform take
    /// the **active** count instead (`round(budget * density)`), which is a
    /// different and smaller number. This one survives as the allocation bound:
    /// the draw clamps its instance range to it, so no arithmetic on `density`
    /// can fetch a vertex past the end of the buffer.
    pub(super) count: u32,
}

/// The grid-**dependent** GPU state: the accumulation field and the four bind
/// groups that reference its two texture views. The only block a size change
/// rebuilds — a texture pair plus four bind groups, none of which compiles a
/// shader (Plan 0029 Phase 1).
pub(super) struct FieldResources {
    /// Two-texture accumulation the trails ping-pong between (ADR-0012 reuse).
    pub(super) field: PingPongField,
    /// Decay/present bind groups reading texture A / texture B — selected by the
    /// field's read side each frame so nothing is rebuilt on the hot path.
    pub(super) decay_bg_a: wgpu::BindGroup,
    pub(super) decay_bg_b: wgpu::BindGroup,
    pub(super) present_bg_a: wgpu::BindGroup,
    pub(super) present_bg_b: wgpu::BindGroup,
    /// The accumulation grid this block was built for; `render` compares the
    /// requested grid against it and rebuilds only this block on a difference.
    pub(super) trail_w: u32,
    pub(super) trail_h: u32,
}

impl Resources {
    pub(super) fn build(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        trail_w: u32,
        trail_h: u32,
        count: u32,
    ) -> Self {
        let pipelines = PipelineResources::build(device, surface_format, count);
        let grid = FieldResources::build(device, &pipelines, trail_w, trail_h);
        Self { pipelines, grid }
    }

    /// Re-allocate the accumulation field at a new grid, reusing every pipeline,
    /// buffer, and texture that does not depend on it. The rebuilt field is
    /// undefined, so the caller re-flags the clear (and the seed upload, which
    /// keeps a capture reproducible from the same starting scatter).
    pub(super) fn rebuild_grid(&mut self, device: &wgpu::Device, trail_w: u32, trail_h: u32) {
        self.grid = FieldResources::build(device, &self.pipelines, trail_w, trail_h);
    }
}

/// The draw pass's instance attributes, with **explicit byte offsets into
/// [`Particle`]**.
///
/// **Spelled out rather than built by `vertex_attr_array!`, and that is the
/// whole point of this constant.** That macro lays its attributes out
/// *consecutively* — which was correct while the struct was `pos`, `seed`,
/// `prev` and one trailing pad, and stopped being correct the moment ADR-0087
/// put `age` and `map` past that pad. A fourth macro entry would have fetched
/// the padding word at offset 28 and fed the draw someone else's bytes, silently
/// and with no compile error. `the_particle_layout_carries_three_channels`
/// measures these offsets against the struct so the two cannot drift.
pub(super) const PARTICLE_ATTRIBUTES: &[wgpu::VertexAttribute] = &[
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x3,
        offset: 0,
        shader_location: 0, // pos (z = 0 for 2D families)
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32,
        offset: 12,
        shader_location: 1, // seed
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x3,
        offset: 16,
        shader_location: 2, // prev (ADR-0069)
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32,
        offset: 36,
        shader_location: 3, // map (ADR-0087) — 36, NOT 28, which is `_pad`
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32,
        offset: 32,
        shader_location: 4, // age (ADR-0087)
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32,
        offset: 40,
        shader_location: 5, // root (ADR-0088) — 40, the first spare word
    },
];

impl PipelineResources {
    pub(super) fn build(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        count: u32,
    ) -> Self {
        // The shared bit-mixer, concatenated in — the same WGSL the tonemap's
        // dither compiles (Plan 0082 Phase 1), so a particle's reseed kick and a
        // display-write LSB cannot drift apart on what the hash is.
        let step_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("attractor-step-shader"),
            source: wgpu::ShaderSource::Wgsl(format!("{}{STEP_SHADER}", gpu::HASH_WGSL).into()),
        });
        let draw_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("attractor-draw-shader"),
            source: wgpu::ShaderSource::Wgsl(DRAW_SHADER.into()),
        });
        // ADR-0048's transform, concatenated in: the same WGSL the engine trails
        // stage compiles, so the two accumulation sinks cannot drift apart on what
        // `fb_rotate` means.
        let decay_shader = gpu::fullscreen_shader(
            device,
            "attractor-decay-shader",
            gpu::FULLSCREEN_VS_UV_FLIPPED,
            &format!("{}{DECAY_SHADER}", crate::render::feedback::TRANSFORM_WGSL),
        );
        let present_shader = gpu::fullscreen_shader(
            device,
            "attractor-present-shader",
            gpu::FULLSCREEN_VS_UV_FLIPPED,
            PRESENT_SHADER,
        );

        // Particle storage buffer: written by the compute step (STORAGE), read by
        // the draw pass as an instance vertex buffer (VERTEX), seeded once from
        // the CPU (COPY_DST). One buffer, two roles — no CPU round-trip.
        //
        // `COPY_SRC` is there for [`read_particles`], the reseed test's readback
        // (Plan 0057 Phase 3). Carried unconditionally rather than behind
        // `cfg(test)` so the test exercises the buffer the app actually allocates;
        // a usage flag costs nothing that is not used, and a test running against a
        // differently-configured resource is a test of something else.
        let particles = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("attractor-particles"),
            size: (count as usize * std::mem::size_of::<Particle>()) as u64,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::VERTEX
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        // [`STEP_SLOTS`] slots in ONE buffer, selected per dispatch by a dynamic
        // offset.
        //
        // **Not two buffers behind two bind groups**, which is what this was first
        // written as and which does not survive the software adapter: a second bind
        // group sharing a live pipeline's layout gets aliased on WARP, so the step
        // dispatch read the *jitter* slot — all zeros, so `count = 0`, so every
        // invocation returned and the cloud never moved. It rendered a plausible
        // static box, moved the golden baseline, and dropped three presets to
        // ~0.000 in `animation`. One layout and one bind group has no aliasing
        // surface to get wrong.
        let step_stride = uniform_stride(device);
        let step_uniform = uniform_buffer(
            device,
            "attractor-step-uniform",
            (step_stride * STEP_SLOTS) as usize,
        );
        let draw_uniform =
            uniform_buffer(device, "attractor-draw-uniform", size_of::<DrawUniform>());
        let decay_uniform =
            uniform_buffer(device, "attractor-decay-uniform", size_of::<DecayUniform>());

        // --- compute: read_write storage + step uniform ---
        let compute_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("attractor-compute-layout"),
            entries: &[
                storage_entry(0),
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        // The sub-step slots and the jitter slot, one dispatch each.
                        has_dynamic_offset: true,
                        min_binding_size: wgpu::BufferSize::new(size_of::<StepUniform>() as u64),
                    },
                    count: None,
                },
            ],
        });
        let compute_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("attractor-compute-bg"),
            layout: &compute_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: particles.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    // A window the size of one `StepUniform`, not the whole
                    // buffer: the dynamic offset slides it between the two slots.
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &step_uniform,
                        offset: 0,
                        size: wgpu::BufferSize::new(size_of::<StepUniform>() as u64),
                    }),
                },
            ],
        });
        let compute_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("attractor-compute-pipeline-layout"),
                bind_group_layouts: &[Some(&compute_layout)],
                immediate_size: 0,
            });
        let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("attractor-compute-pipeline"),
            layout: Some(&compute_pipeline_layout),
            module: &step_shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });

        // Shared gradient LUTs (ADR-0021): two 256×1 textures (A/B) + a repeat
        // sampler, bound to the draw pass and sampled per-particle in the vertex
        // shader (so VERTEX visibility).
        let lut_texture_a = palette::lut_texture(device, "attractor-lut-a");
        let lut_texture_b = palette::lut_texture(device, "attractor-lut-b");
        let lut_view_a = lut_texture_a.create_view(&wgpu::TextureViewDescriptor::default());
        let lut_view_b = lut_texture_b.create_view(&wgpu::TextureViewDescriptor::default());
        let lut_sampler = palette::lut_sampler(device);

        // --- draw: the particle buffer as an instance vertex buffer, additively
        // into the trail field (float target so the accumulation has headroom) ---
        let lut_vertex_texture = |binding: u32| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::VERTEX,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let draw_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("attractor-draw-layout"),
            entries: &[
                gpu::uniform(0, wgpu::ShaderStages::VERTEX),
                lut_vertex_texture(1),
                lut_vertex_texture(2),
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let draw_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("attractor-draw-bg"),
            layout: &draw_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: draw_uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&lut_view_a),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&lut_view_b),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&lut_sampler),
                },
            ],
        });
        let draw_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("attractor-draw-pipeline-layout"),
            bind_group_layouts: &[Some(&draw_layout)],
            immediate_size: 0,
        });
        let draw_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("attractor-draw-pipeline"),
            layout: Some(&draw_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &draw_shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Particle>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: PARTICLE_ATTRIBUTES,
                })],
            },
            fragment: Some(wgpu::FragmentState {
                module: &draw_shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: PingPongField::FORMAT,
                    // Additive: overlapping points bloom brighter (the dense look).
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::One,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent::OVER,
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        // --- decay + present: fullscreen samples of the accumulation field ---
        // The layouts, the sampler and both pipelines are grid-independent; only
        // the bind groups that name the field's views are not, and those live in
        // `FieldResources`.
        let field_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("attractor-sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let decay_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("attractor-decay-layout"),
            entries: &[
                gpu::texture(0, true),
                gpu::sampler(1),
                gpu::uniform(2, wgpu::ShaderStages::FRAGMENT),
            ],
        });
        let decay_pipeline = gpu::fullscreen_pipeline(
            device,
            &decay_shader,
            &[&decay_layout],
            PingPongField::FORMAT,
            // The decay pass overwrites the trail field with the faded previous frame.
            wgpu::BlendState::REPLACE,
            "attractor-decay",
        );

        // Texture, sampler, uniform, **sampler again** — the fourth entry is the
        // same sampler a second time, and it is there to make this layout a shape
        // nothing else in the crate has. `occlude` (ADR-0085) needed a uniform in a
        // pass that had none; see `PRESENT_SHADER` for the measurement that says a
        // colliding shape silently mis-renders on WARP.
        let present_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("attractor-present-layout"),
            entries: &[
                gpu::texture(0, true),
                gpu::sampler(1),
                gpu::uniform(2, wgpu::ShaderStages::FRAGMENT),
                gpu::sampler(3),
            ],
        });
        let present_pipeline = gpu::fullscreen_pipeline(
            device,
            &present_shader,
            &[&present_layout],
            surface_format,
            // Premultiplied-alpha OVER the backdrop (ADR-0026): the accumulation is
            // emissive, so `c` adds over the atmosphere and the present's alpha
            // (accumulated luminance) reveals bg_* in the cloud's empty space. Over
            // the default black backdrop this equals the prior opaque present.
            wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING,
            "attractor-present",
        );

        Self {
            compute_pipeline,
            draw_pipeline,
            decay_pipeline,
            present_pipeline,
            particles,
            step_uniform,
            step_stride,
            draw_uniform,
            decay_uniform,
            compute_bg,
            draw_bg,
            lut_texture_a,
            lut_texture_b,
            decay_layout,
            present_layout,
            field_sampler,
            count,
        }
    }
}

impl FieldResources {
    /// Allocate the accumulation field at `trail_w`x`trail_h` and bind its two
    /// views into the four decay/present groups, reusing `pipelines`' layouts,
    /// sampler and decay uniform. No shader, pipeline, particle or LUT resource
    /// is created here — that is the whole point of the split.
    pub(super) fn build(
        device: &wgpu::Device,
        pipelines: &PipelineResources,
        trail_w: u32,
        trail_h: u32,
    ) -> Self {
        let field = PingPongField::new(device, trail_w, trail_h);
        let decay_bg_a = blit_bind_group(
            device,
            &pipelines.decay_layout,
            "attractor-decay-bg-a",
            field.view_a(),
            &pipelines.field_sampler,
            Some(&pipelines.decay_uniform),
            false,
        );
        let decay_bg_b = blit_bind_group(
            device,
            &pipelines.decay_layout,
            "attractor-decay-bg-b",
            field.view_b(),
            &pipelines.field_sampler,
            Some(&pipelines.decay_uniform),
            false,
        );
        let present_bg_a = blit_bind_group(
            device,
            &pipelines.present_layout,
            "attractor-present-bg-a",
            field.view_a(),
            &pipelines.field_sampler,
            Some(&pipelines.decay_uniform),
            true,
        );
        let present_bg_b = blit_bind_group(
            device,
            &pipelines.present_layout,
            "attractor-present-bg-b",
            field.view_b(),
            &pipelines.field_sampler,
            Some(&pipelines.decay_uniform),
            true,
        );
        Self {
            field,
            decay_bg_a,
            decay_bg_b,
            present_bg_a,
            present_bg_b,
            trail_w,
            trail_h,
        }
    }

    /// Clear both accumulation textures to black — run once after a (re)build so
    /// the first decay pass reads a defined (empty) trail rather than garbage.
    pub(super) fn clear_field(&self, encoder: &mut wgpu::CommandEncoder) {
        for view in [self.field.view_a(), self.field.view_b()] {
            encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("attractor-clear-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
        }
    }
}

/// Byte stride between two dynamically-offset slots of a `StepUniform`, rounded
/// up to the adapter's `min_uniform_buffer_offset_alignment` (256 on the default
/// limits). Read from the device rather than hardcoded: a dynamic offset that is
/// not a multiple of it is a validation error, and the limit is the adapter's to
/// state.
pub(super) fn uniform_stride(device: &wgpu::Device) -> u32 {
    let align = device.limits().min_uniform_buffer_offset_alignment.max(1);
    size_of::<StepUniform>().next_multiple_of(align as usize) as u32
}

pub(super) fn uniform_buffer(device: &wgpu::Device, label: &str, size: usize) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: size as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

pub(super) fn storage_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only: false },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

/// A texture(+sampler)[+uniform][+sampler] bind group for the decay/present
/// fullscreen passes.
///
/// `uniform` is the decay buffer for both — the retention factor for decay, and
/// `occlude` out of the same buffer's second component for present (ADR-0085).
/// `repeat_sampler` binds the sampler a second time at binding 3 and is the
/// **present** pass only: it is what makes that layout a fourth shape rather than
/// a copy of `attractor-decay-layout`'s. See `PRESENT_SHADER`.
pub(super) fn blit_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    label: &str,
    input: &wgpu::TextureView,
    sampler: &wgpu::Sampler,
    uniform: Option<&wgpu::Buffer>,
    repeat_sampler: bool,
) -> wgpu::BindGroup {
    let mut entries = vec![
        wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::TextureView(input),
        },
        wgpu::BindGroupEntry {
            binding: 1,
            resource: wgpu::BindingResource::Sampler(sampler),
        },
    ];
    if let Some(buf) = uniform {
        entries.push(wgpu::BindGroupEntry {
            binding: 2,
            resource: buf.as_entire_binding(),
        });
    }
    if repeat_sampler {
        entries.push(wgpu::BindGroupEntry {
            binding: 3,
            resource: wgpu::BindingResource::Sampler(sampler),
        });
    }
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(label),
        layout,
        entries: &entries,
    })
}
