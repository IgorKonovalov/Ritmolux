//! Warp mesh: a per-vertex UV grid that resamples the previous frame
//! ([ADR-0113](../../../../docs/adrs/0113-milkdrop-presets-are-translated-ahead-of-time-onto-a-warp-mesh-idiom.md)).
//!
//! # What it generalizes
//!
//! [ADR-0048](../../../../docs/adrs/0048-transformed-feedback.md) gave the engine
//! *one* affine transform through which an accumulation reads its own past: a
//! single zoom, rotation and translation applied identically to every texel. This
//! scene is that transform **per vertex**. The frame is covered by a grid of
//! cells; each of its vertices carries its own `zoom`/`rot`/`cx`/`cy`/`dx`/`dy`/
//! `sx`/`sy`/`warp`, and the rasterizer interpolates between them — so the past
//! can spiral in one corner and drift in another, which no single affine can
//! express.
//!
//! Those nine outputs come from a preset's `[per_vertex]` table, whose bindings
//! are evaluated once per vertex per frame with `x`, `y`, `rad` and `ang` bound
//! to that vertex's own position. A preset that declares no such table gets the
//! scalar params of the same names applied everywhere, which is exactly ADR-0048's
//! single shared transform — so the idiom degrades to the one it generalizes.
//!
//! # The grid is a resolution, not a shape
//!
//! [ADR-0037](../../../../docs/adrs/0037-internal-grid-is-a-resolution-not-a-shape.md),
//! and this is the most likely place in the engine to get it wrong, because here
//! the grid is *user-visible*: a preset names `[mesh] x` and `[mesh] y`, and they
//! are quantized and clamped to a tier capacity. **Every screen-destined
//! coordinate here takes its aspect from the render target** — the `rad`/`ang` the
//! per-vertex program reads (computed in `vertex_position`), and the isotropic
//! space the source-uv transform works in (computed in the vertex shader from a
//! uniform the CPU fills with the *target's* aspect). `meshx`/`meshy` appear in
//! neither. A `f32` aspect derived from the mesh size would be the bug.
//!
//! # Three passes
//!
//! 1. **warp** — the mesh is drawn into the write half of a ping-pong field,
//!    sampling the read half through each vertex's source uv and scaling it by
//!    `decay^dt`. This is the only pass that is not fullscreen.
//! 2. **deposit** — a fullscreen pass adding this frame's light onto the warped
//!    past: a palette-coloured gaussian ring with optional angular arms. It runs
//!    *after* the warp, so the light it lays down is "now" and is warped from the
//!    next frame onward.
//! 3. **present** — a fullscreen pass compositing the field over the backdrop,
//!    premultiplied (ADR-0026), scaled by `brightness` and `occlude`.
//!
//! All three rates are **per second** (ADR-0019): `decay`, `zoom`, `sx`, `sy` are
//! factors per second and `rot`/`dx`/`dy`/`warp`/`deposit` are amounts per second,
//! so the look is identical at 60 Hz and 144 Hz.
//!
//! GPU resources are built lazily on first render, for the reason
//! `reaction_diffusion.rs` documents: a capture that never activates this scene
//! never builds this scene's pipelines, so it cannot perturb another scene's
//! render on the DX12 WARP software adapter.

// Hot-path panic-denial pragma (Plan 0002 Phase 2, extended to scenes by Plan
// 0003 Phase 0). Encodes its passes every displayed frame.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

use crate::dsp::AnalysisFrame;
use crate::render::feedback::PingPongField;
use crate::render::gpu;
use crate::render::palette::{self, Palette};

use super::Scene;

/// The smallest grid a `[mesh]` table may name, in cells. Below two the mesh is
/// a single quad and the per-vertex program has no interior to interpolate.
pub const MIN_MESH: u32 = 2;

/// The largest grid **any** tier may name, in cells — the `.milk` format's own
/// ceiling (`meshx <= 128`, `meshy <= 96`), so a converted preset's requested
/// grid is always representable.
pub const MAX_MESH: (u32, u32) = (128, 96);

/// The grid a `[mesh]` table's absent keys mean. Coarse enough to be free on any
/// machine and fine enough that a `rad`-driven program reads as a curve rather
/// than as facets.
pub const DEFAULT_MESH: (u32, u32) = (32, 24);

/// Clamp a preset's requested grid into what `tier` will carry.
///
/// **The one place the tier clamp happens.** Two consumers need the same answer
/// — the scene, which builds the vertex and index buffers, and the renderer,
/// which sizes the per-vertex evaluation scratch — and if they disagreed the
/// renderer would hand the scene a series of the wrong length every frame. Pure,
/// so both can call it and a test can hold them to the same value.
pub fn clamp_grid(requested: (u32, u32), tier: &crate::render::TierConfig) -> (u32, u32) {
    (
        requested
            .0
            .clamp(MIN_MESH, tier.mesh_grid.0.clamp(MIN_MESH, MAX_MESH.0)),
        requested
            .1
            .clamp(MIN_MESH, tier.mesh_grid.1.clamp(MIN_MESH, MAX_MESH.1)),
    )
}

/// How many vertices a grid of `mesh` cells has. One more than the cell count on
/// each axis — the fencepost the whole per-vertex path is sized by.
pub fn vertex_count(mesh: (u32, u32)) -> usize {
    (mesh.0 as usize + 1) * (mesh.1 as usize + 1)
}

/// The `(x, y, rad, ang)` a vertex's `[per_vertex]` bindings are evaluated
/// against, for the vertex at column `col`, row `row` of a `mesh` grid, on a
/// render target of aspect `aspect`.
///
/// `x` and `y` are the vertex's uv in `0..=1`, with `y = 0` at the **top** —
/// texture space, the space every sampler in this file addresses.
///
/// `rad` is the distance from the mesh centre and `ang` the angle there, both in
/// the **aspect-corrected** space of the render target (ADR-0037): `rad` reaches
/// `1.0` at the middle of the top and bottom edges on any display, and further
/// than that at the sides of a wide one. So a program written as
/// `zoom = 1 + rad * 0.2` makes a circular figure on a 16:9 monitor and the same
/// circular figure on a 5:4 one, which is the property the ADR exists for and the
/// reason this takes `aspect` rather than deriving one from `mesh`.
///
/// `ang` is in `0..tau`, measured counter-clockwise from the +x axis in *screen*
/// terms (y is flipped on the way in, so a positive angle turns the way an author
/// looking at the screen expects).
pub fn vertex_position(col: u32, row: u32, mesh: (u32, u32), aspect: f32) -> (f32, f32, f32, f32) {
    let x = col as f32 / mesh.0.max(1) as f32;
    let y = row as f32 / mesh.1.max(1) as f32;
    // Aspect correction on the *x* axis, so one unit of `rad` is one half-height
    // whatever the target's shape. A non-finite or non-positive aspect degrades
    // to square rather than poisoning every vertex with a NaN.
    let aspect = if aspect.is_finite() && aspect > 0.0 {
        aspect
    } else {
        1.0
    };
    let px = (x - 0.5) * 2.0 * aspect;
    // Flip y so +y is up, which is what makes `ang` read the way it looks.
    let py = (0.5 - y) * 2.0;
    let rad = (px * px + py * py).sqrt();
    let mut ang = py.atan2(px);
    if ang < 0.0 {
        ang += std::f32::consts::TAU;
    }
    (x, y, rad, ang)
}

/// The nine outputs a `[per_vertex]` table may bind, in the order this scene
/// stores them. **Keep in step with [`PER_VERTEX_DEFAULTS`] and
/// [`WarpMeshScene::set_per_vertex`].**
///
/// The same nine names are ordinary scalar [`PARAMS`] as well, and that is the
/// design: a scalar sets the output for the whole mesh, and a `[per_vertex]`
/// binding of the same name **replaces** it vertex by vertex. A preset therefore
/// starts from one shared transform and opts into a spatially-varying one output
/// at a time.
pub const PER_VERTEX_PARAMS: &[&str] = &["zoom", "rot", "cx", "cy", "dx", "dy", "sx", "sy", "warp"];

/// Each [`PER_VERTEX_PARAMS`] entry's identity value, positionally.
///
/// The identity of the whole roster is "the past sits still": unit scale, no
/// rotation, no drift, centred, no procedural warp — the same identity
/// [`Transform::IDENTITY`](crate::render::feedback::Transform::IDENTITY) names for
/// the affine this generalizes.
const PER_VERTEX_DEFAULTS: [f32; 9] = [1.0, 0.0, 0.5, 0.5, 0.0, 0.0, 1.0, 1.0, 0.0];

/// How many per-vertex outputs there are — typed off the roster so the arrays
/// below cannot drift from it.
const OUTPUTS: usize = PER_VERTEX_DEFAULTS.len();
const _: () = assert!(
    OUTPUTS == PER_VERTEX_PARAMS.len(),
    "the per-vertex roster and its defaults must be the same length"
);

/// `decay` default: 0.72 of the past survives each second, so a deposited streak
/// fades over roughly a second and a half.
const DEFAULT_DECAY: f32 = 0.72;
/// The most of the past a preset may keep per second. Not 1.0: at exactly 1 the
/// field is a perfect integrator and any deposit accumulates without bound, which
/// in linear light is a slowly whitening frame rather than a clip. Mirrors the
/// `MAX_FADE` ceiling the trails accumulation takes for the same reason.
const MAX_DECAY: f32 = 0.995;

/// Procedural-warp defaults — the spatial scale of the four sinusoids and how
/// fast they animate. `1.0` is MilkDrop's own unit scale.
const DEFAULT_WARP_SCALE: f32 = 1.0;
const DEFAULT_WARP_SPEED: f32 = 1.0;

/// Deposit defaults: a soft blob at the centre, bright enough to see and small
/// enough to be dragged into structure rather than filling the frame.
const DEFAULT_DEPOSIT: f32 = 1.6;
const DEFAULT_DEPOSIT_CENTRE: f32 = 0.5;
const DEFAULT_DEPOSIT_RADIUS: f32 = 0.45;
const DEFAULT_DEPOSIT_WIDTH: f32 = 0.11;
const DEFAULT_DEPOSIT_ARMS: f32 = 0.0;
const DEFAULT_DEPOSIT_TWIST: f32 = 0.0;
const DEFAULT_DEPOSIT_SPIN: f32 = 0.0;

/// Colour defaults (ADR-0021), matching the shared vocabulary every other
/// scene uses.
const DEFAULT_HUE: f32 = 0.0;
const DEFAULT_COLOR_SPAN: f32 = 1.0;
const DEFAULT_COLOR_CENTER: f32 = 0.0;
const DEFAULT_SATURATION: f32 = 1.0;
const DEFAULT_PALETTE_MIX: f32 = 0.0;
const DEFAULT_BRIGHTNESS: f32 = 1.0;

/// Parameter vocabulary — see [`fragment_field::PARAMS`](super::fragment_field::PARAMS).
/// **Keep in sync with `set_param` below.**
pub const PARAMS: &[&str] = &[
    // The nine per-vertex outputs, as whole-mesh scalars.
    "zoom",
    "rot",
    "cx",
    "cy",
    "dx",
    "dy",
    "sx",
    "sy",
    "warp",
    // The procedural warp's own shape.
    "warp_scale",
    "warp_speed",
    // The feedback field.
    "decay",
    // This frame's light.
    "deposit",
    "deposit_x",
    "deposit_y",
    "deposit_radius",
    "deposit_width",
    "deposit_arms",
    "deposit_twist",
    "deposit_spin",
    // Colour.
    "hue",
    "color_span",
    "color_center",
    "saturation",
    "palette_mix",
    "palette_steps",
    "palette_contour",
    "brightness",
];

// ---------------------------------------------------------------------------
// Shaders
// ---------------------------------------------------------------------------

/// The warp pass: one draw over the mesh, resampling the past.
///
/// The vertex stage does the transform rather than the CPU, deliberately. The CPU
/// hands over the nine *outputs* — which is what the per-vertex program produced —
/// and the shader turns them into a source uv. That keeps the per-vertex
/// evaluation on the render thread down to the expressions themselves (which is
/// what the tier capacity is measured against), and it puts the four warp
/// sinusoids on the GPU where they cost nothing.
const WARP_SHADER: &str = r#"
struct Warp {
    // x: render-target aspect, y: dt (s), z: scene time (s), w: warp_scale
    misc: vec4<f32>,
    // x: decay^dt, y: warp_speed, zw: unused
    misc2: vec4<f32>,
}
@group(0) @binding(0) var<uniform> wu: Warp;
@group(0) @binding(1) var past: texture_2d<f32>;
@group(0) @binding(2) var past_samp: sampler;

struct VsIn {
    @location(0) clip: vec2<f32>,
    // zoom, rot, cx, cy
    @location(1) t0: vec4<f32>,
    // dx, dy, sx, sy
    @location(2) t1: vec4<f32>,
    // warp, unused...
    @location(3) t2: vec4<f32>,
}

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) src: vec2<f32>,
}

@vertex
fn vs_main(in: VsIn) -> VsOut {
    let aspect = wu.misc.x;
    let dt     = wu.misc.y;
    let time   = wu.misc.z;
    let wscale = max(wu.misc.w, 1e-3);
    let wspeed = wu.misc2.y;

    // This vertex's own destination uv (texture space, y down).
    let uv = vec2<f32>(in.clip.x * 0.5 + 0.5, 0.5 - in.clip.y * 0.5);

    // Rates are per second (ADR-0019): a factor is raised to dt, an amount is
    // multiplied by it, so two half-length frames compose to one full-length one.
    let zoom = pow(max(in.t0.x, 1e-4), dt);
    let rot  = in.t0.y * dt;
    let ctr  = in.t0.zw;
    let d    = in.t1.xy * dt;
    let sx   = pow(max(in.t1.z, 1e-4), dt);
    let sy   = pow(max(in.t1.w, 1e-4), dt);
    let warp = in.t2.x * dt;

    // Isotropic, about the per-vertex centre. `aspect` is the RENDER TARGET's
    // (ADR-0037) — the mesh's own proportions never enter this.
    var p = uv - ctr;
    p.x = p.x * aspect;

    // The INVERSE of the motion the outputs name: a destination vertex asks where
    // its content came from, so a `zoom` above 1 shrinks the source window and the
    // past appears to grow.
    p = p / zoom;
    p = vec2<f32>(p.x / sx, p.y / sy);
    let c = cos(rot);
    let s = sin(rot);
    p = vec2<f32>(p.x * c - p.y * s, p.x * s + p.y * c);
    p = p - d;

    // The procedural warp, MilkDrop's four sinusoids: a wobble whose own
    // frequencies drift, so it never settles into a visible standing pattern.
    let wt = time * wspeed;
    let f0 = 11.68 + 4.0 * cos(wt * 1.413 + 10.0);
    let f1 =  8.77 + 3.0 * cos(wt * 1.113 +  7.0);
    let f2 = 10.54 + 3.0 * cos(wt * 1.233 +  3.0);
    let f3 = 11.49 + 4.0 * cos(wt * 0.933 +  5.0);
    let inv = 1.0 / wscale;
    let wx = uv.x * 2.0 - 1.0;
    let wy = 1.0 - uv.y * 2.0;
    p.x = p.x + warp * 0.0035 * sin(wt * 0.333 + inv * (wx * f0 - wy * f3));
    p.y = p.y + warp * 0.0035 * cos(wt * 0.375 - inv * (wx * f2 + wy * f1));
    p.x = p.x + warp * 0.0035 * cos(wt * 0.753 - inv * (wx * f1 - wy * f2));
    p.y = p.y + warp * 0.0035 * sin(wt * 0.825 + inv * (wx * f0 + wy * f3));

    p.x = p.x / aspect;

    var out: VsOut;
    out.pos = vec4<f32>(in.clip, 0.0, 1.0);
    out.src = p + ctr;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    // The transparent-border edge policy (ADR-0048): off-field reads contribute
    // nothing. Clamping would re-deposit the border texel every frame until the
    // edge became a permanent bar of colour.
    let inside = f32(all(in.src >= vec2<f32>(0.0)) && all(in.src <= vec2<f32>(1.0)));
    let past_c = textureSampleLevel(past, past_samp, in.src, 0.0);
    return past_c * (wu.misc2.x * inside);
}
"#;

/// The deposit pass: this frame's light, laid onto the warped past.
const DEPOSIT_SHADER: &str = r#"
struct Deposit {
    // x: aspect, y: amount * dt, z: ring radius, w: ring width
    a: vec4<f32>,
    // xy: centre (uv), z: arms, w: twist
    b: vec4<f32>,
    // x: spin phase (rad), y: hue + color_center, z: color_span, w: saturation
    c: vec4<f32>,
    // x: palette_mix, y: palette_steps, z: palette_contour, w: unused
    d: vec4<f32>,
}
@group(0) @binding(0) var lut_a: texture_2d<f32>;
@group(0) @binding(1) var lut_b: texture_2d<f32>;
@group(0) @binding(2) var lut_samp: sampler;
@group(0) @binding(3) var<uniform> dp: Deposit;

// Shared `saturation` (mirrors core/src/render/palette.rs::desaturate verbatim).
fn apply_saturation(col: vec3<f32>, s: f32) -> vec3<f32> {
    let luma = dot(col, vec3<f32>(0.299, 0.587, 0.114));
    return vec3<f32>(luma) + (col - vec3<f32>(luma)) * s;
}

// Shared `palette_steps` (mirrors core/src/render/palette.rs::band_coord).
fn band_coord(t: f32, steps: f32) -> f32 {
    if (steps < 1.5) {
        return t;
    }
    return (floor(t * steps) + 0.5) / steps;
}

// Shared `palette_contour` (ADR-0078), copied verbatim at each fragment site.
fn band_contour(t: f32, steps: f32, amount: f32) -> f32 {
    let f = t * steps;
    let w = max(fwidth(f), 1e-5);
    if (steps < 1.5 || amount <= 0.0) {
        return 1.0;
    }
    let dd = min(fract(f), 1.0 - fract(f));
    return 1.0 - clamp(amount, 0.0, 1.0) * (1.0 - smoothstep(0.0, w, dd));
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let aspect = dp.a.x;
    // Frame-height units about the deposit centre — the target's aspect, never
    // the mesh grid's (ADR-0037), so the ring is round on any display.
    var p = in.uv - dp.b.xy;
    p.x = p.x * aspect;
    let r = length(p);
    // +y up, so a positive `deposit_spin` turns the arms the way it looks.
    let ang = atan2(-p.y, p.x);

    // A gaussian ring: `deposit_radius = 0` degenerates to a blob at the centre.
    let width = max(dp.a.w, 1e-3);
    let dr = (r - dp.a.z) / width;
    var g = exp(-0.5 * dr * dr);

    // Angular arms. Below half an arm the modulation is off entirely rather than
    // a degenerate single lobe.
    let arms = dp.b.z;
    if (arms >= 0.5) {
        let phase = arms * (ang + dp.b.w * r) + dp.c.x;
        g = g * (0.5 + 0.5 * cos(phase));
    }

    let amount = max(dp.a.y, 0.0) * g;

    // Palette by angle, so the deposit lays down colour the warp can drag into
    // structure rather than one flat tone.
    let coord = dp.c.y + dp.c.z * (ang / 6.2831853);
    let banded = band_coord(coord, dp.d.y);
    let ca = textureSample(lut_a, lut_samp, vec2<f32>(banded, 0.5)).rgb;
    let cb = textureSample(lut_b, lut_samp, vec2<f32>(banded, 0.5)).rgb;
    let mixed = mix(ca, cb, clamp(dp.d.x, 0.0, 1.0)) * band_contour(coord, dp.d.y, dp.d.z);
    let col = apply_saturation(mixed, dp.c.w);

    // Additive light with saturating coverage (ADR-0056): premultiplied colour,
    // and an alpha equal to the coverage this fragment actually has.
    return vec4<f32>(col * amount, clamp(amount, 0.0, 1.0));
}
"#;

/// The present pass: the field, over the backdrop.
const PRESENT_SHADER: &str = r#"
struct Present {
    // x: brightness, y: occlude, zw: unused
    a: vec4<f32>,
}
@group(0) @binding(0) var<uniform> pp: Present;
@group(0) @binding(1) var field: texture_2d<f32>;
@group(0) @binding(2) var field_samp: sampler;

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let c = textureSampleLevel(field, field_samp, in.uv, 0.0);
    // The field already holds premultiplied colour and coverage (the deposit
    // writes them that way and the warp scales both together), so `brightness`
    // scales the light and `occlude` scales only how much backdrop is held out
    // (ADR-0085).
    return vec4<f32>(c.rgb * max(pp.a.x, 0.0), clamp(c.a, 0.0, 1.0) * pp.a.y);
}
"#;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct WarpUniform {
    misc: [f32; 4],
    misc2: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct DepositUniform {
    a: [f32; 4],
    b: [f32; 4],
    c: [f32; 4],
    d: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct PresentUniform {
    a: [f32; 4],
}

/// One mesh vertex, as the warp pipeline reads it.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    clip: [f32; 2],
    t0: [f32; 4],
    t1: [f32; 4],
    t2: [f32; 4],
}

/// The GPU-side state, built lazily on first render (module docs).
struct Resources {
    field: PingPongField,
    /// The field's pixel size, so a target resize can be noticed and the pair
    /// rebuilt (ADR-0030: compare against what you already built).
    size: (u32, u32),
    warp_pipeline: wgpu::RenderPipeline,
    deposit_pipeline: wgpu::RenderPipeline,
    present_pipeline: wgpu::RenderPipeline,
    warp_uniform: wgpu::Buffer,
    deposit_uniform: wgpu::Buffer,
    present_uniform: wgpu::Buffer,
    /// Warp/present bind groups reading texture A / texture B — selected by the
    /// field's read side so nothing is rebuilt on the hot path.
    warp_bg_a: wgpu::BindGroup,
    warp_bg_b: wgpu::BindGroup,
    present_bg_a: wgpu::BindGroup,
    present_bg_b: wgpu::BindGroup,
    deposit_bg: wgpu::BindGroup,
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    /// How many indices the current mesh draws, and the grid they were built for.
    index_count: u32,
    mesh: (u32, u32),
    lut_texture_a: wgpu::Texture,
    lut_texture_b: wgpu::Texture,
    /// Whether the field still holds the undefined contents of a fresh
    /// allocation. Cleared by one pass before anything samples it.
    needs_clear: bool,
}

/// The per-frame values the CPU assembles a vertex buffer from.
struct MeshState {
    /// The clamped grid this state is sized for.
    mesh: (u32, u32),
    /// One value per vertex for each of the nine outputs. Only the entries whose
    /// `bound` flag is set this frame are read; the rest fall back to the scalar
    /// param, which is what makes a `[per_vertex]` binding an override.
    values: [Vec<f32>; OUTPUTS],
    bound: [bool; OUTPUTS],
    /// The assembled vertex buffer, resized only when the grid changes.
    vertices: Vec<Vertex>,
}

impl MeshState {
    fn new(mesh: (u32, u32)) -> Self {
        let n = vertex_count(mesh);
        Self {
            mesh,
            values: std::array::from_fn(|_| vec![0.0; n]),
            bound: [false; OUTPUTS],
            vertices: vec![
                Vertex {
                    clip: [0.0; 2],
                    t0: [0.0; 4],
                    t1: [0.0; 4],
                    t2: [0.0; 4],
                };
                n
            ],
        }
    }

    /// Resize to `mesh` if it differs — off the hot path (a preset switch), so
    /// the allocation here is not a per-frame one.
    fn resize(&mut self, mesh: (u32, u32)) {
        if self.mesh == mesh {
            return;
        }
        *self = Self::new(mesh);
    }

    /// Fill `out` with this frame's vertices. `scalars` supplies the fallback for
    /// every output with no `[per_vertex]` binding this frame.
    fn assemble(&mut self, scalars: &[f32; OUTPUTS]) {
        let (mx, my) = self.mesh;
        let mut v = 0usize;
        for row in 0..=my {
            for col in 0..=mx {
                let clip = [
                    (col as f32 / mx.max(1) as f32) * 2.0 - 1.0,
                    1.0 - (row as f32 / my.max(1) as f32) * 2.0,
                ];
                let mut out = [0.0f32; OUTPUTS];
                for (i, slot) in out.iter_mut().enumerate() {
                    *slot = match (self.bound.get(i), self.values.get(i)) {
                        (Some(true), Some(series)) => series.get(v).copied().unwrap_or(0.0),
                        _ => scalars.get(i).copied().unwrap_or(0.0),
                    };
                }
                if let Some(slot) = self.vertices.get_mut(v) {
                    *slot = Vertex {
                        clip,
                        t0: [out[0], out[1], out[2], out[3]],
                        t1: [out[4], out[5], out[6], out[7]],
                        t2: [out[8], 0.0, 0.0, 0.0],
                    };
                }
                v += 1;
            }
        }
    }
}

/// The triangle indices for a `mesh` grid, two triangles per cell.
fn build_indices(mesh: (u32, u32)) -> Vec<u32> {
    let (mx, my) = mesh;
    let stride = mx + 1;
    let mut out = Vec::with_capacity((mx as usize) * (my as usize) * 6);
    for row in 0..my {
        for col in 0..mx {
            let a = row * stride + col;
            let b = a + 1;
            let c = a + stride;
            let d = c + 1;
            out.extend_from_slice(&[a, c, b, b, c, d]);
        }
    }
    out
}

/// The warp mesh scene (ADR-0113).
pub struct WarpMeshScene {
    device: wgpu::Device,
    surface_format: wgpu::TextureFormat,
    res: Option<Resources>,
    /// The tier's mesh ceiling, fixed for the life of the scene like every other
    /// tier capacity — a tier change rebuilds the scene.
    tier_mesh: (u32, u32),
    /// The grid the active preset asked for, before the tier clamp.
    requested_mesh: (u32, u32),
    state: MeshState,
    /// This frame's target size, recorded by `set_target_size` and acted on in
    /// `render` (ADR-0030: never allocate in the setter).
    target: (u32, u32),
    time: f32,
    dt: f32,
    /// The nine per-vertex outputs as whole-mesh scalars, in
    /// [`PER_VERTEX_PARAMS`] order.
    scalars: [f32; OUTPUTS],
    warp_scale: f32,
    warp_speed: f32,
    decay: f32,
    deposit: f32,
    deposit_x: f32,
    deposit_y: f32,
    deposit_radius: f32,
    deposit_width: f32,
    deposit_arms: f32,
    deposit_twist: f32,
    deposit_spin: f32,
    hue: f32,
    color_span: f32,
    color_center: f32,
    saturation: f32,
    palette_mix: f32,
    palette_steps: f32,
    palette_contour: f32,
    brightness: f32,
    occlude: f32,
    palette: Palette,
    palette_dirty: bool,
}

impl WarpMeshScene {
    /// Build the CPU-side state. GPU resources are deferred to the first render
    /// (module docs). `tier_mesh` is the active tier's
    /// [`mesh_grid`](crate::render::TierConfig::mesh_grid).
    pub fn new(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        tier_mesh: (u32, u32),
    ) -> Self {
        let tier = crate::render::TierConfig {
            mesh_grid: tier_mesh,
            ..crate::render::TierConfig::FLOOR
        };
        let mesh = clamp_grid(DEFAULT_MESH, &tier);
        Self {
            device: device.clone(),
            surface_format,
            res: None,
            tier_mesh,
            requested_mesh: DEFAULT_MESH,
            state: MeshState::new(mesh),
            target: (0, 0),
            time: 0.0,
            dt: super::FALLBACK_DT,
            scalars: PER_VERTEX_DEFAULTS,
            warp_scale: DEFAULT_WARP_SCALE,
            warp_speed: DEFAULT_WARP_SPEED,
            decay: DEFAULT_DECAY,
            deposit: DEFAULT_DEPOSIT,
            deposit_x: DEFAULT_DEPOSIT_CENTRE,
            deposit_y: DEFAULT_DEPOSIT_CENTRE,
            deposit_radius: DEFAULT_DEPOSIT_RADIUS,
            deposit_width: DEFAULT_DEPOSIT_WIDTH,
            deposit_arms: DEFAULT_DEPOSIT_ARMS,
            deposit_twist: DEFAULT_DEPOSIT_TWIST,
            deposit_spin: DEFAULT_DEPOSIT_SPIN,
            hue: DEFAULT_HUE,
            color_span: DEFAULT_COLOR_SPAN,
            color_center: DEFAULT_COLOR_CENTER,
            saturation: DEFAULT_SATURATION,
            palette_mix: DEFAULT_PALETTE_MIX,
            palette_steps: palette::DEFAULT_PALETTE_STEPS,
            palette_contour: palette::DEFAULT_PALETTE_CONTOUR,
            brightness: DEFAULT_BRIGHTNESS,
            occlude: crate::render::post::DEFAULT_OCCLUDE,
            palette: Palette::default_spectrum(),
            palette_dirty: true,
        }
    }

    /// The grid this scene actually draws, after the tier clamp. The renderer
    /// calls the same [`clamp_grid`] on the same request, so the per-vertex
    /// series it sends is exactly this long.
    pub fn mesh(&self) -> (u32, u32) {
        self.state.mesh
    }
}

/// The field's format. `Rgba16Float` for the reason
/// [`PingPongField::FORMAT`] gives, and because this field holds **linear-light**
/// premultiplied colour above 1.0 (ADR-0046) — an 8-bit accumulation would clip
/// every deposit the moment two of them overlapped.
const FIELD_FORMAT: wgpu::TextureFormat = PingPongField::FORMAT;

impl Resources {
    fn build(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        size: (u32, u32),
        mesh: (u32, u32),
    ) -> Self {
        let warp_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("warp-mesh-warp-shader"),
            source: wgpu::ShaderSource::Wgsl(WARP_SHADER.into()),
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

        let warp_uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("warp-mesh-warp-uniform"),
            size: std::mem::size_of::<WarpUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let deposit_uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("warp-mesh-deposit-uniform"),
            size: std::mem::size_of::<DepositUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let present_uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("warp-mesh-present-uniform"),
            size: std::mem::size_of::<PresentUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

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
                            std::mem::size_of::<WarpUniform>() as u64,
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
                        resource: wgpu::BindingResource::Sampler(&sampler),
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
                module: &warp_shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![
                        0 => Float32x2,
                        1 => Float32x4,
                        2 => Float32x4,
                        3 => Float32x4,
                    ],
                })],
            },
            fragment: Some(wgpu::FragmentState {
                module: &warp_shader,
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

        // --- deposit pass ---
        let lut_texture_a = palette::lut_texture(device, "warp-mesh-lut-a");
        let lut_texture_b = palette::lut_texture(device, "warp-mesh-lut-b");
        let lut_view_a = lut_texture_a.create_view(&wgpu::TextureViewDescriptor::default());
        let lut_view_b = lut_texture_b.create_view(&wgpu::TextureViewDescriptor::default());
        let lut_sampler = palette::lut_sampler(device);
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
                        min_binding_size: std::num::NonZeroU64::new(std::mem::size_of::<
                            DepositUniform,
                        >()
                            as u64),
                    },
                    count: None,
                },
            ],
        });
        let deposit_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("warp-mesh-deposit-bg"),
            layout: &deposit_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&lut_view_a),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&lut_view_b),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&lut_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: deposit_uniform.as_entire_binding(),
                },
            ],
        });
        let deposit_pipeline = gpu::fullscreen_pipeline(
            device,
            &deposit_shader,
            &[&deposit_layout],
            FIELD_FORMAT,
            gpu::ADDITIVE_LIGHT_SATURATING_COVERAGE,
            "warp-mesh-deposit",
        );

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
                        min_binding_size: std::num::NonZeroU64::new(std::mem::size_of::<
                            PresentUniform,
                        >()
                            as u64),
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
                        resource: wgpu::BindingResource::Sampler(&sampler),
                    },
                ],
            })
        };
        let present_bg_a = present_bind_group(field.view_a());
        let present_bg_b = present_bind_group(field.view_b());
        let present_pipeline = gpu::fullscreen_pipeline(
            device,
            &present_shader,
            &[&present_layout],
            surface_format,
            // Premultiplied-alpha OVER the backdrop (ADR-0026): the field is
            // emissive, and its alpha reveals `bg_*` where nothing was deposited.
            wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING,
            "warp-mesh-present",
        );

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
            lut_texture_a,
            lut_texture_b,
            needs_clear: true,
        }
    }

    /// Clear both halves of a freshly-allocated field. A texture's contents are
    /// undefined until written, and the warp pass samples the read half on its
    /// very first frame.
    fn encode_clear(&self, encoder: &mut wgpu::CommandEncoder) {
        for view in [self.field.view_a(), self.field.view_b()] {
            encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("warp-mesh-field-clear"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
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

impl Scene for WarpMeshScene {
    fn name(&self) -> &'static str {
        "warp mesh"
    }

    fn set_time(&mut self, time: f32) {
        self.time = time;
    }

    fn advance(&mut self, dt: f32) {
        // Every rate in this scene is per second, so the frame's own elapsed time
        // is the whole of what `advance` carries. A non-finite or negative `dt`
        // degrades to the capture step rather than poisoning `pow`.
        self.dt = if dt.is_finite() && dt > 0.0 {
            dt
        } else {
            super::FALLBACK_DT
        };
    }

    fn set_occlude(&mut self, occlude: f32) {
        self.occlude = occlude;
    }

    fn set_target_size(&mut self, width: u32, height: u32) {
        // Record only — ADR-0030 condition 2. `render` notices the difference.
        self.target = (width.max(1), height.max(1));
    }

    fn set_palette(&mut self, palette: &Palette) {
        self.palette = palette.clone();
        self.palette_dirty = true;
    }

    fn configure(&mut self, cfg: &super::GeneratorConfig) -> Option<super::CapOverflow> {
        if let super::GeneratorConfig::WarpMesh { mesh } = cfg {
            self.requested_mesh = *mesh;
            let tier = crate::render::TierConfig {
                mesh_grid: self.tier_mesh,
                ..crate::render::TierConfig::FLOOR
            };
            self.state.resize(clamp_grid(*mesh, &tier));
        }
        None
    }

    fn reset_params(&mut self) {
        self.scalars = PER_VERTEX_DEFAULTS;
        // A `[per_vertex]` binding is re-applied every frame, so clearing the
        // flags here is what makes an unbound output fall back to its scalar.
        self.state.bound = [false; OUTPUTS];
        self.warp_scale = DEFAULT_WARP_SCALE;
        self.warp_speed = DEFAULT_WARP_SPEED;
        self.decay = DEFAULT_DECAY;
        self.deposit = DEFAULT_DEPOSIT;
        self.deposit_x = DEFAULT_DEPOSIT_CENTRE;
        self.deposit_y = DEFAULT_DEPOSIT_CENTRE;
        self.deposit_radius = DEFAULT_DEPOSIT_RADIUS;
        self.deposit_width = DEFAULT_DEPOSIT_WIDTH;
        self.deposit_arms = DEFAULT_DEPOSIT_ARMS;
        self.deposit_twist = DEFAULT_DEPOSIT_TWIST;
        self.deposit_spin = DEFAULT_DEPOSIT_SPIN;
        self.hue = DEFAULT_HUE;
        self.color_span = DEFAULT_COLOR_SPAN;
        self.color_center = DEFAULT_COLOR_CENTER;
        self.saturation = DEFAULT_SATURATION;
        self.palette_mix = DEFAULT_PALETTE_MIX;
        self.palette_steps = palette::DEFAULT_PALETTE_STEPS;
        self.palette_contour = palette::DEFAULT_PALETTE_CONTOUR;
        self.brightness = DEFAULT_BRIGHTNESS;
    }

    fn set_param(&mut self, name: &str, value: f32) {
        // The nine per-vertex outputs, as whole-mesh scalars — the fallback a
        // `[per_vertex]` binding of the same name overrides.
        if let Some(index) = PER_VERTEX_PARAMS.iter().position(|n| *n == name) {
            if let Some(slot) = self.scalars.get_mut(index) {
                *slot = value;
            }
            return;
        }
        match name {
            "warp_scale" => self.warp_scale = value,
            "warp_speed" => self.warp_speed = value,
            "decay" => self.decay = value,
            "deposit" => self.deposit = value,
            "deposit_x" => self.deposit_x = value,
            "deposit_y" => self.deposit_y = value,
            "deposit_radius" => self.deposit_radius = value,
            "deposit_width" => self.deposit_width = value,
            "deposit_arms" => self.deposit_arms = value,
            "deposit_twist" => self.deposit_twist = value,
            "deposit_spin" => self.deposit_spin = value,
            "hue" => self.hue = value,
            "color_span" => self.color_span = value,
            "color_center" => self.color_center = value,
            "saturation" => self.saturation = value,
            "palette_mix" => self.palette_mix = value,
            "palette_steps" => self.palette_steps = value,
            "palette_contour" => self.palette_contour = value,
            "brightness" => self.brightness = value,
            _ => {}
        }
    }

    fn set_per_vertex(&mut self, name: &str, values: &[f32]) {
        let Some(index) = PER_VERTEX_PARAMS.iter().position(|n| *n == name) else {
            return;
        };
        let Some(slot) = self.state.values.get_mut(index) else {
            return;
        };
        // A series of the wrong length means the renderer and this scene clamped
        // the grid differently, which `clamp_grid` exists to prevent. Copy what
        // fits and leave the rest at the scalar rather than panicking on the hot
        // path.
        let n = slot.len().min(values.len());
        if let (Some(dst), Some(src)) = (slot.get_mut(..n), values.get(..n)) {
            dst.copy_from_slice(src);
        }
        if let Some(flag) = self.state.bound.get_mut(index) {
            *flag = n > 0;
        }
    }

    fn update(&mut self, _frame: &AnalysisFrame) {}

    fn render(
        &mut self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        aspect: f32,
    ) {
        let size = if self.target == (0, 0) {
            // No `set_target_size` yet (a caller that renders without the
            // renderer's per-frame hook). Fall back to a square field rather
            // than allocating nothing.
            (256, 256)
        } else {
            self.target
        };
        // Build or rebuild: a fresh scene, a resized target, or a preset whose
        // grid differs from the one the buffers were built for.
        let stale = match self.res.as_ref() {
            None => true,
            Some(res) => res.size != size || res.mesh != self.state.mesh,
        };
        if stale {
            self.res = Some(Resources::build(
                &self.device,
                self.surface_format,
                size,
                self.state.mesh,
            ));
            self.palette_dirty = true;
        }
        let Some(res) = self.res.as_mut() else {
            return;
        };

        if res.needs_clear {
            res.encode_clear(encoder);
            queue.write_buffer(
                &res.indices,
                0,
                bytemuck::cast_slice(&build_indices(res.mesh)),
            );
            res.needs_clear = false;
        }
        if self.palette_dirty {
            palette::write_lut(queue, &res.lut_texture_a, &self.palette.lut_a_bytes());
            palette::write_lut(queue, &res.lut_texture_b, &self.palette.lut_b_bytes());
            self.palette_dirty = false;
        }

        // Assemble and upload this frame's mesh.
        self.state.assemble(&self.scalars);
        queue.write_buffer(&res.vertices, 0, bytemuck::cast_slice(&self.state.vertices));

        let dt = self.dt;
        // `decay` is a factor per second, like `fb_zoom`, and is clamped below 1
        // so the field cannot integrate without bound.
        let decay = self.decay.clamp(0.0, MAX_DECAY).powf(dt);
        queue.write_buffer(
            &res.warp_uniform,
            0,
            bytemuck::bytes_of(&WarpUniform {
                misc: [aspect, dt, self.time, self.warp_scale],
                misc2: [decay, self.warp_speed, 0.0, 0.0],
            }),
        );
        queue.write_buffer(
            &res.deposit_uniform,
            0,
            bytemuck::bytes_of(&DepositUniform {
                a: [
                    aspect,
                    self.deposit * dt,
                    self.deposit_radius,
                    self.deposit_width,
                ],
                b: [
                    self.deposit_x,
                    self.deposit_y,
                    self.deposit_arms,
                    self.deposit_twist,
                ],
                c: [
                    self.deposit_spin * self.time,
                    self.hue + self.color_center,
                    self.color_span,
                    self.saturation,
                ],
                d: [
                    self.palette_mix,
                    palette::band_steps(self.palette_steps),
                    palette::band_contour(self.palette_contour),
                    0.0,
                ],
            }),
        );
        queue.write_buffer(
            &res.present_uniform,
            0,
            bytemuck::bytes_of(&PresentUniform {
                a: [self.brightness, self.occlude, 0.0, 0.0],
            }),
        );

        // --- warp: the past, resampled through the mesh, into the write half ---
        let warp_bg = if res.field.reading_a() {
            &res.warp_bg_a
        } else {
            &res.warp_bg_b
        };
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("warp-mesh-warp-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: res.field.write_view(),
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        // The mesh covers the whole target, but clearing first
                        // makes the pass independent of what the buffer held two
                        // frames ago — which is what keeps a capture a pure
                        // function of its inputs.
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&res.warp_pipeline);
            pass.set_bind_group(0, warp_bg, &[]);
            pass.set_vertex_buffer(0, res.vertices.slice(..));
            pass.set_index_buffer(res.indices.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..res.index_count, 0, 0..1);
        }

        // --- deposit: this frame's light, onto the warped past ---
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("warp-mesh-deposit-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: res.field.write_view(),
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&res.deposit_pipeline);
            pass.set_bind_group(0, &res.deposit_bg, &[]);
            pass.draw(0..3, 0..1);
        }

        // The fresh state becomes the next frame's past.
        res.field.swap();

        // --- present: the field, over the backdrop ---
        let present_bg = if res.field.reading_a() {
            &res.present_bg_a
        } else {
            &res.present_bg_b
        };
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("warp-mesh-present-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&res.present_pipeline);
        pass.set_bind_group(0, present_bg, &[]);
        pass.draw(0..3, 0..1);
    }
}

#[cfg(test)]
mod tests;
