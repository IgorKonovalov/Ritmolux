//! **Flat opaque elements painted on their own paper**
//! ([ADR-0123](../../../../docs/adrs/0123-a-flat-graphic-scene-paints-its-own-paper-and-composites-opaque-elements-in-one-pass.md),
//! Plan 0113).
//!
//! Every other scene in this engine draws **light**: premultiplied additive
//! colour into a linear-light composite, where nothing is in front of anything
//! (ADR-0018, ADR-0046, ADR-0056). This one draws a **graphic**. A pixel starts
//! at the paper colour and walks an array of elements *in array order*,
//! compositing each with `over`, so a black bar genuinely sits in front of a red
//! one and **the array index is the depth**. There is no depth buffer, no sort,
//! and no ordering state — the painter's loop is the whole mechanism.
//!
//! # Why this needs no change to the composite
//!
//! Three facts, all measured before this scene existed (ADR-0123's Context):
//!
//! - A fullscreen scene emitting alpha 1 **holds the backdrop out entirely** —
//!   not darkened, absent (Plan 0091 Phase 1). So a scene that covers every
//!   pixel owns its own ground, which is what lets this one paint paper.
//! - The tonemap is **exactly the identity** below
//!   [`KNEE`](crate::render::tonemap::KNEE)` = 0.6` (ADR-0046), so an element
//!   whose brightest channel is at or under it reaches the display untouched.
//!   Flatness is not argued for against this pipeline; below the knee the
//!   pipeline is a no-op.
//! - Bloom's threshold sits **above** that knee, so a canvas living under it
//!   gets no halo and hard edges stay hard, at no cost and with no parameter.
//!   One constraint buys both properties this look is made of.
//!
//! What the curve does not give is paper at pure white: `f(1.0) = 0.800`, and
//! 1.0 is asymptotically unreachable. Both reference grounds are off-white, so
//! that is a property to author against rather than a defect. See
//! `docs/preset-palettes.md`.
//!
//! # Colour is a palette **coordinate**
//!
//! An element stores a coordinate, never an RGB triple, so every palette, custom
//! stop and A/B crossfade in `docs/preset-palettes.md` applies here on arrival
//! with no special case (ADR-0086, ADR-0102). The paper takes a coordinate too,
//! and deliberately a **raw** one: `color_span` and `palette_shift` move the
//! elements' colours and must not drag the ground along with them.
//!
//! # The aspect comes from the render target ([ADR-0037](../../../../docs/adrs/0037-internal-grid-is-a-resolution-not-a-shape.md))
//!
//! This scene computes screen-destined geometry from a normalized space, which
//! is exactly the shape of the bug that has shipped three times in this repo.
//! The canvas is built in square units by stretching NDC x by the **render
//! target's** aspect, so a circle element is round at every window shape. `tests`
//! renders at 1280x800 and measures a circle's own width against its height:
//! 1920x1080 and this box's 2048x1152 are both exactly 16:9, where no test can
//! tell a target-derived aspect from a grid-derived one, and 16:10 is the case
//! that discriminates.
//!
//! # The cost, and where its bound lives
//!
//! The draw is O(elements) per pixel and the bounding-box reject removes the
//! distance evaluation but **not** the loop step, so a wavefront walks every
//! element regardless. That is the plan's real risk, and the bound on it is
//! [`TierConfig::collage_elements`](crate::render::TierConfig::collage_elements)
//! — measured by `core/tests/collage_cost.rs`, not assumed.
//!
//! Two things here exist to keep that loop cheap and are worth not tidying away:
//! the rotation arrives as a precomputed **cosine and sine pair** rather than an
//! angle (no per-pixel-per-element trig, and no dependence on `sin`'s
//! implementation-defined precision, which ADR-0096 disqualifies elsewhere for
//! the same reason), and the axis-aligned bounding box is computed CPU-side and
//! **tight** for every kind — a loose box is a silent cost regression that no
//! picture would show.

// Hot-path panic-denial pragma, as everywhere under `scenes/`.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

use crate::render::gpu;

use super::Scene;
use crate::dsp::AnalysisFrame;
use crate::render::palette::{self, Palette};

/// Element kind selectors, as they reach the shader's `shape.z`. Phase 7 of
/// Plan 0113 extends this roster; these three are what a suprematist canvas is
/// made of.
pub(crate) const KIND_QUAD: f32 = 0.0;
pub(crate) const KIND_CIRCLE: f32 = 1.0;
pub(crate) const KIND_TRIANGLE: f32 = 2.0;

/// Half the base of the unit triangle, i.e. `cos(30 deg)`. The triangle is
/// equilateral and inscribed in the unit circle — apex at `(0, 1)`, base corners
/// at `(+-SQRT3_2, -0.5)` — scaled by the element's half extents. Stated as a
/// constant because both the WGSL and the CPU-side bounding box are built from
/// it, and two spellings of the same vertex is how a box stops being tight.
const SQRT3_2: f32 = 0.866_025_4;

/// `scale` default — the authored canvas is laid out to fill roughly the frame
/// at 1.0, so the neutral value is the composition as composed.
const DEFAULT_SCALE: f32 = 1.0;
/// Smallest `scale` the shader is handed. Not zero: the canvas transform divides
/// by it.
const MIN_SCALE: f32 = 0.05;
/// Largest `scale`. Past this a single element fills the frame and the canvas is
/// no longer a composition — the end of the useful range, not an arbitrary cap.
const MAX_SCALE: f32 = 20.0;

/// Shared view transform (ADR-0018): `pan_*` moves the canvas.
const DEFAULT_PAN: f32 = 0.0;

/// `paper` default — the top of the gradient, which is where a light stop
/// naturally goes and what both reference grounds are.
const DEFAULT_PAPER: f32 = 1.0;

/// Shared palette colour knobs (ADR-0021). Both defaults are the identity on an
/// element's stored coordinate, so an unbound preset gets exactly the colours it
/// authored into its stops.
const DEFAULT_COLOR_SPAN: f32 = 1.0;
const DEFAULT_PALETTE_SHIFT: f32 = 0.0;
const DEFAULT_SATURATION: f32 = 1.0;
/// `palette_mix` default — 0 = palette A only.
const DEFAULT_PALETTE_MIX: f32 = 0.0;

/// `opacity` default — fully opaque, which is the whole point of the scene.
const DEFAULT_OPACITY: f32 = 1.0;

/// `edge_softness` default — **zero, and that is the hard edge**. Coverage comes
/// from the distance against exactly one pixel, so an edge is analytically
/// antialiased and nothing more. Raising this widens the ramp in pixels; it is
/// an escape from the look, not a quality knob.
const DEFAULT_EDGE_SOFTNESS: f32 = 0.0;
/// Widest ramp, in pixels. Past a few pixels the elements stop reading as flat
/// graphics at all.
const MAX_EDGE_SOFTNESS: f32 = 32.0;

const SHADER: &str = r#"
struct Params {
    // x: aspect (from the RENDER TARGET, ADR-0037), y: live element count
    // (integral, quantized CPU-side), z: scale, w: edge softness in pixels
    a: vec4<f32>,
    // xy: pan (the shared ViewTransform, ADR-0018), z: color_span,
    // w: palette_shift
    b: vec4<f32>,
    // x: saturation, y: palette_mix, z: opacity, w: paper (a RAW palette
    // coordinate — see the module docs on why span/shift do not touch it)
    c: vec4<f32>,
    // x: occlude (ADR-0085), yzw: reserved.
    d: vec4<f32>,
}

// One flat element, 64 bytes. **The array is the painter's order, so the index
// IS the depth.**
struct Element {
    // cx, cy, half_x, half_y — canvas space
    center_size: vec4<f32>,
    // cos(angle), sin(angle), kind, p0 (kind-specific)
    shape: vec4<f32>,
    // palette coordinate, alpha, birth, p1 (kind-specific)
    tint: vec4<f32>,
    // x0, y0, x1, y1 — the precomputed TIGHT reject box, canvas space
    aabb: vec4<f32>,
}

// **A bind-group layout shape nothing else in the crate holds** (ADR-0058: two
// byte-identical layouts alias on the DX12 WARP adapter, and the whole golden
// suite runs there, so a collision would be blessed rather than caught). The
// fragment-visible read-only storage buffer is the discriminator — no other
// layout here binds storage outside a compute stage — so keep binding 4 where
// it is rather than tidying the group.
@group(0) @binding(0) var lut_samp: sampler;
@group(0) @binding(1) var lut_a: texture_2d<f32>;
@group(0) @binding(2) var lut_b: texture_2d<f32>;
@group(0) @binding(3) var<uniform> params: Params;
@group(0) @binding(4) var<storage, read> elements: array<Element>;

// Shared `saturation` (mirrors core/src/render/palette.rs::desaturate verbatim).
fn apply_saturation(c: vec3<f32>, s: f32) -> vec3<f32> {
    let luma = dot(c, vec3<f32>(0.299, 0.587, 0.114));
    return vec3<f32>(luma) + (c - vec3<f32>(luma)) * s;
}

// The crossfaded palette at a coordinate.
//
// `textureSampleLevel`, not `textureSample`, and that is a requirement rather
// than a preference: this is called from inside the element loop, which is
// non-uniform control flow, where an implicit-derivative sample is invalid. The
// LUT is 256x1 with one mip, so level 0 is the whole texture.
fn palette_at(t: f32) -> vec3<f32> {
    let ca = textureSampleLevel(lut_a, lut_samp, vec2<f32>(t, 0.5), 0.0).rgb;
    let cb = textureSampleLevel(lut_b, lut_samp, vec2<f32>(t, 0.5), 0.0).rgb;
    return mix(ca, cb, clamp(params.c.y, 0.0, 1.0));
}

// Exact signed distance to an axis-aligned box of half extents h.
fn sd_box(p: vec2<f32>, h: vec2<f32>) -> f32 {
    let q = abs(p) - h;
    return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0);
}

// An ellipse's distance, approximated by the unit-circle distance scaled back by
// the smaller half axis. **Exact when hx == hy**, which is the circle case the
// aspect test measures; elliptical elements get a distance that is correct in
// sign and slightly conservative in magnitude, which moves an edge by well under
// the one pixel the coverage ramp spans. The exact ellipse distance is iterative
// and this runs per pixel per element.
fn sd_ellipse(p: vec2<f32>, h: vec2<f32>) -> f32 {
    return (length(p / h) - 1.0) * min(h.x, h.y);
}

// Exact signed distance to the triangle with the three given vertices
// (Inigo Quilez's `sdTriangle`). Exact rather than a unit-space approximation
// because the vertices are also what the CPU-side bounding box is built from,
// so the box is tight by construction.
fn sd_triangle(p: vec2<f32>, v0: vec2<f32>, v1: vec2<f32>, v2: vec2<f32>) -> f32 {
    let e0 = v1 - v0;
    let e1 = v2 - v1;
    let e2 = v0 - v2;
    let w0 = p - v0;
    let w1 = p - v1;
    let w2 = p - v2;
    let q0 = w0 - e0 * clamp(dot(w0, e0) / dot(e0, e0), 0.0, 1.0);
    let q1 = w1 - e1 * clamp(dot(w1, e1) / dot(e1, e1), 0.0, 1.0);
    let q2 = w2 - e2 * clamp(dot(w2, e2) / dot(e2, e2), 0.0, 1.0);
    let s = sign(e0.x * e2.y - e0.y * e2.x);
    var d = min(
        vec2<f32>(dot(q0, q0), s * (w0.x * e0.y - w0.y * e0.x)),
        vec2<f32>(dot(q1, q1), s * (w1.x * e1.y - w1.y * e1.x)),
    );
    d = min(d, vec2<f32>(dot(q2, q2), s * (w2.x * e2.y - w2.y * e2.x)));
    return -sqrt(d.x) * sign(d.y);
}

// One element's signed distance at canvas point p.
fn element_distance(e: Element, p: vec2<f32>) -> f32 {
    let h = max(e.center_size.zw, vec2<f32>(1e-5));
    let ca = e.shape.x;
    let sa = e.shape.y;
    let d = p - e.center_size.xy;
    // Into the element's own frame: rotate by -angle, with the pair the CPU
    // precomputed (see the module docs).
    let q = vec2<f32>(ca * d.x + sa * d.y, -sa * d.x + ca * d.y);
    let kind = e.shape.z;
    if (kind < 0.5) {
        return sd_box(q, h);
    }
    if (kind < 1.5) {
        return sd_ellipse(q, h);
    }
    let s3 = 0.8660254;
    return sd_triangle(
        q,
        vec2<f32>(0.0, h.y),
        vec2<f32>(-s3 * h.x, -0.5 * h.y),
        vec2<f32>(s3 * h.x, -0.5 * h.y),
    );
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let aspect = params.a.x;
    let count = u32(max(params.a.y, 0.0));
    let scale = params.a.z;
    let softness = params.a.w;
    let pan = params.b.xy;
    let color_span = params.b.z;
    let palette_shift = params.b.w;
    let saturation = params.c.x;
    let opacity = clamp(params.c.z, 0.0, 1.0);

    // Square units, from the RENDER TARGET's aspect (ADR-0037): stretching x
    // makes one unit the same length on both axes, so a circle element is round
    // and not the window's shape.
    var uv = in.ndc;
    uv.x = uv.x * aspect;

    // One pixel, in uv units, taken under UNIFORM control flow — `fwidth` inside
    // the loop below would be invalid. `uv` is linear in the fragment position,
    // so this is a constant across the frame and equals `2/height` on both axes.
    let pixel_uv = fwidth(uv.x);

    // The canvas frame: `pan` moves it, `scale` sets its size. Elements and
    // their reject boxes live here, so neither is re-derived per frame.
    let p = (uv - pan) / scale;
    // The coverage ramp, in canvas units. Exactly one pixel wide at
    // `edge_softness = 0`, which is the hard edge this scene is for.
    let pw = max(pixel_uv * (1.0 + softness) / scale, 1e-7);

    // The paper. A RAW coordinate — span and shift are the elements' knobs.
    var col = palette_at(params.c.w);

    // **The painter's loop.** Array order is depth order; this is the whole of
    // the occlusion mechanism.
    for (var i = 0u; i < count; i = i + 1u) {
        let e = elements[i];
        let bb = e.aabb;
        // The reject. It saves the distance evaluation, not the iteration —
        // which is exactly why the element cap is load-bearing (ADR-0123).
        if (p.x < bb.x - pw || p.x > bb.z + pw || p.y < bb.y - pw || p.y > bb.w + pw) {
            continue;
        }
        let d = element_distance(e, p);
        // Analytic antialiasing: a box filter one pixel wide across the edge.
        // Full coverage half a pixel inside, none half a pixel outside, and
        // correct under arbitrary rotation because a rotation preserves distance.
        let cov = clamp(0.5 - d / pw, 0.0, 1.0) * clamp(e.tint.y, 0.0, 1.0) * opacity;
        if (cov <= 0.0) {
            continue;
        }
        let c = palette_at(e.tint.x * color_span + palette_shift);
        col = mix(col, c, cov);
    }

    col = apply_saturation(col, saturation);

    // Alpha: this canvas covers every pixel, which is the coverage it honestly
    // has (ADR-0056) and what holds the backdrop out so the paper is the ground.
    // `occlude` scales how much of it the backdrop resolves against (ADR-0085);
    // reached only when no post stage is active, since the chain owns that seam
    // otherwise and the renderer hands a literal 1.0.
    return vec4<f32>(col, params.d.x);
}
"#;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Params {
    a: [f32; 4],
    b: [f32; 4],
    c: [f32; 4],
    d: [f32; 4],
}

/// One flat element, exactly as the shader's `Element` reads it. 64 bytes,
/// 16-byte aligned, so the storage array's stride needs no padding.
///
/// The array **is** the painter's order: index is depth.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct Element {
    /// `cx, cy, half_x, half_y` — canvas space.
    pub(crate) center_size: [f32; 4],
    /// `cos(angle), sin(angle), kind, p0`. The rotation is precomputed here
    /// rather than passed as an angle: it takes a trig pair out of the innermost
    /// loop, and it keeps the geometry off `sin`'s implementation-defined
    /// precision, which ADR-0096 rules out for the same reason elsewhere.
    pub(crate) shape: [f32; 4],
    /// `palette coordinate, alpha, birth, p1`.
    pub(crate) tint: [f32; 4],
    /// `x0, y0, x1, y1` — the reject box, **tight** for every kind.
    pub(crate) aabb: [f32; 4],
}

/// A hand-authored element, in the terms a composition is written in. Turned
/// into an [`Element`] — bounding box and all — by [`Element::build`].
#[derive(Clone, Copy, Debug)]
pub(crate) struct Spec {
    /// One of [`KIND_QUAD`], [`KIND_CIRCLE`], [`KIND_TRIANGLE`].
    pub(crate) kind: f32,
    /// Centre, canvas space.
    pub(crate) center: [f32; 2],
    /// Half extents before rotation, canvas space.
    pub(crate) half: [f32; 2],
    /// Rotation in **degrees**, counter-clockwise. Degrees because a composition
    /// is authored in them and this is off the hot path.
    pub(crate) angle_deg: f32,
    /// Palette coordinate.
    pub(crate) coord: f32,
    /// Per-element alpha. `1.0` is the opaque element this scene is for; below
    /// it the crossing is an `over` composite of both (Plan 0113 Phase 7).
    pub(crate) alpha: f32,
}

impl Element {
    /// Build the GPU element for a spec, computing the **tight** axis-aligned
    /// bounding box of the rotated shape.
    ///
    /// Tightness is per-kind and exact, not a shared conservative box: a loose
    /// box costs a distance evaluation at every pixel it wrongly admits, which
    /// is a cost regression no rendered frame would reveal. A test asserts it
    /// for every kind.
    pub(crate) fn build(spec: Spec) -> Element {
        let angle = spec.angle_deg.to_radians();
        let (sa, ca) = angle.sin_cos();
        let [cx, cy] = spec.center;
        let [hx, hy] = [spec.half[0].abs(), spec.half[1].abs()];

        let (ex, ey) = if spec.kind < 0.5 {
            // A rotated rectangle: the support function of the four corners.
            (ca.abs() * hx + sa.abs() * hy, sa.abs() * hx + ca.abs() * hy)
        } else if spec.kind < 1.5 {
            // A rotated ellipse: the exact extent of the parametric form.
            (
                ((hx * ca) * (hx * ca) + (hy * sa) * (hy * sa)).sqrt(),
                ((hx * sa) * (hx * sa) + (hy * ca) * (hy * ca)).sqrt(),
            )
        } else {
            // The triangle: the extent of its three rotated vertices. Built from
            // the same three points the shader's distance uses.
            let verts = triangle_vertices(hx, hy);
            let mut ex = 0.0f32;
            let mut ey = 0.0f32;
            for [vx, vy] in verts {
                ex = ex.max((ca * vx - sa * vy).abs());
                ey = ey.max((sa * vx + ca * vy).abs());
            }
            (ex, ey)
        };

        Element {
            center_size: [cx, cy, hx, hy],
            shape: [ca, sa, spec.kind, 0.0],
            tint: [spec.coord, spec.alpha, 0.0, 0.0],
            aabb: [cx - ex, cy - ey, cx + ex, cy + ey],
        }
    }
}

/// The unit triangle's three vertices, scaled by half extents — apex up, base
/// down. **The shader's `element_distance` builds the same three points**, and
/// the two must not drift: this is what makes the triangle's bounding box tight
/// rather than approximately right.
pub(crate) fn triangle_vertices(hx: f32, hy: f32) -> [[f32; 2]; 3] {
    [
        [0.0, hy],
        [-SQRT3_2 * hx, -0.5 * hy],
        [SQRT3_2 * hx, -0.5 * hy],
    ]
}

/// **The authored canvas** — a suprematist composition of fourteen elements, in
/// painter order (later entries sit in front).
///
/// Fourteen is counted rather than chosen: ADR-0123 counts 14 elements in
/// Malevich's *Suprematism*, the sparsest canvas in the reference set, and this
/// is that density.
///
/// This list is **Plan 0113 Phase 1's element source and nothing more.** Phase 4
/// replaces it with the seeded layout grammar; until then it is what makes the
/// scene's first user-visible behaviour a static canvas rather than a blank one,
/// and it is what the golden fixture pins.
const SUPREMATIST: &[Spec] = &[
    // The ground of the composition: a broad blue plane on the dominant angle.
    Spec {
        kind: KIND_QUAD,
        center: [-0.15, 0.10],
        half: [0.62, 0.115],
        angle_deg: -22.0,
        coord: 0.4375,
        alpha: 1.0,
    },
    // The black bar that crosses it — the occlusion this scene exists for.
    Spec {
        kind: KIND_QUAD,
        center: [0.05, -0.05],
        half: [0.72, 0.075],
        angle_deg: -22.0,
        coord: 0.0625,
        alpha: 1.0,
    },
    Spec {
        kind: KIND_QUAD,
        center: [-0.30, 0.42],
        half: [0.40, 0.045],
        angle_deg: -22.0,
        coord: 0.3125,
        alpha: 1.0,
    },
    Spec {
        kind: KIND_QUAD,
        center: [0.30, 0.30],
        half: [0.16, 0.160],
        angle_deg: 12.0,
        coord: 0.1875,
        alpha: 1.0,
    },
    Spec {
        kind: KIND_CIRCLE,
        center: [-0.55, -0.32],
        half: [0.13, 0.130],
        angle_deg: 0.0,
        coord: 0.0625,
        alpha: 1.0,
    },
    Spec {
        kind: KIND_QUAD,
        center: [0.42, -0.34],
        half: [0.26, 0.035],
        angle_deg: -22.0,
        coord: 0.5625,
        alpha: 1.0,
    },
    Spec {
        kind: KIND_TRIANGLE,
        center: [-0.05, -0.52],
        half: [0.16, 0.180],
        angle_deg: 8.0,
        coord: 0.6875,
        alpha: 1.0,
    },
    Spec {
        kind: KIND_QUAD,
        center: [-0.62, 0.02],
        half: [0.30, 0.028],
        angle_deg: 62.0,
        coord: 0.1875,
        alpha: 1.0,
    },
    Spec {
        kind: KIND_QUAD,
        center: [0.62, 0.08],
        half: [0.10, 0.220],
        angle_deg: -22.0,
        coord: 0.8125,
        alpha: 1.0,
    },
    Spec {
        kind: KIND_QUAD,
        center: [0.15, 0.52],
        half: [0.09, 0.090],
        angle_deg: 40.0,
        coord: 0.0625,
        alpha: 1.0,
    },
    Spec {
        kind: KIND_CIRCLE,
        center: [0.58, -0.62],
        half: [0.075, 0.075],
        angle_deg: 0.0,
        coord: 0.1875,
        alpha: 1.0,
    },
    // Down in the empty lower-left rather than beside the ochre bar's end: at
    // its first placement it touched that bar, and two same-coloured elements
    // meeting merge into one silhouette — there, an arrow, which is a
    // representational shape this style exists to refuse.
    Spec {
        kind: KIND_TRIANGLE,
        center: [-0.88, -0.58],
        half: [0.12, 0.140],
        angle_deg: -18.0,
        coord: 0.3125,
        alpha: 1.0,
    },
    Spec {
        kind: KIND_QUAD,
        center: [0.00, -0.72],
        half: [0.34, 0.022],
        angle_deg: -22.0,
        coord: 0.4375,
        alpha: 1.0,
    },
    Spec {
        kind: KIND_TRIANGLE,
        center: [0.80, 0.42],
        half: [0.10, 0.120],
        angle_deg: 190.0,
        coord: 0.0625,
        alpha: 1.0,
    },
];

/// How many elements the authored canvas holds — the default `count`, and what
/// [`Grammar::Authored`](layout::Grammar::Authored) cycles.
pub(crate) const AUTHORED_COUNT: usize = SUPREMATIST.len();

/// `layout` default — the authored control, not a grammar (see `layout.rs`).
const DEFAULT_LAYOUT: f32 = 0.0;
/// `seed` default.
const DEFAULT_SEED: f32 = 0.0;
/// `size_hierarchy` default — a middling fall from the largest form to the
/// smallest, so the generated grammars have a range without being dominated.
const DEFAULT_SIZE_HIERARCHY: f32 = 0.5;
/// `angle_bias` default, in **degrees** as an author writes it. `-22` is the
/// authored canvas's own dominant angle, so a generated canvas starts out
/// leaning the same way the control does.
const DEFAULT_ANGLE_BIAS: f32 = -22.0;

/// The largest `seed` a preset can name. `f32` represents integers exactly to
/// `2^24`, and past that a "different seed" silently is not one — so the range
/// ends where the type stops being able to tell two seeds apart.
const MAX_SEED: f32 = 16_777_216.0;

/// The flat-graphic canvas: opaque elements over their own paper, composited in
/// one fullscreen distance-field pass.
pub struct ShapeCollageScene {
    pipeline: wgpu::RenderPipeline,
    uniforms: wgpu::Buffer,
    storage: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    lut_texture_a: wgpu::Texture,
    lut_texture_b: wgpu::Texture,
    palette: Palette,
    palette_dirty: bool,
    /// The element array, preallocated to the tier cap and refilled in place —
    /// never reallocated (the plan's no-allocation-in-the-render-path duty).
    elements: Vec<Element>,
    /// The recipe [`Self::elements`] was last built from, so an unchanged canvas
    /// neither regenerates nor re-uploads. `None` before the first build.
    built: Option<layout::Recipe>,
    /// Whether [`Self::elements`] has changed since the last upload.
    dirty: bool,
    /// A test has installed its own element array, so the canvas must not be
    /// rebuilt from the authored roster.
    ///
    /// **`#[cfg(test)]`, and that gate is the whole justification** — the same
    /// argument `Scene::feedback_field` carries. Order *is* the occlusion
    /// mechanism here, so the assertion that proves it works has to render two
    /// chosen elements in both array orders; nothing a preset can say reverses
    /// a compiled-in roster. This field does not exist in a shipped build.
    #[cfg(test)]
    specs_override: bool,
    /// The live element count, raw as the preset bound it. Quantized on the way
    /// to the rebuild — an eased binding is continuous even where the arithmetic
    /// needs an integer.
    count: f32,
    /// Which layout grammar composes the canvas, raw as the preset bound it.
    /// `layout::Grammar::from_param` quantizes it.
    layout: f32,
    /// The preset's seed, raw as the preset bound it.
    seed: f32,
    /// How steeply generated sizes fall, raw as the preset bound it.
    size_hierarchy: f32,
    /// The canvas's dominant angle in **degrees**, raw as the preset bound it.
    angle_bias: f32,
    scale: f32,
    pan_x: f32,
    pan_y: f32,
    paper: f32,
    color_span: f32,
    palette_shift: f32,
    saturation: f32,
    palette_mix: f32,
    opacity: f32,
    edge_softness: f32,
    /// How much of this canvas's (total) coverage the backdrop resolves against
    /// (ADR-0085). Set by the renderer every frame — **not** a named param, so
    /// it is not reset by `reset_params`.
    occlude: f32,
}

impl ShapeCollageScene {
    /// Build the scene's pipeline, uniform buffer and element storage on
    /// `device`. `cap` is the tier's element cap and sizes both the storage
    /// buffer and the CPU vector, so neither grows afterwards.
    pub fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat, cap: usize) -> Self {
        // At least one element's worth: a zero-length storage buffer is invalid,
        // and a tier could in principle name a small cap.
        let cap = cap.max(1);
        let shader = gpu::fullscreen_shader(
            device,
            "shape-collage-shader",
            gpu::FULLSCREEN_VS_NDC,
            SHADER,
        );
        let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("shape-collage-params"),
            size: std::mem::size_of::<Params>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let storage = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("shape-collage-elements"),
            size: (cap * std::mem::size_of::<Element>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let lut_texture_a = palette::lut_texture(device, "shape-collage-lut-a");
        let lut_texture_b = palette::lut_texture(device, "shape-collage-lut-b");
        let lut_view_a = lut_texture_a.create_view(&wgpu::TextureViewDescriptor::default());
        let lut_view_b = lut_texture_b.create_view(&wgpu::TextureViewDescriptor::default());
        let lut_sampler = palette::lut_sampler(device);
        // See the WGSL's note: the fragment-visible storage buffer is what makes
        // this layout's shape unique in the crate (ADR-0058). Both buffer entries
        // are full literals so each declares a `min_binding_size`, which Plan
        // 0053 Phase 3 measured to be half of what separates two layouts on WARP.
        let bind_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("shape-collage-bind-layout"),
            entries: &[
                gpu::sampler(0),
                gpu::texture(1, true),
                gpu::texture(2, true),
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(
                            std::mem::size_of::<Params>() as u64
                        ),
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(
                            std::mem::size_of::<Element>() as u64
                        ),
                    },
                    count: None,
                },
            ],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("shape-collage-bind-group"),
            layout: &bind_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Sampler(&lut_sampler),
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
                    resource: uniforms.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: storage.as_entire_binding(),
                },
            ],
        });
        let pipeline = gpu::fullscreen_pipeline(
            device,
            &shader,
            &[&bind_layout],
            surface_format,
            wgpu::BlendState::REPLACE,
            "shape-collage",
        );

        Self {
            pipeline,
            uniforms,
            storage,
            bind_group,
            lut_texture_a,
            lut_texture_b,
            palette: Palette::default_spectrum(),
            palette_dirty: true,
            elements: Vec::with_capacity(cap),
            built: None,
            dirty: false,
            #[cfg(test)]
            specs_override: false,
            count: AUTHORED_COUNT as f32,
            layout: DEFAULT_LAYOUT,
            seed: DEFAULT_SEED,
            size_hierarchy: DEFAULT_SIZE_HIERARCHY,
            angle_bias: DEFAULT_ANGLE_BIAS,
            scale: DEFAULT_SCALE,
            pan_x: DEFAULT_PAN,
            pan_y: DEFAULT_PAN,
            paper: DEFAULT_PAPER,
            color_span: DEFAULT_COLOR_SPAN,
            palette_shift: DEFAULT_PALETTE_SHIFT,
            saturation: DEFAULT_SATURATION,
            palette_mix: DEFAULT_PALETTE_MIX,
            opacity: DEFAULT_OPACITY,
            edge_softness: DEFAULT_EDGE_SOFTNESS,
            occlude: crate::render::post::DEFAULT_OCCLUDE,
        }
    }

    /// The canvas this scene's parameters currently describe.
    ///
    /// Every field is conditioned **here**, CPU-side: a grammar selector, an
    /// element count and a seed all need to be integral, and an eased binding
    /// sweeps continuously through the values in between.
    fn recipe(&self) -> layout::Recipe {
        layout::Recipe {
            grammar: layout::Grammar::from_param(self.layout),
            count: applied_count(self.count, self.elements.capacity()),
            seed: applied_seed(self.seed),
            // Phase 6 advances this on a rising edge of `recompose`; until then
            // every canvas is the seed's first composition.
            recompose: 0,
            size_hierarchy: applied_size_hierarchy(self.size_hierarchy),
            angle_bias: applied_angle_bias(self.angle_bias),
        }
    }

    /// Regenerate the element array if the recipe moved, unless a test has
    /// installed its own. A no-op when nothing changed, so the common frame
    /// neither regenerates nor re-uploads.
    fn rebuild(&mut self) {
        #[cfg(test)]
        if self.specs_override {
            return;
        }
        let recipe = self.recipe();
        if self.built == Some(recipe) {
            return;
        }
        layout::generate(&mut self.elements, &recipe);
        self.built = Some(recipe);
        self.dirty = true;
    }

    /// Install an element array of the test's own, in painter order, in place of
    /// the authored canvas. See [`Self::specs_override`] for why this exists.
    #[cfg(test)]
    pub(crate) fn set_specs(&mut self, specs: &[Spec]) {
        self.elements.clear();
        for &spec in specs.iter().take(self.elements.capacity()) {
            self.elements.push(Element::build(spec));
        }
        self.built = None;
        self.dirty = true;
        self.specs_override = true;
    }
}

/// The `scale` the shader is handed: held inside the range the canvas transform
/// needs, with a non-finite binding falling back to the default.
///
/// CPU-side for `shape_field::applied_scale`'s reasons — it can never be reached
/// with a NaN, where WGSL's `clamp` is implementation-defined, and the default
/// stays **exactly** the default on the way to the uniform.
fn applied_scale(scale: f32) -> f32 {
    if scale.is_finite() {
        scale.clamp(MIN_SCALE, MAX_SCALE)
    } else {
        DEFAULT_SCALE
    }
}

/// The live element count for a bound `count`, held in `0..=cap`.
///
/// **Quantized here rather than in the shader**, because an eased binding sweeps
/// continuously through values the arithmetic needs to be integral — the hazard
/// the kaleidoscope seam was fixed for. A non-finite binding falls back to the
/// authored canvas rather than to zero: a blank frame is the worse failure.
fn applied_count(count: f32, cap: usize) -> usize {
    if !count.is_finite() {
        return AUTHORED_COUNT.min(cap);
    }
    // `floor` and not `round`: a `count` easing from 14 toward 20 should admit
    // the fifteenth element when it has actually arrived.
    let n = count.floor();
    if n <= 0.0 {
        return 0;
    }
    (n as usize).min(cap)
}

/// The generator's seed for a bound `seed`.
///
/// Truncated to a whole number and held in `0..=`[`MAX_SEED`]. The ceiling is
/// not arbitrary: an `f32` represents integers exactly only to `2^24`, so past
/// it two "different" seeds can be the same value and a `recompose` would stop
/// advancing. A non-finite binding falls back to the default rather than to
/// whatever `as u64` makes of a NaN.
fn applied_seed(seed: f32) -> u64 {
    if !seed.is_finite() {
        return DEFAULT_SEED as u64;
    }
    seed.clamp(0.0, MAX_SEED).floor() as u64
}

/// The size-hierarchy exponent's input, held in `0..=1`.
fn applied_size_hierarchy(hierarchy: f32) -> f32 {
    if hierarchy.is_finite() {
        hierarchy.clamp(0.0, 1.0)
    } else {
        DEFAULT_SIZE_HIERARCHY
    }
}

/// The canvas's dominant angle in **radians**, from a param an author writes in
/// degrees. Wrapped rather than clamped — an angle has no ends, and a `spin`
/// binding walking past 360 must not stick at a bound.
fn applied_angle_bias(degrees: f32) -> f32 {
    if degrees.is_finite() {
        degrees.rem_euclid(360.0).to_radians()
    } else {
        DEFAULT_ANGLE_BIAS.to_radians()
    }
}

/// The `edge_softness` the shader is handed, in pixels of extra ramp.
fn applied_edge_softness(softness: f32) -> f32 {
    if softness.is_finite() {
        softness.clamp(0.0, MAX_EDGE_SOFTNESS)
    } else {
        DEFAULT_EDGE_SOFTNESS
    }
}

/// The parameter names this scene consumes — the vocabulary a preset binding is
/// checked against at load (ADR-0020). **Keep in sync with `set_param` below**;
/// `declared_params_match_set_param` in `core/tests/preset.rs` fails if the two
/// drift.
pub const PARAMS: &[&str] = &[
    "count",
    "layout",
    "seed",
    "size_hierarchy",
    "angle_bias",
    "scale",
    "pan_x",
    "pan_y",
    "paper",
    "color_span",
    "palette_shift",
    "saturation",
    "palette_mix",
    "opacity",
    "edge_softness",
];

impl Scene for ShapeCollageScene {
    fn name(&self) -> &'static str {
        "shape collage"
    }

    fn set_occlude(&mut self, occlude: f32) {
        self.occlude = occlude;
    }

    fn set_palette(&mut self, palette: &Palette) {
        self.palette = palette.clone();
        self.palette_dirty = true;
    }

    fn reset_params(&mut self) {
        self.count = AUTHORED_COUNT as f32;
        self.layout = DEFAULT_LAYOUT;
        self.seed = DEFAULT_SEED;
        self.size_hierarchy = DEFAULT_SIZE_HIERARCHY;
        self.angle_bias = DEFAULT_ANGLE_BIAS;
        self.scale = DEFAULT_SCALE;
        self.pan_x = DEFAULT_PAN;
        self.pan_y = DEFAULT_PAN;
        self.paper = DEFAULT_PAPER;
        self.color_span = DEFAULT_COLOR_SPAN;
        self.palette_shift = DEFAULT_PALETTE_SHIFT;
        self.saturation = DEFAULT_SATURATION;
        self.palette_mix = DEFAULT_PALETTE_MIX;
        self.opacity = DEFAULT_OPACITY;
        self.edge_softness = DEFAULT_EDGE_SOFTNESS;
    }

    fn set_param(&mut self, name: &str, value: f32) {
        match name {
            "count" => self.count = value,
            "layout" => self.layout = value,
            "seed" => self.seed = value,
            "size_hierarchy" => self.size_hierarchy = value,
            "angle_bias" => self.angle_bias = value,
            "scale" => self.scale = value,
            "pan_x" => self.pan_x = value,
            "pan_y" => self.pan_y = value,
            "paper" => self.paper = value,
            "color_span" => self.color_span = value,
            "palette_shift" => self.palette_shift = value,
            "saturation" => self.saturation = value,
            "palette_mix" => self.palette_mix = value,
            "opacity" => self.opacity = value,
            "edge_softness" => self.edge_softness = value,
            _ => {}
        }
    }

    fn update(&mut self, _frame: &AnalysisFrame) {
        // Fully parameter-driven; the analysis reaches this scene only through
        // the preset expressions bound to its parameters.
    }

    fn render(
        &mut self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        aspect: f32,
    ) {
        if self.palette_dirty {
            palette::write_lut(queue, &self.lut_texture_a, &self.palette.lut_a_bytes());
            palette::write_lut(queue, &self.lut_texture_b, &self.palette.lut_b_bytes());
            self.palette_dirty = false;
        }

        self.rebuild();
        // A zero-length write is not a legal `write_buffer`, and an empty canvas
        // needs no upload: the loop reads `count` entries and stops.
        if self.dirty && !self.elements.is_empty() {
            queue.write_buffer(&self.storage, 0, bytemuck::cast_slice(&self.elements));
            self.dirty = false;
        }

        let params = Params {
            // `aspect` is the argument the chain hands down for the target this
            // scene draws into — never a size this scene chose (ADR-0037).
            a: [
                aspect.max(0.1),
                self.elements.len() as f32,
                applied_scale(self.scale),
                applied_edge_softness(self.edge_softness),
            ],
            b: [self.pan_x, self.pan_y, self.color_span, self.palette_shift],
            c: [self.saturation, self.palette_mix, self.opacity, self.paper],
            d: [self.occlude, 0.0, 0.0, 0.0],
        };
        queue.write_buffer(&self.uniforms, 0, bytemuck::bytes_of(&params));

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("shape-collage-pass"),
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
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}

pub(crate) mod layout;

#[cfg(test)]
mod tests;
