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
