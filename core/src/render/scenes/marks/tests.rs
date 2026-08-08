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
