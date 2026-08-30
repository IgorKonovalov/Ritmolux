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
            normalized_radii(&segs, &[], &mut u, &mut Vec::new());
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
        alpha: 1.0,
        joined: 0,
    };
    // Three concentric chords at radii 0.2, 0.5 and 0.9.
    let segs = vec![
        seg([0.2, 0.0], [0.2, 0.0]),
        seg([0.5, 0.0], [0.5, 0.0]),
        seg([0.9, 0.0], [0.9, 0.0]),
    ];
    let mut u = Vec::new();
    normalized_radii(&segs, &[], &mut u, &mut Vec::new());
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
/// preset ships it. It is a fixture here, and nothing else.
fn mandala_roster() -> Vec<RingSpec> {
    vec![
        ring(Motif::Trefoil, 1, 0.00, 0.46),
        ring(Motif::Diamond, 12, 0.30, 0.20),
        ring(Motif::Petal, 18, 0.52, 0.26),
        ring(Motif::Circle, 24, 0.70, 0.13),
    ]
}

/// **The roster is three families now, and every roster-wide assertion below
/// picks the one it is about** rather than iterating [`Motif::ALL`] and meaning
/// only a third of it. Sampled polylines (`diamond`, `chevron`); exact single
/// arcs (`circle`, `arc`); and fitted G1 arc chains (`petal`, `teardrop`,
/// `trefoil`). ADR-0098 and Plan 0087 Phases 3 and 5.
fn polyline_motifs() -> Vec<Motif> {
    Motif::ALL
        .iter()
        .copied()
        .filter(|m| m.arc_shape().is_none() && m.chain().is_none() && !m.is_scallop())
        .collect()
}

fn single_arc_motifs() -> Vec<Motif> {
    Motif::ALL
        .iter()
        .copied()
        .filter(|m| m.arc_shape().is_some())
        .collect()
}

fn fitted_motifs() -> Vec<Motif> {
    Motif::ALL
        .iter()
        .copied()
        .filter(|m| m.chain().is_some())
        .collect()
}

/// Points **on the drawn figure**, of either kind: a segment's two endpoints,
/// and an arc walked at [`ARC_WALK`] steps. The one measure that reads a placed
/// motif the same whether it is a polyline, a single arc or a fitted chain —
/// which is what a placement assertion needs now that one roster holds all
/// three.
///
/// An arc is **walked** rather than read at its two ends because an extent is
/// what these assertions measure: a fitted chain puts its piece boundaries
/// where the fit's budget ran out, which is nowhere in particular, so the two
/// ends of a piece are not where the figure reaches furthest.
fn drawn_points(segs: &[SegmentInstance], arcs: &[ArcInstance]) -> Vec<[f32; 2]> {
    let mut out = Vec::with_capacity(2 * segs.len() + ARC_WALK * arcs.len());
    for s in segs {
        out.push(s.a);
        out.push(s.b);
    }
    for a in arcs {
        for k in 0..=ARC_WALK {
            let angle = a.angle_start + a.angle_sweep * k as f32 / ARC_WALK as f32;
            out.push([
                a.centre[0] + a.radius * angle.cos(),
                a.centre[1] + a.radius * angle.sin(),
            ]);
        }
    }
    out
}

/// Steps [`drawn_points`] walks one arc in.
const ARC_WALK: usize = 24;

/// A point set's bounding box, as `(centre, the furthest any point reaches from
/// it)` — the placement-blind way to say where a placed copy sits and how big
/// it is.
fn box_extent(pts: &[[f32; 2]]) -> ([f32; 2], f32) {
    let mut lo = [f32::INFINITY; 2];
    let mut hi = [f32::NEG_INFINITY; 2];
    for p in pts {
        for axis in 0..2 {
            lo[axis] = lo[axis].min(p[axis]);
            hi[axis] = hi[axis].max(p[axis]);
        }
    }
    let c = [0.5 * (lo[0] + hi[0]), 0.5 * (lo[1] + hi[1])];
    let reach = pts.iter().fold(0.0f32, |acc, p| {
        let (dx, dy) = (p[0] - c[0], p[1] - c[1]);
        acc.max((dx * dx + dy * dy).sqrt())
    });
    (c, reach)
}

/// The distance from the frame centre to an arc's centre of curvature — an
/// arc's counterpart to a segment's [`midpoint_radius`], and for a placed
/// circular motif exactly where that copy sits on its ring.
fn arc_radius(a: &ArcInstance) -> f32 {
    (a.centre[0] * a.centre[0] + a.centre[1] * a.centre[1]).sqrt()
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
            alpha: 1.0,
            joined: 0,
        };
        3
    ];
    let dropped = build_rings(&[], RingMotion::STATIC, CAP, &mut out, &mut Vec::new());
    assert_eq!(dropped, 0);
    assert!(out.is_empty(), "an empty roster clears rather than appends");
}

/// The roster is closed and its two name maps are inverses — the property
/// that makes an unknown `motif = "..."` a load error rather than a silent
/// fallback to whichever variant happened to be first.
///
/// **The count is eight and the two cut names are checked by name** (Plan
/// 0065 Phase 3, plus `scallop` at Plan 0087 Phase 6). `star` and `triangle`
/// were in the provisional set, so they are the two strings most likely to be
/// written by someone working from a pre-verdict draft — they must reach the
/// *unknown motif* error with its roster list, not silently draw something
/// else.
#[test]
fn the_motif_roster_round_trips_and_rejects_everything_else() {
    for &m in Motif::ALL {
        assert_eq!(Motif::from_name(m.name()), Some(m), "{}", m.name());
    }
    assert_eq!(Motif::ALL.len(), 8, "the closed roster is eight motifs");
    for bad in [
        "", "hexagon", "Circle", "crescent", "petal2", "star", "triangle",
    ] {
        assert_eq!(Motif::from_name(bad), None, "'{bad}' is outside the roster");
    }
}

/// [`Motif::instances`] is what the budget arithmetic multiplies by `count`, so
/// it has to agree with what `build_rings` actually emits rather than be
/// maintained beside it — for **both** kinds.
#[test]
fn the_declared_cost_matches_what_a_ring_emits() {
    let mut pts = Vec::new();
    for &m in Motif::ALL {
        m.outline(&mut pts);
        assert_eq!(pts.len(), m.vertex_count(), "{}: vertex count", m.name());
        assert!(pts.len() >= 3, "{}: a motif needs a shape", m.name());

        let mut out = Vec::new();
        let mut arcs = Vec::new();
        let spec = ring(m, 1, 0.5, 1.0);
        build_rings(&[spec], RingMotion::STATIC, CAP, &mut out, &mut arcs);
        // Copies for every motif but `scallop`, whose `count` is a lobe count
        // with a floor — so the roster-wide invariant is per *placed element*,
        // and for the other seven that is exactly one.
        let placed = placed_count(&spec) as usize;
        assert_eq!(
            out.len(),
            m.segments() * placed,
            "{}: segments emitted",
            m.name()
        );
        assert_eq!(arcs.len(), m.arcs() * placed, "{}: arcs emitted", m.name());
        assert_eq!(
            out.len() + arcs.len(),
            m.instances() * placed,
            "{}: the budget must charge what the ring emits",
            m.name()
        );
    }
}

/// **The circular motifs cost one instance each and have no interior joint —
/// asserted structurally, by the instance count, not by looking at pixels.**
///
/// This is ADR-0098's whole claim about the roster, as a number: a `circle` was
/// `SMOOTH_SAMPLES` segments and `SMOOTH_SAMPLES` additively-overlapping joints,
/// and is now one arc and none. Zero joints is not "fewer joints": there is no
/// interior vertex for a bead to form at, at any motif `scale` and any capture
/// size, because there is no vertex at all.
#[test]
fn a_circular_motif_is_one_arc_with_no_interior_joint() {
    for m in single_arc_motifs() {
        let mut out = Vec::new();
        let mut arcs = Vec::new();
        build_rings(
            &[ring(m, 1, 0.5, 0.3)],
            RingMotion::STATIC,
            CAP,
            &mut out,
            &mut arcs,
        );
        assert_eq!(arcs.len(), 1, "{}: one copy is one arc", m.name());
        assert!(
            out.is_empty(),
            "{}: an arc-drawn motif emits no segments, so it has no vertex for \
             a joint to sit at — got {} of them",
            m.name(),
            out.len()
        );
    }

    // And the saving is the order of magnitude ADR-0098 claims, against what
    // the polyline cost — read off `vertex_count`, which still describes the
    // outline `outline` samples.
    for m in single_arc_motifs() {
        let was = if m.is_closed() {
            m.vertex_count()
        } else {
            m.vertex_count() - 1
        };
        assert!(
            was >= 12 && m.instances() == 1,
            "{}: {was} segments became {} instance",
            m.name(),
            m.instances()
        );
    }
}

/// **Phase 5's claim about the roster, as a property and as numbers.**
///
/// A fitted motif reaches the GPU as a G1 chain: consecutive pieces share both
/// an endpoint and a tangent, so there is no tangent discontinuity for the eye
/// to read as a corner — *except* at the figure's own corners, which the fit
/// preserves deliberately and which are a handful rather than one per sample.
/// That is ADR-0098's "the bead count collapses to the number of genuinely
/// different curves", asserted on the placed geometry rather than on the fit,
/// because placement is where a rotation could quietly break it.
#[test]
fn a_fitted_motif_is_a_g1_chain_rather_than_a_polygon() {
    // An arbitrary placement, so the assertion is about what a mandala draws
    // rather than about the local frame the fit ran in.
    const PHASE: f32 = 0.7;
    const SCALE: f32 = 0.3;
    for m in fitted_motifs() {
        let was = if m.is_closed() {
            m.vertex_count()
        } else {
            m.vertex_count() - 1
        };
        println!(
            "{}: {was} segments -> {} arcs + {} segments",
            m.name(),
            m.arcs(),
            m.segments()
        );
        assert!(
            m.instances() < was,
            "{}: a fitted chain must cost less than the polyline it replaces, got {} against {was}",
            m.name(),
            m.instances()
        );
        assert_eq!(
            m.segments(),
            0,
            "{}: these three outlines are curves everywhere, so the fit has no straight run to emit",
            m.name()
        );

        let mut out = Vec::new();
        let mut arcs = Vec::new();
        build_rings(
            &[RingSpec {
                motif: m,
                count: 1,
                radius: 0.6,
                scale: SCALE,
                phase: PHASE,
            }],
            RingMotion::STATIC,
            CAP,
            &mut out,
            &mut arcs,
        );
        assert_eq!(arcs.len(), m.arcs(), "{}: one copy is its chain", m.name());

        // Walk the placed chain. A closed motif's last piece hands back to its
        // first, which is the joint a fit that merely stopped where it started
        // would get wrong.
        let joints = if m.is_closed() {
            arcs.len()
        } else {
            arcs.len() - 1
        };
        let mut breaks = 0usize;
        for k in 0..joints {
            let (Some(a), Some(b)) = (
                arcs.get(k).copied(),
                arcs.get((k + 1) % arcs.len().max(1)).copied(),
            ) else {
                panic!(
                    "{}: the chain is shorter than its own joint count",
                    m.name()
                );
            };
            let (_, end) = arc_ends(&a);
            let (start, _) = arc_ends(&b);
            let gap = ((end[0] - start[0]).powi(2) + (end[1] - start[1]).powi(2)).sqrt();
            assert!(
                gap < 1e-4,
                "{}: joint {k} is {gap} apart — a chain must be one curve",
                m.name()
            );
            let (_, out_t) = arc_tangents(&a);
            let (in_t, _) = arc_tangents(&b);
            let turn = (out_t[0] * in_t[1] - out_t[1] * in_t[0])
                .atan2(out_t[0] * in_t[0] + out_t[1] * in_t[1]);
            if turn.abs() > 1e-3 {
                breaks += 1;
            }
        }
        assert!(
            breaks * 4 <= was,
            "{}: {breaks} tangent discontinuities left, against {was} the polyline had — that is not a collapse",
            m.name()
        );
    }
}

/// The two ends of a placed arc, in order of travel.
fn arc_ends(a: &ArcInstance) -> ([f32; 2], [f32; 2]) {
    let at = |angle: f32| {
        [
            a.centre[0] + a.radius * angle.cos(),
            a.centre[1] + a.radius * angle.sin(),
        ]
    };
    (at(a.angle_start), at(a.angle_start + a.angle_sweep))
}

/// The unit direction of travel at a placed arc's two ends.
fn arc_tangents(a: &ArcInstance) -> ([f32; 2], [f32; 2]) {
    let sign = if a.angle_sweep < 0.0 { -1.0 } else { 1.0 };
    let at = |angle: f32| [-sign * angle.sin(), sign * angle.cos()];
    (at(a.angle_start), at(a.angle_start + a.angle_sweep))
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
                &mut Vec::new(),
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
    let mut arcs = Vec::new();
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
        &mut arcs,
    );
    // Read off the **drawn** figure: a teardrop is a fitted arc chain since
    // Plan 0087 Phase 5, so its outline reaches the GPU through `arcs` and a
    // segment-only reading of it would measure an empty buffer.
    let placed = drawn_points(&out, &arcs);
    let nearest = placed.iter().fold(f32::INFINITY, |acc, p| {
        acc.min((p[0] * p[0] + p[1] * p[1]).sqrt())
    });
    // The cusp is at local (-0.5, 0) -> radius 0.6 - 0.3 * 0.5 = 0.45, and
    // it really is the innermost point of the placed copy.
    assert!(
        (nearest - 0.45).abs() < 1e-3,
        "the cusp should land at 0.45, got {nearest}"
    );
    // ...and it points at the centre: the copy is centred at (0, 0.6), so
    // the innermost vertex is below it.
    let inner = placed
        .iter()
        .copied()
        .min_by(|x, y| {
            let r = |p: &[f32; 2]| p[0] * p[0] + p[1] * p[1];
            r(x).total_cmp(&r(y))
        })
        .unwrap_or([0.0; 2]);
    assert!(inner[1] < 0.6, "the cusp is on the centre side of the copy");
}

/// **The cap is a silent truncation, not a new failure mode** — ADR-0007's
/// behaviour on the turtle, applied here. It also must not be a *slow* one:
/// the build stops at the cap rather than looping over a large `count` to
/// count drops it will never emit, which is what the arithmetic below stands
/// in for.
#[test]
fn the_cap_truncates_and_the_drop_is_counted_without_being_surfaced() {
    let cap = 100usize;
    let mut out = Vec::with_capacity(cap);
    let mut arcs = Vec::new();
    // 40 trefoils against a 100 cap. The per-copy cost is read from the roster
    // rather than written out, because Plan 0087 Phase 5 moved it once already
    // and a literal here would be asserting the arithmetic against itself.
    let per_copy = Motif::Trefoil.instances();
    let dropped = build_rings(
        &[ring(Motif::Trefoil, 40, 0.6, 0.1)],
        RingMotion::STATIC,
        cap,
        &mut out,
        &mut arcs,
    );
    assert!(per_copy > 2, "a trefoil is more than a couple of instances");
    assert_eq!(out.len() + arcs.len(), cap, "the cap is filled exactly");
    assert_eq!(dropped, 40 * per_copy - cap, "and the rest is counted");

    // The count is honest across several rings too, and across **both kinds**:
    // one budget covers segments and arcs together, so a ring of circles eats
    // into what the diamonds left. 10 diamonds at 4 segments plus 10 circles at
    // one arc each is 50 instances against a cap of 45.
    let mut out = Vec::new();
    let mut arcs = Vec::new();
    let dropped = build_rings(
        &[
            ring(Motif::Diamond, 10, 0.4, 0.2),
            ring(Motif::Circle, 10, 0.7, 0.2),
        ],
        RingMotion::STATIC,
        45,
        &mut out,
        &mut arcs,
    );
    assert_eq!(out.len(), 40, "the diamonds fit");
    assert_eq!(
        arcs.len(),
        5,
        "and the circles take what is left of the cap"
    );
    assert_eq!(dropped, 10 * 4 + 10 - 45);

    // At the maximum a preset can declare, the whole roster still fits the
    // floor tier's cap several times over for anything but the densest
    // motif — the budget claim ADR-0079 makes, as a number.
    let widest = Motif::ALL
        .iter()
        .map(|m| m.instances())
        .max()
        .unwrap_or_default();
    assert_eq!(
        widest,
        Motif::Trefoil.instances(),
        "the trefoil is still the densest motif"
    );
    assert!(
        widest < 36,
        "and it costs fewer than the 36 segments it was, not more: {widest}"
    );
    // ...and the cheapest is a circle, which under sampled polylines is
    // among the dearest. That inversion is ADR-0098's tier-headroom
    // claim stated as an ordering.
    assert_eq!(
        Motif::Circle.instances(),
        1,
        "a circle costs one instance, where it cost SMOOTH_SAMPLES"
    );
    assert!(
        Motif::Circle.instances() < Motif::Chevron.instances(),
        "the circle is now cheaper than the cheapest polyline motif"
    );
    // One maximum ring stays under the floor tier's cap on its own, and it now
    // does so with a third more room than the 18 432 instances the same ring
    // costs with an unfitted trefoil in it (Plan 0087 Phase 5).
    let ceiling = widest * MAX_RING_COUNT as usize;
    assert!(
        ceiling < 18_432,
        "the densest maximum ring must have got cheaper, not dearer: {ceiling}"
    );
    assert!(ceiling <= CAP);
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
    // Both kinds, because the outermost of the four rings is `circle` and since
    // Plan 0087 that ring is arcs. Counting segments alone would drop the shell
    // this measurement most cares about and quietly weaken the claim.
    let occupied = |segs: &[SegmentInstance], arcs: &[ArcInstance]| -> usize {
        let mut hit = [false; SHELLS];
        let mut mark = |r: f32| {
            let k = ((r / 0.9 * SHELLS as f32) as usize).min(SHELLS - 1);
            if let Some(slot) = hit.get_mut(k) {
                *slot = true;
            }
        };
        for seg in segs {
            mark(midpoint_radius(seg));
        }
        for arc in arcs {
            mark(arc_radius(arc));
        }
        hit.iter().filter(|h| **h).count()
    };

    let bare = rosette(12, 20.0);
    let bare_shells = occupied(&bare, &[]);

    let mut rings = Vec::new();
    let mut ring_arcs = Vec::new();
    build_rings(
        &mandala_roster(),
        RingMotion::STATIC,
        CAP,
        &mut rings,
        &mut ring_arcs,
    );
    let mut combined = bare.clone();
    combined.extend(rings.iter());
    let mandala_shells = occupied(&combined, &ring_arcs);

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

    // And the instance count is the other half of "materially more figure",
    // comfortably inside the budget ADR-0079 quotes — and now far further
    // inside it than when the roster was authored, because three of its four
    // rings left the segment budget entirely (ADR-0098): the 24-copy `circle`
    // ring fell from 576 segments to 24 arcs at Phase 3, and the `trefoil` and
    // `petal` rings from 36 and 24 segments a copy to fitted chains at Phase 5.
    // Only the 12 diamonds are still polyline.
    let total = combined.len() + ring_arcs.len();
    let want_arcs = Motif::Trefoil.arcs() + 18 * Motif::Petal.arcs() + 24 * Motif::Circle.arcs();
    assert_eq!(
        ring_arcs.len(),
        want_arcs,
        "the trefoil, petal and circle rings are all arcs now"
    );
    assert_eq!(
        combined.len(),
        24 + 12 * Motif::Diamond.segments(),
        "24 interlace + the diamond ring, the only polyline left in the roster"
    );
    assert_eq!(
        total,
        combined.len() + want_arcs,
        "and that is the whole figure, against 1 116 instances before Plan 0087"
    );
    assert!(total * 17 < CAP, "room for the interlace on top");

    // The radial colour axis was identically flat on the bare rosette (see
    // the module docs); over the combined figure it is a real range, which
    // is what makes `hue_spread` a live lever on a mandala preset.
    let mut u = Vec::new();
    let mut arc_u = Vec::new();
    normalized_radii(&combined, &ring_arcs, &mut u, &mut arc_u);
    let both = || u.iter().chain(arc_u.iter()).copied();
    let lo = both().fold(f32::INFINITY, f32::min);
    let hi = both().fold(f32::NEG_INFINITY, f32::max);
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
    build_rings(
        &mandala_roster(),
        RingMotion::STATIC,
        CAP,
        &mut a,
        &mut Vec::new(),
    );
    build_rings(
        &mandala_roster(),
        RingMotion::STATIC,
        CAP,
        &mut b,
        &mut Vec::new(),
    );
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
    build_rings(
        &roster,
        RingMotion::STATIC,
        CAP,
        &mut declared,
        &mut Vec::new(),
    );
    build_rings(
        &roster,
        RingMotion::from_params(
            DEFAULT_RING_PHASE,
            DEFAULT_RING_SPREAD,
            DEFAULT_RING_SCALE_PARAM,
        ),
        CAP,
        &mut moved,
        &mut Vec::new(),
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
    build_rings(
        &by_hand,
        RingMotion::STATIC,
        CAP,
        &mut want,
        &mut Vec::new(),
    );
    build_rings(
        &roster,
        RingMotion::from_params(0.0, spread, scale),
        CAP,
        &mut got,
        &mut Vec::new(),
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
    //
    // A **polyline** motif, so this stays an assertion about placement rather
    // than about which primitive draws it; the arc counterpart is asserted
    // directly below, on the same roster, because placement is the one thing
    // that had to be re-derived for a shape carrying its own orientation.
    const RADIUS: f32 = 0.6;
    let roster: Vec<RingSpec> = (0..4)
        .map(|_| ring(Motif::Diamond, 1, RADIUS, 0.12))
        .collect();
    let per_ring = Motif::Diamond.segments();

    let turn = 0.35f32;
    let mut out = Vec::new();
    build_rings(
        &roster,
        RingMotion::from_params(turn, 1.0, 1.0),
        CAP,
        &mut out,
        &mut Vec::new(),
    );
    assert_eq!(out.len(), per_ring * roster.len());

    // The same four rings drawn as arcs: one instance per ring, and its centre
    // is where the polyline's centroid is.
    let arc_roster: Vec<RingSpec> = (0..4)
        .map(|_| ring(Motif::Circle, 1, RADIUS, 0.12))
        .collect();
    let mut arcs = Vec::new();
    build_rings(
        &arc_roster,
        RingMotion::from_params(turn, 1.0, 1.0),
        CAP,
        &mut Vec::new(),
        &mut arcs,
    );
    assert_eq!(arcs.len(), arc_roster.len(), "one arc per single-copy ring");
    for (index, arc) in arcs.iter().enumerate() {
        let angle = arc.centre[1].atan2(arc.centre[0]);
        let want = ring_direction(index) * turn;
        assert!(
            (angle - want).abs() < 1e-3,
            "arc ring {index} turned to {angle}, expected {want}"
        );
        assert!(
            (arc_radius(arc) - RADIUS).abs() < 1e-3,
            "arc ring {index} left its radius: {}",
            arc_radius(arc)
        );
        // The placement is one rotation for the orientation **and** the
        // position: a circle looks the same at any orientation, so the only
        // way to see that half is in the angle the instance carries.
        assert!(
            (arc.angle_start - want).abs() < 1e-3,
            "arc ring {index} carries start {} rather than the ring's own turn \
             {want} — a motif whose orientation matters would be placed \
             unrotated",
            arc.angle_start
        );
    }

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
    // A **fitted** motif, so the two levers are asserted on the family Plan
    // 0087 Phase 5 moved rather than on the one it left alone; the measure
    // reads points on the drawn figure and so is blind to which primitive
    // carries them.
    let roster = vec![ring(Motif::Petal, 1, RADIUS, SCALE)];
    let extent = |pts: &[[f32; 2]]| -> (f32, f32) {
        let (c, reach) = box_extent(pts);
        ((c[0] * c[0] + c[1] * c[1]).sqrt(), reach)
    };
    let build = |motion: RingMotion| -> Vec<[f32; 2]> {
        let (mut segs, mut arcs) = (Vec::new(), Vec::new());
        build_rings(&roster, motion, CAP, &mut segs, &mut arcs);
        drawn_points(&segs, &arcs)
    };

    let (r0, reach0) = extent(&build(RingMotion::STATIC));
    let (r1, reach1) = extent(&build(RingMotion::from_params(0.0, 1.6, 1.0)));
    assert!((r1 / r0 - 1.6).abs() < 1e-3, "spread must scale the radius");
    assert!(
        (reach1 - reach0).abs() < 1e-4,
        "spread must not resize the motif"
    );

    let (r2, reach2) = extent(&build(RingMotion::from_params(0.0, 1.0, 2.5)));
    assert!((r2 - r0).abs() < 1e-4, "scale must not move the ring");
    assert!(
        (reach2 / reach0 - 2.5).abs() < 1e-3,
        "scale must resize the motif"
    );

    // Neither can change how much geometry there is — that is what keeps the
    // roster structural and the draw buffer allocation-free once built.
    let counts: Vec<usize> = [
        RingMotion::STATIC,
        RingMotion::from_params(0.0, 1.6, 1.0),
        RingMotion::from_params(0.0, 1.0, 2.5),
    ]
    .into_iter()
    .map(|motion| build(motion).len())
    .collect();
    assert!(
        counts.windows(2).all(|w| w[0] == w[1]),
        "a lever changed the instance count: {counts:?}"
    );

    // The same two levers on an arc-drawn ring, where "how big a copy is" is
    // the instance's own radius rather than a spread of vertices.
    let arc_roster = vec![ring(Motif::Circle, 1, RADIUS, SCALE)];
    let arc_extent = |motion: RingMotion| -> (f32, f32) {
        let mut arcs = Vec::new();
        build_rings(&arc_roster, motion, CAP, &mut Vec::new(), &mut arcs);
        let arc = arcs.first().copied().expect("one arc");
        (arc_radius(&arc), arc.radius)
    };
    let (ar0, areach0) = arc_extent(RingMotion::STATIC);
    let (ar1, areach1) = arc_extent(RingMotion::from_params(0.0, 1.6, 1.0));
    let (ar2, areach2) = arc_extent(RingMotion::from_params(0.0, 1.0, 2.5));
    assert!(
        (ar1 / ar0 - 1.6).abs() < 1e-3 && (areach1 - areach0).abs() < 1e-5,
        "spread must move an arc's ring without resizing it, got \
         {ar0}->{ar1} at radius {areach0}->{areach1}"
    );
    assert!(
        (ar2 - ar0).abs() < 1e-4 && (areach2 / areach0 - 2.5).abs() < 1e-3,
        "scale must resize an arc without moving its ring, got \
         {ar0}->{ar2} at radius {areach0}->{areach2}"
    );
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
    build_rings(&mandala_roster(), hot, CAP, &mut out, &mut Vec::new());
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
    // The polyline family only. Neither the circular motifs nor the fitted
    // ones have interior vertices to join — which
    // `a_circular_motif_is_one_arc_with_no_interior_joint` and
    // `a_fitted_motif_is_a_g1_chain_rather_than_a_polygon` assert directly;
    // iterating the whole roster here would silently assert nothing about them.
    for m in polyline_motifs() {
        let mut out = Vec::new();
        build_rings(
            &[ring(m, 1, 0.5, 0.3)],
            RingMotion::STATIC,
            CAP,
            &mut out,
            &mut Vec::new(),
        );
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

// -----------------------------------------------------------------------
// The circular motif is round at every scale (Plan 0087 Phase 3, ADR-0098)
// -----------------------------------------------------------------------

/// The square target the roundness measurement runs at. Square so a polar
/// sweep about the frame centre needs no aspect correction of its own — the
/// aspect claim is Phase 1's, and `renderer/tests.rs` runs its control at 4:3
/// where a wrong one shows.
const ROUND_SIZE: u32 = 768;

/// Spokes in the polar sweep. Not a multiple of `SMOOTH_SAMPLES`, so a spoke
/// cannot systematically land on a polygon vertex or systematically miss one.
const SPOKES: usize = 181;

/// Draw one ring's geometry through a bare [`LineRenderer`] into a linear
/// target and read the light back, unclamped.
fn round_capture(segments: &[SegmentInstance], arcs: &[ArcInstance]) -> Option<Vec<f32>> {
    use crate::render::capture;
    use crate::render::context::RenderContext;
    use crate::render::scenes::lines::{LineRenderer, ViewTransform};

    const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

    let ctx = match RenderContext::new_headless(ROUND_SIZE, ROUND_SIZE, true) {
        Ok(ctx) => ctx,
        Err(_) => {
            eprintln!("skipped: no GPU adapter on this runner (ADR-0016)");
            return None;
        }
    };
    let texture = ctx.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("round-target"),
        size: wgpu::Extent3d {
            width: ROUND_SIZE,
            height: ROUND_SIZE,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&Default::default());
    let mut encoder = ctx
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("round"),
        });
    // The line pass loads rather than clears; in the shipped chain the
    // background pass owns the clear, so this stands in for it.
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("round-clear"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: &view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });

    let mut renderer = LineRenderer::new_with_arcs(
        &ctx.device,
        FORMAT,
        segments.len().max(1),
        arcs.len().max(1),
        "round",
    );
    renderer.draw_arcs(
        &ctx.queue,
        &mut encoder,
        &view,
        1.0,
        1.0,
        crate::render::scenes::lines::DEFAULT_SOFTNESS,
        ViewTransform::default(),
        segments,
        arcs,
    );

    let (buffer, padded_bpr) = capture::create_linear_readback(&ctx.device, ROUND_SIZE, ROUND_SIZE);
    capture::record_copy(
        &mut encoder,
        &texture,
        &buffer,
        padded_bpr,
        ROUND_SIZE,
        ROUND_SIZE,
    );
    ctx.queue.submit(std::iter::once(encoder.finish()));
    Some(
        capture::read_back_linear(&ctx.device, &buffer, ROUND_SIZE, ROUND_SIZE, padded_bpr)
            .expect("read back the roundness capture"),
    )
}

/// Walk out from the frame centre along `SPOKES` evenly spaced rays and report,
/// per ray, **where** the stroke's light sits and **how much** of it there is:
/// its brightness-weighted mean radius in pixels, and the integral of
/// brightness along the ray.
///
/// Both are integrals rather than peaks, and that is what makes them
/// measurable. A peak sample is the brightest *pixel* near the centreline, so
/// it moves with where the pixel grid happens to fall relative to the stroke —
/// on a 4.6 px half-width that alone spreads the reading 21 % around a ring
/// that is perfectly uniform. An integral over a bilinearly sampled ray has no
/// such phase: a **round** stroke puts its light at the same radius on every
/// ray, and an **unbeaded** one puts the same amount of it there.
fn polar_profile(px: &[f32], max_radius_px: f32) -> Vec<(f32, f32)> {
    let n = ROUND_SIZE as usize;
    let centre = ROUND_SIZE as f32 / 2.0;
    // Bilinear, so the walk resolves the stroke rather than the pixel grid.
    let sample = |x: f32, y: f32| -> f32 {
        let (x0, y0) = (x.floor(), y.floor());
        let (fx, fy) = (x - x0, y - y0);
        let at = |ix: f32, iy: f32| -> f32 {
            if ix < 0.0 || iy < 0.0 || ix >= n as f32 || iy >= n as f32 {
                return 0.0;
            }
            px.get((iy as usize * n + ix as usize) * 4)
                .copied()
                .unwrap_or(0.0)
        };
        let top = at(x0, y0) * (1.0 - fx) + at(x0 + 1.0, y0) * fx;
        let bottom = at(x0, y0 + 1.0) * (1.0 - fx) + at(x0 + 1.0, y0 + 1.0) * fx;
        top * (1.0 - fy) + bottom * fy
    };
    (0..SPOKES)
        .map(|k| {
            let theta = std::f32::consts::TAU * k as f32 / SPOKES as f32;
            let (sin, cos) = theta.sin_cos();
            let (mut sum, mut weighted) = (0.0f32, 0.0f32);
            // Quarter-pixel steps: finer than the stroke being integrated, so
            // the reading is limited by the stroke and not by the walk.
            let steps = (max_radius_px * 4.0) as usize;
            for step in 0..=steps {
                let r = step as f32 / 4.0;
                let v = sample(centre + r * cos - 0.5, centre - r * sin - 0.5);
                sum += v;
                weighted += v * r;
            }
            (if sum > 0.0 { weighted / sum } else { 0.0 }, sum)
        })
        .collect()
}

/// How many times wider the beaded control's per-ray light spread must be than
/// the arc's.
///
/// **A ratio, not a frozen threshold** (ADR-0071). Both readings are the same
/// statistic — `(max - min) / max` of the light each ray integrates — taken in
/// this one test, at one capture size, through one profile, so the absolute
/// figures may move with the rasterizer while their *separation* is the
/// property. The previous form was a bare `0.12` asserted universally against
/// both arms and naming no configuration, while its own comment already said
/// the gap between the arms was what carried the property rather than the
/// absolute figure.
///
/// Measured at 768 px and a 0.012 NDC half-width: the arc spreads **7.9 %** at
/// ornament scale and **7.4 %** at full frame; the 24-gon control spreads
/// **34.3 %** and carries about twice the light per ray — a separation of
/// **4.36x**. It reads the same on both adapters, 34.3 / 7.9 on the software
/// rasterizer and 34.3 / 7.9 on this machine's hardware one, and that agreement
/// is what earns the ratio: ADR-0074 records one that moved 7.3x between two
/// builds of a single rasterizer, because its two terms answered to the machine
/// differently. These two are one statistic over two geometries.
///
/// **The floor is 3.0 because the regression was measured, not guessed.**
/// Dropping the control's `joined` flags — the exact edit the assertion's
/// message names, and what stops a bead summing at every vertex — leaves it
/// spreading **15.8 %**, a separation of **2.01x**. That is well above the
/// spread a bare threshold would have compared against (the retired form's
/// `0.12`, which this regression also passed), because a min/max spread is
/// sign-blind: an unjoined 24-gon has a *gap* at every vertex, so it varies
/// around the ring in the other direction and reads almost as uneven as a
/// beaded one. 3.0 sits between the two with 1.45x of margin above the
/// regression and 1.45x below the passing value.
///
/// **The arc's own figure was 5 % while the stroke was a pure quadratic
/// falloff, and a crisper stroke legitimately quantizes harder** (Plan 0114
/// Phase 5). A solid core contributes whole pixels to a ray's sum where a
/// gradient contributes fractions, so the sum steps as the stroke's sub-pixel
/// position varies around the ring. That is smooth and everywhere, nothing like
/// the 24 localized spots ADR-0041's joins sum at the control's vertices — and
/// it is the reason the absolutes are printed rather than asserted: the arc's
/// figure answers to the stroke profile, and the separation does not.
const BEAD_RATIO: f32 = 3.0;

/// **A `circle` motif is round, and evenly bright, at ornament scale and at
/// full frame** — the resolution-independence claim ADR-0098 makes, checked at
/// the small scale where the polygon was visible rather than at the large one
/// where it was not.
///
/// The control is the **same circle as a `SMOOTH_SAMPLES` polyline**, which is
/// exactly what shipped and what the user rejected: `Motif::outline` still
/// returns it, so the two sides of the comparison come from one definition.
///
/// # What the polyline actually fails on, and it is not roundness
///
/// The chord error of a 24-gon is `r * (1 - cos(pi/24))`, about 0.86 % of the
/// radius — a fifth of a pixel at ornament scale, which no capture can see.
/// The **bead** is what is visible: ADR-0041 extends each joined end by a half
/// width, the composite is additive, and 24 vertices sum to 24 bright spots
/// around the ring. So the assertion that discriminates is on *brightness
/// around the ring*, and the roundness arm is asserted of the arc alone, at
/// both scales, as the resolution claim.
#[test]
fn a_circle_motif_is_round_and_unbeaded_at_ornament_scale_and_full_frame() {
    // `scale` 0.13 is the retired `star_mandala`'s outermost ring; 1.8 fills the
    // frame. A motif spans roughly one unit, so the drawn radius is half of it.
    // Each scale's per-ray light spread, asserted against the control below.
    let mut arc_spread: Vec<(f32, f32)> = Vec::new();
    for motif_scale in [0.13f32, 1.8] {
        let radius_world = 0.5 * motif_scale;
        let radius_px = radius_world * ROUND_SIZE as f32 / 2.0;
        let roster = vec![ring(Motif::Circle, 1, 0.0, motif_scale)];

        let mut segments = Vec::new();
        let mut arcs = Vec::new();
        build_rings(&roster, RingMotion::STATIC, CAP, &mut segments, &mut arcs);
        assert!(
            segments.is_empty() && arcs.len() == 1,
            "the circle motif must be exactly one arc"
        );
        // A visible stroke at both scales; the profile is read off its peak, so
        // the width only has to resolve.
        let width = 0.012;
        for arc in &mut arcs {
            arc.width = width;
        }

        let Some(drawn) = round_capture(&[], &arcs) else {
            return;
        };
        let profile = polar_profile(&drawn, ROUND_SIZE as f32 / 2.0);
        let radii: Vec<f32> = profile.iter().map(|p| p.0).collect();
        let light: Vec<f32> = profile.iter().map(|p| p.1).collect();
        let lo = radii.iter().copied().fold(f32::INFINITY, f32::min);
        let hi = radii.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        let vlo = light.iter().copied().fold(f32::INFINITY, f32::min);
        let vhi = light.iter().copied().fold(f32::NEG_INFINITY, f32::max);

        eprintln!(
            "arc circle at motif scale {motif_scale} ({radius_px:.1} px \
             radius): light at radius {lo:.2}..{hi:.2} px, light per ray \
             {vlo:.3}..{vhi:.3} ({:.1} % spread)",
            (vhi - vlo) / vhi * 100.0
        );
        assert!(
            vlo > 1.0,
            "the stroke is not resolved at motif scale {motif_scale} (dimmest \
             ray carries {vlo:.3}) — the profile below would be read off noise"
        );
        // Round: the light sits at one radius on every ray, and the radius it
        // is meant to sit at is the arc's own.
        assert!(
            hi - lo <= 0.5,
            "the arc-drawn circle's light sits between {lo:.2} and {hi:.2} px \
             at motif scale {motif_scale} — it is not round"
        );
        assert!(
            (0.5 * (lo + hi) - radius_px).abs() <= 1.0,
            "the arc-drawn circle sits at {:.2} px where its own geometry puts \
             it at {radius_px:.2}",
            0.5 * (lo + hi)
        );
        // Unbeaded: it has no interior vertex, so nothing sums anywhere. The
        // reading is carried out of the loop and asserted against the control's
        // below, because the separation is the property and the absolute figure
        // answers to the stroke profile (see `BEAD_RATIO`).
        arc_spread.push((motif_scale, (vhi - vlo) / vhi));
    }

    // --- The control: the same circle as the polyline that shipped. ---
    //
    // Built from `Motif::outline`, placed by the same arithmetic `build_rings`
    // uses for a polyline motif, and joined the way a closed chain is
    // (ADR-0041) — which is the whole of what makes the beads.
    const MOTIF_SCALE: f32 = 0.13;
    let mut pts = Vec::new();
    Motif::Circle.outline(&mut pts);
    let n = pts.len();
    let polyline: Vec<SegmentInstance> = (0..n)
        .filter_map(|e| {
            let a = *pts.get(e)?;
            let b = *pts.get((e + 1) % n)?;
            Some(SegmentInstance {
                a: [a[0] * MOTIF_SCALE, a[1] * MOTIF_SCALE],
                b: [b[0] * MOTIF_SCALE, b[1] * MOTIF_SCALE],
                color: [1.0, 1.0, 1.0],
                width: 0.012,
                joined: JOINED_A | JOINED_B,
                alpha: 1.0,
            })
        })
        .collect();
    assert_eq!(polyline.len(), n, "the control is the full 24-gon");

    let Some(drawn) = round_capture(&polyline, &[]) else {
        return;
    };
    let profile = polar_profile(&drawn, ROUND_SIZE as f32 / 2.0);
    let light: Vec<f32> = profile.iter().map(|p| p.1).collect();
    let vlo = light.iter().copied().fold(f32::INFINITY, f32::min);
    let vhi = light.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    eprintln!(
        "polyline circle at motif scale {MOTIF_SCALE}: light per ray \
         {vlo:.3}..{vhi:.3} ({:.1} % spread)",
        (vhi - vlo) / vhi * 100.0
    );
    let control_spread = (vhi - vlo) / vhi;
    let (worst_scale, worst_arc) =
        arc_spread.iter().copied().fold(
            (f32::NAN, 0.0f32),
            |acc, s| if s.1 > acc.1 { s } else { acc },
        );
    eprintln!(
        "bead separation: control {:.1} % against the arc's worst {:.1} % (at \
         motif scale {worst_scale}) = {:.2}x, floor {BEAD_RATIO:.1}x",
        control_spread * 100.0,
        worst_arc * 100.0,
        control_spread / worst_arc
    );
    assert!(
        control_spread > BEAD_RATIO * worst_arc,
        "the {n}-gon control's per-ray light spreads {:.1} % around the ring \
         against the arc's worst {:.1} %, a separation of only {:.2}x. The \
         property is that a joined polyline BEADS and an arc does not, so \
         either the control has stopped being joined (ADR-0041, which is what \
         sums a bead at every vertex) or the arc has started beading",
        control_spread * 100.0,
        worst_arc * 100.0,
        control_spread / worst_arc
    );
}

/// **Phase 6's done-when: the boundary is one closed curve whose lobe count is
/// a parameter** — the primitive the user chose at Plan 0065 Phase 2 over a
/// ring of overlapping `arc` motifs faking continuity (design-backlog 0071).
///
/// The distinction that makes it the real thing rather than the approximation
/// is asserted here directly: consecutive lobes **share the point where they
/// meet**, so the chain is one curve. A ring of placed copies does not — it
/// overlaps, which is what "faking one" meant.
#[test]
fn the_scalloped_boundary_is_one_closed_chain_of_lobes() {
    const BASE: f32 = 0.7;
    const DEPTH: f32 = 0.12;
    for lobes in [3u32, 6, 12, 40] {
        let mut arcs = Vec::new();
        build_rings(
            &[ring(Motif::Scallop, lobes, BASE, DEPTH)],
            RingMotion::STATIC,
            CAP,
            &mut Vec::new(),
            &mut arcs,
        );
        assert_eq!(
            arcs.len(),
            lobes as usize,
            "the lobe count is the parameter, and it is `count`"
        );

        // One closed curve: every lobe ends exactly where the next begins, and
        // the last hands back to the first.
        for k in 0..arcs.len() {
            let (Some(a), Some(b)) = (
                arcs.get(k).copied(),
                arcs.get((k + 1) % arcs.len()).copied(),
            ) else {
                panic!("{lobes} lobes: the chain is short");
            };
            let (_, end) = arc_ends(&a);
            let (start, _) = arc_ends(&b);
            let gap = ((end[0] - start[0]).powi(2) + (end[1] - start[1]).powi(2)).sqrt();
            assert!(gap < 1e-4, "{lobes} lobes: cusp {k} is {gap} apart");
        }

        // The cusps sit on the base circle and the apexes reach `depth` past
        // it — which is what makes it a scallop rather than a ring or a blob.
        for arc in &arcs {
            let (start, _) = arc_ends(arc);
            let cusp = (start[0] * start[0] + start[1] * start[1]).sqrt();
            assert!(
                (cusp - BASE).abs() < 1e-3,
                "{lobes} lobes: a cusp left the base circle at {cusp}"
            );
            let mid = arc.angle_start + 0.5 * arc.angle_sweep;
            let apex = [
                arc.centre[0] + arc.radius * mid.cos(),
                arc.centre[1] + arc.radius * mid.sin(),
            ];
            let reach = (apex[0] * apex[0] + apex[1] * apex[1]).sqrt();
            assert!(
                (reach - (BASE + DEPTH)).abs() < 1e-3,
                "{lobes} lobes: an apex reached {reach}, not {}",
                BASE + DEPTH
            );
        }
    }
}

/// The boundary's two degenerate ends, which a bound `ring_scale` reaches by
/// sweeping rather than by being asked for.
#[test]
fn a_zero_depth_scallop_is_the_plain_ring_and_a_short_count_is_raised() {
    const BASE: f32 = 0.5;
    // Depth 0: every lobe is an arc of the base circle itself, so the chain is
    // that circle. This is what makes `ring_scale` a continuous lever on this
    // member — a preset sweeping it through zero passes through a plain ring
    // rather than through something undefined.
    let mut arcs = Vec::new();
    build_rings(
        &[ring(Motif::Scallop, 8, BASE, 0.0)],
        RingMotion::STATIC,
        CAP,
        &mut Vec::new(),
        &mut arcs,
    );
    for arc in &arcs {
        assert!(
            (arc.radius - BASE).abs() < 1e-4 && arc_radius(arc) < 1e-4,
            "a zero-depth lobe must be an arc of the base circle, got r = {} at {}",
            arc.radius,
            arc_radius(arc)
        );
    }
    let total: f32 = arcs.iter().map(|a| a.angle_sweep.abs()).sum();
    assert!(
        (total - std::f32::consts::TAU).abs() < 1e-3,
        "eight lobes of the base circle must sum to one turn, got {total}"
    );

    // Below three lobes there is no boundary — one lobe's two ends coincide and
    // its sweep degenerates. `build_rings` raises the count rather than drawing
    // nothing, and the `wanted` fold is raised with it so the drop count stays
    // honest.
    for count in [1u32, 2] {
        let mut arcs = Vec::new();
        let dropped = build_rings(
            &[ring(Motif::Scallop, count, BASE, 0.1)],
            RingMotion::STATIC,
            CAP,
            &mut Vec::new(),
            &mut arcs,
        );
        assert_eq!(arcs.len(), MIN_SCALLOP_LOBES as usize, "count {count}");
        assert_eq!(dropped, 0, "count {count}: nothing was dropped");
        for arc in &arcs {
            assert!(
                arc.angle_sweep.abs() > 1e-3,
                "count {count}: a lobe degenerated to a zero sweep"
            );
        }
    }
}
