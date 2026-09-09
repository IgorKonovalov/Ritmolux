//! Unit tests for the `[path]` parser.
//!
//! Three groups, matching the three things the parser owes an author: it
//! **refuses** what the subset excludes and says where, it **parses** the forms
//! a real exported file uses (the relative commands and the smooth
//! continuations), and it **normalizes** the result to a recorded frame.

use super::*;

/// Parse or panic — the test-side shorthand. Every path here is hand-written and
/// meant to parse; a failure is the finding.
fn shape(d: &str) -> PathShape {
    PathShape::parse(d, 64).unwrap_or_else(|e| panic!("`{d}` should parse, got: {e}"))
}

fn err(d: &str) -> PathError {
    match PathShape::parse(d, 64) {
        Ok(_) => panic!("`{d}` should have been refused"),
        Err(e) => e,
    }
}

/// Two contours agree to within a fraction of the normalized box.
fn assert_same_contour(a: &PathShape, b: &PathShape, what: &str) {
    assert_eq!(a.points().len(), b.points().len(), "{what}: arity differs");
    for (i, (p, q)) in a.points().iter().zip(b.points()).enumerate() {
        let d = ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2)).sqrt();
        assert!(d < 1e-4, "{what}: point {i} differs by {d}: {p:?} vs {q:?}");
    }
}

// ---------------------------------------------------------------- refusals

#[test]
fn the_elliptical_arc_is_refused_by_name_at_its_own_offset() {
    let e = err("M 0,0 A 1 1 0 0 1 2,2 Z");
    assert_eq!(e.kind, PathErrorKind::EllipticalArc('A'));
    assert_eq!(e.offset, 6, "the offset points at the 'A' itself");
    let text = e.to_string();
    assert!(
        text.contains("at character 6"),
        "message leads with the offset: {text}"
    );
    assert!(
        text.contains("elliptical arc"),
        "message names what it found: {text}"
    );
    assert!(
        text.contains("cubics"),
        "message names the way out, since a browser renders this file: {text}"
    );
    // The relative form is refused on the same terms rather than falling
    // through to the unknown-command arm.
    assert_eq!(
        err("M 0,0 a 1 1 0 0 1 2,2 Z").kind,
        PathErrorKind::EllipticalArc('a')
    );
}

#[test]
fn a_second_subpath_is_refused_where_it_begins() {
    let e = err("M 0,0 L 1,0 M 2,2 L 3,3 Z");
    assert_eq!(e.kind, PathErrorKind::MultipleSubpaths);
    assert_eq!(e.offset, 12, "the offset points at the second moveto");
    assert!(
        e.to_string().contains("second subpath"),
        "message names what it found: {e}"
    );
    // A relative second moveto — the form an exported letterform's counter
    // actually takes — is refused identically.
    assert_eq!(
        err("M 0,0 L 1,0 m 2,2 l 1,1 z").kind,
        PathErrorKind::MultipleSubpaths
    );
}

#[test]
fn a_malformed_path_is_an_error_carrying_a_character_offset() {
    // Each case names the offset by hand: the point of the offset is that it is
    // the author's own column, so a test that recomputed it would assert
    // nothing.
    let cases: [(&str, usize, PathErrorKind); 5] = [
        ("M 0,0 X 1,1", 6, PathErrorKind::UnknownCommand('X')),
        ("L 1,1", 0, PathErrorKind::MissingMoveTo),
        ("1,1 L 2,2", 0, PathErrorKind::MissingMoveTo),
        ("M 0,0 L 1,", 10, PathErrorKind::ExpectedNumber),
        ("M 0,0 L 1,1 Z 5", 14, PathErrorKind::UnexpectedOperand),
    ];
    for (d, offset, kind) in cases {
        let e = err(d);
        assert_eq!(e.kind, kind, "`{d}`");
        assert_eq!(e.offset, offset, "`{d}` -> {e}");
        assert!(
            e.to_string().starts_with(&format!("at character {offset}")),
            "`{d}`: the offset leads the message, got {e}"
        );
    }
}

#[test]
fn a_path_that_encloses_no_area_is_refused_rather_than_drawn() {
    // Two distinct points is a line, and one is a dot. Both would normalize to
    // something the field could evaluate, which is exactly why neither may
    // silently become a figure.
    assert_eq!(err("M 0,0 L 1,1 Z").kind, PathErrorKind::Degenerate);
    assert_eq!(err("M 5,5 L 5,5 Z").kind, PathErrorKind::Degenerate);
}

// ------------------------------------------------------------ the subset

#[test]
fn the_relative_commands_draw_the_same_figure_as_their_absolute_forms() {
    // The forms a real exported file uses. Each pair is the same square.
    let absolute = shape("M 0,0 L 10,0 L 10,10 L 0,10 Z");
    let relative = shape("m 0,0 l 10,0 l 0,10 l -10,0 z");
    assert_same_contour(&absolute, &relative, "l vs L");

    let axis_absolute = shape("M 0,0 H 10 V 10 H 0 Z");
    let axis_relative = shape("m 0,0 h 10 v 10 h -10 z");
    assert_same_contour(&absolute, &axis_absolute, "H/V vs L");
    assert_same_contour(&axis_absolute, &axis_relative, "h/v vs H/V");

    // A cubic, absolute and relative. `c`'s control points are relative to the
    // CURRENT point, not to the previous control point — the other place a
    // hand-written parser goes wrong.
    let cubic_absolute = shape("M 0,0 C 2,0 4,2 4,4 C 4,6 2,8 0,8 Z");
    let cubic_relative = shape("m 0,0 c 2,0 4,2 4,4 c 0,2 -2,4 -4,4 z");
    assert_same_contour(&cubic_absolute, &cubic_relative, "c vs C");
}

#[test]
fn the_smooth_continuation_reflects_the_previous_control_point() {
    // `S`'s first control point is the previous cubic's SECOND control point
    // reflected about the current point. Written out, the reflection of (1,1)
    // about (2,1) is (3,1) — so these two paths are the same curve, and a
    // parser that forgot the reflection would draw a visibly different one.
    let explicit = shape("M 0,0 C 1,0 1,1 2,1 C 3,1 3,2 4,2 L 4,0 Z");
    let shorthand = shape("M 0,0 C 1,0 1,1 2,1 S 3,2 4,2 L 4,0 Z");
    assert_same_contour(&explicit, &shorthand, "S reflects the cubic handle");

    // `T` reads the previous QUADRATIC's control point: (1,1) reflected about
    // (2,0) is (3,-1).
    let q_explicit = shape("M 0,0 Q 1,1 2,0 Q 3,-1 4,0 L 4,2 L 0,2 Z");
    let q_shorthand = shape("M 0,0 Q 1,1 2,0 T 4,0 L 4,2 L 0,2 Z");
    assert_same_contour(&q_explicit, &q_shorthand, "T reflects the quad handle");

    // And the case a naive implementation gets wrong in the other direction:
    // after a command that is NOT a curve, the reflection is the current point
    // itself, which makes the shorthand a plain curve out of a stale handle
    // rather than out of nothing.
    let after_line = shape("M 0,0 L 2,0 S 3,2 4,2 L 4,0 Z");
    let as_written = shape("M 0,0 L 2,0 C 2,0 3,2 4,2 L 4,0 Z");
    assert_same_contour(&after_line, &as_written, "S after L reflects nothing");
}

#[test]
fn the_scanner_takes_the_forms_an_exported_file_actually_writes() {
    let spaced = shape("M 0,0 L 10,0 L 10,10 L 0,10 Z");
    // No separators at all between command and operands.
    assert_same_contour(&spaced, &shape("M0,0L10,0L10,10L0,10Z"), "no spaces");
    // Spaces instead of commas.
    assert_same_contour(
        &spaced,
        &shape("M 0 0 L 10 0 L 10 10 L 0 10 Z"),
        "no commas",
    );
    // A moveto's extra coordinate pairs are implicit linetos.
    assert_same_contour(
        &spaced,
        &shape("M 0,0 10,0 10,10 0,10 Z"),
        "implicit L after M",
    );
    // One command letter, several operand groups.
    assert_same_contour(&spaced, &shape("M 0,0 L 10,0 10,10 0,10 Z"), "repeated L");
    // An exponent, which an exporter writes for small numbers.
    assert_same_contour(
        &spaced,
        &shape("M 0,0 L 1e1,0 L 1e1,1e1 L 0,1e1 Z"),
        "exponent notation",
    );
    // `1.5.5` is TWO numbers — where a number ends is part of the grammar.
    assert_same_contour(
        &shape("M0,0L1.5,0.5L0,1Z"),
        &shape("M0,0L1.5.5L0,1Z"),
        "a second '.' starts a new number",
    );
    // A path with no `Z` is closed anyway: a filled silhouette has no open form.
    assert_same_contour(
        &spaced,
        &shape("M 0,0 L 10,0 L 10,10 L 0,10"),
        "implicit close",
    );
}

// ------------------------------------------------------- the normalization

#[test]
fn the_normalization_is_recorded_and_a_path_lands_in_the_same_place_at_any_scale() {
    let small = shape("M 0,0 H 10 V 10 H 0 Z");
    let large = shape("M 100,200 H 140 V 240 H 100 Z");

    // The whole point: the same drawing at another scale and offset produces the
    // same contour, so swapping one path for another does not also move the
    // figure.
    assert_same_contour(&small, &large, "scale and offset are normalized away");

    // And the transform that did it is recorded rather than inferred.
    assert_eq!(small.source_center(), [5.0, 5.0]);
    assert!((small.source_scale() - 0.2).abs() < 1e-6, "1 / half of 10");
    assert_eq!(large.source_center(), [120.0, 220.0]);
    assert!((large.source_scale() - 0.05).abs() < 1e-6, "1 / half of 40");

    // The contour itself lands in the unit box, touching both ends of it.
    let (min, max) = bounds(small.points());
    assert!(
        (min[0] + 1.0).abs() < 1e-4 && (max[0] - 1.0).abs() < 1e-4,
        "x spans [-1, 1]"
    );
    assert!(
        (min[1] + 1.0).abs() < 1e-4 && (max[1] - 1.0).abs() < 1e-4,
        "y spans [-1, 1]"
    );
}

#[test]
fn normalization_is_uniform_so_a_wide_figure_stays_wide() {
    // 20 x 10: the LONGER axis is what maps to [-1, 1], and the shorter one
    // keeps its proportion. A per-axis fit would make every silhouette square.
    let wide = shape("M 0,0 H 20 V 10 H 0 Z");
    let (min, max) = bounds(wide.points());
    assert!((max[0] - min[0] - 2.0).abs() < 1e-4, "x spans the full box");
    assert!(
        (max[1] - min[1] - 1.0).abs() < 1e-4,
        "y keeps the 2:1 proportion, got {}",
        max[1] - min[1]
    );
}

#[test]
fn points_are_spaced_by_arc_length_rather_than_per_command() {
    // A 20 x 2 rectangle: four commands, two of them ten times the length of the
    // other two. Per-command spacing would put a quarter of the points on each
    // side; arc length puts them where the outline actually is.
    let long_thin = PathShape::parse("M 0,0 H 20 V 2 H 0 Z", 64).expect("parses");
    let on_long_edges = long_thin
        .points()
        .iter()
        .filter(|p| p[0].abs() < 0.98)
        .count();
    assert!(
        on_long_edges > 50,
        "the two 20-unit edges carry the points, got {on_long_edges} of 64"
    );

    // And the spacing itself is even: every consecutive chord is one step of the
    // perimeter, short only where a corner falls between two samples (there the
    // chord cuts the corner the arc goes round).
    //
    // The step is the FIGURE's own, stated rather than measured back off the
    // output: normalization maps the 20-unit axis to [-1, 1], so the contour is
    // 2 by 0.2 and its perimeter is 4.4.
    let pts = long_thin.points();
    let step = 4.4 / pts.len() as f32;
    for i in 0..pts.len() {
        let Some((&a, &b)) = pts.get(i).zip(pts.get((i + 1) % pts.len())) else {
            continue;
        };
        let c = chord(a, b);
        assert!(
            c <= step * 1.001 && c >= step * 0.70,
            "chord {i} is {c}, step is {step}"
        );
    }
}

#[test]
fn the_signed_area_reports_the_winding_direction() {
    // Counter-clockwise in a y-up frame is positive; the same square written
    // backwards is the same figure with the opposite sign. This is the quantity
    // a morph pair has to agree on.
    let ccw = shape("M -1,-1 L 1,-1 L 1,1 L -1,1 Z");
    let cw = shape("M -1,-1 L -1,1 L 1,1 L 1,-1 Z");
    assert!(ccw.signed_area() > 0.0, "got {}", ccw.signed_area());
    assert!(cw.signed_area() < 0.0, "got {}", cw.signed_area());
    assert!(
        (ccw.signed_area() + cw.signed_area()).abs() < 1e-3,
        "same figure, opposite sign: {} vs {}",
        ccw.signed_area(),
        cw.signed_area()
    );
}

#[test]
fn a_contour_resamples_to_another_arity_around_the_same_outline() {
    let square = shape("M 0,0 H 10 V 10 H 0 Z");
    let coarse = square
        .resampled(MIN_SAMPLES)
        .expect("resamples to a triangle");
    assert_eq!(coarse.points().len(), MIN_SAMPLES);
    let fine = square
        .resampled(MAX_SAMPLES)
        .expect("resamples to the ceiling");
    assert_eq!(fine.points().len(), MAX_SAMPLES);
    // Resampling keeps the frame it was normalized in — it is a re-spacing of
    // one outline, not a second normalization.
    assert_eq!(fine.source_center(), square.source_center());
    assert_eq!(fine.source_scale(), square.source_scale());
    let (min, max) = bounds(fine.points());
    assert!((min[0] + 1.0).abs() < 1e-3 && (max[0] - 1.0).abs() < 1e-3);
}

#[test]
fn a_curve_is_flattened_finely_enough_that_the_resample_sets_the_fidelity() {
    // Four cubics approximating a circle (the standard 0.5523 handle). Every
    // resampled point should sit on the unit circle to within the chord error of
    // the arity it was resampled at, NOT of the flatten step — which is the
    // property FLATTEN_PER_EXTENT exists to give.
    const K: f32 = 0.5523;
    let d = format!(
        "M 1,0 C 1,{K} {K},1 0,1 C -{K},1 -1,{K} -1,0 C -1,-{K} -{K},-1 0,-1 \
         C {K},-1 1,-{K} 1,0 Z"
    );
    let circle = PathShape::parse(&d, 128).expect("parses");
    // The inscribed polygon's own sagitta at 128 points: 1 - cos(pi/128).
    let sagitta = 1.0 - (std::f32::consts::PI / 128.0).cos();
    for p in circle.points() {
        let r = (p[0] * p[0] + p[1] * p[1]).sqrt();
        assert!(
            (r - 1.0).abs() < sagitta + 2e-3,
            "point {p:?} sits at radius {r}, off the unit circle by more than the \
             resample's own chord error"
        );
    }
}
