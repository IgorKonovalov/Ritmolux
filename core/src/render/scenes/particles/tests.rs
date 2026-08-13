// Tests panic on failure; allowed over the file's hot-path pragma — this is
// not the render path.
#![allow(clippy::panic, clippy::expect_used)]

use super::{
    AttractorFamily, AttractorScene, Basis, DEFAULT_BRIGHTNESS, DEFAULT_DEPTH_FADE,
    DEFAULT_DEPTH_HUE, DEFAULT_SPIN, FIXED_STEP, JITTER_MODE, MAX_PERSPECTIVE,
    MIN_PARTICLE_DENSITY, PARTICLE_ATTRIBUTES, Particle, RESEED_DRAWS_STREAK, SPIN_RATE,
    STEP_SLOTS, Scene, StepUniform, active_particles, advance_spin, brightness_factor,
    deposit_scale, family, ifs, projection_mirror, spin_phase, streak_flag,
};
use crate::dsp::AnalysisFrame;
use crate::render::context::RenderContext;
use crate::render::{Tier, TierConfig};
use family::Framing;
use ifs::{IfsFigure, Levers};

// -----------------------------------------------------------------------
// Entry 0 (ADR-0093)
// -----------------------------------------------------------------------
//
// Every assertion in this file that predates the tuple roster is about the
// canonical tuple's framing, because that is the framing those behaviours
// shipped with and the one an unbound preset still gets. These three helpers
// say so at each call site rather than reaching through `canonical_framing()`
// inline forty times.

/// Roster entry 0's framing for a family.
fn canonical(family: AttractorFamily) -> Framing {
    family.canonical_framing()
}

/// Entry 0's depth normalizer — `d.w` as an unbound preset uploads it.
fn canonical_depth(family: AttractorFamily) -> f32 {
    canonical(family).inv_depth_extent(family)
}

/// The seeded scatter over entry 0's box.
fn canonical_seed(family: AttractorFamily, count: u32) -> Vec<Particle> {
    AttractorScene::seed(family, canonical(family).seed_box, &[], count)
}

// -----------------------------------------------------------------------
// The IFS family (Plan 0062 Phase 1 / ADR-0075)
// -----------------------------------------------------------------------

/// Every curated figure reaches the enum through the **same** `[particles]
/// family` key the map families use, and an unknown name is still rejected —
/// which is what makes it a load error rather than a silent De Jong.
#[test]
fn a_figure_name_selects_the_ifs_family() {
    for figure in ifs::IfsFigure::ALL {
        let name = figure.name();
        assert_eq!(
            AttractorFamily::from_name(name),
            Some(AttractorFamily::Ifs(figure)),
            "'{name}' must select {figure:?}"
        );
    }
    // The map families are untouched, and nothing else parses.
    assert_eq!(
        AttractorFamily::from_name("de_jong"),
        Some(AttractorFamily::DeJong)
    );
    for unknown in ["ifs", "barnsley", "fern_", ""] {
        assert_eq!(AttractorFamily::from_name(unknown), None);
    }
}

/// **The jitter selector must not collide with a real family's**, and the IFS
/// is what moved it (from 4 to 5).
///
/// This is the failure that would be invisible in a unit test and obvious
/// only in a render: a family sharing the jitter's id takes the jitter arm,
/// so the cloud is kicked every step and never iterates its map at all.
#[test]
fn the_jitter_selector_sits_past_every_family() {
    let families = [
        AttractorFamily::DeJong,
        AttractorFamily::Clifford,
        AttractorFamily::Thomas,
        AttractorFamily::Lorenz,
        AttractorFamily::Ifs(ifs::IfsFigure::Fern),
    ];
    for family in families {
        assert!(
            family.shader_id() < JITTER_MODE,
            "{family:?} shares or exceeds the jitter selector {JITTER_MODE}"
        );
    }
    // Every family is a distinct arm, or two of them draw the same figure.
    let mut ids: Vec<u32> = families.iter().map(|f| f.shader_id()).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), families.len(), "two families share a shader id");
    // ...and every figure is the *same* arm, because the figure is data.
    for figure in ifs::IfsFigure::ALL {
        assert_eq!(AttractorFamily::Ifs(figure).shader_id(), 4);
    }
}

/// The step uniform's shape, pinned where ADR-0075 quotes it.
///
/// A Rust/WGSL layout disagreement fails loudly at pipeline creation (wgpu
/// compares the struct size against `min_binding_size`), so this is not the
/// safety net — it is the record of *which* number the two agree on, and of
/// the one place the ADR's arithmetic was off: 160 rather than 144, because
/// `step_index` forced the scalar block up to the next multiple of 16.
///
/// **192 since Plan 0073**, the extra two `vec4` being ADR-0087's four
/// respawn targets. The binding *count* is what matters for the ADR-0058
/// collision surface and it has not moved — one storage, one uniform.
#[test]
fn the_step_uniform_carries_the_ifs_table_in_one_binding() {
    assert_eq!(size_of::<StepUniform>(), 192);
    // A slot per possible sub-step plus the jitter's — the property
    // `encode_steps` relies on when it offsets by the sub-step index.
    assert_eq!(STEP_SLOTS, super::MAX_SUBSTEPS + 1);
    assert_eq!(super::JITTER_SLOT, super::MAX_SUBSTEPS);
}

/// The IFS payload is inert for the four map families — asserted on the value
/// rather than left to the shader's family branch, because "they never read
/// it" is a claim about code that is easy to break.
#[test]
fn the_map_families_upload_no_affine_table() {
    for family in [
        AttractorFamily::DeJong,
        AttractorFamily::Clifford,
        AttractorFamily::Thomas,
        AttractorFamily::Lorenz,
    ] {
        assert_eq!(family.figure(), None);
    }
    assert_eq!(
        AttractorFamily::Ifs(ifs::IfsFigure::Fern).figure(),
        Some(ifs::IfsFigure::Fern)
    );
    assert_eq!(StepUniform::NO_IFS, ifs::IfsPacked::ZERO);
    // Zeroed means *every* cumulative entry is zero, so a stray dispatch on
    // the IFS arm would pick the fourth map and apply an all-zero affine —
    // a single point at the origin, not a second figure.
    assert!(StepUniform::NO_IFS.cumulative_p.iter().all(|c| *c == 0.0));
}

/// ADR-0065's invariance, asserted **on the scalar** rather than inferred from
/// pixels — which is the whole reason the decision is expressible as one.
///
/// The `Floor` half is why no golden baseline moves: `1.0` is not a tolerance
/// or a near-miss, it is exact, so every baseline blessed at the floor renders
/// the same arithmetic it always did.
#[test]
fn the_deposit_scalar_is_exactly_one_at_the_floor_and_a_third_at_rich() {
    let floor = TierConfig::for_tier(Tier::Floor).attractor_particles;
    let rich = TierConfig::for_tier(Tier::Rich).attractor_particles;

    assert_eq!(
        deposit_scale(floor),
        1.0,
        "the floor factor must be exactly 1.0 — every golden is blessed there"
    );
    // 50 000 / 150 000 at the shipped counts. Asserted against the ratio the
    // tier table actually holds rather than a literal 1/3, so a re-calibrated
    // `Rich` (Plan 0044 Phase 4 has never run) changes this test's expectation
    // with it instead of failing for the wrong reason.
    let expected = floor as f32 / rich as f32;
    assert_eq!(deposit_scale(rich), expected);
    assert!(
        (deposit_scale(rich) - 1.0 / 3.0).abs() < 1e-6,
        "at the counts shipped today that ratio is 1/3, got {}",
        deposit_scale(rich)
    );

    // Non-vacuity: the two tiers must actually differ, or the assertions above
    // hold for a table where nothing is normalized at all.
    assert!(
        rich > floor,
        "the rich tier is meant to raise the count ({rich} vs {floor})"
    );

    // Total deposited light is what is invariant, and that is the product.
    // Stated directly because it is the property, and the scalar is only the
    // means: three times the samples at a third the weight is the same light.
    let total = |count: u32| count as f32 * deposit_scale(count);
    assert_eq!(total(floor), total(rich));
}

/// ADR-0080's identity claim, asserted on the scalar for the same reason
/// ADR-0065's is: it is what makes every existing golden baseline
/// **byte-identical** rather than approximately unchanged. A multiply by
/// literal `1.0` is exact in IEEE-754, and this is where that literal is.
#[test]
fn the_brightness_factor_is_exactly_one_by_default_and_scales_linearly() {
    assert_eq!(
        brightness_factor(DEFAULT_BRIGHTNESS),
        1.0,
        "the default must be an exact 1.0 — no golden baseline may move"
    );
    assert_eq!(brightness_factor(0.5), 0.5);
    assert_eq!(brightness_factor(2.0), 2.0);
    // The whole product, at the floor tier where every baseline is blessed:
    // half the brightness is exactly half the deposit.
    let floor = TierConfig::for_tier(Tier::Floor).attractor_particles;
    assert_eq!(
        deposit_scale(floor) * brightness_factor(0.5),
        deposit_scale(floor) * 0.5
    );
}

/// The two guards on that factor. Both matter more here than for an ordinary
/// param because the value lands in an accumulation the trail carries forward:
/// a single poisoned frame is a permanently poisoned field.
#[test]
fn the_brightness_factor_refuses_negative_and_non_finite_bindings() {
    // Negative light would *subtract* from the additive accumulation.
    assert_eq!(brightness_factor(-1.0), 0.0);
    assert_eq!(brightness_factor(-0.0), 0.0);
    // NaN and the infinities fall back rather than reaching the uniform: an
    // `inf` deposit survives every later decay multiply.
    assert_eq!(brightness_factor(f32::NAN), DEFAULT_BRIGHTNESS);
    assert_eq!(brightness_factor(f32::INFINITY), DEFAULT_BRIGHTNESS);
    assert_eq!(brightness_factor(f32::NEG_INFINITY), DEFAULT_BRIGHTNESS);
}

/// A zero-particle scene draws nothing, so the factor is unobservable — but it
/// must not be an infinity on its way to a shader uniform.
#[test]
fn the_deposit_scalar_survives_a_degenerate_count() {
    assert!(deposit_scale(0).is_finite());
    // And it is still monotone the right way round: fewer particles, more
    // weight each, which is what keeps the total constant.
    assert!(deposit_scale(25_000) > deposit_scale(50_000));
    assert!(deposit_scale(50_000) > deposit_scale(100_000));
}

// -----------------------------------------------------------------------
// The projection basis (Plan 0059 Phase 1 / ADR-0068)
// -----------------------------------------------------------------------

/// The basis, pinned per family against an explicit table.
///
/// The **compiler** is what stops a fifth family being added without choosing
/// one — `basis()` matches exhaustively with no wildcard arm. This test pins
/// the values that match resolves to, so a silent flip of Lorenz back to the
/// shared convention fails here rather than in someone's eyes.
#[test]
fn the_projection_basis_is_pinned_per_family() {
    let table = [
        (AttractorFamily::DeJong, Basis::XY),
        (AttractorFamily::Clifford, Basis::XY),
        (AttractorFamily::Thomas, Basis::XY),
        (AttractorFamily::Lorenz, Basis::XZ),
    ];
    for (family, expected) in table {
        assert_eq!(
            family.basis(),
            expected,
            "{family:?} must be viewed in {expected:?}"
        );
    }

    // Non-vacuity: exactly one family departs from the shared convention, so
    // the table above is not four copies of one answer.
    let departures = table.iter().filter(|(_, b)| *b != Basis::XY).count();
    assert_eq!(
        departures, 1,
        "ADR-0068 moves exactly one family's basis; {departures} differ from XY"
    );

    // And the departure is not derived from `dim`. Thomas is 3D and keeps
    // x–y — which is the whole reason ADR-0068 Alternative C was declined,
    // so it is asserted rather than left to the doc comment.
    assert_eq!(canonical(AttractorFamily::Thomas).projection.1, 3.0);
    assert_eq!(AttractorFamily::Thomas.basis(), Basis::XY);
    assert_eq!(canonical(AttractorFamily::Lorenz).projection.1, 3.0);
    assert_ne!(
        AttractorFamily::Thomas.basis(),
        AttractorFamily::Lorenz.basis(),
        "two 3D families disagree about the basis, so `dim == 3.0` cannot decide it"
    );
}

/// The masks the shader dots against, asserted as the geometry they encode.
///
/// `XY` must reproduce the pre-ADR-0068 expression exactly — `cx*cs + cz*sn`
/// vertical `y` — because three families' captures have to stay
/// byte-identical, and that claim rests on these six numbers.
#[test]
fn the_basis_masks_select_the_axes_they_name() {
    // A position with distinguishable components, so a mask that picks the
    // wrong axis cannot agree by coincidence. Destructured rather than
    // indexed — this file denies `indexing_slicing`.
    let [px, py, pz] = [2.0f32, 3.0, 5.0];
    let dot = |[mx, my, mz]: [f32; 3]| px * mx + py * my + pz * mz;

    let (h, v) = Basis::XY.masks();
    assert_eq!(dot(h), 5.0, "XY rotates x against z");
    assert_eq!(dot(v), 3.0, "XY is vertical in y");

    let (h, v) = Basis::XZ.masks();
    assert_eq!(dot(h), 3.0, "XZ rotates x against y");
    assert_eq!(dot(v), 5.0, "XZ is vertical in z");

    // Each selector is one-hot and never picks x — the spin's first
    // horizontal term is always `x`, so a mask with an x component would
    // double-count it.
    for basis in [Basis::XY, Basis::XZ] {
        let (h, v) = basis.masks();
        for m in [h, v] {
            let [mx, ..] = m;
            assert_eq!(mx, 0.0, "{basis:?} selector must not pick x: {m:?}");
            assert_eq!(m.iter().filter(|c| **c != 0.0).count(), 1);
            assert_eq!(m.iter().sum::<f32>(), 1.0);
        }
        // ...and the two selectors are different axes, or the projection
        // collapses onto a line.
        assert_ne!(
            h, v,
            "{basis:?} put the horizontal and vertical on one axis"
        );
    }
}

// -----------------------------------------------------------------------
// The view depth (Plan 0063 Phase 1 / ADR-0076)
// -----------------------------------------------------------------------

/// The inverse depth extent, pinned per family against an explicit table.
///
/// The compiler forces a fifth family to answer — the match is exhaustive.
/// This pins *what* it answers, and above all that the 2D families answer
/// **exactly zero**: every depth cue in the shader is the identity there by
/// arithmetic rather than by a branch, so a non-zero value here would give
/// De Jong and Clifford a depth they do not have.
#[test]
fn the_depth_extent_is_zero_for_every_flat_family() {
    // The half-extents ADR-0076 quotes, as reciprocals — derived from
    // `seed_box`, so this fails if that table moves without the ADR.
    let table = [
        (AttractorFamily::DeJong, 0.0),
        (AttractorFamily::Clifford, 0.0),
        (AttractorFamily::Thomas, 1.0 / 4.5),
        (AttractorFamily::Lorenz, 1.0 / 26.0),
    ];
    for (family, expected) in table {
        assert_eq!(
            canonical_depth(family),
            expected,
            "{family:?}'s inverse depth extent must be exactly {expected}"
        );
    }

    // The flat families' zero is an *exact* zero, not a small number: the
    // shader multiplies by it and clamps, so anything else is a depth.
    for flat in [AttractorFamily::DeJong, AttractorFamily::Clifford] {
        assert_eq!(canonical_depth(flat), 0.0);
        assert!(canonical_depth(flat).is_finite());
    }
    // Non-vacuity: the 3D families genuinely carry one, and the two differ —
    // so this is a per-family derivation and not one shared constant.
    assert!(canonical_depth(AttractorFamily::Thomas) > 0.0);
    assert!(canonical_depth(AttractorFamily::Lorenz) > 0.0);
    assert_ne!(
        canonical_depth(AttractorFamily::Thomas),
        canonical_depth(AttractorFamily::Lorenz)
    );
}

/// The identity every one of these assertions is stated against: the rest
/// rotation (`cs = 1, sn = 0`) and the half turn (`cs = -1, sn = 0`).
///
/// Written as exact components rather than as `cos(0.0)` and `cos(PI)`,
/// because `f32::consts::PI` is not `π` and `cos` of it is `-1.0` with a
/// `sin` of `-8.7e-8` — near enough to look right and far enough to make an
/// *exact* equality fail for a reason that has nothing to do with the claim.
const REST: (f32, f32) = (1.0, 0.0);
const HALF_TURN: (f32, f32) = (-1.0, 0.0);

/// Positions on the two 3D figures, each with a genuinely non-zero depth at
/// rest (the depth at `cs = 1, sn = 0` is `dot(p, bh)`, so the component the
/// family's basis selects must not be zero).
fn depth_samples(family: AttractorFamily) -> [[f32; 3]; 4] {
    match family {
        // Lorenz: basis XZ, so `bh` is y and the depth at rest is `y`.
        AttractorFamily::Lorenz => [
            [10.0, 12.0, 30.0],
            [-14.0, -20.0, 40.0],
            [3.0, 25.0, 10.0],
            [19.0, -5.0, 45.0],
        ],
        // Thomas: basis XY, so `bh` is z and the depth at rest is `z`.
        _ => [
            [2.0, -3.0, 1.5],
            [-4.0, 1.0, -2.5],
            [0.5, 3.5, 4.0],
            [-1.5, -0.5, -3.5],
        ],
    }
}

/// **ADR-0076's diagnosis and its fix, as one dimensionless property.**
///
/// Under orthography the projection at rotation `π` is the exact `x`-mirror
/// of the projection at `0` — at `cs = -1, sn = 0` the horizontal term
/// becomes `-p.x` and the vertical term is untouched. That is why a rotating
/// transparent structure carries no information about *which way* it is
/// turning, and with additive blending there is no occlusion to break the
/// tie either: the percept flips and settles on "flat".
///
/// Under perspective it is not a mirror, because `m(h) != m(-h)` for any
/// `h != 0`. Both halves are asserted here, exactly, on the formula — a
/// capture could only report that the picture changed.
#[test]
fn perspective_breaks_the_orthographic_mirror() {
    for family in [AttractorFamily::Lorenz, AttractorFamily::Thomas] {
        for q in depth_samples(family) {
            // The premise: this sample must actually have depth, or every
            // assertion below is about `m(0) = m(0)`.
            let rest = projection_mirror::project(q, family, REST.0, REST.1);
            let dn = projection_mirror::depth_norm(rest.depth, canonical_depth(family));
            assert!(
                dn.abs() > 0.05,
                "{family:?} sample {q:?} sits at depth {dn} — too near the view plane to \
                 distinguish a perspective divide from an orthographic one"
            );

            // --- the flatness, pinned ---
            let flat_rest = projection_mirror::world(q, family, REST.0, REST.1, 0.0);
            let flat_half = projection_mirror::world(q, family, HALF_TURN.0, HALF_TURN.1, 0.0);
            let ([rx, ry], [hx, hy]) = (flat_rest, flat_half);
            assert_eq!(
                hx, -rx,
                "{family:?} sample {q:?}: at perspective 0 the half turn must be the exact \
                 x-mirror of the rest pose"
            );
            assert_eq!(
                hy, ry,
                "{family:?} sample {q:?}: the vertical must not move"
            );

            // --- and the fix, pinned ---
            const P: f32 = 0.5;
            let deep_rest = projection_mirror::world(q, family, REST.0, REST.1, P);
            let deep_half = projection_mirror::world(q, family, HALF_TURN.0, HALF_TURN.1, P);
            let ([drx, dry], [dhx, dhy]) = (deep_rest, deep_half);
            assert_ne!(
                dhx, -drx,
                "{family:?} sample {q:?}: at perspective {P} the half turn is STILL the \
                 x-mirror of the rest pose — the depth is not reaching the magnification, so \
                 the rotation is as ambiguous as it was"
            );
            // The vertical breaks too, and that is worth stating separately:
            // the magnification scales the whole projected position, so a
            // half turn moves the figure toward or away from the camera
            // rather than merely flipping it.
            assert_ne!(
                dhy, dry,
                "{family:?} sample {q:?}: the vertical is unchanged by the half turn under \
                 perspective — the magnification is not being applied to it"
            );
        }
    }
}

/// **The flat families are untouched at every `perspective`.**
///
/// Stated as invariance rather than as the mirror identity above, and the
/// difference is not pedantry: a 2D map's projection is a full *in-plane*
/// rotation, so its half turn is a point reflection (both axes negated), not
/// an `x`-mirror — that identity was never true for them and is not what
/// this change is about. What must hold is that the depth machinery is
/// **exactly the identity** here, which is what `inv_depth_extent() == 0`
/// buys: same bits at every perspective, including the ceiling.
#[test]
fn perspective_is_exactly_inert_on_a_flat_family() {
    for family in [AttractorFamily::DeJong, AttractorFamily::Clifford] {
        for q in [
            [1.2f32, -0.7, 0.0],
            [-1.5, 1.4, 0.0],
            [0.3, 0.9, 0.0],
            // A stray `z`, which a 2D particle never has — but if one did,
            // it must still not become a depth.
            [0.8, -1.1, 5.0],
        ] {
            for (cs, sn) in [REST, HALF_TURN, (0.6, 0.8)] {
                let base = projection_mirror::world(q, family, cs, sn, 0.0);
                for p in [0.25, 0.5, MAX_PERSPECTIVE] {
                    assert_eq!(
                        projection_mirror::world(q, family, cs, sn, p),
                        base,
                        "{family:?} sample {q:?} moved at perspective {p} — a flat family \
                         must have no depth to spend"
                    );
                }
            }
            // ...and the in-plane rotation is what it always was: a half turn
            // negates both axes exactly.
            let ([rx, ry], [hx, hy]) = (
                projection_mirror::world(q, family, REST.0, REST.1, 0.0),
                projection_mirror::world(q, family, HALF_TURN.0, HALF_TURN.1, 0.0),
            );
            assert_eq!((hx, hy), (-rx, -ry));
        }
    }
}

/// The magnification's arithmetic, which is what makes `perspective` legible
/// rather than magic: it means the figure's depth half-extent as a fraction
/// of the camera distance, so the near-to-far ratio is `(1 + p) / (1 - p)`.
#[test]
fn the_magnification_matches_the_documented_ratio() {
    for (p, expected) in [(0.0, 1.0), (0.5, 3.0), (MAX_PERSPECTIVE, 9.0)] {
        let near = projection_mirror::magnify(1.0, p);
        let far = projection_mirror::magnify(-1.0, p);
        assert!(
            (near / far - expected).abs() < 1e-5,
            "perspective {p} gives a near/far ratio of {:.4}, not the documented {expected}",
            near / far
        );
    }
    // The two ends ADR-0076 quotes at the ceiling.
    assert!((projection_mirror::magnify(1.0, MAX_PERSPECTIVE) - 5.0).abs() < 1e-5);
    assert!((projection_mirror::magnify(-1.0, MAX_PERSPECTIVE) - 0.5556).abs() < 1e-3);

    // At `perspective = 0` it is **exactly** 1.0 — not nearly. A multiply by
    // exactly 1.0 is an identity in IEEE arithmetic, which is what makes the
    // default byte-identical rather than merely close.
    for dn in [-1.0f32, -0.37, 0.0, 0.42, 1.0] {
        assert_eq!(projection_mirror::magnify(dn, 0.0), 1.0);
    }

    // The clamp keeps the divisor away from the singularity at `p = 1`: a
    // converged figure overruns its seed box, so `d_n` before clamping
    // reaches past 1 and an unclamped magnification would blow up.
    assert_eq!(projection_mirror::depth_norm(100.0, 1.0 / 26.0), 1.0);
    assert_eq!(projection_mirror::depth_norm(-100.0, 1.0 / 26.0), -1.0);
    // A flat family's zero extent survives even an absurd depth.
    assert_eq!(projection_mirror::depth_norm(1e30, 0.0), 0.0);
}

// -----------------------------------------------------------------------
// Atmosphere (Plan 0063 Phase 2 / ADR-0076)
// -----------------------------------------------------------------------

/// **Far material is dimmer than near material**, measured on the
/// per-particle multiplier across a sampled depth range.
///
/// Not on pixels, and not because pixels are inconvenient: which screen
/// region holds the far material depends on the spin phase, so a frame's
/// "far half" is not a fixed set of columns and an assertion about one would
/// be reading the clock. The multiplier is where the decision lives.
#[test]
fn distance_dims_the_far_material() {
    const FADE: f32 = 0.8;
    const SAMPLES: usize = 33;
    // A family that actually has depth: any non-zero inverse extent engages the
    // fade; Lorenz's is the one the shipped presets ride.
    let deep = canonical_depth(AttractorFamily::Lorenz);

    // Sample the depth range end to end. `dn = -1` is farthest, `+1` nearest.
    let dn_at = |i: usize| i as f32 / (SAMPLES - 1) as f32 * 2.0 - 1.0;
    let mean = |range: std::ops::Range<usize>| -> f32 {
        let n = range.len() as f32;
        range
            .map(|i| projection_mirror::haze(dn_at(i), FADE, deep))
            .sum::<f32>()
            / n
    };
    let far = mean(0..SAMPLES / 2);
    let near = mean(SAMPLES / 2 + 1..SAMPLES);
    println!("mean haze at depth_fade {FADE}: far half {far:.4}, near half {near:.4}");
    assert!(
        far < near * 0.8,
        "the far half is not measurably dimmer than the near half ({far:.4} against \
         {near:.4}) — `depth_fade` is not attenuating with distance"
    );

    // Monotone the whole way, not merely lower on average: a cue that
    // brightened anywhere in the middle would still pass the halves test.
    for i in 1..SAMPLES {
        let (prev, now) = (
            projection_mirror::haze(dn_at(i - 1), FADE, deep),
            projection_mirror::haze(dn_at(i), FADE, deep),
        );
        assert!(
            now > prev,
            "haze is not monotone in depth: {prev:.5} at d_n {:.3}, {now:.5} at {:.3}",
            dn_at(i - 1),
            dn_at(i)
        );
    }

    // The two ends the parameter is documented by: `depth_fade = 1` takes the
    // far end to black and leaves the near end untouched.
    assert_eq!(projection_mirror::haze(-1.0, 1.0, deep), 0.0);
    assert_eq!(projection_mirror::haze(1.0, 1.0, deep), 1.0);
    // Never negative anywhere in the clamped range — a negative deposit would
    // subtract light from the additive accumulation.
    for i in 0..SAMPLES {
        assert!(projection_mirror::haze(dn_at(i), 1.0, deep) >= 0.0);
    }
}

/// Both cues are **exactly** the identity at their defaults, which is why no
/// existing capture moves. Exact, not approximate: `1.0 - 0.0 * x` is `1.0`
/// and a multiply by it is an IEEE identity, and `0.0 * x` added to a
/// coordinate leaves its bits alone.
#[test]
fn the_atmosphere_is_off_by_default() {
    let deep = canonical_depth(AttractorFamily::Lorenz);
    for dn in [-1.0f32, -0.5, 0.0, 0.25, 1.0] {
        assert_eq!(projection_mirror::haze(dn, DEFAULT_DEPTH_FADE, deep), 1.0);
        assert_eq!(projection_mirror::depth_tint(dn, DEFAULT_DEPTH_HUE), 0.0);
        // And on a flat family the cues are inert whatever they are set to.
        // For the tint that falls out of `d_n` being identically zero (it
        // lands on the centred mid-depth value); for the haze it does NOT —
        // mid-depth is `1 - depth_fade/2`, a uniform 45% dimmer at 0.9, which
        // is design-backlog 0067 — so the fade term is zeroed by the family's
        // zero inverse extent instead (Plan 0075 Phase 2). Exactly 1.0, the
        // identity ADR-0076 always claimed.
        let flat_extent = canonical_depth(AttractorFamily::DeJong);
        assert_eq!(flat_extent, 0.0);
        let flat = projection_mirror::depth_norm(1e6, flat_extent);
        assert_eq!(projection_mirror::haze(flat, 1.0, flat_extent), 1.0);
        assert_eq!(projection_mirror::depth_tint(flat, 1.0), 0.0);
    }
    assert_eq!(DEFAULT_DEPTH_FADE, 0.0);
    assert_eq!(DEFAULT_DEPTH_HUE, 0.0);
}

/// The hue shift spans `±depth_hue/2` across the depth range and is centred,
/// so the mid-depth colour is the one the preset asked for — a cue that
/// merely *tinted* the whole picture would be a `hue` offset with extra
/// steps.
#[test]
fn the_depth_tint_is_centred_and_spans_the_parameter() {
    const HUE: f32 = 0.3;
    assert!((projection_mirror::depth_tint(1.0, HUE) - HUE / 2.0).abs() < 1e-6);
    assert!((projection_mirror::depth_tint(-1.0, HUE) + HUE / 2.0).abs() < 1e-6);
    assert_eq!(projection_mirror::depth_tint(0.0, HUE), 0.0);
    let span = projection_mirror::depth_tint(1.0, HUE) - projection_mirror::depth_tint(-1.0, HUE);
    assert!(
        (span - HUE).abs() < 1e-6,
        "the tint spans {span} across the depth range, not the {HUE} it was asked for"
    );
}

// -----------------------------------------------------------------------
// The spin (Plan 0063 Phase 3 / ADR-0076)
// -----------------------------------------------------------------------

/// A `spin` sequence that runs a while, then changes — a second of turning
/// at the shipped rate, then double, then a reversal, half a second each.
///
/// **The length is the point.** The product form's error is proportional to
/// *elapsed* time, so a sequence that changes on frame 3 barely shows it; a
/// sequence that changes after a second of accumulated rotation shows it as
/// the figure jumping a fifth of a revolution between two frames.
fn spin_ramp() -> Vec<f32> {
    let mut ramp = vec![1.0f32; 60];
    ramp.extend(std::iter::repeat_n(2.0f32, 30));
    ramp.extend(std::iter::repeat_n(-1.0f32, 30));
    ramp
}

/// **The phase is the running sum, and provably not the product.**
///
/// Under `time · spin · SPIN_RATE` a `spin` that changed between frames would
/// retroactively rescale *all* elapsed time, so the figure would snap to a
/// new angle rather than accelerate toward one. Both halves are asserted:
/// the integral matches term for term, and the product does not — computed
/// here rather than argued about.
#[test]
fn the_spin_phase_integrates_rather_than_multiplying() {
    let dt = FIXED_STEP;
    let mut spin_time = 0.0f32;
    let mut running = 0.0f32;
    let mut elapsed = 0.0f32;
    let mut worst_integrated_step = 0.0f32;
    let mut worst_multiplied_step = 0.0f32;
    let mut prev_multiplied = 0.0f32;

    let ramp = spin_ramp();
    for spin in ramp.iter().copied() {
        let before = spin_phase(spin_time);
        spin_time = advance_spin(spin_time, spin, dt);
        running += spin * SPIN_RATE * dt;
        elapsed += dt;

        // The integrated phase moves by at most one frame's worth of angle,
        // whatever the binding did. That is what "accelerates" means.
        worst_integrated_step = worst_integrated_step.max((spin_phase(spin_time) - before).abs());

        // The rejected form, evaluated alongside it.
        let multiplied = elapsed * spin * SPIN_RATE;
        worst_multiplied_step = worst_multiplied_step.max((multiplied - prev_multiplied).abs());
        prev_multiplied = multiplied;
    }

    assert!(
        (spin_phase(spin_time) - running).abs() < 1e-6,
        "the phase is {} but the running sum of spin * SPIN_RATE * dt is {running}",
        spin_phase(spin_time)
    );

    // ...and it is NOT the product. Not a near miss: the two disagree by more
    // than the whole integrated phase.
    let multiplied = elapsed * ramp.last().copied().unwrap_or(0.0) * SPIN_RATE;
    println!(
        "after {} frames: integrated {:.6} rad, multiplied {multiplied:.6} rad",
        ramp.len(),
        spin_phase(spin_time)
    );
    assert!(
        (spin_phase(spin_time) - multiplied).abs() > spin_phase(spin_time).abs(),
        "the integrated phase {} and the multiplied one {multiplied} agree — this test \
         cannot tell the two formulations apart",
        spin_phase(spin_time)
    );

    // The snap, stated as the thing a viewer would see. The largest angle one
    // frame can honestly turn through is `max|spin| * SPIN_RATE * dt`; the
    // integrated phase never exceeds it, and the product form leaps a large
    // multiple of it the instant the binding moves — because it rescales a
    // second of already-elapsed rotation.
    let frame_angle = ramp.iter().fold(0.0f32, |m, s| m.max(s.abs())) * SPIN_RATE * dt;
    println!(
        "worst single-frame phase jump: integrated {worst_integrated_step:.6}, \
         multiplied {worst_multiplied_step:.6} (one frame of rotation is {frame_angle:.6})"
    );
    assert!(
        // A relative slack, because this compares two `f32` roundings of the
        // same product, which differ in the last bits.
        worst_integrated_step <= frame_angle * (1.0 + 1e-5),
        "the integrated phase jumped {worst_integrated_step} in one frame, past the \
         {frame_angle} a frame's rotation can be"
    );
    assert!(
        worst_multiplied_step > frame_angle * 20.0,
        "the multiplied form's worst jump is only {worst_multiplied_step} against a frame's \
         {frame_angle} — this sequence does not exercise the retroactive rescale the \
         integration exists to avoid"
    );
}

/// `spin = 1` is **exactly** the rate this scene shipped with, and `spin = 0`
/// holds the figure still — the two ends the parameter is documented by.
///
/// The exactness is the load-bearing half. At `spin = 1` the accumulator is
/// `Σ dt` term for term, which is bit-for-bit the renderer's own clock, so
/// the phase equals the `time * SPIN_RATE` it replaced and no golden baseline
/// moves. Deferring the `SPIN_RATE` multiply to [`spin_phase`] is what buys
/// that; summing `spin * SPIN_RATE * dt` would drift in the last bits.
#[test]
fn the_default_spin_reproduces_the_shipped_rate_exactly() {
    let dt = FIXED_STEP;
    let (mut spin_time, mut clock) = (0.0f32, 0.0f32);
    for _ in 0..600 {
        spin_time = advance_spin(spin_time, DEFAULT_SPIN, dt);
        // The renderer's own accumulation, verbatim (`self.time += dt`).
        clock += dt;
        assert_eq!(
            spin_phase(spin_time),
            clock * SPIN_RATE,
            "the integrated phase has drifted from the clock-multiplied one it replaced"
        );
    }
    assert_eq!(DEFAULT_SPIN, 1.0);

    // `spin = 0` holds the angle fixed, exactly, however long it runs.
    let mut held = 0.0f32;
    for _ in 0..600 {
        held = advance_spin(held, 0.0, dt);
    }
    assert_eq!(spin_phase(held), 0.0, "spin = 0 must hold the figure still");

    // ...and negative reverses it, rather than being clamped away.
    let mut back = 0.0f32;
    for _ in 0..60 {
        back = advance_spin(back, -1.0, dt);
    }
    assert!(
        spin_phase(back) < 0.0,
        "a negative spin must turn the other way"
    );
}

/// The scene is actually wired to the integration — asserted on the scene's
/// own accumulator across rendered frames, since the two tests above prove
/// the arithmetic and not that anything calls it.
#[test]
fn the_scene_integrates_the_spin_it_is_given() {
    let Some(mut h) = Harness::new(AttractorFamily::Lorenz) else {
        return;
    };
    // A held figure, across the 120 frames the plan names.
    h.scene.set_param("spin", 0.0);
    h.run(120);
    assert_eq!(
        spin_phase(h.scene.spin_time),
        0.0,
        "spin = 0 did not hold the projection angle across 120 frames"
    );

    // ...then let it turn, and the phase is exactly the frames it ran for.
    h.scene.set_param("spin", 1.0);
    h.run(60);
    let mut expected = 0.0f32;
    for _ in 0..60 {
        expected = advance_spin(expected, 1.0, FIXED_STEP);
    }
    assert_eq!(
        h.scene.spin_time, expected,
        "the scene's accumulator is not the frame-by-frame integral of its `spin`"
    );
    assert!(spin_phase(h.scene.spin_time) > 0.0);
}

// -----------------------------------------------------------------------
// The continuous-flow segment (Plan 0059 Phase 3 / ADR-0069)
// -----------------------------------------------------------------------

/// `is_continuous()` pinned per family against an explicit table.
///
/// The compiler is what forces a fifth family to answer — the match is
/// exhaustive with no wildcard. This pins what it answers, and, crucially,
/// **records that its agreement with `dim == 3.0` is a coincidence of this
/// roster**. That is the same hazard ADR-0068 Alternative C declined for the
/// projection basis, and here the two properties happen to line up, which is
/// exactly the condition under which someone "simplifies" one into the other.
#[test]
fn continuity_is_pinned_per_family() {
    let table = [
        (AttractorFamily::DeJong, false),
        (AttractorFamily::Clifford, false),
        (AttractorFamily::Thomas, true),
        (AttractorFamily::Lorenz, true),
    ];
    for (family, expected) in table {
        assert_eq!(
            family.is_continuous(),
            expected,
            "{family:?} continuity must be {expected}"
        );
    }

    // Both answers are represented, so the table is not four copies of one.
    assert!(table.iter().any(|(_, c)| *c));
    assert!(table.iter().any(|(_, c)| !*c));

    // On TODAY's roster `is_continuous()` and `dim == 3.0` agree everywhere.
    // That is recorded as a coincidence, not relied on: the moment a 2-D flow
    // or a 3-D map is added this loop stops holding, and the *correct* fix is
    // to leave `is_continuous` alone. If this assertion ever fails, do not
    // re-derive continuity from `dim` to make it pass.
    for (family, continuous) in table {
        assert_eq!(
            canonical(family).projection.1 == 3.0,
            continuous,
            "{family:?}: the dim/continuity coincidence has broken, which is                  allowed - update this note, do NOT key `is_continuous` off `dim`"
        );
    }
}

/// **The segment's endpoints are this frame's `pos` and the previous frame's
/// `pos`** — asserted on the shader's inputs rather than on pixels, because
/// "is the stroke connected" is not measurable and "is there a gap" is.
///
/// Zero gap by construction is what "the beading closes" means: if `prev`
/// after frame N is bit-identical to `pos` after frame N-1, consecutive
/// segments share an endpoint exactly and the stroke has no seams.
///
/// One caveat this is honest about: the harness advances exactly one
/// [`FIXED_STEP`] per frame, so one compute step runs per frame. `prev` is the
/// position before the *step*, so under a variable `dt` that drains several
/// steps in a frame it is the last step's origin, not the frame's. The
/// shader-level contract is per step, and that is what is asserted here.
#[test]
fn a_segment_starts_where_the_last_one_ended() {
    let Some(mut h) = Harness::new(AttractorFamily::Lorenz) else {
        return;
    };
    // Past the seed, so the cloud is on the attractor and genuinely moving.
    h.run(CONVERGE_FRAMES);
    let before: Vec<[f32; 3]> = h.particles().into_iter().map(|(pos, _)| pos).collect();
    h.run(1);
    let after = h.particles();

    let mut moved = 0usize;
    for (i, ((pos, prev), was)) in after.iter().zip(before.iter()).enumerate() {
        assert_eq!(
            prev, was,
            "particle {i}: this frame's segment starts at {prev:?}, but last                  frame ended at {was:?} - the stroke has a gap"
        );
        if pos != prev {
            moved += 1;
        }
    }
    // Non-vacuity: the particles actually advanced, so the equality above is
    // a statement about a moving cloud and not about a stalled one.
    assert!(
        moved * 2 > after.len(),
        "only {moved} of {} particles moved in a frame - a stalled cloud would              satisfy the endpoint check trivially",
        after.len()
    );
}

/// A discrete map never takes the segment branch, checked where it is decided
/// rather than in the pixels: the draw uniform's streak slot.
///
/// The pixel-level version of this claim is the golden baseline plus the
/// byte-identity captures, which is where a chord across a De Jong's
/// scattered successive points would actually show up.
#[test]
fn only_continuous_families_ask_for_a_segment() {
    // The value the draw uniform actually carries, per family — the shader
    // tests `!= 0.0`, so this is the decision point.
    assert_eq!(streak_flag(AttractorFamily::DeJong.is_continuous()), 0.0);
    assert_eq!(streak_flag(AttractorFamily::Clifford.is_continuous()), 0.0);
    assert_eq!(streak_flag(AttractorFamily::Thomas.is_continuous()), 1.0);
    assert_eq!(streak_flag(AttractorFamily::Lorenz.is_continuous()), 1.0);

    // The reseed streak ships suppressed and Plan 0059 Phase 4 decides it
    // (ADR-0069). Pinned so the provisional default is a value someone chose
    // rather than whatever a later edit last left it at.
    #[expect(
        clippy::assertions_on_constants,
        reason = "the constancy is the point: this pins a provisional default                       that Phase 4 owns flipping"
    )]
    {
        assert!(
            !RESEED_DRAWS_STREAK,
            "the reseed streak ships off; Plan 0059 Phase 4 owns flipping it"
        );
    }
}

// -----------------------------------------------------------------------
// Sample density (Plan 0059 Phase 2 / ADR-0069)
// -----------------------------------------------------------------------

/// `density` resolves against the budget, rounds, and never leaves the range
/// a draw can survive.
#[test]
fn density_resolves_against_the_tier_budget() {
    let floor = TierConfig::FLOOR.attractor_particles;
    let rich = TierConfig::RICH.attractor_particles;
    assert_eq!(active_particles(floor, 1.0), floor);
    assert_eq!(active_particles(rich, 1.0), rich);
    assert_eq!(active_particles(floor, 0.5), 25_000);

    // The documented floor, at both tiers. These are the numbers
    // `MIN_PARTICLE_DENSITY`'s rationale is written against — and that
    // rationale is a rendered capture, so pinning them here is what keeps the
    // doc comment honest if the constant is ever nudged.
    assert_eq!(active_particles(floor, MIN_PARTICLE_DENSITY), 25);
    assert_eq!(active_particles(rich, MIN_PARTICLE_DENSITY), 75);

    // The captures that set the floor were taken at these fractions, so the
    // counts they correspond to are pinned too.
    assert_eq!(active_particles(floor, 0.01), 500);
    assert_eq!(active_particles(floor, 0.002), 100);

    // Never zero (a scene drawing nothing would look like a hang, not an
    // error) and never above the allocation (an out-of-bounds vertex fetch).
    assert_eq!(active_particles(1, MIN_PARTICLE_DENSITY), 1);
    assert_eq!(active_particles(floor, 1.0), floor);
}

/// **Total deposited light is invariant across `density`** — the property that
/// makes this key structural rather than an exposure control (ADR-0069 on top
/// of ADR-0065).
///
/// Asserted on the value, like ADR-0065's own scalar: `active * scale(active)`
/// is the frame's total weight, and it must not depend on `active`. Were it
/// not so, lowering `density` would dim the picture and every author would
/// have to re-tune exposure to change sample count — which is precisely the
/// trap ADR-0065 removed one plan earlier.
#[test]
fn total_deposited_light_is_invariant_across_density() {
    let floor = TierConfig::FLOOR.attractor_particles;
    let reference = floor as f64 * f64::from(deposit_scale(floor));
    for density in [1.0, 0.5, 0.25, 0.1, MIN_PARTICLE_DENSITY] {
        let active = active_particles(floor, density);
        let total = f64::from(active) * f64::from(deposit_scale(active));
        assert!(
            (total - reference).abs() < 1e-6 * reference,
            "density {density} draws {active} particles for total light {total},                  against {reference} at full density"
        );
    }
    // Non-vacuity: the counts genuinely differ, so the constancy above is a
    // property of the scalar and not of an unchanging `active`.
    assert_ne!(
        active_particles(floor, 1.0),
        active_particles(floor, MIN_PARTICLE_DENSITY)
    );
}

/// **A density change rebuilds no GPU resource**, asserted where it can fail
/// rather than by reading the code: the particles past `active_count` still
/// hold their seeded positions after the cloud has run.
///
/// This is the same claim as "the buffer stays allocated at the tier budget
/// and the compute early-returns" (ADR-0069), stated as an observable. It also
/// proves the guard is what does the work — a dispatch that ran over the whole
/// buffer, or a reallocation sized to `active`, both fail here.
#[test]
fn the_tail_beyond_the_active_count_never_moves() {
    const DENSITY: f32 = 0.25;
    let Some(mut h) = Harness::with_density(AttractorFamily::DeJong, DENSITY) else {
        return;
    };
    let active = active_particles(TEST_PARTICLES, DENSITY) as usize;
    assert!(active > 0 && active < TEST_PARTICLES as usize);
    let seeded: Vec<[f32; 3]> = h.scene.seed_particles.iter().map(|p| p.pos).collect();

    h.run(CONVERGE_FRAMES);
    let after = h.positions();

    // The tail is untouched, bit for bit. Not an epsilon: these particles were
    // never dispatched, so anything but equality means they were.
    for (i, (now, seed)) in after.iter().zip(seeded.iter()).enumerate().skip(active) {
        assert_eq!(
            now, seed,
            "particle {i} is past the active count {active} but moved from its seed"
        );
    }

    // Non-vacuity: the active head DID move, so the equality above is a real
    // constraint and not a report that nothing ran at all.
    let moved = after
        .iter()
        .zip(seeded.iter())
        .take(active)
        .filter(|(now, seed)| now != seed)
        .count();
    assert!(
        moved * 2 > active,
        "only {moved} of {active} active particles moved - the dispatch did not run,              so the inert tail proves nothing"
    );
}

// -----------------------------------------------------------------------
// The reseed (Plan 0057 Phase 3 / ADR-0066)
// -----------------------------------------------------------------------

/// Particles for the reseed tests. Small: the property is about *where* the
/// points are, not how many, and a WARP dispatch of 50 000 buys nothing here.
const TEST_PARTICLES: u32 = 4_096;
/// Frames run before a reseed, so the cloud has converged onto the attractor
/// and its measured extent is the attractor's rather than the seed box's.
const CONVERGE_FRAMES: u32 = 120;

/// A scene driven straight, with no `Renderer`: this is a claim about the
/// particle buffer, and going through a renderer would only add ways for the
/// capture path to be what is under test.
struct Harness {
    ctx: RenderContext,
    scene: AttractorScene,
    target: wgpu::TextureView,
}

impl Harness {
    /// `None` on a runner with no GPU adapter at all (ADR-0016). **Software
    /// adapters run this**, like the rest of the differential suite.
    ///
    /// An earlier draft skipped WARP, on the evidence that the compute
    /// dispatch there had no effect — the particle buffer came back
    /// bit-identical to the seeded scatter after 120 frames, with the right
    /// group count, the right uniform, and no validation error. That evidence
    /// was real and the conclusion was wrong: the cause was this scene's own
    /// second bind group aliasing the first on WARP (see
    /// `PipelineResources::build`), so the step dispatch read a zeroed uniform
    /// and returned on `count = 0`. Fixing that fixed the adapter. **A skip
    /// added to route around a symptom would have hidden the defect and left
    /// this test green and vacuous on CI**, which is the shape the skip was
    /// about to take.
    fn new(family: AttractorFamily) -> Option<Self> {
        Self::with_density(family, 1.0)
    }

    /// As [`Self::new`], at a chosen `[particles] density`. The buffer is
    /// still `TEST_PARTICLES` either way — that is the point of the key.
    fn with_density(family: AttractorFamily, density: f32) -> Option<Self> {
        let ctx = match RenderContext::new_headless(64, 64, true) {
            Ok(ctx) => ctx,
            Err(crate::render::RenderError::RequestAdapter(_)) => {
                eprintln!("skipped: no GPU adapter on this runner (ADR-0016)");
                return None;
            }
            Err(e) => panic!("headless context build failed: {e}"),
        };
        // COMPOSITE_FORMAT, not the surface's: every scene upstream of the
        // tonemap is built against it, and a mismatch here fails render-pass
        // validation on submit, which discards the WHOLE command buffer -
        // compute dispatches included. The first draft of this harness used
        // `surface_format()` and read back a cloud that had never stepped.
        let mut scene = AttractorScene::new(
            &ctx.device,
            crate::render::COMPOSITE_FORMAT,
            TEST_PARTICLES,
            TierConfig::FLOOR.attractor_trail_cap,
        );
        scene.configure(&crate::render::scenes::lines::GeneratorConfig::Particles {
            family,
            density,
            morph_to: None,
            tuple_path: None,
        });
        scene.set_target_size(64, 64);
        let target = ctx
            .device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("attractor-test-target"),
                size: wgpu::Extent3d {
                    width: 64,
                    height: 64,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: crate::render::COMPOSITE_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            })
            .create_view(&wgpu::TextureViewDescriptor::default());
        Some(Self { ctx, scene, target })
    }

    /// As [`Self::new`], walking a measured path between two roster entries
    /// (ADR-0093) instead of sitting on one.
    fn with_path(family: AttractorFamily, from: u32, to: u32) -> Option<Self> {
        let mut h = Self::with_density(family, 1.0)?;
        h.scene
            .configure(&crate::render::scenes::lines::GeneratorConfig::Particles {
                family,
                density: 1.0,
                morph_to: None,
                tuple_path: Some((from, to)),
            });
        Some(h)
    }

    /// Advance and render `frames` frames at the fixed capture `dt`.
    fn run(&mut self, frames: u32) {
        for _ in 0..frames {
            self.scene.advance(FIXED_STEP);
            self.scene.update(&AnalysisFrame::default());
            let mut encoder = self
                .ctx
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
            self.scene
                .render(&self.ctx.queue, &mut encoder, &self.target, 1.0);
            self.ctx.queue.submit(std::iter::once(encoder.finish()));
        }
    }

    /// Drive one `reseed` rising edge and render the frame it lands on.
    fn reseed(&mut self) {
        self.scene.set_param("reseed", 1.0);
        self.run(1);
        self.scene.set_param("reseed", 0.0);
    }

    /// A `reseed` with **no fixed step alongside it** — the kick, isolated.
    ///
    /// `advance(0.0)` leaves the accumulator with nothing to drain, so the frame
    /// encodes the jitter dispatch and zero step dispatches. That matters for any
    /// claim about the kick's *magnitude*: on a fast flow one step's own travel
    /// dwarfs the disturbance, so a before/after difference across a normal frame
    /// measures the attractor rather than the reseed.
    fn kick_only(&mut self) {
        self.scene.set_param("reseed", 1.0);
        self.scene.advance(0.0);
        self.scene.update(&AnalysisFrame::default());
        let mut encoder = self
            .ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        self.scene
            .render(&self.ctx.queue, &mut encoder, &self.target, 1.0);
        self.ctx.queue.submit(std::iter::once(encoder.finish()));
        self.scene.set_param("reseed", 0.0);
    }

    fn positions(&self) -> Vec<[f32; 3]> {
        self.raw().into_iter().map(|p| p.pos).collect()
    }

    /// `(pos, prev)` per particle — the pair ADR-0069's segment is drawn
    /// between.
    fn particles(&self) -> Vec<([f32; 3], [f32; 3])> {
        self.raw().into_iter().map(|p| (p.pos, p.prev)).collect()
    }

    /// The whole particle buffer, for the assertions that are about
    /// ADR-0087's channels rather than about position.
    fn raw(&self) -> Vec<Particle> {
        self.scene
            .read_particles(&self.ctx.queue)
            .expect("resources exist after a render")
    }
}

/// Per-axis min/max over a position set.
fn extent(points: &[[f32; 3]]) -> ([f32; 3], [f32; 3]) {
    let mut lo = [f32::INFINITY; 3];
    let mut hi = [f32::NEG_INFINITY; 3];
    for p in points {
        for k in 0..3 {
            let (Some(l), Some(h), Some(v)) = (lo.get_mut(k), hi.get_mut(k), p.get(k)) else {
                continue;
            };
            *l = l.min(*v);
            *h = h.max(*v);
        }
    }
    (lo, hi)
}

/// Cells per axis in the occupancy grid below.
const OCCUPANCY_CELLS: i32 = 24;

/// The region a converged cloud actually occupies, as a set of voxels over its
/// own bounding box.
///
/// **The bounding box is the wrong instrument and measuring it is how the first
/// draft of this test went wrong.** Every family's `seed_box` is sized to the
/// attractor's native extent, so the two boxes agree to within a percent —
/// measured, De Jong converges to `±1.499` against a `±1.5` box and Lorenz's x
/// to `±19.99` against `±20`. What the box cannot see is that an attractor is a
/// *filigree*: it occupies a small fraction of its own bounding volume, and a
/// uniform re-fill of that volume is off the figure almost everywhere while
/// staying entirely inside its extent. Occupancy is the measure that separates
/// them, and it is what ADR-0066 means by "the points stay on the attractor".
struct Occupancy {
    cells: std::collections::HashSet<(i32, i32, i32)>,
    lo: [f32; 3],
    scale: [f32; 3],
}

impl Occupancy {
    fn of(points: &[[f32; 3]]) -> Self {
        let (lo, hi) = extent(points);
        let mut scale = [1.0f32; 3];
        for k in 0..3 {
            let (Some(s), Some(&l), Some(&h)) = (scale.get_mut(k), lo.get(k), hi.get(k)) else {
                continue;
            };
            *s = OCCUPANCY_CELLS as f32 / (h - l).max(f32::EPSILON);
        }
        let mut me = Self {
            cells: std::collections::HashSet::new(),
            lo,
            scale,
        };
        for p in points {
            me.cells.insert(me.cell(p));
        }
        me
    }

    fn cell(&self, p: &[f32; 3]) -> (i32, i32, i32) {
        let at = |k: usize| -> i32 {
            let (Some(&v), Some(&l), Some(&s)) = (p.get(k), self.lo.get(k), self.scale.get(k))
            else {
                return 0;
            };
            ((v - l) * s).floor() as i32
        };
        (at(0), at(1), at(2))
    }

    /// Fraction of `points` landing more than one cell away from anything the
    /// converged cloud occupied.
    ///
    /// **The one-cell dilation is the unit of the measurement, not slack.** The
    /// kick is `0.09` in a figure spanning `3.0` over 24 cells, so it is 0.72
    /// of a cell — a disturbed point routinely lands in the *neighbouring*
    /// cell, and on a figure this sparse a neighbouring cell is usually empty.
    /// Counting that as "off the attractor" would measure the grid's phase
    /// rather than the behaviour. Beyond one cell is a displacement the kick
    /// cannot produce.
    ///
    /// It costs the instrument nothing it needs: the figure occupies ~1.6 % of
    /// its bounding volume, so even dilated it is a small part of the box and a
    /// uniform re-fill still reads overwhelmingly outside — which the test
    /// asserts rather than assumes.
    fn fraction_outside(&self, points: &[[f32; 3]]) -> f32 {
        if points.is_empty() {
            return 0.0;
        }
        let near = |p: &[f32; 3]| {
            let (cx, cy, cz) = self.cell(p);
            for dx in -1..=1 {
                for dy in -1..=1 {
                    for dz in -1..=1 {
                        if self.cells.contains(&(cx + dx, cy + dy, cz + dz)) {
                            return true;
                        }
                    }
                }
            }
            false
        };
        let out = points.iter().filter(|p| !near(p)).count();
        out as f32 / points.len() as f32
    }

    /// Fraction of the bounding box's cells the cloud actually occupies — the
    /// number that makes the filigree claim concrete.
    fn filled(&self) -> f32 {
        let total = OCCUPANCY_CELLS.pow(3) as f32;
        self.cells.len() as f32 / total
    }
}

/// **The Phase 3 done-when, over the particle buffer rather than the pixels.**
///
/// Two directions, and both are load-bearing. A reseed must not throw particles
/// outside the attractor's own converged extent — which is exactly what
/// re-filling `seed_box` did, since the box is sized to the family's *native*
/// extent and much of its volume is off the figure. And the positions must
/// actually *change*, because a reseed that quietly did nothing would satisfy
/// the first half perfectly.
///
/// **The control is the behaviour this replaces.** Rather than argue about a
/// threshold, the same measurement is taken over the exact population the old
/// re-fill would have produced — `AttractorScene::seed`, unchanged and still
/// used for the initial fill — so the test states the two behaviours' readings
/// side by side and asserts they are far apart in the right direction.
#[test]
fn a_reseed_disturbs_the_cloud_without_leaving_the_attractor() {
    // De Jong: the family the artifact was reported on (`attractor_ink`).
    const FAMILY: AttractorFamily = AttractorFamily::DeJong;
    let Some(mut h) = Harness::new(FAMILY) else {
        return;
    };
    h.run(CONVERGE_FRAMES);
    let before = h.positions();
    let occupied = Occupancy::of(&before);

    // The premise, asserted rather than assumed: the attractor must be a
    // filigree inside its own bounding box, or "off the figure" and "outside
    // the box" would name the same region and a re-fill would pass this test.
    let filled = occupied.filled();
    println!(
        "converged cloud fills {:.1}% of its own bounding volume ({} of {} cells)",
        filled * 100.0,
        occupied.cells.len(),
        OCCUPANCY_CELLS.pow(3)
    );
    assert!(
        filled < 0.5,
        "the converged cloud fills {filled:.3} of its bounding box — it is not \
         sparse enough for occupancy to distinguish a re-fill from a jitter"
    );

    h.reseed();
    let after = h.positions();
    assert_eq!(after.len(), before.len(), "the population size is fixed");

    // Direction 1 — the positions actually changed. A reseed that quietly did
    // nothing would satisfy direction 2 perfectly.
    let moved = before
        .iter()
        .zip(after.iter())
        .filter(|(a, b)| a != b)
        .count();
    assert!(
        moved * 10 > before.len() * 9,
        "a reseed must disturb essentially the whole cloud, moved {moved} of {}",
        before.len()
    );

    // Direction 2 — and the cloud is still on the attractor. Measured against
    // the old behaviour under the identical instrument.
    let jittered_outside = occupied.fraction_outside(&after);
    let refilled: Vec<[f32; 3]> = canonical_seed(FAMILY, TEST_PARTICLES)
        .iter()
        .map(|p| p.pos)
        .collect();
    let refill_outside = occupied.fraction_outside(&refilled);
    println!(
        "off the figure after a reseed: jitter {:.1}%, the old seed-box re-fill \
         {:.1}%",
        jittered_outside * 100.0,
        refill_outside * 100.0
    );

    // These read **0.0 % and 100.0 %** as measured, so the bounds below are not
    // thresholds chosen to admit the result: the two behaviours sit at opposite
    // ends of the instrument, and the margins exist for adapter variation
    // rather than for the claim.
    assert!(
        jittered_outside < 0.02,
        "a reseed put {:.1}% of the cloud off the attractor; the kick is ±{:?} \
         in a figure spanning {:?}",
        jittered_outside * 100.0,
        canonical(FAMILY).jitter_extent(),
        extent(&before)
    );
    // Non-vacuity, and the half that makes this a test of ADR-0066 rather than
    // of arithmetic: the instrument must be able to see the behaviour that was
    // replaced, or the assertion above is satisfied by any measurement at all.
    assert!(
        refill_outside > 0.9,
        "the seed-box re-fill reads only {:.1}% off the figure — this instrument \
         cannot see the behaviour ADR-0066 replaced, so it proves nothing about \
         the one that replaced it",
        refill_outside * 100.0
    );
}

/// Determinism (`particles/mod.rs`'s pure-function-of-seed-and-step-sequence
/// claim, NFR §6): two runs of the same input sequence produce **identical**
/// positions after a reseed. The jitter is the one thing here that could have
/// broken it, since it is the only per-particle randomness applied after init.
#[test]
fn a_reseed_is_reproducible_from_the_same_seed() {
    let run = || -> Option<Vec<[f32; 3]>> {
        let mut h = Harness::new(AttractorFamily::DeJong)?;
        h.run(30);
        h.reseed();
        h.run(3);
        Some(h.positions())
    };
    let (Some(a), Some(b)) = (run(), run()) else {
        return;
    };
    assert_eq!(a, b, "the cloud after a reseed is not reproducible");

    // ...and the reseed is genuinely doing something, so the equality above is
    // not two identical no-ops agreeing.
    let Some(mut h) = Harness::new(AttractorFamily::DeJong) else {
        return;
    };
    h.run(30);
    let unreseeded = {
        h.run(4);
        h.positions()
    };
    assert_ne!(
        a, unreseeded,
        "a reseeded run and an unreseeded one are identical — the jitter did nothing"
    );
}

/// Successive reseeds must kick a given particle *differently*. Salting the
/// hash with the particle's seed alone would apply one fixed displacement field
/// every time, which over a session is a rigid pattern rather than a
/// disturbance — and it is invisible in a single-reseed test.
#[test]
fn successive_reseeds_kick_in_different_directions() {
    let Some(mut h) = Harness::new(AttractorFamily::DeJong) else {
        return;
    };
    h.run(60);
    let base = h.positions();
    h.reseed();
    let first: Vec<[f32; 3]> = h.positions();

    // The displacement the first reseed applied, per particle. Includes the
    // frame's own step, which is common to both reseeds and so cancels in the
    // comparison below.
    let delta = |from: &[[f32; 3]], to: &[[f32; 3]]| -> Vec<[f32; 3]> {
        from.iter()
            .zip(to.iter())
            .map(|(a, b)| {
                [
                    b.first().unwrap_or(&0.0) - a.first().unwrap_or(&0.0),
                    b.get(1).unwrap_or(&0.0) - a.get(1).unwrap_or(&0.0),
                    b.get(2).unwrap_or(&0.0) - a.get(2).unwrap_or(&0.0),
                ]
            })
            .collect()
    };
    let d1 = delta(&base, &first);

    let second_base = h.positions();
    h.reseed();
    let d2 = delta(&second_base, &h.positions());

    let identical = d1.iter().zip(d2.iter()).filter(|(a, b)| a == b).count();
    assert!(
        identical * 10 < d1.len(),
        "two reseeds applied the same displacement to {identical} of {} particles — \
         the hash is not salted by the reseed counter",
        d1.len()
    );
}

/// The `Particle` layout the storage buffer and the vertex attributes both
/// assume: two tight 16-byte std430 halves, then ADR-0087's two channels,
/// ADR-0088's third, and the one word WGSL's 16-byte round-up still leaves
/// behind — stride **48**. The readback above casts raw bytes to this, so a
/// change here silently reinterprets every position.
///
/// It was a tight 16 until ADR-0069 added `prev`, and 32 until ADR-0087
/// added `age` and `map`. **The offsets matter more than the size**, and the
/// reason is the trap Plan 0073 walked into: `vertex_attr_array!` lays its
/// attributes out *consecutively*, so a fourth entry would have fetched
/// `map` from offset 28 — the padding word — rather than from 36, silently
/// and with no compile error. [`PARTICLE_ATTRIBUTES`] spells the offsets out
/// and this test is what holds them to the struct.
///
/// **The size not moving is half the claim.** ADR-0088 spends one of the two
/// words ADR-0087 reserved rather than growing a struct four families share,
/// so `root` arriving at 48 bytes is the decision, not a coincidence.
#[test]
fn the_particle_layout_carries_three_channels() {
    assert_eq!(std::mem::size_of::<Particle>(), 48);
    assert_eq!(std::mem::align_of::<Particle>(), 4);

    // The offsets the vertex layout hard-codes, measured rather than assumed.
    let p = Particle {
        pos: [0.0; 3],
        seed: 0.0,
        prev: [0.0; 3],
        _pad: 0.0,
        age: 0.0,
        map: 0.0,
        root: 0.0,
        _spare: 0.0,
    };
    let base = std::ptr::from_ref(&p) as usize;
    let offset_of = |field: &f32| std::ptr::from_ref(field) as usize - base;
    assert_eq!(std::ptr::from_ref(&p.pos) as usize - base, 0);
    assert_eq!(offset_of(&p.seed), 12);
    assert_eq!(std::ptr::from_ref(&p.prev) as usize - base, 16);
    assert_eq!(offset_of(&p.age), 32);
    assert_eq!(offset_of(&p.map), 36);
    assert_eq!(offset_of(&p.root), 40);
    // One word left. Named here because ADR-0088's Consequences turn on it:
    // the next per-particle channel after this one is a struct change.
    assert_eq!(offset_of(&p._spare), 44);

    // ...and the vertex attributes agree with them. This is the assertion
    // that would have caught the consecutive-layout bug: it compares the
    // constant the pipeline is built from against the struct itself.
    //
    // Locations 3 and 4 are deliberately **out of offset order**: `map` took
    // location 3 when Plan 0073 Phase 1 added it and `age` took location 4 in
    // Phase 3, so the locations follow the order the channels shipped rather
    // than the order they sit in the struct. Each attribute carries its own
    // offset so nothing depends on the ordering — spelled out because it
    // reads like a mistake.
    let attr_offsets: Vec<u64> = PARTICLE_ATTRIBUTES.iter().map(|a| a.offset).collect();
    assert_eq!(attr_offsets, vec![0, 12, 16, 36, 32, 40]);
    let locations: Vec<u32> = PARTICLE_ATTRIBUTES
        .iter()
        .map(|a| a.shader_location)
        .collect();
    assert_eq!(locations, vec![0, 1, 2, 3, 4, 5]);

    // The memory this costs, at both tier budgets — ADR-0087 quotes the rise
    // from 1.6/4.8 MB, so it fails here if the struct grows again.
    let floor = TierConfig::FLOOR.attractor_particles as usize;
    let rich = TierConfig::RICH.attractor_particles as usize;
    assert_eq!(floor * size_of::<Particle>(), 2_400_000);
    assert_eq!(rich * size_of::<Particle>(), 7_200_000);
}

/// **Backlog 0064's rectangle stops existing, stated as a count** (Plan 0073
/// Phase 2).
///
/// "The initial fill is not a box" is exactly this: an IFS seeds at most as
/// many *distinct* positions as its figure has drawn maps — at most four —
/// where a box fill seeds `count` of them. There is no statistic to converge
/// and no tolerance to tune: a box fill cannot pass it and a fixed-point fill
/// cannot fail it.
///
/// The box families are asserted in the same test rather than a separate one,
/// because the number that makes this claim mean anything is the *contrast* —
/// `count` against four.
#[test]
fn the_ifs_fill_is_four_points_and_a_box_fill_is_every_point() {
    const COUNT: u32 = 512;

    // Compared as bit patterns: two particles at the same position are the
    // same position, and `f32` has no equality wrinkle here that `to_bits`
    // does not settle exactly.
    fn distinct(particles: &[Particle]) -> usize {
        particles
            .iter()
            .map(|p| p.pos.map(f32::to_bits))
            .collect::<std::collections::BTreeSet<_>>()
            .len()
    }

    for figure in ifs::IfsFigure::ALL {
        let family = AttractorFamily::Ifs(figure);
        let drawn = figure.table().maps.iter().filter(|m| m.p > 0.0).count();
        let seeded = distinct(&canonical_seed(family, COUNT));
        assert!(
            seeded <= drawn,
            "{figure:?} seeded {seeded} distinct positions for {drawn} drawn maps — \
             the fill is not landing on the fixed points"
        );
        assert!(
            seeded >= 2,
            "{figure:?} seeded {seeded} distinct positions: a fill collapsed to one \
             point is not a figure either"
        );
    }

    // ...and the contrast, on the families that still fill a box. Every
    // particle lands somewhere of its own, which is the artifact ADR-0066
    // named and the reason the IFS number above is worth asserting.
    for family in [
        AttractorFamily::DeJong,
        AttractorFamily::Clifford,
        AttractorFamily::Thomas,
        AttractorFamily::Lorenz,
    ] {
        assert_eq!(
            distinct(&canonical_seed(family, COUNT)),
            COUNT as usize,
            "{family:?} should still scatter over its seed box — this plan changes \
             only what the IFS writes"
        );
    }
}

/// One particle under the CPU reference of the step shader's IFS arm.
///
/// **The WGSL is the source and this is the mirror**, the discipline
/// `projection_mirror` follows. It exists because the property Plan 0073
/// Phase 3 has to establish — that no reachable table sends a particle
/// anywhere unbounded, respawns included — is about *positions*, and a
/// capture could only report that the picture looked plausible.
struct ChurnState {
    at: [f32; 2],
    age: f32,
    /// The particle's fixed seed. Every draw below is a pure function of it
    /// and the step index, which is the determinism claim itself.
    seed: f32,
}

/// A resolved table in the form the reference steps against - the same two
/// things the GPU is handed on the step uniform, resolved once per table
/// rather than once per particle per step.
struct ChurnTable {
    maps: [ifs::Affine; ifs::MAPS],
    cumulative_p: [f32; ifs::MAPS],
    fixed: [[f32; 2]; ifs::MAPS],
}

impl ChurnTable {
    fn resolve(table: &ifs::IfsTable) -> Self {
        Self {
            maps: table.maps.map(|m| m.to_affine()),
            cumulative_p: ifs::pack(table).cumulative_p,
            fixed: ifs::fixed_points(table),
        }
    }
}

/// The four fixed points as the **shader** reads them — unpacked back out of
/// the two `vec4` rows [`ifs::pack`] laid them into, exactly as
/// `ifs_fixed_point()` in [`STEP_SHADER`](super::STEP_SHADER) does.
///
/// Deliberately *not* a second call to [`ifs::fixed_points`]: the whole point
/// of routing through the packed rows is that the packing itself — which
/// point lands in which half of which row — is under test.
fn packed_points(packed: &ifs::IfsPacked) -> [[f32; 2]; ifs::MAPS] {
    let [row0, row1] = packed.fixed;
    let [a, b, c, d] = row0;
    let [e, f, g, h] = row1;
    [[a, b], [c, d], [e, f], [g, h]]
}

/// Mirrors `ifs_root_distance()` in [`STEP_SHADER`](super::STEP_SHADER) —
/// ADR-0088's channel, from the uniform's own bytes.
///
/// **The WGSL is the source and this is the mirror**, the discipline
/// `projection_mirror` follows against the draw shader. It exists because the
/// claim is about a *number* — that the stored value is the normalised
/// nearest-point distance and is exactly zero at a fixed point — and a
/// capture could only say the picture had colours in it.
fn root_distance(q: [f32; 2], packed: &ifs::IfsPacked) -> f32 {
    let [qx, qy] = q;
    packed_points(packed)
        .into_iter()
        .fold(f32::INFINITY, |acc, [x, y]| acc.min((qx - x).hypot(qy - y)))
        * packed.root_recip
}

impl ChurnState {
    /// Advance one fixed step. Returns whether the particle respawned.
    fn step(&mut self, step_index: u32, table: &ChurnTable) -> bool {
        let [c0, c1, c2, _] = table.cumulative_p;
        // The shader's map draw, salted by the step counter.
        let r = super::hash_unit(self.seed.to_bits() ^ step_index.wrapping_mul(0x9E37_79B9));
        let k = if r < c0 {
            0
        } else if r < c1 {
            1
        } else if r < c2 {
            2
        } else {
            ifs::MAPS - 1
        };
        let [x, y] = self.at;
        if let Some(m) = table.maps.get(k) {
            self.at = [m.a * x + m.b * y + m.e, m.c * x + m.d * y + m.f];
        }

        self.age += 1.0;
        if self.age < super::churn_lifetime(self.seed) {
            return false;
        }
        // The respawn, salted differently so where a particle restarts does
        // not correlate with how long it lived.
        let u = super::hash_unit(self.seed.to_bits() ^ step_index.wrapping_mul(0x85EB_CA6B));
        let slot = ((u * ifs::MAPS as f32) as usize).min(ifs::MAPS - 1);
        if let Some(point) = table.fixed.get(slot) {
            self.at = *point;
        }
        self.age = 0.0;
        true
    }
}

/// The churn constants the CPU and the shader **both** need, held to one
/// source (Plan 0073 Phase 3).
///
/// `seed()` places every particle at a point inside a lifetime the *shader*
/// computes, so a disagreement would seed ages outside the life they are
/// measured against — particles respawning on their first step, or never.
/// The WGSL cannot import a Rust constant, so the shader carries literals and
/// this asserts they are the right literals. Crude, and it is the failure
/// mode that would otherwise be silent.
#[test]
fn the_churn_constants_agree_between_rust_and_wgsl() {
    let [lo, hi] = super::CHURN_LIFETIME_SPREAD;
    for expected in [
        format!("const LIFETIME_STEPS: f32 = {:.1};", super::CHURN_LIFETIME),
        format!("const LIFETIME_LO: f32 = {lo:.1};"),
        format!("const LIFETIME_HI: f32 = {hi:.1};"),
        format!("const LIFETIME_SALT: u32 = 0x{:X}u;", super::LIFETIME_SALT),
    ] {
        assert!(
            super::STEP_SHADER.contains(&expected),
            "the step shader should carry `{expected}` — the Rust constant moved \
             and the WGSL literal did not"
        );
    }
}

/// **ADR-0088's channel is what it claims to be** (Plan 0074 Phase 1, claim
/// 2), asserted on the CPU transcription rather than on pixels.
///
/// Two claims, and they are different. The first is that the value routed
/// through the packed uniform rows — which half of which `vec4` each point
/// landed in, and the reciprocal alongside them — agrees with the distance
/// computed straight from [`ifs::fixed_points`] and [`ifs::skeleton_scale`].
/// A packing that transposed two points would still produce a plausible
/// gradient and would fail here.
///
/// The second is that a particle **at** a fixed point reads **exactly** `0`.
/// That is the respawn's own state, so it is one end of the ramp rather than
/// a special case, and "approximately zero" would not be the same claim —
/// the emergence ramp lands a thousand particles a frame on exactly those
/// points.
#[test]
fn the_root_channel_measures_distance_to_the_nearest_fixed_point() {
    // Positions sampled by running the reference chaos game, so they are on
    // the figure rather than on a grid over its bounding box.
    const WARMUP: u32 = 64;
    const SAMPLES: u32 = 256;

    for figure in ifs::IfsFigure::ALL {
        // Morphed as well as static: the fixed points travel across a morph,
        // and the packing has to follow them.
        for (target, morph) in [(figure, 0.0), (ifs::IfsFigure::Sierpinski, 0.4)] {
            let table = ifs::resolve(&figure.table(), &target.table(), morph, Levers::NEUTRAL);
            let packed = ifs::pack(&table);
            let reference = ChurnTable::resolve(&table);
            let points = ifs::fixed_points(&table);
            let recip = 1.0 / ifs::skeleton_scale(&table);

            // At each fixed point itself: exactly zero, compared on bits.
            for (k, p) in points.into_iter().enumerate() {
                let d = root_distance(p, &packed);
                assert_eq!(
                    d.to_bits(),
                    0.0f32.to_bits(),
                    "{figure:?} -> {target:?} at morph {morph}: a particle sitting on \
                     fixed point {k} reads {d}, not an exact 0 — the respawn's own \
                     state is one end of this channel's ramp"
                );
            }

            // ...and on the figure, against a distance computed without ever
            // touching the packed rows.
            let mut state = ChurnState {
                at: points.first().copied().unwrap_or([0.0, 0.0]),
                age: 0.0,
                seed: 0.375,
            };
            for step in 0..WARMUP {
                state.step(step, &reference);
            }
            let mut seen_far = false;
            for step in WARMUP..WARMUP + SAMPLES {
                state.step(step, &reference);
                let [qx, qy] = state.at;
                let direct = points
                    .into_iter()
                    .fold(f32::INFINITY, |acc, [x, y]| acc.min((qx - x).hypot(qy - y)))
                    * recip;
                let got = root_distance(state.at, &packed);
                // `f32` rounding on two paths through the same arithmetic:
                // the operations are identical, so this is tight on purpose.
                assert!(
                    (got - direct).abs() <= 1e-6 * direct.max(1.0),
                    "{figure:?} -> {target:?} at morph {morph}, step {step}: the \
                     packed rows give {got} where the points give {direct}"
                );
                seen_far |= got > 0.1;
            }
            // Non-vacuity: a table whose orbit never left the fixed points
            // would satisfy every assertion above with a column of zeros.
            assert!(
                seen_far,
                "{figure:?} -> {target:?} at morph {morph}: every sample sat within \
                 0.1 of a fixed point — the reference is not exploring the figure"
            );
        }
    }
}

/// **The mirror above is held to the shader it transcribes** (Plan 0074
/// Phase 1), the way `the_churn_constants_agree_between_rust_and_wgsl` holds
/// the churn's literals.
///
/// This channel adds no shared numeric constant — the normaliser is computed
/// on the CPU and arrives as one uniform word — so what there is to pin is
/// the *expression*: a `min` over all four slots times the reciprocal at the
/// write, and a clamp to `[0, 1]` at the read. Crude, and it is the failure
/// mode that would otherwise be silent: a shader edited to clamp at the write
/// or to `min` over three slots would leave every test above green, because
/// every test above runs the mirror.
#[test]
fn the_root_channel_maths_agree_between_rust_and_wgsl() {
    for expected in [
        "let d0 = distance(q, step.fixed01.xy);",
        "let d1 = distance(q, step.fixed01.zw);",
        "let d2 = distance(q, step.fixed23.xy);",
        "let d3 = distance(q, step.fixed23.zw);",
        "return min(min(d0, d1), min(d2, d3)) * step.root_recip;",
        // Written after the respawn branch, so a restarted particle reads an
        // exact 0 — the property the test above asserts on bits.
        "particles[i].root = ifs_root_distance(p.xy);",
    ] {
        assert!(
            super::STEP_SHADER.contains(expected),
            "the step shader should carry `{expected}` — the Rust mirror \
             `root_distance` transcribes it, and the two have drifted"
        );
    }
    // The other half of ADR-0088's Notes: normalised at the write, clamped at
    // the READ, so the stored value stays a faithful measurement.
    assert!(
        super::DRAW_SHADER.contains("let root01 = clamp(root, 0.0, 1.0);"),
        "the draw shader must clamp the root channel at the read — clamping at \
         the write would throw away a legitimate measurement"
    );
    // ...and the anchoring, which is the one place this channel departs from
    // every other term on the coordinate. Pinned as source text because the
    // mirror cannot catch it: `root_shift` and `channel_shift` are both
    // one-line multiplies, so a shader "tidied" into the shared helper would
    // leave every mirror-based assertion green while sliding the figure.
    assert!(
        super::DRAW_SHADER.contains("+ draw.ch.z * root01;"),
        "the root channel's palette term must be ANCHORED (`draw.ch.z * root01`), \
         not routed through the centred `channel_shift` — ADR-0088's Anchoring \
         section, and root01 does not span [0, 1]"
    );
    assert!(
        super::DRAW_SHADER.contains("channel_shift(map01, draw.ch.y) + draw.ch.w * root01"),
        "the root channel's HUE term must be anchored too — a centred one would \
         rotate the skeleton itself by -root_hue/2, and unlike the tint route \
         there is no `hue_center` to absorb it"
    );
    assert!(
        !super::DRAW_SHADER.contains("channel_shift(root01"),
        "the root channel must not use `channel_shift` — it is centred on 0.5, \
         which root01 reaches on only one of the five figures"
    );
    // The retirement, asserted on the shader rather than on the roster: a
    // draw path still reading an age-normalized channel would keep working
    // and keep not producing a gradient.
    assert!(
        !super::DRAW_SHADER.contains("age01"),
        "the age colour channel is retired (Plan 0074 Phase 3) — `age` survives \
         only as the emergence ramp's input"
    );
}

/// **The CPU mirror of the shader's hash agrees with the shader's own draw.**
///
/// Not a re-derivation: this asserts the mirror is a *bijection-preserving*
/// transcription by checking the property `seed()` actually depends on —
/// every seeded age lands strictly inside that particle's own lifetime, so
/// nothing respawns on its first step. If `hash_unit` and `mix32` disagreed,
/// the seeded ages would be drawn against a different life and this fails.
#[test]
fn every_seeded_age_sits_inside_its_own_lifetime() {
    for figure in ifs::IfsFigure::ALL {
        let particles = canonical_seed(AttractorFamily::Ifs(figure), 4096);
        let [lo, hi] = super::CHURN_LIFETIME_SPREAD;
        for p in &particles {
            let life = super::churn_lifetime(p.seed);
            assert!(
                life >= super::CHURN_LIFETIME * lo && life <= super::CHURN_LIFETIME * hi,
                "{figure:?}: lifetime {life} outside the spread"
            );
            assert!(
                p.age >= 0.0 && p.age < life,
                "{figure:?}: seeded age {} is not inside a life of {life} — a \
                 particle would respawn on its first step",
                p.age
            );
        }
        // ...and the ages are actually spread, not merely in range: a bulk
        // start is the artifact this seeding exists to avoid.
        let spread = particles
            .iter()
            .map(|p| p.age)
            .fold(f32::NEG_INFINITY, f32::max);
        assert!(
            spread > super::CHURN_LIFETIME * lo * 0.9,
            "{figure:?}: the oldest seeded age is only {spread} — the stagger is \
             not covering a lifetime"
        );
    }
}

/// **The churn never sends a particle anywhere unbounded**, over a 10 000-step
/// CPU reference at every morph and every lever extreme, respawns included
/// (Plan 0073 Phase 3).
///
/// Deliberately CPU-side and deliberately not a render, for ADR-0075's
/// reason: the failure this excludes is a permanently dead particle buffer,
/// and a capture of a preset that only diverges on a loud passage would pass.
/// The respawn is the new thing it has to cover — it teleports a particle,
/// which is the one operation in this family that is not a contraction.
#[test]
fn the_churn_stays_finite_across_ten_thousand_steps() {
    // Enough particles that a range of lifetimes is represented and every
    // one of them recycles many times over 10 000 steps.
    const PARTICLES: usize = 64;
    const STEPS: u32 = 10_000;
    // The largest curated figure spans ~14 world units, so anything past
    // this is a divergence in progress rather than a big fern — the bound
    // `no_reachable_morph_diverges` already uses.
    const BOUND: f32 = 1_000.0;

    let [low_levers, high_levers] = Levers::EXTREMES;
    let mut respawns = 0u64;
    for figure in ifs::IfsFigure::ALL {
        for target in ifs::IfsFigure::ALL {
            for morph in [0.0, 0.35, 0.5, 1.0] {
                for levers in [Levers::NEUTRAL, low_levers, high_levers] {
                    let table = ChurnTable::resolve(&ifs::resolve(
                        &figure.table(),
                        &target.table(),
                        morph,
                        levers,
                    ));
                    let seeded = canonical_seed(AttractorFamily::Ifs(figure), PARTICLES as u32);
                    let mut state: Vec<ChurnState> = seeded
                        .iter()
                        .map(|p| {
                            let [x, y, _] = p.pos;
                            ChurnState {
                                at: [x, y],
                                age: p.age,
                                seed: p.seed,
                            }
                        })
                        .collect();
                    for step_index in 0..STEPS {
                        for particle in state.iter_mut() {
                            if particle.step(step_index, &table) {
                                respawns += 1;
                            }
                            let [x, y] = particle.at;
                            assert!(
                                x.is_finite()
                                    && y.is_finite()
                                    && x.abs() < BOUND
                                    && y.abs() < BOUND,
                                "{figure:?} -> {target:?} at morph {morph}, levers \
                                 {levers:?}, step {step_index}: position [{x}, {y}] is \
                                 not inside +/-{BOUND}"
                            );
                        }
                    }
                }
            }
        }
    }
    // The respawn path was actually exercised — without this the test above
    // would pass just as well on a build where the churn never fired.
    assert!(
        respawns > 1_000_000,
        "only {respawns} respawns over the sweep — the churn is not running"
    );
}

/// **Every colour channel defaults to the identity** (Plan 0073 Phase 4,
/// extended by Plan 0074 Phase 1).
///
/// "The default is the identity" is a claim about arithmetic rather than a
/// hope, and it is what the unmoved golden baselines rest on: `*_tint`
/// adds an exact `0` to the palette coordinate and `*_hue` compares equal to
/// literal `0.0` and takes `shift_hue`'s early return. Asserted here on the
/// scene's own state, and by the golden suite on the pixels.
///
/// Counted by the list rather than by the name, because Plan 0074 makes the
/// roster five and then four again.
#[test]
fn the_colour_channels_default_to_the_identity() {
    use projection_mirror as m;

    let names = ["map_tint", "map_hue", "root_tint", "root_hue"];
    for name in names {
        assert!(
            super::PARAMS.contains(&name),
            "`{name}` is missing from PARAMS, so a preset binding it warns instead \
             of working"
        );
    }

    let Some(mut h) = Harness::new(AttractorFamily::Ifs(IfsFigure::Fern)) else {
        return;
    };
    // Bound away from the default, then reset — `reset_params` is what runs
    // between presets, so a channel it forgot would leak the last preset's.
    for name in names {
        h.scene.set_param(name, 0.9);
    }
    h.scene.reset_params();
    for (name, got) in names.into_iter().zip([
        h.scene.map_tint,
        h.scene.map_hue,
        h.scene.root_tint,
        h.scene.root_hue,
    ]) {
        assert_eq!(got, 0.0, "`{name}` did not reset to the identity");
    }

    // ...and zero really is the identity on every route, at any channel
    // value — including the anchored one, where it is `0.0 * unit` rather
    // than `0.0 * (unit - 0.5)` and just as exact.
    for unit in [0.0f32, 0.33, 0.5, 1.0] {
        assert_eq!(m::channel_shift(unit, 0.0), 0.0);
        assert_eq!(m::root_shift(unit, 0.0), 0.0);
    }

    // The anchored route is inert on a family whose channel is identically
    // zero **by arithmetic**, whatever the binding — which the centred route
    // is not, and which is why the engine has to zero the `ch` row off the
    // IFS rather than relying on a default (ADR-0088's Anchoring section).
    for amount in [0.0f32, 0.3, 0.9, -0.4] {
        assert_eq!(m::root_shift(0.0, amount), 0.0);
    }
    assert_eq!(m::channel_shift(0.0, 0.4), -0.2);
    let colour = [0.2f32, 0.65, 0.35];
    assert_eq!(
        m::shift_hue(colour, 0.0).map(f32::to_bits),
        colour.map(f32::to_bits)
    );
}

/// **The two routes are genuinely two routes** (Plan 0073 Phase 4), asserted
/// on the transcribed CPU maths rather than on pixels — which is the only
/// place the claim is even expressible, since a capture cannot separate
/// "sampled the ramp elsewhere" from "rotated the colour the ramp produced".
///
/// `*_tint` moves the palette **coordinate**, which is the number handed to
/// the LUT; `*_hue` leaves that coordinate untouched and rotates the hue of
/// the colour that came back. So one rides the preset's own gradient — a
/// custom ramp, `palette_mix` and `saturation` all reach it for free — and the
/// other nudges a part of the figure off that gradient without editing it.
#[test]
fn the_hue_route_moves_hue_and_leaves_the_palette_coordinate_alone() {
    use projection_mirror as m;

    // A saturated mid-tone: hue is well defined, so a rotation is measurable.
    let base = [0.20f32, 0.65, 0.35];

    // The hue route is a rotation about the wheel: hue moves by the shift,
    // and saturation and value are carried through.
    let [h0, s0, v0] = m::rgb2hsv(base);
    for turns in [0.05f32, 0.25, -0.30, 0.75] {
        let [h1, s1, v1] = m::rgb2hsv(m::shift_hue(base, turns));
        let moved = (h1 - (h0 + turns).rem_euclid(1.0)).abs();
        assert!(
            moved < 1e-3 || (moved - 1.0).abs() < 1e-3,
            "a shift of {turns} took hue {h0} to {h1}, not to {}",
            (h0 + turns).rem_euclid(1.0)
        );
        assert!(
            (s1 - s0).abs() < 1e-3 && (v1 - v0).abs() < 1e-3,
            "the hue route moved saturation {s0} -> {s1} or value {v0} -> {v1}"
        );
    }

    // ...and it is EXACTLY the identity at zero, which is what sixteen
    // baselines rest on. Compared as bits: the round trip is not bit-exact,
    // so "close enough" would not be the same claim.
    assert_eq!(
        m::shift_hue(base, 0.0).map(f32::to_bits),
        base.map(f32::to_bits)
    );

    // The tint route is the other one: it moves the coordinate, and it is an
    // exact zero at its own default whatever the channel reads.
    for unit in [0.0f32, 0.25, 0.5, 1.0] {
        assert_eq!(m::channel_shift(unit, 0.0).abs(), 0.0);
    }
    assert!(m::channel_shift(1.0, 0.4) > 0.0 && m::channel_shift(0.0, 0.4) < 0.0);
    // Centred, so the mid-channel colour is the one the preset asked for and
    // raising the amount opens a spread rather than sliding the figure.
    assert_eq!(m::channel_shift(0.5, 0.4), 0.0);
}

/// **`root_hue` moves hue and leaves the palette coordinate alone** (Plan
/// 0074 Phase 3), asserted on the transcribed CPU maths rather than on
/// pixels — the only place the claim is expressible, since a capture cannot
/// separate "sampled the ramp elsewhere" from "rotated the colour the ramp
/// returned".
///
/// The two routes share `root01` and diverge in what they do with it: the
/// tint route adds `root_shift(root01, root_tint)` to the coordinate handed
/// to the LUT, and the hue route leaves that coordinate untouched and rotates
/// the colour that came back. That separation is what makes `root_hue` the
/// escape when the palette coordinate is already fully spent — which Plan
/// 0074's Phase 2 gate measured it to be on `attractor_fern`.
#[test]
fn the_root_hue_route_rotates_without_touching_the_palette_coordinate() {
    use projection_mirror as m;

    let base = [0.20f32, 0.65, 0.35];
    let [h0, s0, v0] = m::rgb2hsv(base);

    for unit in [0.0f32, 0.25, 0.46, 1.0] {
        for amount in [0.10f32, 0.35, -0.25] {
            let turns = m::root_shift(unit, amount);

            // The hue route rotates by exactly the anchored shift, carrying
            // saturation and value through untouched.
            let [h1, s1, v1] = m::rgb2hsv(m::shift_hue(base, turns));
            let moved = (h1 - (h0 + turns).rem_euclid(1.0)).abs();
            assert!(
                moved < 1e-3 || (moved - 1.0).abs() < 1e-3,
                "root01 {unit} at root_hue {amount} took hue {h0} to {h1}, not to {}",
                (h0 + turns).rem_euclid(1.0)
            );
            assert!(
                (s1 - s0).abs() < 1e-3 && (v1 - v0).abs() < 1e-3,
                "the hue route moved saturation {s0} -> {s1} or value {v0} -> {v1}"
            );
        }
    }

    // ...and it is ANCHORED, like the tint route: a particle ON a fixed
    // point takes no rotation at all, so the skeleton keeps exactly the
    // colour the ramp gave it. A centred route would rotate it by
    // `-root_hue/2`, and unlike the tint route there is no `hue_center` to
    // absorb that.
    for amount in [0.0f32, 0.2, 0.9, -0.4] {
        assert_eq!(m::root_shift(0.0, amount), 0.0);
        assert_eq!(
            m::shift_hue(base, m::root_shift(0.0, amount)).map(f32::to_bits),
            base.map(f32::to_bits),
            "a particle on a fixed point must take root_hue {amount} as the identity"
        );
    }

    // The two routes are genuinely separate: the same channel value feeds
    // both, and binding one leaves the other's input alone. Stated as the
    // arithmetic rather than as a picture, because a capture cannot tell the
    // two apart.
    let unit = 0.4f32;
    assert_eq!(
        m::root_shift(unit, 0.0),
        0.0,
        "root_hue bound alone must not move the coordinate"
    );
    assert!(m::root_shift(unit, 0.5) > 0.0);
}

/// **`emergence` at its default is bit-identical to the constant it
/// replaced** (Plan 0074 Phase 4).
///
/// This is what every baseline rests on. The param arrived by making a
/// constant bindable, so "the default changes nothing" is not a tolerance
/// claim — the packed rate must be the *same bits* `1.0 / EMERGENCE_STEPS`
/// produced, or all nineteen goldens move for a change that was supposed to
/// be inert.
#[test]
fn the_emergence_default_reproduces_the_constant_exactly() {
    assert_eq!(super::DEFAULT_EMERGENCE, 8.0);
    assert_eq!(
        super::emergence_rate(super::DEFAULT_EMERGENCE).to_bits(),
        (1.0f32 / 8.0).to_bits(),
        "the default rate must be the exact bits the fixed constant produced"
    );

    // ...and the scene actually starts and resets there, which is the other
    // half: a correct rate reached from a wrong default is still a moved
    // baseline.
    let Some(mut h) = Harness::new(AttractorFamily::Ifs(IfsFigure::Fern)) else {
        return;
    };
    assert_eq!(h.scene.emergence, super::DEFAULT_EMERGENCE);
    h.scene.set_param("emergence", 40.0);
    h.scene.reset_params();
    assert_eq!(
        h.scene.emergence,
        super::DEFAULT_EMERGENCE,
        "`emergence` did not reset — a preset that bound it would leak into the next"
    );
}

/// **A binding the ramp's maths cannot accept is clamped at the pack site**
/// (Plan 0074 Phase 4), not passed through to a division.
///
/// `em.x` is `1 / emergence`. Zero divides; a negative *inverts* the ramp, so
/// a just-respawned particle would start bright and dim — the four-blob
/// artifact the ramp exists to remove, arriving through the front door. And
/// non-finite is the case a bare `clamp` cannot fix, because `f32::clamp`
/// propagates `NaN` straight into the division.
///
/// **A smoothing curve is what makes this necessary rather than defensive.**
/// An eased param is continuous even when its own maths is not, so a binding
/// easing from `8` toward `0` sweeps *through* the invalid range whatever its
/// endpoints are — this repo has been bitten by exactly that shape before.
#[test]
fn an_unusable_emergence_binding_clamps_instead_of_dividing() {
    let floor_rate = 1.0f32 / super::MIN_EMERGENCE;

    for bad in [0.0f32, -1.0, -1e9, 0.5, f32::MIN_POSITIVE] {
        let rate = super::emergence_rate(bad);
        assert!(
            rate.is_finite(),
            "emergence {bad} produced a non-finite rate {rate}"
        );
        assert!(
            rate > 0.0,
            "emergence {bad} produced rate {rate} — a non-positive rate inverts \
             the ramp, which is the artifact it exists to remove"
        );
        assert_eq!(
            rate.to_bits(),
            floor_rate.to_bits(),
            "emergence {bad} should clamp to the {} floor",
            super::MIN_EMERGENCE
        );
    }

    // Non-finite falls back to the default rather than to the floor: a `NaN`
    // is not a small number, it is an absent one.
    for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let rate = super::emergence_rate(bad);
        assert_eq!(
            rate.to_bits(),
            (1.0f32 / super::DEFAULT_EMERGENCE).to_bits(),
            "a non-finite emergence must fall back to the default, got {rate}"
        );
    }

    // The range is pinned, not just its edges: inside it the param is the
    // reciprocal it claims to be, and it is monotone — a longer ramp is a
    // smaller per-step increment.
    let mut previous = f32::INFINITY;
    for steps in [1.0f32, 2.0, 8.0, 60.0, 270.0, 1e6] {
        let rate = super::emergence_rate(steps);
        assert_eq!(
            rate.to_bits(),
            (1.0f32 / steps).to_bits(),
            "at {steps} steps"
        );
        assert!(rate < previous, "the rate must fall as the ramp lengthens");
        previous = rate;
    }
}

/// **The retired names no longer resolve** (Plan 0074 Phase 3).
///
/// `set_param` silently ignores an unknown name — that is how a preset
/// binding a typo warns at load rather than failing — so "it no longer
/// resolves" has to be asserted as *the field it used to write does not
/// move*, which here means the whole colour row stays at its default.
#[test]
fn the_age_colour_channel_is_gone_from_the_roster() {
    for retired in ["age_tint", "age_hue"] {
        assert!(
            !super::PARAMS.contains(&retired),
            "`{retired}` is still in PARAMS — it was retired at Plan 0074 Phase 3 \
             because `age` proxied distance-from-the-fixed-points and the proxy decayed"
        );
    }
    for live in ["map_tint", "map_hue", "root_tint", "root_hue"] {
        assert!(
            super::PARAMS.contains(&live),
            "`{live}` is missing from PARAMS, so a preset binding it warns instead \
             of working"
        );
    }

    let Some(mut h) = Harness::new(AttractorFamily::Ifs(IfsFigure::Fern)) else {
        return;
    };
    // Binding a retired name must be inert, not merely absent from the list.
    for retired in ["age_tint", "age_hue"] {
        h.scene.set_param(retired, 0.9);
    }
    for (name, got) in ["map_tint", "map_hue", "root_tint", "root_hue"]
        .into_iter()
        .zip([
            h.scene.map_tint,
            h.scene.map_hue,
            h.scene.root_tint,
            h.scene.root_hue,
        ])
    {
        assert_eq!(
            got, 0.0,
            "setting a retired age param moved `{name}` — the name was re-pointed \
             rather than removed"
        );
    }
}

/// **The CPU mirror agrees with itself**, so the assertions above are about
/// the transcription rather than about one direction of it.
///
/// The round trip is not bit-exact — which is exactly why `shift_hue` early-
/// returns on a literal zero rather than trusting it.
#[test]
fn the_hsv_mirror_round_trips() {
    use projection_mirror as m;

    for c in [
        [0.20f32, 0.65, 0.35],
        [0.90, 0.10, 0.05],
        [0.05, 0.05, 0.90],
        [0.50, 0.50, 0.50],
        [0.00, 0.00, 0.00],
    ] {
        let back = m::hsv2rgb(m::rgb2hsv(c));
        for (got, want) in back.iter().zip(c.iter()) {
            assert!(
                (got - want).abs() < 1e-4,
                "round trip of {c:?} gave {back:?}"
            );
        }
    }
}

/// **The respawn is a pure function of the particle's fixed seed and the step
/// index** (Plan 0073 Phase 3), asserted on the buffer rather than on a
/// picture.
///
/// Two runs of the same figure at the same injected `dt` sequence must
/// produce **bit-identical** particle buffers — positions, ages and map
/// indices alike. A respawn that reached for a clock, an unseeded draw or a
/// frame counter would diverge here and nowhere else: the picture would look
/// exactly as alive either way.
#[test]
fn two_runs_of_the_churn_produce_identical_buffers() {
    const FRAMES: u32 = 240;

    let Some(mut first) = Harness::new(AttractorFamily::Ifs(IfsFigure::Fern)) else {
        return;
    };
    first.run(FRAMES);
    let a = first.raw();

    let Some(mut second) = Harness::new(AttractorFamily::Ifs(IfsFigure::Fern)) else {
        return;
    };
    second.run(FRAMES);
    let b = second.raw();

    assert_eq!(a.len(), b.len());
    // Compared as bits: two buffers agreeing to within a tolerance is not the
    // claim — determinism is exact or it is not determinism.
    let differing = a
        .iter()
        .zip(b.iter())
        .filter(|(x, y)| {
            x.pos.map(f32::to_bits) != y.pos.map(f32::to_bits)
                || x.age.to_bits() != y.age.to_bits()
                || x.map.to_bits() != y.map.to_bits()
        })
        .count();
    assert_eq!(
        differing,
        0,
        "{differing} of {} particles differ between two identical runs — the churn \
         is reading something that is not the seed and the step index",
        a.len()
    );

    // ...and the run actually churned, or the assertion above is about a
    // buffer that never moved. 240 frames is past the shortest lifetime.
    assert!(
        a.iter().any(|p| p.age < super::CHURN_LIFETIME * 0.1),
        "no particle is freshly respawned after {FRAMES} frames — the churn did \
         not run, so the determinism claim is vacuous"
    );
}

/// **The property the phase exists for, asserted as a distribution rather
/// than as a picture** (Plan 0073 Phase 3).
///
/// After 600 steps the population must still hold *every* age. That is what
/// makes Phase 4's colour gradient permanent rather than a one-second startup
/// animation — and it is the assertion a bulk respawn would fail while
/// passing every other check here, because a synchronized population is
/// deterministic, finite and continuously churning too.
///
/// Measured on each particle's age as a fraction of **its own** lifetime,
/// which is the quantity the colour channel reads: lifetimes differ per
/// particle, so raw ages would blur the deciles rather than test them.
#[test]
fn after_six_hundred_steps_the_population_holds_every_age() {
    const FRAMES: u32 = 600;
    const DECILES: usize = 10;

    let Some(mut h) = Harness::new(AttractorFamily::Ifs(IfsFigure::Fern)) else {
        return;
    };
    h.run(FRAMES);
    let particles = h.raw();

    let mut deciles = [0usize; DECILES];
    let (mut lowest, mut highest) = (f32::INFINITY, f32::NEG_INFINITY);
    for p in &particles {
        let life = super::churn_lifetime(p.seed);
        let fraction = p.age / life;
        assert!(
            (0.0..1.0).contains(&fraction),
            "age {} is outside a lifetime of {life}",
            p.age
        );
        lowest = lowest.min(fraction);
        highest = highest.max(fraction);
        let bucket = ((fraction * DECILES as f32) as usize).min(DECILES - 1);
        if let Some(slot) = deciles.get_mut(bucket) {
            *slot += 1;
        }
    }

    assert!(
        highest - lowest >= 0.9,
        "the ages span only {:.3} of a lifetime ({lowest:.3}..{highest:.3}) — the \
         population has synchronized",
        highest - lowest
    );
    for (i, count) in deciles.iter().enumerate() {
        assert!(
            *count > 0,
            "decile {i} of the age range is empty after {FRAMES} steps: {deciles:?}"
        );
    }
}

/// **The root channel's population is spread rather than clustered, on every
/// figure** (Plan 0074 Phase 1, claim 3, as restated 2026-08-07).
///
/// **This is the cheap early warning for the exact failure Plan 0073 hit.** A
/// channel whose population clusters cannot show a gradient however it is
/// coloured, and this catches that on the readback buffer before a human
/// looks at a picture. It does **not** establish that the gradient is
/// *visible* — that is the Phase 2 gate's job, and no readback can do it.
///
/// **What it asserts, and what it deliberately only prints.** The claim
/// originally read "spans at least 90 % of `[0, 1]`"; that is false on four
/// of the five figures, because the fixed-point diameter is not a *lower*
/// bound on the attractor's reach any more than it is an upper one. The
/// **ceiling is therefore measured and printed, never asserted** — it is a
/// property of each figure's invariant measure rather than of this code, and
/// a threshold on it would be a frozen number asserted universally, the
/// shape [ADR-0071] forbids. What *is* asserted is the property the claim was
/// reaching for and which holds on all five: the values reach an exact `0`
/// and rise with **no gap wider than a decile of the occupied bulk**.
///
/// The bulk is `[0, p99]` rather than `[0, max]` on purpose. The spiral's top
/// few particles are a handful out of 4 096, so a statistic anchored on the
/// single farthest one would be measuring that particle rather than the
/// distribution — and would flake.
///
/// The printed table is what the Phase 2 gate reads: it is the only place the
/// per-figure ceilings are recorded from a live run.
///
/// [ADR-0071]: ../../../../docs/adrs/0071-a-numeric-test-contract-states-a-property-or-names-its-machine.md
#[test]
fn after_six_hundred_steps_the_root_channel_is_spread_on_every_figure() {
    const FRAMES: u32 = 600;
    const DECILES: usize = 10;

    for figure in ifs::IfsFigure::ALL {
        let Some(mut h) = Harness::new(AttractorFamily::Ifs(figure)) else {
            return;
        };
        h.run(FRAMES);

        let mut roots: Vec<f32> = h
            .raw()
            .iter()
            .map(|p| {
                assert!(
                    p.root.is_finite() && p.root >= 0.0,
                    "{figure:?}: root {} is not a finite distance — the normaliser \
                     diverged",
                    p.root
                );
                p.root
            })
            .collect();
        roots.sort_by(f32::total_cmp);

        // The respawn state, on bits. A particle that has just restarted sits
        // *on* a fixed point, so its distance is not approximately zero — it
        // is zero, and that is one end of this channel's ramp. Non-vacuous
        // too: it is the evidence the churn ran at all during the readback
        // frame.
        let at_zero = roots
            .iter()
            .filter(|r| r.to_bits() == 0.0f32.to_bits())
            .count();

        let ceiling = roots.last().copied().unwrap_or(0.0);
        // The 99th percentile of ~4 096 samples — the 41st value from the
        // top, which is stable in a way the maximum is not.
        let p99 = roots
            .get(roots.len().saturating_sub(1) * 99 / 100)
            .copied()
            .unwrap_or(0.0);

        // Printed, not asserted (see the doc comment): this is the Phase 2
        // gate's input.
        let mut deciles = [0usize; DECILES];
        for r in &roots {
            let bucket = ((r.clamp(0.0, 1.0) * DECILES as f32) as usize).min(DECILES - 1);
            if let Some(slot) = deciles.get_mut(bucket) {
                *slot += 1;
            }
        }
        println!(
            "{figure:?}: root 0.000..{ceiling:.3} (p99 {p99:.3}), {at_zero} at exact 0, \
             deciles of [0, 1] {deciles:?}"
        );

        assert!(
            at_zero > 0,
            "{figure:?}: no particle reads an exact 0 after {FRAMES} steps — either \
             nothing respawned during the readback frame, or the distance is not \
             exact at a fixed point"
        );

        // The property itself: no hole wider than a decile of the bulk. This
        // subsumes a decile-occupancy check and states the claim directly —
        // a clustered population is exactly one with a large interior gap.
        let widest = roots
            .iter()
            .copied()
            .filter(|r| *r <= p99)
            .collect::<Vec<_>>()
            .windows(2)
            .filter_map(|w| match w {
                [a, b] => Some(b - a),
                _ => None,
            })
            .fold(0.0f32, f32::max);
        let limit = p99 / DECILES as f32;
        assert!(
            widest <= limit,
            "{figure:?}: the root values have a {widest:.4} hole inside [0, {p99:.3}], \
             wider than the {limit:.4} decile of that bulk — the population is \
             clustered, and a clustered channel cannot show a gradient whatever \
             colours it"
        );
    }
}

/// ADR-0087's `map` channel is written by the IFS arm and by nothing else.
///
/// **Both halves are the test.** That the fern reaches all four values says
/// the channel is a live partition of the figure rather than a constant that
/// happens to render; that De Jong stays at `0.0` says the write is confined
/// to the arm that owns it, which is what makes `map_tint`/`map_hue` exactly
/// inert on the four map families without a branch anywhere.
///
/// One step is enough on purpose: `map` names the map applied on the *most
/// recent* step, so its distribution is a property of a single step and not
/// of a converged run.
#[test]
fn the_ifs_writes_every_map_index_and_no_other_family_writes_any() {
    let Some(mut fern) = Harness::new(AttractorFamily::Ifs(IfsFigure::Fern)) else {
        return;
    };
    fern.run(1);
    let seen: std::collections::BTreeSet<u32> = fern
        .raw()
        .iter()
        .map(|p| {
            assert!(
                p.map >= 0.0 && p.map <= (ifs::MAPS - 1) as f32 && p.map.fract() == 0.0,
                "map must be a whole index in 0..{}, got {}",
                ifs::MAPS,
                p.map
            );
            p.map as u32
        })
        .collect();
    assert_eq!(
        seen,
        (0..ifs::MAPS as u32).collect(),
        "after one step the fern's population should sit in all {} sub-copies",
        ifs::MAPS
    );

    let Some(mut de_jong) = Harness::new(AttractorFamily::DeJong) else {
        return;
    };
    de_jong.run(4);
    assert!(
        de_jong.raw().iter().all(|p| p.map == 0.0),
        "a map family must leave `map` at its seeded 0.0 — anything else \
         makes `map_tint` reach a family it has no meaning on"
    );
    // ...and ADR-0088's channel is confined to the same arm, for the same
    // reason: a map family has no closed-form on-attractor point, so it has
    // no skeleton to measure a distance from.
    assert!(
        de_jong.raw().iter().all(|p| p.root == 0.0),
        "a map family must leave `root` at its seeded 0.0 — it has no fixed \
         points, so any value there is measured against a zeroed table"
    );
}

// -----------------------------------------------------------------------
// The tuple roster (Plan 0079 Phase 1 / ADR-0093)
// -----------------------------------------------------------------------

/// Lorenz's provisional second entry — the rho ~ 100 torus knot, the regime
/// Plan 0075 cohort 5 measured as unreachable.
const KNOT: usize = 1;

/// **Entry 0 is today**, on every family — the claim the whole "no golden
/// baseline moves" argument rests on, stated where it can fail loudly rather
/// than inferred from a green capture suite.
///
/// Two halves, and the second is the one that could silently rot: the entry's
/// coefficients must be the family's canonical ones, and its framing must be
/// the literal constants the scene shipped with. A roster whose entry 0 was
/// *measured* would land within a percent of these and would move every
/// attractor baseline by a pixel.
#[test]
fn roster_entry_zero_is_the_canonical_tuple_unchanged() {
    for family in [
        AttractorFamily::DeJong,
        AttractorFamily::Clifford,
        AttractorFamily::Thomas,
        AttractorFamily::Lorenz,
        AttractorFamily::Ifs(IfsFigure::Fern),
    ] {
        let roster = family::resolve_roster(family);
        let first = roster
            .first()
            .map(|e| e.tuple)
            .expect("a roster is never empty");
        assert_eq!(
            first.coeffs,
            family.default_coeffs(),
            "{family:?} entry 0 must carry the canonical coefficients"
        );
        assert_eq!(
            first.framing,
            canonical(family),
            "{family:?} entry 0 must carry the framing this scene shipped with"
        );
    }

    // The literals themselves, spelled out once: these are the numbers the
    // pre-roster `projection()` / `seed_box()` returned, and a refactor that
    // "tidied" one of them would otherwise only show up as a golden diff.
    assert_eq!(
        canonical(AttractorFamily::Lorenz),
        Framing {
            projection: (0.022, 3.0, [0.0, 0.0, 25.0]),
            seed_box: ([20.0, 26.0, 24.0], [0.0, 0.0, 25.0]),
        }
    );
    assert_eq!(
        canonical(AttractorFamily::DeJong),
        Framing {
            projection: (0.42, 2.0, [0.0, 0.0, 0.0]),
            seed_box: ([1.5, 1.5, 1.5], [0.0, 0.0, 0.0]),
        }
    );
}

/// A scene that binds nothing sits on entry 0, and its coefficients are the
/// ones `reset_params` hands out — the scene-level half of the claim above.
#[test]
fn an_unbound_tuple_selects_the_canonical_entry() {
    let Some(mut h) = Harness::new(AttractorFamily::Lorenz) else {
        return;
    };
    h.scene.reset_params();
    h.scene.update(&AnalysisFrame::default());
    assert_eq!(h.scene.entry().framing, canonical(AttractorFamily::Lorenz));
    assert_eq!(
        [h.scene.a, h.scene.b, h.scene.c, h.scene.d],
        AttractorFamily::Lorenz.default_coeffs(),
        "an unbound preset must still get the canonical coefficients"
    );

    // ...and the selector reaches the other entry, so the equality above is a
    // choice rather than the only thing the roster can express.
    h.scene.set_param("tuple", 1.0);
    h.scene.update(&AnalysisFrame::default());
    assert_ne!(
        h.scene.entry().framing,
        canonical(AttractorFamily::Lorenz),
        "entry 1 must carry framing of its own, or the roster buys nothing"
    );
}

/// **A fractional `tuple` never reaches a fractional figure** (ADR-0093).
///
/// The hazard is not a bad index — the index is a `usize` and cannot be
/// fractional. It is that a *smoothing curve* makes intermediate values
/// unavoidable: an eased binding from `0` toward `1` passes through `0.4`
/// whatever its endpoints are, and interpolating coefficients there would draw
/// a third, unmeasured attractor with neither endpoint's framing. So the
/// assertion is on the resolved *coefficients*: every value along a sweep must
/// land bit-exactly on some roster entry.
#[test]
fn an_eased_tuple_never_lands_between_two_figures() {
    let roster = family::resolve_roster(AttractorFamily::Lorenz);
    assert!(
        roster.len() > 1,
        "the sweep needs two entries to have anything to land between"
    );
    for step in 0..=40 {
        let raw = step as f32 / 20.0 - 0.5;
        let index = family::roster_index(raw, roster.len());
        let landed = roster
            .get(index)
            .map(|e| e.tuple)
            .expect("clamped into the roster");
        assert!(
            roster.iter().any(|e| e.tuple.coeffs == landed.coeffs),
            "tuple = {raw} resolved to coefficients {:?}, which are no entry's",
            landed.coeffs
        );
    }

    // Nearest-integer, not truncation: an eased sweep lands on the entry it is
    // closest to rather than lagging a whole figure behind.
    assert_eq!(family::roster_index(0.4, 2), 0);
    assert_eq!(family::roster_index(0.6, 2), 1);
    // Clamped into the roster at both ends, so an over-driven binding holds the
    // last figure rather than selecting nothing...
    assert_eq!(family::roster_index(-7.0, 2), 0);
    assert_eq!(family::roster_index(1e9, 2), 1);
    // ...and a non-finite one falls back to the canonical entry rather than
    // saturating — `f32::clamp` propagates `NaN`, and `kaleido_edge` /
    // `kaleido_spiral` both answer their default rather than a bound here.
    // An infinity is *not* the last entry: a binding that has blown up is a
    // broken binding, and holding the figure it started on is the readable
    // failure.
    assert_eq!(family::roster_index(f32::NAN, 2), 0);
    assert_eq!(family::roster_index(f32::INFINITY, 2), 0);
    assert_eq!(family::roster_index(f32::NEG_INFINITY, 2), 0);
    // A one-entry roster (every family but Lorenz today, and every IFS figure)
    // answers 0 to everything rather than indexing past its end.
    assert_eq!(family::roster_index(3.0, 1), 0);
}

/// **The wall and the fix, on one instrument** (backlog 0055, ADR-0093).
///
/// The rho ~ 100 Lorenz is unreachable *not* because its coefficients cannot be
/// bound — they can — but because the canonical framing puts it off-centre and
/// out of frame: it is centred on `z ~ 102` where the canonical projection
/// subtracts `25`, and it spans roughly twice the canonical extent. So this
/// measures the figure once and projects it through **both** framings: through
/// the canonical one it lands outside the frame (the wall), and through its own
/// it is centred and inside (the fix).
#[test]
fn a_measured_entry_frames_a_tuple_the_canonical_framing_cannot() {
    const FAMILY: AttractorFamily = AttractorFamily::Lorenz;
    let roster = family::resolve_roster(FAMILY);
    let knot = roster
        .get(KNOT)
        .map(|e| e.tuple)
        .expect("the provisional entry ships");
    let knot_figure = family::measure_figure(FAMILY, knot.coeffs).expect("rho ~ 100 is bounded");
    let measured = &knot_figure.extent;

    // The premise: this really is a figure the canonical framing was not sized
    // for. Both halves matter — a bigger figure alone could be zoomed and an
    // off-centre one alone could be panned; it is the pair that neither rescues.
    let [_, _, cz] = measured.centre;
    let (_, _, [_, _, canonical_cz]) = canonical(FAMILY).projection;
    assert!(
        (cz - canonical_cz).abs() > 50.0,
        "the knot is centred on z = {cz}, only {} from the canonical {canonical_cz} — \
         the premise this entry exists to demonstrate is gone",
        (cz - canonical_cz).abs()
    );
    let canonical_span = family::framed_half(FAMILY, canonical(FAMILY).seed_box.0);
    assert!(
        family::framed_half(FAMILY, measured.half) > 1.5 * canonical_span,
        "the knot no longer overruns the canonical extent"
    );

    // The wall: the figure's farthest point, projected through the CANONICAL
    // framing, sits outside NDC. `1.0` is the frame edge — the vertical axis is
    // not divided by the aspect, so this is the honest bound on either.
    let wall = {
        let (scale, _, [_, _, ctr_z]) = canonical(FAMILY).projection;
        let [_, _, hz] = measured.half;
        (hz + (cz - ctr_z).abs()) * scale
    };
    assert!(
        wall > 1.0,
        "the knot fits the canonical frame after all (reach {wall:.2}) — then \
         ADR-0093's framing argument is not what this entry demonstrates"
    );

    // The fix: through its own framing it pivots on its own figure, and every
    // point of it is inside the frame.
    let (scale, _, centre) = knot.framing.projection;
    assert_eq!(
        centre, measured.centre,
        "a measured entry must pivot on its own figure"
    );
    let reach = family::framed_half(FAMILY, measured.half) * scale;
    println!("the knot fills {reach:.3} of the frame");
    assert!(
        (0.2..=1.0).contains(&reach),
        "the knot projects to {reach:.2} of the frame — out of frame above 1.0, \
         and a dot below 0.2"
    );

    // ...and at the same on-screen size as the family's canonical figure, which
    // is what `measured_framing` scales by rather than by an invented fill
    // constant.
    let butterfly =
        family::measure_figure(FAMILY, FAMILY.default_coeffs()).expect("the butterfly is bounded");
    let canonical_reach =
        family::framed_half(FAMILY, butterfly.extent.half) * canonical(FAMILY).projection.0;
    let ratio = reach / canonical_reach;
    assert!(
        (0.99..=1.01).contains(&ratio),
        "a measured entry should occupy the same footprint as its family's \
         canonical figure; this one is {ratio:.3}x"
    );

    // **The fill starts on the figure** (ADR-0093, the ADR-0087 argument): the
    // bank the measurement collected is what a measured entry seeds from, and
    // every point of it is inside the figure's own box by construction. Seeded
    // from a uniform fill of that box instead, this tuple wanders out to 2.2x
    // its own extent for its first several seconds.
    let banked = family::resolve_roster(FAMILY)
        .into_iter()
        .nth(KNOT)
        .map(|entry| entry.fill)
        .expect("the provisional entry ships");
    assert!(
        banked.len() > 1000,
        "the knot banked only {} fill points — too few to cover the figure",
        banked.len()
    );
    let ([hx, hy, hz], [bcx, bcy, bcz]) = knot.framing.seed_box;
    for [x, y, z] in &banked {
        assert!(
            (x - bcx).abs() <= hx && (y - bcy).abs() <= hy && (z - bcz).abs() <= hz,
            "banked fill point {:?} is outside the figure it was measured from",
            [x, y, z]
        );
    }
}

/// A diverging tuple keeps its slot rather than renumbering the roster, and
/// cannot send an infinity to the GPU.
///
/// Euler is conditionally stable and this scene integrates at a fixed sub-step,
/// so rho ~ 160 genuinely blows up — the guard is the difference between a
/// fallback and a `NaN` scale.
#[test]
fn a_divergent_tuple_falls_back_instead_of_reaching_the_gpu() {
    const FAMILY: AttractorFamily = AttractorFamily::Lorenz;
    let wild = [10.0, 160.0, 2.6667, 0.0];
    assert!(
        family::measure_figure(FAMILY, wild).is_none(),
        "rho = 160 no longer diverges at this sub-step — pick another witness"
    );
    // Whatever reference it is handed, a figure that cannot be measured cannot
    // be framed.
    let degenerate = family::Extent {
        half: [0.0; 3],
        centre: [0.0; 3],
    };
    assert!(family::measured_framing(FAMILY, &degenerate, 30.0).is_none());

    // Every shipped entry's framing is finite and usable, which is what the
    // fallback guarantees for a curated roster that later grows a bad tuple.
    for family in [
        AttractorFamily::DeJong,
        AttractorFamily::Clifford,
        AttractorFamily::Thomas,
        AttractorFamily::Lorenz,
    ] {
        for entry in family::resolve_roster(family) {
            let (scale, dim, centre) = entry.tuple.framing.projection;
            let (half, _) = entry.tuple.framing.seed_box;
            assert!(
                scale.is_finite() && scale > 0.0,
                "{family:?} entry scale {scale} is unusable"
            );
            assert_eq!(dim, canonical(family).projection.1, "{family:?} dim moved");
            assert!(
                centre.iter().all(|v| v.is_finite()) && half.iter().all(|v| v.is_finite()),
                "{family:?} entry framing is not finite"
            );
            assert!(
                entry.tuple.framing.inv_depth_extent(family).is_finite(),
                "{family:?} entry would send a non-finite depth normalizer to the GPU"
            );
        }
    }
}

/// **Every roster entry is in frame** — Plan 0079 Phase 2's done-when, asserted
/// on the geometry rather than by eye on the contact sheets.
///
/// It is not a tolerance and it is barely a measurement: `measured_framing`
/// scales each entry so its [`family::framed_half`] times its scale equals the
/// canonical figure's, so every entry of a family occupies **the same fraction
/// of the frame** as the figure that family shipped with. That is the property
/// worth pinning — a candidate sheet is only a judgement of figures if no cell
/// is bigger or smaller than the others for framing reasons.
#[test]
fn every_roster_entry_fills_the_frame_like_its_canonical_figure() {
    for family in [
        AttractorFamily::DeJong,
        AttractorFamily::Clifford,
        AttractorFamily::Thomas,
        AttractorFamily::Lorenz,
    ] {
        let roster = family::resolve_roster(family);
        assert!(
            roster.len() > 1,
            "{family:?} has no candidates to sheet — Phase 2 needs a menu"
        );
        let reference = family::framed_half(family, canonical(family).seed_box.0)
            * canonical(family).projection.0;
        for (index, entry) in roster.iter().enumerate() {
            let (scale, _, _) = entry.tuple.framing.projection;
            let reach = family::framed_half(family, entry.tuple.framing.seed_box.0) * scale;
            assert!(
                reach <= 1.0,
                "{family:?} entry {index} reaches {reach:.2} of the frame — out of frame"
            );
            // Entry 0 is measured against its own *pinned* box rather than a
            // measurement, so it sits within a few per cent of the reference
            // instead of exactly on it; the measured entries land on it.
            assert!(
                (reach / reference - 1.0).abs() < 0.35,
                "{family:?} entry {index} fills {reach:.3} against the canonical \
                 {reference:.3} — the cells are not comparable"
            );
        }
    }
}

/// **The framing travels with the tuple, so `reseed` does too** — the Plan 0062
/// coupling ADR-0093 names as the thing a naive roster would silently break.
///
/// `jitter_extent` is a fraction of the entry's own box, so a bigger figure gets
/// a proportionally bigger kick. Had the roster carried coefficients without
/// framing, the kick would have stayed sized to the canonical butterfly — which
/// on a figure twice the size is a disturbance half as strong as the one the
/// preset asked for, and that failure is invisible in a still.
#[test]
fn a_measured_entry_derives_its_own_reseed_kick() {
    const FAMILY: AttractorFamily = AttractorFamily::Lorenz;
    let roster = family::resolve_roster(FAMILY);
    let knot = roster
        .get(KNOT)
        .map(|e| e.tuple)
        .expect("the provisional entry ships");
    let [kx, ky, kz] = knot.framing.jitter_extent();
    let [cx, cy, cz] = canonical(FAMILY).jitter_extent();
    // Per axis, and every one of them larger: the knot overruns the canonical
    // butterfly on all three.
    for (kick, canon, axis) in [(kx, cx, "x"), (ky, cy, "y"), (kz, cz, "z")] {
        assert!(
            kick > canon * 1.1,
            "the knot's {axis} kick is {kick}, the canonical {canon} — the \
             derivation is not following the entry"
        );
    }
    // And it is still the same *fraction* of the entry's own box, which is what
    // makes one constant serve every figure.
    let ([hx, hy, hz], _) = knot.framing.seed_box;
    for (kick, half) in [(kx, hx), (ky, hy), (kz, hz)] {
        assert!(
            (kick / half - super::JITTER_FRACTION).abs() < 1e-6,
            "kick {kick} against half-extent {half} is not JITTER_FRACTION"
        );
    }
}

/// The kick lands on the GPU too, on a measured entry, and the cloud comes back
/// — the render-path half of the test above, and the done-when the Plan 0062
/// coupling is checked by.
#[test]
fn a_reseed_disturbs_a_measured_entry_and_it_reconverges() {
    const FAMILY: AttractorFamily = AttractorFamily::Lorenz;
    let Some(mut h) = Harness::new(FAMILY) else {
        return;
    };
    h.scene.set_param("tuple", KNOT as f32);
    h.run(CONVERGE_FRAMES);
    assert_eq!(
        h.scene.tuple_index, KNOT,
        "the scene is not on the entry under test"
    );
    let before = h.positions();
    let occupied = Occupancy::of(&before);
    let (lo_before, hi_before) = extent(&before);

    // The kick alone, with no step alongside it — see `kick_only`. At rho ~ 100
    // one fixed step carries a particle several world units, which is more than
    // the disturbance itself, so a normal frame cannot measure the kick.
    h.kick_only();
    let after = h.positions();
    let moved = before
        .iter()
        .zip(after.iter())
        .filter(|(a, b)| a != b)
        .count();
    assert!(
        moved * 10 > before.len() * 9,
        "a reseed on entry {KNOT} moved only {moved} of {}",
        before.len()
    );

    // The kick's measured magnitude, against what each framing would predict.
    // A uniform draw over [-j, j] has mean |offset| = j/2, so this separates
    // "derived from the entry" from "derived from the family" numerically
    // rather than by inspection.
    let mean_kick = before
        .iter()
        .zip(after.iter())
        .map(|(a, b)| {
            let ([_, ay, _], [_, by, _]) = (a, b);
            (by - ay).abs()
        })
        .sum::<f32>()
        / before.len() as f32;
    let [_, entry_j, _] = h.scene.entry().framing.jitter_extent();
    let [_, canonical_j, _] = canonical(FAMILY).jitter_extent();
    println!(
        "mean |dy| after a reseed on entry {KNOT}: {mean_kick:.3} (the entry \
         predicts {:.3}, the canonical framing {:.3})",
        entry_j / 2.0,
        canonical_j / 2.0
    );
    assert!(
        (mean_kick - entry_j / 2.0).abs() < entry_j / 8.0,
        "the kick does not match the entry's own extent"
    );
    assert!(
        (mean_kick - canonical_j / 2.0).abs() > canonical_j / 4.0,
        "the instrument cannot tell the two framings apart, so it proves nothing"
    );

    // ...and the figure survives it: still on the attractor, and back to its own
    // extent afterwards.
    let outside = occupied.fraction_outside(&after);
    assert!(
        outside < 0.05,
        "a reseed put {:.1}% of the knot off its own figure",
        outside * 100.0
    );
    // **The disturbance decays**, which is the claim — not that it is gone by
    // some particular frame. rho ~ 100 is a marginally stable orbit: a kick of a
    // few per cent sends the trajectory on a wide excursion before it falls
    // back, measured at ~2.2x the figure's extent and decaying over hundreds of
    // steps, where the canonical butterfly absorbs the same disturbance within a
    // handful of frames. That is a property of this provisional tuple worth
    // knowing before it is curated in — a `reseed` on it is a slow bloom rather
    // than a shimmer — and it is why the assertion below is about the direction
    // the extent is moving rather than about a threshold it must be under.
    h.run(120);
    let (lo_peak, hi_peak) = extent(&h.positions());
    h.run(600);
    let (lo_late, hi_late) = extent(&h.positions());
    let span = |lo: [f32; 3], hi: [f32; 3], axis: usize| {
        hi.get(axis).copied().unwrap_or(0.0) - lo.get(axis).copied().unwrap_or(0.0)
    };
    for axis in 0..3 {
        let (was, peak, late) = (
            span(lo_before, hi_before, axis),
            span(lo_peak, hi_peak, axis),
            span(lo_late, hi_late, axis),
        );
        println!("axis {axis}: {was:.1} before -> {peak:.1} at the excursion -> {late:.1} after");
        assert!(
            peak > was,
            "axis {axis} never widened past its pre-reseed {was} — the kick did \
             not reach the figure"
        );
        assert!(
            late < peak,
            "axis {axis} spans {late} after ten more seconds against {peak} at \
             the excursion — the disturbance is not decaying"
        );
        assert!(
            late < 1.5 * was,
            "axis {axis} is still {late} against a pre-reseed {was} — the cloud \
             has been left off its figure rather than disturbed on it"
        );
    }
}

/// **The CPU step mirror is the shader's**, asserted by running both.
///
/// The whole roster rests on it: a framing measured off different arithmetic
/// frames a figure the GPU does not draw. A transcription error in one of the
/// four arms would produce a plausible-looking extent and a quietly mis-framed
/// entry, which no capture-level check would name.
#[test]
fn the_cpu_step_mirrors_the_shader() {
    for family in [
        AttractorFamily::DeJong,
        AttractorFamily::Clifford,
        AttractorFamily::Thomas,
        AttractorFamily::Lorenz,
    ] {
        let Some(mut h) = Harness::new(family) else {
            return;
        };
        let seeded = canonical_seed(family, TEST_PARTICLES);
        // One frame is exactly one fixed step at the capture `dt`.
        h.run(1);
        let stepped = h.positions();
        let coeffs = family.default_coeffs();
        let ([hx, hy, hz], _) = canonical(family).seed_box;
        let scale = hx.max(hy).max(hz);
        let worst = seeded
            .iter()
            .zip(stepped.iter())
            .map(|(seed, gpu)| {
                let [cx, cy, cz] = family::step_once(family, coeffs, seed.pos);
                let [gx, gy, gz] = *gpu;
                (cx - gx).abs().max((cy - gy).abs()).max((cz - gz).abs())
            })
            .fold(0.0f32, f32::max);
        println!(
            "{family:?}: worst CPU/GPU step disagreement {worst:.3e} in a figure \
             of scale {scale}"
        );
        assert!(
            worst < 1e-3 * scale,
            "{family:?}: the CPU step mirror disagrees with the shader by {worst} \
             (figure scale {scale})"
        );
    }
}

/// The integrator's sub-step count is one number in two languages, held to the
/// Rust side — `the_churn_constants_agree_between_rust_and_wgsl`'s discipline,
/// applied to the constant the measurement depends on.
#[test]
fn the_ode_substeps_agree_between_rust_and_wgsl() {
    let expected = format!("const ODE_SUBSTEPS: i32 = {};", family::ODE_SUBSTEPS);
    assert!(
        super::STEP_SHADER.contains(&expected),
        "the step shader should carry `{expected}` — the Rust constant moved and \
         the WGSL literal did not, so a measured framing would frame a figure the \
         GPU does not draw"
    );
}

// -----------------------------------------------------------------------
// The tuple walk (Plan 0079 Phase 5 / ADR-0093)
// -----------------------------------------------------------------------

/// The mechanism is **inert until a preset names a path** — Phase 5's done-when,
/// and the reason no golden baseline moves.
///
/// `morph` is an existing param that has always been IFS-only in effect; giving
/// it a second meaning on the map families is only safe if the map families
/// still ignore it when no path is configured. Asserted on the coefficients and
/// the framing together, since the walk drives both.
#[test]
fn without_a_path_morph_leaves_a_map_family_alone() {
    let Some(mut h) = Harness::new(AttractorFamily::Lorenz) else {
        return;
    };
    h.scene.reset_params();
    h.scene.update(&AnalysisFrame::default());
    let (coeffs, framing) = (
        [h.scene.a, h.scene.b, h.scene.c, h.scene.d],
        h.scene.entry().framing,
    );
    for morph in [0.0f32, 0.25, 0.5, 1.0, 7.0, -3.0] {
        h.scene.reset_params();
        h.scene.set_param("morph", morph);
        h.scene.update(&AnalysisFrame::default());
        assert_eq!(
            [h.scene.a, h.scene.b, h.scene.c, h.scene.d],
            coeffs,
            "morph = {morph} moved a map family's coefficients with no path configured"
        );
        assert_eq!(
            h.scene.entry().framing,
            framing,
            "morph = {morph} moved the framing with no path configured"
        );
    }
    assert!(
        h.scene.tuple_walk.is_none(),
        "no path was configured, so there should be no walk"
    );
}

/// **The ends are the entries, exactly** — a walk parked at either end renders
/// the roster entry it names rather than a re-measurement of it.
///
/// That is what makes a path safe to add to an existing preset: at `morph = 0`
/// nothing about the figure changes.
#[test]
fn a_walk_lands_exactly_on_its_endpoints() {
    const FAMILY: AttractorFamily = AttractorFamily::Lorenz;
    const FROM: u32 = 0;
    const TO: u32 = 4;
    let roster = family::resolve_roster(FAMILY);
    let reference = family::family_reference(FAMILY).expect("the butterfly is measurable");
    let (a, b) = (
        roster.get(FROM as usize).expect("entry 0").tuple,
        roster.get(TO as usize).expect("entry 4").tuple,
    );
    let walk = family::TupleWalk::build(FAMILY, a, b, reference).expect("a measurable path");

    assert_eq!(walk.coeffs_at(0.0), a.coeffs);
    assert_eq!(walk.coeffs_at(1.0), b.coeffs);
    assert_eq!(walk.framing_at(0.0), a.framing);
    assert_eq!(walk.framing_at(1.0), b.framing);

    // Out of range parks on an end rather than leaving the measured pair — the
    // walk must never reach coefficients nobody framed.
    assert_eq!(walk.coeffs_at(-2.0), a.coeffs);
    assert_eq!(walk.coeffs_at(9.0), b.coeffs);
    assert_eq!(walk.coeffs_at(f32::NAN), a.coeffs);
    assert_eq!(walk.framing_at(f32::NAN), a.framing);
}

/// **The middle is measured, not averaged** — the property the whole walk rests
/// on, and the one an interpolated framing would silently get wrong.
///
/// A figure halfway between two tuples is a figure in its own right; its extent
/// is whatever it is, and only coincidentally the mean of its endpoints'. So the
/// mid-walk framing must differ from the average of the two ends — and must
/// still be a usable frame.
#[test]
fn the_middle_of_a_walk_is_measured_rather_than_averaged() {
    const FAMILY: AttractorFamily = AttractorFamily::Lorenz;
    let roster = family::resolve_roster(FAMILY);
    let reference = family::family_reference(FAMILY).expect("measurable");
    let (a, b) = (
        roster.first().expect("entry 0").tuple,
        roster.get(4).expect("entry 4").tuple,
    );
    let walk = family::TupleWalk::build(FAMILY, a, b, reference).expect("a measurable path");

    let mid = walk.framing_at(0.5);
    let averaged = (a.framing.projection.0 + b.framing.projection.0) * 0.5;
    println!(
        "mid-walk scale {:.5}, endpoint average {:.5}",
        mid.projection.0, averaged
    );
    assert!(
        (mid.projection.0 - averaged).abs() > 1e-6,
        "the mid-walk framing is the endpoints' average, so it was interpolated \
         rather than measured"
    );

    // ...and every sample along it is a frame the GPU can use.
    for step in 0..=20 {
        let t = step as f32 / 20.0;
        let framing = walk.framing_at(t);
        let (scale, _, centre) = framing.projection;
        let (half, box_centre) = framing.seed_box;
        assert!(
            scale.is_finite() && scale > 0.0,
            "scale {scale} at morph {t} is unusable"
        );
        assert!(
            centre
                .iter()
                .chain(half.iter())
                .chain(box_centre.iter())
                .all(|v| v.is_finite()),
            "framing at morph {t} is not finite"
        );
        let reach = family::framed_half(FAMILY, half) * scale;
        assert!(
            reach <= 1.0,
            "the walk reaches {reach:.2} of the frame at morph {t} — it spills"
        );
    }
}

/// The walk moves the figure **continuously**, which is the whole point: this is
/// the one place in the attractor's surface where a param is *not* quantized,
/// and the contrast with `tuple` is the mechanism ADR-0093 describes.
#[test]
fn a_walk_is_continuous_where_the_selector_cuts() {
    const FAMILY: AttractorFamily = AttractorFamily::Lorenz;
    let roster = family::resolve_roster(FAMILY);
    let reference = family::family_reference(FAMILY).expect("measurable");
    let (a, b) = (
        roster.first().expect("entry 0").tuple,
        roster.get(4).expect("entry 4").tuple,
    );
    let walk = family::TupleWalk::build(FAMILY, a, b, reference).expect("a measurable path");

    // Coefficients: every small step in `morph` is a small step in the figure.
    let span = a
        .coeffs
        .iter()
        .zip(b.coeffs.iter())
        .map(|(x, y)| (x - y).abs())
        .fold(0.0f32, f32::max);
    let mut worst = 0.0f32;
    let mut previous = walk.coeffs_at(0.0);
    for step in 1..=100 {
        let now = walk.coeffs_at(step as f32 / 100.0);
        let jump = previous
            .iter()
            .zip(now.iter())
            .map(|(x, y)| (x - y).abs())
            .fold(0.0f32, f32::max);
        worst = worst.max(jump);
        previous = now;
    }
    println!("worst per-percent coefficient step {worst:.4} across a span of {span:.2}");
    assert!(
        worst < span * 0.02,
        "a 1% step in morph moved a coefficient by {worst}, which is not a walk"
    );

    // ...where the selector, handed the same sweep, only ever lands on whole
    // entries. The two together are the ADR's distinction.
    let landed: Vec<usize> = (0..=100)
        .map(|step| family::roster_index(step as f32 / 100.0 * 4.0, roster.len()))
        .collect();
    assert!(
        landed.iter().all(|&i| i <= 4),
        "the selector left the swept range"
    );
    assert_eq!(
        landed
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        5,
        "the selector should visit exactly the five whole entries it swept across"
    );
}

/// The walk reaches the GPU: a preset that names a path renders differently at
/// two positions along it, and identically at the same one.
#[test]
fn a_walked_figure_renders_and_stays_reproducible() {
    const FAMILY: AttractorFamily = AttractorFamily::Lorenz;
    let capture = |morph: f32| -> Option<Vec<[f32; 3]>> {
        let mut h = Harness::with_path(FAMILY, 0, 4)?;
        h.scene.set_param("morph", morph);
        h.run(90);
        Some(h.positions())
    };
    let (Some(near), Some(mid), Some(far)) = (capture(0.0), capture(0.5), capture(1.0)) else {
        return;
    };
    assert_ne!(near, mid, "morph = 0.5 draws the same cloud as morph = 0");
    assert_ne!(mid, far, "morph = 0.5 draws the same cloud as morph = 1");
    let Some(again) = capture(0.5) else {
        return;
    };
    assert_eq!(mid, again, "a walked figure is not reproducible");
}

/// A path a preset cannot have is a **load error**, not a silent no-op — the
/// `morph_to` discipline, on the key that turns the walk on.
#[test]
fn an_impossible_tuple_path_is_refused_at_load() {
    let preset = |table: &str| {
        crate::preset::Preset::from_toml_str(&format!(
            "system = \"attractor\"\nname = \"p\"\n[particles]\n{table}"
        ))
    };
    // A path on an IFS, which travels through `morph_to` instead.
    assert!(preset("family = \"fern\"\ntuple_to = 1\n").is_err());
    // Either end past the roster.
    assert!(preset("family = \"lorenz\"\ntuple_to = 99\n").is_err());
    assert!(preset("family = \"lorenz\"\ntuple_from = 99\ntuple_to = 1\n").is_err());
    // A near end with no far end reads like a path and is not one.
    assert!(preset("family = \"lorenz\"\ntuple_from = 2\n").is_err());
    // A path of zero length.
    assert!(preset("family = \"lorenz\"\ntuple_from = 3\ntuple_to = 3\n").is_err());
    // ...and the valid form loads.
    assert!(preset("family = \"lorenz\"\ntuple_from = 0\ntuple_to = 4\n").is_ok());
    assert!(preset("family = \"lorenz\"\ntuple_to = 4\n").is_ok());
}

/// **Which swept pairs have a walk at all** — the CPU half of Phase 5's
/// evidence, and the half a filmstrip cannot show.
///
/// A pair whose middle cannot be framed yields no walk, and the preset then
/// simply sits on its near end: every cell of its strip renders identically,
/// which reads as "the walk does nothing" rather than "there is no walk". This
/// prints the distinction so the sweep's index can record it, and asserts only
/// the floor — that the mechanism produces *some* survivors, or Phase 5 shipped
/// a mechanism with nothing to gate.
#[test]
fn the_swept_pairs_report_whether_they_have_a_walk() {
    let pairs: [(AttractorFamily, u32, u32); 20] = [
        (AttractorFamily::DeJong, 0, 6),
        (AttractorFamily::DeJong, 0, 10),
        (AttractorFamily::DeJong, 6, 2),
        (AttractorFamily::DeJong, 9, 12),
        (AttractorFamily::DeJong, 1, 3),
        (AttractorFamily::Clifford, 0, 9),
        (AttractorFamily::Clifford, 0, 5),
        (AttractorFamily::Clifford, 9, 10),
        (AttractorFamily::Clifford, 7, 12),
        (AttractorFamily::Clifford, 2, 3),
        (AttractorFamily::Thomas, 1, 12),
        (AttractorFamily::Thomas, 2, 6),
        (AttractorFamily::Thomas, 5, 8),
        (AttractorFamily::Thomas, 8, 11),
        (AttractorFamily::Thomas, 0, 9),
        (AttractorFamily::Lorenz, 0, 1),
        (AttractorFamily::Lorenz, 0, 4),
        (AttractorFamily::Lorenz, 4, 5),
        (AttractorFamily::Lorenz, 2, 1),
        (AttractorFamily::Lorenz, 0, 11),
    ];
    let mut built = 0;
    for (family, from, to) in pairs {
        let roster = family::resolve_roster(family);
        let reference = family::family_reference(family).expect("a measurable family");
        let (Some(a), Some(b)) = (roster.get(from as usize), roster.get(to as usize)) else {
            panic!("{family:?} {from}->{to} is past the roster");
        };
        let walk = family::TupleWalk::build(family, a.tuple, b.tuple, reference);
        built += usize::from(walk.is_some());
        println!(
            "{family:?} {from} -> {to}: {}",
            if walk.is_some() {
                "walk"
            } else {
                "NO WALK - a point along it cannot be framed"
            }
        );
    }
    println!("{built} of {} swept pairs have a walk", pairs.len());
    assert!(
        built > 0,
        "no swept pair produced a walk, so the mechanism has nothing to gate"
    );
}
