//! **A particle mark's silhouette, as a signed-distance function**
//! ([ADR-0084](../../../../docs/adrs/0084-a-particle-marks-silhouette-is-a-signed-distance-function.md),
//! Plan 0070).
//!
//! Every mark this engine drew before Plan 0070 was a round additive blob. The
//! swarm's fragment shader was three lines —
//! `let d = length(in.local); let falloff = max(0.0, 1.0 - d); let g = falloff * falloff;`
//! — with no shape input at all, and the emitter's sprite was the same idea with
//! one axis scaled. This module is the shape vocabulary those two scenes share:
//! one WGSL chunk, one roster, one quantizer, so the two cannot drift.
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
//!   *literally* the line it replaces, not an approximation of it. That is why
//!   every pre-existing golden baseline is byte-identical: no shipped preset
//!   names a shape, so they all take this arm.
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

/// `star`: the inner (valley) radius as a fraction of the outer (tip) one.
///
/// Chosen for the size the ask is *for* — small marks, a few pixels across.
/// Below about a third the spikes are thinner than a pixel at those sizes and
/// the mark reads as a dot with a halo; above about a half it reads as a
/// slightly bumpy polygon. At 0.45 a seven-pointed star still has seven legible
/// points at a dozen pixels across.
const STAR_INNER: f32 = 0.45;

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
pub(crate) const PARAMS: [&str; 2] = ["shape", "points"];

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
/// vocabulary teaches — `variant` interpolates
/// ([ADR-0060](../../../../docs/adrs/0060-star-pattern-variants-interpolate.md)),
/// the IFS morphs
/// ([ADR-0075](../../../../docs/adrs/0075-ifs-family-morphs-in-singular-value-space.md))
/// — which is exactly why it is stated at the parameter rather than assumed.
/// A star's angle fold is periodic in the count: a fractional count is a
/// discontinuity, not an intermediate figure.
pub(crate) fn mark_points(v: f32) -> f32 {
    if v.is_finite() {
        v.clamp(MIN_POINTS, MAX_POINTS).round()
    } else {
        DEFAULT_POINTS
    }
}

/// The shared distance-function chunk, with its constants substituted in.
///
/// Prepended to each particle scene's shader source, so both scenes evaluate the
/// **same** `mark_distance` and a roster change reaches both at once. It defines
/// no bindings and no entry points — it is arithmetic — so splicing it in
/// changes neither scene's bind-group layout (which on the DX12 WARP adapter is
/// not a free thing to change; see `emitter.rs`'s layout comment and
/// [ADR-0058](../../../../docs/adrs/0058-bind-group-layout-collisions-carry-evidence.md)).
pub(crate) fn sdf_wgsl() -> String {
    SDF_WGSL
        .replace("%RING_MID%", &format!("{RING_MID:?}"))
        .replace("%RING_HALF%", &format!("{RING_HALF:?}"))
        .replace("%STAR_INNER%", &format!("{STAR_INNER:?}"))
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
const MARK_STAR_INNER: f32 = %STAR_INNER%;
const MARK_HEART_LOBE_R: f32 = %HEART_LOBE_R%;
const MARK_HEART_INRADIUS: f32 = %HEART_INRADIUS%;
const MARK_HEART_SCALE: f32 = %HEART_SCALE%;
const MARK_HEART_CY: f32 = %HEART_CY%;

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
// `shape` and `points` are per-draw values, identical for every fragment of
// every sprite in the draw, so this branch is uniform across a warp.
fn mark_distance(p: vec2<f32>, shape: f32, points: f32) -> f32 {
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
        // n-pointed star, tip radius 1 on +x, valley radius MARK_STAR_INNER.
        // Fold the angle into a half-wedge so f = 0 at a tip and f = h at a
        // valley, then measure against the straight edge joining them. Writing
        // that edge's plane as n.p = c and dividing through by c leaves one
        // multiply-add: the normalization's 1/R and the normal's length cancel.
        let seg = MARK_TAU / points;
        let h = 0.5 * seg;
        let a = atan2(p.y, p.x);
        let f = abs(a - seg * floor((a + h) / seg));
        let r = length(p);
        let k = MARK_STAR_INNER;
        let d_line = r * cos(f) + r * sin(f) * (1.0 - k * cos(h)) / (k * sin(h));
        // The interior arm is unchanged and deliberately still APPROXIMATE — it
        // is the plane, not the figure, and Plan 0091 Phase 2 measured it 0.066
        // out at the worst interior sample. It stays because the sprite reads
        // only here and every shipped `shape = 3` mark is this arithmetic.
        if (d_line <= 1.0) {
            return d_line;
        }
        // Outside, the half-wedge fold has already picked the one edge that can
        // be nearest, so the exact distance is to that edge as a SEGMENT: the
        // clamp is what makes a point past a tip measure to the tip. Unrepaired
        // this arm was 1.057 out — over half a sprite half-width.
        let b = (1.0 - k * cos(h)) / (k * sin(h));
        let inradius = inverseSqrt(1.0 + b * b);
        let q = vec2<f32>(r * cos(f), r * sin(f));
        let tip = vec2<f32>(1.0, 0.0);
        let valley = vec2<f32>(k * cos(h), k * sin(h));
        let edge = valley - tip;
        let t = clamp(dot(q - tip, edge) / dot(edge, edge), 0.0, 1.0);
        return 1.0 + length(q - (tip + t * edge)) / inradius;
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
