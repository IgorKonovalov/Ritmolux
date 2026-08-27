//! **Flat opaque elements painted on their own paper** (ADR-0123).
//!
//! Every other scene in this engine draws **light**: premultiplied additive
//! colour into a linear-light composite, where nothing is in front of anything
//! (ADR-0018, ADR-0046, ADR-0056). This one draws a **graphic**. A pixel starts
//! at the paper colour and walks an array of elements *in array order*,
//! compositing each with `over`, so a black bar genuinely sits in front of a red
//! one and **the array index is the depth**. There is no depth buffer, no sort,
//! and no ordering state — the painter's loop is the whole mechanism.
//!
//! # Three engine properties this look rests on, and breaks without
//!
//! Measured in ADR-0123's Context; named here because each is a thing an
//! unrelated edit could take away.
//!
//! - A fullscreen scene emitting alpha 1 **holds the backdrop out entirely** —
//!   not darkened, absent. So a scene covering every pixel owns its own ground.
//! - The tonemap is **exactly the identity** below
//!   [`KNEE`](crate::render::tonemap::KNEE)` = 0.6` (ADR-0046), so an element at
//!   or under it leaves the post chain **unshaded**. Below the knee the pipeline
//!   is a no-op — flatness is not argued for against it.
//! - Bloom's threshold sits **above** that knee, so a canvas living under it gets
//!   no halo and hard edges stay hard, at no cost and no parameter.
//!
//! **This is not the claim that the authored hex reaches the display.** A
//! palette stop is a linear coefficient with no sRGB decode
//! (`docs/preset-palettes.md`), so the byte written is its sRGB encoding and
//! `#111111` presents as `#494949`. Same curve for every element, no shading and
//! no halo — that is the property the look rests on. Nor does the curve give
//! paper at pure white: `f(1.0) = 0.800`, and 1.0 is asymptotically unreachable,
//! so both reference grounds are off-white by construction.
//!
//! # Colour is a palette **coordinate**
//!
//! An element stores a coordinate, never an RGB triple, so every palette, custom
//! stop and A/B crossfade in `docs/preset-palettes.md` applies here on arrival
//! with no special case (ADR-0086, ADR-0102). The paper takes a coordinate too,
//! and deliberately a **raw** one: `color_span` and `palette_shift` move the
//! elements' colours and must not drag the ground along with them.
//!
//! # The aspect comes from the render target (ADR-0037)
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
//! element regardless. The bound is
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
/// The Kandinsky half of the roster (Plan 0113 Phase 7). `sdf.rs` carries what
/// each kind's half extents and `p0`/`p1` mean.
pub(crate) const KIND_BAR: f32 = 3.0;
pub(crate) const KIND_RING: f32 = 4.0;
pub(crate) const KIND_SEGMENT: f32 = 5.0;
pub(crate) const KIND_ARC: f32 = 6.0;
pub(crate) const KIND_CHECKER: f32 = 7.0;

/// Every kind, for the rendered sweep that must cover the roster.
///
/// `#[cfg(test)]`: the shipped painter selects a kind by number in WGSL and has
/// no use for a Rust roster, so this exists only so the box check cannot quietly
/// stop covering a kind someone added.
#[cfg(test)]
pub(crate) const ALL_KINDS: [f32; 8] = [
    KIND_QUAD,
    KIND_CIRCLE,
    KIND_TRIANGLE,
    KIND_BAR,
    KIND_RING,
    KIND_SEGMENT,
    KIND_ARC,
    KIND_CHECKER,
];

/// The name each kind goes by in `presets/README.md` and in a failure message.
/// `#[cfg(test)]` for [`ALL_KINDS`]'s reason — nothing shipped names a kind.
#[cfg(test)]
pub(crate) fn kind_name(kind: f32) -> &'static str {
    match kind as i32 {
        0 => "quad",
        1 => "circle",
        2 => "triangle",
        3 => "bar",
        4 => "ring",
        5 => "segment",
        6 => "arc",
        _ => "checker",
    }
}

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
/// Largest `scale`. Past this a single element fills the frame and the
/// canvas stops being a composition — the end of the useful range, not
/// an arbitrary cap.
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

// `Element` and every distance function are declared by the chunk `sdf.rs`
// splices in ahead of this body — the struct travels with the functions that
// read it, which is also what makes the chunk parse on its own.

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
#[derive(Clone, Copy, Debug, PartialEq)]
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
    /// Kind-specific shape parameters — `sdf.rs`'s table says what each kind
    /// reads. `segment` and `arc` take their half-aperture in radians from `p0`;
    /// `checker` takes its cells-per-axis from `p1`. Inert on every other kind,
    /// and nothing warns: that is the roster's own documentation's job.
    pub(crate) p0: f32,
    pub(crate) p1: f32,
}

impl Spec {
    /// A spec with the kind-specific parameters at their defaults, which is what
    /// the three original kinds want and what a caller that does not care
    /// should write.
    #[cfg(test)]
    pub(crate) fn new(
        kind: f32,
        center: [f32; 2],
        half: [f32; 2],
        angle_deg: f32,
        coord: f32,
        alpha: f32,
    ) -> Spec {
        Spec {
            kind,
            center,
            half,
            angle_deg,
            coord,
            alpha,
            p0: DEFAULT_APERTURE,
            p1: DEFAULT_CHECKER_CELLS,
        }
    }
}

/// `segment` and `arc` half-aperture default, in radians — a quarter turn either
/// side, so an unparameterised sector is a half disc rather than a sliver or a
/// whole one.
const DEFAULT_APERTURE: f32 = std::f32::consts::FRAC_PI_2;
/// `checker` cells per axis, default. Even, for the reason `checker_cells`
/// gives.
const DEFAULT_CHECKER_CELLS: f32 = 4.0;

/// The cells-per-axis a `checker` actually uses: **even**, at least two.
///
/// Even is load-bearing rather than tidy. With an even count the cells at both
/// ends of each axis are filled, so the patch's drawn extent is its box and the
/// bounding box below is exact. With an odd count two opposite corners are empty
/// and the box would be loose in a way no picture shows — which is precisely the
/// silent cost regression Phase 7 asks be asserted away.
pub(crate) fn checker_cells(p1: f32) -> f32 {
    if !p1.is_finite() {
        return DEFAULT_CHECKER_CELLS;
    }
    let n = (p1 * 0.5).round() * 2.0;
    n.clamp(2.0, 32.0)
}

/// The half-aperture a `segment` or `arc` actually uses, radians, held inside
/// `(0, PI]` — zero is an invisible sliver and past PI the sector is the whole
/// disc twice over.
pub(crate) fn aperture(p0: f32) -> f32 {
    if p0.is_finite() {
        p0.clamp(0.02, std::f32::consts::PI)
    } else {
        DEFAULT_APERTURE
    }
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

        // **The box is a min/max pair, not a half extent, and that matters for
        // two kinds.** A triangle's apex is at `+hy` while its base sits at
        // `-hy/2`, and a sector reaches its radius only where its own span does
        // — both are *asymmetric about their centre*, so a symmetric box stands
        // off them. It shipped that way through Phase 1 (a quarter of the
        // triangle's height of empty box on one side) because the check was
        // CPU-only and compared half extents to half extents; the rendered check
        // in `tests` is what found it.
        let kind = spec.kind;
        let (lo, hi) = if !(0.5..=6.5).contains(&kind) {
            // A rotated rectangle: the support function of the four corners.
            // `checker` shares it — its cell count is forced even, so the cells
            // at both ends of each axis are filled and the patch's drawn extent
            // is its box (`checker_cells`).
            symmetric(ca.abs() * hx + sa.abs() * hy, sa.abs() * hx + ca.abs() * hy)
        } else if kind < 1.5 {
            // A rotated ellipse: the exact extent of the parametric form.
            symmetric(
                ((hx * ca) * (hx * ca) + (hy * sa) * (hy * sa)).sqrt(),
                ((hx * sa) * (hx * sa) + (hy * ca) * (hy * ca)).sqrt(),
            )
        } else if kind < 2.5 {
            // The triangle, over its three rotated vertices — built from the same
            // three points the shader's distance uses. **Asymmetric**: the apex
            // is at `+hy` and the base at `-hy/2`, so a symmetric box would stand
            // a quarter of the figure's height off its bottom edge.
            hull(
                triangle_vertices(hx, hy)
                    .iter()
                    .map(|&[vx, vy]| [ca * vx - sa * vy, sa * vx + ca * vy]),
            )
        } else if kind < 3.5 {
            // A capsule is a segment swept by a disc, so its extent is the
            // rotated segment's plus the radius on both axes — exact, and the one
            // place a Minkowski sum makes the box trivial.
            let r = hy.min(hx);
            let half = (hx - r).max(0.0);
            symmetric(ca.abs() * half + r, sa.abs() * half + r)
        } else if kind < 4.5 {
            // A ring's outer boundary is the circle of radius hx, so its box is
            // that square whatever the rotation.
            symmetric(hx, hx)
        } else {
            // `segment` and `arc`: a circular sector, so the box is taken over the
            // sector's angular span in the WORLD frame, and it is **asymmetric**
            // for the obvious reason — a sector reaches its radius only where it
            // opens. The candidates are exhaustive: a circular arc can only touch
            // an axis extreme at a cardinal angle or at one of its own ends, and
            // the figure is otherwise bounded by its straight edges.
            let a = aperture(spec.p0);
            // An `arc` is an annulus cut by the sector, so its near edge sits at
            // `hx - thickness` rather than at the apex.
            let inner = if kind < 5.5 {
                0.0
            } else {
                (hx - hy.min(hx)).max(0.0)
            };
            // **A fixed array, not a `Vec`.** `compose` calls this for every
            // live element on every frame, so a heap allocation here is one per
            // sector per frame on the render thread — see
            // `the_element_builder_allocates_nothing`. Nine is exhaustive: one
            // apex, two ends at two radii, and at most four cardinal touches.
            let mut points = [[0.0f32; 2]; 9];
            let mut n = 0usize;
            {
                let mut push = |p: [f32; 2]| {
                    // The `debug_assert` is the point of the branch, not the
                    // fallback. Dropping a candidate would shrink the hull, and
                    // a hull that is too SMALL is a bounding box the painter
                    // rejects real pixels against — a silent clip, which
                    // `every_kind_is_contained_by_its_own_bounding_box` only
                    // catches at an angle that happens to expose it. A future
                    // kind that adds a tenth candidate should fail loudly here.
                    debug_assert!(
                        n < points.len(),
                        "the hull candidate buffer is full at {n}; a new element kind needs \
                         the array widened, not its candidates dropped"
                    );
                    if let Some(slot) = points.get_mut(n) {
                        *slot = p;
                        n += 1;
                    }
                };
                // A `segment`'s apex is its own centre; an `arc` has none.
                if kind < 5.5 {
                    push([0.0, 0.0]);
                }
                for end in [-a, a] {
                    let (se, ce) = (angle + end).sin_cos();
                    for radius in [inner, hx] {
                        push([radius * ce, radius * se]);
                    }
                }
                for k in 0..4 {
                    let phi = k as f32 * std::f32::consts::FRAC_PI_2;
                    // Angular distance from the sector's world-frame axis,
                    // wrapped into [-PI, PI].
                    let tau = std::f32::consts::TAU;
                    let raw = phi - angle;
                    let delta = raw - tau * ((raw + std::f32::consts::PI) / tau).floor();
                    if delta.abs() <= a {
                        push([hx * phi.cos(), hx * phi.sin()]);
                    }
                }
            }
            hull(points.iter().take(n).copied())
        };

        Element {
            center_size: [cx, cy, hx, hy],
            shape: [ca, sa, spec.kind, aperture(spec.p0)],
            tint: [spec.coord, spec.alpha, 0.0, checker_cells(spec.p1)],
            aabb: [cx + lo[0], cy + lo[1], cx + hi[0], cy + hi[1]],
        }
    }
}

/// A box centred on the element, as the `(min, max)` offset pair every arm of
/// [`Element::build`] returns. For the kinds that are symmetric about their own
/// centre, which is most of them.
fn symmetric(ex: f32, ey: f32) -> ([f32; 2], [f32; 2]) {
    ([-ex, -ey], [ex, ey])
}

/// The `(min, max)` box of a point set — for the kinds whose figure is **not**
/// centred on the element's own centre, where a half extent would be a box with
/// empty space on one side.
fn hull(points: impl Iterator<Item = [f32; 2]>) -> ([f32; 2], [f32; 2]) {
    let mut lo = [f32::INFINITY; 2];
    let mut hi = [f32::NEG_INFINITY; 2];
    for p in points {
        for axis in 0..2 {
            if let (Some(l), Some(h), Some(v)) = (lo.get_mut(axis), hi.get_mut(axis), p.get(axis)) {
                *l = l.min(*v);
                *h = h.max(*v);
            }
        }
    }
    if lo[0].is_finite() {
        (lo, hi)
    } else {
        ([0.0; 2], [0.0; 2])
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
        p0: DEFAULT_APERTURE,
        p1: DEFAULT_CHECKER_CELLS,
    },
    // The black bar that crosses it — the occlusion this scene exists for.
    Spec {
        kind: KIND_QUAD,
        center: [0.05, -0.05],
        half: [0.72, 0.075],
        angle_deg: -22.0,
        coord: 0.0625,
        alpha: 1.0,
        p0: DEFAULT_APERTURE,
        p1: DEFAULT_CHECKER_CELLS,
    },
    Spec {
        kind: KIND_QUAD,
        center: [-0.30, 0.42],
        half: [0.40, 0.045],
        angle_deg: -22.0,
        coord: 0.3125,
        alpha: 1.0,
        p0: DEFAULT_APERTURE,
        p1: DEFAULT_CHECKER_CELLS,
    },
    Spec {
        kind: KIND_QUAD,
        center: [0.30, 0.30],
        half: [0.16, 0.160],
        angle_deg: 12.0,
        coord: 0.1875,
        alpha: 1.0,
        p0: DEFAULT_APERTURE,
        p1: DEFAULT_CHECKER_CELLS,
    },
    Spec {
        kind: KIND_CIRCLE,
        center: [-0.55, -0.32],
        half: [0.13, 0.130],
        angle_deg: 0.0,
        coord: 0.0625,
        alpha: 1.0,
        p0: DEFAULT_APERTURE,
        p1: DEFAULT_CHECKER_CELLS,
    },
    Spec {
        kind: KIND_QUAD,
        center: [0.42, -0.34],
        half: [0.26, 0.035],
        angle_deg: -22.0,
        coord: 0.5625,
        alpha: 1.0,
        p0: DEFAULT_APERTURE,
        p1: DEFAULT_CHECKER_CELLS,
    },
    Spec {
        kind: KIND_TRIANGLE,
        center: [-0.05, -0.52],
        half: [0.16, 0.180],
        angle_deg: 8.0,
        coord: 0.6875,
        alpha: 1.0,
        p0: DEFAULT_APERTURE,
        p1: DEFAULT_CHECKER_CELLS,
    },
    Spec {
        kind: KIND_QUAD,
        center: [-0.62, 0.02],
        half: [0.30, 0.028],
        angle_deg: 62.0,
        coord: 0.1875,
        alpha: 1.0,
        p0: DEFAULT_APERTURE,
        p1: DEFAULT_CHECKER_CELLS,
    },
    Spec {
        kind: KIND_QUAD,
        center: [0.62, 0.08],
        half: [0.10, 0.220],
        angle_deg: -22.0,
        coord: 0.8125,
        alpha: 1.0,
        p0: DEFAULT_APERTURE,
        p1: DEFAULT_CHECKER_CELLS,
    },
    Spec {
        kind: KIND_QUAD,
        center: [0.15, 0.52],
        half: [0.09, 0.090],
        angle_deg: 40.0,
        coord: 0.0625,
        alpha: 1.0,
        p0: DEFAULT_APERTURE,
        p1: DEFAULT_CHECKER_CELLS,
    },
    Spec {
        kind: KIND_CIRCLE,
        center: [0.58, -0.62],
        half: [0.075, 0.075],
        angle_deg: 0.0,
        coord: 0.1875,
        alpha: 1.0,
        p0: DEFAULT_APERTURE,
        p1: DEFAULT_CHECKER_CELLS,
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
        p0: DEFAULT_APERTURE,
        p1: DEFAULT_CHECKER_CELLS,
    },
    Spec {
        kind: KIND_QUAD,
        center: [0.00, -0.72],
        half: [0.34, 0.022],
        angle_deg: -22.0,
        coord: 0.4375,
        alpha: 1.0,
        p0: DEFAULT_APERTURE,
        p1: DEFAULT_CHECKER_CELLS,
    },
    Spec {
        kind: KIND_TRIANGLE,
        center: [0.80, 0.42],
        half: [0.10, 0.120],
        angle_deg: 190.0,
        coord: 0.0625,
        alpha: 1.0,
        p0: DEFAULT_APERTURE,
        p1: DEFAULT_CHECKER_CELLS,
    },
];

/// How many elements the authored canvas holds — the default `count`, and what
/// [`Grammar::Authored`](layout::Grammar::Authored) cycles.
pub(crate) const AUTHORED_COUNT: usize = SUPREMATIST.len();

/// `roster` default — the suprematist three, so a preset that says nothing
/// draws the canvas Phase 5 settled on.
const DEFAULT_ROSTER: f32 = 0.0;
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

/// `density` default — every generated element is live.
const DEFAULT_DENSITY: f32 = 1.0;
/// `drift`, `spin`, `recompose`, `recompose_blend`, `pump_size`, `pump_alpha`
/// defaults. **Every one of them is the identity**, so a preset that binds none
/// of Phase 6's levers draws exactly the still canvas Phase 5 settled on — which
/// is what lets the golden baseline survive this phase unchanged.
const DEFAULT_DRIFT: f32 = 0.0;
const DEFAULT_SPIN: f32 = 0.0;
const DEFAULT_RECOMPOSE: f32 = 0.0;
const DEFAULT_RECOMPOSE_BLEND: f32 = 0.0;
const DEFAULT_PUMP: f32 = 0.0;

/// `recompose` rises past this to recompose once — **edge-triggered**, the
/// engine's convention and its reason (`swarm`, `particles`): a sustained beat
/// flag must not re-run the generator every frame.
const RECOMPOSE_THRESHOLD: f32 = 0.5;
/// The longest crossfade `recompose_blend` may name, in seconds. Past this a
/// recomposition stops reading as an event.
const MAX_BLEND_SECS: f32 = 10.0;
/// How long an element takes to fade in or out when `density` moves it across
/// the gate. Short enough to read as an arrival, long enough not to pop.
const FADE_SECS: f32 = 0.45;
/// The internal pump oscillator's rate, in Hz.
///
/// A constant rather than a parameter, deliberately: `pump_size` and
/// `pump_alpha` are **depths**, and an author drives them from the music. What
/// this sets is only how fast the per-element phases sweep past each other, and
/// a second rate knob would be one more thing to keep in step with the beat for
/// no visual gain the depth does not already give.
const PUMP_RATE: f32 = 0.55;

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
    /// The element array the GPU reads, rebuilt every frame from [`Self::live`]
    /// (and [`Self::outgoing`] during a blend) with this frame's time applied.
    ///
    /// **Capacity is twice the tier cap**, because a recomposition crossfade has
    /// two whole canvases on screen at once — see [`Self::blend`].
    elements: Vec<Element>,
    /// The live canvas, as generated: geometry plus per-element motion rates.
    live: Vec<layout::Placed>,
    /// The canvas being crossfaded *out* of, during a recomposition.
    outgoing: Vec<layout::Placed>,
    /// Crossfade progress, `0..=1`. `1.0` means no blend is in flight and
    /// [`Self::outgoing`] is not drawn.
    blend: f32,
    /// Seconds the blend in flight runs over, captured at the edge so a preset
    /// changing `recompose_blend` mid-blend does not change its own duration.
    blend_secs: f32,
    /// Real seconds since the scene was built, accumulated from the **injected**
    /// `dt` (`Scene::advance`). Every motion below is a function of this, never
    /// of a per-frame constant, which is what makes the canvas move identically
    /// at any refresh rate (ADR-0012).
    elapsed: f32,
    /// [`Self::elapsed`] when the live canvas was composed, so a recomposition
    /// starts its drift from zero rather than teleporting.
    born: f32,
    /// The same for [`Self::outgoing`].
    outgoing_born: f32,
    /// How many recompositions have fired — the generator's recomposition index.
    recompose_count: u64,
    /// Previous frame's `recompose` level, for rising-edge detection.
    prev_recompose: f32,
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
    /// Which kinds the canvas draws from, raw as the preset bound it.
    roster: f32,
    /// What fraction of the generated list is live, raw as the preset bound it.
    density: f32,
    /// Per-element drift and spin multipliers, raw as the preset bound them.
    drift: f32,
    spin: f32,
    /// This frame's `recompose` level, and how long its crossfade should run.
    recompose: f32,
    recompose_blend: f32,
    /// Per-element pump depths, raw as the preset bound them.
    pump_size: f32,
    pump_alpha: f32,
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
        // The roster's `Element` struct and its distance functions are spliced
        // in ahead of the painter's own body, exactly as the two particle scenes
        // splice in `marks`: the chunk declares no bindings and no entry points,
        // so the pipeline's layout is unchanged by it.
        let source = format!("{}{SHADER}", sdf::wgsl());
        let shader = gpu::fullscreen_shader(
            device,
            "shape-collage-shader",
            gpu::FULLSCREEN_VS_NDC,
            &source,
        );
        let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("shape-collage-params"),
            size: std::mem::size_of::<Params>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let storage = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("shape-collage-elements"),
            // Twice the cap: a recomposition crossfade draws two whole canvases.
            size: (2 * cap * std::mem::size_of::<Element>()) as u64,
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
            elements: Vec::with_capacity(2 * cap),
            live: Vec::with_capacity(cap),
            outgoing: Vec::with_capacity(cap),
            blend: 1.0,
            blend_secs: 0.0,
            elapsed: 0.0,
            born: 0.0,
            outgoing_born: 0.0,
            recompose_count: 0,
            prev_recompose: 0.0,
            built: None,
            dirty: false,
            #[cfg(test)]
            specs_override: false,
            count: AUTHORED_COUNT as f32,
            layout: DEFAULT_LAYOUT,
            seed: DEFAULT_SEED,
            size_hierarchy: DEFAULT_SIZE_HIERARCHY,
            angle_bias: DEFAULT_ANGLE_BIAS,
            roster: DEFAULT_ROSTER,
            density: DEFAULT_DENSITY,
            drift: DEFAULT_DRIFT,
            spin: DEFAULT_SPIN,
            recompose: DEFAULT_RECOMPOSE,
            recompose_blend: DEFAULT_RECOMPOSE_BLEND,
            pump_size: DEFAULT_PUMP,
            pump_alpha: DEFAULT_PUMP,
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
            count: applied_count(self.count, self.live.capacity()),
            seed: applied_seed(self.seed),
            recompose: self.recompose_count,
            size_hierarchy: applied_size_hierarchy(self.size_hierarchy),
            angle_bias: applied_angle_bias(self.angle_bias),
            roster: layout::Roster::from_param(self.roster),
        }
    }

    /// Regenerate the live canvas if the recipe moved, unless a test has
    /// installed its own. A no-op when nothing changed, so the common frame does
    /// not re-run the generator.
    fn rebuild(&mut self) {
        #[cfg(test)]
        if self.specs_override {
            return;
        }
        let recipe = self.recipe();
        if self.built == Some(recipe) {
            return;
        }
        layout::generate(&mut self.live, &recipe);
        self.snap_fades();
        self.born = self.elapsed;
        self.built = Some(recipe);
    }

    /// Set every live element's fade straight to its density target.
    ///
    /// A **fresh canvas does not fade itself in**: `density` is the spawn/decay
    /// lever and it animates when it moves, but a canvas arriving is covered by
    /// the recomposition blend instead. Without this a two-frame capture would
    /// read every element at partial alpha.
    fn snap_fades(&mut self) {
        let live_count = self.live_count(self.live.len());
        for (i, p) in self.live.iter_mut().enumerate() {
            p.fade = if i < live_count { 1.0 } else { 0.0 };
        }
    }

    /// How many of `total` elements the `density` gate admits.
    ///
    /// **A prefix, and that is the whole of the stability guarantee.** Birth
    /// order is the array's own order, so raising `density` only ever *extends*
    /// the live set — an element that is already live keeps its index, its
    /// colour and its place in the painter's order, and nothing reorders or
    /// pops. Any scheme that picked a live *subset* per frame would fail that.
    fn live_count(&self, total: usize) -> usize {
        let d = if self.density.is_finite() {
            self.density.clamp(0.0, 1.0)
        } else {
            DEFAULT_DENSITY
        };
        // `ceil`, so any density above zero keeps at least one element: a canvas
        // that vanishes entirely is indistinguishable from a broken one.
        ((total as f32 * d).ceil() as usize).min(total)
    }

    /// Advance the recomposition edge, the crossfade and the per-element fades
    /// by `dt` real seconds, then rebuild the GPU array for this instant.
    ///
    /// Split out of [`Scene::render`] so it can be driven — and inspected —
    /// without a GPU, which is what the frame-rate-independence assertion needs.
    fn step(&mut self, dt: f32) {
        let dt = if dt.is_finite() && dt > 0.0 { dt } else { 0.0 };
        self.elapsed += dt;

        // **The recomposition edge.** Rising past the threshold recomposes once;
        // a held gate does not fire again, which is why the previous level
        // survives `reset_params`.
        let rising =
            self.recompose >= RECOMPOSE_THRESHOLD && self.prev_recompose < RECOMPOSE_THRESHOLD;
        self.prev_recompose = self.recompose;
        #[cfg(test)]
        let rising = rising && !self.specs_override;
        if rising {
            self.recompose_count = self.recompose_count.wrapping_add(1);
            std::mem::swap(&mut self.live, &mut self.outgoing);
            self.outgoing_born = self.born;
            let recipe = self.recipe();
            layout::generate(&mut self.live, &recipe);
            self.snap_fades();
            self.born = self.elapsed;
            self.built = Some(recipe);
            self.blend_secs = applied_blend_secs(self.recompose_blend);
            // At zero seconds this is already finished, which is the hard cut.
            self.blend = if self.blend_secs > 0.0 { 0.0 } else { 1.0 };
        } else if self.blend < 1.0 {
            self.blend = if self.blend_secs > 0.0 {
                (self.blend + dt / self.blend_secs).min(1.0)
            } else {
                1.0
            };
        }

        // The density gate, eased so an element arrives and leaves rather than
        // popping. Frame-rate independent: the step is `dt / FADE_SECS`.
        let live_count = self.live_count(self.live.len());
        let step = if FADE_SECS > 0.0 { dt / FADE_SECS } else { 1.0 };
        for (i, p) in self.live.iter_mut().enumerate() {
            let target = if i < live_count { 1.0 } else { 0.0 };
            if p.fade < target {
                p.fade = (p.fade + step).min(target);
            } else if p.fade > target {
                p.fade = (p.fade - step).max(target);
            }
        }

        self.compose();
    }

    /// Rebuild the GPU element array for this instant, from the live canvas and
    /// — while a recomposition crossfades — the outgoing one under it.
    fn compose(&mut self) {
        #[cfg(test)]
        if self.specs_override {
            return;
        }
        let drift = finite_or(self.drift, DEFAULT_DRIFT);
        let spin = finite_or(self.spin, DEFAULT_SPIN);
        let pump_size = finite_or(self.pump_size, DEFAULT_PUMP);
        let pump_alpha = finite_or(self.pump_alpha, DEFAULT_PUMP);
        let blend = self.blend.clamp(0.0, 1.0);

        self.elements.clear();
        // **Equal-power weights, not linear ones**, and this is the difference
        // between a dissolve and a wash. The two canvases composite
        // *sequentially* with `over` rather than being mixed, so at linear
        // weights a pixel covered by both reads
        // `t(1-t)*paper + (1-t)^2*A + t*B` — a quarter of it is bare paper at
        // the midpoint, and a preset recomposing often sits there permanently.
        // The frames go pale, which is what a filmstrip under a click track
        // showed at Plan 0113 Phase 6. `sqrt` weights are the same fix audio
        // uses for an equal-power pan: they take the paper leak at the midpoint
        // from 25 % to under 9 %, and they still reach 0 and 1 at the ends.
        let out_alpha = (1.0 - blend).sqrt();
        let in_alpha = blend.sqrt();

        // The outgoing canvas goes in FIRST, so the incoming one paints over it —
        // array order is depth, and a recomposition should arrive on top of what
        // it replaces rather than under it.
        if blend < 1.0 {
            let age = self.elapsed - self.outgoing_born;
            for p in &self.outgoing {
                self.elements.push(apply_time(
                    p, age, drift, spin, pump_size, pump_alpha, out_alpha,
                ));
            }
        }
        let age = self.elapsed - self.born;
        for p in &self.live {
            self.elements.push(apply_time(
                p, age, drift, spin, pump_size, pump_alpha, in_alpha,
            ));
        }
        self.dirty = true;
    }

    /// The element array this scene would upload right now — the composed
    /// canvas at this instant, after drift, spin, fades and the blend.
    #[cfg(test)]
    pub(crate) fn composed(&self) -> &[Element] {
        &self.elements
    }

    /// How many recompositions have fired, for the edge-trigger assertion.
    #[cfg(test)]
    pub(crate) fn recompositions(&self) -> u64 {
        self.recompose_count
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

/// A finite value, or the default for a broken binding. The one-line form of
/// the fallback every conditioner here performs.
fn finite_or(value: f32, default: f32) -> f32 {
    if value.is_finite() { value } else { default }
}

/// The crossfade duration a bound `recompose_blend` names, in seconds. Exactly
/// zero — the default — is the hard cut.
fn applied_blend_secs(blend: f32) -> f32 {
    if blend.is_finite() {
        blend.clamp(0.0, MAX_BLEND_SECS)
    } else {
        DEFAULT_RECOMPOSE_BLEND
    }
}

/// **One element at one instant**: the generated element with `age` seconds of
/// drift, spin and pumping applied, and its alpha scaled by the crossfade.
///
/// `age` is real seconds since this element's canvas was composed, accumulated
/// from the **injected** `dt` — so the position is a pure function of elapsed
/// time and not of how many frames it took to get there (ADR-0012). That is the
/// property the frame-rate test asserts.
fn apply_time(
    p: &layout::Placed,
    age: f32,
    drift: f32,
    spin: f32,
    pump_size: f32,
    pump_alpha: f32,
    canvas_alpha: f32,
) -> Element {
    // One oscillator per element, **phase-offset at generation**, so the canvas
    // does not breathe in unison.
    let pump = (std::f32::consts::TAU * (age * PUMP_RATE + p.phase)).sin();
    // Never to zero or below: an element scaled through zero inverts, and the
    // distance functions would draw it inside out on the way.
    let size = (1.0 + pump_size * pump).max(0.05);
    let alpha = p.spec.alpha * p.fade * canvas_alpha * (1.0 + pump_alpha * pump).clamp(0.0, 1.0);
    // Drift **wraps** into the canvas rather than travelling off it: over a long
    // set a linear drift empties the canvas entirely, and a wrap at the edge is
    // the cheaper artefact. It is also what keeps the position a pure function
    // of `age`, which a bounce would not be.
    let wrap = |v: f32, half: f32| (v + half).rem_euclid(2.0 * half) - half;
    Element::build(Spec {
        center: [
            wrap(p.spec.center[0] + p.vel[0] * drift * age, layout::CANVAS_X),
            wrap(p.spec.center[1] + p.vel[1] * drift * age, layout::CANVAS_Y),
        ],
        half: [p.spec.half[0] * size, p.spec.half[1] * size],
        angle_deg: p.spec.angle_deg + (p.spin * spin * age).to_degrees(),
        alpha,
        ..p.spec
    })
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
    "roster",
    "density",
    "drift",
    "spin",
    "recompose",
    "recompose_blend",
    "pump_size",
    "pump_alpha",
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
        self.roster = DEFAULT_ROSTER;
        self.density = DEFAULT_DENSITY;
        self.drift = DEFAULT_DRIFT;
        self.spin = DEFAULT_SPIN;
        self.recompose_blend = DEFAULT_RECOMPOSE_BLEND;
        self.pump_size = DEFAULT_PUMP;
        self.pump_alpha = DEFAULT_PUMP;
        // `prev_recompose` is deliberately NOT reset — this runs every frame
        // before the bindings are routed, and resetting the previous level would
        // turn a held gate into an edge per frame, recomposing continuously
        // instead of on the beat. `swarm::reset_params` makes the same omission
        // for the same reason.
        self.recompose = DEFAULT_RECOMPOSE;
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
            "roster" => self.roster = value,
            "density" => self.density = value,
            "drift" => self.drift = value,
            "spin" => self.spin = value,
            "recompose" => self.recompose = value,
            "recompose_blend" => self.recompose_blend = value,
            "pump_size" => self.pump_size = value,
            "pump_alpha" => self.pump_alpha = value,
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

    /// Advance the canvas by `dt` real seconds (ADR-0012).
    ///
    /// **The whole of this scene's animation hangs off this argument** — the
    /// recomposition edge, the crossfade, the density fades, and every element's
    /// drift, spin and pump. Nothing here reads a clock or assumes a frame rate,
    /// so a second of music moves the canvas the same distance at 30 Hz and at
    /// 144 Hz.
    fn advance(&mut self, dt: f32) {
        self.rebuild();
        self.step(dt);
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

        // `advance` has already stepped the canvas for this frame; a renderer
        // that never calls it (there is none) still gets a composed canvas.
        if self.built.is_none() {
            self.rebuild();
            self.compose();
        }
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
pub(crate) mod sdf;

#[cfg(test)]
mod tests;
