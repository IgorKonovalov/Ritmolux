#![allow(clippy::indexing_slicing)]

use super::*;

/// Build one rosette exactly as `build` does.
fn rosette(n: u32, contact_deg: f32) -> Vec<SegmentInstance> {
    let mut segs = Vec::new();
    hankin::star_rosette(n, contact_deg.to_radians(), &mut segs);
    turtle::normalize_fit(&mut segs, 0.9);
    segs
}

fn radius(s: &SegmentInstance) -> f32 {
    let (x, y) = (0.5 * (s.a[0] + s.b[0]), 0.5 * (s.a[1] + s.b[1]));
    (x * x + y * y).sqrt()
}

/// **Plan 0054 Phase 2's honesty clause, as a measurement rather than a
/// claim.** ADR-0059 gives this scene a radial colour axis and its own
/// Consequences call that axis "the weakest of the four"; the number is
/// worse than "weak" and it is pinned here so nobody has to re-derive it.
///
/// A Hankin rosette is `2n` **congruent** segments about a centre
/// `normalize_fit` leaves at the origin (every accepted tiling order is
/// even, so the bounding box is centred). Each therefore occupies the *same*
/// radial interval, and one colour per segment has nothing to tell them
/// apart — the spread across segments is zero at every order and every
/// contact angle, so `hue_spread` is exactly the identity on this scene.
///
/// **If this test starts failing, the interior work landed and the ramp came
/// alive.** That is the good outcome; re-point the assertion then.
#[test]
fn the_radial_axis_has_no_spread_on_the_current_rosette() {
    for n in [4u32, 6, 8, 12] {
        for contact in [CONTACT_MIN_DEG, 12.0, 20.0, 30.0, 54.0, CONTACT_MAX_DEG] {
            let segs = rosette(n, contact);
            assert_eq!(segs.len(), 2 * n as usize, "{n}-fold at {contact} deg");

            let radii: Vec<f32> = segs.iter().map(radius).collect();
            let lo = radii.iter().copied().fold(f32::INFINITY, f32::min);
            let hi = radii.iter().copied().fold(f32::NEG_INFINITY, f32::max);
            assert!(
                hi - lo < RADIAL_FLOOR,
                "{n}-fold at {contact} deg: segment radii span {lo}..{hi}, so \
                 the radial axis is no longer flat and this measurement is stale"
            );

            // ...and the normalization reports that as "no ramp" rather than
            // amplifying the float noise into a palette sweep.
            let mut u = Vec::new();
            normalized_radii(&segs, &mut u);
            assert_eq!(u.len(), segs.len());
            assert!(
                u.iter().all(|&x| x == 0.0),
                "{n}-fold at {contact} deg: a figure with no radial spread \
                 must collapse to u = 0, not to noise"
            );
        }
    }
}

/// The figure's own radial extent — the "hollow ring" measurement — is a
/// different quantity from the per-segment spread above, and it is the one
/// design-backlog 0007 reported. The rosette's vertices span
/// `sin(a) / sin(pi/n + a)` to `1` before the fit; everything inside is
/// empty. Recorded so the interior work has a starting number.
#[test]
fn the_rosette_leaves_its_interior_empty() {
    let n = 12u32;
    for contact in [12.0f32, 20.0, 30.0] {
        let segs = rosette(n, contact);
        let vertex = |p: [f32; 2]| (p[0] * p[0] + p[1] * p[1]).sqrt();
        let mut lo = f32::INFINITY;
        let mut hi = f32::NEG_INFINITY;
        for seg in &segs {
            for p in [seg.a, seg.b] {
                lo = lo.min(vertex(p));
                hi = hi.max(vertex(p));
            }
        }
        let a = contact.to_radians();
        let predicted = a.sin() / (std::f32::consts::PI / n as f32 + a).sin();
        assert!(
            (lo / hi - predicted).abs() < 1e-3,
            "{n}-fold at {contact} deg: inner radius fraction {} should be \
             sin(a)/sin(pi/n + a) = {predicted}",
            lo / hi
        );
        assert!(
            lo / hi > 0.4,
            "the interior is empty from the centre out to {}",
            lo / hi
        );
    }
}

/// The angle a variant asks for, at the golden fixture's base angle.
const BASE: f32 = 35.0;

fn geometry_at(variant: f32) -> Vec<SegmentInstance> {
    let mut cache = RosetteCache::default();
    cache.request(12, contact_angle_deg(BASE, variant));
    cache.segments.clone()
}

fn differs(a: &[SegmentInstance], b: &[SegmentInstance]) -> bool {
    a.len() != b.len()
        || a.iter().zip(b).any(|(x, y)| {
            (x.a[0] - y.a[0]).abs() > 1e-4
                || (x.a[1] - y.a[1]).abs() > 1e-4
                || (x.b[0] - y.b[0]).abs() > 1e-4
                || (x.b[1] - y.b[1]).abs() > 1e-4
        })
}

/// **Plan 0054 Phase 3 done-when 1 (ADR-0060): intermediate `variant` values
/// produce intermediate geometry.** Under the old `floor` into three cached
/// rosettes the middle frame was *identical* to one of the ends, which is
/// exactly what makes this non-vacuous — the assertion that would have failed
/// before is the one that the halfway figure differs from **both**.
#[test]
fn a_half_variant_is_a_real_rosette_between_the_two_ends() {
    let lo = geometry_at(0.0);
    let mid = geometry_at(0.5);
    let hi = geometry_at(1.0);

    assert!(differs(&lo, &hi), "the two ends must differ at all");
    assert!(
        differs(&mid, &lo),
        "variant 0.5 collapsed onto variant 0 — this is the floor, back again"
    );
    assert!(
        differs(&mid, &hi),
        "variant 0.5 collapsed onto variant 1 — this is the floor, back again"
    );

    // And it really is *between* them, not merely different: the petal tip
    // radius is monotone in the contact angle, so the middle figure's inner
    // radius sits between the two ends'.
    let inner = |segs: &[SegmentInstance]| {
        segs.iter().fold(f32::INFINITY, |acc, s| {
            acc.min((s.a[0].powi(2) + s.a[1].powi(2)).sqrt())
                .min((s.b[0].powi(2) + s.b[1].powi(2)).sqrt())
        })
    };
    let (a, b, c) = (inner(&lo), inner(&mid), inner(&hi));
    assert!(a < b && b < c, "inner radii must be ordered: {a} {b} {c}");
}

/// The compatibility claim ADR-0060's "the precomputed-variant vocabulary
/// disappears" was worried about, and the reason no baseline moved: `variant`
/// 0 / 1 / 2 still name the `-24 / 0 / +24` degree offsets the three cached
/// rosettes held, so a preset binding integers draws what it always drew.
#[test]
fn the_integer_variants_are_the_angles_the_cache_used_to_hold() {
    for (variant, offset) in [(0.0f32, -24.0f32), (1.0, 0.0), (2.0, 24.0)] {
        let want = (BASE + offset).clamp(CONTACT_MIN_DEG, CONTACT_MAX_DEG);
        assert!(
            (contact_angle_deg(BASE, variant) - want).abs() < 1e-5,
            "variant {variant} must still mean {want} degrees"
        );
    }
    // Out of range clamps to the ends rather than running off, as the old
    // `min(variants - 1)` index clamp did.
    assert_eq!(contact_angle_deg(BASE, -3.0), contact_angle_deg(BASE, 0.0));
    assert_eq!(contact_angle_deg(BASE, 9.0), contact_angle_deg(BASE, 2.0));
    // Total over what an expression can actually produce.
    for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let a = contact_angle_deg(BASE, bad);
        assert!(
            a.is_finite() && (CONTACT_MIN_DEG..=CONTACT_MAX_DEG).contains(&a),
            "variant {bad} produced {a}"
        );
    }
}

/// **Plan 0054 Phase 3 done-when 2: a swept `variant` does not rebuild every
/// frame.** The bound is *distance travelled / step*, not the frame count —
/// which is the whole content of the hysteresis, and the thing that keeps
/// ADR-0007's off-hot-path guarantee once a bound param can reach the
/// generator.
#[test]
fn a_swept_variant_rebuilds_per_step_not_per_frame() {
    const FRAMES: usize = 2_000;
    let mut cache = RosetteCache::default();

    for frame in 0..FRAMES {
        let variant = 2.0 * frame as f32 / (FRAMES - 1) as f32;
        cache.request(12, contact_angle_deg(BASE, variant));
    }

    // The sweep covers the whole variant range: 2 * VARIANT_SPAN_DEG degrees
    // of contact angle. `+ 2` for the initial build and the final partial
    // step.
    let travelled = 2.0 * VARIANT_SPAN_DEG;
    let bound = (travelled / STEP_DEG) as u64 + 2;
    assert!(
        cache.rebuilds() <= bound,
        "{} rebuilds over a {travelled}-degree sweep exceeds the {bound} the \
         {STEP_DEG}-degree step allows",
        cache.rebuilds()
    );
    assert!(
        cache.rebuilds() < FRAMES as u64 / 2,
        "{} rebuilds over {FRAMES} frames is tracking the frame rate, not \
         the step",
        cache.rebuilds()
    );

    // The other half of hysteresis, and the half a quantized-bucket key
    // would get wrong: a `variant` dithering inside one step never rebuilds
    // after the first, however many frames it runs for.
    let mut steady = RosetteCache::default();
    steady.request(12, contact_angle_deg(BASE, 1.0));
    let after_first = steady.rebuilds();
    for frame in 0..600 {
        // +/- 0.002 of a variant unit is +/- 0.048 degrees, just under half
        // a step — the band a quantized-bucket key would rebuild across on
        // every crossing.
        let jitter = if frame % 2 == 0 { 0.002 } else { -0.002 };
        steady.request(12, contact_angle_deg(BASE, 1.0 + jitter));
    }
    assert_eq!(
        steady.rebuilds(),
        after_first,
        "a variant jittering inside one step must never rebuild"
    );
}

/// The rebuild is bounded work, and this pins the number the frame-budget
/// measurement in the module docs rests on: a rosette is `2n` segments, and
/// the loader's whole tiling vocabulary stops at `n = 12`. So the rebuild
/// this scene can actually be asked for is 24 segments — three orders of
/// magnitude under the floor tier's 20 000-segment cap.
#[test]
fn the_reachable_rebuild_is_two_dozen_segments() {
    let mut widest = 0usize;
    for name in ["square", "hexagon", "octagon", "dodecagon"] {
        let order = hankin::tiling_order(name).unwrap_or(0);
        let mut cache = RosetteCache::default();
        cache.request(order, contact_angle_deg(BASE, 1.0));
        assert_eq!(cache.segments.len(), 2 * order as usize, "{name}");
        widest = widest.max(cache.segments.len());
    }
    assert_eq!(widest, 24, "the loader's largest tiling is 12-fold");
    assert!(widest * 800 < crate::render::TierConfig::FLOOR.max_segments);
}

/// A rebuild reuses its buffers, so a sweeping `variant` allocates nothing
/// after the first build for an order — the ADR-0007 property the cache
/// exists to keep.
#[test]
fn rebuilding_reuses_the_cache_buffers() {
    let mut cache = RosetteCache::default();
    cache.request(12, contact_angle_deg(BASE, 0.0));
    let (segs, radii) = (cache.segments.capacity(), cache.radii.capacity());
    for frame in 0..200 {
        let variant = 2.0 * frame as f32 / 199.0;
        cache.request(12, contact_angle_deg(BASE, variant));
    }
    assert!(cache.rebuilds() > 1, "the sweep must actually rebuild");
    assert_eq!(cache.segments.capacity(), segs, "segments reallocated");
    assert_eq!(cache.radii.capacity(), radii, "radii reallocated");
}

/// The degenerate guard is a *spread* floor, not a radius floor: a figure
/// that genuinely does span radii still ramps across its full range.
#[test]
fn a_figure_with_radial_spread_still_ramps_across_it() {
    let seg = |a: [f32; 2], b: [f32; 2]| SegmentInstance {
        a,
        b,
        color: [0.0; 3],
        width: 0.01,
        joined: 0,
    };
    // Three concentric chords at radii 0.2, 0.5 and 0.9.
    let segs = vec![
        seg([0.2, 0.0], [0.2, 0.0]),
        seg([0.5, 0.0], [0.5, 0.0]),
        seg([0.9, 0.0], [0.9, 0.0]),
    ];
    let mut u = Vec::new();
    normalized_radii(&segs, &mut u);
    assert!((u[0] - 0.0).abs() < 1e-5, "innermost is 0, got {}", u[0]);
    assert!((u[2] - 1.0).abs() < 1e-5, "outermost is 1, got {}", u[2]);
    assert!(
        u[1] > u[0] && u[1] < u[2],
        "the middle chord lands between, got {}",
        u[1]
    );
}

// -----------------------------------------------------------------------
// The mandala interior (Plan 0065 Phase 1 / ADR-0079)
// -----------------------------------------------------------------------

const CAP: usize = crate::render::TierConfig::FLOOR.max_segments;

fn ring(motif: Motif, count: u32, radius: f32, scale: f32) -> RingSpec {
    RingSpec {
        motif,
        count,
        radius,
        scale,
        phase: 0.0,
    }
}

/// The four-ring roster `presets/star_mandala.toml` shipped with, so the
/// coverage claim below is measured on a figure that was really authored
/// rather than on a fixture invented to pass.
///
/// **That preset was retired on 2026-08-06** — the sampled-polyline motifs
/// read as polygons at ornament scale and the user rejected the look
/// (design-backlog 0073). The roster is kept here verbatim because what it
/// pins is the *geometry* claim — rings put segments at four radii where a
/// bare interlace puts them at one — and that is unaffected by whether any
/// preset ships it. It is now a fixture, and no longer also a preset.
fn mandala_roster() -> Vec<RingSpec> {
    vec![
        ring(Motif::Trefoil, 1, 0.00, 0.46),
        ring(Motif::Diamond, 12, 0.30, 0.20),
        ring(Motif::Petal, 18, 0.52, 0.26),
        ring(Motif::Circle, 24, 0.70, 0.13),
    ]
}

fn midpoint_radius(s: &SegmentInstance) -> f32 {
    let (x, y) = (0.5 * (s.a[0] + s.b[0]), 0.5 * (s.a[1] + s.b[1]));
    (x * x + y * y).sqrt()
}

/// **Phase 1 done-when 1, stated where it can be checked without a GPU: with
/// `rings` absent nothing new runs.** The pixel half of the claim is the
/// `star_pattern` golden fixture (also rings-less) still matching its
/// committed baseline; this is the half that says *why* it must — an empty
/// roster produces an empty ornament, and `base` then hands out the cache's
/// own buffers rather than a copy of them.
#[test]
fn an_absent_roster_builds_no_ornament_at_all() {
    let mut out = vec![
        SegmentInstance {
            a: [9.0, 9.0],
            b: [9.0, 9.0],
            color: [0.0; 3],
            width: 1.0,
            joined: 0,
        };
        3
    ];
    let dropped = build_rings(&[], RingMotion::STATIC, CAP, &mut out);
    assert_eq!(dropped, 0);
    assert!(out.is_empty(), "an empty roster clears rather than appends");
}

/// The roster is closed and its two name maps are inverses — the property
/// that makes an unknown `motif = "..."` a load error rather than a silent
/// fallback to whichever variant happened to be first.
///
/// **The count is seven and the two cut names are checked by name** (Plan
/// 0065 Phase 3). `star` and `triangle` were in the provisional set, so they
/// are the two strings most likely to be written by someone working from a
/// pre-verdict draft — they must reach the *unknown motif* error with its
/// roster list, not silently draw something else.
#[test]
fn the_motif_roster_round_trips_and_rejects_everything_else() {
    for &m in Motif::ALL {
        assert_eq!(Motif::from_name(m.name()), Some(m), "{}", m.name());
    }
    assert_eq!(Motif::ALL.len(), 7, "the closed roster is seven motifs");
    for bad in [
        "", "hexagon", "Circle", "crescent", "petal2", "star", "triangle",
    ] {
        assert_eq!(Motif::from_name(bad), None, "'{bad}' is outside the roster");
    }
}

/// [`Motif::segments`] is what the budget arithmetic multiplies by `count`,
/// so it has to agree with the outline rather than be maintained beside it.
#[test]
fn the_declared_segment_count_matches_the_outline() {
    let mut pts = Vec::new();
    for &m in Motif::ALL {
        m.outline(&mut pts);
        assert_eq!(pts.len(), m.vertex_count(), "{}: vertex count", m.name());
        assert!(pts.len() >= 3, "{}: a motif needs a shape", m.name());
        let expected = if m.is_closed() {
            pts.len()
        } else {
            pts.len() - 1
        };
        assert_eq!(m.segments(), expected, "{}: segment count", m.name());

        let mut out = Vec::new();
        build_rings(&[ring(m, 1, 0.5, 1.0)], RingMotion::STATIC, CAP, &mut out);
        assert_eq!(out.len(), m.segments(), "{}: one copy", m.name());
    }
}

/// **The placement arithmetic, which is the whole of ADR-0079's geometry.**
/// `count` copies land at `2*pi*i/count + phase` and at `radius` from the
/// centre, so a ring's segment set is invariant under a `2*pi/count`
/// rotation — the same property `hankin.rs` asserts of the rosette, and the
/// one that lets a ring count and a fold order be chosen together.
#[test]
fn a_ring_places_its_copies_evenly_around_the_frame_centre() {
    for &m in Motif::ALL {
        for count in [3u32, 8, 17] {
            let radius = 0.6;
            let scale = 0.18;
            let mut out = Vec::new();
            build_rings(
                &[ring(m, count, radius, scale)],
                RingMotion::STATIC,
                CAP,
                &mut out,
            );
            assert_eq!(out.len(), m.segments() * count as usize);

            // Every segment sits within one motif's reach of the ring.
            for seg in &out {
                let r = midpoint_radius(seg);
                assert!(
                    (r - radius).abs() <= scale,
                    "{} x{count}: a segment at {r} is off the {radius} ring",
                    m.name()
                );
            }

            // ...and the set as a whole is invariant under one sector.
            let ang = TAU / count as f32;
            let (s, c) = ang.sin_cos();
            let rot = |p: [f32; 2]| [p[0] * c - p[1] * s, p[0] * s + p[1] * c];
            for seg in &out {
                let (ra, rb) = (rot(seg.a), rot(seg.b));
                let matched = out.iter().any(|o| {
                    let close = |x: [f32; 2], y: [f32; 2]| {
                        (x[0] - y[0]).abs() < 1e-4 && (x[1] - y[1]).abs() < 1e-4
                    };
                    close(o.a, ra) && close(o.b, rb)
                });
                assert!(matched, "{} x{count}: not {count}-fold symmetric", m.name());
            }
        }
    }
}

/// The local convention every outline is authored to — **outward is `+x`** —
/// is what makes one rotation supply both position and orientation. Checked
/// on the one motif where "which way is out" is unambiguous: the teardrop's
/// cusp must end up pointing at the frame centre, not away from it.
#[test]
fn a_placed_motif_keeps_its_outward_end_outward() {
    let mut pts = Vec::new();
    Motif::Teardrop.outline(&mut pts);
    // Local: the widest part of the drop sits on the +x half.
    let widest = pts
        .iter()
        .fold(f32::NEG_INFINITY, |acc, p| acc.max(p[1].abs()));
    let widest_x = pts
        .iter()
        .filter(|p| p[1].abs() > 0.5 * widest)
        .fold(f32::NEG_INFINITY, |acc, p| acc.max(p[0]));
    assert!(widest_x > 0.0, "the drop's body is on the outward side");

    // Placed at the top of a ring (phase = pi/2), the cusp is the point
    // nearest the centre and the body is further out.
    let mut out = Vec::new();
    build_rings(
        &[RingSpec {
            motif: Motif::Teardrop,
            count: 1,
            radius: 0.6,
            scale: 0.3,
            phase: std::f32::consts::FRAC_PI_2,
        }],
        RingMotion::STATIC,
        CAP,
        &mut out,
    );
    let nearest = out.iter().fold(f32::INFINITY, |acc, s| {
        acc.min((s.a[0] * s.a[0] + s.a[1] * s.a[1]).sqrt())
    });
    // The cusp is at local (-0.5, 0) -> radius 0.6 - 0.3 * 0.5 = 0.45, and
    // it really is the innermost point of the placed copy.
    assert!(
        (nearest - 0.45).abs() < 1e-3,
        "the cusp should land at 0.45, got {nearest}"
    );
    // ...and it points at the centre: the copy is centred at (0, 0.6), so
    // the innermost vertex is below it.
    let inner = out
        .iter()
        .min_by(|x, y| {
            let r = |s: &SegmentInstance| s.a[0] * s.a[0] + s.a[1] * s.a[1];
            r(x).total_cmp(&r(y))
        })
        .map(|s| s.a)
        .unwrap_or([0.0; 2]);
    assert!(inner[1] < 0.6, "the cusp is on the centre side of the copy");
}

/// **The cap is a silent truncation, not a new failure mode** (the plan's
/// explicit instruction, and ADR-0007's behaviour on the turtle). It also
/// must not be a *slow* one: the build stops at the cap rather than looping
/// over a large `count` to count drops it will never emit, which is what the
/// arithmetic below stands in for.
#[test]
fn the_cap_truncates_and_the_drop_is_counted_without_being_surfaced() {
    let cap = 100usize;
    let mut out = Vec::with_capacity(cap);
    // 40 trefoils at 36 segments each: 1 440 wanted against a 100 cap.
    let dropped = build_rings(
        &[ring(Motif::Trefoil, 40, 0.6, 0.1)],
        RingMotion::STATIC,
        cap,
        &mut out,
    );
    assert_eq!(out.len(), cap, "the cap is filled exactly");
    assert_eq!(dropped, 40 * 36 - cap, "and the rest is counted");

    // The count is honest across several rings too — the second one is the
    // whole of what a mandala over budget loses.
    let mut out = Vec::new();
    let dropped = build_rings(
        &[
            ring(Motif::Diamond, 10, 0.4, 0.2),
            ring(Motif::Circle, 10, 0.7, 0.2),
        ],
        RingMotion::STATIC,
        40,
        &mut out,
    );
    assert_eq!(out.len(), 40);
    assert_eq!(dropped, 10 * 4 + 10 * 24 - 40);

    // At the maximum a preset can declare, the whole roster still fits the
    // floor tier's cap several times over for anything but the densest
    // motif — the budget claim ADR-0079 makes, as a number.
    let widest = Motif::ALL
        .iter()
        .map(|m| m.segments())
        .max()
        .unwrap_or_default();
    assert_eq!(widest, 36, "the trefoil is the densest motif");
    assert_eq!(
        widest * MAX_RING_COUNT as usize,
        18_432,
        "one maximum ring is under the floor tier's cap on its own"
    );
    assert!(widest * MAX_RING_COUNT as usize <= CAP);
}

/// **Phase 1 done-when 3, as geometry rather than as an opinion.** Backlog
/// 0007's "hollow ring" is a *radial occupancy* claim: at `star_rosette`'s
/// 12-fold / 20-degree rosette every segment sits in one thin band near the
/// rim and the inner 60 % of the disc holds nothing. The shipped four-ring
/// roster occupies the interior instead, and the two numbers are measured
/// side by side here so the pixel-level `cover` comparison has a structural
/// counterpart that no capture size can wobble.
#[test]
fn four_rings_occupy_the_interior_the_bare_rosette_leaves_empty() {
    const SHELLS: usize = 10;
    let occupied = |segs: &[SegmentInstance]| -> usize {
        let mut hit = [false; SHELLS];
        for seg in segs {
            let r = midpoint_radius(seg) / 0.9;
            let k = ((r * SHELLS as f32) as usize).min(SHELLS - 1);
            if let Some(slot) = hit.get_mut(k) {
                *slot = true;
            }
        }
        hit.iter().filter(|h| **h).count()
    };

    let bare = rosette(12, 20.0);
    let bare_shells = occupied(&bare);

    let mut rings = Vec::new();
    build_rings(&mandala_roster(), RingMotion::STATIC, CAP, &mut rings);
    let mut combined = bare.clone();
    combined.extend(rings.iter());
    let mandala_shells = occupied(&combined);

    println!(
        "radial shells occupied (of {SHELLS}): bare rosette {bare_shells}, \
         four-ring mandala {mandala_shells}"
    );
    assert_eq!(
        bare_shells, 1,
        "the bare rosette really does live in one shell — this is the \
         'hollow ring'"
    );
    assert!(
        mandala_shells >= 6,
        "a four-ring mandala must reach the interior, got {mandala_shells}"
    );

    // And the segment count is the other half of "materially more figure",
    // comfortably inside the budget ADR-0079 quotes.
    assert_eq!(combined.len(), 1_116, "24 interlace + 1 092 ornament");
    assert!(combined.len() * 17 < CAP, "room for the interlace on top");

    // The radial colour axis was identically flat on the bare rosette (see
    // the module docs); over the combined figure it is a real range, which
    // is what makes `hue_spread` a live lever on a mandala preset.
    let mut u = Vec::new();
    normalized_radii(&combined, &mut u);
    let lo = u.iter().copied().fold(f32::INFINITY, f32::min);
    let hi = u.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    assert!(
        (lo - 0.0).abs() < 1e-5 && (hi - 1.0).abs() < 1e-5,
        "the ramp must span 0..1, got {lo}..{hi}"
    );
}

/// A rings-only preset (`tiling = "none"`, order 0) draws the ornament alone
/// — the composition the reference image is, and the one the cache has to
/// stay out of the way of. The rosette construction already returns nothing
/// below `n = 3`; what this pins is that the *cache* then rebuilds rather
/// than matching its own just-invalidated order-0 state and serving whatever
/// angle it happened to hold.
#[test]
fn order_zero_draws_no_interlace_and_never_reuses_a_stale_rosette() {
    let mut cache = RosetteCache::default();
    cache.request(12, 20.0);
    assert_eq!(cache.segments.len(), 24);

    cache.invalidate();
    let rebuilt = cache.request(0, 20.0);
    assert!(
        rebuilt,
        "an invalidated cache must rebuild, even at order 0"
    );
    assert!(cache.segments.is_empty(), "order 0 draws no interlace");
    assert!(cache.radii.is_empty());
}

/// The ornament is a **pure function of its roster** — no clock, no
/// randomness — so a mandala is the same figure on every device and in every
/// capture (the determinism rule).
#[test]
fn the_ornament_is_deterministic() {
    let mut a = Vec::new();
    let mut b = Vec::new();
    build_rings(&mandala_roster(), RingMotion::STATIC, CAP, &mut a);
    build_rings(&mandala_roster(), RingMotion::STATIC, CAP, &mut b);
    assert_eq!(a, b, "same roster -> identical geometry");
    assert!(a.iter().all(|s| s.a[0].is_finite() && s.a[1].is_finite()));
}

// -----------------------------------------------------------------------
// The rings move (Plan 0065 Phase 4)
// -----------------------------------------------------------------------

/// The mean of a slice of segment midpoints — where a single placed copy
/// sits, since every motif is authored about its own centre.
fn centroid(segs: &[SegmentInstance]) -> [f32; 2] {
    let n = segs.len().max(1) as f32;
    let mut acc = [0.0f32; 2];
    for s in segs {
        acc[0] += 0.25 * (s.a[0] + s.b[0]) * 2.0 / n;
        acc[1] += 0.25 * (s.a[1] + s.b[1]) * 2.0 / n;
    }
    acc
}

/// **The default is the exact identity, which is why Phase 1's captures do
/// not move.** Not "close enough": `+ 0.0`, `* 1.0` and `* 1.0` are exact in
/// IEEE, so the static roster is reproduced bit for bit and the shipped
/// golden baseline for this scene is untouched by Phase 4 existing.
///
/// The second half is the meaning of the two radial levers, stated as an
/// identity rather than as a tolerance: `spread` and `scale` multiply the
/// roster, so moving them is *exactly* the same figure as declaring the
/// multiplied roster in the first place.
#[test]
fn the_static_motion_is_the_identity_and_the_radial_levers_are_multipliers() {
    let roster = mandala_roster();

    let mut declared = Vec::new();
    let mut moved = Vec::new();
    build_rings(&roster, RingMotion::STATIC, CAP, &mut declared);
    build_rings(
        &roster,
        RingMotion::from_params(
            DEFAULT_RING_PHASE,
            DEFAULT_RING_SPREAD,
            DEFAULT_RING_SCALE_PARAM,
        ),
        CAP,
        &mut moved,
    );
    assert_eq!(
        declared, moved,
        "the three defaults must be RingMotion::STATIC"
    );

    // A roster scaled by hand, against the same roster moved by the levers.
    let (spread, scale) = (1.7f32, 0.6f32);
    let by_hand: Vec<RingSpec> = roster
        .iter()
        .map(|r| RingSpec {
            radius: r.radius * spread,
            scale: r.scale * scale,
            ..*r
        })
        .collect();
    let mut want = Vec::new();
    let mut got = Vec::new();
    build_rings(&by_hand, RingMotion::STATIC, CAP, &mut want);
    build_rings(
        &roster,
        RingMotion::from_params(0.0, spread, scale),
        CAP,
        &mut got,
    );
    assert_eq!(want, got, "spread and scale are multipliers on the roster");
}

/// **Phase 4's done-when, on the placement arithmetic rather than on pixels**
/// — deliberately, because the gate that would otherwise check it cannot:
/// `animation.rs` diffs 96x96 frames and a ring mandala is nearly invariant
/// under rotation, so a spinning ornament reads as frozen there however fast
/// it turns (design-backlog 0009, with more force than the rosette had).
///
/// So the claim is asserted where it is true: for a **positive** `ring_phase`,
/// adjacent rings turn in **opposite** directions.
#[test]
fn a_positive_ring_phase_turns_adjacent_rings_opposite_ways() {
    // Alternating signs, starting positive, is the whole of counter-rotation.
    for i in 0..6usize {
        let want = if i.is_multiple_of(2) { 1.0 } else { -1.0 };
        assert_eq!(ring_direction(i), want, "ring {i}");
    }

    // Four single-copy rings at the same radius: each copy's centre sits at
    // exactly its ring's own phase, so the turn is readable as an angle.
    const RADIUS: f32 = 0.6;
    let roster: Vec<RingSpec> = (0..4)
        .map(|_| ring(Motif::Circle, 1, RADIUS, 0.12))
        .collect();
    let per_ring = Motif::Circle.segments();

    let turn = 0.35f32;
    let mut out = Vec::new();
    build_rings(
        &roster,
        RingMotion::from_params(turn, 1.0, 1.0),
        CAP,
        &mut out,
    );
    assert_eq!(out.len(), per_ring * roster.len());

    for (index, chunk) in out.chunks(per_ring).enumerate() {
        let c = centroid(chunk);
        let angle = c[1].atan2(c[0]);
        let want = ring_direction(index) * turn;
        assert!(
            (angle - want).abs() < 1e-3,
            "ring {index} turned to {angle}, expected {want}"
        );
        // ...and it is a rotation, not a drift: the copy stays on its ring.
        let r = (c[0] * c[0] + c[1] * c[1]).sqrt();
        assert!(
            (r - RADIUS).abs() < 1e-3,
            "ring {index} left its radius: {r}"
        );
    }

    // The pairing is the roster's own order, so two adjacent rings really do
    // separate: the angle between ring 0 and ring 1 is twice the turn.
    let first = centroid(out.get(..per_ring).unwrap_or_default());
    let second = centroid(out.get(per_ring..2 * per_ring).unwrap_or_default());
    let between = first[1].atan2(first[0]) - second[1].atan2(second[0]);
    assert!(
        (between - 2.0 * turn).abs() < 1e-3,
        "adjacent rings should separate by twice the turn, got {between}"
    );
}

/// The two **radial** levers, which are the ones a shipped preset carries its
/// animation on. `ring_spread` moves each ring's centre distance; `ring_scale`
/// changes how big a copy is without moving the ring it sits on.
#[test]
fn spread_moves_the_rings_out_and_scale_only_grows_the_motifs() {
    const RADIUS: f32 = 0.5;
    const SCALE: f32 = 0.2;
    let roster = vec![ring(Motif::Circle, 1, RADIUS, SCALE)];
    let extent = |segs: &[SegmentInstance]| -> (f32, f32) {
        let c = centroid(segs);
        let r = (c[0] * c[0] + c[1] * c[1]).sqrt();
        let reach = segs.iter().fold(0.0f32, |acc, s| {
            let (dx, dy) = (
                0.5 * (s.a[0] + s.b[0]) - c[0],
                0.5 * (s.a[1] + s.b[1]) - c[1],
            );
            acc.max((dx * dx + dy * dy).sqrt())
        });
        (r, reach)
    };

    let mut base = Vec::new();
    build_rings(&roster, RingMotion::STATIC, CAP, &mut base);
    let (r0, reach0) = extent(&base);

    let mut spread = Vec::new();
    build_rings(
        &roster,
        RingMotion::from_params(0.0, 1.6, 1.0),
        CAP,
        &mut spread,
    );
    let (r1, reach1) = extent(&spread);
    assert!((r1 / r0 - 1.6).abs() < 1e-3, "spread must scale the radius");
    assert!(
        (reach1 - reach0).abs() < 1e-4,
        "spread must not resize the motif"
    );

    let mut scaled = Vec::new();
    build_rings(
        &roster,
        RingMotion::from_params(0.0, 1.0, 2.5),
        CAP,
        &mut scaled,
    );
    let (r2, reach2) = extent(&scaled);
    assert!((r2 - r0).abs() < 1e-4, "scale must not move the ring");
    assert!(
        (reach2 / reach0 - 2.5).abs() < 1e-3,
        "scale must resize the motif"
    );

    // Neither can change how much geometry there is — that is what keeps the
    // roster structural and the draw buffer allocation-free once built.
    assert_eq!(base.len(), spread.len());
    assert_eq!(base.len(), scaled.len());
}

/// The three levers run per frame from author expressions, so
/// [`RingMotion::from_params`] is **total**: nothing an expression can produce
/// reaches the placement arithmetic as a NaN vertex or as a figure so large
/// the frame is a wall of stroke.
#[test]
fn the_ring_levers_survive_everything_an_expression_can_produce() {
    for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let m = RingMotion::from_params(bad, bad, bad);
        assert_eq!(m, RingMotion::STATIC, "{bad} must fall back to static");
    }
    // Out of range clamps rather than running off.
    let hot = RingMotion::from_params(0.0, 99.0, 99.0);
    assert_eq!(hot.spread, MAX_RING_SPREAD);
    assert_eq!(hot.scale, MAX_RING_SCALE);
    let cold = RingMotion::from_params(0.0, -3.0, -3.0);
    assert_eq!(cold.spread, 0.0);
    assert_eq!(cold.scale, 0.0);

    // `ring_phase` is typically `k * time`, so it wraps rather than clamping —
    // an angle has no ends, and an unwrapped one loses the precision the
    // hysteresis below needs to resolve a step at all.
    let wrapped = RingMotion::from_params(3.0 * TAU + 0.25, 1.0, 1.0);
    assert!(
        (wrapped.phase - 0.25).abs() < 1e-3,
        "phase must wrap into one turn, got {}",
        wrapped.phase
    );
    assert!(
        RingMotion::from_params(-0.5, 1.0, 1.0).phase > 0.0,
        "and stay positive"
    );

    // A moved motion still places finite geometry.
    let mut out = Vec::new();
    build_rings(&mandala_roster(), hot, CAP, &mut out);
    assert!(out.iter().all(|s| s.a[0].is_finite() && s.b[1].is_finite()));
}

/// The ornament is rebuilt under **hysteresis**, exactly as the rosette is
/// (ADR-0060), and this pins what that does and does not buy — because the
/// two cases differ and the honest version is the useful one.
///
/// The bound is *distance travelled / step*, never the frame count. So a
/// preset binding none of the three never rebuilds at all, a slow lever
/// rebuilds on a fraction of its frames, and a lever turning fast enough to
/// see rebuilds every frame — which is affordable at the measured 4.9 us, see
/// [`RING_PHASE_STEP`], and is *not* a claim this test hides.
#[test]
fn the_ornament_rebuilds_per_step_not_per_frame() {
    const FRAMES: usize = 2_000;
    let rebuilds_over = |span: f32| {
        let mut held = RingMotion::STATIC;
        let mut n = 0u32;
        for frame in 0..FRAMES {
            let want = RingMotion::from_params(span * frame as f32 / (FRAMES - 1) as f32, 1.0, 1.0);
            if held.needs_rebuild(want) {
                held = want;
                n += 1;
            }
        }
        n
    };

    // A full turn over the window: `+ 2` for the first build and the final
    // partial step. The distance bound holds...
    let full = rebuilds_over(TAU);
    let bound = (TAU / RING_PHASE_STEP) as u32 + 2;
    assert!(full <= bound, "{full} rebuilds exceeds the {bound} allowed");

    // ...and at that rate it really is every frame, which is the part the
    // step does not save and the measurement in `RING_PHASE_STEP` covers.
    assert!(
        full >= FRAMES as u32 - 2,
        "a full turn per 2 000 frames moves more than a step per frame"
    );

    // A tenth of a turn over the same window is a tenth of the rebuilds —
    // distance travelled, not frames elapsed, which is the whole claim.
    let tenth = rebuilds_over(0.1 * TAU);
    assert!(
        tenth < FRAMES as u32 / 2,
        "{tenth} rebuilds over {FRAMES} frames tracks the frame rate, not the step"
    );
    let tenth_bound = (0.1 * TAU / RING_PHASE_STEP) as u32 + 2;
    assert!(
        tenth <= tenth_bound,
        "{tenth} rebuilds over a tenth of a turn exceeds the {tenth_bound} \
         its distance allows"
    );

    // Dithering inside one step never rebuilds — the half a quantized-bucket
    // key would get wrong.
    let held = RingMotion::from_params(1.0, 1.0, 1.0);
    for frame in 0..600u32 {
        let jitter = if frame.is_multiple_of(2) {
            0.0004
        } else {
            -0.0004
        };
        let want = RingMotion::from_params(1.0 + jitter, 1.0 + jitter, 1.0 + jitter);
        assert!(
            !held.needs_rebuild(want),
            "a sub-step dither must not rebuild"
        );
    }

    // And a preset that binds none of them never leaves the static motion.
    assert!(
        !RingMotion::STATIC.needs_rebuild(RingMotion::from_params(
            DEFAULT_RING_PHASE,
            DEFAULT_RING_SPREAD,
            DEFAULT_RING_SCALE_PARAM,
        )),
        "an unbound preset must never rebuild its ornament"
    );

    // Each lever is watched on its own, or a preset moving only one of them
    // would freeze.
    for want in [
        RingMotion::from_params(0.5, 1.0, 1.0),
        RingMotion::from_params(0.0, 1.4, 1.0),
        RingMotion::from_params(0.0, 1.0, 1.4),
    ] {
        assert!(
            RingMotion::STATIC.needs_rebuild(want),
            "{want:?} must rebuild"
        );
    }
}

/// The three levers are in the param vocabulary, which is what makes a
/// preset's `ring_spread = "..."` a binding rather than an ignored key — an
/// unknown name is dropped by the loader, so a lever missing from here is a
/// preset that quietly does nothing.
///
/// The defaults are pinned in the same place because they are the compat
/// claim: `reset_params` puts the scene back on [`RingMotion::STATIC`], so a
/// preset that binds none of them draws the roster it declared.
#[test]
fn the_ring_levers_are_bindable_and_default_to_the_static_configuration() {
    for lever in ["ring_phase", "ring_spread", "ring_scale"] {
        assert!(PARAMS.contains(&lever), "{lever} must be in PARAMS");
    }
    let mut seen = PARAMS.to_vec();
    seen.sort_unstable();
    let before = seen.len();
    seen.dedup();
    assert_eq!(before, seen.len(), "a param is declared twice");

    assert_eq!(
        RingMotion::from_params(
            DEFAULT_RING_PHASE,
            DEFAULT_RING_SPREAD,
            DEFAULT_RING_SCALE_PARAM,
        ),
        RingMotion::STATIC,
        "the three defaults must be the static configuration"
    );
}

/// A closed outline is a closed chain and every vertex is a joint
/// (ADR-0041); an open one is free at its two ends only. Getting this wrong
/// leaves a notch in the stroke at every motif vertex, which at the counts a
/// mandala uses is the whole figure.
#[test]
fn closed_motifs_join_everywhere_and_open_ones_only_inside() {
    for &m in Motif::ALL {
        let mut out = Vec::new();
        build_rings(&[ring(m, 1, 0.5, 0.3)], RingMotion::STATIC, CAP, &mut out);
        let flags: Vec<u32> = out.iter().map(|s| s.joined).collect();
        if m.is_closed() {
            assert!(
                flags.iter().all(|&f| f == JOINED_A | JOINED_B),
                "{}: a closed outline joins at every vertex, got {flags:?}",
                m.name()
            );
            // ...and it really does close: the last segment's `b` end is the
            // first segment's `a` end.
            let start = out.first().map(|s| s.a).unwrap_or([f32::NAN; 2]);
            let end = out.last().map(|s| s.b).unwrap_or([f32::NAN; 2]);
            assert!(
                (start[0] - end[0]).abs() < 1e-5 && (start[1] - end[1]).abs() < 1e-5,
                "{}: the outline must close",
                m.name()
            );
        } else {
            assert_eq!(flags.first().copied(), Some(JOINED_B), "{}", m.name());
            assert_eq!(flags.last().copied(), Some(JOINED_A), "{}", m.name());
            for (i, &f) in flags.iter().enumerate() {
                if i > 0 && i + 1 < flags.len() {
                    assert_eq!(f, JOINED_A | JOINED_B, "{} interior {i}", m.name());
                }
            }
        }
    }
}
