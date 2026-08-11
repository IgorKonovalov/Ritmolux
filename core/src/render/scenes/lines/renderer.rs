//! The shared line primitive: a GPU helper that draws thick, glowing lines as
//! instanced camera-facing quads. Each [`SegmentInstance`] (two endpoints, a
//! colour, a half-width) is expanded in the vertex shader into a quad whose
//! width is uniform *on screen* — the swarm scene's instanced-quad pipeline
//! (ADR-0007) with segments in place of points. Additive blend, so overlapping
//! and dense strokes bloom.
//!
//! Native wgpu line primitives are deliberately not used: their width is locked
//! near 1px and varies by backend (ADR-0007). The buffer is fixed-capacity and
//! reused every frame, so a full curve upload never allocates on the hot path.

// Hot-path panic-denial pragma (Plan 0002 Phase 2, extended to scenes by Plan
// 0003 Phase 0). `draw` runs every displayed frame.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

/// [`SegmentInstance::joined`] bit: the `a` end continues a neighbouring
/// segment, so the quad extends **backward** along its own direction by the
/// half-width (ADR-0041).
pub const JOINED_A: u32 = 1 << 0;
/// [`SegmentInstance::joined`] bit: the `b` end continues a neighbouring
/// segment, so the quad extends **forward** by the half-width (ADR-0041).
pub const JOINED_B: u32 = 1 << 1;

/// One line segment: endpoints `a`/`b` in world space (x is divided by aspect
/// in the shader, matching the swarm's convention), an RGB colour, a
/// half-width in NDC-y units (uniform on screen after the aspect divide), and
/// the per-endpoint connectivity the join needs.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SegmentInstance {
    /// First endpoint (world space).
    pub a: [f32; 2],
    /// Second endpoint (world space).
    pub b: [f32; 2],
    /// RGB colour (pre-brightness; additive blend sums overlaps).
    pub color: [f32; 3],
    /// Half-width in NDC-y units.
    pub width: f32,
    /// Per-endpoint join flags — [`JOINED_A`] and/or [`JOINED_B`] (ADR-0041).
    ///
    /// Connectivity is the **producer's** to declare: only it knows whether an
    /// end is shared with a neighbour. `0` (neither end joined) renders exactly
    /// the pre-Plan-0039 geometry, which is what keeps the isolated producers —
    /// `spectrum`'s `Bars` and `RadialRing` — byte-identical.
    ///
    /// A bitfield rather than two `f32`s: 4 bytes instead of 8 against
    /// ADR-0007's fixed-capacity instance buffer, still `Pod` (32 + 4 = 36 with
    /// no padding at align 4), and it leaves 30 bits for whatever the next
    /// per-endpoint property turns out to be.
    pub joined: u32,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    // x: aspect, y: glow multiplier, zw: unused
    v: [f32; 4],
    // x: zoom, yz: pan, w: unused — the shared ViewTransform (ADR-0018)
    view: [f32; 4],
}

/// The WGSL body, minus the join-bit constants — [`shader_source`] prepends
/// those, generated from [`JOINED_A`] / [`JOINED_B`] themselves.
const SHADER_BODY: &str = r#"
struct Uniforms {
    v: vec4<f32>,
    view: vec4<f32>,
}

@group(0) @binding(0) var<uniform> u: Uniforms;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) side: f32,
    @location(1) color: vec3<f32>,
}

@vertex
fn vs_main(
    @builtin(vertex_index) vi: u32,
    @location(0) a: vec2<f32>,
    @location(1) b: vec2<f32>,
    @location(2) color: vec3<f32>,
    @location(3) width: f32,
    @location(4) joined: u32,
) -> VsOut {
    // (along, side): along runs a->b, side spans -1..1 across the width.
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, -1.0), vec2<f32>(1.0, -1.0), vec2<f32>(0.0, 1.0),
        vec2<f32>(0.0, 1.0), vec2<f32>(1.0, -1.0), vec2<f32>(1.0, 1.0),
    );
    let c = corners[vi];
    let aspect = max(u.v.x, 0.1);
    let inv_aspect = 1.0 / aspect;

    // Shared ViewTransform (ADR-0018): zoom about the frame centre, then pan, in
    // world space before the aspect divide. Endpoints move; stroke width does not.
    let zoom = u.view.x;
    let pan = u.view.yz;
    let a_v = a * zoom + pan;
    let b_v = b * zoom + pan;

    // Work in aspect-corrected space so the perpendicular offset is a uniform
    // on-screen thickness whatever the segment's orientation.
    let a_s = vec2<f32>(a_v.x * inv_aspect, a_v.y);
    let b_s = vec2<f32>(b_v.x * inv_aspect, b_v.y);
    var dir = b_s - a_s;
    let len = length(dir);
    if (len > 1e-6) {
        dir = dir / len;
    } else {
        dir = vec2<f32>(1.0, 0.0);
    }
    let nrm = vec2<f32>(-dir.y, dir.x);

    // Join (ADR-0041): a flagged end continues into a neighbouring segment, so
    // push the quad past that endpoint by the half-width along its **own**
    // direction. Adjacent quads then overlap by half a stroke on both sides of
    // the shared vertex and the additive falloff fills the wedge the two
    // divergent perpendiculars would otherwise leave. Each end is independent,
    // and an unflagged end keeps its exact previous geometry — `dir * 0.0` is
    // exactly zero, so a producer that flags nothing is byte-identical.
    let ext_a = select(0.0, width, (joined & JOINED_A) != 0u);
    let ext_b = select(0.0, width, (joined & JOINED_B) != 0u);
    let a_j = a_s - dir * ext_a;
    let b_j = b_s + dir * ext_b;

    let base = mix(a_j, b_j, c.x);
    let pos = base + nrm * c.y * width;

    var out: VsOut;
    out.pos = vec4<f32>(pos, 0.0, 1.0);
    out.side = c.y;
    out.color = color;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    // Bright core, quadratic falloff to the quad edge: a soft glowing stroke.
    let d = abs(in.side);
    let falloff = max(0.0, 1.0 - d);
    let g = falloff * falloff;
    // Premultiplied: colour AND alpha carry the same coverage `g`, so the two
    // long edges of the quad - where the across-the-stroke falloff reaches zero
    // - write nothing at all rather than opaque black (ADR-0056). Note the glow
    // multiplier scales the LIGHT, not the coverage: a dimmed stroke still
    // covers its own footprint. See `gpu::ADDITIVE_LIGHT_SATURATING_COVERAGE`.
    return vec4<f32>(in.color * g * u.v.y, g);
}
"#;

/// The full WGSL, with the join bits **generated from the Rust constants**
/// rather than restated as literals (Plan 0040 Phase 2).
///
/// The shader used to test `(joined & 1u)` and `(joined & 2u)` against
/// [`JOINED_A`] / [`JOINED_B`] defined here, with nothing tying the two together
/// — a swap or a renumbering would have compiled, passed, and rendered wrongly.
/// Emitting the WGSL `const`s from the Rust ones makes that divergence
/// **unrepresentable** rather than merely detected: there is one definition, and
/// the shader reads it by name. Prepending a generated prelude rather than
/// `format!`-ing the whole body is deliberate — the body is full of braces, and
/// every one would need escaping.
///
/// Runs once per [`LineRenderer::new`] (pipeline build, not the hot path).
fn shader_source() -> String {
    format!("const JOINED_A: u32 = {JOINED_A}u;\nconst JOINED_B: u32 = {JOINED_B}u;\n{SHADER_BODY}")
}

// ---------------------------------------------------------------------------
// The in-frame geometry diagnostic (Plan 0069, ADR-0083)
// ---------------------------------------------------------------------------

/// How much of the drawn segment length landed inside the render target, summed
/// over one [`LineRenderer::draw`] call (Plan 0069, [ADR-0083]).
///
/// Pixel coverage cannot see an over-scaled figure: a comb roots every bar on a
/// shared baseline and a corona roots every spoke at a centre, so clipping the
/// tips costs a rounding error of lit pixels and the statistic goes the *wrong
/// way*. Length does see it — a bar that overshoots loses in-frame length in
/// exact proportion to the overshoot.
///
/// **Length, not area.** The stroke's width and the ADR-0041 join extensions are
/// not counted, so a thick stroke leaving the frame is under-counted. That is
/// the right measure for *overshoot* and a poor one for anything else.
///
/// [ADR-0083]: ../../../../../docs/adrs/0083-in-frame-geometry-is-measured-at-the-line-renderers-draw-seam.md
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DrawExtent {
    /// World-space length of every segment actually drawn (post view transform).
    pub total_len: f32,
    /// The share of that length lying inside `[-aspect, aspect] x [-1, 1]`.
    pub in_frame_len: f32,
}

impl DrawExtent {
    /// The in-frame fraction — exactly `1.0` when nothing was clipped, exactly
    /// `0.0` when the whole figure is outside.
    ///
    /// `None` when nothing was drawn at all: that is a `0/0`, and inventing a
    /// number for it is what made Plan 0058's table print `inf`. "Nothing drawn"
    /// is the *total* case and `core/tests/sanity.rs` is its instrument, not this
    /// one.
    pub fn fraction(self) -> Option<f32> {
        (self.total_len > 0.0).then(|| self.in_frame_len / self.total_len)
    }
}

// Thread-local rather than a field on `LineRenderer`, because the four line
// scenes reach the one shared renderer through an `Rc<RefCell<..>>` owned by the
// scene registry and nothing outside `render` holds a handle to it — see
// `scenes::create_all`. Thread-local rather than a global: the renderer is
// single-threaded by construction (`Rc`), so this is the cheapest correct sink,
// and it also keeps one test's switch out of another's capture when the harness
// runs test threads in parallel.
thread_local! {
    /// Whether `draw` measures. **Off in the shipped render path** — that is the
    /// whole of the switch, and `core/tests/geometry_extent.rs` asserts "off"
    /// means byte-identical output.
    static EXTENT_ON: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// The most recent measured draw, if any.
    static LAST_EXTENT: std::cell::Cell<Option<DrawExtent>> = const { std::cell::Cell::new(None) };
}

/// Turn the in-frame geometry diagnostic on or off for **this thread**, clearing
/// any previously recorded measurement. Off by default; the shipped render path
/// never calls this.
pub fn set_extent_diagnostic(on: bool) {
    EXTENT_ON.with(|flag| flag.set(on));
    LAST_EXTENT.with(|slot| slot.set(None));
}

/// Take the extent of the **most recent** measured `draw`, leaving the slot
/// empty. `None` when no line scene has drawn since the diagnostic was enabled
/// (or when it is off) — distinct from a recorded draw whose
/// [`fraction`](DrawExtent::fraction) is `None` because nothing was drawn.
///
/// A frame usually holds one line draw ([`scenes::shares_resources`] forbids
/// two *roster* line scenes in a frame), and then "the most recent draw" is
/// "this frame's figure". Since Plan 0076 a preset may layer a second line
/// scene through its own per-preset `LineRenderer`
/// ([`scenes::create_layer_scene`]) — the layer draws **after** the main
/// scene, so on a layered line-on-line preset this slot holds the *layer's*
/// figure. The harness reads this around single-figure captures; a consumer
/// measuring a layered preset must know which draw it is measuring.
///
/// [`scenes::shares_resources`]: crate::render::scenes
/// [`scenes::create_layer_scene`]: crate::render::scenes::create_layer_scene
pub fn take_draw_extent() -> Option<DrawExtent> {
    LAST_EXTENT.with(|slot| slot.take())
}

/// The share of the segment `a -> b` lying inside `[-aspect, aspect] x [-1, 1]`,
/// as a fraction of its own length: Liang-Barsky against the four edges.
///
/// **Exactly `1.0`** when the segment is untouched by any edge — the two
/// parameters start at `0.0` and `1.0` and no edge moves them — which is what
/// lets an unclipped figure sum to exactly its own total. **Exactly `0.0`** when
/// the segment is wholly outside.
///
/// A parametric clip rather than an endpoint test on purpose: the case a naive
/// "are both ends outside" check gets wrong is a segment whose ends are both out
/// but which crosses the frame between them, and that is precisely what a badly
/// over-scaled figure is made of.
fn in_frame_fraction(a: [f32; 2], b: [f32; 2], aspect: f32) -> f32 {
    let [ax, ay] = a;
    let [bx, by] = b;
    let (dx, dy) = (bx - ax, by - ay);
    let (mut t0, mut t1) = (0.0f32, 1.0f32);
    // (direction, distance) per edge: left, right, bottom, top.
    for (p, q) in [
        (-dx, ax + aspect),
        (dx, aspect - ax),
        (-dy, ay + 1.0),
        (dy, 1.0 - ay),
    ] {
        if p == 0.0 {
            // Parallel to this edge: wholly out if it starts outside it.
            if q < 0.0 {
                return 0.0;
            }
            continue;
        }
        let r = q / p;
        if p < 0.0 {
            if r > t1 {
                return 0.0;
            }
            if r > t0 {
                t0 = r;
            }
        } else {
            if r < t0 {
                return 0.0;
            }
            if r < t1 {
                t1 = r;
            }
        }
    }
    t1 - t0
}

/// Measure `segments` against the frame — the diagnostic's whole computation.
///
/// **The aspect is a parameter, and it is the only source of one in here**
/// ([ADR-0037](../../../../../docs/adrs/0037-internal-grid-is-a-resolution-not-a-shape.md)):
/// this is a free function over the endpoints, so there is no internal grid, no
/// texture and no `self` for a second aspect to come from. Its caller hands it
/// the value `draw` was handed, which is the **render target's**.
///
/// The view transform is applied first, exactly as the vertex shader applies it
/// (`a * zoom + pan`, before the aspect divide), because a figure pushed off the
/// frame by `zoom` or `pan_y` has overshot just as surely as one scaled off it.
fn measure_extent(
    segments: &[SegmentInstance],
    aspect: f32,
    xform: super::ViewTransform,
) -> DrawExtent {
    let mut extent = DrawExtent::default();
    let [pan_x, pan_y] = xform.pan;
    for segment in segments {
        let [ax, ay] = segment.a;
        let [bx, by] = segment.b;
        let a = [ax * xform.zoom + pan_x, ay * xform.zoom + pan_y];
        let b = [bx * xform.zoom + pan_x, by * xform.zoom + pan_y];
        let ([ax, ay], [bx, by]) = (a, b);
        let (dx, dy) = (bx - ax, by - ay);
        let len = (dx * dx + dy * dy).sqrt();
        if len <= 0.0 || !len.is_finite() {
            continue; // a degenerate (or non-finite) segment measures nothing
        }
        extent.total_len += len;
        // `len * 1.0` is `len` exactly, so an unclipped figure adds the same
        // value to both sums and the fraction is exactly 1.0.
        extent.in_frame_len += len * in_frame_fraction(a, b, aspect);
    }
    extent
}

/// Draws segment buffers as thick glowing quads. Owns its pipeline, a
/// fixed-capacity instance buffer, and the aspect/glow uniform.
pub struct LineRenderer {
    pipeline: wgpu::RenderPipeline,
    instances: wgpu::Buffer,
    uniforms: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    /// Maximum segments the instance buffer holds; extra are dropped by `draw`.
    capacity: usize,
}

impl LineRenderer {
    /// Build the pipeline and a `capacity`-segment instance buffer on `device`.
    /// `label` names this instance's GPU resources; it must be **unique per
    /// LineRenderer** — two line scenes coexist (parametric + generator), and
    /// distinct labels keep their pipelines/buffers unambiguous in tooling and
    /// captures.
    pub fn new(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        capacity: usize,
        label: &str,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(&format!("{label}-shader")),
            source: wgpu::ShaderSource::Wgsl(shader_source().into()),
        });
        let instances = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(&format!("{label}-instances")),
            size: (capacity * std::mem::size_of::<SegmentInstance>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(&format!("{label}-uniforms")),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some(&format!("{label}-bind-layout")),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(&format!("{label}-bind-group")),
            layout: &bind_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniforms.as_entire_binding(),
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some(&format!("{label}-pipeline-layout")),
            bind_group_layouts: &[Some(&bind_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(&format!("{label}-pipeline")),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<SegmentInstance>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &wgpu::vertex_attr_array![
                        0 => Float32x2,
                        1 => Float32x2,
                        2 => Float32x3,
                        3 => Float32,
                        4 => Uint32,
                    ],
                })],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    // Additive light, saturating coverage (ADR-0056) — the same
                    // constant the swarm's sprite pipeline takes, so the two
                    // draw seams cannot drift apart.
                    blend: Some(crate::render::gpu::ADDITIVE_LIGHT_SATURATING_COVERAGE),
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
            uniforms,
            bind_group,
            capacity,
        }
    }

    /// Segments the instance buffer can hold — the scene clamps its geometry to
    /// this and surfaces any drop at load (ADR-0007 cap must never be silent).
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Draw `segments` as thick glowing quads at the given `aspect` and `glow`
    /// multiplier, under the shared `xform` camera transform (zoom/pan, ADR-0018),
    /// **loading** over the engine backdrop rather than clearing (Plan 0018 Phase
    /// 3 — the background pass owns the clear). Segments beyond `capacity` are
    /// dropped defensively (the scene is responsible for capping at load).
    #[allow(
        clippy::too_many_arguments,
        reason = "distinct GPU handles plus the per-frame draw parameters (aspect, glow, \
                  view transform); bundling them would only shuffle the same values behind a \
                  one-use struct"
    )]
    pub fn draw(
        &mut self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        aspect: f32,
        glow: f32,
        xform: super::ViewTransform,
        segments: &[SegmentInstance],
    ) {
        let count = segments.len().min(self.capacity);
        let drawn = segments.get(..count).unwrap_or(&[]);

        // The in-frame geometry diagnostic (Plan 0069, ADR-0083). Off in the
        // shipped path, and it reads `drawn` — the segments that actually reach
        // the instance buffer — without touching a GPU resource, so "off" is a
        // `Cell::get` and "on" changes nothing about the picture. The aspect it
        // measures against is the one this call was handed: the render target's
        // (ADR-0037), under the same `max(0.1)` clamp the uniform and the shader
        // apply, so the rectangle is the one the frame actually shows.
        if EXTENT_ON.with(std::cell::Cell::get) {
            let extent = measure_extent(drawn, aspect.max(0.1), xform);
            LAST_EXTENT.with(|slot| slot.set(Some(extent)));
        }

        if !drawn.is_empty() {
            queue.write_buffer(&self.instances, 0, bytemuck::cast_slice(drawn));
        }
        queue.write_buffer(
            &self.uniforms,
            0,
            bytemuck::bytes_of(&Uniforms {
                v: [aspect.max(0.1), glow, 0.0, 0.0],
                view: [xform.zoom, xform.pan[0], xform.pan[1], 0.0],
            }),
        );

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("line-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    // Load over the engine backdrop (ADR-0018); additive strokes
                    // bloom over it and the empty space reveals it.
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        if drawn.is_empty() {
            return; // nothing to stroke; the backdrop shows through
        }
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.set_vertex_buffer(0, self.instances.slice(..));
        pass.draw(0..6, 0..drawn.len() as u32);
    }
}

#[cfg(test)]
mod tests;
