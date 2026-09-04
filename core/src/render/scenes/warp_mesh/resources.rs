//! The warp mesh's GPU resources: the field, the four pipelines, their bind
//! groups and buffers, built once per (target size, grid, shader key).
//!
//! # The creation order is load-bearing
//!
//! [`Resources::build`] is a sequence of stage functions rather than one block,
//! and they are called in the order the objects were created when it was one:
//! a resource created earlier or later changes what a later pass resolves to on
//! the DX12 WARP software adapter (ADR-0058, and the hazard
//! `core/tests/composite.rs` records). Moving a creation between stages is a
//! pixel change, not a tidy. The converted-shader surface is built last for the
//! same reason: a native preset's allocation sequence has to stay what it was.

// Hot-path panic-denial pragma (Plan 0002 Phase 2; render/ is scanned by the
// hygiene guard).
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

// A continuation of one module split across five files, so it needs the names
// `warp_mesh/mod.rs` has in scope.
use super::*;

/// The field's format. `Rgba16Float` for the reason
/// [`PingPongField::FORMAT`] gives, and because this field holds **linear-light**
/// premultiplied colour above 1.0 (ADR-0046) — an 8-bit accumulation would clip
/// every deposit the moment two of them overlapped.
pub(super) const FIELD_FORMAT: wgpu::TextureFormat = PingPongField::FORMAT;

/// The GPU-side state, built lazily on first render (module docs).
pub(super) struct Resources {
    pub(super) field: PingPongField,
    /// The field's pixel size, so a target resize can be noticed and the pair
    /// rebuilt (ADR-0030: compare against what you already built).
    pub(super) size: (u32, u32),
    pub(super) warp_pipeline: wgpu::RenderPipeline,
    pub(super) deposit_pipeline: wgpu::RenderPipeline,
    pub(super) present_pipeline: wgpu::RenderPipeline,
    pub(super) warp_uniform: wgpu::Buffer,
    pub(super) deposit_uniform: wgpu::Buffer,
    pub(super) present_uniform: wgpu::Buffer,
    /// Warp/present bind groups reading texture A / texture B — selected by the
    /// field's read side so nothing is rebuilt on the hot path.
    pub(super) warp_bg_a: wgpu::BindGroup,
    pub(super) warp_bg_b: wgpu::BindGroup,
    pub(super) present_bg_a: wgpu::BindGroup,
    pub(super) present_bg_b: wgpu::BindGroup,
    pub(super) deposit_bg: wgpu::BindGroup,
    pub(super) vertices: wgpu::Buffer,
    pub(super) indices: wgpu::Buffer,
    /// How many indices the current mesh draws, and the grid they were built for.
    pub(super) index_count: u32,
    pub(super) mesh: (u32, u32),
    /// The shared gradient LUT pair (A/B) the deposit pass samples + crossfades
    /// (ADR-0021). A fresh pair is dirty, so a (re)build uploads on its first
    /// frame; a preset switch re-`set`s it through the scene's `set_palette`.
    pub(super) luts: palette::LutPair,
    /// The draw layer's own line renderer (Plan 0100 Phase 4).
    ///
    /// **Its own, not the roster's shared one**, for the reason
    /// `scenes::create_layer_scene` records: a `LineRenderer` uploads its
    /// instance and uniform buffers through `Queue::write_buffer`, queued writes
    /// land before any pass in the submission executes, and two draws through one
    /// renderer in a frame would both rasterize the second draw's segments. The
    /// warp mesh draws its layer in the same frame a line preset could be drawing
    /// its figure — across a dissolve — so sharing is not available here either.
    ///
    /// Built lazily with the rest of `Resources`, so a session that never
    /// activates this scene builds no second line pipeline and cannot meet
    /// ADR-0058's WARP hazard at all.
    pub(super) lines: lines::LineRenderer,
    /// The filled-shape pipeline and its buffers.
    pub(super) shape_pipeline: wgpu::RenderPipeline,
    /// The same pipeline compositing **over** rather than adding, for a shape
    /// instance whose `additive` register is off — see `draw`'s module docs.
    pub(super) shape_over_pipeline: wgpu::RenderPipeline,
    pub(super) shape_vertices: wgpu::Buffer,
    pub(super) shape_uniform: wgpu::Buffer,
    pub(super) shape_bind_group: wgpu::BindGroup,
    /// How many shape vertices the buffer holds.
    pub(super) shape_capacity: usize,
    /// Whether the field still holds the undefined contents of a fresh
    /// allocation. Cleared by one pass before anything samples it.
    pub(super) needs_clear: bool,
    /// The converted-shader surface (Plan 0100 Phase 6) — noise, blur chain,
    /// custom warp/comp pipelines. `None` for every preset without WGSL, which
    /// is what keeps the allocation sequence — and therefore every existing
    /// WARP golden — identical to before this existed.
    pub(super) milk_shaders: Option<shader::MilkShaderResources>,
    /// The [`shader::ShaderSpec::key`] these resources were built for.
    pub(super) shader_key: u64,
}

/// The objects every later stage reads: the three shader modules, the field, the
/// three per-pass uniform buffers, and the one sampler the warp and present bind
/// groups share. Built first, in this order.
struct Common {
    warp_shader: wgpu::ShaderModule,
    deposit_shader: wgpu::ShaderModule,
    present_shader: wgpu::ShaderModule,
    field: PingPongField,
    warp_uniform: wgpu::Buffer,
    deposit_uniform: wgpu::Buffer,
    present_uniform: wgpu::Buffer,
    sampler: wgpu::Sampler,
}

/// The warp pass. `layout` comes back out because the converted-warp pipeline is
/// built against the same bind-group layout.
struct WarpParts {
    layout: wgpu::BindGroupLayout,
    pipeline: wgpu::RenderPipeline,
    bg_a: wgpu::BindGroup,
    bg_b: wgpu::BindGroup,
}

/// The deposit pass, and the gradient LUT pair it samples.
struct DepositParts {
    pipeline: wgpu::RenderPipeline,
    bg: wgpu::BindGroup,
    luts: palette::LutPair,
}

/// The present pass, with one bind group per read side.
struct PresentParts {
    pipeline: wgpu::RenderPipeline,
    bg_a: wgpu::BindGroup,
    bg_b: wgpu::BindGroup,
}

/// The draw layer (Plan 0100 Phase 4): the line batch and the filled-shape pair.
struct DrawParts {
    lines: lines::LineRenderer,
    shape_pipeline: wgpu::RenderPipeline,
    shape_over_pipeline: wgpu::RenderPipeline,
    shape_vertices: wgpu::Buffer,
    shape_uniform: wgpu::Buffer,
    shape_bind_group: wgpu::BindGroup,
    shape_capacity: usize,
}

fn build_common(device: &wgpu::Device, size: (u32, u32)) -> Common {
    let warp_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("warp-mesh-warp-shader"),
        // The quantizer is prepended rather than written out here: a
        // converted shader's own epilogue calls the same text out of
        // `milk::shader::QUANTIZE_WGSL`, and a transfer function that exists
        // in two places drifts (ADR-0118).
        source: wgpu::ShaderSource::Wgsl(
            format!("{}{}", crate::milk::shader::QUANTIZE_WGSL, WARP_SHADER).into(),
        ),
    });
    let deposit_shader = gpu::fullscreen_shader(
        device,
        "warp-mesh-deposit-shader",
        gpu::FULLSCREEN_VS_UV_FLIPPED,
        DEPOSIT_SHADER,
    );
    let present_shader = gpu::fullscreen_shader(
        device,
        "warp-mesh-present-shader",
        gpu::FULLSCREEN_VS_UV_FLIPPED,
        PRESENT_SHADER,
    );

    let field = PingPongField::new(device, size.0.max(1), size.1.max(1));

    let warp_uniform = gpu::uniform_buffer(
        device,
        "warp-mesh-warp-uniform",
        std::mem::size_of::<WarpUniform>(),
    );
    let deposit_uniform = gpu::uniform_buffer(
        device,
        "warp-mesh-deposit-uniform",
        std::mem::size_of::<DepositUniform>(),
    );
    let present_uniform = gpu::uniform_buffer(
        device,
        "warp-mesh-present-uniform",
        std::mem::size_of::<PresentUniform>(),
    );

    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("warp-mesh-sampler"),
        // Clamp, not repeat: the edge policy is the warp shader's `inside`
        // mask, which contributes nothing off-field. A repeating address mode
        // would wrap the past around the frame instead.
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });

    Common {
        warp_shader,
        deposit_shader,
        present_shader,
        field,
        warp_uniform,
        deposit_uniform,
        present_uniform,
        sampler,
    }
}

fn build_warp(device: &wgpu::Device, common: &Common) -> WarpParts {
    let Common {
        warp_shader,
        field,
        warp_uniform,
        sampler,
        ..
    } = common;

    // --- warp pass ---
    //
    // **This layout's shape is deliberately its own** (ADR-0058): the uniform
    // is `VERTEX_FRAGMENT` and declares a `min_binding_size`, which separates
    // it from `attractor-decay-layout`'s `[Texture, Sampler, Uniform]` and
    // from every other three-entry group in the crate. The vertex visibility
    // is honest rather than decorative — the vertex stage genuinely reads
    // `aspect`, `dt`, `time` and `warp_scale` out of this buffer.
    let warp_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("warp-mesh-warp-layout"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: std::num::NonZeroU64::new(
                        std::mem::size_of::<WarpUniform>() as u64
                    ),
                },
                count: None,
            },
            gpu::texture(1, true),
            gpu::sampler(2),
        ],
    });
    let warp_bind_group = |view: &wgpu::TextureView| {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("warp-mesh-warp-bg"),
            layout: &warp_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: warp_uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        })
    };
    let warp_bg_a = warp_bind_group(field.view_a());
    let warp_bg_b = warp_bind_group(field.view_b());
    let warp_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("warp-mesh-warp-pipeline-layout"),
        bind_group_layouts: &[Some(&warp_layout)],
        immediate_size: 0,
    });
    let warp_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("warp-mesh-warp-pipeline"),
        layout: Some(&warp_pipeline_layout),
        vertex: wgpu::VertexState {
            module: warp_shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[Some(wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<Vertex>() as u64,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &VERTEX_ATTRS,
            })],
        },
        fragment: Some(wgpu::FragmentState {
            module: warp_shader,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: FIELD_FORMAT,
                // The mesh covers the whole target, so the warped past
                // replaces rather than blends with whatever was there.
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    });

    WarpParts {
        layout: warp_layout,
        pipeline: warp_pipeline,
        bg_a: warp_bg_a,
        bg_b: warp_bg_b,
    }
}

fn build_deposit(device: &wgpu::Device, common: &Common) -> DepositParts {
    let Common {
        deposit_shader,
        deposit_uniform,
        ..
    } = common;

    // --- deposit pass ---
    let luts = palette::LutPair::new(device, "warp-mesh");
    // The uniform sits **last** and declares a size, which is what keeps this
    // shape off `blend-bind-layout`'s `[Uniform+size, Texture, Texture,
    // Sampler]` and off `shape-field-bind-layout`'s (ADR-0058). Do not tidy
    // the ordering.
    let deposit_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("warp-mesh-deposit-layout"),
        entries: &[
            gpu::texture(0, true),
            gpu::texture(1, true),
            gpu::sampler(2),
            wgpu::BindGroupLayoutEntry {
                binding: 3,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: std::num::NonZeroU64::new(
                        std::mem::size_of::<DepositUniform>() as u64,
                    ),
                },
                count: None,
            },
        ],
    });
    let deposit_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("warp-mesh-deposit-bg"),
        layout: &deposit_layout,
        entries: &{
            let [lut_a, lut_b, lut_sampler] = luts.bind_entries(0, 1, 2);
            [
                lut_a,
                lut_b,
                lut_sampler,
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: deposit_uniform.as_entire_binding(),
                },
            ]
        },
    });
    let deposit_pipeline = gpu::fullscreen_pipeline(
        device,
        deposit_shader,
        &[&deposit_layout],
        FIELD_FORMAT,
        gpu::ADDITIVE_LIGHT_SATURATING_COVERAGE,
        "warp-mesh-deposit",
    );

    DepositParts {
        pipeline: deposit_pipeline,
        bg: deposit_bg,
        luts,
    }
}

fn build_present(
    device: &wgpu::Device,
    surface_format: wgpu::TextureFormat,
    common: &Common,
) -> PresentParts {
    let Common {
        present_shader,
        field,
        present_uniform,
        sampler,
        ..
    } = common;

    // --- present pass ---
    //
    // Uniform first and sized, which separates this from `ink-bind-layout`'s
    // unsized `[Uniform, Texture, Sampler]` (ADR-0058).
    let present_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("warp-mesh-present-layout"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: std::num::NonZeroU64::new(
                        std::mem::size_of::<PresentUniform>() as u64,
                    ),
                },
                count: None,
            },
            gpu::texture(1, true),
            gpu::sampler(2),
        ],
    });
    let present_bind_group = |view: &wgpu::TextureView| {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("warp-mesh-present-bg"),
            layout: &present_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: present_uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        })
    };
    let present_bg_a = present_bind_group(field.view_a());
    let present_bg_b = present_bind_group(field.view_b());
    let present_pipeline = gpu::fullscreen_pipeline(
        device,
        present_shader,
        &[&present_layout],
        surface_format,
        // Premultiplied-alpha OVER the backdrop (ADR-0026): the field is
        // emissive, and its alpha reveals `bg_*` where nothing was deposited.
        wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING,
        "warp-mesh-present",
    );

    PresentParts {
        pipeline: present_pipeline,
        bg_a: present_bg_a,
        bg_b: present_bg_b,
    }
}

fn build_draw_layer(
    device: &wgpu::Device,
    surface_format: wgpu::TextureFormat,
    max_segments: usize,
) -> DrawParts {
    // --- the draw layer (Plan 0100 Phase 4) ---
    // `new_split`, not `new`: this is the one scene whose batch carries both
    // blend modes (see `draw`'s module docs), and the only one that should
    // pay for the second pipeline.
    let lines = lines::LineRenderer::new_split(device, surface_format, max_segments, "warp-mesh");
    let shape_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("warp-mesh-shape-shader"),
        source: wgpu::ShaderSource::Wgsl(SHAPE_SHADER.into()),
    });
    let shape_uniform = gpu::uniform_buffer(
        device,
        "warp-mesh-shape-uniform",
        std::mem::size_of::<ShapeUniform>(),
    );
    // A vertex-visible sized uniform, which is a shape nothing else in
    // `core/src` holds (ADR-0058): `swarm-bind-layout` is the same kind and
    // visibility with **no** declared size, and that difference is exactly
    // what Plan 0053 Phase 3 measured as a real separation. Do not tidy it.
    let shape_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("warp-mesh-shape-layout"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: std::num::NonZeroU64::new(
                    std::mem::size_of::<ShapeUniform>() as u64
                ),
            },
            count: None,
        }],
    });
    let shape_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("warp-mesh-shape-bg"),
        layout: &shape_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: shape_uniform.as_entire_binding(),
        }],
    });
    let shape_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("warp-mesh-shape-pipeline-layout"),
        bind_group_layouts: &[Some(&shape_layout)],
        immediate_size: 0,
    });
    // Two pipelines differing in exactly one field, from one closure — the
    // same shape the line renderer's pair takes, and for the same reason: a
    // MilkDrop shape chooses its blend per instance (`additive`), and reading
    // both as additive is what saturated the frame. They share this layout
    // and this bind group, so ADR-0058 has nothing to separate.
    let shape_pipeline_of = |blend: wgpu::BlendState, suffix: &str| {
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(&format!("warp-mesh-shape-pipeline{suffix}")),
            layout: Some(&shape_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shape_shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<draw::ShapeVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![
                        0 => Float32x2,
                        1 => Float32x3,
                        2 => Float32,
                    ],
                })],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shape_shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: FIELD_FORMAT,
                    blend: Some(blend),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        })
    };
    // The shared draw seam (ADR-0056), same as the line batch beside it.
    let shape_pipeline = shape_pipeline_of(gpu::ADDITIVE_LIGHT_SATURATING_COVERAGE, "");
    // ...and premultiplied OVER, for an instance whose source blend replaces.
    let shape_over_pipeline =
        shape_pipeline_of(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING, "-over");
    let shape_capacity = MAX_SHAPE_VERTICES;
    let shape_vertices = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("warp-mesh-shape-vertices"),
        size: (shape_capacity * std::mem::size_of::<draw::ShapeVertex>()) as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    DrawParts {
        lines,
        shape_pipeline,
        shape_over_pipeline,
        shape_vertices,
        shape_uniform,
        shape_bind_group,
        shape_capacity,
    }
}

impl Resources {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn build(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        surface_format: wgpu::TextureFormat,
        size: (u32, u32),
        mesh: (u32, u32),
        max_segments: usize,
        shader_spec: Option<&shader::ShaderSpec>,
        shader_key: u64,
    ) -> Self {
        let common = build_common(device, size);
        let warp = build_warp(device, &common);
        let deposit = build_deposit(device, &common);
        let present = build_present(device, surface_format, &common);
        let draw = build_draw_layer(device, surface_format, max_segments);

        let Common {
            warp_shader,
            field,
            warp_uniform,
            deposit_uniform,
            present_uniform,
            ..
        } = common;
        let WarpParts {
            layout: warp_layout,
            pipeline: warp_pipeline,
            bg_a: warp_bg_a,
            bg_b: warp_bg_b,
        } = warp;
        let DepositParts {
            pipeline: deposit_pipeline,
            bg: deposit_bg,
            luts,
        } = deposit;
        let PresentParts {
            pipeline: present_pipeline,
            bg_a: present_bg_a,
            bg_b: present_bg_b,
        } = present;
        let DrawParts {
            lines,
            shape_pipeline,
            shape_over_pipeline,
            shape_vertices,
            shape_uniform,
            shape_bind_group,
            shape_capacity,
        } = draw;

        // The converted-shader surface, only when this preset carries WGSL
        // (Plan 0100 Phase 6). Built after everything above so the allocation
        // sequence up to here is byte-for-byte what a native preset builds.
        let milk_shaders = shader_spec.map(|spec| {
            shader::MilkShaderResources::build(
                device,
                queue,
                spec,
                &field,
                size,
                surface_format,
                &warp_shader,
                &warp_layout,
            )
        });

        let indices_data = build_indices(mesh);
        let indices = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("warp-mesh-indices"),
            size: (indices_data.len() * std::mem::size_of::<u32>()).max(4) as u64,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let vertices = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("warp-mesh-vertices"),
            size: (vertex_count(mesh) * std::mem::size_of::<Vertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            field,
            size,
            warp_pipeline,
            deposit_pipeline,
            present_pipeline,
            warp_uniform,
            deposit_uniform,
            present_uniform,
            warp_bg_a,
            warp_bg_b,
            present_bg_a,
            present_bg_b,
            deposit_bg,
            vertices,
            indices,
            index_count: indices_data.len() as u32,
            mesh,
            luts,
            lines,
            shape_pipeline,
            shape_over_pipeline,
            shape_vertices,
            shape_uniform,
            shape_bind_group,
            shape_capacity,
            needs_clear: true,
            milk_shaders,
            shader_key,
        }
    }

    /// Clear both halves of a freshly-allocated field. A texture's contents are
    /// undefined until written, and the warp pass samples the read half on its
    /// very first frame.
    pub(super) fn encode_clear(&self, encoder: &mut wgpu::CommandEncoder) {
        for view in [self.field.view_a(), self.field.view_b()] {
            gpu::color_pass(
                encoder,
                "warp-mesh-field-clear",
                view,
                wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
            );
        }
    }
}
