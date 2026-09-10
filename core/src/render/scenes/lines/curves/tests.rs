// Test asserts use indexing on the produced Vec; allowed here over the
// file's hot-path pragma since test code is not the render path.
#![allow(clippy::indexing_slicing)]

use super::*;

/// The base rose these tests vary one field at a time from. Named-field
/// construction is the point of [`CurveParams`]: a reader can see which lever
/// each test pulls without counting argument positions.
fn rose() -> CurveParams {
    CurveParams {
        n: 6.0,
        d: 71.0,
        phase: 0.0,
        radial_offset: 0.0,
        samples: 360,
        scale: 1.0,
        rotation: 0.0,
        draw_progress: 1.0,
        color: [1.0, 1.0, 1.0],
        width: 0.01,
        levers: Levers::default(),
    }
}

fn dist(p: [f32; 2]) -> f32 {
    (p[0] * p[0] + p[1] * p[1]).sqrt()
}

#[test]
fn rose_is_deterministic_and_capped() {
    let mut a = Vec::with_capacity(400);
    let mut b = Vec::with_capacity(400);
    maurer_rose(rose(), &mut a);
    maurer_rose(rose(), &mut b);
    assert_eq!(a, b, "same parameters yield byte-identical geometry");
    // `samples` is the chord count when fully drawn.
    assert_eq!(a.len(), 360);
}

#[test]
fn draw_progress_reveals_a_prefix() {
    let mut full = Vec::with_capacity(400);
    let mut half = Vec::with_capacity(400);
    maurer_rose(rose(), &mut full);
    maurer_rose(
        CurveParams {
            draw_progress: 0.5,
            ..rose()
        },
        &mut half,
    );
    assert!(half.len() < full.len(), "half progress draws fewer chords");
    // The drawn chords are a prefix of the full curve (same start).
    assert_eq!(half[0], full[0]);
}

#[test]
fn sampling_into_a_preallocated_buffer_does_not_grow_it() {
    let mut out = Vec::with_capacity(512);
    let cap = out.capacity();
    for frame in 0..8 {
        maurer_rose(
            CurveParams {
                n: 5.0,
                d: 97.0,
                samples: 361,
                scale: 0.8,
                rotation: frame as f32 * 0.1,
                color: [0.5; 3],
                width: 0.008,
                ..rose()
            },
            &mut out,
        );
    }
    assert_eq!(out.capacity(), cap, "resampling reused the buffer capacity");
}

/// The zero-default no-op property: with `phase = 0` and `radial_offset = 0`
/// the radius is exactly the plain `sin(n * theta)` rose, so every sampled
/// point sits at distance `|sin(n * theta)| * scale` from the origin (with no
/// screen-space rotation). This pins that the shape args reduce to the
/// pre-Plan-0028 sampler — the reason the golden fixture needs no re-bless.
#[test]
fn zero_phase_and_offset_reduce_to_the_plain_sine_rose() {
    let p = CurveParams {
        samples: 4,
        ..rose()
    };
    let mut out = Vec::with_capacity(8);
    maurer_rose(p, &mut out);
    // Segment `k` (0-indexed) ends at sampled point `k + 1`.
    for (k, seg) in out.iter().enumerate() {
        let theta = ((k + 1) as f32 * p.d).to_radians();
        let expected_r = (p.n * theta).sin();
        assert!(
            (dist(seg.b) - expected_r.abs() * p.scale).abs() < 1e-5,
            "point {} distance {} should equal |sin(n*theta)|*scale {}",
            k + 1,
            dist(seg.b),
            expected_r.abs() * p.scale,
        );
    }
}

/// **Every interior vertex is a joint, and each one reaches the point its own
/// angle asks for** (ADR-0158). Asserted on the produced extensions, not on
/// pixels: a producer that forgets a joint keeps the notch, and one that
/// computes the wrong angle now draws a *wrong-length* stroke — neither is
/// visible anywhere else in the pipeline, so only a per-producer test catches
/// them.
///
/// The expected length comes from [`expected_miter`], which measures the turn
/// with `atan2` where the producer uses a dot product, so this compares two
/// derivations rather than restating one.
///
/// [`expected_miter`]: crate::render::scenes::lines::expected_miter
#[test]
fn the_rose_extends_every_interior_vertex_of_its_chain() {
    use crate::render::scenes::lines::{MITER_SLACK, expected_miter};

    /// The half-width the walk is measured against.
    const W: f32 = 0.01;

    let mut arc = Vec::with_capacity(8);
    maurer_rose(
        CurveParams {
            samples: 3,
            ..rose()
        },
        &mut arc,
    );
    assert_eq!(arc.len(), 3, "three chords through four sampled points");
    // Which ends are joints at all: the two interior vertices, and not the
    // walk's own two ends.
    assert_eq!(
        arc.iter()
            .map(|s| (s.ext_a > 0.0, s.ext_b > 0.0))
            .collect::<Vec<_>>(),
        vec![(false, true), (true, true), (true, false)],
        "the two interior vertices are joints; the walk's own ends are free"
    );
    // Each extension stands for a genuinely shared point, and is the miter that
    // point's own interior angle asks for.
    for k in 1..arc.len() {
        assert_eq!(arc[k - 1].b, arc[k].a, "chord {k} continues the previous");
        let want = expected_miter(W, arc[k - 1].a, arc[k].a, arc[k].b);
        for (side, got) in [
            ("b of the chord before", arc[k - 1].ext_b),
            ("a", arc[k].ext_a),
        ] {
            assert!(
                (got - want).abs() <= want * MITER_SLACK,
                "vertex {k} ({side}): extension {got} against the miter {want} \
                 its own interior angle asks for"
            );
        }
    }
    // Non-vacuity: this walk's corners are sharp enough that the miter is a
    // long way from the flat half-width a blunt joint would carry.
    assert!(
        arc[1].ext_a > 1.5 * W,
        "a `d = 71` web turns hard at every sample, so its miters must be well \
         past the flat {W} — got {}, and this fixture is not separating a \
         mitred corner from a bevelled one",
        arc[1].ext_a
    );

    // A partially revealed curve ends where it actually stopped: the head of
    // the drawn prefix is a free end, not a joint into geometry that is not
    // being drawn.
    let mut half = Vec::with_capacity(16);
    maurer_rose(
        CurveParams {
            samples: 8,
            draw_progress: 0.5,
            ..rose()
        },
        &mut half,
    );
    assert_eq!(half.len(), 4, "half of eight chords");
    assert!(
        half[3].ext_a > 0.0 && half[3].ext_b == 0.0,
        "the drawing head is a free end, so draw_progress cannot push the \
         stroke past the point it reached"
    );

    // One chord is all ends and no joint.
    let mut single = Vec::with_capacity(4);
    maurer_rose(
        CurveParams {
            samples: 1,
            ..rose()
        },
        &mut single,
    );
    assert_eq!(single.len(), 1);
    assert_eq!((single[0].ext_a, single[0].ext_b), (0.0, 0.0));
}

/// A nonzero `radial_offset` shifts every sampled radius by that constant.
/// With no screen-space rotation and unit scale, distance-from-origin equals
/// `|r|`; at a point where `r > 0`, adding `offset` moves the distance by
/// exactly `offset`.
#[test]
fn radial_offset_shifts_the_radius_by_a_constant() {
    let offset = 0.5_f32;
    let base_params = CurveParams {
        samples: 4,
        ..rose()
    };
    let mut base = Vec::with_capacity(8);
    let mut shifted = Vec::with_capacity(8);
    maurer_rose(base_params, &mut base);
    maurer_rose(
        CurveParams {
            radial_offset: offset,
            ..base_params
        },
        &mut shifted,
    );
    // First sampled point (segment 0's endpoint): sin(6 * 71deg) > 0, so the
    // radius stays positive after the shift and the distance grows by `offset`.
    let d0 = dist(base[0].b);
    let d1 = dist(shifted[0].b);
    assert!(
        (d1 - d0 - offset).abs() < 1e-5,
        "radial_offset {offset} should grow the radius by that constant: {d0} -> {d1}",
    );
}

/// A nonzero `phase` reshapes the geometry — the sampled segments differ from
/// the zero-phase curve (distinct from `rotation`, which spins the finished
/// figure but preserves its structure).
#[test]
fn phase_changes_the_geometry() {
    let mut zero = Vec::with_capacity(400);
    let mut shifted = Vec::with_capacity(400);
    maurer_rose(rose(), &mut zero);
    maurer_rose(
        CurveParams {
            phase: 1.0,
            ..rose()
        },
        &mut shifted,
    );
    assert_ne!(
        zero, shifted,
        "a nonzero phase should change the curve geometry"
    );
}

/// `phase` and `rotation` are **not** the same lever, and a transposed pair
/// is now visible rather than merely compiling: rotating the finished figure
/// preserves every point's distance from the origin, while phasing the sine
/// changes the radii themselves.
#[test]
fn rotation_preserves_radii_where_phase_does_not() {
    let base_params = CurveParams {
        samples: 8,
        ..rose()
    };
    let mut base = Vec::with_capacity(16);
    let mut rotated = Vec::with_capacity(16);
    let mut phased = Vec::with_capacity(16);
    maurer_rose(base_params, &mut base);
    maurer_rose(
        CurveParams {
            rotation: 0.7,
            ..base_params
        },
        &mut rotated,
    );
    maurer_rose(
        CurveParams {
            phase: 0.7,
            ..base_params
        },
        &mut phased,
    );
    for (b, r) in base.iter().zip(&rotated) {
        assert!(
            (dist(b.b) - dist(r.b)).abs() < 1e-5,
            "rotation must not change a point's radius"
        );
    }
    assert!(
        base.iter()
            .zip(&phased)
            .any(|(b, p)| (dist(b.b) - dist(p.b)).abs() > 1e-3),
        "phase must change at least one point's radius"
    );
}

/// **The gate, on the two families a Maurer walk actually holds.**
///
/// A chord web declines the fit outright, which is what leaves every shipped
/// `d` — and the ~20 golden baselines that capture one — drawing exactly the
/// chords it always drew. A smooth rose takes it and comes back as arcs.
#[test]
fn a_chord_web_declines_the_fit_and_a_rose_takes_it() {
    let walk = |n: f32, d: f32, samples: usize| {
        let (mut points, mut pieces, mut at) = (Vec::new(), Vec::new(), Vec::new());
        let fitted = maurer_rose_pieces(
            CurveParams {
                n,
                d,
                phase: 0.0,
                radial_offset: 0.0,
                samples,
                scale: 0.9,
                rotation: 0.0,
                draw_progress: 1.0,
                color: [0.0; 3],
                width: 0.01,
                levers: Levers::default(),
            },
            &mut points,
            &mut pieces,
            &mut at,
        );
        (fitted, pieces)
    };

    // The shipped webs: `curve_nightbloom` binds 29 / 37 / 43, the golden
    // fixture and `parametric_curve`'s own default bind 71.
    for d in [29.0, 37.0, 43.0, 71.0] {
        let (fitted, pieces) = walk(7.0, d, 240);
        assert!(!fitted, "d = {d} is a chord web and must decline the fit");
        assert!(pieces.is_empty(), "d = {d} left pieces behind");
    }

    // `curve_ionwake`'s rose, which is the figure this phase exists for.
    let (fitted, pieces) = walk(5.0, 2.0, 240);
    assert!(fitted, "a d = 2 rose is a curve and must be fitted");
    assert!(
        pieces.iter().any(|p| matches!(p, Piece::Arc { .. })),
        "a fitted rose must come back as arcs"
    );
    assert!(
        pieces.len() < 239,
        "a fit that costs more than the chords it replaces is not a fit: {}",
        pieces.len()
    );
}

// ---------------------------------------------------------------------------
// The families beyond the rose
// ---------------------------------------------------------------------------

/// A walk of `family` through [`fit_walk`], with the three buffers it fills.
fn walk_of(family: CurveFamily, p: CurveParams) -> (Fit, Vec<[f32; 2]>, Vec<Piece>) {
    let (mut points, mut pieces, mut at) = (Vec::new(), Vec::new(), Vec::new());
    let fit = fit_walk(family, p, &mut points, &mut pieces, &mut at);
    (fit, points, pieces)
}

/// The signed angle from unit direction `a` to `b`, in `(-PI, PI]`.
fn turn_between(a: [f32; 2], b: [f32; 2]) -> f32 {
    (a[0] * b[1] - a[1] * b[0]).atan2(a[0] * b[0] + a[1] * b[1])
}

/// The radius past which the fit emits a straight `Line` in place of an arc —
/// `biarc`'s own `MAX_RADIUS`, restated because it is private to that module.
const FLAT_RADIUS: f32 = 64.0;

/// Every joint of `pieces` — including the wrap from the last piece to the
/// first when `closed` — shares its endpoint and its tangent: the chain is
/// **G1 all the way round**, not merely along its runs. Returns the joint count
/// so a caller can refuse a vacuous chain.
///
/// # The one bounded exception, and why it is not a tangent break
///
/// Where the outline is flatter than [`FLAT_RADIUS`] — an inflection, where the
/// curvature passes through zero — the fit draws the span as a straight line,
/// and a line's direction is its chord rather than the tangent the fit asked
/// for. The two differ by at most `asin(L / 2R)` for a line of length `L`,
/// which is the bound asserted at a joint touching a line. Arc to arc, the
/// construction is exact and the bound is f32 rounding.
fn assert_g1(pieces: &[Piece], closed: bool, label: &str) -> usize {
    let joints = if closed {
        pieces.len()
    } else {
        pieces.len() - 1
    };
    let flat = |piece: Piece| match piece {
        Piece::Line { a, b } => (super::dist(a, b) / (2.0 * FLAT_RADIUS)).asin(),
        Piece::Arc { .. } => 0.0,
    };
    for k in 0..joints {
        let (here, next) = (pieces[k], pieces[(k + 1) % pieces.len()]);
        let gap = super::dist(here.end_point(), next.start_point());
        assert!(
            gap < 1e-4,
            "{label}: joint {k} opens a {gap} gap between one piece and the next"
        );
        let bend = turn_between(here.end_tangent(), next.start_tangent()).abs();
        let allowed = 1e-3 + flat(here) + flat(next);
        assert!(
            bend < allowed,
            "{label}: joint {k} breaks the tangent by {} degrees between {here:?} and {next:?}",
            bend.to_degrees()
        );
    }
    joints
}

/// The Lissajous base these tests vary from: the 3:2 figure of the done-when.
fn lissajous() -> CurveParams {
    CurveParams {
        n: 3.0,
        d: 2.0,
        samples: 360,
        scale: 0.9,
        ..rose()
    }
}

/// Plan 0162 Phase 1's done-when: `n = 3`, `d = 2` draws the 3:2 Lissajous
/// figure as **one closed G1 arc chain with no tangent break** — the wrap
/// included, which is the joint an open fit would have left as two free ends.
///
/// "The 3:2 figure" is asserted on the walk, point by point against the formula
/// computed here, so a sampler that drew some other smooth closed curve fails.
#[test]
fn a_three_two_lissajous_is_one_closed_g1_chain() {
    let p = lissajous();
    let (fit, points, pieces) = walk_of(CurveFamily::Lissajous, p);
    assert!(
        fit.fitted,
        "a Lissajous is a curve and always takes the fit"
    );
    assert!(fit.closed, "whole frequencies close over one turn");
    assert_eq!(
        points.len(),
        p.samples,
        "a closed walk leaves out the repeated start, which the wrap supplies"
    );

    let step = std::f32::consts::TAU / p.samples as f32;
    for (k, point) in points.iter().enumerate() {
        let t = step * k as f32;
        let want = [(3.0 * t).sin() * p.scale, (2.0 * t).sin() * p.scale];
        assert!(
            super::dist(*point, want) < 1e-5,
            "sample {k} sits at {point:?}, not on the 3:2 figure's {want:?}"
        );
    }

    assert!(
        pieces
            .iter()
            .any(|piece| matches!(piece, Piece::Arc { .. })),
        "a fitted Lissajous comes back as arcs"
    );
    assert!(
        pieces.len() < p.samples,
        "the fit must cost fewer pieces than the {} chords it replaces, got {}",
        p.samples,
        pieces.len()
    );
    let joints = assert_g1(&pieces, true, "3:2 Lissajous");
    assert_eq!(joints, pieces.len(), "the wrap is a joint like any other");
}

/// Closure is read off the walk, not assumed from the family: a fractional
/// frequency never returns to its start, and a partial reveal is always open so
/// its drawing head stays a free end.
#[test]
fn a_lissajous_closes_only_when_the_whole_trace_returns_to_its_start() {
    let (fit, points, _) = walk_of(
        CurveFamily::Lissajous,
        CurveParams {
            n: 3.4,
            ..lissajous()
        },
    );
    assert!(fit.fitted && !fit.closed, "a 3.4:2 trace does not close");
    assert_eq!(points.len(), 361, "an open walk keeps both its ends");

    let (fit, points, _) = walk_of(
        CurveFamily::Lissajous,
        CurveParams {
            draw_progress: 0.5,
            ..lissajous()
        },
    );
    assert!(fit.fitted && !fit.closed, "half a 3:2 figure is open");
    assert_eq!(points.len(), 181, "half of 360 chords, plus the start");
}

/// Adding the Lissajous arm left the rose exactly where it was: the walk the
/// dispatch hands the fit is the walk [`maurer_rose`] draws, point for point,
/// and the verdict is still the corner share — so every rose golden, web or
/// fitted, sees the arithmetic it was blessed against.
#[test]
fn the_rose_arm_walks_exactly_the_points_its_polyline_draws() {
    for (n, d) in [(6.0, 71.0), (5.0, 2.0)] {
        let p = CurveParams {
            n,
            d,
            samples: 240,
            ..rose()
        };
        let mut chords = Vec::with_capacity(256);
        maurer_rose(p, &mut chords);
        let (fit, points, _) = walk_of(CurveFamily::MaurerRose, p);
        assert!(!fit.closed, "a Maurer walk is never closed");
        assert_eq!(points.len(), chords.len() + 1);
        for (k, chord) in chords.iter().enumerate() {
            assert_eq!(points[k], chord.a, "d = {d}: chord {k} starts off the walk");
            assert_eq!(
                points[k + 1],
                chord.b,
                "d = {d}: chord {k} ends off the walk"
            );
        }
    }
}

/// Every family round-trips through the loader by its own name, and a name
/// outside the roster is still a load error rather than a fallback to the rose.
#[test]
fn every_family_loads_by_name_and_an_unknown_one_is_rejected() {
    use crate::preset::Preset;
    use crate::render::scenes::GeneratorConfig;

    let load = |family: &str| {
        Preset::from_toml_str(&format!(
            "system = \"parametric_curve\"\nname = \"f\"\n[curve]\nfamily = \"{family}\"\n"
        ))
    };
    for family in CurveFamily::ALL {
        let preset = load(family.as_str())
            .unwrap_or_else(|e| panic!("`{}` must load: {e}", family.as_str()));
        assert!(
            matches!(
                preset.config,
                Some(GeneratorConfig::Curve { family: loaded }) if loaded == family
            ),
            "`{}` loaded as {:?}",
            family.as_str(),
            preset.config
        );
    }
    for unknown in ["lissajou", "Lissajous", "spirograph", ""] {
        assert!(
            load(unknown).is_err(),
            "the unknown family `{unknown}` must be rejected, not defaulted"
        );
    }
}

/// The polyline a declining family draws chains every vertex, and a closed
/// walk's wrap is a joint too — no free end where it meets itself.
#[test]
fn a_closed_polyline_joins_its_wrap_and_an_open_one_leaves_its_ends_free() {
    let square = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
    let p = CurveParams {
        width: 0.01,
        ..rose()
    };
    let mut out = Vec::new();
    polyline_of(&p, &square, true, &mut out);
    assert_eq!(out.len(), 4, "a closed walk of four points is four chords");
    assert_eq!(out[3].b, out[0].a, "the last chord returns to the first");
    for (k, seg) in out.iter().enumerate() {
        assert!(
            seg.ext_a > 0.0 && seg.ext_b > 0.0,
            "chord {k}: every end of a closed polyline is a joint"
        );
    }

    polyline_of(&p, &square, false, &mut out);
    assert_eq!(out.len(), 3, "an open walk of four points is three chords");
    assert_eq!(out[0].ext_a, 0.0, "the open walk's first end is free");
    assert_eq!(out[2].ext_b, 0.0, "and so is its last");
}

/// A spirograph: a circle one fifth the size rolling inside the fixed one, five
/// cusps' worth of walk, the pen at `pen` rolling radii.
fn spirograph(pen: f32) -> CurveParams {
    CurveParams {
        n: 5.0,
        d: 5.0,
        samples: 360,
        levers: Levers {
            pen,
            ..Levers::default()
        },
        ..rose()
    }
}

/// Joints of `pieces` (wrapping when `closed`) that turn by more than 30
/// degrees — the corners the fit kept.
fn corners(pieces: &[Piece], closed: bool) -> usize {
    let joints = if closed {
        pieces.len()
    } else {
        pieces.len() - 1
    };
    (0..joints)
        .filter(|&k| {
            let (a, b) = (pieces[k], pieces[(k + 1) % pieces.len()]);
            turn_between(a.end_tangent(), b.start_tangent()).abs() > 30f32.to_radians()
        })
        .count()
}

/// Plan 0162 Phase 2's first done-when: the classic spirograph figure. A
/// 5:1 roll with the pen inside the rolling circle closes after one turn of the
/// fixed circle into a five-lobed figure that is exactly **five-fold
/// symmetric** — a fifth of a turn maps the walk onto itself, sample for
/// sample — and whose every point lies in the annulus the construction allows:
/// `(1 - r) - pen` to `(1 - r) + pen` from the centre, over the extent.
#[test]
fn a_hypotrochoid_draws_the_classic_spirograph_figure() {
    let p = spirograph(0.6);
    let (fit, points, pieces) = walk_of(CurveFamily::Hypotrochoid, p);
    assert!(
        fit.fitted && fit.closed,
        "a 5:1 roll over five cusps closes"
    );
    assert_eq!(points.len(), p.samples);

    let (sin, cos) = (std::f32::consts::TAU / 5.0).sin_cos();
    let fifth = p.samples / 5;
    for (j, q) in points.iter().enumerate() {
        let turned = [q[0] * cos - q[1] * sin, q[0] * sin + q[1] * cos];
        let there = points[(j + fifth) % points.len()];
        assert!(
            super::dist(turned, there) < 1e-4,
            "sample {j} turned a fifth of a turn lands at {turned:?}, not on sample \
             {}'s {there:?}",
            (j + fifth) % points.len()
        );
    }

    let (rolling, pen) = (0.2f32, 0.6 * 0.2);
    let extent = (1.0 - rolling) + pen;
    let (inner, outer) = (((1.0 - rolling) - pen) / extent, 1.0);
    for (j, q) in points.iter().enumerate() {
        let r = dist(*q);
        assert!(
            r >= inner - 1e-4 && r <= outer + 1e-4,
            "sample {j} sits at radius {r}, outside the construction's {inner}..{outer}"
        );
    }
    assert!(
        (dist(points[0]) - outer).abs() < 1e-5,
        "the walk starts on a lobe's outermost point"
    );

    assert_g1(&pieces, true, "curtate spirograph");
    assert_eq!(corners(&pieces, true), 0, "a curtate trace has no cusp");
}

/// Plan 0162 Phase 2's second done-when: `pen` sweeps continuously from the
/// curtate form through the cusped one into the prolate one **without the
/// sampler folding or the fitter breaking**.
///
/// "Not folding" is continuity: a small step in `pen` moves every sample a
/// small distance, because the figure is divided by an extent that is itself
/// continuous in `pen`. "Not breaking" is that every step still fits into one
/// closed chain whose pieces meet end to end and that costs fewer pieces than
/// the chords it replaces — and that the chain is G1 everywhere the figure has
/// no cusp, while at `pen = 1` it keeps exactly the five cusps the figure has.
#[test]
fn pen_sweeps_from_curtate_through_prolate_without_folding_or_breaking_the_fit() {
    let mut previous: Option<Vec<[f32; 2]>> = None;
    let mut swept = 0;
    for step in 4..=40 {
        let pen = step as f32 * 0.05;
        let label = format!("pen = {pen}");
        let p = spirograph(pen);
        let (fit, points, pieces) = walk_of(CurveFamily::Hypotrochoid, p);
        assert!(
            fit.fitted && fit.closed,
            "{label}: the walk must fit and close"
        );
        for q in &points {
            assert!(
                q[0].is_finite() && q[1].is_finite() && dist(*q) <= 1.0 + 1e-4,
                "{label}: the vertex {q:?} is not a finite point in the unit disc"
            );
        }
        assert!(
            pieces.len() < p.samples,
            "{label}: the fit cost {} pieces for {} chords",
            pieces.len(),
            p.samples
        );
        for k in 0..pieces.len() {
            let (a, b) = (pieces[k], pieces[(k + 1) % pieces.len()]);
            assert!(
                super::dist(a.end_point(), b.start_point()) < 1e-4,
                "{label}: the chain opens a gap at joint {k}"
            );
        }
        if (pen - 1.0).abs() < 1e-3 {
            assert_eq!(
                corners(&pieces, true),
                5,
                "{label}: the cycloid proper keeps its five cusps as corners"
            );
        } else if (pen - 1.0).abs() >= 0.4 {
            assert_g1(&pieces, true, &label);
        }
        if let Some(before) = &previous {
            let moved = before
                .iter()
                .zip(&points)
                .map(|(a, b)| super::dist(*a, *b))
                .fold(0.0f32, f32::max);
            assert!(
                moved < 0.05,
                "{label}: a 0.05 step in pen moved a sample {moved}, so the figure folded"
            );
        }
        previous = Some(points);
        swept += 1;
    }
    assert_eq!(swept, 37, "the sweep must run curtate to prolate");
}

/// Plan 0162 Phase 2's third done-when: a **negative `n` rolls the circle
/// outside** and draws the epicycloid. The two forms are told apart by where
/// their cusps sit: a hypocycloid's point outward from a figure inside the
/// fixed circle, an epicycloid's point inward from a figure wrapped round it.
#[test]
fn a_negative_n_rolls_outside_and_draws_the_epicycloid() {
    let at = |n: f32| {
        let p = CurveParams {
            n,
            d: 4.0,
            samples: 400,
            levers: Levers {
                pen: 1.0,
                ..Levers::default()
            },
            ..rose()
        };
        walk_of(CurveFamily::Hypotrochoid, p)
    };

    let (fit, epi, pieces) = at(-4.0);
    assert!(fit.fitted && fit.closed);
    // Rolling radius a quarter, outside: the fixed circle sits at
    // `1 / (1 + 2r)` of the extent, and the cusps touch it.
    let fixed = 1.0 / (1.0 + 2.0 * 0.25);
    let start = dist(epi[0]);
    assert!(
        (start - fixed).abs() < 1e-5,
        "the epicycloid's first cusp must touch the fixed circle at {fixed}, got {start}"
    );
    for (j, q) in epi.iter().enumerate() {
        assert!(
            dist(*q) >= fixed - 1e-4,
            "sample {j} of the epicycloid dips inside the fixed circle"
        );
    }
    assert_eq!(corners(&pieces, true), 4, "four cusps, pointing inward");

    let (fit, hypo, pieces) = at(4.0);
    assert!(fit.fitted && fit.closed);
    assert!(
        (dist(hypo[0]) - 1.0).abs() < 1e-5,
        "the hypocycloid's first cusp is its outermost point"
    );
    for (j, q) in hypo.iter().enumerate() {
        assert!(
            dist(*q) <= 1.0 + 1e-4,
            "sample {j} of the hypocycloid leaves the fixed circle"
        );
    }
    assert_eq!(corners(&pieces, true), 4, "four cusps, pointing outward");
}

/// No hypotrochoid inside the declared ranges — nor one fed a non-finite
/// expression result — emits a non-finite vertex, including the degenerate
/// rolls: `n = 0` (clamped to the smallest ratio) and `n = 1` rolling inside,
/// whose pen never leaves one point.
#[test]
fn no_hypotrochoid_emits_a_non_finite_vertex() {
    for n in [
        -8.0,
        -2.5,
        -1.0,
        -0.1,
        0.0,
        0.1,
        1.0,
        2.0,
        2.5,
        8.0,
        f32::NAN,
    ] {
        for d in [1.0, 5.0, 24.0, f32::INFINITY] {
            for pen in [0.0, 0.5, 1.0, 2.0, f32::NAN] {
                let (_, points, pieces) = walk_of(
                    CurveFamily::Hypotrochoid,
                    CurveParams {
                        n,
                        d,
                        levers: Levers {
                            pen,
                            ..Levers::default()
                        },
                        ..rose()
                    },
                );
                assert!(
                    points.iter().flatten().all(|c| c.is_finite()),
                    "n = {n}, d = {d}, pen = {pen} emitted a non-finite vertex"
                );
                assert!(
                    pieces
                        .iter()
                        .all(|piece| piece.start_point().iter().all(|c| c.is_finite())),
                    "n = {n}, d = {d}, pen = {pen} fitted a non-finite piece"
                );
            }
        }
    }
}

/// A superformula at `sym`, `sharpness` and `lobe`, with `d = 1` — no skew, so
/// `n2 = n3` and the figure has its full `sym`-fold symmetry.
fn superformula(sym: f32, sharpness: f32, lobe: f32) -> CurveParams {
    CurveParams {
        d: 1.0,
        samples: 400,
        levers: Levers {
            sym,
            sharpness,
            lobe,
            ..Levers::default()
        },
        ..rose()
    }
}

/// The local maxima of the distance from the centre around a **closed** walk —
/// a figure's lobes, counted as a viewer counts them.
fn lobes(points: &[[f32; 2]]) -> usize {
    let n = points.len();
    let r = |k: usize| dist(points[k % n]);
    (0..n)
        .filter(|&k| r(k) > r(k + n - 1) && r(k) >= r(k + 1))
        .count()
}

/// Plan 0162 Phase 3's first done-when: `sym = 5` at a low `sharpness` draws a
/// **five-lobed star**, and a high `sharpness` rounds the same figure toward a
/// **circle**.
///
/// "Star" is three properties, each a count or a ratio rather than a look: five
/// lobes, a deep waist between them (the innermost point well inside the
/// outermost), and a pointed tip at each lobe that the fit keeps as a corner.
/// "Toward a circle" is the waist closing: every point within a few percent of
/// one radius.
#[test]
fn a_low_sharpness_draws_a_five_lobed_star_and_a_high_one_a_near_circle() {
    let (fit, star, pieces) = walk_of(CurveFamily::Superformula, superformula(5.0, 0.3, 1.0));
    assert!(fit.fitted && fit.closed, "a whole sym closes over one turn");
    assert_eq!(lobes(&star), 5, "sym = 5 must draw five lobes");
    let (inner, outer) = star
        .iter()
        .map(|p| dist(*p))
        .fold((f32::MAX, 0.0f32), |(lo, hi), r| (lo.min(r), hi.max(r)));
    assert!(
        inner < 0.5 * outer,
        "a star's waist must sit well inside its tips: {inner} against {outer}"
    );
    assert_eq!(
        corners(&pieces, true),
        5,
        "each of the five tips is a point, which the fit keeps as a corner"
    );

    // Five-fold: a fifth of a turn maps the walk onto itself, sample for sample.
    let (sin, cos) = (std::f32::consts::TAU / 5.0).sin_cos();
    let fifth = star.len() / 5;
    for (j, q) in star.iter().enumerate() {
        let turned = [q[0] * cos - q[1] * sin, q[0] * sin + q[1] * cos];
        assert!(
            super::dist(turned, star[(j + fifth) % star.len()]) < 1e-4,
            "sample {j} is not carried onto its fifth-turn image"
        );
    }

    let (fit, round, pieces) = walk_of(CurveFamily::Superformula, superformula(5.0, 20.0, 1.0));
    assert!(fit.fitted && fit.closed);
    let (inner, outer) = round
        .iter()
        .map(|p| dist(*p))
        .fold((f32::MAX, 0.0f32), |(lo, hi), r| (lo.min(r), hi.max(r)));
    assert!(
        inner > 0.97 * outer,
        "a high sharpness must round the figure toward a circle: {inner} against {outer}"
    );
    assert_eq!(corners(&pieces, true), 0, "a near-circle has no tip left");
}

/// Plan 0162 Phase 3's second done-when, and its degenerate-figure risk stated
/// as a property: **no parameter combination inside the declared ranges emits a
/// non-finite vertex**, and every vertex lies inside the stated radius — the
/// figure's `scale`. Swept past the ranges too, including the exponents' poles:
/// `sharpness` at and below zero, which sends a naive `r` to infinity.
#[test]
fn no_superformula_emits_a_non_finite_vertex_or_leaves_its_radius() {
    let mut walks = 0;
    for sym in [0.0, 1.0, 2.0, 3.0, 5.0, 8.0, 13.0, 24.0, 7.4] {
        for sharpness in [0.0, 0.01, 0.1, 0.5, 1.0, 5.0, 20.0, -1.0, f32::NAN] {
            for lobe in [0.0, 0.1, 1.0, 5.0, 10.0, 40.0] {
                for d in [0.0, 0.5, 1.0, 4.0, 71.0, f32::INFINITY] {
                    let p = CurveParams {
                        d,
                        samples: 240,
                        scale: 0.9,
                        levers: Levers {
                            sym,
                            sharpness,
                            lobe,
                            ..Levers::default()
                        },
                        ..rose()
                    };
                    let (_, points, pieces) = walk_of(CurveFamily::Superformula, p);
                    let label = format!("sym {sym}, sharpness {sharpness}, lobe {lobe}, d {d}");
                    for q in &points {
                        assert!(
                            q[0].is_finite() && q[1].is_finite(),
                            "{label}: non-finite vertex {q:?}"
                        );
                        assert!(
                            dist(*q) <= p.scale * (1.0 + 1e-5),
                            "{label}: vertex {q:?} lies outside the radius {}",
                            p.scale
                        );
                    }
                    assert!(
                        pieces.iter().all(|piece| {
                            let [a, b] = [piece.start_point(), piece.end_point()];
                            a.iter().chain(&b).all(|c| c.is_finite())
                        }),
                        "{label}: the fit produced a non-finite piece"
                    );
                    walks += 1;
                }
            }
        }
    }
    assert_eq!(walks, 9 * 9 * 6 * 6);
}

/// `d` skews the two exponents apart: at `d = 1` the lobes are symmetric about
/// their tips, and away from it the figure changes shape. Pinned so the skew is
/// a lever that reaches the sampler rather than a documented one that does not.
#[test]
fn d_skews_the_superformula_lobes_apart() {
    let symmetric = walk_of(CurveFamily::Superformula, superformula(4.0, 1.0, 2.0)).1;
    let skewed = walk_of(
        CurveFamily::Superformula,
        CurveParams {
            d: 3.0,
            ..superformula(4.0, 1.0, 2.0)
        },
    )
    .1;
    let moved = symmetric
        .iter()
        .zip(&skewed)
        .map(|(a, b)| super::dist(*a, *b))
        .fold(0.0f32, f32::max);
    assert!(moved > 0.05, "a d of 3 moved no sample past {moved}");
}

/// Plan 0162 Phase 3's third done-when: `sym` bound under `[hold] sym = "bar"`
/// **changes the figure's symmetry once a bar and holds it between**.
///
/// Composed from the real pieces rather than restated: the loader reads the
/// preset and folds its `[hold]` table and `sym`'s `Structural` kind into the
/// binding, the render layer's own hold decides which frame's value stands, the
/// kind rounds it, and the sampler draws it — and the lobe count is read off
/// the drawn walk. The binding moves **every frame**, so a hold that leaked
/// would show more than one figure inside a bar.
#[test]
fn sym_held_on_the_bar_changes_the_figure_once_a_bar() {
    use crate::dsp::AnalysisFrame;
    use crate::preset::{HoldEdge, Preset, Variables};
    use crate::render::roster::ParamHold;
    use crate::render::scenes::ParamKind;

    let preset = Preset::from_toml_str(
        "system = \"parametric_curve\"\nname = \"held\"\n\
         [curve]\nfamily = \"superformula\"\n\
         [params]\nsym = \"3 + floor(time * 8)\"\nsharpness = \"0.3\"\nd = \"1\"\n\
         [hold]\nsym = \"bar\"\n",
    )
    .expect("a held superformula loads");
    let (index, binding) = preset
        .params
        .iter()
        .enumerate()
        .find(|(_, b)| b.name == "sym")
        .expect("sym is bound");
    assert_eq!(
        binding.hold,
        Some(HoldEdge::Bar),
        "the hold reached the binding"
    );
    assert_eq!(binding.kind, ParamKind::Structural, "sym rounds as a count");

    let mut hold = ParamHold::default();
    let mut per_bar: Vec<Vec<usize>> = vec![Vec::new(); 3];
    let mut raw_seen = Vec::new();
    for frame in 0..12 {
        let bar = frame / 4;
        // An eighth of a second a frame, exact in binary, so the binding reads
        // `3 + frame` with no rounding at the floor.
        let time = frame as f32 / 8.0;
        let raw = binding.expr.eval(&Variables::new(
            0.0, 0.0, 0.0, 0.0, 0.0, 0.0, time, 0.0, 0.0,
        ));
        raw_seen.push(raw);
        let analysis = AnalysisFrame {
            bar_index: bar as u32,
            ..AnalysisFrame::default()
        };
        let sym = binding
            .kind
            .quantize(hold.hold(index, raw, binding.hold, &analysis, time));
        let (_, points, _) = walk_of(CurveFamily::Superformula, superformula(sym, 0.3, 1.0));
        per_bar[bar].push(lobes(&points));
    }
    assert!(
        raw_seen.windows(2).all(|w| w[0] != w[1]),
        "the binding must move every frame for the hold to have anything to hold: {raw_seen:?}"
    );
    for (bar, figures) in per_bar.iter().enumerate() {
        assert!(
            figures.iter().all(|&f| f == figures[0]),
            "bar {bar} showed {figures:?} lobes, so the symmetry moved inside a bar"
        );
    }
    let shown: Vec<usize> = per_bar.iter().map(|f| f[0]).collect();
    assert_eq!(
        shown,
        vec![3, 7, 11],
        "each bar must show the symmetry its own first frame evaluated to"
    );
}
