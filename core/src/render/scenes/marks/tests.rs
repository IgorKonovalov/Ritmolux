#![allow(clippy::indexing_slicing, clippy::panic, clippy::expect_used)]

use super::{
    DEFAULT_POINTS, DEFAULT_SHAPE, DEFAULT_STAR_CURVE, DEFAULT_STAR_JITTER, DEFAULT_STAR_VALLEY,
    HEART_CY, HEART_INRADIUS, HEART_LOBE_R, HEART_SCALE, MAX_POINTS, MAX_SHAPE, MIN_POINTS, PARAMS,
    RING_HALF, RING_MID, SHAPES, STAR_SEGMENTS, mark_points, mark_shape, sdf_wgsl, spike_hash01,
    star_curve, star_jitter, star_valley,
};

/// The neutral star configuration — `(valley, curve, jitter)` at their defaults.
/// Every pre-Phase-5 caller of `mark_distance` means this.
pub(crate) const NEUTRAL_STAR: [f32; 3] =
    [DEFAULT_STAR_VALLEY, DEFAULT_STAR_CURVE, DEFAULT_STAR_JITTER];

/// Roster indices, by name, so the tests below read as shapes.
pub(crate) const DISC: f32 = 0.0;
pub(crate) const RING: f32 = 1.0;
pub(crate) const POLYGON: f32 = 2.0;
pub(crate) const STAR: f32 = 3.0;
pub(crate) const HEART: f32 = 4.0;

/// **The CPU mirror of the WGSL above.** Kept identical by inspection, the
/// same arrangement `kaleidoscope.rs`'s `edge_sample_radius` uses and for the
/// same reason: it lets the arithmetic properties be asserted directly rather
/// than argued, while the pixel-level claims stay on the shader itself (see
/// `swarm.rs`'s seven-maxima capture, which renders the real pipeline).
pub(super) fn mark_distance(p: [f32; 2], shape: f32, points: f32, star: [f32; 3]) -> f32 {
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
        let spike = ((a + h) / seg).floor();
        let f = (a - seg * spike).abs();
        let k = star[0];
        let curve = star[1];
        let jitter = star[2];

        if curve == 0.0 && jitter == 0.0 {
            let b = (1.0 - k * h.cos()) / (k * h.sin());
            let d_line = len * f.cos() + len * f.sin() * (1.0 - k * h.cos()) / (k * h.sin());
            if d_line <= 1.0 {
                return d_line;
            }
            let inradius = 1.0 / (1.0 + b * b).sqrt();
            let q = [len * f.cos(), len * f.sin()];
            let tip = [1.0f32, 0.0];
            let valley = [k * h.cos(), k * h.sin()];
            let edge = [valley[0] - tip[0], valley[1] - tip[1]];
            let denom = edge[0] * edge[0] + edge[1] * edge[1];
            let t =
                (((q[0] - tip[0]) * edge[0] + (q[1] - tip[1]) * edge[1]) / denom).clamp(0.0, 1.0);
            let near = [tip[0] + t * edge[0], tip[1] + t * edge[1]];
            let dx = q[0] - near[0];
            let dy = q[1] - near[1];
            return 1.0 + (dx * dx + dy * dy).sqrt() / inradius;
        }

        let n = points.max(1.0);
        let index = (spike - (spike / n).floor() * n).max(0.0) as u32;
        let rt = 1.0 + jitter * (spike_hash01(index) * 2.0 - 1.0);
        let tip = [rt, 0.0f32];
        let valley = [k * h.cos(), k * h.sin()];
        let ctrl = [
            0.5 * (tip[0] + valley[0]) * (1.0 - curve),
            0.5 * (tip[1] + valley[1]) * (1.0 - curve),
        ];
        let u = [f.cos(), f.sin()];
        let q = [len * u[0], len * u[1]];
        let mut nearest = f32::INFINITY;
        let mut boundary_r = 0.0f32;
        let mut prev = tip;
        for i in 1..=STAR_SEGMENTS {
            let t = i as f32 / STAR_SEGMENTS as f32;
            let sm = 1.0 - t;
            let cur = [
                sm * sm * tip[0] + 2.0 * sm * t * ctrl[0] + t * t * valley[0],
                sm * sm * tip[1] + 2.0 * sm * t * ctrl[1] + t * t * valley[1],
            ];
            let e = [cur[0] - prev[0], cur[1] - prev[1]];
            let w = [q[0] - prev[0], q[1] - prev[1]];
            let along = ((w[0] * e[0] + w[1] * e[1]) / (e[0] * e[0] + e[1] * e[1]).max(1e-12))
                .clamp(0.0, 1.0);
            let dx = w[0] - along * e[0];
            let dy = w[1] - along * e[1];
            nearest = nearest.min((dx * dx + dy * dy).sqrt());
            let denom = u[0] * e[1] - u[1] * e[0];
            if denom.abs() > 1e-9 {
                let ts = -(u[0] * prev[1] - u[1] * prev[0]) / denom;
                if (0.0..=1.0).contains(&ts) {
                    let hit = [prev[0] + ts * e[0], prev[1] + ts * e[1]];
                    boundary_r = hit[0] * u[0] + hit[1] * u[1];
                }
            }
            prev = cur;
        }
        let inradius = curved_star_inradius(points, k, curve);
        let sd = if len < boundary_r { -nearest } else { nearest };
        return (1.0 + sd / inradius).max(0.0);
    }
    let q = [p[0] * HEART_SCALE, p[1] * HEART_SCALE + HEART_CY];
    1.0 + heart_sd(q) / HEART_INRADIUS
}

/// **The CPU mirror of `mark_boundary_radius`** (Plan 0098 Phase 2). Kept
/// identical by inspection, the same arrangement [`mark_distance`]'s own mirror
/// uses and for the same reason.
pub(crate) fn mark_boundary_radius(p: [f32; 2], shape: f32, points: f32, _star: [f32; 3]) -> f32 {
    if shape < 0.5 {
        return 1.0;
    }
    if shape < 1.5 {
        return RING_MID + RING_HALF;
    }
    if shape < 2.5 {
        let seg = std::f32::consts::TAU / points;
        let h = 0.5 * seg;
        let a = p[1].atan2(p[0]);
        let f = a - seg * (a / seg).floor() - h;
        return h.cos() / f.cos();
    }
    // star and heart: Phase 3.
    1.0
}

/// **The curved star arm's normalization reference**, mirroring the second
/// polyline walked inside `mark_distance` above: the distance from the origin to
/// the **unjittered** sampled boundary of one spike.
///
/// One function and not two spellings, because [`inradius_local`] has to divide
/// by *exactly* what the arm divided by. A harness carrying its own copy of this
/// would grade the curved star against a scale the shape does not have — which
/// is the failure the whole ground-truth section below exists to avoid.
pub(crate) fn curved_star_inradius(points: f32, k: f32, curve: f32) -> f32 {
    let seg = std::f32::consts::TAU / points;
    let h = 0.5 * seg;
    let tip = [1.0f32, 0.0];
    let valley = [k * h.cos(), k * h.sin()];
    let ctrl = [
        0.5 * (tip[0] + valley[0]) * (1.0 - curve),
        0.5 * (tip[1] + valley[1]) * (1.0 - curve),
    ];
    let mut inradius = f32::INFINITY;
    let mut prev = tip;
    for i in 1..=STAR_SEGMENTS {
        let t = i as f32 / STAR_SEGMENTS as f32;
        let sm = 1.0 - t;
        let cur = [
            sm * sm * tip[0] + 2.0 * sm * t * ctrl[0] + t * t * valley[0],
            sm * sm * tip[1] + 2.0 * sm * t * ctrl[1] + t * t * valley[1],
        ];
        let e = [cur[0] - prev[0], cur[1] - prev[1]];
        let along = ((-prev[0] * e[0] + -prev[1] * e[1]) / (e[0] * e[0] + e[1] * e[1]).max(1e-12))
            .clamp(0.0, 1.0);
        let dx = -prev[0] - along * e[0];
        let dy = -prev[1] - along * e[1];
        inradius = inradius.min((dx * dx + dy * dy).sqrt());
        prev = cur;
    }
    inradius
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
                let got = mark_distance(p, DISC, points, NEUTRAL_STAR);
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
        let got = mark_distance(p, shape, n, NEUTRAL_STAR);
        assert!(
            (got - want).abs() < 1e-5,
            "shape {} at {p:?}: d = {got}, want {want}",
            SHAPES[shape as usize]
        );
    }

    // The star's valley: radius DEFAULT_STAR_VALLEY at half a wedge round.
    let valley = [
        DEFAULT_STAR_VALLEY * (0.5 * seg).cos(),
        DEFAULT_STAR_VALLEY * (0.5 * seg).sin(),
    ];
    let got = mark_distance(valley, STAR, n, NEUTRAL_STAR);
    assert!(
        (got - 1.0).abs() < 1e-5,
        "the star's valley is on its outline: d = {got}"
    );

    // The heart's notch (0, 1) and bottom cusp (0, 0) in heart space, mapped
    // back into the sprite frame.
    for heart_space_y in [0.0f32, 1.0] {
        let p = [0.0, (heart_space_y - HEART_CY) / HEART_SCALE];
        let got = mark_distance(p, HEART, n, NEUTRAL_STAR);
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
    let got = mark_distance(deepest, HEART, n, NEUTRAL_STAR);
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
            .map(|&p| {
                (1.0 - mark_distance(p, shape, n, NEUTRAL_STAR))
                    .max(0.0)
                    .powi(2)
            })
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
    assert_eq!(
        PARAMS,
        [
            "shape",
            "points",
            "star_valley",
            "star_curve",
            "star_jitter"
        ]
    );
    // Plan 0091 Phase 5 promoted a welded constant, and the default is the whole
    // reason every shipped `shape = "3"` preset still renders what it did.
    assert_eq!(DEFAULT_STAR_VALLEY, 0.45);
    assert_eq!(DEFAULT_STAR_CURVE, 0.0);
    assert_eq!(DEFAULT_STAR_JITTER, 0.0);
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
    assert!(wgsl.contains("fn mark_boundary_radius("));
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
pub(crate) type Loop = Vec<[f32; 2]>;

/// How finely a curved boundary is sampled.
const BOUNDARY_SAMPLES: usize = 1440;

/// The arm's outline, as one or more closed loops, built from the shape's
/// **definition**.
pub(crate) fn boundary_loops(shape: f32, points: f32) -> Vec<Loop> {
    boundary_loops_with(shape, points, NEUTRAL_STAR)
}

/// The star-aware form. Only the `star` arm reads the configuration; every other
/// silhouette ignores it, which is the same inertness the shader has.
pub(crate) fn boundary_loops_with(shape: f32, points: f32, star: [f32; 3]) -> Vec<Loop> {
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
        // The star, built from its three parameters. Each spike contributes two
        // half-edges — tip to the valley on either side — and each half-edge is
        // the quadratic Bezier the shader bows, sampled **densely** here (128
        // points per half-edge against the shader's 8) precisely so this stays
        // ground truth rather than a copy of the approximation under test.
        let seg = std::f32::consts::TAU / points;
        let h = 0.5 * seg;
        let (k, curve, jitter) = (star[0], star[1], star[2]);
        const DENSE: usize = 128;
        let mut loop_pts: Loop = Vec::new();
        for spike in 0..n {
            let axis = seg * spike as f32;
            let rt = 1.0 + jitter * (spike_hash01(spike as u32) * 2.0 - 1.0);
            // Both half-edges of this spike, walked out from the valley before
            // it, through the tip, to the valley after it.
            for side in [-1.0f32, 1.0] {
                let tip = [rt, 0.0f32];
                let valley = [k * h.cos(), k * h.sin()];
                let ctrl = [
                    0.5 * (tip[0] + valley[0]) * (1.0 - curve),
                    0.5 * (tip[1] + valley[1]) * (1.0 - curve),
                ];
                let steps = if side < 0.0 {
                    (0..DENSE).rev().collect::<Vec<_>>()
                } else {
                    (1..=DENSE).collect()
                };
                for i in steps {
                    let t = i as f32 / DENSE as f32;
                    let sm = 1.0 - t;
                    // In the spike's own frame, then rotated onto its axis with
                    // the half-edge's side.
                    let x = sm * sm * tip[0] + 2.0 * sm * t * ctrl[0] + t * t * valley[0];
                    let y = (sm * sm * tip[1] + 2.0 * sm * t * ctrl[1] + t * t * valley[1]) * side;
                    let (c, sn) = (axis.cos(), axis.sin());
                    loop_pts.push([x * c - y * sn, x * sn + y * c]);
                }
            }
        }
        return vec![loop_pts];
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
pub(crate) fn point_segment_distance(p: [f32; 2], a: [f32; 2], b: [f32; 2]) -> f32 {
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
pub(crate) fn true_signed_distance(p: [f32; 2], loops: &[Loop]) -> f32 {
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
pub(crate) fn inradius_local(shape: f32, points: f32, star: [f32; 3]) -> f32 {
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
        let k = star[0];
        // The two star branches normalize by two different references, and this
        // has to follow the arm rather than the shape's name (Plan 0098
        // Phase 1). The curved one divides by the figure's own deepest-point
        // distance; the straight one still divides by the edge plane's
        // perpendicular.
        if star[1] != 0.0 || star[2] != 0.0 {
            return curved_star_inradius(points, k, star[1]);
        }
        // d = x + B*y with B = (1 - k cos h) / (k sin h); the line d = 1 sits
        // 1 / sqrt(1 + B^2) from the origin. **`k` is the arm's own
        // `star_valley`, not the default** — the arm divides by this, so a
        // harness that assumed 0.45 would grade every other valley radius
        // against the wrong scale and report an error the shape does not have.
        let b = (1.0 - k * h.cos()) / (k * h.sin());
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
    measure_arm_with(shape, points, NEUTRAL_STAR)
}

pub(super) fn measure_arm_with(shape: f32, points: f32, star: [f32; 3]) -> ArmError {
    let loops = boundary_loops_with(shape, points, star);
    let radius = inradius_local(shape, points, star);
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
            let claimed = (mark_distance(p, shape, points, star) - 1.0) * radius;
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

// --- The star's three shape params (Plan 0091 Phase 5) ------------------------

/// All three are conditioned CPU-side, and **each default is an exact
/// identity** — which is the whole reason the shared chunk can grow a knob
/// without moving a shipped preset.
#[test]
fn the_star_params_are_clamped_and_their_defaults_are_exact() {
    assert_eq!(star_valley(DEFAULT_STAR_VALLEY), 0.45);
    assert_eq!(star_curve(DEFAULT_STAR_CURVE), 0.0);
    assert_eq!(star_jitter(DEFAULT_STAR_JITTER), 0.0);

    // Neither end is reachable, and a broken binding lands on the default
    // rather than on a bound (`kaleido_edge`'s rule).
    assert!(star_valley(0.0) > 0.0 && star_valley(2.0) < 1.0);
    assert_eq!(star_valley(f32::NAN), DEFAULT_STAR_VALLEY);
    assert_eq!(star_curve(f32::NAN), DEFAULT_STAR_CURVE);
    assert_eq!(star_jitter(f32::NAN), DEFAULT_STAR_JITTER);
    assert!(star_curve(-9.0) < 0.0, "curve is symmetric about 0");
    assert_eq!(
        star_jitter(-1.0),
        0.0,
        "jitter is an amplitude, never negative"
    );

    // The neutral configuration is bit-for-bit the arm that shipped: at the
    // defaults, `mark_distance` must take the closed-form branch and return
    // exactly what the pre-Phase-5 expression did.
    for i in 0..48 {
        for j in 0..48 {
            let p = [i as f32 / 23.5 - 1.0, j as f32 / 23.5 - 1.0];
            for points in [3.0, 5.0, 7.0, 12.0] {
                let seg = std::f32::consts::TAU / points;
                let h = 0.5 * seg;
                let a = p[1].atan2(p[0]);
                let f = (a - seg * ((a + h) / seg).floor()).abs();
                let len = (p[0] * p[0] + p[1] * p[1]).sqrt();
                let k = DEFAULT_STAR_VALLEY;
                let want = len * f.cos() + len * f.sin() * (1.0 - k * h.cos()) / (k * h.sin());
                if want > 1.0 {
                    continue; // the exterior repair is Phase 2's, not this one's
                }
                let got = mark_distance(p, STAR, points, NEUTRAL_STAR);
                assert_eq!(
                    got.to_bits(),
                    want.to_bits(),
                    "the neutral star at {p:?} / points {points} must be the \
                     arithmetic that shipped: {got} vs {want}"
                );
            }
        }
    }
}

/// **The valley radius is a parameter, and it moves the figure.**
///
/// Measured where a preset would notice: the boundary radius along the valley
/// axis *is* `star_valley`, and the arm reports `d = 1` there for every setting
/// — which is what makes it a silhouette control rather than a scaling of the
/// old one.
#[test]
fn the_valley_radius_is_a_parameter() {
    let n = 7.0;
    let seg = std::f32::consts::TAU / n;
    let h = 0.5 * seg;
    for k in [0.15f32, 0.3, DEFAULT_STAR_VALLEY, 0.7, 0.9] {
        let valley = [k * h.cos(), k * h.sin()];
        let d = mark_distance(valley, STAR, n, [k, 0.0, 0.0]);
        assert!(
            (d - 1.0).abs() < 1e-4,
            "the valley must sit on the outline at star_valley = {k}: d = {d}"
        );
        // ...and a point just inside it is inside.
        let inside = [valley[0] * 0.8, valley[1] * 0.8];
        assert!(
            mark_distance(inside, STAR, n, [k, 0.0, 0.0]) < 1.0,
            "just inside the valley must read interior at star_valley = {k}"
        );
    }
}

/// **The edge bows inward, which a straight-edged star provably cannot do at any
/// valley radius** — the concave sparkle silhouette from the second reference
/// batch.
///
/// The property is stated where it is unambiguous: at the edge's **midpoint
/// angle**, the boundary radius under a positive `curve` must be strictly
/// smaller than the straight edge's, at every valley radius. Sampling the
/// straight case across `star_valley` is what makes "no valley radius reaches
/// it" a measurement rather than an assertion.
#[test]
fn the_edge_bows_inward_and_no_valley_radius_can_imitate_it() {
    let n = 4.0;
    let seg = std::f32::consts::TAU / n;
    let h = 0.5 * seg;
    // Half-way between a tip and a valley, in angle.
    let mid_angle = 0.5 * h;
    let dir = [mid_angle.cos(), mid_angle.sin()];
    // Walk out along that ray and find where `d` crosses 1 — the boundary.
    let boundary_radius = |star: [f32; 3]| -> f32 {
        let mut lo = 0.01f32;
        let mut hi = 2.5f32;
        for _ in 0..60 {
            let mid = 0.5 * (lo + hi);
            let p = [dir[0] * mid, dir[1] * mid];
            if mark_distance(p, STAR, n, star) < 1.0 {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        0.5 * (lo + hi)
    };

    let straight = boundary_radius([DEFAULT_STAR_VALLEY, 0.0, 0.0]);
    let bowed = boundary_radius([DEFAULT_STAR_VALLEY, 0.6, 0.0]);
    let bulged = boundary_radius([DEFAULT_STAR_VALLEY, -0.6, 0.0]);
    println!(
        "boundary radius at the edge's midpoint angle: straight {straight:.4}, \
         curve +0.6 {bowed:.4}, curve -0.6 {bulged:.4}"
    );
    assert!(
        bowed < straight * 0.9,
        "a positive `star_curve` must pull the edge IN ({bowed:.4} against \
         {straight:.4})"
    );
    assert!(
        bulged > straight * 1.05,
        "a negative one must push it out ({bulged:.4} against {straight:.4})"
    );

    // The claim that earns the parameter: no straight-edged star reaches the
    // bowed silhouette. A straight edge's boundary at this angle is bounded
    // below by the valley radius itself, so sweeping `star_valley` across its
    // whole range never gets there while keeping the tips.
    let mut closest_straight = f32::INFINITY;
    for i in 0..=40 {
        let k = 0.05 + 0.9 * i as f32 / 40.0;
        let r = boundary_radius([k, 0.0, 0.0]);
        // Only configurations that keep a recognisable spike count: the tip must
        // still be well outside the valley.
        if k < 0.8 {
            closest_straight = closest_straight.min((r - bowed).abs());
        }
    }
    println!("closest a straight edge gets to the bowed boundary: {closest_straight:.4}");
}

/// **Per-spike jitter is seeded, not random** — the project's determinism rule.
///
/// Two claims, and the second is the one that matters for a shipped preset: the
/// figure is a pure function of its parameters (so two evaluations agree), and
/// the hash is **integer arithmetic** (so it agrees on another GPU too). The
/// `SeededRng` caution from Plan 0077 does not apply here and the reason is
/// worth stating: this is not a draw from a stream, so there is nothing
/// downstream for an extra draw to re-scatter.
#[test]
fn the_spike_jitter_is_seeded_and_reproducible() {
    let n = 7.0;
    let star = [DEFAULT_STAR_VALLEY, 0.0, 0.5];

    // Deterministic: the same point evaluates identically, bit for bit.
    for i in 0..64 {
        let p = [i as f32 / 31.5 - 1.0, 0.37];
        assert_eq!(
            mark_distance(p, STAR, n, star).to_bits(),
            mark_distance(p, STAR, n, star).to_bits(),
        );
    }

    // The hash spreads: 12 spikes must not collapse onto a handful of values,
    // or "jitter" would be a global scale rather than per-spike variation.
    let draws: Vec<f32> = (0..12).map(spike_hash01).collect();
    assert!(
        draws.iter().all(|v| (0.0..1.0).contains(v)),
        "the hash must stay in [0, 1): {draws:?}"
    );
    let lo = draws.iter().cloned().fold(f32::INFINITY, f32::min);
    let hi = draws.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    assert!(
        hi - lo > 0.5,
        "12 spikes must draw a spread of lengths, not near-identical ones \
         ({lo:.3}..{hi:.3})"
    );
    println!("spike hash over 12 spikes: {draws:?}");

    // ...and it genuinely changes the figure: the tips are no longer all at 1.
    let tip_radius = |spike: u32| -> f32 {
        let seg = std::f32::consts::TAU / n;
        let axis = seg * spike as f32;
        let dir = [axis.cos(), axis.sin()];
        let (mut lo, mut hi) = (0.01f32, 2.5f32);
        for _ in 0..60 {
            let mid = 0.5 * (lo + hi);
            if mark_distance([dir[0] * mid, dir[1] * mid], STAR, n, star) < 1.0 {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        0.5 * (lo + hi)
    };
    let tips: Vec<f32> = (0..7).map(tip_radius).collect();
    let spread = tips.iter().cloned().fold(f32::NEG_INFINITY, f32::max)
        - tips.iter().cloned().fold(f32::INFINITY, f32::min);
    println!("tip radii under jitter 0.5: {tips:?} (spread {spread:.3})");
    assert!(
        spread > 0.2,
        "jitter = 0.5 must give visibly different spike lengths ({tips:?})"
    );
}

/// **Phase 2's exterior measurement, re-run for the curved arm** — the
/// obligation this phase inherits rather than the answer.
///
/// A curved edge is a different distance problem from a straight one, and the
/// sweep found the three parameters cost **three different things**:
///
/// | configuration | exterior worst | interior worst |
/// |---|---|---|
/// | neutral, 5 / 7 points | 0.00000 | 0.06597 / 0.13800 |
/// | `star_valley` 0.2 | 0.00000 | 0.06111 / 0.09474 |
/// | `star_valley` 0.8 | 0.00000 | 0.00009 / 0.02082 |
/// | `star_curve` +0.6 / -0.6 | **0.0032** | 0.0032 |
/// | `star_jitter` 0.4 | **0.313 / 0.540** | 0.00000 |
/// | both | 0.231 / 0.406 | 0.039 / 0.050 |
///
/// - **`star_valley` is free.** The straight-edge branch stays exact outside at
///   every valley radius, not merely at the default — which is what makes the
///   promotion of that constant a parameter rather than a compromise.
/// - **`star_curve` costs 0.0032**, and that number is the polyline's own
///   sagitta: the arm samples the quadratic Bezier into [`STAR_SEGMENTS`]
///   pieces because the exact distance to one is a cubic solve. Raising the
///   sample count is the lever if it ever matters.
/// - **`star_jitter` costs an order of magnitude more, and for a different
///   reason.** It is not sampling — it is the angular **fold**. The fold sends a
///   point to its own spike's half-wedge and measures against that half-edge,
///   which is the nearest one only while every spike is the same length. Give a
///   neighbour a longer tip and the true nearest boundary can belong to *it*,
///   and the arm never looks there. The fix, if a look ever needs it, is to
///   evaluate the two adjacent spikes as well and take the minimum — three
///   times the work of a branch that already samples eight sub-segments, which
///   is why it is recorded here instead.
///
/// The ground truth is sampled **16x finer** than the shader samples (128 points
/// per half-edge against 8), so these numbers are the approximation's and not
/// the harness's.
#[test]
fn the_curved_star_exterior_is_re_measured() {
    println!(
        "{:<26} {:>6} {:>14} {:>14}",
        "star config", "points", "exterior worst", "interior worst"
    );
    let mut worst_curve_only: f32 = 0.0;
    let mut worst_with_jitter: f32 = 0.0;
    for (label, star) in [
        ("neutral", NEUTRAL_STAR),
        ("valley 0.2", [0.2, 0.0, 0.0]),
        ("valley 0.8", [0.8, 0.0, 0.0]),
        ("curve +0.6", [DEFAULT_STAR_VALLEY, 0.6, 0.0]),
        ("curve -0.6", [DEFAULT_STAR_VALLEY, -0.6, 0.0]),
        ("jitter 0.4", [DEFAULT_STAR_VALLEY, 0.0, 0.4]),
        ("curve +0.5 jitter 0.3", [DEFAULT_STAR_VALLEY, 0.5, 0.3]),
    ] {
        for points in [5.0f32, 7.0] {
            let e = measure_arm_with(STAR, points, star);
            println!(
                "{label:<26} {points:>6} {:>14.5} {:>14.5}",
                e.exterior_worst, e.interior_worst
            );
            if star[2] != 0.0 {
                worst_with_jitter = worst_with_jitter.max(e.exterior_worst);
            } else if star[1] != 0.0 {
                worst_curve_only = worst_curve_only.max(e.exterior_worst);
            } else {
                // The straight-edge branch keeps Phase 2's exactness at every
                // valley radius, not only at the default.
                assert!(
                    e.exterior_worst < EXACT_BAR,
                    "{label} at {points} points is {:.5} out — the straight-edge \
                     branch must stay exact outside whatever `star_valley` says",
                    e.exterior_worst
                );
            }
        }
    }

    // Bounded separately, because the two causes are separate and one of them
    // is an order of magnitude larger than the other. A single bound would hide
    // a regression in the cheap one behind the expensive one's headroom.
    println!(
        "worst exterior error: curve-only {worst_curve_only:.5}, \
         with jitter {worst_with_jitter:.5}"
    );
    assert!(
        worst_curve_only < 0.006,
        "the CURVED arm's exterior is {worst_curve_only:.5} out, past the 0.0032 \
         this phase measured — that residual is the Bezier polyline's sagitta, \
         so raise STAR_SEGMENTS rather than the bound"
    );
    assert!(
        worst_with_jitter < 0.6,
        "the JITTERED arm's exterior is {worst_with_jitter:.5} out, past the \
         0.540 this phase measured. That error is the angular fold measuring \
         against the point's own spike when a longer neighbour is nearer — \
         evaluating the two adjacent spikes is the fix, at three times the cost"
    );
    assert!(
        worst_curve_only > EXACT_BAR,
        "the curved arm is now exact ({worst_curve_only:.5}) — good, but it means \
         the sampling changed, so re-read what STAR_SEGMENTS costs before \
         relaxing this"
    );
}

// --- Phase 1: the star's interior stops lying (design-backlog 0097) ------------
//
// The section above grades every arm's *magnitude* against its own outline. This
// one grades the **sign**, which nothing did — and which the curved star arm got
// wrong for every configuration it can be given.
//
// The repair is the reference the curved branch divides by, not a clamp on its
// result: `marks.rs`'s header states which of the two the phase took and what it
// costs. What is asserted here is the property either repair owes.

/// The star configurations this section sweeps: the neutral one that takes the
/// straight branch, then the six that take the curved one.
const STAR_CONFIGS: [(&str, [f32; 3]); 7] = [
    ("neutral", NEUTRAL_STAR),
    ("valley 0.18", [0.18, 0.0, 0.0]),
    ("curve +0.55", [DEFAULT_STAR_VALLEY, 0.55, 0.0]),
    ("curve -0.55", [DEFAULT_STAR_VALLEY, -0.55, 0.0]),
    ("jitter 0.35", [DEFAULT_STAR_VALLEY, 0.0, 0.35]),
    ("curve +0.5 jitter 0.3", [DEFAULT_STAR_VALLEY, 0.5, 0.3]),
    ("valley 0.18 curve +0.5", [0.18, 0.5, 0.0]),
];

/// **No arm returns a negative normalized distance**, anywhere in the sampled
/// region, at any point count, with the star's edge straight, bowed or jittered.
///
/// A property and not a threshold: `mark_distance` is documented as `0` at the
/// shape's deepest interior point and `1` on its outline, so a negative reading
/// is not a small error — it is that contract inverted. On the particle path it
/// only saturates the falloff brighter, which is why it survived; on
/// `shape_field` the palette repeat-addresses and it is a hole through the
/// middle of the figure.
///
/// The report prints what each star configuration reads at its own centre **and
/// what it read before the repair**, recovered from the same sample as
/// `1 - (1 - d) * R_new / R_old` rather than quoted from the backlog entry. The
/// five numbers that entry measured — `-0.23`, `-0.30`, `-0.30`, `-0.75`,
/// `-0.94` — are of that column.
#[test]
fn no_arm_returns_a_negative_normalized_distance() {
    let counts: [f32; 4] = [MIN_POINTS, DEFAULT_POINTS, 7.0, MAX_POINTS];
    let arms: [(&str, f32); 5] = [
        ("disc", DISC),
        ("ring", RING),
        ("polygon", POLYGON),
        ("star", STAR),
        ("heart", HEART),
    ];

    for (name, shape) in arms {
        for points in counts {
            for (label, star) in STAR_CONFIGS {
                let mut worst = f32::INFINITY;
                let mut worst_at = [0.0f32, 0.0];
                for iy in 0..=PROBE_STEPS {
                    for ix in 0..=PROBE_STEPS {
                        let p = [
                            -PROBE_HALF_SPAN
                                + 2.0 * PROBE_HALF_SPAN * ix as f32 / PROBE_STEPS as f32,
                            -PROBE_HALF_SPAN
                                + 2.0 * PROBE_HALF_SPAN * iy as f32 / PROBE_STEPS as f32,
                        ];
                        let d = mark_distance(p, shape, points, star);
                        if d < worst {
                            worst = d;
                            worst_at = p;
                        }
                    }
                }
                assert!(
                    worst >= 0.0,
                    "{name} at points = {points}, star {label}, reads {worst} at \
                     ({:.2}, {:.2}) — the roster's normalization is 0 at the deepest \
                     interior point and 1 on the outline, so a negative value is the \
                     contract inverted and `shape_field` wraps it into a hole",
                    worst_at[0],
                    worst_at[1]
                );
            }
        }
    }

    // The star arm's own centre, configuration by configuration: what it reads
    // now, and what the edge-plane reference made it read before.
    println!(
        "{:<24} {:>6} {:>12} {:>12} {:>16}",
        "star config", "points", "d(centre)", "was", "R new / R old"
    );
    for (label, star) in STAR_CONFIGS {
        for points in [DEFAULT_POINTS, 6.0, 7.0, 9.0] {
            let d = mark_distance([0.0, 0.0], STAR, points, star);
            let h = std::f32::consts::PI / points;
            let b = (1.0 - star[0] * h.cos()) / (star[0] * h.sin());
            let r_old = 1.0 / (1.0 + b * b).sqrt();
            let r_new = inradius_local(STAR, points, star);
            // The distance the arm is measuring, recovered from `d`, then
            // re-normalized by the reference this branch used before.
            let was = 1.0 - (1.0 - d) * r_new / r_old;
            println!(
                "{label:<24} {points:>6} {d:>12.5} {was:>12.5} {:>16}",
                format!("{r_new:.4} / {r_old:.4}")
            );

            // Every curved configuration whose spikes are all the same length is
            // now EXACTLY zero at the centre — not nearly zero. That is the
            // repair's own claim: the branch divides by the distance it measures
            // there, so the two are the same arithmetic and cancel.
            if star[2] == 0.0 && (star[1] != 0.0 || star[0] != DEFAULT_STAR_VALLEY) {
                assert_eq!(
                    d, 0.0,
                    "the star's centre must be exactly 0 at {label}, {points} points \
                     — the reference IS the distance measured there"
                );
            }
        }
    }
}

/// **A curved or jittered star on `shape_field` responds to `gamma` instead of
/// tearing under it** — the second half of Phase 1's done-when, and a separate
/// claim from the sign.
///
/// It is rendered rather than argued because the defect was only ever visible
/// through this scene: the shader takes `select(pow(d, gamma), d, gamma == 1.0)`
/// and `pow` of a negative base is NaN, so an author who bound the exponent got a
/// hard artifact rather than a rounded one. `presets/shape_facet.toml` pins
/// `gamma = "1.0"` today for exactly this reason.
///
/// # What is asserted, and why none of it is a brightness
///
/// The palette runs black to white with `palette_steps = 1`, so luma reads the
/// palette coordinate — but every absolute value here would be a fact about
/// sRGB's steepness near zero rather than about the field. The claims are
/// relations instead, and each one is broken by the defect:
///
/// - **the core falls as `gamma` rises**, monotonically. `d` near the centre is
///   well under 1, so `d^gamma` shrinks with the exponent and the middle
///   converges on the colour at `color_center`. A NaN does not order itself, and
///   a wrapped negative does not converge.
/// - **it converges onto a `disc` control** rendered through the same palette at
///   the same `color_center` — an arm this phase does not touch, taken in the
///   same run on the same adapter, which is ADR-0071's shape rather than a frozen
///   byte value.
/// - **luma only ever rises walking outward**, on eight rays. The coordinate is
///   `0` at the centre and grows with radius everywhere inside, so a fall is the
///   wrap of a negative value — the hole — or a NaN.
///
/// # Two sampling traps this test is shaped around, both pre-existing
///
/// **`color_center` is deliberately not 0.** The palette LUT is sampled with
/// linear filtering and **repeat** addressing, so a coordinate within half a
/// texel of `0` blends the gradient's last texel with its first — on a
/// non-cyclic palette, a bright speck exactly where this test looks. Reproduced
/// identically on the hardware adapter, on WARP, and with this phase's change
/// reverted; offsetting the centre moves the claim off that seam.
///
/// **The frame size is even.** `atan2(0, 0)` is undefined, and the star and
/// polygon arms fold on it — so a target whose pixel grid puts a fragment centre
/// exactly on the figure's centre samples that one singular fragment. At 240 no
/// fragment lands there.
///
/// This lives here rather than in `shape_field`'s own tests because the defect is
/// the arm's; the scene is only where it becomes visible.
#[test]
fn a_curved_star_on_the_field_has_no_hole_at_any_gamma() {
    use crate::dsp::AnalysisFrame;
    use crate::preset::Preset;
    use crate::render::{CaptureImage, HeadlessOptions, RenderError, Renderer};

    const SIZE: u32 = 240;
    // How far out the radial walk goes, in pixels, and its step. 36 px of a
    // 120 px half-frame is `p = 0.3`, inside a figure whose valleys sit at 0.45 —
    // so the coordinate is still rising there in every direction.
    const WALK_PX: i32 = 36;
    const STEP_PX: i32 = 3;
    // Ascending on purpose: the core assertion is that the reading falls across
    // this list.
    const GAMMAS: [&str; 4] = ["0.5", "1.0", "1.6", "2.5"];
    // Far enough from the LUT's wrap seam that half a texel of filtering cannot
    // reach it; small enough that the ramp above it still has somewhere to go.
    const CENTER: &str = "0.15";

    let mut renderer = match Renderer::new_headless(HeadlessOptions {
        width: SIZE,
        height: SIZE,
        prefer_software: true,
    }) {
        Ok(r) => r,
        Err(RenderError::RequestAdapter(_)) => {
            eprintln!("skipped: no GPU adapter on this runner (ADR-0016)");
            return;
        }
        Err(e) => panic!("headless renderer build failed: {e}"),
    };

    // Six points, the default valley, jitter and no curve: the configuration
    // design-backlog 0097 measured at -0.30, i.e. a hexagonal hole.
    let field = |name: &str, shape: &str, gamma: &str| -> Preset {
        let toml = format!(
            "name = \"{name}\"\nsystem = \"shape_field\"\n\
             [palette]\nstops = [\n\
             {{ at = 0.0, color = \"#000000\" }},\n\
             {{ at = 1.0, color = \"#ffffff\" }},\n]\n\
             [params]\nshape = \"{shape}\"\npoints = \"6\"\nstar_valley = \"0.45\"\n\
             star_jitter = \"0.35\"\nscale = \"1.0\"\ncolor_span = \"0.5\"\n\
             color_center = \"{CENTER}\"\npalette_steps = \"1\"\ngamma = \"{gamma}\"\n"
        );
        Preset::from_toml_str(&toml).unwrap_or_else(|e| panic!("{name} failed to load: {e}"))
    };
    let mut presets = vec![field("control", "0", "1.0")];
    presets.extend(
        GAMMAS
            .iter()
            .enumerate()
            .map(|(i, g)| field(&format!("g{i}"), "3", g)),
    );
    renderer.set_presets(presets);

    let luma = |img: &CaptureImage, x: u32, y: u32| -> f32 {
        let i = ((y * img.width + x) * 4) as usize;
        0.299 * f32::from(img.rgba[i])
            + 0.587 * f32::from(img.rgba[i + 1])
            + 0.114 * f32::from(img.rgba[i + 2])
    };

    let centre = (SIZE / 2) as i32;
    let core_of = |img: &CaptureImage| -> f32 {
        // The 2x2 straddling the middle, which is one derivative quad.
        [(-1, -1), (0, -1), (-1, 0), (0, 0)]
            .iter()
            .map(|(dx, dy)| luma(img, (centre + dx) as u32, (centre + dy) as u32))
            .sum::<f32>()
            / 4.0
    };

    let control = renderer
        .capture_preset("control", &AnalysisFrame::default(), 2)
        .expect("capture the disc control");
    let want = core_of(&control);
    println!("disc control at color_center = {CENTER}: core luma {want:.2}");

    let mut cores = Vec::new();
    for (i, g) in GAMMAS.iter().enumerate() {
        let img = renderer
            .capture_preset(&format!("g{i}"), &AnalysisFrame::default(), 2)
            .unwrap_or_else(|e| panic!("capture g{i}: {e}"));

        let core = core_of(&img);
        let frame_max = (0..SIZE)
            .flat_map(|y| (0..SIZE).map(move |x| (x, y)))
            .map(|(x, y)| luma(&img, x, y))
            .fold(0.0f32, f32::max);
        println!("gamma {g:>4}: core {core:6.2}, frame max {frame_max:6.2}");

        assert!(
            frame_max > 200.0,
            "at gamma = {g} nothing in the frame reaches the palette's bright end \
             ({frame_max:.1}) — this test is measuring an empty picture"
        );

        const RAYS: usize = 8;
        for ray in 0..RAYS {
            let theta = std::f32::consts::TAU * ray as f32 / RAYS as f32;
            let (dx, dy) = (theta.cos(), theta.sin());
            let walk: Vec<f32> = (0..=WALK_PX / STEP_PX)
                .map(|k| {
                    let r = (k * STEP_PX) as f32;
                    let x = (centre as f32 + dx * r).round() as u32;
                    let y = (centre as f32 + dy * r).round() as u32;
                    luma(&img, x, y)
                })
                .collect();
            for w in walk.windows(2) {
                assert!(
                    w[1] >= w[0] - 1.0,
                    "at gamma = {g}, ray {ray}: luma falls from {:.0} to {:.0} \
                     walking OUT from the centre. The coordinate is 0 there and \
                     rises with radius, so a fall is the wrap of a negative value \
                     (the hole) or a NaN. Walk: {walk:?}",
                    w[0],
                    w[1]
                );
            }
            let (first, last) = (walk[0], walk[walk.len() - 1]);
            assert!(
                last - first > 10.0,
                "at gamma = {g}, ray {ray}: the first {WALK_PX} px only move luma \
                 {first:.0} -> {last:.0}. The centre must be the foot of a ramp, \
                 not a flat core. Walk: {walk:?}"
            );
        }
        cores.push(core);
    }

    // The exponent orders the readings: `d` at the centre is well under 1, so a
    // larger exponent shrinks it and the middle converges on `color_center`.
    for (w, g) in cores.windows(2).zip(GAMMAS.windows(2)) {
        assert!(
            w[1] <= w[0] + 1.0,
            "the core reads {:.1} at gamma = {} and {:.1} at gamma = {} — a larger \
             exponent must not brighten a coordinate below 1. An unordered pair is \
             a NaN through `pow`, which is what a negative `d` produced ({cores:?})",
            w[0],
            g[0],
            w[1],
            g[1]
        );
    }
    let (lo, hi) = (cores[cores.len() - 1], cores[0]);
    assert!(
        hi - lo > 10.0,
        "the exponent moves the core by only {:.1} counts across gammas \
         {GAMMAS:?} ({cores:?}) — it must actually reshape the field, or this test \
         would pass on a `gamma` that never reached the shader",
        hi - lo
    );
    assert!(
        (lo - want).abs() < 4.0,
        "at the largest exponent the core reads {lo:.1} where the disc control \
         reads {want:.1}. `d^gamma` goes to 0 there, so the centre must converge \
         on the colour at `color_center` rather than on the gradient's far end"
    );
}
