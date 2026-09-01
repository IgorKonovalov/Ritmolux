// Test asserts index the produced Vec; allowed here over the file's hot-path
// pragma since test code is not the render path.
#![allow(clippy::indexing_slicing)]

use super::*;

/// The cap these tests run at — the floor tier's, which is the value they were
/// written against and the one every shipped preset is authored and gated on
/// (Plan 0044).
const FLOOR_CAP: usize = crate::render::TierConfig::FLOOR.max_segments;

/// The half-width [`seg`] builds at, and so the extension a joined end carries
/// in these tests.
const W: f32 = 0.01;

fn seg(a: [f32; 2], b: [f32; 2]) -> SegmentInstance {
    SegmentInstance {
        a,
        b,
        color: [0.4, 0.7, 1.0],
        width: W,
        alpha: 1.0,
        ext_a: 0.0,
        ext_b: 0.0,
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
/// extensions through verbatim — dropping them would silently reopen the notch
/// on every mirrored copy while the un-mirrored original looked correct.
///
/// Verbatim is right *here* because `replicate_mirror` does not restyle: it
/// places a copy at scale `1.0` and leaves `width` alone, so a length measured
/// against that width is still measured against it. The sibling test below is
/// the case where width moves and the extension has to move with it.
#[test]
fn the_mirror_carries_the_end_extensions_through() {
    let mut single = vec![
        seg([0.1, 0.05], [0.4, 0.2]),
        seg([0.4, 0.2], [0.3, 0.5]),
        seg([0.3, 0.5], [0.6, 0.45]),
    ];
    // A three-segment chain: the interior vertices are joints, the two outer
    // ends are free — the pattern every chained producer emits.
    single[0].ext_b = W;
    single[1].ext_a = W;
    single[1].ext_b = W;
    single[2].ext_a = W;
    let expected: Vec<(f32, f32)> = single.iter().map(|s| (s.ext_a, s.ext_b)).collect();

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
                (produced.ext_a, produced.ext_b),
                expected[i % expected.len()],
                "{spec:?} copy of segment {} lost its connectivity",
                i % expected.len()
            );
        }
    }
}

/// The same claim for the generator scenes' per-frame transform — with the one
/// difference a *length* makes that a flag did not.
///
/// Rotation, scale and colour are this frame's styling and connectivity belongs
/// to the cached structure, so which ends are extended survives untouched. But
/// `styled` also stamps this frame's half-width onto a figure cached at a
/// placeholder one, and an extension is measured in those units (ADR-0158): it
/// has to be **carried to the new width**, not passed through. A flag needed no
/// such thing, which is exactly why this is worth pinning — the failure is
/// invisible until `thickness` moves, and then every joint on the two cached
/// scenes is extended by the wrong length.
#[test]
fn the_cached_transform_rescales_the_end_extensions_to_this_frames_width() {
    let mut base = vec![seg([0.0, 0.0], [0.3, 0.1]), seg([0.3, 0.1], [0.5, 0.4])];
    // `seg` is built at width `W`, and a joined end extends by its own
    // half-width — the Phase 1 plumbing value.
    base[0].ext_b = W;
    base[1].ext_a = W;

    let frame_width = 4.0 * W;
    let mut out = Vec::new();
    transform_cached(&base, 0.7, 1.4, [0.2, 0.9, 0.3], frame_width, 1.0, &mut out);
    assert_eq!(out.len(), base.len());
    for (produced, source) in out.iter().zip(&base) {
        assert_eq!(produced.width, frame_width, "styling still applies");
        assert_ne!(produced.color, source.color, "styling still applies");
        // Which ends are extended is unchanged; how far is this frame's.
        assert_eq!(
            (produced.ext_a > 0.0, produced.ext_b > 0.0),
            (source.ext_a > 0.0, source.ext_b > 0.0),
            "the transform must not change which ends are joints"
        );
        assert!(
            (produced.ext_a - source.ext_a * 4.0).abs() < 1e-9,
            "ext_a rode the width ratio: {} is not 4x {}",
            produced.ext_a,
            source.ext_a
        );
        assert!(
            (produced.ext_b - source.ext_b * 4.0).abs() < 1e-9,
            "ext_b rode the width ratio: {} is not 4x {}",
            produced.ext_b,
            source.ext_b
        );
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

// -----------------------------------------------------------------------
// The stroke floor's dead zone (Plan 0087 Phase 1b, design-backlog 0098)
// -----------------------------------------------------------------------

/// A `parametric_curve` preset drawing a static rose at `thickness`, with every
/// other key frozen. Deliberately the same figure the golden fixture draws, so
/// the only thing varying across the captures below is the stroke.
fn thickness_preset(thickness: f32) -> String {
    format!(
        r#"
system = "parametric_curve"
name   = "thickness_{thickness}"

[curve]
family = "maurer_rose"

[params]
n             = "6"
d             = "71"
samples       = "361"
scale         = "0.8"
spin          = "0"
hue           = "0.55"
thickness     = "{thickness}"
brightness    = "0.9"
draw_progress = "1"
"#
    )
}

/// Capture that preset headless, on the software adapter.
///
/// Builds and drops **one** renderer per call rather than holding three: a
/// second live device in a binary is what the software adapter falls over on,
/// and building GPU resources mid-run shifts what a later stage resolves to on
/// WARP.
fn capture_at_thickness(thickness: f32) -> Option<crate::render::CaptureImage> {
    use crate::dsp::AnalysisFrame;
    use crate::preset::Preset;
    use crate::render::context::RenderError;
    use crate::render::{HeadlessOptions, Renderer};

    let mut renderer = match Renderer::new_headless(HeadlessOptions {
        width: 256,
        height: 256,
        prefer_software: true,
    }) {
        Ok(renderer) => renderer,
        Err(RenderError::RequestAdapter(_)) => {
            eprintln!("skipped: no GPU adapter on this runner (ADR-0016)");
            return None;
        }
        Err(e) => panic!("headless renderer build failed: {e}"),
    };
    let preset =
        Preset::from_toml_str(&thickness_preset(thickness)).expect("the thickness fixture parses");
    let name = preset.name.clone();
    renderer.set_presets(vec![preset]);
    Some(
        renderer
            .capture_preset(&name, &AnalysisFrame::default(), 4)
            .expect("capture the thickness fixture"),
    )
}

/// **Below [`MIN_USEFUL_THICKNESS`] every value draws the identical picture** —
/// which is what makes the range dead, and why re-tuning inside it disproves
/// the correct hypothesis.
///
/// `fragment_vitrail` shipped at `0.016`; the content lane swept it to `0.022`
/// and `0.038` looking for a change, saw none, discarded the thickness
/// hypothesis as disproved and went on to sweep chord count and sample count.
/// All three clamp to [`MIN_HALF_WIDTH`].
///
/// **Both halves matter.** Without the second — a value above the threshold
/// drawing something *different* — this test would pass just as happily on a
/// renderer that ignored `thickness` altogether.
#[test]
fn every_sub_floor_thickness_renders_identically_and_a_useful_one_does_not() {
    // The two historical values from the `fragment_vitrail` sweep, and one the
    // library actually ships at. Read out of `let`s so the straddle is checked
    // against the constant rather than restated as a comment.
    let (low_t, higher_t, useful_t) = (0.016f32, 0.038f32, 1.8f32);
    let floor = MIN_USEFUL_THICKNESS;
    assert!(
        low_t < floor && higher_t < floor && useful_t > floor,
        "the fixtures must straddle {floor}, or this test compares nothing"
    );

    let Some(low) = capture_at_thickness(low_t) else {
        return;
    };
    let Some(higher) = capture_at_thickness(higher_t) else {
        return;
    };
    let Some(useful) = capture_at_thickness(useful_t) else {
        return;
    };

    // Non-vacuity: the figure has to be on screen, or "identical" is two black
    // frames agreeing.
    let lit =
        |img: &crate::render::CaptureImage| img.rgba.chunks_exact(4).filter(|p| p[0] > 8).count();
    eprintln!(
        "thickness captures: {low_t} lights {} px, {higher_t} lights {} px, \n         {useful_t} lights {} px",
        lit(&low),
        lit(&higher),
        lit(&useful)
    );
    assert!(
        lit(&low) > 0 && lit(&useful) > 0,
        "one of the captures drew nothing"
    );

    // The dead zone: byte-for-byte, not within a tolerance. Two values that
    // clamp to the same half-width feed the identical geometry to the identical
    // pipeline, so anything other than equality would be a different defect.
    assert_eq!(
        low.rgba, higher.rgba,
        "thickness 0.016 and 0.038 drew different pictures — they clamp to the \
         same half-width, so the dead zone this warns about is not where the \
         warning says it is"
    );

    // And the other side of it, or the assertion above is satisfied by a
    // renderer that never reads `thickness` at all.
    assert_ne!(
        low.rgba, useful.rgba,
        "thickness 0.016 and 1.8 drew the same picture — `thickness` is not \
         reaching the stroke, and the equality above proves nothing"
    );
}
