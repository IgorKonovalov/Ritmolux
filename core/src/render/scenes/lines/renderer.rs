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
/// Only one line scene draws per frame ([`scenes::shares_resources`] forbids
/// two), so "the most recent draw" is "this frame's figure".
///
/// [`scenes::shares_resources`]: crate::render::scenes
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
mod tests {
    // Tests index slices and panic on failure; allowed over the file's hot-path
    // pragma — this is not the render path.
    #![allow(clippy::indexing_slicing, clippy::panic, clippy::expect_used)]

    // -----------------------------------------------------------------------
    // The in-frame geometry fraction (Plan 0069 Phase 1, ADR-0083)
    // -----------------------------------------------------------------------

    use super::super::ViewTransform;
    use super::{DrawExtent, SegmentInstance, measure_extent};

    /// A segment with everything but its endpoints held constant — colour, width
    /// and the join flags are not part of this measurement (it is length, not
    /// area).
    fn seg(a: [f32; 2], b: [f32; 2]) -> SegmentInstance {
        SegmentInstance {
            a,
            b,
            color: [1.0, 1.0, 1.0],
            width: 0.01,
            joined: 0,
        }
    }

    /// Slack for the *clipped* cases only. The two endpoints of the property —
    /// wholly inside and wholly outside — are asserted **exactly** below, which
    /// is the point: they are the same sum and the empty sum respectively, not
    /// two nearly-equal ones.
    const EPS: f32 = 1e-6;

    /// A figure entirely inside the frame is **exactly** 1.0: no segment is
    /// clipped, so `len * 1.0` adds the same value to both sums.
    #[test]
    fn a_figure_inside_the_frame_measures_exactly_one() {
        let segments = [
            seg([-0.5, -0.5], [0.5, 0.5]),
            seg([0.2, -0.9], [1.5, 0.9]),
            seg([-1.55, 0.0], [1.55, 0.1]),
        ];
        let extent = measure_extent(&segments, 1.6, ViewTransform::default());
        assert!(extent.total_len > 0.0, "the fixture must draw something");
        assert_eq!(
            extent.in_frame_len, extent.total_len,
            "an unclipped figure must accumulate the identical sum twice"
        );
        assert_eq!(extent.fraction(), Some(1.0));
    }

    /// A figure entirely outside is **exactly** 0.0 — a real denominator over an
    /// empty numerator, which is what distinguishes it from "nothing drawn".
    #[test]
    fn a_figure_outside_the_frame_measures_exactly_zero() {
        let segments = [
            seg([2.0, 2.0], [3.0, 3.0]),
            seg([-4.0, 1.5], [4.0, 1.5]), // spans the width, above the top edge
        ];
        let extent = measure_extent(&segments, 1.0, ViewTransform::default());
        assert!(extent.total_len > 0.0, "the fixture must draw something");
        assert_eq!(extent.in_frame_len, 0.0);
        assert_eq!(extent.fraction(), Some(0.0));
    }

    /// A segment crossing an edge contributes its **clipped share**, not
    /// all-or-nothing — including the case both endpoints are outside, which is
    /// the one an endpoint test gets wrong and exactly what an over-scaled figure
    /// is made of.
    ///
    /// Every case is hand-computed against the unit frame (`aspect = 1.0`, so the
    /// rectangle is `[-1, 1]^2`):
    ///
    /// | segment                   | length | inside | fraction |
    /// |---------------------------|--------|--------|----------|
    /// | `(0,0) -> (4,0)`          | 4      | 1      | 0.25     |
    /// | `(-2,0) -> (2,0)`         | 4      | 2      | 0.5      |
    /// | `(-2,-2) -> (2,2)`        | √32    | √8     | 0.5      |
    /// | `(-2,0.5) -> (0.5,-2)`    | √12.5  | ·0.2   | 0.2      |
    /// | `(-2,1.5) -> (2,1.5)`     | 4      | 0      | 0        |
    #[test]
    fn a_crossing_segment_contributes_its_clipped_share() {
        let cases: [([f32; 2], [f32; 2], f32); 5] = [
            ([0.0, 0.0], [4.0, 0.0], 0.25),
            ([-2.0, 0.0], [2.0, 0.0], 0.5),
            ([-2.0, -2.0], [2.0, 2.0], 0.5),
            ([-2.0, 0.5], [0.5, -2.0], 0.2),
            ([-2.0, 1.5], [2.0, 1.5], 0.0),
        ];
        for (a, b, want) in cases {
            let extent = measure_extent(&[seg(a, b)], 1.0, ViewTransform::default());
            let got = extent.fraction().expect("the segment has a length");
            assert!(
                (got - want).abs() < EPS,
                "{a:?} -> {b:?} measured {got}, hand-computed {want}"
            );
        }
    }

    /// **The rectangle follows the aspect the caller hands in, and only that**
    /// ([ADR-0037](../../../../../docs/adrs/0037-internal-grid-is-a-resolution-not-a-shape.md)).
    ///
    /// `measure_extent` is a free function over the endpoints: there is no
    /// `self`, no texture and no internal grid in scope, so the aspect parameter
    /// is the *only* source of one — and `draw` passes the value it was handed,
    /// which is the render target's. This asserts the consequence, which is what
    /// a reader can check: the same segment measures 0.5 in a square frame and
    /// 1.0 in one twice as wide, while the **vertical** half-extent stays at 1.0
    /// whatever the aspect (the renderer maps two world units to the frame
    /// *height*).
    #[test]
    fn the_rectangle_follows_the_aspect_the_draw_is_handed() {
        let across = [seg([0.0, 0.0], [2.0, 0.0])];
        let up = [seg([0.0, 0.0], [0.0, 2.0])];
        let square = measure_extent(&across, 1.0, ViewTransform::default());
        let wide = measure_extent(&across, 2.0, ViewTransform::default());
        assert!((square.fraction().expect("length") - 0.5).abs() < EPS);
        assert_eq!(
            wide.fraction(),
            Some(1.0),
            "at aspect 2 the same horizontal segment fits exactly"
        );
        for aspect in [1.0, 2.0, 3.5] {
            let vertical = measure_extent(&up, aspect, ViewTransform::default());
            assert!(
                (vertical.fraction().expect("length") - 0.5).abs() < EPS,
                "the frame's half-height is 1.0 at every aspect, got {vertical:?} at {aspect}"
            );
        }
    }

    /// The view transform is part of what the frame shows, so it is part of the
    /// measurement: the shader moves endpoints by `a * zoom + pan` before the
    /// aspect divide, and a figure pushed off the top by `pan_y` has overshot
    /// just as surely as one scaled off it.
    #[test]
    fn the_view_transform_moves_the_figure_against_the_frame() {
        let segments = [seg([0.0, 0.0], [0.0, 0.5])];
        let zoomed = ViewTransform {
            zoom: 4.0,
            ..Default::default()
        };
        let panned = ViewTransform {
            pan: [0.0, 1.0],
            ..Default::default()
        };
        assert_eq!(
            measure_extent(&segments, 1.0, ViewTransform::default()).fraction(),
            Some(1.0)
        );
        let zoomed = measure_extent(&segments, 1.0, zoomed);
        assert!((zoomed.fraction().expect("length") - 0.5).abs() < EPS);
        assert_eq!(
            measure_extent(&segments, 1.0, panned).fraction(),
            Some(0.0),
            "panned up by a full half-height, the segment sits on and above the edge"
        );
    }

    /// Nothing drawn is **not** a fraction of zero. Plan 0058's table printed
    /// `inf` for a preset that drew nothing; this reports `None` and leaves the
    /// total case to `sanity.rs`.
    #[test]
    fn nothing_drawn_reports_no_fraction() {
        assert_eq!(DrawExtent::default().fraction(), None);
        assert_eq!(
            measure_extent(&[], 1.6, ViewTransform::default()).fraction(),
            None
        );
        // A degenerate segment is length zero wherever it sits, so it moves
        // neither sum — a figure collapsed to a point is `sanity.rs`'s question.
        let degenerate = [seg([9.0, 9.0], [9.0, 9.0])];
        assert_eq!(
            measure_extent(&degenerate, 1.6, ViewTransform::default()).fraction(),
            None
        );
    }

    // -----------------------------------------------------------------------
    // The stroke seam does not punch holes in the backdrop (Plan 0051 Phase 2)
    // -----------------------------------------------------------------------

    /// The lit-backdrop fixture this guard captures three ways. Its `bg_bright`
    /// and `draw_progress` lines are **stripped and rewritten** per capture — one
    /// scene at three configurations — so the numbers are read back out of the
    /// file rather than restated here, and editing the fixture moves the test
    /// with it. Read its header before touching it: `thickness = 9` is what makes
    /// the defect measurable at all.
    const LIT_FIXTURE: &str = include_str!("../../../../tests/fixtures/lines_lit_backdrop.toml");

    /// The square capture size — twice the swarm guard's, and an exact multiple
    /// of the post chain's 256 px grid step so the trails stage runs 1:1 with the
    /// target.
    ///
    /// The feature under test here is a **rim a fraction of a stroke width
    /// across**, not a sprite corner, so it needs the pixels: at 256 the fat
    /// stroke is only 7 px wide and there is little left of its edge to measure.
    /// Same argument `line_joint_zigzag.toml` makes for its own 512.
    const CAPTURE_SIZE: u32 = 512;

    /// Frames per capture. The fixture is frozen (`spin = 0`), so this only
    /// clears the draw-in and lets the trail history settle.
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

    /// Slack for half-precision rounding, the same shape the swarm guard and
    /// `bloom.rs`'s use. The composite is `Rgba16Float`, so a value of magnitude
    /// `m` is stored to roughly `m / 1024`, and the lit capture quantizes a
    /// different sum than the backdrop-only one does.
    ///
    /// It is slack, not a tolerance: the property below is **exact** in real
    /// arithmetic. Measured on this fixture, the fixed shader's worst `|L - B|`
    /// is **0.0000** across 0 channels; the pre-fix one's is **0.4944** — very
    /// nearly the backdrop's own `bg_bright`, i.e. discarded outright — across 15
    /// channels. The magnitude is unambiguous; the *count* is small, and the
    /// comment on `lit_mask` below says why that is geometry rather than a weak
    /// fixture.
    fn half_slack(value: f32) -> f32 {
        (4.0 / 1024.0) * value.abs().max(1.0)
    }

    /// **Where the strokes drew no light, the backdrop arrives intact** — the
    /// same guard `swarm.rs` installs, on the other draw seam.
    ///
    /// `fs_main` used to return `vec4(in.color * g * u.v.y, 1.0)`: colour carried
    /// the across-the-stroke falloff, alpha was a literal constant. With the
    /// alpha blend at `BlendComponent::OVER` and a source alpha of exactly 1,
    /// destination alpha saturated across the whole stroke quad — including the
    /// two long edges where the falloff reaches zero and the shader wrote
    /// nothing. The chain resolves `src.rgb + backdrop * (1 - src.a)`
    /// (ADR-0055), so those edges discarded the backdrop and rendered as black
    /// rims and wedges over the figure.
    ///
    /// **This is the quiet seam of the two, and that is a geometric fact rather
    /// than a difference in kind.** The swarm's falloff is radial over a *square*
    /// quad, so its zero-colour region is four large hard-edged corners; the
    /// line's is one-dimensional across the stroke, so its zero-colour region is
    /// a rim whose width scales with `thickness`. At shipped widths that rim is
    /// nearly a hairline — which is why the swarm was reported and this was not,
    /// and why the fixture uses a deliberately fat stroke.
    ///
    /// One edit and one guard cover all four line scenes: `parametric_curve`,
    /// `lsystem`, `star_pattern` and `spectrum` all stroke through this renderer.
    ///
    /// See the swarm guard for why this reads the **linear** composite rather
    /// than the capture (the tonemap scales every channel off the brightest one,
    /// so no byte-level tolerance separates the defect from the curve behaving as
    /// designed) and why it therefore lives in the render module rather than in
    /// `core/tests/`.
    #[test]
    fn a_lit_backdrop_survives_where_the_strokes_drew_nothing() {
        use crate::dsp::AnalysisFrame;
        use crate::preset::Preset;
        use crate::render::capture;
        use crate::render::context::RenderError;
        use crate::render::{HeadlessOptions, Renderer};

        // --- Non-vacuity, before any GPU work: the fixture must still describe
        // the configuration this guard exists for. ---
        let backdrop = fixture_value("bg_bright");
        let progress = fixture_value("draw_progress");
        let thickness = fixture_value("thickness");
        let trails = fixture_value("trails");
        assert!(
            backdrop > 0.0,
            "lines_lit_backdrop.toml no longer ships a lit backdrop (bg_bright = \
             {backdrop}); on black this whole comparison is black against black"
        );
        assert!(
            progress > 0.0,
            "lines_lit_backdrop.toml no longer draws the curve (draw_progress = \
             {progress})"
        );
        assert!(
            thickness >= 6.0,
            "lines_lit_backdrop.toml is down to thickness = {thickness}. The dark \
             region this guards is a RIM whose width scales with the stroke, and \
             at shipped widths (2 to 3) it is close to a hairline that a capture \
             cannot discriminate — a thin fixture leaves this test green and \
             blind. See the file's header"
        );
        assert!(
            trails > 0.0,
            "lines_lit_backdrop.toml no longer binds `trails` (= {trails}), so no \
             post stage is active. With an empty chain the scene draws straight \
             onto the backdrop and its additive colour cannot remove light — the \
             defect is unrepresentable and this test proves nothing"
        );

        /// The linear composite the tonemap is about to map, at a given backdrop
        /// brightness and reveal fraction.
        ///
        /// Builds and drops **one** renderer per call rather than holding three:
        /// a second live device in a binary is what the software adapter falls
        /// over on, and building GPU resources mid-run shifts what the trails
        /// stage resolves to on WARP.
        fn linear_composite(bg_bright: f32, draw_progress: f32) -> Option<Vec<f32>> {
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
                    !line.starts_with("bg_bright") && !line.starts_with("draw_progress")
                })
                .collect::<Vec<_>>()
                .join("\n");
            let toml = format!(
                "{base}\nbg_bright = \"{bg_bright}\"\ndraw_progress = \"{draw_progress}\"\n"
            );
            let preset = Preset::from_toml_str(&toml)
                .expect("the lit-backdrop line fixture parses with overrides");
            let name = preset.name.clone();
            renderer.set_presets(vec![preset]);

            // Every binding is a constant, so the analysis frame only has to be
            // well-formed — nothing in this fixture reads it.
            let frame = AnalysisFrame::default();
            renderer
                .capture_preset(&name, &frame, CAPTURE_FRAMES)
                .expect("capture the lit-backdrop line fixture");

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
                label: Some("lines-backdrop-readback"),
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
        // backdrop with the scene contributing nothing — at `draw_progress = 0`
        // the curve yields zero segments and the renderer returns without a draw
        // call, so this is the backdrop alone through the same pipeline as `L`.
        let Some(lit) = linear_composite(backdrop, progress) else {
            return;
        };
        let Some(dark) = linear_composite(0.0, progress) else {
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
        // Which pixels the scene put light into, so the report below can say how
        // much of the untouched region actually **borders** the geometry. That
        // sub-count is the part of the domain a broken alpha can reach: the rest
        // is open backdrop no quad ever covered, which passes either way.
        //
        // It is deliberately reported and not asserted on. Unlike the swarm's
        // corners — a 2-D region where the radial falloff is identically zero
        // over ~21 % of every sprite — the line falloff is one-dimensional and
        // *quadratic*, so its exactly-zero band is the outermost sub-pixel sliver
        // of the quad and only a handful of sample points land in it. The rim the
        // eye sees is the much wider band where coverage is near zero rather than
        // zero. That is a property of the geometry, not of this fixture, and no
        // choice of `samples`/`scale`/`thickness` widens it.
        let lit_mask: Vec<bool> = dark
            .chunks_exact(4)
            .map(|texel| texel[0] != 0.0 || texel[1] != 0.0 || texel[2] != 0.0)
            .collect();
        let borders_geometry = |pixel: usize| -> bool {
            let (w, h) = (CAPTURE_SIZE as usize, CAPTURE_SIZE as usize);
            let (x, y) = (pixel % w, pixel / w);
            let mut neighbours = Vec::with_capacity(4);
            if x > 0 {
                neighbours.push(pixel - 1);
            }
            if x + 1 < w {
                neighbours.push(pixel + 1);
            }
            if y > 0 {
                neighbours.push(pixel - w);
            }
            if y + 1 < h {
                neighbours.push(pixel + w);
            }
            neighbours.iter().any(|&n| lit_mask[n])
        };

        let (mut untouched, mut drawn, mut over_backdrop, mut on_the_rim) =
            (0usize, 0usize, 0usize, 0usize);
        let (mut violations, mut worst) = (0usize, 0.0f32);
        for (pixel, texel) in dark.chunks_exact(4).enumerate() {
            if texel[0] != 0.0 || texel[1] != 0.0 || texel[2] != 0.0 {
                drawn += 1;
                continue; // the scene put light here; the property says nothing
            }
            untouched += 1;
            if borders_geometry(pixel) {
                on_the_rim += 1;
            }
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
            "lines lit backdrop at {CAPTURE_SIZE}x{CAPTURE_SIZE}: {untouched} of \
             {total} pixels untouched by the scene ({over_backdrop} of those over \
             a lit backdrop, {on_the_rim} of them bordering geometry), {drawn} lit \
             by it; worst |L - B| {worst:.4}"
        );

        // --- Non-vacuity: the region the property speaks about is a substantial
        // part of the frame, the strokes genuinely drew into the rest, and the
        // backdrop genuinely reached the frame underneath. A fixture edit that
        // quietly empties any of the three shows up here rather than passing. ---
        assert!(
            untouched * 4 > total,
            "only {untouched} of {total} pixels are untouched by the scene — the \
             fixture has filled the frame and the property covers almost nothing"
        );
        assert!(
            drawn * 50 > total,
            "only {drawn} of {total} pixels carry any stroke light — the fixture \
             has stopped drawing, so the stroke rims this guards are not in the \
             frame"
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
             alone at pixels where the strokes wrote NO light (worst {worst:.4}). \
             Upstream of the tonemap this is a plain premultiplied OVER, so where \
             nothing was drawn the backdrop must arrive intact — a difference \
             here is a stroke emitting coverage it does not have, rimming itself \
             in backdrop it never painted over"
        );
    }
}
