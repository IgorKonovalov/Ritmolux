//! **The mark roster, drawn at frame scale as a distance field**
//! (ADR-0105, Plan 0091).
//!
//! Every other scene here hands the palette a *level* — a noise field, a
//! chemical concentration, a particle's depth. This one hands it a **distance**,
//! and that single substitution is the whole scene:
//!
//! ```text
//! palette coordinate = mark_distance(p) * color_span + color_center
//! ```
//!
//! (that is `coord_mode`'s default; the second coordinate is below.)
//!
//! Because a band of the palette coordinate is now a band of constant distance,
//! turning `palette_steps` up produces **concentric offset contours of the
//! chosen shape** — not concentric circles, and not an outline sampled to
//! straight segments. It is per pixel, resolution-independent, and there is no
//! geometry to facet (which is the entire argument of ADR-0105 against routing
//! this through the line renderer). `palette_contour` then draws thin outlines
//! at those band boundaries, and this is the third scene that param does
//! anything in.
//!
//! # Two coordinates now, and the second is the one the references asked for
//! (ADR-0111, Plan 0098)
//!
//! An offset family is an **erosion**, and erosion rounds a reflex corner while
//! keeping convex ones sharp — so a nested heart keeps its bottom point and
//! loses its top notch as the contours move inward, and no amount of tuning
//! reaches the construction two batches of user reference images have asked for.
//! `coord_mode = "1"` hands the palette
//!
//! ```text
//! s = length(p) / r_boundary(theta)
//! ```
//!
//! instead — `0` at the centre and exactly `1` on the outline, the same contract,
//! but its level sets are **scaled copies** of the outline. The ring count is
//! then `palette_steps` alone and the innermost figure is a scaled copy at any
//! count, so notch sharpness stops trading against ring count.
//!
//! **The distance stays the default and stays bit-identical**, which is what
//! keeps every shipped preset and every golden baseline on the arithmetic it has
//! today. The two are not interchangeable settings of one knob: `color_span`
//! means a different thing under each, because the exterior is divided by the
//! shape's inradius under one and grows linearly in `r` under the other.
//!
//! # It shares the shape vocabulary rather than restating it
//!
//! The silhouettes come from `marks` — the same WGSL chunk
//! `swarm` and `emitter` splice in, and the same CPU-side quantizers for the
//! `shape` selector and the `points` count. So a mark a particle can wear and a
//! figure this scene can be cannot drift apart, and the roster stays closed at
//! five names (ADR-0084's consequence, restated in ADR-0105).
//!
//! # A silhouette can also be authored (ADR-0107)
//!
//! A `[path]` table hands this scene a closed contour parsed from inline SVG
//! path data, and it takes the place of the roster's arm in exactly one spot:
//! where the shader asks for the figure's coordinate. Everything downstream —
//! both `coord_mode`s, `gamma`, the banding, the contours, the palette — is the
//! same code reading the same `d`, so an authored figure gets the whole colour
//! surface for free. The roster is untouched and still closed at five names; a
//! preset naming no path executes not one instruction of the contour walk.
//!
//! Two things fall out of the distance being a **`min` over segments**. It is
//! `O(N)` per pixel and the field is fullscreen, so the arity is a per-frame
//! cost paid whether or not the figure is on screen — `MAX_SAMPLES` is where
//! `core/tests/path_cost.rs` measured that becoming half the floor tier's frame
//! budget, and asking for more is a load error. And **fill and stroke stop
//! being two routes**: the interior is `d < 1` and the outline is
//! `abs(d - 1) < w` on the one evaluation, which is what the `stroke` param is.
//!
//! What *is* new is that this scene reads the field **outside** the silhouette,
//! where the particle path never looked. Plan 0091 Phase 2 measured that region
//! and repaired the two arms that were wrong out there; see `marks.rs`'s own
//! header for what it found and what it deliberately left approximate.
//!
//! # The aspect comes from the render target (ADR-0037)
//!
//! There is no internal grid here to take an aspect from by accident, which
//! removes the usual mechanism for that bug but not the obligation. The figure
//! is drawn in a square-unit space built by stretching NDC x by the **render
//! target's** aspect, so a disc is round at every window shape. `tests` renders
//! at 2:1 and 1:2 and measures the figure's own width against its height,
//! because both 1920x1080 and this box's 2048x1152 quantize to exactly 16:9 —
//! where no test can tell a right aspect source from a wrong one.

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
use super::common;
use super::marks;
use crate::dsp::AnalysisFrame;
use crate::preset::path::{MAX_ARC_PIECES, MAX_SAMPLES};
use crate::render::palette::{self, Palette};
use crate::render::scenes::{ParamKind, ParamSpec, default_of};

/// How many `vec4` one arc piece occupies: its circle, its sector test, its two
/// endpoints, and its signed sweep. See the WGSL's `arc_chain_sd` for what each
/// field is for.
const VEC4S_PER_PIECE: usize = 4;

/// How many `vec4` the uniform's geometry array holds — enough for
/// [`MAX_ARC_PIECES`] arc pieces, which is more than the polyline's two points
/// per element ever needs.
///
/// The geometry rides the **uniform** buffer rather than a storage one, and that
/// is an ADR-0058 choice rather than a performance one: a fragment-visible
/// read-only storage entry after this layout's uniform would make its shape
/// byte-identical to `shape-collage-bind-layout`, which is live in the same
/// frame during a preset dissolve. Two layouts of one shape alias on the DX12
/// WARP adapter the whole golden suite captures on, so the collision would be
/// blessed rather than caught. Packing into the uniform leaves the layout's
/// four entries exactly as they were.
const PATH_VEC4S: usize = VEC4S_PER_PIECE * MAX_ARC_PIECES;

/// The WGSL below spells the array length as a literal — `format!` cannot reach
/// into a raw string full of braces — so the two are held together here. Raise
/// either bound and this fails the build rather than letting the shader read
/// past what the uniform carries.
const _: () = assert!(
    PATH_VEC4S == 128 && PATH_VEC4S >= MAX_SAMPLES / 2,
    "the WGSL `path` array must be PATH_VEC4S long, and hold either geometry"
);

/// `scale` default — the figure's outline sits at 0.6 of the frame's short
/// half-axis, which leaves room for several contour bands around it before they
/// leave the frame. The whole point of this scene is what happens *outside* the
/// silhouette, so a figure filling the frame would be the wrong default.
const DEFAULT_SCALE: f32 = default_of(PARAMS, "scale");
/// Smallest `scale` the shader is handed. Not zero: at zero the figure has no
/// size and every pixel is infinitely far outside it in units of nothing, so
/// the coordinate degenerates rather than fading out.
const MIN_SCALE: f32 = 0.01;
/// Largest `scale`. Past this the figure is far outside the frame and the whole
/// screen is one interior band — reachable, but it is the end of the useful
/// range rather than an arbitrary cap.
const MAX_SCALE: f32 = 20.0;

/// `rotation` default — **0, and an exact arithmetic identity**: the shader
/// tests for it and skips the rotation entirely, so every shipped preset and
/// every golden baseline stays on the arithmetic it has today.
///
/// Radians, matching `lines/star.rs` and `lines/lsystem.rs` — the two other
/// figure-drawing scenes that carry this name. Unclamped for the same reason
/// they are: an angle wraps, so there is no end of the useful range to hold it
/// inside; a non-finite binding falls back to the identity because `cos(NaN)`
/// would take the whole frame with it.
const DEFAULT_ROTATION: f32 = default_of(PARAMS, "rotation");

/// `stroke` default — **0, the filled figure, and an exact arithmetic
/// identity**: the shader tests for it and skips the stroke mask entirely.
const DEFAULT_STROKE: f32 = default_of(PARAMS, "stroke");
/// Largest `stroke`. One coordinate unit is the figure's whole interior — 0 at
/// its deepest point, 1 on the outline — so a half-width of 1 is a band
/// reaching from the centre to twice the outline, and past that the stroke has
/// stopped being an outline of anything.
const MAX_STROKE: f32 = 1.0;

/// `morph` default — **0, the authored figure**, and an exact identity: at 0 the
/// packed contour is `[path] d` verbatim, with no interpolation run at all. A
/// preset declaring no `morph_to` has nothing to travel towards and this is
/// inert whatever it is bound to, exactly as the attractor's `morph` is.
const DEFAULT_MORPH: f32 = default_of(PARAMS, "morph");

/// `gamma` default — **the identity**, and it is exactly `1.0` on the way to the
/// uniform because the shader's identity branch tests for it (`pow(x, 1.0)` is
/// not bit-exact, ADR-0092's care).
const DEFAULT_GAMMA: f32 = default_of(PARAMS, "gamma");
/// The range `gamma` is held in. Same shape and the same reasoning as
/// `ink_gamma` and `bg_ramp_gamma`: positive on both sides, wide enough that the
/// clamp is the end of the useful range rather than a limit an author meets.
const MIN_GAMMA: f32 = 0.05;
const MAX_GAMMA: f32 = 20.0;

/// The `coord_mode` roster, in the order the numeric parameter selects them.
///
/// `0` hands the palette the normalized **distance** to the figure, whose level
/// sets are offset curves; `1` hands it `r / r_boundary(theta)`, whose level
/// sets are **scaled copies** of the outline
/// (ADR-0111). Both are `0` at the figure's centre and exactly `1` on its outline; what
/// differs is the shape of everything in between.
pub(crate) const COORD_MODES: [&str; 2] = ["distance", "radius"];

/// `coord_mode` default — **0, the distance**, and that is an obligation rather
/// than a preference: it is the arithmetic every shipped preset and every golden
/// baseline has today.
const DEFAULT_COORD_MODE: f32 = default_of(PARAMS, "coord_mode");
const MIN_COORD_MODE: f32 = 0.0;
const MAX_COORD_MODE: f32 = COORD_MODES.len() as f32 - 1.0;

/// Shared palette colour knobs (ADR-0021). `color_span = 0.6` puts the
/// silhouette's interior (`d` in `0..1`) across the gradient's first 60 %, so
/// the exterior contours have somewhere to go.
const DEFAULT_COLOR_SPAN: f32 = default_of(PARAMS, "color_span");
const DEFAULT_COLOR_CENTER: f32 = default_of(PARAMS, "color_center");

/// How many of the palette's [`LUT_SIZE`] texels a resting `color_span` spends on
/// the **figure's own interior**.
///
/// Both coordinates are `0` at the figure's centre and exactly `1` on its
/// outline, so the interior is one unit of the coordinate whatever the shape and
/// whatever `gamma` does to the spacing inside it — which makes this share
/// exact. What it does *not* know is how much of the frame that interior covers:
/// a figure spanning half the screen stretches those texels across hundreds of
/// pixels, and one filling a corner does not. That is why the warning built on
/// it says estimate.
pub(crate) fn interior_texels(color_span: f32) -> f32 {
    color_span.abs() * crate::render::palette::LUT_SIZE as f32
}

/// Below this many texels across the interior, the linear-filtered LUT is
/// stretching too few distinct colours across the figure and the result reads as
/// an upscaled gradient rather than as shading (design-backlog 0099).
///
/// **The property is the count, not this constant.** A figure's interior drawn
/// through N texels carries at most N colours no matter how large it is on
/// screen, so somewhere below a few dozen the sampler is interpolating more than
/// it is reading. The Plan 0091 Phase 6 star probes bracket where that becomes
/// visible — 8.6 texels read as *"dirty and upscaled"*, 32.3 did not — and this
/// is the middle of that bracket rounded to a power of two. It is a warning
/// threshold, so a value inside it is legal and renders exactly as asked;
/// `palette_steps` is the remedy, because a quantized coordinate samples one
/// texel per band and interpolates nothing.
pub(crate) const MIN_INTERIOR_TEXELS: f32 = 16.0;

const SHADER: &str = r#"
struct Params {
    // x: aspect (from the RENDER TARGET), y: shape index (quantized CPU-side),
    // z: points (quantized CPU-side), w: scale
    a: vec4<f32>,
    // xy: pan (the shared ViewTransform, ADR-0018), z: color_span,
    // w: color_center
    b: vec4<f32>,
    // x: saturation, y: palette_mix, z: palette_steps (integral, quantized
    // CPU-side), w: palette_contour
    c: vec4<f32>,
    // x: occlude (ADR-0085), y: gamma (the response exponent on the distance,
    // exactly 1.0 for the identity), z: coord_mode (quantized CPU-side; 0 = the
    // distance, 1 = the scaled-copy radius), w: rotation in radians, exactly 0.0
    // for the identity.
    d: vec4<f32>,
    // xyz: the star arm's shape params (valley, curve, jitter), conditioned
    // CPU-side. Inert on every other silhouette.
    e: vec4<f32>,
    // x: path point count (0 = no authored contour, and every line of the path
    // arms below is unreached), y: the contour's inradius — the divisor that
    // makes the distance 0 at its deepest interior point, measured CPU-side,
    // z: stroke half-width in coordinate units (exactly 0.0 = filled),
    // w: arc piece count — nonzero means `path` holds an ARC CHAIN rather than
    // a polyline, and `x` is then unread.
    f: vec4<f32>,
    // The authored contour (ADR-0107), in one of two packings.
    //
    // **As a polyline** (`f.w == 0`): TWO POINTS PER ELEMENT, point `i` at
    // `path[i >> 1].xy` for even `i` and `.zw` for odd. Packed because a uniform
    // array's elements are 16-byte aligned, so an `array<vec2<f32>, N>` would
    // spend half the buffer on padding.
    //
    // **As an arc chain** (`f.w > 0`): FOUR ELEMENTS PER PIECE, piece `i` at
    // `path[i * 4 ..]`:
    //   +0  (kind, cx, cy, radius)    kind 0 = straight run, 1 = arc
    //   +1  (mx, my, cos_half, 0)     the sector's mid direction and half-angle
    //   +2  (ax, ay, bx, by)          the piece's two endpoints
    //   +3  (start, sweep, 0, 0)      the signed sweep, for the crossing test
    path: array<vec4<f32>, 128>,
}

// **One bind group, sampler first and uniform last — and that arrangement is
// what buys this pipeline a layout shape nothing else holds** (ADR-0058: two
// byte-identical layouts alias on the DX12 WARP adapter, and the whole golden
// suite runs there, so a collision is blessed rather than caught).
//
// It is deliberately not `fragment_field`'s two-group split, because that split
// has no free shape left for a tenth scene. A lone uniform group can vary only
// by visibility and by whether it declares a `min_binding_size`, and all four
// combinations are taken: `[Uniform:FRAGMENT]` by the fragment field, the RD
// init and the test disc; `+size` by the backdrop; `VERTEX_FRAGMENT` by the
// line renderer; and `VERTEX_FRAGMENT+size` by the emitter. Merging the groups
// is what keeps this unique WITHOUT padding a layout with a binding the shader
// does not use, which is the cure ADR-0058's Alternative A refuses.
//
// Pick another free shape rather than tidying this back into two groups.
@group(0) @binding(0) var lut_samp: sampler;
@group(0) @binding(1) var lut_a: texture_2d<f32>;
@group(0) @binding(2) var lut_b: texture_2d<f32>;
@group(0) @binding(3) var<uniform> params: Params;

// Shared `saturation` (mirrors core/src/render/palette.rs::desaturate verbatim).
fn apply_saturation(c: vec3<f32>, s: f32) -> vec3<f32> {
    let luma = dot(c, vec3<f32>(0.299, 0.587, 0.114));
    return vec3<f32>(luma) + (c - vec3<f32>(luma)) * s;
}

// Shared `palette_steps` (mirrors core/src/render/palette.rs::band_coord
// verbatim, ADR-0078): snap the palette coordinate to a band centre before the
// LUT read. Below 1.5 steps it is the exact identity, not a one-band degenerate.
fn band_coord(t: f32, steps: f32) -> f32 {
    if (steps < 1.5) {
        return t;
    }
    return (floor(t * steps) + 0.5) / steps;
}

// Shared `palette_contour` (ADR-0078 / ADR-0133; the WGSL is the implementation,
// copied verbatim at each fragment-stage site — palette.rs has no CPU
// counterpart to be canonical, since `fwidth` exists only here).
//
// Darkens within one PIXEL of a band edge, so the line has the same weight where
// the field is shallow and where it is steep — AND ONLY WHERE THE INK ACTUALLY
// CHANGES (ADR-0133). It samples the two band centres either side of the nearest
// edge and returns unchanged when they resolve to the same colour within half a
// code value, which is below the LUT's own 8-bit quantization. On a smooth
// palette two distinct centres always differ by at least one code value, so
// every edge draws exactly as it did at any `palette_steps`; inside a plateau
// the LUT is literally constant and the samples are bit-equal, so the line
// vanishes there and survives at the run boundaries. One rule, both behaviours,
// no new parameter.
//
// The two LUTs, the sampler and `palette_mix` are EXPLICIT parameters rather
// than module-scope globals this happens to find: all four sites name them the
// same today, so implicit capture would compile — and would silently bind the
// shared function to whatever a future site called its textures.
//
// `textureSampleLevel`, not `textureSample`: the LUT has one mip, and an
// explicit LOD keeps these reads free of the uniformity requirement that a
// sample after a conditional return would otherwise carry.
fn band_contour(
    t: f32,
    steps: f32,
    amount: f32,
    lut_a: texture_2d<f32>,
    lut_b: texture_2d<f32>,
    lut_samp: sampler,
    mix_ab: f32,
) -> f32 {
    let f = t * steps;
    let w = max(fwidth(f), 1e-5);
    if (steps < 1.5 || amount <= 0.0) {
        return 1.0;
    }
    let n = round(f);
    let m = clamp(mix_ab, 0.0, 1.0);
    let lo = mix(
        textureSampleLevel(lut_a, lut_samp, vec2<f32>((n - 0.5) / steps, 0.5), 0.0).rgb,
        textureSampleLevel(lut_b, lut_samp, vec2<f32>((n - 0.5) / steps, 0.5), 0.0).rgb,
        m
    );
    let hi = mix(
        textureSampleLevel(lut_a, lut_samp, vec2<f32>((n + 0.5) / steps, 0.5), 0.0).rgb,
        textureSampleLevel(lut_b, lut_samp, vec2<f32>((n + 0.5) / steps, 0.5), 0.0).rgb,
        m
    );
    if (all(abs(hi - lo) < vec3<f32>(0.5 / 255.0))) {
        return 1.0;
    }
    let d = min(fract(f), 1.0 - fract(f));
    return 1.0 - clamp(amount, 0.0, 1.0) * (1.0 - smoothstep(0.0, w, d));
}

// Point `i` of the authored contour, unpacked from the two-per-vec4 array.
fn path_pt(i: u32) -> vec2<f32> {
    let v = params.path[i >> 1u];
    if ((i & 1u) == 0u) {
        return v.xy;
    }
    return v.zw;
}

// **The authored contour's signed distance**: `min` over the distance to each
// closing segment, signed by a crossing count (ADR-0107).
//
// The sign is a RAY-CROSSING PARITY rather than an orientation test, so it does
// not care which way the author wound their path — which is what lets Phase 1
// keep the contour's own winding and leave the alignment to the morph.
//
// `min` over segment distances has no quads, no overlap and no vertex bead: it
// is exactly correct at every join, which is why ADR-0098's faceting objection
// against a polyline stroke does not transfer to a polyline FILL. The cost is
// `O(n)` per pixel and it is paid at every pixel of the frame whether or not the
// figure is on screen, which is what the arity ceiling exists to bound.
fn path_sd(p: vec2<f32>, n: u32) -> f32 {
    var best = 1e20;
    var s = 1.0;
    // The previous point is carried rather than re-indexed, so each iteration
    // makes one dynamically indexed uniform read instead of two. It measured as
    // free — `path_cost.rs` reports the same ms/frame either way, so the loop's
    // cost is its arithmetic and not its loads — and it stays because it is the
    // simpler loop, not because it bought anything.
    var b = path_pt(n - 1u);
    for (var i = 0u; i < n; i = i + 1u) {
        let a = path_pt(i);
        let e = b - a;
        let w = p - a;
        // The nearest point ON THE SEGMENT, not on its infinite line: the clamp
        // is what makes a sample beyond an end measure to the vertex.
        let t = clamp(dot(w, e) / max(dot(e, e), 1e-20), 0.0, 1.0);
        let q = w - e * t;
        best = min(best, dot(q, q));
        let c1 = p.y >= a.y;
        let c2 = p.y < b.y;
        let c3 = e.x * w.y > e.y * w.x;
        if ((c1 && c2 && c3) || (!c1 && !c2 && !c3)) {
            s = -s;
        }
        b = a;
    }
    return s * sqrt(best);
}

// **The authored contour's signed distance, as a chain of circular arcs**
// (ADR-0098's primitive, ADR-0107's figure).
//
// The same two quantities as `path_sd` — a `min` over pieces for the magnitude,
// a ray-crossing parity for the sign — over a chain that a curve needs FIVE TO
// TEN TIMES fewer of than the polyline it was fitted from. A piece costs more
// than a segment; whether that trade is a win is `path_cost.rs`'s reading, not
// an assertion here.
//
// **No `atan2` on the distance path.** Whether the nearest point on the circle
// lies within the piece's sweep is a sector test, and a sector test is a dot
// product against the sweep's mid direction — both precomputed CPU-side. The
// crossing test below does need the angle, but only for a piece the scan line
// actually meets, which is a small minority of them.
fn arc_chain_sd(p: vec2<f32>, n: u32) -> f32 {
    let TAU = 6.28318530718;
    var best = 1e20;
    var crossings = 0u;
    for (var i = 0u; i < n; i = i + 1u) {
        let base = i * 4u;
        let head = params.path[base];
        let ends = params.path[base + 2u];
        let a = ends.xy;
        let b = ends.zw;

        if (head.x < 0.5) {
            // A straight run — the fitter emits these for a corner it must keep
            // and for an arc whose radius is too large to shade stably, so this
            // arm carries a real share of a polygonal figure.
            let e = b - a;
            let w = p - a;
            let t = clamp(dot(w, e) / max(dot(e, e), 1e-20), 0.0, 1.0);
            let q = w - e * t;
            best = min(best, dot(q, q));
            // The half-open rule on y, exactly as the polyline uses it: a joint
            // lying on the scan line belongs to one piece, not to both.
            let c1 = p.y >= a.y;
            let c2 = p.y < b.y;
            let c3 = e.x * w.y > e.y * w.x;
            if ((c1 && c2 && c3) || (!c1 && !c2 && !c3)) {
                crossings = crossings + 1u;
            }
            continue;
        }

        let c = head.yz;
        let r = head.w;
        let sector = params.path[base + 1u];
        let sweep = params.path[base + 3u];
        let w = p - c;
        let l = length(w);
        // Inside the sweep, the nearest point on the circle is the nearest point
        // on the arc; outside it, the nearest point is whichever end is closer.
        if (l > 1e-9 && dot(w / l, sector.xy) >= sector.z) {
            let d = abs(l - r);
            best = min(best, d * d);
        } else {
            best = min(best, min(dot(p - a, p - a), dot(p - b, p - b)));
        }

        // The crossing test: where the scan line `y = p.y` meets this circle, to
        // the RIGHT of `p`, and inside the sweep.
        let dy = p.y - c.y;
        let disc = r * r - dy * dy;
        if (disc > 0.0) {
            let sx = sqrt(disc);
            for (var k = 0u; k < 2u; k = k + 1u) {
                let xr = c.x + select(-sx, sx, k == 1u);
                if (xr <= p.x) {
                    continue;
                }
                // Half-open on the sweep — `u < span`, not `<=` — so a joint on
                // the scan line is counted by the piece that starts there and
                // not also by the one that ends there.
                let ang = atan2(dy, xr - c.x);
                var u = (ang - sweep.x) * sign(sweep.y);
                u = u - TAU * floor(u / TAU);
                if (u < abs(sweep.y)) {
                    crossings = crossings + 1u;
                }
            }
        }
    }
    return select(1.0, -1.0, (crossings & 1u) == 1u) * sqrt(best);
}

// The contour's radius along the ray from the figure's centre through `p` — the
// divisor of `coord_mode = 1`'s scaled-copy coordinate (ADR-0111), on an
// authored contour instead of a rostered arm.
//
// The OUTERMOST crossing is taken. A single closed contour that is star-shaped
// about its centre has exactly one, and the choice only shows on one that is
// not (a crescent), where the outer edge is the boundary and the concavity is
// interior to the coordinate.
fn path_boundary_radius(p: vec2<f32>, n: u32) -> f32 {
    let l = length(p);
    if (l < 1e-6) {
        return 1e-6;
    }
    let u = p / l;
    var r = 0.0;
    var b = path_pt(n - 1u);
    for (var i = 0u; i < n; i = i + 1u) {
        let a = path_pt(i);
        let e = b - a;
        // Cross both sides of `s*u = a + e*t` with `u` to drop `s`, then solve
        // for the segment parameter `t`.
        let denom = e.x * u.y - e.y * u.x;
        if (abs(denom) > 1e-9) {
            let t = (a.y * u.x - a.x * u.y) / denom;
            if (t >= 0.0 && t <= 1.0) {
                let s = dot(a + e * t, u);
                r = max(r, s);
            }
        }
        b = a;
    }
    return max(r, 1e-6);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let aspect = params.a.x;
    let shape = params.a.y;
    let points = params.a.z;
    let scale = params.a.w;
    let pan = params.b.xy;
    let color_span = params.b.z;
    let color_center = params.b.w;
    let saturation = params.c.x;
    let palette_mix = params.c.y;
    let palette_steps = params.c.z;
    let palette_contour = params.c.w;
    let gamma = params.d.y;
    let coord_mode = params.d.z;
    let rotation = params.d.w;
    let star = params.e.xyz;
    let path_n = u32(params.f.x);
    let path_inradius = params.f.y;
    let stroke = params.f.z;
    let path_arcs = u32(params.f.w);

    // Square units, from the RENDER TARGET's aspect (ADR-0037): stretching x
    // makes one unit of `uv` the same length on both axes, so the figure below
    // is the shape it claims to be and not the window's shape.
    var uv = in.ndc;
    uv.x = uv.x * aspect;

    // The figure's own frame: `pan` moves its centre, `scale` sets its size.
    //
    // **`rotation` is applied AFTER the pan, and that is a choice.** Turning the
    // sample point before subtracting `pan` would swing the figure around the
    // frame's centre — an orbit — and turning it after swings it about its own.
    // Both are defensible and they look completely different; this scene draws
    // ONE figure, and a figure that spins in place is what `rotation` means on
    // `lines/star.rs` and `lines/lsystem.rs` too.
    //
    // It is done in `uv`, which is already SQUARE units (ADR-0037): x has been
    // stretched by the render target's aspect, so one unit is the same length on
    // both axes and this is a rotation. In raw NDC the same two lines would
    // SHEAR — invisible at 16:9, where the stretch is nearly 1, and obvious at
    // 2:1. `tests` renders a square at 2:1 and turns it a quarter turn.
    //
    // A branch rather than an unconditional multiply, so 0 is an exact identity
    // and no shipped preset moves through `cos`/`sin` (ADR-0092's care, the same
    // reason `gamma` has one).
    var q = uv - pan;
    if (rotation != 0.0) {
        let cr = cos(rotation);
        let sr = sin(rotation);
        // The INVERSE rotation on the sample point, so a positive `rotation`
        // turns the figure counter-clockwise on screen rather than the frame.
        q = vec2<f32>(cr * q.x + sr * q.y, cr * q.y - sr * q.x);
    }
    let p = q / scale;

    // THE substitution this scene exists for: the palette coordinate is a
    // FIGURE coordinate rather than a level. Both modes are 0 at the figure's
    // centre and exactly 1 on its outline, and both grow outward — what differs
    // is what a band of the coordinate is a band OF.
    //
    // An `if` rather than a `select`, and that is not style: `select` evaluates
    // both arms, and the second arm here is a whole second shape evaluation. The
    // mode is a per-draw uniform, so this branch is uniform across a warp and
    // the hardware takes one arm rather than both.
    //
    // An authored contour takes the same two modes on the same terms
    // (ADR-0107): what changes is where the silhouette came from, not what a
    // band of the coordinate is a band of. `path_n` is 0 for every preset that
    // declares no `[path]`, so those take the roster arms below and not one
    // instruction of the contour walk executes.
    var d: f32;
    if (path_arcs >= 1u) {
        // The arc chain, chosen CPU-side and only where it can serve: the
        // distance coordinate, and no morph in flight. `path_inradius` is the
        // POLYLINE's, which describes the same figure to within the fit's own
        // lateral budget — a sub-pixel difference in a divisor.
        d = max(1.0 + arc_chain_sd(p, path_arcs) / max(path_inradius, 1e-6), 0.0);
    } else if (path_n >= 3u) {
        if (coord_mode < 0.5) {
            // `1 + sd / inradius` — the SAME normalization `mark_distance`
            // applies to the roster, so an authored figure reads 0 at its
            // deepest interior point and exactly 1 on its outline like every
            // other silhouette this scene draws. Held at 0 from below because
            // the inradius is measured on a grid and can land a hair short of
            // the true deepest point; a negative coordinate would be a NaN
            // under a bound `gamma` (`pow` of a negative base).
            d = max(1.0 + path_sd(p, path_n) / max(path_inradius, 1e-6), 0.0);
        } else {
            d = length(p) / path_boundary_radius(p, path_n);
        }
    } else if (coord_mode < 0.5) {
        // Mode 0 — a band of the coordinate is a band of constant DISTANCE,
        // which is the definition of an offset curve (ADR-0105). This is the
        // default and it is bit-for-bit the arithmetic that shipped.
        d = mark_distance(p, shape, points, star);
    } else {
        // Mode 1 — a band of the coordinate is a band of constant SCALING, so
        // its level sets are scaled copies of the outline (ADR-0111). On a
        // polygon that keeps the corners the offsets round off; on a heart it
        // keeps the notch, which is the construction the reference images are.
        d = length(p) / max(mark_boundary_radius(p, shape, points, star), 1e-6);
    }

    // **The stroke's screen width, taken before any branch.** A derivative has
    // to be evaluated in uniform control flow, and hoisting it is what keeps
    // that true however the branch below is compiled — `band_contour` hoists
    // its own for the same reason.
    let d_width = max(fwidth(d), 1e-5);
    // The response exponent, applied to the distance BEFORE it becomes a palette
    // coordinate — so it reshapes where the contours sit rather than which
    // colours they take. Above 1 the bands crowd toward the centre, which is what
    // the reference images do and what a raw (evenly spaced) distance cannot.
    // `select` rather than a branch, and the identity is exact: `pow(x, 1.0)` is
    // not bit-exact, so an unbound preset must not go through it (ADR-0092).
    let shaped = select(pow(d, gamma), d, gamma == 1.0);
    let coord = shaped * color_span + color_center;

    // Hard bands, then the contour drawn from the SAME coordinate (ADR-0078).
    let banded = band_coord(coord, palette_steps);
    let ca = textureSample(lut_a, lut_samp, vec2<f32>(banded, 0.5)).rgb;
    let cb = textureSample(lut_b, lut_samp, vec2<f32>(banded, 0.5)).rgb;
    var col = mix(ca, cb, clamp(palette_mix, 0.0, 1.0));
    col = col * band_contour(
        coord, palette_steps, palette_contour, lut_a, lut_b, lut_samp, palette_mix
    );
    col = apply_saturation(col, saturation);

    // **Fill and stroke are one field, not two routes** (ADR-0107). `d` is the
    // single evaluation above; the interior is `d < 1` and the outline is
    // `abs(d - 1) < w`, so a stroke cannot drift off the fill it belongs to
    // because there is nothing for it to drift from. (The ADR writes the pair
    // as `d < 0` and `abs(d) < w` against a raw signed distance; this scene's
    // coordinate is that distance normalized to 1 on the outline, so the two
    // tests are the same two tests shifted by one.)
    //
    // Exactly 0 is the identity and takes the branch away, which is what keeps
    // every shipped preset and every golden baseline on the arithmetic it has.
    if (stroke > 0.0) {
        col = col * (1.0 - smoothstep(stroke - d_width, stroke + d_width, abs(d - 1.0)));
    }

    // Alpha: this field covers every pixel, which is the coverage it honestly
    // has (ADR-0056). `occlude` scales how much of that the backdrop underneath
    // resolves against (ADR-0085). Reached only when no post stage is active;
    // the chain owns the seam otherwise and the renderer hands a literal 1.0.
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
    e: [f32; 4],
    f: [f32; 4],
    /// The authored contour, two points per element — see the WGSL's `path_pt`.
    /// Written every frame with the rest of the struct; it changes only on a
    /// preset switch, and 1.5 KB of `write_buffer` is far below the cost of
    /// splitting it into a second binding whose layout shape would then have to
    /// be argued against ADR-0058.
    path: [[f32; 4]; PATH_VEC4S],
}

/// A fullscreen signed-distance figure from the shared mark roster, coloured
/// through the shared palette.
pub struct ShapeFieldScene {
    /// The pipeline, the uniform buffer, the 256x1 gradient LUT pair (A/B) the
    /// fragment samples + crossfades for colour (ADR-0021), and the one bind
    /// group this scene binds.
    gpu: gpu::FullscreenScene,
    /// The silhouette and its point count, raw as the preset bound them —
    /// `marks::mark_shape` / `mark_points` quantize on the way to the uniform,
    /// which is where a selector's precondition belongs (the `kaleido_edge`
    /// precedent).
    shape: f32,
    points: f32,
    /// The `star` arm's three shape params, raw as the preset bound them
    /// (Plan 0091 Phase 5). `marks::star_*` condition them on the way to the
    /// uniform. Inert on every other silhouette, and nothing warns —
    /// `presets/README.md` carries that.
    star_valley: f32,
    star_curve: f32,
    star_jitter: f32,
    scale: f32,
    /// The shared palette knobs (ADR-0021). This scene has no `hue` or
    /// `brightness`.
    colour: common::PaletteParams,
    /// The shared view transform (ADR-0018).
    pan: common::PanParams,
    color_span: f32,
    color_center: f32,
    /// The response exponent on the distance, raw as the preset bound it;
    /// [`applied_gamma`] conditions it on the way to the uniform.
    gamma: f32,
    /// Which coordinate the palette is handed, raw as the preset bound it;
    /// [`applied_coord_mode`] quantizes it on the way to the uniform, which is
    /// where a selector's precondition belongs.
    coord_mode: f32,
    /// The figure's own turn, in radians, raw as the preset bound it. Applied
    /// about the figure's centre rather than the frame's — see the shader.
    rotation: f32,
    /// The stroke half-width in coordinate units, raw as the preset bound it;
    /// [`applied_stroke`] conditions it on the way to the uniform. `0` — the
    /// default — is the filled figure and an exact arithmetic identity.
    stroke: f32,
    /// The authored contour, and the one `morph` travels towards (ADR-0107).
    /// Both are set by [`Scene::configure`] on a preset switch and by nothing
    /// else — geometry is structural, not a param, so `reset_params` does not
    /// touch them.
    ///
    /// Fewer than 3 points in `path_from` means no authored contour and the
    /// scene draws the `marks` roster, which is what every preset declaring no
    /// `[path]` gets. An empty `path_to` means no morph target, so `morph` is
    /// inert and the packed contour is `path_from` verbatim.
    path_from: Vec<[f32; 2]>,
    path_to: Vec<[f32; 2]>,
    /// The same authored outline as a **G1-continuous chain of circular arcs**,
    /// fitted at load through the line renderer's own fitter (ADR-0098). Empty
    /// where the fit was not worth keeping, and unread while a morph is in
    /// flight or under the scaled-copy coordinate — see [`Self::pack_path`].
    pieces: Vec<crate::render::scenes::lines::biarc::Piece>,
    /// The contour packed for the uniform — the interpolation of the two above
    /// at this frame's `morph`, rebuilt in `render`.
    ///
    /// A field rather than a local so the per-frame pack writes into an
    /// allocation made once. This is the render thread, not the audio callback,
    /// but the rule that a per-frame path does not allocate is the same one.
    path: Box<[[f32; 4]; PATH_VEC4S]>,
    /// The two contours' inradii — the distance from each one's deepest interior
    /// point to its own outline, measured by [`contour_inradius`] at configure
    /// time.
    ///
    /// It is the divisor that makes the authored figure's coordinate `0` at that
    /// deepest point and `1` on the outline, which is the contract every
    /// silhouette in this scene meets (`marks`' own header states it for the
    /// roster).
    ///
    /// **Mid-morph the two are interpolated rather than re-measured.** The true
    /// inradius of an interpolated contour is not the interpolation of the two
    /// inradii, and measuring it is a grid search — load-time work, not
    /// per-frame. The error is bounded and one-sided in the direction that
    /// matters: the shader clamps the coordinate at 0 from below, so an
    /// underestimate costs nothing and an overestimate leaves the innermost
    /// sliver short of the palette's first texel.
    path_inradius: f32,
    path_inradius_to: f32,
    /// How far along `path_from` -> `path_to` the figure is, raw as the preset
    /// bound it. `0` — the default — is the authored figure, and an exact
    /// identity: the interpolation is skipped entirely.
    morph: f32,
    /// How much of this field's (total) coverage the backdrop resolves against
    /// (ADR-0085). Set by the renderer every frame — **not** a named param, so
    /// it is not reset by `reset_params`.
    occlude: f32,
}

impl ShapeFieldScene {
    /// Build the scene's pipeline and uniform buffer on `device`.
    pub fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Self {
        // The mark roster's chunk is prepended, exactly as the two particle
        // scenes do it: it declares no bindings and no entry points, so
        // splicing it in changes nothing about this pipeline's layout.
        let source = format!("{}{SHADER}", marks::sdf_wgsl());
        let shader = gpu::fullscreen_shader(
            device,
            "shape-field-shader",
            gpu::FULLSCREEN_VS_NDC,
            &source,
        );
        let parts = gpu::FullscreenParts::new(device, "shape-field", std::mem::size_of::<Params>());
        // One group, sampler first and uniform last — see the WGSL's note for
        // why this shape and not `fragment_field`'s two-group split. The uniform
        // entry is a full literal rather than `gpu::uniform` because that helper
        // passes `min_binding_size: None`, and declaring one is half of what
        // makes this shape unique.
        let bind_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("shape-field-bind-layout"),
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
            ],
        });
        // This layout binds the sampler first and the two textures after it, so
        // the pair's role-ordered array is destructured into binding order here.
        let [lut_a, lut_b, lut_sampler] = parts.luts().bind_entries(1, 2, 0);
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("shape-field-bind-group"),
            layout: &bind_layout,
            entries: &[
                lut_sampler,
                lut_a,
                lut_b,
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: parts.uniforms().as_entire_binding(),
                },
            ],
        });

        Self {
            gpu: parts.finish(
                device,
                &shader,
                &[&bind_layout],
                bind_group,
                None,
                surface_format,
                wgpu::BlendState::REPLACE,
                "shape-field",
            ),
            shape: marks::DEFAULT_SHAPE,
            points: marks::DEFAULT_POINTS,
            star_valley: marks::DEFAULT_STAR_VALLEY,
            star_curve: marks::DEFAULT_STAR_CURVE,
            star_jitter: marks::DEFAULT_STAR_JITTER,
            scale: DEFAULT_SCALE,
            colour: common::PaletteParams::new(0.0, common::DEFAULT_BRIGHTNESS),
            pan: common::PanParams::default(),
            color_span: DEFAULT_COLOR_SPAN,
            color_center: DEFAULT_COLOR_CENTER,
            gamma: DEFAULT_GAMMA,
            coord_mode: DEFAULT_COORD_MODE,
            rotation: DEFAULT_ROTATION,
            stroke: DEFAULT_STROKE,
            morph: DEFAULT_MORPH,
            path_from: Vec::new(),
            path_to: Vec::new(),
            pieces: Vec::new(),
            path: Box::new([[0.0; 4]; PATH_VEC4S]),
            path_inradius: 1.0,
            path_inradius_to: 1.0,
            occlude: crate::render::post::DEFAULT_OCCLUDE,
        }
    }
}

/// The `scale` the shader is handed: held inside the range the arithmetic needs,
/// with a non-finite binding falling back to the default.
///
/// The clamp is CPU-side for the reason `background::applied_ramp_gamma` states:
/// it can never be reached with a NaN, where WGSL's `clamp` is
/// implementation-defined, and the default stays **exactly** the default on the
/// way to the uniform.
fn applied_scale(scale: f32) -> f32 {
    if scale.is_finite() {
        scale.clamp(MIN_SCALE, MAX_SCALE)
    } else {
        DEFAULT_SCALE
    }
}

/// The `rotation` the shader is handed: passed through, with a non-finite
/// binding falling back to the identity.
///
/// No clamp, because an angle wraps and there is no end of the range to hold it
/// inside — the same treatment `lines/star.rs` gives the name. The finiteness
/// guard is not decoration: `cos(NaN)` is `NaN`, and one `NaN` in the figure's
/// own frame takes every pixel of the frame with it.
fn applied_rotation(rotation: f32) -> f32 {
    if rotation.is_finite() {
        rotation
    } else {
        DEFAULT_ROTATION
    }
}

/// The `coord_mode` the shader is handed: clamped into the roster, then
/// **rounded to an integer**, with a non-finite binding falling back to the
/// default — and **forced back to the distance on a `ring`**.
///
/// The quantizing half is `marks::mark_shape`'s treatment for
/// `marks::mark_shape`'s reason, and the `kaleido_edge` precedent behind both. A
/// mode's values are **identities** rather than a quantity: `[smoothing]` and
/// preset dissolves interpolate a binding continuously from one setting to
/// another, so easing the distance to the radius passes through 0.4 and 0.6, and
/// there is nothing halfway between an offset curve and a scaled copy for the
/// shader to draw there.
///
/// # The `ring` fallback, and why it is not silent
///
/// An annulus's centre is in its hole, so `r / r_boundary` has no single value
/// there — the one behavioural choice ADR-0111 leaves open. Plan 0098
/// Phase 4 rendered the three defensible answers before picking, and what
/// settled it is that defining the boundary as the outer rim produces a figure
/// **byte-identical to a `disc`**: the coordinate collapses to `length(p)` and
/// the hole stops existing, so a preset naming one roster entry would be shown
/// another. That is the negative ADR-0111 records, reached in practice.
///
/// So the combination is refused rather than approximated, and the refusal is
/// **announced**: `Preset::from_toml_str` warns at load when a preset rests on
/// it (ADR-0020's shape, the `thickness` dead-zone precedent). The silent
/// fallback was the third candidate and it is the one this rejects — it renders
/// the same pixels as this does and costs an author the afternoon.
fn applied_coord_mode(mode: f32, shape: f32) -> f32 {
    if shape == marks::RING_SHAPE {
        return DEFAULT_COORD_MODE;
    }
    if mode.is_finite() {
        mode.clamp(MIN_COORD_MODE, MAX_COORD_MODE).round()
    } else {
        DEFAULT_COORD_MODE
    }
}

/// The exponent the shader will **actually apply** for a bound `gamma`: a
/// non-finite binding falls back to the identity, and a finite one is held
/// inside the positive range ([`MIN_GAMMA`], [`MAX_GAMMA`]).
///
/// CPU-side for `ink::applied_gamma`'s two reasons: `1.0` stays **exactly**
/// `1.0` on the way to the uniform, which is what the shader's identity branch
/// tests, and the clamp can never be reached with a NaN, where WGSL's `clamp` is
/// implementation-defined.
fn applied_gamma(gamma: f32) -> f32 {
    if gamma.is_finite() {
        gamma.clamp(MIN_GAMMA, MAX_GAMMA)
    } else {
        DEFAULT_GAMMA
    }
}

/// The `stroke` the shader is handed: held inside its range, with a non-finite
/// binding falling back to the filled figure.
///
/// **`0` survives as exactly `0`**, which the shader's identity branch tests
/// for: a filled figure must execute none of the stroke arithmetic, so every
/// shipped preset and every golden baseline stays on what it has (ADR-0092's
/// care, the same reason `gamma` and `rotation` have identity branches).
fn applied_stroke(stroke: f32) -> f32 {
    if stroke.is_finite() {
        stroke.clamp(0.0, MAX_STROKE)
    } else {
        DEFAULT_STROKE
    }
}

/// The `morph` the contour is interpolated at: held inside `0..=1`, with a
/// non-finite binding falling back to the authored figure.
///
/// Clamped rather than wrapped, and not extrapolated past either end: outside
/// `0..=1` the interpolation leaves both authored silhouettes behind and the
/// figure is one nobody drew — which is a different thing from the mid-morph
/// shapes nobody drew, because those at least lie between two that someone did.
fn applied_morph(morph: f32) -> f32 {
    if morph.is_finite() {
        morph.clamp(0.0, 1.0)
    } else {
        DEFAULT_MORPH
    }
}

/// The contour's **inradius**: the distance from its deepest interior point to
/// its own outline, in the normalized `[-1, 1]` frame the contour lives in.
///
/// Measured rather than derived, because a closed contour has no closed form for
/// it. A coarse grid over the box finds the deepest cell, then three rounds of
/// local search shrink around it — so the reading is the grid's resolution only
/// until the refinement, and the refinement halves its neighbourhood each round.
///
/// **It can still land a hair short**, which is why the shader clamps the
/// coordinate at 0 from below rather than trusting this. Short is the safe
/// direction: it makes the innermost sliver of the figure read as the palette's
/// first texel, where over-reporting would leave the interior never reaching it.
fn contour_inradius(points: &[[f32; 2]]) -> f32 {
    /// Cells per axis of the first pass, over the `[-1, 1]` box.
    const GRID: i32 = 96;
    /// Local refinement rounds, each halving the search radius.
    const REFINE: u32 = 12;

    let depth_at = |p: [f32; 2]| -> f32 {
        // Unsigned distance to the closing polygon, and a crossing parity for
        // whether `p` is inside it — the CPU counterpart of the WGSL's
        // `path_sd`, kept to the one quantity the shader needs from the CPU
        // rather than mirroring the whole field.
        let n = points.len();
        let mut best = f32::INFINITY;
        let mut inside = false;
        for i in 0..n {
            let (Some(&a), Some(&b)) = (points.get(i), points.get((i + n - 1) % n)) else {
                continue;
            };
            let e = [b[0] - a[0], b[1] - a[1]];
            let w = [p[0] - a[0], p[1] - a[1]];
            let ee = (e[0] * e[0] + e[1] * e[1]).max(1e-20);
            let t = ((w[0] * e[0] + w[1] * e[1]) / ee).clamp(0.0, 1.0);
            let q = [w[0] - e[0] * t, w[1] - e[1] * t];
            best = best.min(q[0] * q[0] + q[1] * q[1]);
            let c1 = p[1] >= a[1];
            let c2 = p[1] < b[1];
            let c3 = e[0] * w[1] > e[1] * w[0];
            if (c1 && c2 && c3) || (!c1 && !c2 && !c3) {
                inside = !inside;
            }
        }
        if inside { best.sqrt() } else { 0.0 }
    };

    let mut best_p = [0.0f32, 0.0];
    let mut best_d = depth_at(best_p);
    for gy in 0..=GRID {
        for gx in 0..=GRID {
            let p = [
                (gx as f32 / GRID as f32) * 2.0 - 1.0,
                (gy as f32 / GRID as f32) * 2.0 - 1.0,
            ];
            let d = depth_at(p);
            if d > best_d {
                best_d = d;
                best_p = p;
            }
        }
    }
    let mut radius = 2.0 / GRID as f32;
    for _ in 0..REFINE {
        for (dx, dy) in [
            (-1.0f32, 0.0f32),
            (1.0, 0.0),
            (0.0, -1.0),
            (0.0, 1.0),
            (-1.0, -1.0),
            (1.0, -1.0),
            (-1.0, 1.0),
            (1.0, 1.0),
        ] {
            let p = [best_p[0] + dx * radius, best_p[1] + dy * radius];
            let d = depth_at(p);
            if d > best_d {
                best_d = d;
                best_p = p;
            }
        }
        radius *= 0.5;
    }
    // A contour with no interior the search could find would divide the whole
    // frame by zero; the floor keeps the coordinate finite and the figure reads
    // as all exterior, which is what a zero-area contour is.
    best_d.max(1e-4)
}

/// The palette coordinate this scene hands the LUT, as a CPU mirror of the
/// shader's two lines — so the exponent's properties are testable without a GPU
/// (the arrangement `ink::key` and `tonemap::map` both use).
#[cfg(test)]
pub(crate) fn coord(distance: f32, gamma: f32, color_span: f32, color_center: f32) -> f32 {
    let g = applied_gamma(gamma);
    let shaped = if g == 1.0 { distance } else { distance.powf(g) };
    shaped * color_span + color_center
}

/// The parameter names this scene consumes — the vocabulary a preset binding is
/// checked against at load (ADR-0020). **Keep in sync with `set_param` below**;
/// `declared_params_match_set_param` in `core/tests/preset.rs` fails if the two
/// drift.
pub const PARAMS: &[ParamSpec] = &[
    crate::render::scenes::marks::SHAPE,
    crate::render::scenes::marks::POINTS,
    crate::render::scenes::marks::STAR_VALLEY,
    crate::render::scenes::marks::STAR_CURVE,
    crate::render::scenes::marks::STAR_JITTER,
    ParamSpec {
        name: "scale",
        default: 0.6,
        range: Some([0.05, 2.0]),
        doc: "Size of the shape within the frame.",
        kind: ParamKind::Modal,
    },
    crate::render::scenes::common::PAN_X,
    crate::render::scenes::common::PAN_Y,
    ParamSpec {
        name: "color_span",
        default: 0.6,
        range: Some([0.0, 1.0]),
        doc: "How much of the palette the field's range covers.",
        kind: ParamKind::Modal,
    },
    ParamSpec {
        name: "color_center",
        default: 0.0,
        range: Some([-1.0, 1.0]),
        doc: "Shifts which part of that range lands in the middle of the palette.",
        kind: ParamKind::Modal,
    },
    crate::render::scenes::common::SATURATION,
    crate::render::scenes::common::PALETTE_MIX,
    crate::render::scenes::common::PALETTE_STEPS,
    crate::render::scenes::common::PALETTE_CONTOUR,
    ParamSpec {
        name: "gamma",
        default: 1.0,
        range: Some([0.25, 4.0]),
        doc: "Shapes the falloff from the shape's edge; below 1 it bites sooner.",
        kind: ParamKind::Modal,
    },
    ParamSpec {
        name: "coord_mode",
        default: 0.0,
        // The top of the range is the roster's last index, which is where
        // `applied_coord_mode` clamps. A range above it advertises a mode that
        // silently resolves to another one.
        range: Some([MIN_COORD_MODE, MAX_COORD_MODE]),
        doc: "Which coordinate frame the distance is measured in, which changes the shape's whole geometry.",
        kind: ParamKind::Modal,
    },
    ParamSpec {
        name: "rotation",
        default: 0.0,
        range: Some([0.0, 1.0]),
        doc: "Turns the shape, as a fraction of a full turn.",
        kind: ParamKind::Modal,
    },
    ParamSpec {
        name: "stroke",
        default: 0.0,
        range: Some([0.0, 1.0]),
        doc: "Draws the outline instead of the filled figure, at this half-width; 0 fills.",
        kind: ParamKind::Modal,
    },
    ParamSpec {
        name: "morph",
        default: 0.0,
        range: Some([0.0, 1.0]),
        doc: "Travels the authored path towards its morph_to silhouette; inert without one.",
        kind: ParamKind::Modal,
    },
];

impl ShapeFieldScene {
    /// Pack this frame's contour into the uniform array and report `(point
    /// count, inradius)` for the uniform's scalars.
    ///
    /// **The morph is interpolated here, on the CPU, once per frame** — not per
    /// pixel in the shader. The alternative was to hand the GPU both contours
    /// and lerp inside the distance loop, which would double the uniform, double
    /// the per-pixel loads, and re-derive at 2 M pixels a value that changes once
    /// a frame. `morph` is a parameter, not geometry.
    ///
    /// At `morph = 0`, or with no target, the authored contour is copied
    /// verbatim and no interpolation runs — the identity every other param on
    /// this scene keeps.
    fn pack_path(&mut self, coord_mode: f32) -> (usize, usize, f32) {
        let n = self.path_from.len().min(MAX_SAMPLES);
        if n < 3 {
            return (0, 0, 1.0);
        }
        let morphing = self.path_to.len() == self.path_from.len();

        // **The arc chain serves where it can, and the polyline everywhere
        // else.** Two things put a figure back on points, and both are the
        // chain's own limits rather than a preference:
        //
        // - **a morph in flight.** Phase-correspondent points interpolate; two
        //   arc chains have no such correspondence, and inventing one is the
        //   representation problem ADR-0075 exists about. So a morphing pair
        //   travels on the polyline it was aligned as.
        // - **the scaled-copy coordinate.** `coord_mode = 1` needs the boundary
        //   radius along a ray, which is a second intersection routine the chain
        //   does not carry.
        if !morphing && coord_mode < 0.5 && !self.pieces.is_empty() {
            let pieces = self.pieces.len().min(MAX_ARC_PIECES);
            self.pack_pieces(pieces);
            return (0, pieces, self.path_inradius);
        }

        let t = if morphing {
            applied_morph(self.morph)
        } else {
            0.0
        };
        for i in 0..n {
            let Some(&a) = self.path_from.get(i) else {
                continue;
            };
            let p = match self.path_to.get(i) {
                Some(&b) if t != 0.0 => [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t],
                _ => a,
            };
            let slot = i >> 1;
            let half = (i & 1) * 2;
            if let Some(v) = self.path.get_mut(slot) {
                if let Some(x) = v.get_mut(half) {
                    *x = p[0];
                }
                if let Some(y) = v.get_mut(half + 1) {
                    *y = p[1];
                }
            }
        }
        let inradius = self.path_inradius + (self.path_inradius_to - self.path_inradius) * t;
        (n, 0, inradius.max(1e-4))
    }

    /// Pack the fitted arc chain into the uniform: four elements per piece, in
    /// the layout the WGSL's `Params.path` comment spells out.
    ///
    /// The sector test's mid direction and half-angle are computed **here**, so
    /// the fragment's own test is a dot product rather than an `atan2` per piece
    /// per pixel. Same for the endpoints, which the outside-the-sweep arm needs.
    fn pack_pieces(&mut self, count: usize) {
        use crate::render::scenes::lines::biarc::Piece;
        for i in 0..count {
            let Some(&piece) = self.pieces.get(i) else {
                continue;
            };
            let base = i * VEC4S_PER_PIECE;
            let (a, b) = (piece.start_point(), piece.end_point());
            let (head, sector, sweep) = match piece {
                Piece::Arc {
                    centre,
                    radius,
                    start,
                    sweep,
                } => {
                    let mid = start + sweep * 0.5;
                    (
                        [1.0, centre[0], centre[1], radius],
                        [mid.cos(), mid.sin(), (sweep.abs() * 0.5).cos(), 0.0],
                        [start, sweep, 0.0, 0.0],
                    )
                }
                Piece::Line { .. } => ([0.0; 4], [0.0; 4], [0.0; 4]),
            };
            for (offset, value) in [
                (0, head),
                (1, sector),
                (2, [a[0], a[1], b[0], b[1]]),
                (3, sweep),
            ] {
                if let Some(slot) = self.path.get_mut(base + offset) {
                    *slot = value;
                }
            }
        }
    }
}

impl Scene for ShapeFieldScene {
    fn name(&self) -> &'static str {
        "shape field"
    }

    fn set_occlude(&mut self, occlude: f32) {
        self.occlude = occlude;
    }

    fn set_palette(&mut self, palette: &Palette) {
        self.gpu.set_palette(palette);
    }

    fn reset_params(&mut self) {
        self.shape = marks::DEFAULT_SHAPE;
        self.points = marks::DEFAULT_POINTS;
        self.star_valley = marks::DEFAULT_STAR_VALLEY;
        self.star_curve = marks::DEFAULT_STAR_CURVE;
        self.star_jitter = marks::DEFAULT_STAR_JITTER;
        self.scale = DEFAULT_SCALE;
        self.colour.reset();
        self.pan.reset();
        self.color_span = DEFAULT_COLOR_SPAN;
        self.color_center = DEFAULT_COLOR_CENTER;
        self.gamma = DEFAULT_GAMMA;
        self.coord_mode = DEFAULT_COORD_MODE;
        self.rotation = DEFAULT_ROTATION;
        self.stroke = DEFAULT_STROKE;
        self.morph = DEFAULT_MORPH;
    }

    /// The `[path]` table (ADR-0107), which is the only structural config this
    /// scene takes.
    ///
    /// **Called on every `shape_field` preset switch, table or no table** — the
    /// loader hands `Some(Path { shape: None })` for a preset that declares
    /// none, exactly so this runs and clears the contour. Without that, a switch
    /// from a path preset to a roster one would keep drawing the outgoing
    /// preset's silhouette.
    fn configure(
        &mut self,
        cfg: &super::lines::GeneratorConfig,
    ) -> Option<super::lines::CapOverflow> {
        if let super::lines::GeneratorConfig::Path { shape, morph_to } = cfg {
            self.path_from.clear();
            self.path_to.clear();
            self.pieces.clear();
            self.path_inradius = 1.0;
            self.path_inradius_to = 1.0;
            if let Some(contour) = shape {
                // The load boundary already refused an arity above the ceiling,
                // so the `take` is a belt on a boundary that holds rather than a
                // decimation an author is not told about.
                self.path_from
                    .extend(contour.points().iter().take(MAX_SAMPLES).copied());
                self.path_inradius = contour_inradius(&self.path_from);
                self.pieces.extend_from_slice(contour.pieces());
            }
            // The pair is aligned at load; a target of a different arity would
            // mean the loader let one through, so it is dropped rather than
            // interpolated against the wrong correspondent.
            if let Some(target) = morph_to
                .as_ref()
                .filter(|t| t.points().len() == self.path_from.len())
            {
                self.path_to.extend(target.points().iter().copied());
                self.path_inradius_to = contour_inradius(&self.path_to);
            }
        }
        None
    }

    fn set_param(&mut self, name: &str, value: f32) {
        // The shared param blocks first, this scene's own names after
        // (`scenes::common`).
        if self.colour.set(name, value) || self.pan.set(name, value) {
            return;
        }
        match name {
            "shape" => self.shape = value,
            "points" => self.points = value,
            "star_valley" => self.star_valley = value,
            "star_curve" => self.star_curve = value,
            "star_jitter" => self.star_jitter = value,
            "scale" => self.scale = value,
            "color_span" => self.color_span = value,
            "color_center" => self.color_center = value,
            "gamma" => self.gamma = value,
            "coord_mode" => self.coord_mode = value,
            "rotation" => self.rotation = value,
            "stroke" => self.stroke = value,
            "morph" => self.morph = value,
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
        // Quantized once, because `applied_coord_mode` has to see the same value
        // the shader will: the `ring` refusal is a fact about the SELECTED arm,
        // not about the raw binding.
        let shape = marks::mark_shape(self.shape);
        self.gpu.flush_palette(queue);
        let coord_mode = applied_coord_mode(self.coord_mode, shape);
        let (path_count, path_arcs, path_inradius) = self.pack_path(coord_mode);

        let params = Params {
            // `aspect` is the argument the chain hands down for the target this
            // scene is drawing into — never a size this scene chose (ADR-0037).
            a: [
                aspect.max(0.1),
                shape,
                marks::mark_points(self.points),
                applied_scale(self.scale),
            ],
            b: [self.pan.x, self.pan.y, self.color_span, self.color_center],
            c: [
                self.colour.saturation,
                self.colour.mix,
                palette::band_steps(self.colour.steps),
                palette::band_contour(self.colour.contour),
            ],
            d: [
                self.occlude,
                applied_gamma(self.gamma),
                coord_mode,
                applied_rotation(self.rotation),
            ],
            e: [
                marks::star_valley(self.star_valley),
                marks::star_curve(self.star_curve),
                marks::star_jitter(self.star_jitter),
                0.0,
            ],
            f: [
                path_count as f32,
                path_inradius,
                applied_stroke(self.stroke),
                path_arcs as f32,
            ],
            path: *self.path,
        };
        self.gpu.write_uniform(queue, &params);
        self.gpu
            .draw(encoder, "shape-field-pass", view, wgpu::LoadOp::Load);
    }
}

#[cfg(test)]
mod tests;
