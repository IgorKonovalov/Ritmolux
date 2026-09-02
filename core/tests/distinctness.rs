//! Distinctness report (Plan 0013 Phase 4, ADVISORY — prints, never asserts).
//! Per family, capture every preset at one fixed frame and print two pairwise
//! matrices — pixel (`frame_diff`) and shape (`struct_diff`). Pairs whose shape
//! difference is below a small threshold are flagged as **near-duplicate
//! geometry**; the recolor case (high pixel diff, low shape diff) is the one to
//! catch. This tool only measures — redesigning too-similar presets is separate
//! content work (a Plan 0013 followup).
//!
//! Run with: `cargo test -p rlx-core --test distinctness -- --nocapture`
//!
//! **Cost, measured at Plan 0067 Phase 1c** when the family list went from six
//! to all nine: 25 captures -> 41, and the wall clock **22 s -> 41 s**
//! (interleaved runs on one machine, software adapter). That is +82 % for +64 %
//! more presets, because the two families added are the expensive ones —
//! `attractor` is a compute-particle scene and `reaction_diffusion` a
//! stateful-feedback one, and both cost more per frame than the six line and
//! fragment families that were already here.

use rlx_core::preset::{SystemKind, default_presets};
use rlx_core::render::{
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
/// **The pair count is asserted against the library, not against the loop.** A
/// per-family fan-out can drop or double a comparison silently, and a report
/// that prints nothing wrong while measuring fewer pairs than the family
/// contains is precisely the failure no reader would catch — so the count below
/// is re-derived from [`default_presets`] rather than from the loop that
/// produced it.
///
/// Each `#[test]` passes its family as a literal rather than by index into
/// [`FAMILIES`], so reordering that array cannot leave a test running one family
/// under another's name; membership in the curated list is then checked here.
fn report_distinctness_within(system: SystemKind, label: &str) {
    assert!(
        FAMILIES.contains(&(system, label)),
        "{label} is not in this report's curated list, so either the list lost an entry that \
         still has a test or a test's (SystemKind, label) pair has drifted apart"
    );

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
    // The pair count is checked against the LIBRARY's own membership, not
    // against the loop that just ran. `compared` is incremented once per
    // iteration of that loop, so comparing it against `n(n-1)/2` for
    // `n = caps.len()` is an arithmetic identity and cannot fail whatever the
    // fan-out did. Re-counting the family from `default_presets()` puts the two
    // sides of the comparison in different places, which is the only shape in
    // which this says anything: a family whose captures went missing between the
    // filter and the matrix fails, and so does a test handed a `SystemKind` that
    // does not match the label it reports under.
    let shipped = default_presets()
        .into_iter()
        .filter(|p| p.system == system)
        .count();
    assert_eq!(
        compared,
        shipped * (shipped - 1) / 2,
        "{label}: compared {compared} pairs, but the shipped set holds {shipped} preset(s) in \
         this family and every pair of them is {}. The fan-out has dropped or doubled a \
         comparison, or this test is running a family other than the one it is named for",
        shipped * (shipped - 1) / 2
    );
}

#[test]
fn distinctness_fragment_field() {
    report_distinctness_within(SystemKind::FragmentField, "fragment_field");
}

#[test]
fn distinctness_swarm() {
    report_distinctness_within(SystemKind::Swarm, "swarm");
}

#[test]
fn distinctness_parametric_curve() {
    report_distinctness_within(SystemKind::ParametricCurve, "parametric_curve");
}

#[test]
fn distinctness_lsystem() {
    report_distinctness_within(SystemKind::LSystem, "lsystem");
}

#[test]
fn distinctness_star_pattern() {
    report_distinctness_within(SystemKind::StarPattern, "star_pattern");
}

#[test]
fn distinctness_spectrum() {
    report_distinctness_within(SystemKind::Spectrum, "spectrum");
}

#[test]
fn distinctness_attractor() {
    report_distinctness_within(SystemKind::Attractor, "attractor");
}

#[test]
fn distinctness_reaction_diffusion() {
    report_distinctness_within(SystemKind::ReactionDiffusion, "reaction_diffusion");
}

#[test]
fn distinctness_emitter() {
    report_distinctness_within(SystemKind::Emitter, "emitter");
}

/// **Growing [`FAMILIES`] must force a decision about a test to run it.**
///
/// The fan-out here is hand-written, one `#[test]` per curated family, so a
/// tenth entry in that array would otherwise be measured by nothing and report
/// nothing — the same silent-absence failure the array's own doc comment
/// describes for a new `SystemKind`. This pins the count so the array and the
/// roster of tests below it cannot drift apart unnoticed. Adding a family means
/// adding its `#[test]` and moving this number, in one commit.
#[test]
fn every_curated_family_has_its_own_test() {
    assert_eq!(
        FAMILIES.len(),
        9,
        "the curated family list changed size; add or remove the matching #[test] below it, \
         then move this number"
    );
}
