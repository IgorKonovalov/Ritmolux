// Test asserts use indexing on the produced Vec; allowed here over the
// file's hot-path pragma since test code is not the render path.
#![allow(clippy::indexing_slicing)]

use super::*;

/// The base rose these tests vary one field at a time from. Named-field
/// construction is the point of [`RoseParams`]: a reader can see which lever
/// each test pulls without counting argument positions.
fn rose() -> RoseParams {
    RoseParams {
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
        RoseParams {
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
            RoseParams {
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
    let p = RoseParams {
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

/// Plan 0039 Phase 3 done-when 1 and 4 (ADR-0041). Asserted on the **flag
/// pattern**, not on pixels: a producer that silently forgets to flag its
/// joints keeps the notch and nothing else in the pipeline notices, so only
/// a per-producer test catches it.
#[test]
fn the_rose_flags_every_interior_vertex_of_its_chain() {
    let mut arc = Vec::with_capacity(8);
    maurer_rose(
        RoseParams {
            samples: 3,
            ..rose()
        },
        &mut arc,
    );
    assert_eq!(arc.len(), 3, "three chords through four sampled points");
    assert_eq!(
        arc.iter().map(|s| s.joined).collect::<Vec<_>>(),
        vec![JOINED_B, JOINED_A | JOINED_B, JOINED_A],
        "the two interior vertices are joints; the walk's own ends are free"
    );
    // Each flag stands for a genuinely shared point.
    for k in 1..arc.len() {
        assert_eq!(arc[k - 1].b, arc[k].a, "chord {k} continues the previous");
    }

    // A partially revealed curve ends where it actually stopped: the head of
    // the drawn prefix is a free end, not a joint into geometry that is not
    // being drawn.
    let mut half = Vec::with_capacity(16);
    maurer_rose(
        RoseParams {
            samples: 8,
            draw_progress: 0.5,
            ..rose()
        },
        &mut half,
    );
    assert_eq!(half.len(), 4, "half of eight chords");
    assert_eq!(
        half[3].joined, JOINED_A,
        "the drawing head is a free end, so draw_progress cannot push the \
         stroke past the point it reached"
    );

    // One chord is all ends and no joint.
    let mut single = Vec::with_capacity(4);
    maurer_rose(
        RoseParams {
            samples: 1,
            ..rose()
        },
        &mut single,
    );
    assert_eq!(single.len(), 1);
    assert_eq!(single[0].joined, 0);
}

/// A nonzero `radial_offset` shifts every sampled radius by that constant.
/// With no screen-space rotation and unit scale, distance-from-origin equals
/// `|r|`; at a point where `r > 0`, adding `offset` moves the distance by
/// exactly `offset`.
#[test]
fn radial_offset_shifts_the_radius_by_a_constant() {
    let offset = 0.5_f32;
    let base_params = RoseParams {
        samples: 4,
        ..rose()
    };
    let mut base = Vec::with_capacity(8);
    let mut shifted = Vec::with_capacity(8);
    maurer_rose(base_params, &mut base);
    maurer_rose(
        RoseParams {
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
        RoseParams {
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
    let base_params = RoseParams {
        samples: 8,
        ..rose()
    };
    let mut base = Vec::with_capacity(16);
    let mut rotated = Vec::with_capacity(16);
    let mut phased = Vec::with_capacity(16);
    maurer_rose(base_params, &mut base);
    maurer_rose(
        RoseParams {
            rotation: 0.7,
            ..base_params
        },
        &mut rotated,
    );
    maurer_rose(
        RoseParams {
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
            RoseParams {
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
