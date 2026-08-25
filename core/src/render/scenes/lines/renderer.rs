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
    /// ADR-0007's fixed-capacity instance buffer, still `Pod` (36 + 4 = 40 with
    /// no padding at align 4), and it leaves 30 bits for whatever the next
    /// per-endpoint property turns out to be.
    pub joined: u32,
    /// **How much of its own footprint this stroke occupies**, on top of the
    /// across-the-stroke falloff — the fragment's alpha is `falloff * alpha`.
    ///
    /// `1.0` for every producer that draws through the additive seam, and that is
    /// the value to pass unless you know otherwise: ADR-0056's rule is that a
    /// dimmed stroke still covers its own footprint, so brightness belongs in
    /// [`color`](Self::color) and never here. Every line scene in this crate
    /// passes `1.0`, which makes the fragment exactly what it was before this
    /// field existed.
    ///
    /// What it is for is [`LineRenderer::draw_split`]'s second range, where the
    /// stroke is composited **over** rather than added and the blend needs the
    /// producer's real coverage: a MilkDrop waveform at `wave_a = 0.1` must
    /// replace a tenth of what is under it, not all of it (Plan 0100 Phase 4).
    ///
    /// **Declared last, and that is load-bearing.** `vertex_attr_array!` derives
    /// each attribute's byte offset from the order of the *shader locations*, so a
    /// field inserted before `joined` puts location 4 (`Uint32`) over this float
    /// and location 5 over the flags. The result compiles, renders, and quietly
    /// reinterprets every stroke's join bits as alpha's mantissa — which is what
    /// it did, moving five composite golden baselines.
    pub alpha: f32,
}

/// One **circular arc**: a centre and radius in world space, a signed angular
/// span in radians, an RGB colour and a half-width in NDC-y units — the same
/// conventions [`SegmentInstance`] uses, so the two kinds place geometry in one
/// coordinate system and stroke it to one profile.
///
/// Expanded in the vertex shader to a single bounding quad and shaded by the
/// **per-pixel distance to the arc** ([ADR-0098]) rather than by an
/// interpolated across-the-stroke coordinate. So a `circle` is one instance
/// with no vertices at any resolution, where the segment path needs one
/// instance and one additive joint per sample.
///
/// **No `joined` field: an arc has no interior joints**, which is the whole
/// point of the primitive. Where two arcs in a chain meet they overlap by
/// [ADR-0041]'s half-width as any two strokes do, and the additive composite
/// sums that overlap exactly as it does for segments — the bead is reduced by
/// there being fewer joints, not by a joint doing anything different.
///
/// **No `alpha` field either.** Every arc producer draws through ADR-0056's
/// additive seam, where the coverage a premultiplied fragment carries is the
/// stroke's own falloff; the OVER range [`LineRenderer::draw_split`] serves is
/// a MilkDrop waveform, which is segments. A future over-blended arc adds the
/// field **last**, for the reason [`SegmentInstance::alpha`] records.
///
/// **Field order is shader-location order**, and that is load-bearing for the
/// same reason it is on [`SegmentInstance`]: `vertex_attr_array!` derives each
/// attribute's byte offset from the order of the locations, so a field inserted
/// anywhere but the end silently re-points every attribute after it.
///
/// [ADR-0098]: ../../../../../docs/adrs/0098-the-line-renderer-draws-arcs-as-per-pixel-distance-fields.md
/// [ADR-0041]: ../../../../../docs/adrs/0041-line-joins-are-per-endpoint-on-the-segment-instance.md
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ArcInstance {
    /// Centre of curvature (world space).
    pub centre: [f32; 2],
    /// Radius (world space). The arc's centreline, not either stroke edge.
    pub radius: f32,
    /// Where the span starts, in radians, measured the usual way from `+x`.
    pub angle_start: f32,
    /// How far it sweeps, **signed**. `|sweep|` may exceed `PI`, and a full
    /// circle is one instance at `sweep = TAU`.
    pub angle_sweep: f32,
    /// RGB colour (pre-brightness; additive blend sums overlaps).
    pub color: [f32; 3],
    /// Half-width in NDC-y units — the same quantity, in the same space, as
    /// [`SegmentInstance::width`]. Named `width` to match its sibling; it is a
    /// half-width in both.
    pub width: f32,
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
    @location(2) alpha: f32,
}

@vertex
fn vs_main(
    @builtin(vertex_index) vi: u32,
    @location(0) a: vec2<f32>,
    @location(1) b: vec2<f32>,
    @location(2) color: vec3<f32>,
    @location(3) width: f32,
    @location(4) joined: u32,
    @location(5) alpha: f32,
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
    out.alpha = alpha;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    // Bright core, quadratic falloff to the quad edge: a soft glowing stroke.
    let d = abs(in.side);
    let falloff = max(0.0, 1.0 - d);
    let g = falloff * falloff;
    // Premultiplied: colour AND alpha carry the same coverage `g * alpha`, so
    // the two long edges of the quad - where the across-the-stroke falloff
    // reaches zero - write nothing at all rather than opaque black (ADR-0056).
    // Note the glow multiplier scales the LIGHT, not the coverage: a dimmed
    // stroke still covers its own footprint. See
    // `gpu::ADDITIVE_LIGHT_SATURATING_COVERAGE`.
    //
    // `alpha` is 1.0 for every additive producer, so this is byte-identical to
    // the pre-Plan-0100 fragment for all of them; it is the OVER pipeline's
    // second range that passes anything else. The colour is NOT divided by it -
    // an over-blended producer arrives premultiplied already.
    return vec4<f32>(in.color * g * u.v.y, g * in.alpha);
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

/// The arc pipeline's WGSL, **a separate shader module** from [`SHADER_BODY`]
/// and built only when a scene asked for arcs.
///
/// Separate rather than two more entry points in the one module, so a
/// `LineRenderer` without arcs creates exactly the resources it created before
/// this existed. Appending to the shared module would have changed what every
/// line scene compiles, and on the WARP software adapter the golden suite
/// captures on, a changed resource is a changed picture (ADR-0058).
///
/// # What the fragment computes, and in which space
///
/// The arc is authored in **world space**, which is isotropic on screen: the
/// vertex shader divides x by the aspect on the way out, so a world circle is a
/// circle in pixels. The **stroke**, though, is a half-width in NDC - that is
/// what the segment path strokes (`nrm * width` is applied after the aspect
/// divide), and this primitive has to draw the same picture as a densely
/// sampled polyline of the same arc, not a better one.
///
/// So the distance is taken where each half is exact and converted once:
/// `abs(length(p - c) - r)` is the exact **world** radial distance, and
/// dividing by the NDC length of that distance's own gradient expresses it in
/// the NDC metric the stroke is measured in. Outside the angular span the
/// distance is to the nearer endpoint, which is a point, so that arm
/// approximates nothing.
///
/// **The aspect is the render target's** ([ADR-0037]): it arrives in the
/// uniform `draw` was handed, and there is no internal grid, texture or second
/// size anywhere in this shader for another one to come from. This family has
/// shipped that bug three times, which is why the control renders at a
/// non-16:9 target where a grid-derived aspect and the target's disagree.
///
/// [ADR-0037]: ../../../../../docs/adrs/0037-internal-grid-is-a-resolution-not-a-shape.md
const ARC_SHADER: &str = r#"
struct Uniforms {
    v: vec4<f32>,
    view: vec4<f32>,
}

@group(0) @binding(0) var<uniform> u: Uniforms;

const TAU: f32 = 6.2831853071795864;
const QUARTER_TURN: f32 = 1.5707963267948966;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) ndc: vec2<f32>,
    @location(1) @interpolate(flat) centre: vec2<f32>,
    @location(2) @interpolate(flat) radius: f32,
    // (lo, hi): the span's two ends in increasing order, so the sweep's sign
    // stops mattering past this point and the two endpoints are the same two
    // points either way.
    @location(3) @interpolate(flat) span: vec2<f32>,
    @location(4) @interpolate(flat) color: vec3<f32>,
    @location(5) @interpolate(flat) width: f32,
}

@vertex
fn vs_main(
    @builtin(vertex_index) vi: u32,
    @location(0) centre: vec2<f32>,
    @location(1) radius: f32,
    @location(2) angle_start: f32,
    @location(3) angle_sweep: f32,
    @location(4) color: vec3<f32>,
    @location(5) width: f32,
) -> VsOut {
    // The unit square, expanded to the arc's own bounding box below.
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(0.0, 1.0),
        vec2<f32>(0.0, 1.0), vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0),
    );
    let c = corners[vi];
    let aspect = max(u.v.x, 0.1);

    // Shared ViewTransform (ADR-0018), applied exactly as the segment shader
    // applies it: in world space, before the aspect divide. A uniform zoom
    // scales the radius with the centre; stroke width does not move.
    let centre_v = centre * u.view.x + u.view.yz;
    let radius_v = radius * u.view.x;

    let a0 = angle_start;
    let a1 = angle_start + angle_sweep;
    let lo = min(a0, a1);
    let hi = max(a0, a1);

    // The centreline's bounding box: the two ends always, plus each axis
    // extreme the span actually reaches. A full turn reaches all four and the
    // box is the whole circle; a short span gets a box barely bigger than its
    // own chord, which is what keeps the shaded area near the stroke's.
    let end0 = vec2<f32>(cos(a0), sin(a0));
    let end1 = vec2<f32>(cos(a1), sin(a1));
    var lo_dir = min(end0, end1);
    var hi_dir = max(end0, end1);
    for (var k = 0u; k < 4u; k = k + 1u) {
        let ang = f32(k) * QUARTER_TURN;
        // The smallest representative of `ang` modulo TAU at or above `lo`.
        let t = ang + TAU * ceil((lo - ang) / TAU);
        if (t <= hi) {
            let d = vec2<f32>(cos(ang), sin(ang));
            lo_dir = min(lo_dir, d);
            hi_dir = max(hi_dir, d);
        }
    }

    // The stroke reaches `width` in NDC on every side, so the world-space pad
    // is anisotropic even though the stroke is not: one NDC x unit is `aspect`
    // world units. The 2 % is slack for the distance being a first-order
    // expression of a curved level set - see the fragment.
    let pad = vec2<f32>(width * aspect, width) * 1.02;
    let lo_w = centre_v + radius_v * lo_dir - pad;
    let hi_w = centre_v + radius_v * hi_dir + pad;
    let p = mix(lo_w, hi_w, c);
    let ndc = vec2<f32>(p.x / aspect, p.y);

    var out: VsOut;
    out.pos = vec4<f32>(ndc, 0.0, 1.0);
    out.ndc = ndc;
    out.centre = centre_v;
    out.radius = radius_v;
    out.span = vec2<f32>(lo, hi);
    out.color = color;
    out.width = width;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let aspect = max(u.v.x, 0.1);
    // Back into the isotropic world space the arc is authored in.
    let p = vec2<f32>(in.ndc.x * aspect, in.ndc.y);
    let q = p - in.centre;
    let len = length(q);

    // Inside the span, or past one of its ends? The same modulo reduction the
    // vertex shader uses, so an arc that wraps the branch cut of atan2 is not
    // a special case.
    let theta = atan2(q.y, q.x);
    let t = theta + TAU * ceil((in.span.x - theta) / TAU);

    var d: f32;
    if (t <= in.span.y) {
        // The exact world radial distance, converted into the NDC metric by
        // the NDC length of its own gradient: moving one NDC unit along x
        // covers `aspect` world units, so a radial step is worth that much
        // less NDC where the arc runs vertically.
        let dir = select(vec2<f32>(1.0, 0.0), q / len, len > 1e-6);
        let grad = length(vec2<f32>(dir.x * aspect, dir.y));
        d = abs(len - in.radius) / max(grad, 1e-6);
    } else {
        // Past an end: the distance to the nearer endpoint. A point has an
        // exact NDC distance, so this arm approximates nothing.
        let e0 = in.centre + in.radius * vec2<f32>(cos(in.span.x), sin(in.span.x));
        let e1 = in.centre + in.radius * vec2<f32>(cos(in.span.y), sin(in.span.y));
        let d0 = length(vec2<f32>((p.x - e0.x) / aspect, p.y - e0.y));
        let d1 = length(vec2<f32>((p.x - e1.x) / aspect, p.y - e1.y));
        d = min(d0, d1);
    }

    // The segment path's profile on the arc's own distance: bright core,
    // quadratic falloff to the stroke edge. Premultiplied, so colour and alpha
    // carry the same coverage and the quad outside the stroke writes nothing
    // at all rather than opaque black (ADR-0056). The glow multiplier scales
    // the LIGHT, not the coverage, exactly as it does for a segment.
    let falloff = max(0.0, 1.0 - d / max(in.width, 1e-8));
    let g = falloff * falloff;
    return vec4<f32>(in.color * g * u.v.y, g);
}
"#;

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
/// **Arcs count too** (Plan 0087 Phase 2). An [`ArcInstance`] contributes its
/// own arc length, `|sweep| * radius`, to both sums. This is a correctness
/// obligation of the arc primitive rather than a feature: an arc contributing
/// nothing would shrink the denominator, and every arc-drawing preset would
/// read better-framed than it is — the more so as the primitive replaces whole
/// motifs, where the missing length is most of the figure.
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

/// Sub-arcs one [`ArcInstance`] is measured in — a **power of two**, which is
/// load-bearing.
///
/// Each sub-arc is clipped by its own chord and weighted `1 / ARC_STEPS`, so an
/// arc wholly inside the frame accumulates that weight exactly `ARC_STEPS`
/// times. At a power of two the weight is exact in binary and the sum is
/// **exactly 1.0**, which is what keeps an unclipped figure measuring exactly
/// its own length — the property `in_frame_fraction` is written to preserve for
/// segments.
///
/// Sixty-four puts 5.6 degrees in a sub-arc of a full circle, whose chord
/// departs from it by `r * (1 - cos(2.8 deg))`, about `0.0012 * r`. The error is
/// a function of the angle per step alone, so it does not grow with radius.
const ARC_STEPS: usize = 64;

/// The world-space length of `arc` and the share of it inside
/// `[-aspect, aspect] x [-1, 1]`, under the same view transform the vertex
/// shader applies.
///
/// Measured as [`ARC_STEPS`] sub-arcs clipped by their own chords rather than by
/// solving the circle against the four edges: the closed form needs the
/// intersection of four half-planes with a circle, which is up to four disjoint
/// angular components and considerably more code than the property is worth.
/// Both sums are taken from the arc's **own** length (`|sweep| * radius`), never
/// from the chords', so the sub-chord sampling changes only *where* the arc is
/// judged to be, never how long it is.
fn measure_arc(arc: &ArcInstance, aspect: f32, xform: super::ViewTransform) -> (f32, f32) {
    let [pan_x, pan_y] = xform.pan;
    let centre = [
        arc.centre[0] * xform.zoom + pan_x,
        arc.centre[1] * xform.zoom + pan_y,
    ];
    let radius = arc.radius * xform.zoom;
    let len = (radius * arc.angle_sweep).abs();
    if len <= 0.0 || !len.is_finite() {
        return (0.0, 0.0); // a degenerate (or non-finite) arc measures nothing
    }
    let step = 1.0 / ARC_STEPS as f32;
    let at = |k: usize| {
        let t = arc.angle_start + arc.angle_sweep * k as f32 * step;
        [centre[0] + radius * t.cos(), centre[1] + radius * t.sin()]
    };
    let mut inside = 0.0;
    for k in 0..ARC_STEPS {
        inside += in_frame_fraction(at(k), at(k + 1), aspect) * step;
    }
    (len, len * inside)
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
    arcs: &[ArcInstance],
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
    // Arcs, into the same two sums: the fraction is over everything drawn, and
    // a batch of both kinds has one denominator.
    for arc in arcs {
        let (len, in_frame) = measure_arc(arc, aspect, xform);
        extent.total_len += len;
        extent.in_frame_len += in_frame;
    }
    extent
}

/// Draws segment buffers as thick glowing quads. Owns its pipeline, a
/// fixed-capacity instance buffer, and the aspect/glow uniform.
pub struct LineRenderer {
    pipeline: wgpu::RenderPipeline,
    /// The same pipeline with the light composited **over** rather than added —
    /// see [`LineRenderer::draw_split`]. It shares the shader, the layout and the
    /// bind group with [`pipeline`](Self::pipeline), so ADR-0058 has nothing to
    /// separate: there is one layout, not two that happen to match.
    ///
    /// **`None` unless the scene asked for it** ([`LineRenderer::new_split`]),
    /// and that is not a micro-optimization. Building a pipeline the scene never
    /// binds still allocates on the device, and on the WARP software adapter a
    /// changed allocation order changes what a later pass resolves to — the
    /// hazard `core/tests/composite.rs`'s header records and the golden suite
    /// captures on. Building this for the nine line scenes that do not use it
    /// moved five composite baselines while changing nothing a driver would
    /// render differently.
    over_pipeline: Option<wgpu::RenderPipeline>,
    /// The [`ArcInstance`] pipeline, drawn in the same additive pass from its
    /// own buffer — [`ARC_SHADER`] and [ADR-0098].
    ///
    /// **`None` unless the scene asked for it**
    /// ([`LineRenderer::new_with_arcs`]), for the reason
    /// [`over_pipeline`](Self::over_pipeline) records and with the same
    /// evidence behind it: building a pipeline nobody binds still allocates on
    /// the device, and on WARP a changed allocation order changes what a later
    /// pass resolves to. It **shares the bind-group layout, the bind group and
    /// the pipeline layout** with the segment pipelines — one uniform, one
    /// layout, so ADR-0058 has nothing new to separate. Only the vertex layout
    /// and the shader module differ.
    ///
    /// [ADR-0098]: ../../../../../docs/adrs/0098-the-line-renderer-draws-arcs-as-per-pixel-distance-fields.md
    arc_pipeline: Option<wgpu::RenderPipeline>,
    instances: wgpu::Buffer,
    /// The arc instance buffer, `Some` exactly when
    /// [`arc_pipeline`](Self::arc_pipeline) is.
    arcs: Option<wgpu::Buffer>,
    uniforms: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    /// Maximum segments the instance buffer holds; extra are dropped by `draw`.
    capacity: usize,
    /// Maximum arcs the arc buffer holds; `0` when there is no arc pipeline.
    arc_capacity: usize,
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
        Self::build(device, surface_format, capacity, label, false, 0)
    }

    /// [`new`](Self::new), plus the second pipeline
    /// [`draw_split`](Self::draw_split) needs. Only a scene that actually splits
    /// its batch by blend mode should call this — see
    /// [`over_pipeline`](Self::over_pipeline) for why building it unconditionally
    /// is not free.
    pub fn new_split(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        capacity: usize,
        label: &str,
    ) -> Self {
        Self::build(device, surface_format, capacity, label, true, 0)
    }

    /// [`new`](Self::new), plus the arc pipeline and an `arc_capacity`-instance
    /// arc buffer ([`ArcInstance`], ADR-0098).
    ///
    /// Only a scene that actually draws arcs should call this — see
    /// [`arc_pipeline`](Self::arc_pipeline) for why building it unconditionally
    /// is not free. `arc_capacity` is its own budget rather than a share of
    /// `capacity`: an arc replaces many segments, so the two counts are not the
    /// same order and sizing one from the other would waste most of it.
    pub fn new_with_arcs(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        capacity: usize,
        arc_capacity: usize,
        label: &str,
    ) -> Self {
        Self::build(device, surface_format, capacity, label, false, arc_capacity)
    }

    fn build(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        capacity: usize,
        label: &str,
        split: bool,
        arc_capacity: usize,
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
        // The two pipelines differ in exactly one field — the blend state — so
        // they are built from one closure. Anything else that diverges between
        // them would be a bug that renders as a difference between two ranges of
        // the same batch, which is close to unreadable in a capture.
        let make = |blend: wgpu::BlendState, suffix: &str| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(&format!("{label}-pipeline{suffix}")),
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
                            5 => Float32,
                        ],
                    })],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: surface_format,
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
        // Additive light, saturating coverage (ADR-0056) — the same constant the
        // swarm's sprite pipeline takes, so the two draw seams cannot drift
        // apart. This is what every line scene draws through.
        let pipeline = make(crate::render::gpu::ADDITIVE_LIGHT_SATURATING_COVERAGE, "");
        // Premultiplied OVER, for a producer whose source blend *replaces* rather
        // than accumulates — see `draw_split`. The fragment is premultiplied
        // either way, which is why one shader serves both.
        let over_pipeline =
            split.then(|| make(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING, "-over"));

        // The arc pipeline (ADR-0098). Its own shader module and vertex layout,
        // the *same* bind layout, bind group and pipeline layout as above, and
        // the same additive blend — so an arc and a segment emit into one pass
        // through one uniform and cannot drift apart on aspect, glow or the
        // view transform.
        let arcs = (arc_capacity > 0).then(|| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(&format!("{label}-arc-instances")),
                size: (arc_capacity * std::mem::size_of::<ArcInstance>()) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        });
        let arc_pipeline = arcs.is_some().then(|| {
            let arc_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some(&format!("{label}-arc-shader")),
                source: wgpu::ShaderSource::Wgsl(ARC_SHADER.into()),
            });
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(&format!("{label}-arc-pipeline")),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &arc_shader,
                    entry_point: Some("vs_main"),
                    compilation_options: Default::default(),
                    buffers: &[Some(wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<ArcInstance>() as u64,
                        step_mode: wgpu::VertexStepMode::Instance,
                        attributes: &wgpu::vertex_attr_array![
                            0 => Float32x2,
                            1 => Float32,
                            2 => Float32,
                            3 => Float32,
                            4 => Float32x3,
                            5 => Float32,
                        ],
                    })],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &arc_shader,
                    entry_point: Some("fs_main"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: surface_format,
                        blend: Some(crate::render::gpu::ADDITIVE_LIGHT_SATURATING_COVERAGE),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview_mask: None,
                cache: None,
            })
        });

        // Zero unless the pipeline exists, so `draw_all` needs no second test:
        // a renderer without arcs clamps every arc batch to nothing.
        let arc_capacity = if arc_pipeline.is_some() {
            arc_capacity
        } else {
            0
        };

        Self {
            pipeline,
            over_pipeline,
            arc_pipeline,
            instances,
            arcs,
            uniforms,
            bind_group,
            capacity,
            arc_capacity,
        }
    }

    /// Segments the instance buffer can hold — the scene clamps its geometry to
    /// this and surfaces any drop at load (ADR-0007 cap must never be silent).
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Arcs the arc buffer can hold — `0` when this renderer was not built with
    /// [`new_with_arcs`](Self::new_with_arcs), in which case
    /// [`draw_arcs`](Self::draw_arcs) draws none.
    pub fn arc_capacity(&self) -> usize {
        self.arc_capacity
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
        self.draw_split(
            queue,
            encoder,
            view,
            aspect,
            glow,
            xform,
            segments,
            segments.len(),
        );
    }

    /// [`draw`](Self::draw), with the batch split by blend mode: the first
    /// `n_additive` segments are added (ADR-0056's seam, what every line scene
    /// uses), and the rest are composited **over** using each segment's own
    /// [`alpha`](SegmentInstance::alpha).
    ///
    /// # Why a split range rather than two calls
    ///
    /// One instance buffer, one upload, one render pass, two `draw` calls that
    /// differ only in the pipeline bound. Two calls would mean two passes over
    /// the same attachment and a second buffer, and the *order* would stop being
    /// expressible: an over-blended stroke has to land on top of the additive
    /// light it covers, which a single ordered batch gives for free.
    ///
    /// The caller partitions — it is the only thing that knows which producer
    /// each segment came from. Passing `n_additive >= segments.len()` is exactly
    /// [`draw`](Self::draw).
    #[allow(
        clippy::too_many_arguments,
        reason = "see `draw` — this is that signature plus the partition index"
    )]
    pub fn draw_split(
        &mut self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        aspect: f32,
        glow: f32,
        xform: super::ViewTransform,
        segments: &[SegmentInstance],
        n_additive: usize,
    ) {
        self.draw_all(
            queue,
            encoder,
            view,
            aspect,
            glow,
            xform,
            segments,
            n_additive,
            &[],
        );
    }

    /// [`draw`](Self::draw), plus `arcs` — [`ArcInstance`]s stroked by the
    /// per-pixel distance field (ADR-0098) **in the same additive pass**, from
    /// the same uniform, after the segments.
    ///
    /// One pass rather than two for the reason [`draw_split`](Self::draw_split)
    /// gives: a second pass would mean a second load of the attachment and a
    /// second set of uniforms to keep in step. Additive blending is
    /// order-independent, so "after the segments" is a statement about the
    /// command stream and not about the picture.
    ///
    /// Arcs beyond [`arc_capacity`](Self::arc_capacity) are dropped defensively,
    /// exactly as segments beyond `capacity` are; a renderer built without the
    /// arc pipeline has a capacity of zero and draws none.
    #[allow(
        clippy::too_many_arguments,
        reason = "see `draw` — this is that signature plus the arc batch"
    )]
    pub fn draw_arcs(
        &mut self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        aspect: f32,
        glow: f32,
        xform: super::ViewTransform,
        segments: &[SegmentInstance],
        arcs: &[ArcInstance],
    ) {
        self.draw_all(
            queue,
            encoder,
            view,
            aspect,
            glow,
            xform,
            segments,
            segments.len(),
            arcs,
        );
    }

    /// The one body behind [`draw`](Self::draw),
    /// [`draw_split`](Self::draw_split) and [`draw_arcs`](Self::draw_arcs):
    /// one buffer upload per kind, one uniform write, one render pass.
    #[allow(
        clippy::too_many_arguments,
        reason = "see `draw` — this is that signature plus the partition index \
                  and the arc batch"
    )]
    fn draw_all(
        &mut self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        aspect: f32,
        glow: f32,
        xform: super::ViewTransform,
        segments: &[SegmentInstance],
        n_additive: usize,
        arcs: &[ArcInstance],
    ) {
        let count = segments.len().min(self.capacity);
        let drawn = segments.get(..count).unwrap_or(&[]);
        let arc_count = arcs.len().min(self.arc_capacity);
        let arcs_drawn = arcs.get(..arc_count).unwrap_or(&[]);

        // The in-frame geometry diagnostic (Plan 0069, ADR-0083). Off in the
        // shipped path, and it reads `drawn` — the segments that actually reach
        // the instance buffer — without touching a GPU resource, so "off" is a
        // `Cell::get` and "on" changes nothing about the picture. The aspect it
        // measures against is the one this call was handed: the render target's
        // (ADR-0037), under the same `max(0.1)` clamp the uniform and the shader
        // apply, so the rectangle is the one the frame actually shows.
        if EXTENT_ON.with(std::cell::Cell::get) {
            let extent = measure_extent(drawn, arcs_drawn, aspect.max(0.1), xform);
            LAST_EXTENT.with(|slot| slot.set(Some(extent)));
        }

        if !drawn.is_empty() {
            queue.write_buffer(&self.instances, 0, bytemuck::cast_slice(drawn));
        }
        if let (Some(buffer), false) = (&self.arcs, arcs_drawn.is_empty()) {
            queue.write_buffer(buffer, 0, bytemuck::cast_slice(arcs_drawn));
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
        if drawn.is_empty() && arcs_drawn.is_empty() {
            return; // nothing to stroke; the backdrop shows through
        }
        // Clamped to what actually reached the buffer: `n_additive` counts into
        // `segments`, which may be longer than `drawn`.
        let split = n_additive.min(drawn.len()) as u32;
        pass.set_bind_group(0, &self.bind_group, &[]);
        if split > 0 {
            pass.set_pipeline(&self.pipeline);
            pass.set_vertex_buffer(0, self.instances.slice(..));
            pass.draw(0..6, 0..split);
        }
        // Between the two segment ranges rather than after both: an arc is
        // additive light, and the OVER range has to land on top of the light it
        // covers. No scene draws both today; the ordering is here so that the
        // day one does, it is already right.
        if let (Some(pipeline), Some(buffer), false) =
            (&self.arc_pipeline, &self.arcs, arcs_drawn.is_empty())
        {
            pass.set_pipeline(pipeline);
            pass.set_vertex_buffer(0, buffer.slice(..));
            pass.draw(0..6, 0..arcs_drawn.len() as u32);
        }
        if (drawn.len() as u32) > split {
            // Falls back to the additive pipeline when the scene did not ask for
            // the second one. That is the wrong blend rather than a panic, and it
            // is unreachable in practice: the only caller that passes a partition
            // is the one that built with `new_split`.
            pass.set_pipeline(self.over_pipeline.as_ref().unwrap_or(&self.pipeline));
            pass.set_vertex_buffer(0, self.instances.slice(..));
            pass.draw(0..6, split..drawn.len() as u32);
        }
    }
}

#[cfg(test)]
mod tests;
