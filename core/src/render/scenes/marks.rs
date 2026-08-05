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
//! - **Only the interior matters.** The falloff downstream is unchanged —
//!   `g = max(0, 1 - d)^2` — so every fragment at `d >= 1` is black. The lit
//!   region *is* the silhouette, and a distance function only has to be right
//!   inside it. That is what makes the polygon and star arms two cheap lines
//!   rather than an exact convex-polygon SDF.
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
        let a = atan2(p.y, p.x);
        let f = a - seg * floor(a / seg) - 0.5 * seg;
        return length(p) * cos(f) / cos(0.5 * seg);
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
        return r * cos(f) + r * sin(f) * (1.0 - k * cos(h)) / (k * sin(h));
    }
    // heart: recentred and scaled into the sprite quad. The scale cancels out of
    // 1 + sd/R, so the inradius below is the unscaled figure's.
    let q = p * MARK_HEART_SCALE + vec2<f32>(0.0, MARK_HEART_CY);
    return 1.0 + mark_heart_sd(q) / MARK_HEART_INRADIUS;
}
"#;

#[cfg(test)]
mod tests {
    #![allow(clippy::indexing_slicing, clippy::panic, clippy::expect_used)]

    use super::{
        DEFAULT_POINTS, DEFAULT_SHAPE, HEART_CY, HEART_INRADIUS, HEART_LOBE_R, HEART_SCALE,
        MAX_POINTS, MAX_SHAPE, MIN_POINTS, PARAMS, RING_HALF, RING_MID, SHAPES, STAR_INNER,
        mark_points, mark_shape, sdf_wgsl,
    };

    /// Roster indices, by name, so the tests below read as shapes.
    pub(super) const DISC: f32 = 0.0;
    pub(super) const RING: f32 = 1.0;
    pub(super) const POLYGON: f32 = 2.0;
    pub(super) const STAR: f32 = 3.0;
    pub(super) const HEART: f32 = 4.0;

    /// **The CPU mirror of the WGSL above.** Kept identical by inspection, the
    /// same arrangement `kaleidoscope.rs`'s `edge_sample_radius` uses and for the
    /// same reason: it lets the arithmetic properties be asserted directly rather
    /// than argued, while the pixel-level claims stay on the shader itself (see
    /// `swarm.rs`'s seven-maxima capture, which renders the real pipeline).
    pub(super) fn mark_distance(p: [f32; 2], shape: f32, points: f32) -> f32 {
        let len = (p[0] * p[0] + p[1] * p[1]).sqrt();
        if shape < 0.5 {
            return len;
        }
        if shape < 1.5 {
            return (len - RING_MID).abs() / RING_HALF;
        }
        if shape < 2.5 {
            let seg = std::f32::consts::TAU / points;
            let a = p[1].atan2(p[0]);
            let f = a - seg * (a / seg).floor() - 0.5 * seg;
            return len * f.cos() / (0.5 * seg).cos();
        }
        if shape < 3.5 {
            let seg = std::f32::consts::TAU / points;
            let h = 0.5 * seg;
            let a = p[1].atan2(p[0]);
            let f = (a - seg * ((a + h) / seg).floor()).abs();
            let k = STAR_INNER;
            return len * f.cos() + len * f.sin() * (1.0 - k * h.cos()) / (k * h.sin());
        }
        let q = [p[0] * HEART_SCALE, p[1] * HEART_SCALE + HEART_CY];
        1.0 + heart_sd(q) / HEART_INRADIUS
    }

    /// The CPU mirror of `mark_heart_sd`.
    pub(super) fn heart_sd(p_in: [f32; 2]) -> f32 {
        let p = [p_in[0].abs(), p_in[1]];
        if p[1] + p[0] > 1.0 {
            return ((p[0] - 0.25).powi(2) + (p[1] - 0.75).powi(2)).sqrt() - HEART_LOBE_R;
        }
        let a2 = p[0] * p[0] + (p[1] - 1.0).powi(2);
        let m = 0.5 * (p[0] + p[1]).max(0.0);
        let b2 = (p[0] - m).powi(2) + (p[1] - m).powi(2);
        a2.min(b2).sqrt() * (p[0] - p[1]).signum()
    }

    /// **`disc` is `length(p)`, exactly** — the claim every untouched golden
    /// baseline rests on.
    ///
    /// Asserted as bit equality rather than within a tolerance, because the
    /// property is exact: the disc arm is not an SDF specialized to a circle, it
    /// is the same expression the shader held before the roster existed.
    #[test]
    fn the_disc_arm_is_bit_identical_to_the_length_it_replaces() {
        for i in 0..64 {
            for j in 0..64 {
                let p = [i as f32 / 31.5 - 1.0, j as f32 / 31.5 - 1.0];
                let want = (p[0] * p[0] + p[1] * p[1]).sqrt();
                for points in [3.0, 5.0, 7.0, 12.0] {
                    let got = mark_distance(p, DISC, points);
                    assert_eq!(
                        got.to_bits(),
                        want.to_bits(),
                        "disc at {p:?} with points={points} must be length(p): {got} vs {want}"
                    );
                }
            }
        }
    }

    /// Every shape is `0` at its own deepest interior point and exactly `1` on
    /// its outline — the normalization the whole roster shares.
    ///
    /// The tips are the boundary points a closed form is available for, so those
    /// are what this pins: the disc's rim, the ring's two edges, the polygon's
    /// vertex, the star's tip and valley, and the heart's notch and bottom cusp.
    #[test]
    fn every_shape_is_one_on_its_outline_and_zero_at_its_core() {
        let n = 7.0;
        let seg = std::f32::consts::TAU / n;
        let cases: [(f32, [f32; 2], f32); 8] = [
            // (shape, point, expected d)
            (DISC, [0.0, 0.0], 0.0),
            (DISC, [1.0, 0.0], 1.0),
            (RING, [RING_MID, 0.0], 0.0),
            (RING, [RING_MID + RING_HALF, 0.0], 1.0),
            (POLYGON, [0.0, 0.0], 0.0),
            // A polygon vertex sits on +x at radius 1 (the fold's wedge edge).
            (POLYGON, [1.0, 0.0], 1.0),
            (STAR, [0.0, 0.0], 0.0),
            (STAR, [1.0, 0.0], 1.0),
        ];
        for (shape, p, want) in cases {
            let got = mark_distance(p, shape, n);
            assert!(
                (got - want).abs() < 1e-5,
                "shape {} at {p:?}: d = {got}, want {want}",
                SHAPES[shape as usize]
            );
        }

        // The star's valley: radius STAR_INNER at half a wedge round.
        let valley = [
            STAR_INNER * (0.5 * seg).cos(),
            STAR_INNER * (0.5 * seg).sin(),
        ];
        let got = mark_distance(valley, STAR, n);
        assert!(
            (got - 1.0).abs() < 1e-5,
            "the star's valley is on its outline: d = {got}"
        );

        // The heart's notch (0, 1) and bottom cusp (0, 0) in heart space, mapped
        // back into the sprite frame.
        for heart_space_y in [0.0f32, 1.0] {
            let p = [0.0, (heart_space_y - HEART_CY) / HEART_SCALE];
            let got = mark_distance(p, HEART, n);
            assert!(
                (got - 1.0).abs() < 1e-4,
                "the heart's outline at heart-space y={heart_space_y}: d = {got}"
            );
        }
        // ...and its deepest interior point is the zero.
        let deepest = [
            0.0,
            (2.0 - std::f32::consts::SQRT_2 - HEART_CY) / HEART_SCALE,
        ];
        let got = mark_distance(deepest, HEART, n);
        assert!(
            got.abs() < 1e-4,
            "the heart's deepest interior point is its zero: d = {got}"
        );
    }

    /// The heart's inradius is the deepest interior point's distance to the
    /// outline — **measured**, not assumed, because it is the one constant in
    /// this file that the normalization would silently mis-scale.
    ///
    /// A grid search over the figure against the closed form `sqrt(2) - 1`. If
    /// they disagreed, `d` would not reach 0 inside the heart (too large) or
    /// would go negative (too small), and either way the falloff would be
    /// measuring from the wrong place.
    #[test]
    fn the_heart_inradius_is_its_deepest_interior_point() {
        let mut deepest = 0.0f32;
        let n = 900;
        for i in 0..=n {
            let x = -1.0 + 2.0 * i as f32 / n as f32;
            for j in 0..=n {
                let y = -0.5 + 2.0 * j as f32 / n as f32;
                deepest = deepest.min(heart_sd([x, y]));
            }
        }
        assert!(
            (-deepest - HEART_INRADIUS).abs() < 5e-3,
            "grid search found a depth of {:.5}, closed form says {HEART_INRADIUS:.5}",
            -deepest
        );
    }

    /// **Non-vacuity for the whole roster**: no two shapes draw the same figure.
    ///
    /// Compared as *coverage* (`g = max(0, 1 - d)^2`, the value the shader
    /// actually emits) rather than as a lit/unlit mask, because the mask is the
    /// weaker statement — a ring and a disc light nearly the same pixels and
    /// differ entirely in where they are bright.
    ///
    /// The bar is dimensionless: each pair must differ, on average, by more than
    /// 5 % of the disc's own mean coverage. It is pure CPU arithmetic, so this is
    /// a property and not a machine measurement (ADR-0071). The closest pair is
    /// **disc against a 7-gon at 0.129**, 2.6x the bar — and it is close because
    /// a seven-sided polygon genuinely is nearly a circle, which is a fact about
    /// polygons rather than a defect. Two roster entries that evaluated
    /// identically would read 0.
    #[test]
    fn no_two_shapes_draw_the_same_figure() {
        const N: usize = 96;
        let n = 7.0;
        let grid: Vec<[f32; 2]> = (0..N)
            .flat_map(|i| {
                (0..N).map(move |j| {
                    [
                        i as f32 / (N as f32 - 1.0) * 2.0 - 1.0,
                        j as f32 / (N as f32 - 1.0) * 2.0 - 1.0,
                    ]
                })
            })
            .collect();
        let coverage = |shape: f32| -> Vec<f32> {
            grid.iter()
                .map(|&p| (1.0 - mark_distance(p, shape, n)).max(0.0).powi(2))
                .collect()
        };
        let fields: Vec<(f32, Vec<f32>)> = [DISC, RING, POLYGON, STAR, HEART]
            .into_iter()
            .map(|s| (s, coverage(s)))
            .collect();
        let mean = |v: &[f32]| v.iter().sum::<f32>() / v.len() as f32;
        let disc_mean = mean(&fields[0].1);
        assert!(disc_mean > 0.0, "the disc must light something");

        for (shape, field) in &fields {
            let lit = field.iter().filter(|&&g| g > 0.0).count();
            assert!(
                lit * 8 > grid.len() && lit < grid.len(),
                "{} lights {lit} of {} samples — a shape that covers almost nothing \
                 (or the whole quad) is not a silhouette",
                SHAPES[*shape as usize],
                grid.len()
            );
        }
        let mut closest = f32::INFINITY;
        for (i, (a, fa)) in fields.iter().enumerate() {
            for (b, fb) in fields.iter().skip(i + 1) {
                let diff: f32 = fa
                    .iter()
                    .zip(fb.iter())
                    .map(|(x, y)| (x - y).abs())
                    .sum::<f32>()
                    / fa.len() as f32;
                let ratio = diff / disc_mean;
                closest = closest.min(ratio);
                assert!(
                    ratio > 0.05,
                    "{} and {} differ by only {ratio:.3} of the disc's mean coverage",
                    SHAPES[*a as usize],
                    SHAPES[*b as usize],
                );
            }
        }
        eprintln!("closest pair in the shape roster: {closest:.3} of the disc's mean coverage");
    }

    /// The roster is closed at both ends and a broken binding lands on the
    /// default rather than on a bound (`kaleido_edge`'s rule: a selector has no
    /// "as far as you can go" reading).
    #[test]
    fn the_shape_selector_clamps_rounds_and_falls_back() {
        assert_eq!(mark_shape(-3.0), 0.0);
        assert_eq!(mark_shape(99.0), MAX_SHAPE);
        assert_eq!(mark_shape(2.4), 2.0);
        assert_eq!(mark_shape(2.6), 3.0);
        assert_eq!(mark_shape(f32::NAN), DEFAULT_SHAPE);
        assert_eq!(mark_shape(f32::INFINITY), DEFAULT_SHAPE);
        // Every index the roster names survives the quantizer unchanged.
        for (i, _) in SHAPES.iter().enumerate() {
            assert_eq!(mark_shape(i as f32), i as f32);
        }
    }

    /// **An eased `points` steps: it visits only whole counts, never a partial
    /// lobe** (Plan 0070 Phase 3's done-when).
    ///
    /// The stated behaviour is the claim, so it is asserted as a set rather than
    /// as a tolerance: a `[smoothing]`-eased sweep from 7 to 9 — 400 samples, the
    /// shape an ease actually produces — reaches the shader as exactly the three
    /// figures `{7, 8, 9}` and nothing between them.
    ///
    /// The second half is what stops this from being a tautology about `round`:
    /// the *raw* sweep genuinely takes hundreds of distinct fractional values, so
    /// the quantizer is doing work rather than being handed integers.
    #[test]
    fn an_eased_points_sweep_visits_only_whole_counts() {
        const SAMPLES: usize = 400;
        let raw: Vec<f32> = (0..=SAMPLES)
            .map(|i| {
                // An exponential ease from 7 toward 9, which is the shape
                // `[smoothing]` produces (a one-pole per frame).
                let t = 1.0 - (1.0 - 1.0 / 40.0f32).powi(i as i32);
                7.0 + 2.0 * t
            })
            .collect();

        let mut distinct_raw: Vec<u32> = raw.iter().map(|v| v.to_bits()).collect();
        distinct_raw.sort_unstable();
        distinct_raw.dedup();
        assert!(
            distinct_raw.len() > 100,
            "the raw sweep must genuinely be continuous, or the quantizer is being \
             handed integers and this proves nothing: {} distinct values",
            distinct_raw.len()
        );

        let mut seen: Vec<f32> = raw.iter().map(|&v| mark_points(v)).collect();
        seen.sort_by(f32::total_cmp);
        seen.dedup();
        assert_eq!(
            seen,
            vec![7.0, 8.0, 9.0],
            "an eased 7 -> 9 sweep must render exactly the figures at 7, 8 and 9"
        );

        // ...and nothing the quantizer emits is ever fractional, anywhere in
        // range, so no fold can receive a count that tears the mark.
        for i in 0..=2000 {
            let v = MIN_POINTS - 2.0 + (MAX_POINTS + 4.0 - MIN_POINTS) * i as f32 / 2000.0;
            let q = mark_points(v);
            assert_eq!(q, q.round(), "points quantized to {q} from {v}");
            assert!(
                (MIN_POINTS..=MAX_POINTS).contains(&q),
                "points {q} out of range"
            );
        }
        assert_eq!(mark_points(f32::NAN), DEFAULT_POINTS);
    }

    /// The roster is one list in one place, and these are its values.
    ///
    /// Pinned so a silent reorder is a failing test rather than a shipped preset
    /// that quietly changed meaning: `shape` is a numeric selector, so an index
    /// that moves is a look that moves. The cross-scene half of this claim — that
    /// both particle scenes accept exactly these values — lives in `emitter.rs`,
    /// which is the file that can see both.
    #[test]
    fn the_shape_roster_is_pinned() {
        assert_eq!(SHAPES, ["disc", "ring", "polygon", "star", "heart"]);
        assert_eq!(MAX_SHAPE, 4.0);
        assert_eq!(DEFAULT_SHAPE, 0.0);
        assert_eq!(DEFAULT_POINTS, 5.0);
        assert_eq!(PARAMS, ["shape", "points"]);
    }

    /// The templated chunk carries no unsubstituted placeholder — a `%NAME%` that
    /// survived would be a WGSL parse error at pipeline build, i.e. a panic on a
    /// preset switch rather than a test failure.
    #[test]
    fn the_shader_chunk_substitutes_every_placeholder() {
        let wgsl = sdf_wgsl();
        assert!(
            !wgsl.contains('%'),
            "unsubstituted placeholder in the mark SDF chunk:\n{wgsl}"
        );
        assert!(wgsl.contains("fn mark_distance("));
    }
}
