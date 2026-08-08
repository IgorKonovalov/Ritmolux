// Test asserts index the produced Vec; allowed here over the file's hot-path
// pragma since test code is not the render path.
#![allow(clippy::indexing_slicing)]

use super::*;

/// The cap these tests run at — the floor tier's, which is the value they were
/// written against and the one every shipped preset is authored and gated on
/// (Plan 0044).
const FLOOR_CAP: usize = crate::render::TierConfig::FLOOR.max_segments;

fn seg(a: [f32; 2], b: [f32; 2]) -> SegmentInstance {
    SegmentInstance {
        a,
        b,
        color: [0.4, 0.7, 1.0],
        width: 0.01,
        joined: 0,
    }
}

fn close(a: [f32; 2], b: [f32; 2]) -> bool {
    (a[0] - b[0]).abs() < 1e-3 && (a[1] - b[1]).abs() < 1e-3
}

/// A 6-fold mirror of an asymmetric base must be invariant under a `2*pi/6`
/// rotation (the Hankin-style symmetry proof) and emit exactly `order` copies.
#[test]
fn mirror_is_invariant_under_a_2pi_over_order_rotation() {
    // A deliberately asymmetric little scribble so symmetry is non-trivial.
    let single = vec![seg([0.1, 0.05], [0.4, 0.2]), seg([0.4, 0.2], [0.3, 0.5])];
    let order = 6u32;
    let mut out = Vec::new();
    let dropped = replicate_mirror(
        &single,
        MirrorSpec {
            order,
            reflect: false,
        },
        10_000,
        &mut out,
    );
    assert_eq!(dropped, 0, "well under the cap");
    assert_eq!(
        out.len(),
        single.len() * order as usize,
        "one rotated copy of the base per sector"
    );

    let ang = std::f32::consts::TAU / order as f32;
    let (s, c) = ang.sin_cos();
    let rot = |p: [f32; 2]| [p[0] * c - p[1] * s, p[0] * s + p[1] * c];
    for seg in &out {
        let ra = rot(seg.a);
        let rb = rot(seg.b);
        let matched = out.iter().any(|other| {
            (close(other.a, ra) && close(other.b, rb)) || (close(other.a, rb) && close(other.b, ra))
        });
        assert!(matched, "rotated segment has no image in the mirrored set");
    }
}

/// The identity spec (`from_params(1, 0)`) copies the base through unchanged —
/// which is *why* the scenes skip the call entirely at that spec (Plan 0031
/// Phase 4). This test is the equivalence that licenses the skip: replication
/// at an identity spec yields exactly the input, no drop and no transform, so
/// swapping the buffers instead produces the same frame.
#[test]
fn identity_spec_copies_the_base_unchanged() {
    let single = vec![
        seg([0.2, 0.3], [0.5, 0.1]),
        seg([-0.4, 0.05], [0.15, -0.35]),
    ];
    let spec = MirrorSpec::from_params(1.0, 0.0);
    assert!(spec.is_identity(), "the spec the scenes skip on");
    let mut out = Vec::new();
    let dropped = replicate_mirror(&single, spec, 10_000, &mut out);
    assert_eq!(dropped, 0, "identity never truncates");
    assert_eq!(out.len(), single.len(), "no copies added or lost");
    for (produced, source) in out.iter().zip(&single) {
        assert!(
            close(produced.a, source.a) && close(produced.b, source.b),
            "identity applies no transform"
        );
        assert_eq!(produced.color, source.color);
        assert_eq!(produced.width, source.width);
    }
}

/// Exactly which specs the scenes may skip replication for. A spec that is
/// *not* the identity must not be treated as one — that would silently drop a
/// preset's mirror.
#[test]
fn is_identity_covers_exactly_the_no_op_specs() {
    assert!(
        MirrorSpec {
            order: 1,
            reflect: false
        }
        .is_identity()
    );
    // A non-finite or sub-1 order clamps to 1, which is the identity.
    assert!(MirrorSpec::from_params(0.0, 0.0).is_identity());
    assert!(MirrorSpec::from_params(f32::NAN, 0.0).is_identity());
    assert!(
        MirrorSpec::from_params(1.4, 0.0).is_identity(),
        "rounds to 1"
    );
    // Reflection at order 1 still doubles the geometry — not an identity.
    assert!(
        !MirrorSpec {
            order: 1,
            reflect: true
        }
        .is_identity()
    );
    assert!(!MirrorSpec::from_params(1.0, 1.0).is_identity());
    assert!(!MirrorSpec::from_params(2.0, 0.0).is_identity());
    assert!(
        !MirrorSpec::from_params(1.6, 0.0).is_identity(),
        "rounds to 2"
    );
}

/// The message an operator actually reads. Plan 0031 Phase 4 replaced the
/// per-frame `format!` with an enum that formats only in `Display`; ADR-0007
/// requires the text stay informative, so both renderings are pinned here.
#[test]
fn the_overflow_message_is_unchanged() {
    assert_eq!(OverflowContext::Mirror(6).to_string(), "mirror x6");
    assert_eq!(OverflowContext::Depth(6).to_string(), "depth 6");
    assert_eq!(
        CapOverflow {
            dropped: 350,
            context: OverflowContext::Mirror(6),
            cap: FLOOR_CAP,
        }
        .to_string(),
        format!(
            "geometry exceeded the {FLOOR_CAP}-segment cap at mirror x6 \
             (dropped 350 segment(s)); reduce the structure or its depth"
        )
    );
    assert_eq!(
        CapOverflow {
            dropped: 1,
            context: OverflowContext::Depth(7),
            cap: FLOOR_CAP,
        }
        .to_string(),
        format!(
            "geometry exceeded the {FLOOR_CAP}-segment cap at depth 7 \
             (dropped 1 segment(s)); reduce the structure or its depth"
        )
    );
}

/// **The message names the cap that actually bit**, not a constant.
///
/// The cap is a tier value now (Plan 0044), so the two assertions above would
/// both still pass if `Display` had gone back to reading a hardcoded 20 000 —
/// the floor's cap *is* 20 000. Formatting the same overflow at the rich cap
/// is what makes the reading non-vacuous: a reverted `Display` would print the
/// floor's number here and fail. ADR-0007 requires the surfaced cut be
/// informative, and a message quoting a cap the run was not using is worse
/// than none.
#[test]
fn the_overflow_message_names_the_cap_it_carries() {
    let rich_cap = crate::render::TierConfig::RICH.max_segments;
    assert_ne!(
        rich_cap, FLOOR_CAP,
        "the two tiers must differ for this to test anything"
    );

    let at = |cap: usize| {
        CapOverflow {
            dropped: 7,
            context: OverflowContext::Mirror(3),
            cap,
        }
        .to_string()
    };
    assert!(
        at(rich_cap).contains(&rich_cap.to_string()),
        "{}",
        at(rich_cap)
    );
    assert!(
        !at(rich_cap).contains(&FLOOR_CAP.to_string()),
        "a rich-tier overflow must not quote the floor's cap: {}",
        at(rich_cap)
    );
    assert_ne!(at(rich_cap), at(FLOOR_CAP));
}

/// Plan 0039 Phase 1 done-when 4. Replication moves geometry; it does not
/// change **topology**. A rotated or reflected copy of a joined chain is
/// still a joined chain, so every copy must carry its source's per-endpoint
/// flags through verbatim — dropping them would silently reopen the notch on
/// every mirrored copy while the un-mirrored original looked correct.
#[test]
fn the_mirror_carries_the_join_flags_through() {
    let mut single = vec![
        seg([0.1, 0.05], [0.4, 0.2]),
        seg([0.4, 0.2], [0.3, 0.5]),
        seg([0.3, 0.5], [0.6, 0.45]),
    ];
    // A three-segment chain: the interior vertices are joints, the two outer
    // ends are free — the pattern every chained producer emits.
    single[0].joined = JOINED_B;
    single[1].joined = JOINED_A | JOINED_B;
    single[2].joined = JOINED_A;
    let expected: Vec<u32> = single.iter().map(|s| s.joined).collect();

    for spec in [
        MirrorSpec {
            order: 4,
            reflect: false,
        },
        MirrorSpec {
            order: 3,
            reflect: true,
        },
    ] {
        let mut out = Vec::new();
        let dropped = replicate_mirror(&single, spec, 10_000, &mut out);
        assert_eq!(dropped, 0, "well under the cap");
        assert_eq!(out.len(), single.len() * spec.copies());
        for (i, produced) in out.iter().enumerate() {
            assert_eq!(
                produced.joined,
                expected[i % expected.len()],
                "{spec:?} copy of segment {} lost its connectivity",
                i % expected.len()
            );
        }
    }
}

/// The same claim for the generator scenes' per-frame transform: rotation,
/// scale, colour and width are this frame's styling, but connectivity
/// belongs to the cached structure and survives untouched.
#[test]
fn the_cached_transform_carries_the_join_flags_through() {
    let mut base = vec![seg([0.0, 0.0], [0.3, 0.1]), seg([0.3, 0.1], [0.5, 0.4])];
    base[0].joined = JOINED_B;
    base[1].joined = JOINED_A;

    let mut out = Vec::new();
    transform_cached(&base, 0.7, 1.4, [0.2, 0.9, 0.3], 0.02, 1.0, &mut out);
    assert_eq!(out.len(), base.len());
    for (produced, source) in out.iter().zip(&base) {
        assert_eq!(produced.joined, source.joined);
        assert_ne!(produced.color, source.color, "styling still applies");
    }
}

/// Reflection doubles the copy count and stays rotationally symmetric.
#[test]
fn reflection_doubles_the_copies() {
    let single = vec![seg([0.1, 0.2], [0.4, 0.3])];
    let mut out = Vec::new();
    replicate_mirror(
        &single,
        MirrorSpec {
            order: 5,
            reflect: true,
        },
        10_000,
        &mut out,
    );
    assert_eq!(out.len(), single.len() * 5 * 2, "rotation x reflection");
}

/// Exceeding `cap` truncates the output and reports the exact drop — the cap
/// is never a silent cut (ADR-0007).
#[test]
fn overflow_truncates_and_reports_the_drop() {
    // 100 base segments, 6-fold = 600 wanted, capped at 250 -> 350 dropped.
    let single: Vec<_> = (0..100)
        .map(|i| seg([i as f32 * 0.001, 0.1], [0.2, i as f32 * 0.001]))
        .collect();
    let mut out = Vec::new();
    let dropped = replicate_mirror(
        &single,
        MirrorSpec {
            order: 6,
            reflect: false,
        },
        250,
        &mut out,
    );
    assert_eq!(out.len(), 250, "output is truncated at the cap");
    assert_eq!(dropped, 600 - 250, "the exact drop is reported");
}
