//! Distinctness report (Plan 0013 Phase 4, ADVISORY — prints, never asserts).
//! Per family, capture every preset at one fixed frame and print two pairwise
//! matrices — pixel (`frame_diff`) and shape (`struct_diff`). Pairs whose shape
//! difference is below a small threshold are flagged as **near-duplicate
//! geometry**; the recolor case (high pixel diff, low shape diff) is the one to
//! catch. This tool only measures — redesigning too-similar presets is separate
//! content work (a Plan 0013 followup).
//!
//! Run with: `cargo test -p lmv-core --test distinctness -- --nocapture`
//!
//! **Cost, measured at Plan 0067 Phase 1c** when the family list went from six
//! to all nine: 25 captures -> 41, and the wall clock **22 s -> 41 s**
//! (interleaved runs on one machine, software adapter). That is +82 % for +64 %
//! more presets, because the two families added are the expensive ones —
//! `attractor` is a compute-particle scene and `reaction_diffusion` a
//! stateful-feedback one, and both cost more per frame than the six line and
//! fragment families that were already here.

use lmv_core::preset::{SystemKind, default_presets};
use lmv_core::render::{
    CaptureImage,
    metrics::{frame_diff, struct_diff},
};

mod common;

const SIZE: u32 = 128;
const FRAMES: u32 = 60;
/// A `struct_diff` below this flags a pair as near-duplicate geometry.
const NEAR_DUP_STRUCT: f32 = 0.08;

fn print_matrix(
    title: &str,
    caps: &[(String, CaptureImage)],
    metric: impl Fn(&CaptureImage, &CaptureImage) -> f32,
) {
    println!("  {title}");
    print!("           ");
    for (name, _) in caps {
        print!("{:>8.8} ", name);
    }
    println!();
    for (rname, ra) in caps {
        print!("  {rname:>8.8} ");
        for (_, rb) in caps {
            print!("{:>8.3} ", metric(ra, rb));
        }
        println!();
    }
}

/// The families this report covers, and the one place that list lives.
///
/// THIS LIST IS CURATED, NOT EXHAUSTIVE. It is a plain array rather than a
/// match over `SystemKind`, so adding a scene does not force a decision here
/// — which is exactly why the reasoning has to be written down instead of
/// inferred from an absence.
///
/// **As of Plan 0067 Phase 1c it happens to cover all nine families that
/// existed then**, and nothing about that is automatic: a new `SystemKind`
/// will not appear here and nothing will fail. Three shipped families —
/// `shape_field`, `warp_mesh` and `shape_collage` — are absent for exactly
/// that reason. If a family is ever taken back out, the mechanical reason
/// belongs in this comment.
///
/// The report's unit is a PAIRWISE matrix within a family, so a family needs
/// at least two shipped presets before it can say anything at all. That —
/// and only that — is why `attractor`, `reaction_diffusion` and `emitter`
/// were absent from this list for so long. The premise had gone stale by a
/// wide margin before anyone re-read it: at the time they were added,
/// `attractor` had **eight** presets (28 pairs this report had never
/// measured), `reaction_diffusion` six (15 pairs) and `emitter` two (1) —
/// and `attractor` is the family with three plans of shape work behind it
/// (0057, 0059, 0063) and therefore the most likely in the library to have
/// converged. A count is a fine reason to leave a family out and a terrible
/// one to leave written down, because it stops being true silently.
const FAMILIES: [(SystemKind, &str); 9] = [
    (SystemKind::FragmentField, "fragment_field"),
    (SystemKind::Swarm, "swarm"),
    (SystemKind::ParametricCurve, "parametric_curve"),
    (SystemKind::LSystem, "lsystem"),
    (SystemKind::StarPattern, "star_pattern"),
    (SystemKind::Spectrum, "spectrum"),
    (SystemKind::Attractor, "attractor"),
    (SystemKind::ReactionDiffusion, "reaction_diffusion"),
    (SystemKind::Emitter, "emitter"),
];

/// One family's pairwise report, as each `#[test]` below calls it.
///
/// **This sweep splits per family and is never sampled** (ADR-0157). The claim
/// is pairwise *within* a family, so it does not decompose to one preset, and
/// sampling a family to two presets would leave a single comparison — which
/// retires the check rather than narrowing it. The split is for the packing
/// reason the other sweeps split for: nextest parallelizes across tests and
/// never inside one.
///
/// **The pair count is asserted, not trusted.** A per-family fan-out can drop or
/// double a comparison silently, and a report that prints nothing wrong while
/// measuring fewer pairs than the family contains is precisely the failure no
/// reader would catch.
fn report_distinctness_within(system: SystemKind, label: &str) {
    let Some(mut renderer) = common::headless(SIZE, SIZE) else {
        return;
    };
    let frame = common::fixed_frame_spectrum();

    let names: Vec<String> = default_presets()
        .into_iter()
        .filter(|p| p.system == system)
        .map(|p| p.name)
        .collect();

    let caps: Vec<(String, CaptureImage)> = names
        .iter()
        .map(|name| {
            let img = renderer
                .capture_preset(name, &frame, FRAMES)
                .expect("capture preset");
            (name.clone(), img)
        })
        .collect();

    println!("\n=== family: {label} ({} presets) ===", caps.len());
    print_matrix("pixel (frame_diff)", &caps, frame_diff);
    print_matrix("shape (struct_diff)", &caps, struct_diff);

    let mut flagged = false;
    let mut compared = 0usize;
    for i in 0..caps.len() {
        for j in (i + 1)..caps.len() {
            compared += 1;
            let sd = struct_diff(&caps[i].1, &caps[j].1);
            let pd = frame_diff(&caps[i].1, &caps[j].1);
            if sd < NEAR_DUP_STRUCT {
                println!(
                    "  NEAR-DUP: {} ~ {}  (shape {sd:.3}, pixel {pd:.3})",
                    caps[i].0, caps[j].0
                );
                flagged = true;
            }
        }
    }
    if !flagged {
        println!("  (no near-duplicate geometry below shape {NEAR_DUP_STRUCT})");
    }

    // A family in this list with fewer than two presets compares nothing and
    // reports clean, which is the one way this advisory can go quiet without
    // anyone noticing — the same staleness the list's own comment records.
    let n = caps.len();
    assert!(
        n >= 2,
        "{label} is in this report's curated list but ships {n} preset(s), so it has no pair to \
         compare and the report says nothing about it"
    );
    assert_eq!(
        compared,
        n * (n - 1) / 2,
        "{label}: compared {compared} pairs across {n} presets, not the {} every pair of a \
         family makes — the split has dropped or doubled a comparison",
        n * (n - 1) / 2
    );
}

#[test]
fn distinctness_fragment_field() {
    report_distinctness_within(FAMILIES[0].0, FAMILIES[0].1);
}

#[test]
fn distinctness_swarm() {
    report_distinctness_within(FAMILIES[1].0, FAMILIES[1].1);
}

#[test]
fn distinctness_parametric_curve() {
    report_distinctness_within(FAMILIES[2].0, FAMILIES[2].1);
}

#[test]
fn distinctness_lsystem() {
    report_distinctness_within(FAMILIES[3].0, FAMILIES[3].1);
}

#[test]
fn distinctness_star_pattern() {
    report_distinctness_within(FAMILIES[4].0, FAMILIES[4].1);
}

#[test]
fn distinctness_spectrum() {
    report_distinctness_within(FAMILIES[5].0, FAMILIES[5].1);
}

#[test]
fn distinctness_attractor() {
    report_distinctness_within(FAMILIES[6].0, FAMILIES[6].1);
}

#[test]
fn distinctness_reaction_diffusion() {
    report_distinctness_within(FAMILIES[7].0, FAMILIES[7].1);
}

#[test]
fn distinctness_emitter() {
    report_distinctness_within(FAMILIES[8].0, FAMILIES[8].1);
}
