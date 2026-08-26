//! Unit tests for the biarc fit.
//!
//! The load-bearing one is [`chain_is_g1_at_every_joint_that_is_not_a_corner`]:
//! ADR-0098's whole argument is that tangent continuity, not sample density, is
//! what makes a chain read as a curve, so the property is asserted directly
//! rather than taken on trust from the construction.

#![allow(clippy::indexing_slicing)]

use super::*;

/// A closed circle of `n` samples, radius `r`.
fn circle(n: usize, r: f32) -> Vec<[f32; 2]> {
    (0..n)
        .map(|k| {
            let t = TAU * k as f32 / n as f32;
            [r * t.cos(), r * t.sin()]
        })
        .collect()
}

/// A closed ellipse — the simplest outline whose curvature actually varies, so
/// one arc cannot carry it and the fit has to work.
fn ellipse(n: usize, a: f32, b: f32) -> Vec<[f32; 2]> {
    (0..n)
        .map(|k| {
            let t = TAU * k as f32 / n as f32;
            [a * t.cos(), b * t.sin()]
        })
        .collect()
}

/// `star.rs`'s `petal` outline, at an arbitrary sample count — the roster
/// member with a point of unbounded curvature at each end.
fn petal(n: usize) -> Vec<[f32; 2]> {
    (0..n)
        .map(|k| {
            let t = TAU * k as f32 / n as f32;
            let s = t.sin();
            [0.5 * t.cos(), 0.30 * s.signum() * s.abs().powf(1.6)]
        })
        .collect()
}

/// A Maurer walk: `samples` points at a fixed angular step `d` degrees on
/// `r = sin(n * theta)`. Large `d` is the chord web; small `d` is a rose.
fn maurer(n: f32, d: f32, samples: usize) -> Vec<[f32; 2]> {
    (0..samples)
        .map(|k| {
            let theta = (k as f32 * d).to_radians();
            let r = (n * theta).sin();
            [r * theta.cos(), r * theta.sin()]
        })
        .collect()
}

/// The tests fit at **one pixel at 1080p**, which is `star.rs`'s budget quoted
/// at the scale a motif is authored in rather than the one it is placed at —
/// the strictest thing either shipped caller asks for.
const TEST_BUDGET: f32 = PIXEL_1080P;

fn run(points: &[[f32; 2]], closed: bool) -> (Vec<Piece>, Vec<f32>, FitStats) {
    run_at(points, closed, TEST_BUDGET)
}

fn run_at(points: &[[f32; 2]], closed: bool, lateral: f32) -> (Vec<Piece>, Vec<f32>, FitStats) {
    let (mut out, mut at) = (Vec::new(), Vec::new());
    let stats = fit(points, closed, lateral, &mut out, &mut at);
    (out, at, stats)
}

/// Joints whose two pieces do not share a tangent direction — the tangent
/// discontinuities the chain still contains.
fn tangent_breaks(pieces: &[Piece], closed: bool) -> usize {
    let joints = if closed {
        pieces.len()
    } else {
        pieces.len().saturating_sub(1)
    };
    (0..joints)
        .filter(|&k| {
            let a = pieces[k];
            let b = pieces[(k + 1) % pieces.len()];
            turn(a.end_tangent(), b.start_tangent()).abs() > 1e-3
        })
        .count()
}

#[test]
fn a_circle_fits_within_the_budget_and_stays_round() {
    let points = circle(FIT_SAMPLES, 0.5);
    let (pieces, _, stats) = run(&points, true);
    assert!(
        stats.max_deviation <= TEST_BUDGET,
        "deviation {} over budget",
        stats.max_deviation
    );
    assert_eq!(stats.corners, 0, "a circle has no corners");
    // Every piece is an arc of the source circle: the fit is exact here, so the
    // radius and centre come back to what they were sampled from.
    for piece in &pieces {
        match *piece {
            Piece::Arc { centre, radius, .. } => {
                assert!(
                    (radius - 0.5).abs() < 2e-3,
                    "radius {radius} is not the circle's"
                );
                assert!(norm(centre) < 2e-3, "centre {centre:?} is not the origin");
            }
            Piece::Line { .. } => panic!("a circle fitted to a straight line"),
        }
    }
}

#[test]
fn chain_is_g1_at_every_joint_that_is_not_a_corner() {
    // The three closed outlines the roster fits, plus the two shapes that make
    // the property non-trivial: an ellipse (curvature varies) and a rose (the
    // per-frame path).
    for (name, points, closed) in [
        ("circle", circle(FIT_SAMPLES, 0.5), true),
        ("ellipse", ellipse(FIT_SAMPLES, 0.5, 0.18), true),
        ("petal", petal(FIT_SAMPLES), true),
        ("rose", maurer(5.0, 2.0, 240), false),
    ] {
        let (pieces, _, stats) = run(&points, closed);
        assert_eq!(
            tangent_breaks(&pieces, closed),
            stats.corners,
            "{name}: the only tangent discontinuities left must be the figure's own corners"
        );
        // Every joint is also positionally continuous: the chain is one curve,
        // not a scatter of arcs near one.
        for k in 0..pieces.len().saturating_sub(1) {
            let gap = dist(pieces[k].end_point(), pieces[k + 1].start_point());
            assert!(gap < 1e-4, "{name}: joint {k} is {gap} apart");
        }
    }
}

#[test]
fn both_budgets_hold_on_every_fitted_outline() {
    for (name, points, closed) in [
        ("ellipse", ellipse(FIT_SAMPLES, 0.5, 0.18), true),
        ("petal", petal(FIT_SAMPLES), true),
        ("rose", maurer(5.0, 2.0, 240), false),
    ] {
        let (_, _, stats) = run(&points, closed);
        assert!(
            stats.max_tangent_err <= TANGENT_BUDGET,
            "{name}: tangent error {} over {TANGENT_BUDGET}",
            stats.max_tangent_err
        );
        assert!(
            stats.max_deviation <= TEST_BUDGET,
            "{name}: deviation {} over {TEST_BUDGET}",
            stats.max_deviation
        );
    }
}

#[test]
fn an_all_corner_walk_comes_back_as_its_own_polyline() {
    // The degenerate case, and the reason the fit is safe to hand any walk: a
    // run with a corner at every vertex is a run of one chord, and one chord
    // with no interior sample is reproduced rather than approximated.
    //
    // **A Maurer chord web is NOT this case**, which is the finding that put
    // the smoothness decision in the caller: a `d = 29` walk is ~90 % corners
    // and the fit turns the remaining tenth into arcs. `curves` gates on
    // `corner_fraction` for exactly that reason.
    let zigzag: Vec<[f32; 2]> = (0..24)
        .map(|k| [k as f32 * 0.05, if k % 2 == 0 { 0.0 } else { 0.4 }])
        .collect();
    let (pieces, _, stats) = run(&zigzag, false);
    assert_eq!(stats.arcs, 0, "a zigzag fitted to arcs");
    assert_eq!(
        pieces.len(),
        zigzag.len() - 1,
        "the fit changed the chord count"
    );
    for (k, piece) in pieces.iter().enumerate() {
        assert_eq!(
            *piece,
            Piece::Line {
                a: zigzag[k],
                b: zigzag[k + 1]
            },
            "chord {k} came back changed"
        );
    }
    // And the measurement the caller's gate rests on, stated as a fact about
    // the web rather than as a hope: the fit alone does not preserve one.
    let web = maurer(7.0, 29.0, 240);
    assert!(
        run(&web, false).2.arcs > 0,
        "if a web survived the fit untouched the gate would be unnecessary"
    );
}

#[test]
fn the_corner_fraction_separates_a_web_from_a_rose() {
    // The number `parametric_curve` gates on. The gap between the two families
    // is the whole reason a threshold can exist at all.
    let rose = corner_fraction(&maurer(5.0, 2.0, 240), false);
    assert!(rose < 0.15, "a d = 2 rose reads as {rose} corners");
    for d in [29.0, 37.0, 43.0, 71.0] {
        let web = corner_fraction(&maurer(7.0, d, 240), false);
        assert!(web > 0.85, "a d = {d} web reads as only {web} corners");
    }
}

#[test]
fn corners_break_the_chain_and_stay_corners() {
    // A square: four corners, no curve anywhere, and the fit must not round it.
    let square = vec![[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]];
    let (pieces, _, stats) = run(&square, true);
    assert_eq!(stats.corners, 4);
    assert_eq!(stats.arcs, 0);
    assert_eq!(pieces.len(), 4);
}

#[test]
fn the_fit_is_a_pure_function_of_its_outline() {
    // Determinism (NFR 6): the same outline twice, piece for piece and bit for
    // bit — no clock, no accumulated state, nothing to drift.
    let points = petal(FIT_SAMPLES);
    let (first, first_at, first_stats) = run(&points, true);
    let (second, second_at, second_stats) = run(&points, true);
    assert_eq!(first, second);
    assert_eq!(first_at, second_at);
    assert_eq!(first_stats, second_stats);
}

#[test]
fn a_degenerate_outline_yields_nothing_rather_than_a_panic() {
    for points in [vec![], vec![[0.0, 0.0]]] {
        let (pieces, at, stats) = run(&points, true);
        assert!(pieces.is_empty());
        assert!(at.is_empty());
        assert_eq!(stats.arcs + stats.lines, 0);
    }
    // Two coincident samples: a chord of zero length, which the arc
    // construction cannot turn into a circle and must not divide by.
    let (pieces, _, _) = run(&[[0.2, 0.3], [0.2, 0.3]], false);
    for piece in pieces {
        assert!(piece.start_point()[0].is_finite());
        assert!(piece.start_point()[1].is_finite());
    }
}

#[test]
fn a_flat_run_is_a_line_rather_than_a_vast_circle() {
    // `abs(length(p - c) - r)` at `r = 1e6` resolves to 0.06 in f32, twenty
    // stroke widths — so a span this flat has to leave the arc path entirely.
    let points: Vec<[f32; 2]> = (0..8).map(|k| [k as f32 * 0.1, 0.0]).collect();
    let (pieces, _, stats) = run(&points, false);
    assert_eq!(stats.arcs, 0, "a straight run fitted to arcs");
    assert!(!pieces.is_empty());
}

#[test]
fn each_piece_carries_where_it_sits_along_the_walk() {
    let points = maurer(5.0, 2.0, 240);
    let (pieces, at, _) = run(&points, false);
    assert_eq!(pieces.len(), at.len());
    // Monotone and inside the walk: the colour ramp reads this as its axis, so
    // an out-of-order entry would run the palette backwards mid-figure.
    for k in 1..at.len() {
        assert!(at[k] >= at[k - 1], "walk position went backwards at {k}");
    }
    assert!(at.first().copied().unwrap_or(-1.0) >= 0.0);
    assert!(at.last().copied().unwrap_or(f32::MAX) <= points.len() as f32);
}
