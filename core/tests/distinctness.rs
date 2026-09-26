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
/// The label is the system's canonical name, read from [`SystemKind::as_str`],
/// so a test cannot report one family under another's name.
fn report_distinctness_within(system: SystemKind) {
    let label = system.as_str();

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

    // A family with fewer than two presets compares nothing and reports clean,
    // which is the one way this advisory can go quiet without anyone noticing.
    let n = caps.len();
    assert!(
        n >= 2,
        "{label} ships {n} preset(s), so it has no pair to compare and the report says nothing \
         about it"
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

/// One `#[test]` per family, and an exhaustive match over the same list.
///
/// The roster is every [`SystemKind`] — ADR-0234. nextest needs a named test per
/// family to run them in parallel, so the names are written here, and the match
/// the macro emits over the same variants is what keeps that list whole: a
/// `SystemKind` variant with no line below is a non-exhaustive-match compile
/// error naming the missing variant, never a family the report silently skips.
macro_rules! family_tests {
    ($($test:ident => $kind:ident),* $(,)?) => {
        $(
            #[test]
            fn $test() {
                report_distinctness_within(SystemKind::$kind);
            }
        )*

        const _: fn(SystemKind) = |system| match system {
            $(SystemKind::$kind => (),)*
        };
    };
}

family_tests! {
    distinctness_fragment_field => FragmentField,
    distinctness_swarm => Swarm,
    distinctness_parametric_curve => ParametricCurve,
    distinctness_lsystem => LSystem,
    distinctness_star_pattern => StarPattern,
    distinctness_reaction_diffusion => ReactionDiffusion,
    distinctness_attractor => Attractor,
    distinctness_spectrum => Spectrum,
    distinctness_emitter => Emitter,
    distinctness_shape_field => ShapeField,
    distinctness_warp_mesh => WarpMesh,
    distinctness_shape_collage => ShapeCollage,
    distinctness_analytic_field => AnalyticField,
    distinctness_cellular => Cellular,
}
