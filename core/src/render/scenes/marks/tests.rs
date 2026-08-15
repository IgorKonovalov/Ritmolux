#![allow(clippy::indexing_slicing, clippy::panic, clippy::expect_used)]

use super::{
    DEFAULT_POINTS, DEFAULT_SHAPE, HEART_CY, HEART_INRADIUS, HEART_LOBE_R, HEART_SCALE, MAX_POINTS,
    MAX_SHAPE, MIN_POINTS, PARAMS, RING_HALF, RING_MID, SHAPES, STAR_INNER, mark_points,
    mark_shape, sdf_wgsl,
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
        let h = 0.5 * seg;
        let a = p[1].atan2(p[0]);
        let f = a - seg * (a / seg).floor() - h;
        let apothem = h.cos();
        let d_line = len * f.cos() / apothem;
        if d_line <= 1.0 {
            return d_line;
        }
        let q = [len * f.cos(), (len * f.sin()).abs()];
        let past_vertex = (q[1] - h.sin()).max(0.0);
        let dx = q[0] - apothem;
        return 1.0 + (dx * dx + past_vertex * past_vertex).sqrt() / apothem;
    }
    if shape < 3.5 {
        let seg = std::f32::consts::TAU / points;
        let h = 0.5 * seg;
        let a = p[1].atan2(p[0]);
        let f = (a - seg * ((a + h) / seg).floor()).abs();
        let k = STAR_INNER;
        let d_line = len * f.cos() + len * f.sin() * (1.0 - k * h.cos()) / (k * h.sin());
        if d_line <= 1.0 {
            return d_line;
        }
        let b = (1.0 - k * h.cos()) / (k * h.sin());
        let inradius = 1.0 / (1.0 + b * b).sqrt();
        let q = [len * f.cos(), len * f.sin()];
        let tip = [1.0f32, 0.0];
        let valley = [k * h.cos(), k * h.sin()];
        let edge = [valley[0] - tip[0], valley[1] - tip[1]];
        let denom = edge[0] * edge[0] + edge[1] * edge[1];
        let t = (((q[0] - tip[0]) * edge[0] + (q[1] - tip[1]) * edge[1]) / denom).clamp(0.0, 1.0);
        let near = [tip[0] + t * edge[0], tip[1] + t * edge[1]];
        let dx = q[0] - near[0];
        let dy = q[1] - near[1];
        return 1.0 + (dx * dx + dy * dy).sqrt() / inradius;
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

// --- The exterior contract (Plan 0091 Phase 2) ---------------------------------
//
// Everything above this line grades the roster on the contract it was built
// for: `d` is 0 at the deepest interior point, 1 on the outline, and the
// falloff blacks out everything past it. `shape_field` reads the region past
// it, so the arms now owe a distance out there too — and the module docs say
// plainly that two of them were never built to give one.
//
// The ground truth is a **dense boundary polyline per arm**, built from each
// shape's own defining geometry and never from `mark_distance`, so this
// measures the roster against the figure rather than against itself. Distance
// is point-to-*segment* (exact for a polyline, so only the polyline's own
// sagitta enters — under 3e-6 at these sample counts), and inside/outside is an
// even-odd crossing count over the same segments, which is what lets `ring`
// work with no special case: a point in the hole crosses two rims and reads
// outside.

/// A closed loop of boundary points, in sprite-local coordinates.
type Loop = Vec<[f32; 2]>;

/// How finely a curved boundary is sampled.
const BOUNDARY_SAMPLES: usize = 1440;

/// The arm's outline, as one or more closed loops, built from the shape's
/// **definition**.
fn boundary_loops(shape: f32, points: f32) -> Vec<Loop> {
    let n = points as usize;
    let circle = |r: f32| -> Loop {
        (0..BOUNDARY_SAMPLES)
            .map(|i| {
                let a = std::f32::consts::TAU * i as f32 / BOUNDARY_SAMPLES as f32;
                [r * a.cos(), r * a.sin()]
            })
            .collect()
    };
    if shape < 0.5 {
        return vec![circle(1.0)];
    }
    if shape < 1.5 {
        // The annulus: two rims, at RING_MID +- RING_HALF.
        return vec![circle(RING_MID + RING_HALF), circle(RING_MID - RING_HALF)];
    }
    if shape < 2.5 {
        // Regular n-gon, circumradius 1, one vertex on +x. Straight edges, so
        // the vertices alone are an exact polyline.
        let seg = std::f32::consts::TAU / points;
        return vec![
            (0..n)
                .map(|k| {
                    let a = seg * k as f32;
                    [a.cos(), a.sin()]
                })
                .collect(),
        ];
    }
    if shape < 3.5 {
        // n-pointed star: 2n vertices alternating tip radius 1 at k*seg with
        // valley radius STAR_INNER at the half-step between.
        let seg = std::f32::consts::TAU / points;
        return vec![
            (0..2 * n)
                .map(|k| {
                    let a = 0.5 * seg * k as f32;
                    let r = if k % 2 == 0 { 1.0 } else { STAR_INNER };
                    [r * a.cos(), r * a.sin()]
                })
                .collect(),
        ];
    }
    // The heart, in ITS OWN frame first, then mapped back to sprite-local.
    // Right half, walked from the bottom cusp: the 45-degree ray out to the
    // tangent point (0.5, 0.5), then the lobe's outer semicircle round to the
    // notch at (0, 1). Mirrored for the left half.
    let arc_steps = BOUNDARY_SAMPLES / 4;
    let ray_steps = BOUNDARY_SAMPLES / 8;
    let mut heart: Loop = Vec::new();
    for i in 0..=ray_steps {
        let t = 0.5 * i as f32 / ray_steps as f32;
        heart.push([t, t]);
    }
    // The lobe centred (0.25, 0.75): from the tangent point at -45 degrees,
    // round the outside, to the notch at +135.
    for i in 1..=arc_steps {
        let a = -std::f32::consts::FRAC_PI_4 + std::f32::consts::PI * i as f32 / arc_steps as f32;
        heart.push([0.25 + HEART_LOBE_R * a.cos(), 0.75 + HEART_LOBE_R * a.sin()]);
    }
    // ...and back down the mirrored half, skipping the shared notch and cusp so
    // the loop does not double a vertex.
    let mirrored: Loop = heart
        .iter()
        .rev()
        .skip(1)
        .take(heart.len().saturating_sub(2))
        .map(|p| [-p[0], p[1]])
        .collect();
    heart.extend(mirrored);
    // Heart space -> sprite-local: q = p * HEART_SCALE + (0, HEART_CY).
    vec![
        heart
            .into_iter()
            .map(|q| [q[0] / HEART_SCALE, (q[1] - HEART_CY) / HEART_SCALE])
            .collect(),
    ]
}

/// Distance from `p` to the segment `a`-`b`. Exact, so the polyline's own
/// resolution is the only approximation in the ground truth.
fn point_segment_distance(p: [f32; 2], a: [f32; 2], b: [f32; 2]) -> f32 {
    let (ab, ap) = ([b[0] - a[0], b[1] - a[1]], [p[0] - a[0], p[1] - a[1]]);
    let denom = ab[0] * ab[0] + ab[1] * ab[1];
    let t = if denom > 0.0 {
        ((ap[0] * ab[0] + ap[1] * ab[1]) / denom).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let d = [ap[0] - t * ab[0], ap[1] - t * ab[1]];
    (d[0] * d[0] + d[1] * d[1]).sqrt()
}

/// The ground-truth **signed** distance: negative inside the figure, positive
/// outside, the sign from an even-odd crossing count over the same segments the
/// magnitude came from.
fn true_signed_distance(p: [f32; 2], loops: &[Loop]) -> f32 {
    let mut nearest = f32::INFINITY;
    let mut inside = false;
    for lp in loops {
        for i in 0..lp.len() {
            let a = lp[i];
            let b = lp[(i + 1) % lp.len()];
            nearest = nearest.min(point_segment_distance(p, a, b));
            // Even-odd crossing count along the +x ray from `p`.
            if (a[1] > p[1]) != (b[1] > p[1]) {
                let x = a[0] + (p[1] - a[1]) / (b[1] - a[1]) * (b[0] - a[0]);
                if x > p[0] {
                    inside = !inside;
                }
            }
        }
    }
    if inside { -nearest } else { nearest }
}

/// The arm's **inradius in sprite-local units** — what `d = 1 + sd / R` divides
/// by, so `(d - 1) * R` is the signed distance the arm is claiming.
///
/// Each value comes from the arm's own arithmetic rather than from a fit:
/// `disc` is the unit circle, `ring` its half-width, the polygon's is its
/// apothem, the star's is the perpendicular from the origin to the edge plane
/// the arm measures against, and the heart's is `HEART_INRADIUS` over the scale
/// that maps heart space into the sprite.
fn inradius_local(shape: f32, points: f32) -> f32 {
    if shape < 0.5 {
        return 1.0;
    }
    if shape < 1.5 {
        return RING_HALF;
    }
    let h = std::f32::consts::PI / points;
    if shape < 2.5 {
        return h.cos();
    }
    if shape < 3.5 {
        // d = x + B*y with B = (1 - k cos h) / (k sin h); the line d = 1 sits
        // 1 / sqrt(1 + B^2) from the origin.
        let b = (1.0 - STAR_INNER * h.cos()) / (STAR_INNER * h.sin());
        return 1.0 / (1.0 + b * b).sqrt();
    }
    HEART_INRADIUS / HEART_SCALE
}

/// The sampled region, in sprite-local units. The sprite quad is `[-1, 1]`; a
/// contour reads well past it.
const PROBE_HALF_SPAN: f32 = 2.2;
const PROBE_STEPS: usize = 96;

/// The per-arm error report: worst absolute deviation from the true signed
/// distance, inside and outside separately, in sprite-local units.
pub(super) struct ArmError {
    pub exterior_worst: f32,
    pub exterior_at: [f32; 2],
    pub interior_worst: f32,
}

pub(super) fn measure_arm(shape: f32, points: f32) -> ArmError {
    let loops = boundary_loops(shape, points);
    let radius = inradius_local(shape, points);
    let mut out = ArmError {
        exterior_worst: 0.0,
        exterior_at: [0.0, 0.0],
        interior_worst: 0.0,
    };
    for iy in 0..=PROBE_STEPS {
        for ix in 0..=PROBE_STEPS {
            let p = [
                -PROBE_HALF_SPAN + 2.0 * PROBE_HALF_SPAN * ix as f32 / PROBE_STEPS as f32,
                -PROBE_HALF_SPAN + 2.0 * PROBE_HALF_SPAN * iy as f32 / PROBE_STEPS as f32,
            ];
            let truth = true_signed_distance(p, &loops);
            let claimed = (mark_distance(p, shape, points) - 1.0) * radius;
            let err = (claimed - truth).abs();
            if truth > 0.0 {
                if err > out.exterior_worst {
                    out.exterior_worst = err;
                    out.exterior_at = p;
                }
            } else {
                out.interior_worst = out.interior_worst.max(err);
            }
        }
    }
    out
}

/// The bar an arm has to clear to count as **exact**. Three orders above the
/// boundary polyline's own sagitta and three below the errors this phase found,
/// so nothing lands ambiguously between the two verdicts.
const EXACT_BAR: f32 = 1e-4;

/// `star`'s worst interior error, **recorded rather than repaired** — see the
/// test below and `mark_distance`'s own comment at that arm.
///
/// It is the value at the *most* pointed star the roster allows, because the
/// error **scales with the point count** rather than being one number: measured
/// 0.00075 at 3 points, 0.06597 at 5, 0.13800 at 7 and 0.24822 at 12. More
/// spikes means a narrower wedge, and the plane the arm measures against
/// diverges from the figure faster near the centre. A twelve-pointed star is
/// therefore a quarter of a sprite half-width out at its middle.
const STAR_INTERIOR_ERROR: f32 = 0.25;

/// **The Phase 2 measurement, and the verdict it produced.** Every arm's
/// returned distance against a numerically computed distance to its own
/// outline, over a region reaching well outside the silhouette — swept across
/// the point counts, since the two arms that take one are the two that were
/// wrong.
///
/// What it found, before the repair in `mark_distance`:
///
/// | arm | exterior worst | interior worst | verdict |
/// |---|---|---|---|
/// | `disc` | 0.00000 | 0.00000 | exact by construction — the control |
/// | `ring` | 0.00000 | 0.00000 | exact |
/// | `polygon` | **0.32628** | 0.00000 | **repaired** |
/// | `star` | **1.05685** | 0.06597 | **exterior repaired, interior recorded** |
/// | `heart` | 0.00001 | 0.00001 | exact (IQ's, to the polyline's resolution) |
///
/// `disc` is the control and it is load-bearing: it is `length(p) - 1` exactly,
/// so an arm that could not reproduce it would convict the harness rather than
/// the shape. It reads 0.
///
/// **`star`'s interior stays approximate on purpose.** It measures against the
/// edge *plane* rather than the figure, and the error **grows with the point
/// count** — 0.00075 at 3 points, 0.06597 at 5, 0.13800 at 7, 0.24822 at 12
/// (see [`STAR_INTERIOR_ERROR`]). Repairing it would change what every shipped
/// `shape = "3"` mark looks like, since the sprite reads only the interior, and
/// this plan's contract is that the particle path moves zero pixels. So it is
/// recorded with its error, which is the other half of the phase's done-when.
///
/// The one place that matters today is `core/tests/fixtures/swarm_shaped.toml`,
/// a 7-pointed star: its marks are drawn from a field 0.138 out at the centre,
/// and always have been.
#[test]
fn the_exterior_distance_is_measured_against_each_shapes_own_outline() {
    // `polygon` and `star` fold by the point count, so the repair has to hold
    // across the roster and not merely at the default.
    let counts: &[f32] = &[MIN_POINTS, DEFAULT_POINTS, 7.0, MAX_POINTS];
    let arms: [(&str, f32, &[f32]); 5] = [
        ("disc", DISC, &[DEFAULT_POINTS]),
        ("ring", RING, &[DEFAULT_POINTS]),
        ("polygon", POLYGON, counts),
        ("star", STAR, counts),
        ("heart", HEART, &[DEFAULT_POINTS]),
    ];

    println!(
        "{:<9} {:>6} {:>14} {:>20} {:>14}",
        "arm", "points", "exterior worst", "at", "interior worst"
    );
    let mut worst_interior_star: f32 = 0.0;
    for (name, shape, points_list) in arms {
        for &points in points_list {
            let e = measure_arm(shape, points);
            println!(
                "{name:<9} {points:>6} {:>14.5} {:>20} {:>14.5}",
                e.exterior_worst,
                format!("({:.2}, {:.2})", e.exterior_at[0], e.exterior_at[1]),
                e.interior_worst
            );

            // Every arm owes an exact EXTERIOR — that is what this phase bought.
            assert!(
                e.exterior_worst < EXACT_BAR,
                "{name} at points = {points} is {:.5} from its own outline at \
                 ({:.2}, {:.2}) — the contours `shape_field` draws are level sets \
                 of this number, so an arm that is wrong out here draws the wrong \
                 figure",
                e.exterior_worst,
                e.exterior_at[0],
                e.exterior_at[1]
            );

            if shape == STAR {
                // Recorded, not repaired. Bounded so it cannot silently grow.
                worst_interior_star = worst_interior_star.max(e.interior_worst);
            } else {
                assert!(
                    e.interior_worst < EXACT_BAR,
                    "{name} at points = {points} drifted inside its own silhouette \
                     ({:.5}) — only `star` is knowingly approximate there",
                    e.interior_worst
                );
            }
        }
    }

    assert!(
        worst_interior_star <= STAR_INTERIOR_ERROR,
        "the star's interior error grew to {worst_interior_star:.5}, past the \
         {STAR_INTERIOR_ERROR} this phase recorded — it measures against the edge \
         plane rather than the figure, and that is a documented approximation, \
         not a licence for it to get worse"
    );
    assert!(
        worst_interior_star > EXACT_BAR,
        "the star's interior is now exact ({worst_interior_star:.5}) — good, but \
         it means the sprite's arithmetic changed, which this plan's contract \
         forbids. Check the golden baselines before relaxing this."
    );
}
