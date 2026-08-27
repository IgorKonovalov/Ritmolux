// Tests panic on failure; allowed over the file's hot-path pragma.
#![allow(clippy::panic, clippy::expect_used, clippy::indexing_slicing)]

use super::{
    FIT_STEPS, FitLut, IfsFigure, IfsMap, Levers, MAPS, SIGMA_CEILING, SKELETON_FLOOR,
    chaos_extent, decompose, fit_scale, fixed_point, fixed_points, lerp_angle, recompose, resolve,
    skeleton_diameter, skeleton_scale,
};

/// The tolerance on the round trip: well above `f32` round-trip error on
/// values of order 1 through five multiplies and two trig calls, and
/// well below any coefficient difference that would be visible.
const ROUND_TRIP_TOL: f32 = 1e-5;

/// Every figure name a preset can write parses, and nothing else does.
#[test]
fn a_figure_name_parses_or_is_rejected() {
    for figure in IfsFigure::ALL {
        assert_eq!(
            IfsFigure::from_name(figure.name()),
            Some(figure),
            "{figure:?} must parse back from its own name '{}'",
            figure.name()
        );
    }
    for unknown in ["", "Fern", "barnsley", "de_jong", "frond", "ifs"] {
        assert_eq!(
            IfsFigure::from_name(unknown),
            None,
            "'{unknown}' must not parse as a figure"
        );
    }
    // The roster and the parser agree on its size — a figure added to the
    // enum but not to `ALL` would silently escape every sweep below.
    assert_eq!(IfsFigure::ALL.len(), 5);
}

/// **The round trip, on every curated map.**
///
/// `decompose` then `recompose` must return the authored 2x2 within
/// [`ROUND_TRIP_TOL`] per entry — the property the whole parameterization
/// rests on, because a decomposition that cannot represent a map is a figure
/// drawn wrong with nothing failing.
#[test]
fn the_decomposition_round_trips_every_curated_map() {
    for figure in IfsFigure::ALL {
        for (i, (affine, _)) in figure.authored().into_iter().enumerate() {
            let (theta, phi, sx, sy) = decompose(affine.a, affine.b, affine.c, affine.d);
            let (a, b, c, d) = recompose(theta, phi, sx, sy);
            for (got, want, which) in [
                (a, affine.a, "a"),
                (b, affine.b, "b"),
                (c, affine.c, "c"),
                (d, affine.d, "d"),
            ] {
                assert!(
                    (got - want).abs() < ROUND_TRIP_TOL,
                    "{figure:?} map {i} entry {which}: round trip gave {got}, authored {want}"
                );
            }
            // `σ₁σ₂ = det M` — the independent check on each row, and the
            // one that catches a decomposition that got the magnitudes right
            // and the signs wrong.
            let det = affine.a * affine.d - affine.b * affine.c;
            assert!(
                (sx * sy - det).abs() < ROUND_TRIP_TOL,
                "{figure:?} map {i}: sx*sy = {} but det = {det}",
                sx * sy
            );
            // `sx` is the larger singular value and is non-negative.
            assert!(sx >= 0.0, "{figure:?} map {i}: sx = {sx} is negative");
            assert!(
                sx >= sy.abs() - ROUND_TRIP_TOL,
                "{figure:?} map {i}: sx = {sx} is not the larger of the two ({sy})"
            );
        }
    }
}

/// **The row that matters, named outright: the fern's `f₄` reflects.**
///
/// A parameterization that cannot represent a reflection reproduces the fern
/// with its right-hand frond wrong and passes every other row — so the
/// signed `sy` is asserted on that map specifically, not inferred from the
/// sweep above.
#[test]
fn the_ferns_right_frond_survives_the_decomposition_as_a_reflection() {
    let table = IfsFigure::Fern.table();
    let f4 = table.maps[3];

    assert!(
        f4.sy < 0.0,
        "f4 must decompose to a NEGATIVE second singular value, got {}",
        f4.sy
    );
    assert!(
        (f4.sx * f4.sy + 0.1088).abs() < ROUND_TRIP_TOL,
        "f4's determinant is -0.1088 (ADR-0075's table), got {}",
        f4.sx * f4.sy
    );
    // And it survives the trip back: the recomposed map still reverses
    // orientation, which is the property the render depends on.
    let back = f4.to_affine();
    assert!(
        back.a * back.d - back.b * back.c < 0.0,
        "the recomposed f4 no longer reflects"
    );

    // The other three rows do not reflect, so the assertion above is about
    // f4 rather than about the whole table. The stem is the rank-1 case —
    // the other degenerate map the parameterization has to survive.
    assert!(table.maps[0].sy.abs() < ROUND_TRIP_TOL, "f1 is rank 1");
    assert!(table.maps[1].sy > 0.0);
    assert!(table.maps[2].sy > 0.0);
}

/// **Every curated table is contractive, with headroom.**
///
/// The precondition for everything Phases 3-5 build: a figure whose own
/// table already sat at or above 1 could not be made safe by any lever.
/// The `0.97` bound is asserted too, because that is `vigor`'s ceiling — a
/// figure above it would have the clamp fire at *neutral* levers and shrink
/// silently, which is what [`SPIRAL_ARM`](super::SPIRAL_ARM) exists to
/// prevent.
#[test]
fn every_curated_table_contracts_below_the_vigor_ceiling() {
    for figure in IfsFigure::ALL {
        let sigma = figure.table().sigma_max();
        assert!(
            sigma < 1.0,
            "{figure:?} does not contract: sigma_max = {sigma}"
        );
        assert!(
            sigma <= 0.97,
            "{figure:?} sits above vigor's ceiling at neutral: sigma_max = {sigma}"
        );
        // Non-vacuity: a table of near-zero maps would pass the two above
        // and draw a dot. Every figure has a map doing real work.
        assert!(
            sigma > 0.4,
            "{figure:?} contracts so hard it cannot draw a figure: {sigma}"
        );
    }
    // The fern's is ADR-0075's quoted number, which the headroom argument
    // (~17 % to 1.0) is stated against.
    let fern = IfsFigure::Fern.table().sigma_max();
    assert!(
        (fern - 0.851).abs() < 1e-3,
        "the fern's sigma_max is 0.851 in ADR-0075, got {fern}"
    );
}

/// Every table is exactly [`MAPS`] maps with probabilities summing to 1, and
/// the padding is a **duplicate at zero** rather than a zero map.
///
/// The distinction matters for the morph: a zero map at index `i` would drag
/// its partner's map toward the origin at every intermediate `morph`, so the
/// padded slot has to be a real map that simply never gets picked.
#[test]
fn every_table_is_four_maps_padded_by_duplication() {
    for figure in IfsFigure::ALL {
        let authored = figure.authored();
        assert_eq!(authored.len(), MAPS);

        let sum: f32 = authored.iter().map(|(_, p)| *p).sum();
        assert!(
            (sum - 1.0).abs() < 1e-5,
            "{figure:?}'s probabilities sum to {sum}, not 1"
        );
        for (i, (_, p)) in authored.iter().enumerate() {
            assert!(*p >= 0.0, "{figure:?} map {i} has a negative probability");
        }
        // Each padded slot duplicates a slot that is actually drawn.
        for (i, (affine, p)) in authored.iter().enumerate() {
            if *p > 0.0 {
                continue;
            }
            assert!(
                authored
                    .iter()
                    .any(|(other, q)| *q > 0.0 && other == affine),
                "{figure:?} map {i} has probability 0 but is not a duplicate of a drawn map"
            );
        }
    }
    // Non-vacuity: the fern and the tree have no padding at all, so the loop
    // above is not four figures' worth of vacuous truth.
    for full in [IfsFigure::Fern, IfsFigure::Tree, IfsFigure::Sierpinski] {
        assert!(full.authored().iter().filter(|(_, p)| *p == 0.0).count() <= 1);
    }
    assert_eq!(
        IfsFigure::Dragon
            .authored()
            .iter()
            .filter(|(_, p)| *p == 0.0)
            .count(),
        2,
        "the dragon has two real maps, so two slots are padding"
    );
}

/// The packing the shader reads: cumulative probabilities that rise, and a
/// final entry of exactly 1.0.
#[test]
fn the_packed_probabilities_are_cumulative_and_end_at_one() {
    for figure in IfsFigure::ALL {
        let table = figure.table();
        let packed = figure.packed();

        let mut running = 0.0;
        for i in 0..MAPS {
            running += table.maps[i].p;
            let expected = if i == MAPS - 1 { 1.0 } else { running };
            assert!(
                (packed.cumulative_p[i] - expected).abs() < 1e-6,
                "{figure:?} cumulative[{i}] = {}, expected {expected}",
                packed.cumulative_p[i]
            );
        }
        assert_eq!(
            packed.cumulative_p[MAPS - 1],
            1.0,
            "the last entry is the shader's `else` arm and must be exactly 1.0"
        );
        // Non-decreasing, or the shader's three compares select the wrong map.
        for i in 1..MAPS {
            assert!(packed.cumulative_p[i] >= packed.cumulative_p[i - 1]);
        }
    }
}

/// The linear parts and translations land where the shader looks for them —
/// `(e, f)` packed two maps per row is the one place an off-by-one is
/// invisible in a render (it would draw a different, plausible figure).
#[test]
fn the_packing_puts_each_map_where_the_shader_reads_it() {
    for figure in IfsFigure::ALL {
        let table = figure.table();
        let packed = figure.packed();
        for (i, map) in table.maps.iter().enumerate() {
            let affine = map.to_affine();
            assert_eq!(
                packed.linear[i],
                [affine.a, affine.b, affine.c, affine.d],
                "{figure:?} map {i}"
            );
            assert_eq!(packed.translate[i / 2][(i % 2) * 2], affine.e);
            assert_eq!(packed.translate[i / 2][(i % 2) * 2 + 1], affine.f);
        }
    }
}

/// [`IfsFigure::frame`] — the **fallback** framing, used by the seeded
/// scatter and by the CPU transcription of the draw shader — frames every
/// figure at the reference aspect.
///
/// The render path does not go through here since Phase 4; what it uses is
/// asserted by `every_shipped_pair_stays_framed_at_both_aspects`. This
/// stays because the fallback is what sizes the seed box, and a figure
/// seeded outside its own attractor takes visibly longer to converge.
#[test]
fn every_figure_is_framed_at_the_reference_aspect() {
    const REFERENCE: f32 = 16.0 / 9.0;
    for figure in IfsFigure::ALL {
        let (scale, _) = figure.frame();
        let ([hx, hy, _], _) = figure.seed_box();
        assert!(
            hy * scale < 1.0,
            "{figure:?} overflows the frame vertically: {}",
            hy * scale
        );
        assert!(
            hx * scale / REFERENCE < 1.0,
            "{figure:?} overflows a 16:9 frame horizontally: {}",
            hx * scale / REFERENCE
        );
        // Non-vacuity: it must also *fill* the frame along its binding axis
        // rather than sit as a speck in the middle — a scale of 0.001 would
        // pass the two above.
        let fill = (hy * scale).max(hx * scale / REFERENCE);
        assert!(
            fill > 0.7,
            "{figure:?} occupies only {fill} of the frame's binding axis"
        );
    }

    // The fern's Phase 1 claim, kept: it is tall, so it is framed at a
    // portrait aspect too without the fit having to narrow it.
    let (scale, centre) = IfsFigure::Fern.frame();
    assert_eq!(centre, [0.237, 4.999, 0.0]);
    let ([hx, hy, _], _) = IfsFigure::Fern.seed_box();
    assert!(hy * scale < 1.0);
    assert!(hx * scale / (9.0 / 16.0) < 1.0);
}

// -----------------------------------------------------------------------
// The morph (Plan 0062 Phase 3 / ADR-0075)
// -----------------------------------------------------------------------

/// The sweep resolution, which is the one the fit LUT uses too.
const MORPH_STEPS: usize = 33;

/// The 33 sweep positions, `0.0` and `1.0` inclusive and exact at both ends.
fn morph_sweep() -> impl Iterator<Item = f32> {
    (0..MORPH_STEPS).map(|i| i as f32 / (MORPH_STEPS - 1) as f32)
}

/// **The property the whole family rests on, asserted without a GPU.**
///
/// A CPU run of the same step the shader runs, on the resolved table, at
/// every position of a 33-point sweep, for **every ordered pair** of the five
/// figures: 25 pairs x 33 positions x 10 000 iterations. Every position must
/// stay finite and inside a bounded box, and every map's `max σ` must stay
/// below 1.
///
/// Ordered pairs rather than unordered, because `resolve` is not symmetric —
/// the shortest-arc angle interpolation and the clamp both read `a` first —
/// and a preset names a start and a target, not a set.
///
/// Deliberately CPU-side (ADR-0075): the failure this excludes is a
/// permanently dead particle buffer, and a capture of a preset that only
/// diverges on a loud passage would pass.
#[test]
fn no_reachable_morph_diverges() {
    // Generous, and it is meant to be: the point is to catch a run to
    // infinity, not to pin a figure's size. The largest curated figure spans
    // ~14 world units, so anything past 1000 is a divergence in progress
    // rather than a big fern.
    const BOUND: f32 = 1_000.0;
    let mut pairs = 0;
    for start in IfsFigure::ALL {
        for target in IfsFigure::ALL {
            pairs += 1;
            let (a, b) = (start.table(), target.table());
            for morph in morph_sweep() {
                let table = resolve(&a, &b, morph, Levers::NEUTRAL);

                // Contractivity first — it is the *reason* the box below is
                // bounded, and asserting it separately is what distinguishes
                // "converges" from "happened not to blow up in 10 000 steps".
                for (i, map) in table.maps.iter().enumerate() {
                    let sigma = map.sigma_max();
                    assert!(
                        sigma < 1.0,
                        "{start:?} -> {target:?} at morph {morph}: map {i} has \
                         sigma_max = {sigma}, which does not contract"
                    );
                }

                let extent = chaos_extent(&table, 10_000);
                assert!(
                    extent.is_finite(),
                    "{start:?} -> {target:?} at morph {morph}: the orbit left \
                     the reals ({extent:?})"
                );
                for v in extent.lo.iter().chain(extent.hi.iter()) {
                    assert!(
                        v.abs() < BOUND,
                        "{start:?} -> {target:?} at morph {morph}: the orbit \
                         reached {v}, past the {BOUND} bound ({extent:?})"
                    );
                }
                // Non-vacuity: a table that collapsed to a point would pass
                // every assertion above and draw nothing. Every intermediate
                // is a real figure with real extent.
                let [hx, hy] = extent.half();
                assert!(
                    hx.max(hy) > 0.01,
                    "{start:?} -> {target:?} at morph {morph}: collapsed to a \
                     point ({extent:?})"
                );
            }
        }
    }
    assert_eq!(pairs, 25, "every ordered pair of the five figures");
}

/// **The normaliser never approaches its floor** (Plan 0074 Phase 1, claim 1)
/// — measured across the whole reachable morph rather than assumed.
///
/// [`SKELETON_FLOOR`] exists because two drawn maps' fixed points *can*
/// approach each other as a morph interpolates, and a diameter of zero gives
/// a divergent reciprocal. This is the assertion that finds out whether the
/// floor is doing nothing (good) or is load-bearing — which would mean the
/// root channel degenerates somewhere in the morph, and the printed figure
/// pair is where to look.
///
/// **There is no tolerance here to tune.** The two numbers are the
/// measurement and the constant; if they ever meet, the answer is a finding
/// rather than a smaller floor.
#[test]
fn the_skeleton_never_collapses_across_the_morph() {
    let [low, high] = Levers::EXTREMES;
    let mut worst = f32::INFINITY;
    let mut worst_at = None;
    for start in IfsFigure::ALL {
        for target in IfsFigure::ALL {
            let (a, b) = (start.table(), target.table());
            for morph in morph_sweep() {
                for levers in [Levers::NEUTRAL, low, high] {
                    let d = skeleton_diameter(&resolve(&a, &b, morph, levers));
                    assert!(
                        d.is_finite(),
                        "{start:?} -> {target:?} at morph {morph}, levers \
                         {levers:?}: the skeleton's diameter is {d}"
                    );
                    if d < worst {
                        worst = d;
                        worst_at = Some((start, target, morph, levers));
                    }
                }
            }
        }
    }
    // Printed whether or not it passes — the margin is the finding, and a
    // green run that never says how close it came is exactly the shape of
    // report this claim exists to avoid.
    println!(
        "minimum fixed-point diameter over the sweep: {worst} (floor {SKELETON_FLOOR}, \
         margin x{:.1}) at {worst_at:?}",
        worst / SKELETON_FLOOR
    );
    assert!(
        worst > SKELETON_FLOOR,
        "the fixed-point diameter falls to {worst} at {worst_at:?}, at or below the \
         {SKELETON_FLOOR} floor — the root channel degenerates there, and the floor \
         has become load-bearing rather than defensive"
    );
}

/// The scale is the diameter, floored, and the pack ships its reciprocal.
///
/// Three separate claims, and the middle one is the only place the floor's
/// arithmetic is exercised at all — no reachable table reaches it (the sweep
/// above), so a degenerate one is constructed here on purpose.
#[test]
fn the_skeleton_scale_floors_the_diameter_and_packs_as_a_reciprocal() {
    for figure in IfsFigure::ALL {
        let table = figure.table();
        let diameter = skeleton_diameter(&table);
        // Printed for the Phase 2 gate: the diameter is the *scale* the
        // gradient is drawn against, so a figure whose skeleton is small
        // relative to its own reach saturates the channel at 1 over most of
        // itself, and one whose skeleton is large uses only the bottom of the
        // range. Neither is visible in a pass/fail.
        println!("{figure:?}: skeleton diameter {diameter}");
        assert!(
            diameter > SKELETON_FLOOR,
            "{figure:?} has a skeleton of {diameter}, at or below the floor"
        );
        assert_eq!(skeleton_scale(&table), diameter, "{figure:?}");
        // The reciprocal the GPU is handed, against the scale it came from —
        // the property that stops a caller uploading the skeleton and
        // forgetting its size.
        assert_eq!(super::pack(&table).root_recip, 1.0 / diameter, "{figure:?}");

        // ...and it is the DRAWN set's diameter: the padded slots duplicate
        // drawn maps, so they contribute nothing but a zero distance.
        let points = fixed_points(&table);
        let mut by_hand = 0.0f32;
        for (i, p) in points.iter().enumerate() {
            for q in points.iter().skip(i + 1) {
                by_hand = by_hand.max((q[0] - p[0]).hypot(q[1] - p[1]));
            }
        }
        assert_eq!(by_hand, diameter, "{figure:?}");
    }

    // The floor, on a table no morph reaches: every map contracts to the
    // same point, so the skeleton has no extent at all and the raw
    // reciprocal would be infinite.
    let point = IfsMap {
        theta: 0.0,
        phi: 0.0,
        sx: 0.5,
        sy: 0.5,
        t: [1.0, -2.0],
        p: 0.25,
    };
    let degenerate = super::IfsTable {
        maps: [point; MAPS],
    };
    assert_eq!(skeleton_diameter(&degenerate), 0.0);
    assert_eq!(skeleton_scale(&degenerate), SKELETON_FLOOR);
    let recip = super::pack(&degenerate).root_recip;
    assert!(
        recip.is_finite() && recip == 1.0 / SKELETON_FLOOR,
        "a collapsed skeleton must clamp to the floor's reciprocal, got {recip}"
    );
}

/// The endpoints are the figures themselves, exactly — `morph = 0` is the
/// configured figure and `morph = 1` is its target, with no drift from the
/// lerp at either end.
///
/// This is what makes an unbound `morph` a no-op rather than a slight
/// distortion, and what lets `morph_to` be absent by giving both ends the
/// same table.
#[test]
fn the_morph_endpoints_are_the_figures_themselves() {
    for start in IfsFigure::ALL {
        for target in IfsFigure::ALL {
            let (a, b) = (start.table(), target.table());
            assert_eq!(
                resolve(&a, &b, 0.0, Levers::NEUTRAL),
                a,
                "{start:?} -> {target:?} at morph 0 must be {start:?}"
            );
            assert_eq!(
                resolve(&a, &b, 1.0, Levers::NEUTRAL),
                b,
                "{start:?} -> {target:?} at morph 1 must be {target:?}"
            );
        }
    }
    // A figure morphing to itself is the identity at *every* position, which
    // is the absent-`morph_to` case: `morph` is inert by arithmetic rather
    // than by a branch on the render path.
    for figure in IfsFigure::ALL {
        let t = figure.table();
        for morph in morph_sweep() {
            assert_eq!(
                resolve(&t, &t, morph, Levers::NEUTRAL),
                t,
                "{figure:?} morphing to itself moved at {morph}"
            );
        }
    }
}

/// `morph` is bindable, so a preset expression can hand `resolve` anything.
/// Out of range it must **clamp**, not extrapolate — an extrapolated singular
/// value is the one number that can leave the contractive ball.
#[test]
fn an_out_of_range_morph_clamps_rather_than_extrapolating() {
    let (a, b) = (IfsFigure::Fern.table(), IfsFigure::Spiral.table());
    for wild in [-5.0, -1.0, -0.001, 1.001, 2.0, 47.0] {
        let table = resolve(&a, &b, wild, Levers::NEUTRAL);
        let expected = if wild < 0.0 { a } else { b };
        assert_eq!(table, expected, "morph {wild} must clamp to an endpoint");
        assert!(table.sigma_max() < 1.0);
    }
    // A NaN — reachable from a preset expression like `0/0` — must not
    // produce a NaN table. `clamp` panics on a NaN bound but propagates a
    // NaN value, so this is asserted rather than assumed.
    let table = resolve(&a, &b, f32::NAN, Levers::NEUTRAL);
    assert!(
        table.maps.iter().all(|m| m.sigma_max().is_finite()),
        "a NaN morph produced a non-finite table: {table:?}"
    );
}

/// Angles take the **shortest arc**, which is visible rather than pedantic:
/// a branch at `+3.0` rad and one at `−3.0` rad are a tenth of a turn apart,
/// and a plain lerp would sweep it almost all the way round and back.
#[test]
fn angles_interpolate_the_short_way_round() {
    use std::f32::consts::PI;

    // Across the +/-pi discontinuity: the midpoint is just past pi, not 0.
    let mid = lerp_angle(3.0, -3.0, 0.5);
    assert!(
        (mid.abs() - PI).abs() < 0.15,
        "the midpoint of 3.0 -> -3.0 should sit near +/-pi, got {mid}"
    );
    // ...and the total travel is the short arc, not the long one.
    let travel = (lerp_angle(3.0, -3.0, 1.0) - 3.0).abs();
    assert!(
        travel < PI,
        "3.0 -> -3.0 travelled {travel} rad, the long way round"
    );

    // Endpoints are exact, and an ordinary in-range pair is the plain lerp.
    assert_eq!(lerp_angle(0.3, 1.1, 0.0), 0.3);
    assert!((lerp_angle(0.3, 1.1, 1.0) - 1.1).abs() < 1e-6);
    assert!((lerp_angle(0.0, 1.0, 0.5) - 0.5).abs() < 1e-6);
}

/// The CPU reference is deterministic and actually measures the figure —
/// otherwise the sweep above is asserting something about noise.
#[test]
fn the_chaos_reference_is_deterministic_and_measures_the_figure() {
    let fern = IfsFigure::Fern.table();
    let a = chaos_extent(&fern, 20_000);
    let b = chaos_extent(&fern, 20_000);
    assert_eq!(
        a, b,
        "the CPU reference must be a pure function of its table"
    );

    // It finds the fern's published box: x in ~[-2.2, 2.7], y in ~[0, 10].
    assert!(a.lo[1] > -0.1 && a.lo[1] < 0.3, "fern y floor: {}", a.lo[1]);
    assert!(
        a.hi[1] > 9.0 && a.hi[1] < 10.2,
        "fern y ceiling: {}",
        a.hi[1]
    );
    assert!(
        a.lo[0] > -2.6 && a.lo[0] < -1.8,
        "fern x floor: {}",
        a.lo[0]
    );
    assert!(
        a.hi[0] > 2.2 && a.hi[0] < 3.0,
        "fern x ceiling: {}",
        a.hi[0]
    );

    // And it agrees with the measured literals `frame`/`seed_box` are built
    // from — the two must not drift, since Phase 4 replaces those literals
    // with this function's output.
    for figure in IfsFigure::ALL {
        let measured = chaos_extent(&figure.table(), 200_000);
        let ([hx, hy, _], [cx, cy, _]) = figure.seed_box();
        let [mcx, mcy] = measured.centre();
        let [mhx, mhy] = measured.half();
        for (got, want, axis) in [(mcx, cx, "centre x"), (mcy, cy, "centre y")] {
            assert!(
                (got - want).abs() < 0.05 * (1.0 + want.abs()),
                "{figure:?} {axis}: reference says {got}, the literal says {want}"
            );
        }
        for (got, want, axis) in [(mhx, hx, "half x"), (mhy, hy, "half y")] {
            assert!(
                (got - want).abs() < 0.05 * (1.0 + want.abs()),
                "{figure:?} {axis}: reference says {got}, the literal says {want}"
            );
        }
    }
}

// -----------------------------------------------------------------------
// The fixed points (Plan 0073 Phase 2 / ADR-0087)
// -----------------------------------------------------------------------

/// The residual a genuine fixed point is allowed, relative to `1 + ‖p‖`.
///
/// **Derived, not measured.** `cond(I − M) ≤ (1 + σ_max)/(1 − σ_max)`, which
/// is at most `65.7` under [`SIGMA_CEILING`], so an `f32` solve
/// (`ε ≈ 1.2e-7`) carries `‖δp‖ ≲ 8e-6 ‖p‖` and the residual `(M − I) δp` is
/// `≲ 1.6e-5 ‖p‖`. This is that with an order of magnitude of headroom, and
/// it holds on every machine CI runs: the inputs are committed constants and
/// every IEEE-754 operation on the path is correctly rounded.
const RESIDUAL_TOL: f32 = 1e-4;

/// Relative slack on the two *inequalities* below, for the rounding in
/// computing each side. Not a tuning knob — both bounds are exact in real
/// arithmetic, and this is only the last-bits allowance.
const BOUND_SLACK: f32 = 1e-4;

/// Neutral plus both documented extremes — neutral is included because
/// it is what every unbound preset actually ships.
const LEVER_SETTINGS: [Levers; 3] = [Levers::NEUTRAL, Levers::EXTREMES[0], Levers::EXTREMES[1]];

fn norm(q: [f32; 2]) -> f32 {
    q[0].hypot(q[1])
}

/// `‖f(q) − q‖` — how far `q` is from being this map's fixed point.
///
/// **This is the whole instrument of Phase 2.** It is zero exactly at the
/// fixed point and, by `‖fₖ(q) − q‖ = ‖(M − I)(q − pₖ)‖ ≥ (1 − σ_max)‖q − pₖ‖`,
/// grows at least linearly away from it — so one number both certifies the
/// closed form and separates a fixed point from anything else, with a
/// provable margin and no sampling in it.
fn residual(map: &IfsMap, q: [f32; 2]) -> f32 {
    let a = map.to_affine();
    let [x, y] = q;
    norm([a.a * x + a.b * y + a.e - x, a.c * x + a.d * y + a.f - y])
}

/// **Claims 1 and 2 of Plan 0073 Phase 2**, over every reachable table: the
/// closed form really does return a fixed point, and its magnitude respects
/// ADR-0087's own bound.
///
/// Asserted as a **residual** rather than as a location, because "is this
/// point on the attractor" is a theorem (ADR-0075's Notes) and not something
/// a measurement can establish — a chaos game is a finite sample of a measure
/// whose *support* is the attractor, so it can fail to contradict membership
/// and never certify it. What can actually be wrong here is the transcription
/// of `(I − M)⁻¹ t`, and that is exactly what a residual catches.
///
/// The magnitude bound is the second half and it is not redundant:
/// `‖p‖ ≤ ‖t‖/(1 − σ_max)` subsumes "every returned point is finite" and
/// fails loudly on a table that escaped [`SIGMA_CEILING`], which a residual
/// alone would not notice.
#[test]
fn the_fixed_point_closed_form_is_right_and_bounded() {
    let mut worst_residual = 0.0f32;
    let mut tightest_magnitude = 0.0f32;
    for start in IfsFigure::ALL {
        for target in IfsFigure::ALL {
            for morph in morph_sweep() {
                for levers in LEVER_SETTINGS {
                    let table = resolve(&start.table(), &target.table(), morph, levers);
                    // Every map, pads included: the closed form has to be
                    // right for any contractive map, and a pad becomes a
                    // respawn target the moment it duplicates a drawn one.
                    for (i, map) in table.maps.iter().enumerate() {
                        let where_ = format!(
                            "{start:?} -> {target:?} at morph {morph}, levers {levers:?}, map {i}"
                        );
                        let p = fixed_point(map);
                        assert!(
                            p[0].is_finite() && p[1].is_finite(),
                            "{where_}: fixed point is {p:?}"
                        );

                        // Claim 1 — the closed form returns a fixed point.
                        let r = residual(map, p);
                        let scale = 1.0 + norm(p);
                        assert!(
                            r <= RESIDUAL_TOL * scale,
                            "{where_}: f(p) - p has norm {r}, past {} for p = {p:?} — \
                             the closed form is transcribed wrong",
                            RESIDUAL_TOL * scale
                        );
                        worst_residual = worst_residual.max(r / scale);

                        // Claim 2 — ADR-0087's magnitude bound. Written as a
                        // multiply rather than as a divide so a table that
                        // somehow reached sigma >= 1 fails here instead of
                        // producing a negative or infinite limit to pass.
                        let headroom = 1.0 - map.sigma_max();
                        assert!(
                            headroom > 0.0,
                            "{where_}: sigma_max = {} does not contract",
                            map.sigma_max()
                        );
                        let ratio = norm(p) * headroom / norm(map.t).max(f32::MIN_POSITIVE);
                        assert!(
                            norm(p) * headroom <= norm(map.t) * (1.0 + BOUND_SLACK),
                            "{where_}: ||p|| = {} exceeds ||t||/(1 - sigma_max) = {}",
                            norm(p),
                            norm(map.t) / headroom
                        );
                        if norm(map.t) > 0.0 {
                            tightest_magnitude = tightest_magnitude.max(ratio);
                        }
                    }
                }
            }
        }
    }
    println!(
        "worst relative residual {worst_residual:.3e} against a {RESIDUAL_TOL:.0e} \
         bound; tightest magnitude ratio {tightest_magnitude:.4} of the theorem's 1.0"
    );
}

/// **Claim 2's second half, with no threshold in it at all**: the padded
/// slots duplicate a *drawn* map, and no drawn map is left unreachable.
///
/// A padded slot's own fixed point is on the attractor only when the pad
/// happens to duplicate a drawn map — true of all five curated tables today,
/// and exactly what stops being true when a sixth figure is added. Exact
/// equality rather than a tolerance, because both sides come from the same
/// function applied to the same map.
#[test]
fn every_slot_is_a_drawn_maps_fixed_point_and_every_drawn_map_gets_one() {
    for start in IfsFigure::ALL {
        for target in IfsFigure::ALL {
            for morph in morph_sweep() {
                for levers in LEVER_SETTINGS {
                    let table = resolve(&start.table(), &target.table(), morph, levers);
                    let slots = fixed_points(&table);
                    let drawn: Vec<[f32; 2]> = table
                        .maps
                        .iter()
                        .filter(|m| m.p > 0.0)
                        .map(fixed_point)
                        .collect();
                    let where_ =
                        format!("{start:?} -> {target:?} at morph {morph}, levers {levers:?}");
                    assert!(!drawn.is_empty(), "{where_}: no map is drawn at all");
                    for (i, slot) in slots.iter().enumerate() {
                        assert!(
                            drawn.contains(slot),
                            "{where_}: slot {i} is {slot:?}, which is no drawn map's \
                             fixed point — a respawn there lands off the attractor"
                        );
                    }
                    for (k, point) in drawn.iter().enumerate() {
                        assert!(
                            slots.contains(point),
                            "{where_}: drawn map {k}'s fixed point {point:?} appears in \
                             no slot, so one sub-copy is never a respawn target"
                        );
                    }
                }
            }
        }
    }
}

/// **The sensitivity claim, discharged where it is exact.**
///
/// A seed-box corner — which is precisely where backlog 0064's rectangle put
/// particles — fails the residual bound *by construction*, and by a margin
/// that is a theorem rather than a hope:
/// `‖fₖ(q) − q‖ = ‖(M − I)(q − pₖ)‖ ≥ (1 − σ_max)‖q − pₖ‖`, so at
/// [`SIGMA_CEILING`] at least `0.03 ‖q − pₖ‖`. Both halves are asserted: the
/// lower bound holds for **every** corner-and-map pair, and the corners are
/// far enough from the fixed points that the residual clears
/// [`RESIDUAL_TOL`].
///
/// **Two pairs are exempt from the second half, and that is a fact about one
/// figure rather than a weakness.** A Sierpinski gasket's three vertices
/// *are* its three maps' fixed points, and its seed box is its own extent —
/// so the box's lower-left and lower-right corners, `(0, 0)` and `(1, 0)`,
/// land exactly on two of them. (The apex sits at the midpoint of the top
/// edge, not at a corner, which is why it is two and not three.) The lower
/// bound still holds there — both sides are zero — which is precisely why
/// *that* is the half stated over every pair.
///
/// **The measured margins are wide except at one pair, and that pair is
/// honest too.** Most corners clear [`RESIDUAL_TOL`] by three orders of
/// magnitude; the weakest is the fern's top-right corner `[2.656, 9.967]`
/// against its body map, at **1.2x**. That is not the test being thin — the
/// fern's body map's fixed point is the frond tip, which genuinely sits
/// `0.0085` from the corner of the fern's own bounding box, so a point that
/// is nearly a fixed point nearly passes. It is the lower bound, not this
/// margin, that carries the claim.
#[test]
fn a_seed_box_corner_fails_the_residual_bound_by_a_provable_margin() {
    let mut exempt = 0;
    let mut worst_margin = f32::INFINITY;
    let mut weakest = String::new();
    for figure in IfsFigure::ALL {
        let table = figure.table();
        let ([hx, hy, _], [cx, cy, _]) = figure.seed_box();
        for (sx, sy) in [(1.0, 1.0), (1.0, -1.0), (-1.0, 1.0), (-1.0, -1.0)] {
            let q = [cx + sx * hx, cy + sy * hy];
            for (k, map) in table.maps.iter().enumerate() {
                let p = fixed_point(map);
                let distance = norm([q[0] - p[0], q[1] - p[1]]);
                let r = residual(map, q);

                // The theorem, over every pair.
                let lower = (1.0 - map.sigma_max()) * distance;
                assert!(
                    r >= lower * (1.0 - BOUND_SLACK),
                    "{figure:?} corner {q:?} vs map {k}: residual {r} is under the \
                     provable floor {lower}"
                );

                // ...and the consequence, everywhere the corner is not
                // literally the fixed point.
                if distance == 0.0 {
                    exempt += 1;
                    continue;
                }
                let bound = RESIDUAL_TOL * (1.0 + norm(q));
                assert!(
                    r > bound,
                    "{figure:?} corner {q:?} vs map {k}: residual {r} does not clear \
                     {bound}, so the closed-form assertion is measuring nothing"
                );
                if r / bound < worst_margin {
                    worst_margin = r / bound;
                    weakest = format!("{figure:?} corner {q:?} vs map {k} (distance {distance})");
                }
            }
        }
    }
    assert_eq!(
        exempt, 2,
        "exactly two corner-and-map pairs should coincide (the Sierpinski gasket's \
         two lower vertices); {exempt} did, so the figures have changed"
    );
    println!(
        "the weakest seed-box corner clears the residual bound by {worst_margin:.1}x: {weakest}"
    );
}

// -----------------------------------------------------------------------
// The fit (Plan 0062 Phase 4 / ADR-0075)
// -----------------------------------------------------------------------

/// The two aspects the framing has to hold at: a 16:9 landscape window and a
/// 9:16 portrait one. The wide figures are the reason the second is checked —
/// the dragon and the spiral are about twice as wide as they are tall, and a
/// scale fitted at 16:9 leaves them hanging out of a portrait frame.
const FRAMED_ASPECTS: [f32; 2] = [16.0 / 9.0, 9.0 / 16.0];

/// **The figure stays in the frame, at every morph, at both aspects.**
///
/// Asserted on the fitted extent rather than on pixels: what a capture could
/// say is that the picture looked plausible at one size, and the claim is
/// about every position of a continuous parameter at any window shape.
#[test]
fn every_shipped_pair_stays_framed_at_both_aspects() {
    for start in IfsFigure::ALL {
        for target in IfsFigure::ALL {
            let (a, b) = (start.table(), target.table());
            let fit = FitLut::build(&a, &b);
            // Sampled between the LUT's own entries too, so the assertion
            // covers the interpolation and not only the measured points.
            for i in 0..=128 {
                let morph = i as f32 / 128.0;
                let (centre, half) = fit.sample(morph);
                assert!(
                    centre.iter().chain(half.iter()).all(|v| v.is_finite()),
                    "{start:?} -> {target:?} at {morph}: non-finite framing"
                );
                for aspect in FRAMED_ASPECTS {
                    let scale = fit_scale(half, aspect);
                    // The frame is NDC |y| <= 1 and |x| <= aspect (the
                    // vertex shader divides x by the aspect).
                    let (ndc_x, ndc_y) = (half[0] * scale / aspect, half[1] * scale);
                    assert!(
                        ndc_x <= 1.0 && ndc_y <= 1.0,
                        "{start:?} -> {target:?} at morph {morph}, aspect {aspect}: \
                         the figure reaches ({ndc_x}, {ndc_y}) in NDC"
                    );
                    // Non-vacuity: it fills the frame along its binding axis
                    // rather than shrinking to a dot to stay inside.
                    assert!(
                        ndc_x.max(ndc_y) > 0.7,
                        "{start:?} -> {target:?} at morph {morph}, aspect {aspect}: \
                         occupies only {} of the binding axis",
                        ndc_x.max(ndc_y)
                    );
                }
            }
        }
    }
}

/// The fit's endpoints are each figure's own framing — so a preset that
/// never binds `morph` is framed exactly as the single figure it named.
#[test]
fn the_fit_endpoints_are_the_figures_own_framing() {
    for start in IfsFigure::ALL {
        for target in IfsFigure::ALL {
            let fit = FitLut::build(&start.table(), &target.table());
            for (morph, figure) in [(0.0, start), (1.0, target)] {
                let (centre, half) = fit.sample(morph);
                let solo = chaos_extent(&figure.table(), super::FIT_ITERATIONS);
                assert_eq!(
                    (centre, half),
                    (solo.centre(), solo.half()),
                    "{start:?} -> {target:?} at morph {morph} must frame {figure:?} \
                     exactly as it frames itself"
                );
            }
        }
    }
}

/// The fit is a **function of `morph` and the figure pair only** — the
/// property the whole design rests on, since a fit that saw `vigor` would
/// cancel it (ADR-0075 Alternative C).
///
/// Phase 4 can assert the structural half: the builder takes two tables and
/// nothing else, and the same pair produces a bit-identical table however
/// many times it is built. Phase 5 asserts the half that needs levers to
/// exist — that moving all four to their extremes does not move this.
#[test]
fn the_fit_is_reproducible_and_sees_nothing_but_the_pair() {
    for start in IfsFigure::ALL {
        for target in IfsFigure::ALL {
            let (a, b) = (start.table(), target.table());
            assert_eq!(
                FitLut::build(&a, &b),
                FitLut::build(&a, &b),
                "{start:?} -> {target:?}: the fit is not reproducible, so a \
                 capture's framing would depend on when it ran"
            );
        }
    }
    // Different pairs give different fits — otherwise the assertion above
    // holds for a builder that ignores its arguments.
    assert_ne!(
        FitLut::build(&IfsFigure::Fern.table(), &IfsFigure::Fern.table()),
        FitLut::build(&IfsFigure::Fern.table(), &IfsFigure::Spiral.table())
    );
}

/// `morph` is bindable, so the LUT is sampled with whatever a preset
/// produces. Out of range and at `NaN` it must stay on the table.
#[test]
fn sampling_the_fit_out_of_range_stays_on_the_table() {
    let fit = FitLut::build(&IfsFigure::Fern.table(), &IfsFigure::Dragon.table());
    let at0 = fit.sample(0.0);
    let at1 = fit.sample(1.0);
    for (wild, expected) in [
        (-1.0, at0),
        (-0.001, at0),
        (f32::NAN, at0),
        (1.001, at1),
        (99.0, at1),
    ] {
        assert_eq!(fit.sample(wild), expected, "sampling at {wild}");
    }
    // And the table really has FIT_STEPS entries the sampler walks — an
    // off-by-one at the top end would read past the last.
    assert_eq!(FIT_STEPS, 33);
    assert!(fit.sample(1.0).1.iter().all(|v| v.is_finite()));
}

/// The fit is built at `configure`, so it has to sit inside the
/// preset-switch budget (~150 ms).
///
/// **Asserted on the work rather than on a wall clock**, and not only
/// because this crate forbids reading one (the determinism rule, enforced by
/// a clippy `disallowed_methods` entry). An iteration count is the thing the
/// cost is actually proportional to, it is the same number on every machine,
/// and it is what an order-of-magnitude mistake would move — a chaos game
/// per frame, or an iteration count with an extra zero. A timing assertion
/// would measure the CI runner's load instead and flake.
///
/// The constant is calibrated against a measured run: the 140 448 iterations
/// this table costs take **0.51 ms** in an optimized build, so a 500 000
/// ceiling is under 2 ms — about 1 % of the switch budget.
#[test]
fn the_fit_is_built_inside_the_preset_switch_budget() {
    const ITERATION_CEILING: u32 = 500_000;
    let total = FIT_STEPS as u32 * (super::FIT_ITERATIONS + super::CHAOS_BURN_IN);
    assert!(
        total <= ITERATION_CEILING,
        "the fit runs {total} chaos-game iterations at every preset switch, \
         past the {ITERATION_CEILING} ceiling"
    );

    // Non-vacuity: not so few iterations that the box is noise. How close
    // it has to be is the next test's question, not this one's.
    const { assert!(super::FIT_ITERATIONS >= 1_000) };
    assert!(
        FitLut::build(&IfsFigure::Fern.table(), &IfsFigure::Spiral.table())
            .sample(0.5)
            .1
            .iter()
            .all(|h| *h > 0.0)
    );
}

/// **What the fit under-measures, [`FRAME_FILL`](super::FRAME_FILL)'s margin
/// has to absorb** — and this is the assertion that makes
/// `every_shipped_pair_stays_framed_at_both_aspects` a claim about the
/// figure rather than about the lookup's own numbers.
///
/// A sampled bounding box is a **maximum over a finite orbit**, so it
/// converges from below: the fit always measures a figure slightly smaller
/// than it is, and therefore draws it slightly larger than it planned. If the
/// under-measure is `e`, the true fill is `FRAME_FILL / (1 - e)`, so the
/// figure stays inside the frame exactly while `e < 1 - FRAME_FILL`.
///
/// **Iterating the error away is not available, and that is worth knowing
/// before someone tries.** The tree's trunk map is `(0, 0, 0, 0.5)` — it
/// halves `y` towards zero — so the figure's true infimum in `y` is
/// approached and never reached, and the sampled box keeps creeping for as
/// long as anyone is willing to iterate. Measured: **7.9 % under at 4 000
/// iterations and still 6.6 % at 32 000**, for eight times the load cost.
/// The margin is the mechanism; the iteration count only has to keep the
/// error inside it.
///
/// So this asserts the **consequence** — the fraction of the frame the true
/// figure occupies — rather than the error itself. The error is a proxy with
/// an arbitrary budget; the fill is the property, and a figure whose fill
/// crosses 1 is a figure hanging out of the frame.
#[test]
fn the_fit_leaves_margin_for_what_it_under_measures() {
    /// How much of the frame the true figure may occupy along its binding
    /// axis. Below `1.0` is the property; this leaves visible daylight so a
    /// pass is not a near-miss.
    const FILL_CEILING: f32 = 0.97;

    let mut cases: Vec<(String, super::IfsTable)> = IfsFigure::ALL
        .iter()
        .map(|f| (format!("{f:?}"), f.table()))
        .collect();
    for (start, target) in [
        (IfsFigure::Fern, IfsFigure::Spiral),
        (IfsFigure::Fern, IfsFigure::Dragon),
    ] {
        let (a, b) = (start.table(), target.table());
        for morph in [0.25, 0.5, 0.75] {
            cases.push((
                format!("{start:?}->{target:?}@{morph}"),
                resolve(&a, &b, morph, Levers::NEUTRAL),
            ));
        }
    }

    let mut worst_fill = 0.0f32;
    for (name, table) in &cases {
        let quick = chaos_extent(table, super::FIT_ITERATIONS).half();
        let long = chaos_extent(table, 200_000).half();
        for axis in 0..2 {
            let (Some(q), Some(c)) = (quick.get(axis), long.get(axis)) else {
                continue;
            };
            // The fit scales so the MEASURED half-extent occupies
            // `FRAME_FILL`; the TRUE half-extent then occupies this.
            let fill = super::FRAME_FILL * c / q;
            worst_fill = worst_fill.max(fill);
            assert!(
                fill < FILL_CEILING,
                "{name} axis {axis}: the fit measures {q} against a long-run \
                 {c}, so the true figure fills {:.3} of the frame — past the \
                 {FILL_CEILING} ceiling, and {:.3} is where it leaves it",
                fill,
                1.0f32
            );
        }
    }
    // Non-vacuity in both directions: the under-measure is real (so the
    // margin is doing work rather than the two runs agreeing exactly), and
    // it never inverts (a sampled maximum cannot over-measure).
    assert!(
        worst_fill > super::FRAME_FILL + 0.001,
        "no figure under-measured at all ({worst_fill}), so this test is not \
         measuring what it claims"
    );
}

/// The aspect-aware fit is what makes a wide figure legal in a narrow
/// window, so the scale must genuinely differ between the two aspects for a
/// wide figure and not for a tall one.
#[test]
fn the_fit_scale_binds_on_whichever_axis_is_tighter() {
    // The fern is tall: vertically bound at both aspects, same scale.
    let fern_half = chaos_extent(&IfsFigure::Fern.table(), 20_000).half();
    assert!(
        (fit_scale(fern_half, 16.0 / 9.0) - fit_scale(fern_half, 9.0 / 16.0)).abs() < 1e-6,
        "a tall figure is vertically bound at every aspect"
    );

    // The dragon is wide: horizontally bound in portrait, so its scale must
    // drop — that is the whole point of taking the aspect.
    let dragon_half = chaos_extent(&IfsFigure::Dragon.table(), 20_000).half();
    assert!(
        fit_scale(dragon_half, 9.0 / 16.0) < fit_scale(dragon_half, 16.0 / 9.0) * 0.6,
        "a wide figure must shrink in a portrait window"
    );

    // A degenerate half-extent cannot produce an infinity on its way to a
    // uniform — `Extent::half` floors it, and this states the consequence.
    assert!(fit_scale([1e-6, 1e-6], 1.0).is_finite());
    assert!(fit_scale([1.0, 1.0], 0.0).is_finite());
}

/// **The fit frames a figure that does not turn** (ADR-0103), and this is the
/// closed form that says so, asserted against the shipped [`fit_scale`].
///
/// The fit measures an **axis-aligned** box; `project`'s 2D branch then
/// centres and rotates in-plane by the spin phase, which defaults to on. A
/// centred AABB of half-extents `(hx, hy)` rotated by `θ` has half-extents
/// `hx·|cos θ| + hy·|sin θ|` and `hx·|sin θ| + hy·|cos θ|`, and **both reach
/// `r = hypot(hx, hy)` at their worst angle** — so with the vertical budget
/// the scarcer one at any aspect at or above 1, the worst-angle fill is
/// simply `fit_scale · r`. Writing `a = hx / hy`, that is
/// `FRAME_FILL · sqrt(1 + a²)` when the vertical binds and
/// `FRAME_FILL · aspect · sqrt(1 + a²) / a` when the horizontal does.
///
/// Every constant here is **derived, not measured** (ADR-0071): the
/// compliance bound is `sqrt(1/FRAME_FILL² − 1)` written as that expression,
/// so moving `FRAME_FILL` moves the bound rather than falsifying a literal.
/// Nothing in it depends on an adapter, a display or a machine.
///
/// **It is non-vacuous in both directions and must stay so**: the fern comes
/// back inside the bound and every other shipped figure comes back outside
/// it. If a change ever puts all five on one side, this test proves nothing
/// and the assertion at the bottom fails rather than passing quietly.
#[test]
fn the_fit_frames_a_figure_that_does_not_turn() {
    use std::f32::consts::PI;

    /// Angular samples over the half-turn `|cos|`/`|sin|` are periodic on.
    const SWEEP_STEPS: usize = 720;
    /// Relative slack for an `f32` comparison of two ways of writing one
    /// product. Far below any difference that would mean the closed form and
    /// the shipped fit disagree.
    const REL_TOL: f32 = 1e-4;

    // The compliance bound, from `FRAME_FILL · sqrt(1 + a²) <= 1`.
    let bound = (1.0 / (super::FRAME_FILL * super::FRAME_FILL) - 1.0).sqrt();
    let aspect = super::REFERENCE_ASPECT;
    assert!(
        aspect >= 1.0,
        "the fill below takes the vertical as the scarcer \
                            budget, which needs a landscape or square target"
    );

    let mut inside: Vec<String> = Vec::new();
    let mut outside: Vec<String> = Vec::new();

    for figure in IfsFigure::ALL {
        // The SAME under-measured half-extents the fit itself reads. Mixing a
        // long run into one side of this comparison and a sampled one into
        // the other would compare two different figures.
        let half = chaos_extent(&figure.table(), super::FIT_ITERATIONS).half();
        let [hx, hy] = half;
        let a = hx / hy;
        let r = hx.hypot(hy);

        // 1. The measured worst angle agrees with the closed `r`.
        let step = PI / SWEEP_STEPS as f32;
        let mut measured = 0.0f32;
        for i in 0..=SWEEP_STEPS {
            let (sin, cos) = (step * i as f32).sin_cos();
            let (sin, cos) = (sin.abs(), cos.abs());
            measured = measured.max(hx * cos + hy * sin).max(hx * sin + hy * cos);
        }
        // A sweep can straddle the maximiser by half a step, and the extremum
        // is smooth, so the shortfall is at most `r·(1 − cos(step/2))`. It
        // can never exceed `r`, which is the exact supremum.
        let shortfall = r * (1.0 - (step * 0.5).cos());
        assert!(
            measured <= r * (1.0 + REL_TOL) && measured >= r - shortfall - r * REL_TOL,
            "{figure:?}: the swept worst-angle half-extent {measured} disagrees \
             with hypot(hx, hy) = {r} by more than the sweep's own angular step \
             can account for ({shortfall})"
        );

        // 2. The closed form is what the SHIPPED fit produces, not a parallel
        //    arithmetic that happens to agree with it.
        let fill = fit_scale(half, aspect) * r;
        let closed = if a <= aspect {
            super::FRAME_FILL * (1.0 + a * a).sqrt()
        } else {
            super::FRAME_FILL * aspect * (1.0 + a * a).sqrt() / a
        };
        assert!(
            (fill - closed).abs() <= closed * REL_TOL,
            "{figure:?}: fit_scale · r = {fill}, but the closed form says {closed}"
        );

        // 3. The bound predicts the sign of the outcome. Asserted as an
        //    equivalence, which is only meaningful because no figure sits on
        //    the knife edge — checked, so a rounding flip cannot decide it.
        if a <= aspect {
            assert!(
                (a - bound).abs() > 1e-3,
                "{figure:?}: a = {a} sits on the compliance bound {bound}, so \
                 the equivalence below would be decided by f32 rounding"
            );
            assert_eq!(
                fill <= 1.0,
                a <= bound,
                "{figure:?}: a = {a} against the bound {bound} must predict \
                 the worst-angle fill {fill}"
            );
        } else {
            // `sqrt(1 + a²)/a > 1` for every finite `a`, and FRAME_FILL·aspect
            // is already above 1 at 16:9 — so this case cannot be satisfied.
            assert!(
                fill > 1.0,
                "{figure:?}: horizontal binding is unsatisfiable at any aspect \
                 at or above 1, yet the fill came back {fill}"
            );
        }

        if fill <= 1.0 {
            &mut inside
        } else {
            &mut outside
        }
        .push(format!("{figure:?}@a={a:.4}"));
    }

    // A square figure overruns by 24.4 % at 45°, which is ADR-0103's headline
    // and depends on no figure in the roster.
    let square = fit_scale([1.0, 1.0], aspect) * 2.0f32.sqrt();
    assert!(
        (square - super::FRAME_FILL * 2.0f32.sqrt()).abs() < 1e-6 && square > 1.24,
        "a square figure's worst-angle fill is FRAME_FILL·sqrt(2), not {square}"
    );

    // Non-vacuity in both directions: the assertion can tell the two apart
    // only if both sides are populated.
    assert!(
        inside
            .iter()
            .any(|n| n.starts_with(&format!("{:?}@", IfsFigure::Fern))),
        "the fern must satisfy the rotated bound — it is the figure the fit \
         was developed against, and it is what exercises the equivalence on \
         its true side. Inside: {inside:?}"
    );
    assert!(
        !outside.is_empty(),
        "every shipped figure satisfies the rotated bound ({inside:?}), so \
         rotation is NOT the framing mechanism and ADR-0103's finding needs \
         re-deriving rather than this test needs tuning"
    );
}

// -----------------------------------------------------------------------
// The levers (Plan 0062 Phase 5 / ADR-0075)
// -----------------------------------------------------------------------

/// Each lever alone at each documented extreme, plus all four at once —
/// the cases the sweep below has to cover.
fn lever_cases() -> Vec<(String, Levers)> {
    let mut out = vec![("neutral".into(), Levers::NEUTRAL)];
    for (end, extreme) in Levers::EXTREMES.into_iter().enumerate() {
        // One at a time, so a lever that is only safe because another one
        // happened to shrink the table is caught.
        for (name, solo) in [
            (
                "curl",
                Levers {
                    curl: extreme.curl,
                    ..Levers::NEUTRAL
                },
            ),
            (
                "vigor",
                Levers {
                    vigor: extreme.vigor,
                    ..Levers::NEUTRAL
                },
            ),
            (
                "lean",
                Levers {
                    lean: extreme.lean,
                    ..Levers::NEUTRAL
                },
            ),
            (
                "bias",
                Levers {
                    bias: extreme.bias,
                    ..Levers::NEUTRAL
                },
            ),
        ] {
            out.push((format!("{name}@{end}"), solo));
        }
        out.push((format!("all@{end}"), extreme));
    }
    out
}

/// **Divergence is excluded before a shader runs**: every figure, every
/// lever at both documented extremes, all four at once, at every position of
/// the morph sweep — `max σ` stays below 1 throughout, and the orbit stays
/// finite and bounded.
#[test]
fn no_reachable_lever_setting_diverges() {
    const BOUND: f32 = 1_000.0;
    for start in IfsFigure::ALL {
        for target in IfsFigure::ALL {
            let (a, b) = (start.table(), target.table());
            for (name, levers) in lever_cases() {
                // Coarser in `morph` than the Phase 3 sweep, and deliberately:
                // this multiplies that sweep by eleven lever settings, and
                // contractivity is a property of the resolved table rather
                // than of how finely the path is sampled.
                for i in 0..=8 {
                    let morph = i as f32 / 8.0;
                    let table = resolve(&a, &b, morph, levers);
                    let sigma = table.sigma_max();
                    assert!(
                        sigma < 1.0,
                        "{start:?} -> {target:?} at morph {morph} with {name}: \
                         sigma_max = {sigma}"
                    );
                    assert!(
                        sigma <= SIGMA_CEILING + 1e-6,
                        "{start:?} -> {target:?} at morph {morph} with {name}: \
                         sigma_max = {sigma} is past the {SIGMA_CEILING} ceiling"
                    );
                    // Probabilities stay a distribution — a negative or NaN
                    // one would corrupt the cumulative table the shader
                    // compares against.
                    let total: f32 = table.maps.iter().map(|m| m.p).sum();
                    assert!(
                        table.maps.iter().all(|m| m.p >= 0.0 && m.p.is_finite()),
                        "{start:?} -> {target:?} with {name}: bad probabilities"
                    );
                    assert!(
                        (total - 1.0).abs() < 1e-4,
                        "{start:?} -> {target:?} with {name}: probabilities sum \
                         to {total}"
                    );

                    let extent = chaos_extent(&table, 4_000);
                    assert!(
                        extent.is_finite(),
                        "{start:?} -> {target:?} at morph {morph} with {name}: \
                         the orbit left the reals"
                    );
                    for v in extent.lo.iter().chain(extent.hi.iter()) {
                        assert!(
                            v.abs() < BOUND,
                            "{start:?} -> {target:?} at morph {morph} with \
                             {name}: the orbit reached {v}"
                        );
                    }
                }
            }
        }
    }
}

/// A preset expression can produce a `NaN` or an infinity for any lever.
/// None of them may reach the affine table.
#[test]
fn a_wild_lever_value_never_reaches_the_table() {
    let (a, b) = (IfsFigure::Fern.table(), IfsFigure::Tree.table());
    for wild in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -1.0, 0.0] {
        for levers in [
            Levers {
                curl: wild,
                ..Levers::NEUTRAL
            },
            Levers {
                vigor: wild,
                ..Levers::NEUTRAL
            },
            Levers {
                lean: wild,
                ..Levers::NEUTRAL
            },
            Levers {
                bias: wild,
                ..Levers::NEUTRAL
            },
            Levers {
                curl: wild,
                vigor: wild,
                lean: wild,
                bias: wild,
            },
        ] {
            let table = resolve(&a, &b, 0.4, levers);
            assert!(
                table.sigma_max() < 1.0,
                "{levers:?} produced sigma_max {}",
                table.sigma_max()
            );
            for map in &table.maps {
                let affine = map.to_affine();
                for v in [
                    affine.a, affine.b, affine.c, affine.d, affine.e, affine.f, map.p,
                ] {
                    assert!(v.is_finite(), "{levers:?} put {v} in the table");
                }
            }
        }
    }
}

/// **Neutral is exact.** Every lever operation is guarded so that resting
/// values leave the morphed table bit-identical — which is what keeps the
/// framing (fitted at neutral) agreeing with the figure, and what keeps a
/// preset that binds no lever byte-stable.
#[test]
fn every_lever_is_bit_exact_at_rest() {
    for start in IfsFigure::ALL {
        for target in IfsFigure::ALL {
            let (a, b) = (start.table(), target.table());
            for i in 0..=8 {
                let morph = i as f32 / 8.0;
                assert_eq!(
                    resolve(&a, &b, morph, Levers::NEUTRAL),
                    super::morph_tables(&a, &b, morph),
                    "{start:?} -> {target:?} at morph {morph}: the neutral \
                     levers moved the table"
                );
            }
        }
    }
}

/// **The fit does not see the levers** — Phase 4's deferred half, and the
/// property `vigor` being audible at all depends on (ADR-0075 Alternative C).
///
/// Stated as the pair it has to be: the levers genuinely move the resolved
/// table, and the framing built from the same figure pair is bit-identical
/// regardless. One without the other is vacuous.
#[test]
fn the_fit_is_bit_identical_under_every_lever_extreme() {
    for start in IfsFigure::ALL {
        for target in IfsFigure::ALL {
            let (a, b) = (start.table(), target.table());
            let neutral_fit = FitLut::build(&a, &b);

            for (name, levers) in lever_cases() {
                if levers == Levers::NEUTRAL {
                    continue;
                }
                // The framing is the same table, to the bit.
                assert_eq!(
                    FitLut::build(&a, &b),
                    neutral_fit,
                    "{start:?} -> {target:?} with {name}: the fit moved"
                );
                // ...and the figure it frames genuinely did move, or the
                // assertion above is about a lever that does nothing.
                let moved = (0..=4).any(|i| {
                    let morph = i as f32 / 4.0;
                    resolve(&a, &b, morph, levers) != resolve(&a, &b, morph, Levers::NEUTRAL)
                });
                // The one documented exception, pinned separately below:
                // `bias` is inert on the dragon, whose two real maps both
                // live in the branch slots, so there is nothing to shift
                // weight away from.
                let dragon_bias = name.starts_with("bias")
                    && start == IfsFigure::Dragon
                    && target == IfsFigure::Dragon;
                assert!(
                    moved || dragon_bias,
                    "{start:?} -> {target:?}: lever {name} changed nothing, so \
                     the invariance above is vacuous"
                );
            }
        }
    }
}

/// **`bias` is inert on the dragon, and that is a property rather than a
/// bug** — pinned here so the exemption in the invariance test above is a
/// stated fact and not a hand-wave.
///
/// The dragon has two real maps and both live in the *branch* slots (its
/// body slots are the padding), so there is no body weight to move. Scaling
/// one group and renormalizing is the identity. A special case could invent
/// something for it; ADR-0075's roster is five hand-authored figures and
/// this is the honest consequence of the dragon being one of them. It is a
/// Phase 7 note, not a Phase 5 fix.
#[test]
fn bias_is_inert_on_the_dragon_and_live_on_every_other_figure() {
    let biased = |figure: IfsFigure, b: f32| {
        let t = figure.table();
        resolve(
            &t,
            &t,
            0.0,
            Levers {
                bias: b,
                ..Levers::NEUTRAL
            },
        )
    };
    for extreme in [-1.0, 1.0] {
        assert_eq!(
            biased(IfsFigure::Dragon, extreme),
            IfsFigure::Dragon.table(),
            "bias {extreme} on the dragon must be exactly the identity"
        );
    }
    for figure in IfsFigure::ALL {
        if figure == IfsFigure::Dragon {
            continue;
        }
        assert_ne!(
            biased(figure, 1.0),
            figure.table(),
            "bias must reach {figure:?}"
        );
    }
}

/// Each lever moves the thing it names and leaves the others alone — which
/// is what makes them four levers rather than one with four names.
#[test]
fn each_lever_moves_its_own_axis() {
    let fern = IfsFigure::Fern.table();
    let at = |levers| resolve(&fern, &fern, 0.0, levers);
    let base = at(Levers::NEUTRAL);

    // `curl` rotates, so every theta moves by the same amount and nothing
    // else does.
    let curled = at(Levers {
        curl: 0.4,
        ..Levers::NEUTRAL
    });
    for (c, n) in curled.maps.iter().zip(base.maps.iter()) {
        assert!((c.theta - n.theta - 0.4).abs() < 1e-6);
        assert_eq!(c.sx, n.sx);
        assert_eq!(c.sy, n.sy);
        assert_eq!(c.t, n.t);
        assert_eq!(c.p, n.p);
    }

    // `vigor` scales the singular values and nothing else. Below the ceiling
    // so the clamp does not fire and the ratio is exactly the multiplier.
    let vigorous = at(Levers {
        vigor: 1.1,
        ..Levers::NEUTRAL
    });
    assert!(vigorous.sigma_max() > base.sigma_max());
    for (v, n) in vigorous.maps.iter().zip(base.maps.iter()) {
        assert!((v.sx - n.sx * 1.1).abs() < 1e-6);
        assert_eq!(v.theta, n.theta);
        assert_eq!(v.t, n.t);
        assert_eq!(v.p, n.p);
    }

    // `lean` rotates the translations, preserving their length — the sense
    // in which it bends the plant rather than stretching it.
    let leaning = at(Levers {
        lean: 0.5,
        ..Levers::NEUTRAL
    });
    for (l, n) in leaning.maps.iter().zip(base.maps.iter()) {
        let len = |[x, y]: [f32; 2]| (x * x + y * y).sqrt();
        assert!((len(l.t) - len(n.t)).abs() < 1e-5);
        assert_eq!(l.sx, n.sx);
        assert_eq!(l.theta, n.theta);
        assert_eq!(l.p, n.p);
    }
    // ...and it actually moved one: the fern's body translates (0, 1.6).
    assert_ne!(leaning.maps[1].t, base.maps[1].t);

    // `bias` moves probability from the body maps to the branch maps and
    // touches no geometry at all.
    let biased = at(Levers {
        bias: 1.0,
        ..Levers::NEUTRAL
    });
    for (x, n) in biased.maps.iter().zip(base.maps.iter()) {
        assert_eq!(x.sx, n.sx);
        assert_eq!(x.sy, n.sy);
        assert_eq!(x.theta, n.theta);
        assert_eq!(x.t, n.t);
    }
    let body = |t: &super::IfsTable| t.maps[0].p + t.maps[1].p;
    let branch = |t: &super::IfsTable| t.maps[2].p + t.maps[3].p;
    assert!(
        branch(&biased) > branch(&base),
        "a positive bias must move weight to the branches: {} -> {}",
        branch(&base),
        branch(&biased)
    );
    assert!(body(&biased) < body(&base));
    // ...and symmetrically the other way.
    let unbiased = at(Levers {
        bias: -1.0,
        ..Levers::NEUTRAL
    });
    assert!(branch(&unbiased) < branch(&base));
    // No probability is ever driven to zero — that would stop drawing part
    // of the figure rather than re-weighting it (see `BIAS_DEPTH`).
    for extreme in [-1.0, 1.0] {
        let t = at(Levers {
            bias: extreme,
            ..Levers::NEUTRAL
        });
        for (i, map) in t.maps.iter().enumerate() {
            if base.maps.get(i).is_some_and(|b| b.p > 0.0) {
                assert!(map.p > 0.0, "bias {extreme} zeroed a drawn map {i}");
            }
        }
    }
}

/// **`vigor` is a real lever within the ceiling and a silent no-op past it**,
/// which is the trade ADR-0075 accepts and `presets/README.md` documents.
#[test]
fn vigor_grows_the_figure_until_the_ceiling_stops_it() {
    let fern = IfsFigure::Fern.table();
    let at = |v: f32| {
        resolve(
            &fern,
            &fern,
            0.0,
            Levers {
                vigor: v,
                ..Levers::NEUTRAL
            },
        )
    };

    // Below the ceiling the figure genuinely grows — asserted on the
    // measured extent, not on the multiplier, because "the lever is visible"
    // is a claim about the drawing.
    let base = chaos_extent(&at(1.0), 20_000).half();
    let grown = chaos_extent(&at(1.1), 20_000).half();
    assert!(
        grown[1] > base[1] * 1.05,
        "vigor 1.1 must visibly grow the fern: {} -> {}",
        base[1],
        grown[1]
    );

    // The fern's own sigma_max is 0.851, so the ceiling is reached at
    // 0.97/0.851 = 1.14. Past there every value gives the same table —
    // silence rather than an error.
    let ceiling_ratio = SIGMA_CEILING / fern.sigma_max();
    assert!((ceiling_ratio - 1.14).abs() < 0.01, "{ceiling_ratio}");
    for past in [ceiling_ratio + 0.01, 1.5, 3.0, 20.0] {
        let table = at(past);
        assert!(
            (table.sigma_max() - SIGMA_CEILING).abs() < 1e-5,
            "vigor {past} must land exactly on the ceiling, got {}",
            table.sigma_max()
        );
    }
    // Past the ceiling every value draws the same figure. Compared within a
    // tolerance rather than bit-for-bit: the clamp is
    // `sx · vigor · (ceiling / sigma)`, so multiplying up by 1.5 and back
    // down rounds differently from multiplying up by 20 and back down. The
    // claim is that the *figure* is the same, and a last-bit difference in
    // a singular value is not a different figure.
    for (x, y) in at(1.5).maps.iter().zip(at(20.0).maps.iter()) {
        assert!((x.sx - y.sx).abs() < 1e-6 && (x.sy - y.sy).abs() < 1e-6);
    }

    // The clamp scales the WHOLE table by one factor, so the figure's
    // proportions are preserved — it is a smaller figure, not a different
    // one. Asserted on the ratio between two maps.
    let clamped = at(3.0);
    let ratio = |t: &super::IfsTable| t.maps[2].sx / t.maps[1].sx;
    assert!((ratio(&clamped) - ratio(&at(1.0))).abs() < 1e-5);
}
