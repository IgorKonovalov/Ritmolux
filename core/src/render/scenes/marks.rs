//! **A particle mark's silhouette, as a signed-distance function**
//! (ADR-0084, Plan 0070).
//!
//! This module is the shape vocabulary `swarm` and `emitter` share: one WGSL chunk, one
//! roster, one quantizer, so the two cannot drift. Without it a mark is a round additive
//! blob — `let d = length(in.local); let falloff = max(0.0, 1.0 - d); let g = falloff *
//! falloff;` — with no shape input at all.
//!
//! # The normalization is one rule, and it is what keeps `disc` exact
//!
//! [`mark_distance`](self) returns
//!
//! ```text
//! d = 1 + sd(p) / R
//! ```
//!
//! where `sd` is the shape's signed distance in the sprite's local frame
//! (`local` spans `[-1, 1]` on both axes) and `R` is that shape's **inradius** —
//! the distance from its deepest interior point to its own outline. So `d` is
//! `0` at that deepest point, exactly `1` on the outline, and greater than 1
//! outside it, whatever the shape.
//!
//! Two consequences, both load-bearing:
//!
//! - **The `disc` arm is `length(p)` and nothing else.** A unit disc has
//!   `sd = length(p) - 1` and `R = 1`, so the rule collapses to `length(p)` —
//!   the unshaped blob's own expression, not an approximation of it. A preset
//!   that names no shape takes this arm and is byte-identical to one drawn
//!   without the roster.
//! - **Only the interior matters — *to a particle*.** The falloff downstream is
//!   unchanged — `g = max(0, 1 - d)^2` — so every fragment at `d >= 1` is
//!   black. For `swarm` and `emitter` the lit region *is* the silhouette, and a
//!   distance function only has to be right inside it. That is what made the
//!   polygon and star arms two cheap lines rather than exact SDFs.
//!
//! # The exterior became load-bearing (ADR-0105, Plan 0091 Phase 2)
//!
//! `shape_field` draws this roster at frame scale and bands the distance into
//! contours, so it reads the region **outside** the silhouette — which nothing
//! had ever looked at. Measured against a numerically sampled outline (see
//! [`the_exterior_distance_is_measured_against_each_shapes_own_outline`](tests::the_exterior_distance_is_measured_against_each_shapes_own_outline),
//! which carries the full table), the two cheap arms were exactly as wrong as
//! the sentence above implies: `polygon` 0.326 and `star` 1.057 out, in
//! sprite-local units where the whole sprite is 2 wide.
//!
//! Both are now **exact outside**, and the repair is shaped so the particle path
//! cannot notice:
//!
//! - each arm keeps its original expression verbatim for `d <= 1`, so every
//!   fragment a sprite lights is the arithmetic it was before, bit for bit —
//!   asserted by all 29 golden baselines re-blessing byte-identical;
//! - past that, the fold has already selected the one edge that can be nearest,
//!   so the exact distance is to that edge as a **segment** rather than as an
//!   infinite line. The clamp is the whole repair: it is what makes a point
//!   beyond a vertex measure to the vertex.
//!
//! **`star`'s interior stays approximate, knowingly.** It measures against the
//! edge plane rather than the figure, and the error grows with the point count —
//! 0.00075 at 3 points, 0.066 at 5, 0.138 at 7, 0.248 at 12. Repairing it would
//! move every shipped `shape = "3"` mark, so it is recorded rather than fixed.
//!
//! # What this deliberately is not
//!
//! A silhouette **in additive light**. There is no fill and no outline — black
//! adds zero, so a heart here is a heart-shaped *glow*, not a red body with a
//! dark edge. ADR-0084 answers the silhouette half of design-backlog 0033 and
//! says plainly that it does not answer the other half; `presets/README.md`
//! repeats the warning at the parameter.

// Hot-path panic-denial pragma, as everywhere under `scenes/`.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

/// The `shape` roster, in the order the numeric parameter selects them.
///
/// `shape` is a **numeric selector**, like `kaleido_edge`: the preset expression
/// grammar has no strings, so a preset writes `shape = "3"` for a star. This
/// list is the single statement of what each index means, and both particle
/// scenes read it — a look wanting a shape that is not here routes back through
/// `architect` (ADR-0084's closed-roster consequence).
pub(crate) const SHAPES: [&str; 5] = ["disc", "ring", "polygon", "star", "heart"];

/// The first index the roster defines (0 = `disc`).
const MIN_SHAPE: f32 = 0.0;
/// The last index the roster defines. Values past it clamp here rather than
/// selecting the shader's fall-through arm by accident.
const MAX_SHAPE: f32 = SHAPES.len() as f32 - 1.0;
/// `shape` default — **`disc`**, which is exactly the arithmetic every mark drew
/// before this module existed.
pub(crate) const DEFAULT_SHAPE: f32 = 0.0;

/// Fewest points a polygon or star may have. Two "points" is a line, and the
/// star arm's inner vertex is only defined for `n >= 3`.
const MIN_POINTS: f32 = 3.0;
/// Most points a polygon or star may have. Past a dozen the marks these are
/// *for* — a few pixels across — are a disc with a rough edge, and the angular
/// fold costs the same either way.
const MAX_POINTS: f32 = 12.0;
/// `points` default — a five-pointed star / a pentagon.
pub(crate) const DEFAULT_POINTS: f32 = 5.0;

// --- The silhouette constants the WGSL is templated with -----------------------
//
// Substituted into [`SDF_WGSL`] rather than written twice: a second copy in a
// shader string is a constant that drifts silently the first time the Rust one
// moves (the `%ANISO%` precedent in `emitter.rs`).

/// `ring`: the lit circle's radius, and the half-width of its band. The outer
/// edge lands exactly on the sprite quad's inscribed circle (`0.7 + 0.3 = 1`),
/// so a ring is exactly as large as a disc, and its hole reaches `0.4` — wide
/// enough that the hole survives at the sizes these marks are for, which a
/// narrower one does not.
const RING_MID: f32 = 0.7;
const RING_HALF: f32 = 0.3;

/// `star_valley` default — the inner (valley) radius as a fraction of the outer
/// (tip) one, and **exactly** the constant this arm was welded to before Plan
/// 0091 Phase 5 promoted it.
///
/// Chosen for the size the ask is *for* — small marks, a few pixels across.
/// Below about a third the spikes are thinner than a pixel at those sizes and
/// the mark reads as a dot with a halo; above about a half it reads as a
/// slightly bumpy polygon. At 0.45 a seven-pointed star still has seven legible
/// points at a dozen pixels across.
///
/// **The default is not a judgement call, it is an obligation.** The shared
/// chunk means this knob reaches `swarm` and `emitter` as well as
/// `shape_field`, so anything but the old constant moves every shipped
/// `shape = "3"` preset (ADR-0105's shared-chunk consequence).
pub(crate) const DEFAULT_STAR_VALLEY: f32 = 0.45;
/// The range `star_valley` is held in. Not 0 and not 1: at 0 the spikes meet at
/// a point and the figure has no interior, and at 1 it is a polygon with the
/// valley on the circumcircle, so both ends are degenerate rather than extreme.
const MIN_STAR_VALLEY: f32 = 0.05;
const MAX_STAR_VALLEY: f32 = 0.95;

/// `star_curve` default — **0, the straight edge**, and an exact identity: at
/// this value the arm takes the closed-form straight-edge branch, which is the
/// arithmetic that shipped.
pub(crate) const DEFAULT_STAR_CURVE: f32 = 0.0;
/// How far the edge may bow. Positive pulls the edge's midpoint toward the
/// centre (the concave sparkle silhouette); negative pushes it out. Bounded
/// short of 1, where the midpoint would reach the origin and the edge would fold
/// through itself.
const MAX_STAR_CURVE: f32 = 0.9;

/// `star_jitter` default — **0, every spike the same length**, and an exact
/// identity for the same reason `star_curve`'s is.
pub(crate) const DEFAULT_STAR_JITTER: f32 = 0.0;
/// The most a spike's tip radius may vary. At 1 a spike can vanish into the
/// valley circle, which is the end of the range rather than a useful value.
const MAX_STAR_JITTER: f32 = 1.0;

/// How many sub-segments the curved edge is sampled into.
///
/// The exact distance to a quadratic Bezier is a cubic solve; this samples it
/// instead and measures against the polyline, which is the same trade the
/// project's own ground-truth harness makes. Eight is where the residual stops
/// mattering — `marks/tests.rs` measures what it actually is. Only the curved
/// branch pays for it: the neutral configuration takes the closed form.
const STAR_SEGMENTS: i32 = 8;

/// `heart`: the lobe circles' radius, `sqrt(2) / 4`.
///
/// The heart below is Inigo Quilez's, and it is the one shape here whose
/// distance function is worth naming a source for: two lobe circles centred
/// `(+-0.25, 0.75)` of this radius, closed underneath by the two 45-degree rays
/// from the origin that are *tangent* to them at `(+-0.5, 0.5)`, with the top
/// notch at `(0, 1)` where the circles meet.
const HEART_LOBE_R: f32 = 0.353_553_4;

/// `heart`: the deepest interior point's distance to the outline — the inradius
/// the normalization divides by.
///
/// **Exact, not fitted.** By symmetry the deepest point sits on the axis at
/// `(0, y)`, where the distance to the notch `(0, 1)` is `1 - y` and the
/// distance to the tangent ray `y = x` is `y / sqrt(2)`. Setting them equal
/// gives `y = 2 - sqrt(2)` and a common value of `sqrt(2) - 1`. A grid search
/// over the shape agrees to four decimals — see
/// [`the_heart_inradius_is_its_deepest_interior_point`](tests::the_heart_inradius_is_its_deepest_interior_point).
const HEART_INRADIUS: f32 = std::f32::consts::SQRT_2 - 1.0;

/// `heart`: sprite-local half-width the heart is drawn at, and the heart-space
/// height its centre sits at.
///
/// The unshifted figure spans `x` in `+-0.6036` and `y` in `0..1.1036` — wider
/// than it is tall and sitting entirely above the origin — so it is recentred on
/// `HEART_CY` and scaled so the sprite's `x = +-1` maps just outside its widest
/// point. That leaves the whole heart inside the quad with a small margin, which
/// matters because the quad is the only clip there is.
const HEART_SCALE: f32 = 0.65;
const HEART_CY: f32 = 0.552;

/// The two `[params]` names a scene gains by adopting this roster.
///
/// This is the single statement of the pair;
/// `emitter.rs`'s `both_particle_scenes_carry_the_same_shape_vocabulary` holds
/// both particle scenes' own `PARAMS` to it, so the two cannot drift into
/// different spellings of the same idea. Test-only because that guard is its
/// only reader: each scene lists its own vocabulary literally, which is what
/// `core/tests/preset.rs`'s source-scanning `set_param` drift guard requires.
#[cfg(test)]
pub(crate) const PARAMS: [&str; 5] = [
    "shape",
    "points",
    "star_valley",
    "star_curve",
    "star_jitter",
];

/// The `shape` index the shader is handed: clamped into the roster, then
/// **rounded to an integer**, with a non-finite binding falling back to the
/// default.
///
/// This is `kaleido_edge`'s treatment for `kaleido_edge`'s reason. A selector's
/// values are *identities* rather than a quantity, and `[smoothing]` and preset
/// dissolves interpolate a binding continuously from one setting to another — so
/// easing `disc` to `star` passes through 1.4 and 2.6, and without this the
/// shader would receive a value no arm defines. Rounding here rather than in
/// WGSL keeps that precondition visible on the CPU side, where the roster lives.
pub(crate) fn mark_shape(v: f32) -> f32 {
    if v.is_finite() {
        v.clamp(MIN_SHAPE, MAX_SHAPE).round()
    } else {
        DEFAULT_SHAPE
    }
}

/// The point count the shader is handed: clamped into `3..=12`, then **rounded
/// to an integer**, with a non-finite binding falling back to the default.
///
/// **This is the `kaleido_order` precedent, and it is the same mechanism, not
/// merely the same habit.** Both the polygon and the star arm fold the angle
/// with `a - seg * floor(a / seg)` where `seg = 2*pi / points` — a function
/// periodic in `seg`. `atan2`'s branch cut lies on the -x ray: crossing it, `a`
/// jumps by exactly `2*pi`, and a `seg`-periodic function absorbs that jump only
/// when `2*pi` is a whole multiple of `seg`, i.e. only when `points` is an
/// integer. At a fractional count the mark tears along one ray.
///
/// So an eased `points` **steps**. That is the opposite of what the surrounding
/// vocabulary teaches — `variant` interpolates (ADR-0060), the IFS morphs
/// (ADR-0075) — which is exactly why it is stated at the parameter rather than
/// assumed. A star's angle fold is periodic in the count: a fractional count is
/// a discontinuity, not an intermediate figure.
pub(crate) fn mark_points(v: f32) -> f32 {
    if v.is_finite() {
        v.clamp(MIN_POINTS, MAX_POINTS).round()
    } else {
        DEFAULT_POINTS
    }
}

/// The `star_valley` the shader is handed: held inside the range the arithmetic
/// needs, with a non-finite binding falling back to the default.
///
/// CPU-side, so the default reaches the uniform as **exactly** `0.45` and the
/// clamp can never be met with a NaN (WGSL's `clamp` is implementation-defined
/// there) — `ink::applied_gamma`'s two reasons, and the second matters more here
/// because the value divides.
pub(crate) fn star_valley(v: f32) -> f32 {
    if v.is_finite() {
        v.clamp(MIN_STAR_VALLEY, MAX_STAR_VALLEY)
    } else {
        DEFAULT_STAR_VALLEY
    }
}

/// The `star_curve` the shader is handed. Symmetric about 0, which is the
/// identity the straight-edge branch tests for.
pub(crate) fn star_curve(v: f32) -> f32 {
    if v.is_finite() {
        v.clamp(-MAX_STAR_CURVE, MAX_STAR_CURVE)
    } else {
        DEFAULT_STAR_CURVE
    }
}

/// The `star_jitter` the shader is handed. One-sided: it is an amplitude.
pub(crate) fn star_jitter(v: f32) -> f32 {
    if v.is_finite() {
        v.clamp(0.0, MAX_STAR_JITTER)
    } else {
        DEFAULT_STAR_JITTER
    }
}

/// The per-spike hash the jitter draws from — **integer arithmetic only**, so a
/// spike's length is bit-identical on every GPU and in every run.
///
/// This is a *pure function of the spike index*, not a draw from a stream, and
/// that distinction is deliberate: Plan 0077's `SeededRng` caution is that an
/// extra draw re-scatters everything downstream of it, and a hash has no
/// downstream to disturb. A `sin`-based hash would have been shorter and is
/// exactly what the determinism rule forbids — its low bits differ between
/// GPUs, so the same preset would draw a different figure on another machine.
///
/// Test-only on the Rust side: the shipped path evaluates this in WGSL, and the
/// mirror exists so the figure is assertable without a GPU (the arrangement
/// `mark_distance`'s own mirror uses).
#[cfg(test)]
pub(crate) fn spike_hash01(index: u32) -> f32 {
    let mut h = index.wrapping_mul(747_796_405).wrapping_add(2_891_336_453);
    h = ((h >> ((h >> 28) + 4)) ^ h).wrapping_mul(277_803_737);
    h = (h >> 22) ^ h;
    h as f32 * 2.328_306_4e-10
}

/// The shared distance-function chunk, with its constants substituted in.
///
/// Prepended to each particle scene's shader source, so both scenes evaluate the
/// **same** `mark_distance` and a roster change reaches both at once. It defines
/// no bindings and no entry points — it is arithmetic — so splicing it in
/// changes neither scene's bind-group layout (which on the DX12 WARP adapter is
/// not a free thing to change; see `emitter.rs`'s layout comment and ADR-0058).
pub(crate) fn sdf_wgsl() -> String {
    SDF_WGSL
        .replace("%RING_MID%", &format!("{RING_MID:?}"))
        .replace("%RING_HALF%", &format!("{RING_HALF:?}"))
        .replace("%STAR_SEGMENTS%", &format!("{STAR_SEGMENTS}"))
        .replace("%HEART_LOBE_R%", &format!("{HEART_LOBE_R:?}"))
        .replace("%HEART_INRADIUS%", &format!("{HEART_INRADIUS:?}"))
        .replace("%HEART_SCALE%", &format!("{HEART_SCALE:?}"))
        .replace("%HEART_CY%", &format!("{HEART_CY:?}"))
}

/// The chunk itself. `%NAME%` placeholders are substituted by [`sdf_wgsl`].
const SDF_WGSL: &str = r#"
const MARK_TAU: f32 = 6.28318530718;
const MARK_RING_MID: f32 = %RING_MID%;
const MARK_RING_HALF: f32 = %RING_HALF%;
const MARK_STAR_SEGMENTS: i32 = %STAR_SEGMENTS%;
const MARK_HEART_LOBE_R: f32 = %HEART_LOBE_R%;
const MARK_HEART_INRADIUS: f32 = %HEART_INRADIUS%;
const MARK_HEART_SCALE: f32 = %HEART_SCALE%;
const MARK_HEART_CY: f32 = %HEART_CY%;

// The per-spike hash `star_jitter` draws from. INTEGER arithmetic only: a
// sin-based hash is shorter and its low bits differ between GPUs, which would
// make the same preset draw a different figure on another machine. Mirrored
// verbatim by `spike_hash01` on the Rust side.
fn mark_spike_hash01(index: u32) -> f32 {
    var h = index * 747796405u + 2891336453u;
    h = ((h >> ((h >> 28u) + 4u)) ^ h) * 277803737u;
    h = (h >> 22u) ^ h;
    return f32(h) * 2.3283064e-10;
}

// Inigo Quilez's heart, in its own frame: two lobe circles centred
// (+-0.25, 0.75), closed underneath by the 45-degree rays from the origin that
// are tangent to them, notched at (0, 1).
fn mark_heart_sd(p_in: vec2<f32>) -> f32 {
    let p = vec2<f32>(abs(p_in.x), p_in.y);
    if (p.y + p.x > 1.0) {
        return length(p - vec2<f32>(0.25, 0.75)) - MARK_HEART_LOBE_R;
    }
    let a = p - vec2<f32>(0.0, 1.0);
    let m = 0.5 * max(p.x + p.y, 0.0);
    let b = p - vec2<f32>(m, m);
    return sqrt(min(dot(a, a), dot(b, b))) * sign(p.x - p.y);
}

// Normalized distance from a mark's silhouette: 0 at the shape's deepest
// interior point, exactly 1 on its outline, greater than 1 outside it. The
// caller's falloff is unchanged, so everything at d >= 1 is black and only the
// interior has to be right.
//
// `shape`, `points` and `star` are per-draw values, identical for every fragment
// of every sprite in the draw, so every branch here is uniform across a warp —
// including the star arm's straight/curved split, which is what makes the
// neutral configuration cost nothing extra.
//
// `star` is `vec3(valley, curve, jitter)`, all three conditioned CPU-side.
fn mark_distance(p: vec2<f32>, shape: f32, points: f32, star: vec3<f32>) -> f32 {
    if (shape < 0.5) {
        // disc: sd = length(p) - 1, R = 1. The three lines this replaced.
        return length(p);
    }
    if (shape < 1.5) {
        // ring: sd = abs(length(p) - mid) - half, R = half.
        return abs(length(p) - MARK_RING_MID) / MARK_RING_HALF;
    }
    if (shape < 2.5) {
        // regular polygon, circumradius 1, one vertex on +x. Fold the angle into
        // a wedge and measure against that wedge's edge line; R is the apothem.
        let seg = MARK_TAU / points;
        let h = 0.5 * seg;
        let a = atan2(p.y, p.x);
        let f = a - seg * floor(a / seg) - h;
        let r = length(p);
        let apothem = cos(h);
        let d_line = r * cos(f) / apothem;
        // Inside a CONVEX polygon the nearest boundary is always an edge, never
        // a vertex, so the line above is already the exact distance — and it is
        // the expression the sprite has evaluated since ADR-0084, bit for bit.
        if (d_line <= 1.0) {
            return d_line;
        }
        // Outside it is not: past a vertex the nearest boundary is that vertex,
        // and an infinite edge line measures straight past it (Plan 0091 Phase 2
        // measured 0.326 of a sprite half-width at the worst sample). Clamp
        // along the edge instead — folded, it is the segment x = apothem,
        // |y| <= sin(h).
        let q = vec2<f32>(r * cos(f), abs(r * sin(f)));
        let past_vertex = max(q.y - sin(h), 0.0);
        return 1.0 + length(vec2<f32>(q.x - apothem, past_vertex)) / apothem;
    }
    if (shape < 3.5) {
        // n-pointed star: `points` spikes at tip radius 1 on +x, valleys at
        // `star.x` of it. Fold the angle into a half-wedge so f = 0 at a tip and
        // f = h at a valley; that fold also names WHICH spike, which is what the
        // per-spike jitter needs.
        let seg = MARK_TAU / points;
        let h = 0.5 * seg;
        let a = atan2(p.y, p.x);
        let spike = floor((a + h) / seg);
        let f = abs(a - seg * spike);
        let r = length(p);
        let k = star.x;
        let curve = star.y;
        let jitter = star.z;
        // The reference inradius: the perpendicular from the origin to the
        // STRAIGHT, unjittered edge. Held fixed across all three params, so `d`
        // stays exactly 1 on the outline whatever they do (the signed distance
        // is 0 there either way) and the neutral configuration is bit-for-bit
        // the arithmetic that shipped.
        let b = (1.0 - k * cos(h)) / (k * sin(h));

        if (curve == 0.0 && jitter == 0.0) {
            // The straight-edge closed form. Writing the edge's plane as n.p = c
            // and dividing through by c leaves one multiply-add: the
            // normalization's 1/R and the normal's length cancel.
            //
            // Written out rather than reusing `b` above, and that is not
            // redundancy: `x * (A / B)` and `x * A / B` associate differently
            // and disagree in the last bit. This spelling is the one every
            // shipped `shape = "3"` mark has evaluated since ADR-0084, and
            // `the_star_params_are_clamped_and_their_defaults_are_exact`
            // asserts it bit for bit — it caught exactly this substitution.
            let d_line = r * cos(f) + r * sin(f) * (1.0 - k * cos(h)) / (k * sin(h));
            // The interior arm is deliberately still APPROXIMATE — it is the
            // plane, not the figure, and Plan 0091 Phase 2 measured it 0.066 out
            // at 5 points and 0.248 at 12. It stays because the sprite reads
            // only here and every shipped `shape = "3"` mark is this arithmetic.
            if (d_line <= 1.0) {
                return d_line;
            }
            // Outside, the fold has already picked the one edge that can be
            // nearest, so the exact distance is to that edge as a SEGMENT: the
            // clamp is what makes a point past a tip measure to the tip.
            let inradius = inverseSqrt(1.0 + b * b);
            let q = vec2<f32>(r * cos(f), r * sin(f));
            let tip = vec2<f32>(1.0, 0.0);
            let valley = vec2<f32>(k * cos(h), k * sin(h));
            let edge = valley - tip;
            let t = clamp(dot(q - tip, edge) / dot(edge, edge), 0.0, 1.0);
            return 1.0 + length(q - (tip + t * edge)) / inradius;
        }

        // Curved and/or jittered. The edge becomes a quadratic Bezier and the
        // exact distance to one is a cubic solve, so it is SAMPLED into
        // MARK_STAR_SEGMENTS sub-segments and measured against the polyline —
        // the same trade this project's own ground-truth harness makes. The
        // residual is the polyline's sagitta and `marks/tests.rs` measures it.
        let n = max(points, 1.0);
        let index = u32(max(spike - floor(spike / n) * n, 0.0));
        // Symmetric about the unjittered radius, so the figure keeps its size
        // rather than only ever shrinking.
        let rt = 1.0 + jitter * (mark_spike_hash01(index) * 2.0 - 1.0);
        let tip = vec2<f32>(rt, 0.0);
        let valley = vec2<f32>(k * cos(h), k * sin(h));
        // The control point is the edge's midpoint pulled toward the origin, so
        // POSITIVE `curve` bows the edge inward — the concave sparkle a
        // straight-edged star provably cannot make at any valley radius — and
        // negative bows it out.
        let ctrl = 0.5 * (tip + valley) * (1.0 - curve);
        let u = vec2<f32>(cos(f), sin(f));
        let q = r * u;
        var nearest = 1e9;
        // Where the ray from the origin along `u` crosses the boundary. The
        // region is star-shaped about the origin, so exactly one sub-segment
        // spans this angle, and comparing radii is the inside test — found in
        // the same loop as the distance rather than in a second pass.
        var boundary_r = 0.0;
        var prev = tip;
        for (var i = 1; i <= MARK_STAR_SEGMENTS; i = i + 1) {
            let t = f32(i) / f32(MARK_STAR_SEGMENTS);
            let s = 1.0 - t;
            let cur = s * s * tip + 2.0 * s * t * ctrl + t * t * valley;
            let e = cur - prev;
            let w = q - prev;
            let along = clamp(dot(w, e) / max(dot(e, e), 1e-12), 0.0, 1.0);
            nearest = min(nearest, length(w - along * e));
            let denom = u.x * e.y - u.y * e.x;
            if (abs(denom) > 1e-9) {
                let ts = -(u.x * prev.y - u.y * prev.x) / denom;
                if (ts >= 0.0 && ts <= 1.0) {
                    boundary_r = dot(prev + ts * e, u);
                }
            }
            prev = cur;
        }
        let inradius = inverseSqrt(1.0 + b * b);
        let sd = select(nearest, -nearest, r < boundary_r);
        return 1.0 + sd / inradius;
    }
    // heart: recentred and scaled into the sprite quad. The scale cancels out of
    // 1 + sd/R, so the inradius below is the unscaled figure's.
    let q = p * MARK_HEART_SCALE + vec2<f32>(0.0, MARK_HEART_CY);
    return 1.0 + mark_heart_sd(q) / MARK_HEART_INRADIUS;
}
"#;

// Crate-visible under `cfg(test)` only: `shape_field`'s contour test needs the
// numerically-sampled outline this module's tests already build, and a second
// copy of a ground truth is a ground truth that can disagree with itself.
#[cfg(test)]
pub(crate) mod tests;
