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

use lmv_core::dsp::AnalysisFrame;
use lmv_core::preset::{SystemKind, default_presets};
use lmv_core::render::{
    CaptureImage, HeadlessOptions, RenderError, Renderer,
    metrics::{frame_diff, struct_diff},
};

const SIZE: u32 = 128;
const FRAMES: u32 = 60;
/// A `struct_diff` below this flags a pair as near-duplicate geometry.
const NEAR_DUP_STRUCT: f32 = 0.08;

/// Build a headless `Renderer`, or `None` (a logged skip) when the runner
/// exposes no GPU adapter — macOS has no software Metal fallback (ADR-0016).
/// Any other build error still panics loudly.
fn headless() -> Option<Renderer> {
    match Renderer::new_headless(HeadlessOptions {
        width: SIZE,
        height: SIZE,
        prefer_software: true,
    }) {
        Ok(r) => Some(r),
        Err(RenderError::RequestAdapter(_)) => {
            eprintln!("skipped: no GPU adapter on this runner (ADR-0016)");
            None
        }
        Err(e) => panic!("headless renderer build failed: {e}"),
    }
}

/// One representative non-silent frame, shared by every capture so the only
/// variable across a family is the preset. Carries a falling band profile as
/// well as the three scalars (Plan 0034 Phase 2), so a spectrum preset draws
/// something to compare.
fn fixed_frame() -> AnalysisFrame {
    let mut frame = AnalysisFrame {
        bass: 0.6,
        mid: 0.5,
        treb: 0.6,
        onset: 0.4,
        bar: 0.25,
        ..Default::default()
    };
    let bands = frame.spectrum.len() as f32;
    for (i, band) in frame.spectrum.iter_mut().enumerate() {
        let t = i as f32 / bands;
        *band = (0.9 - 0.7 * t) * (0.75 + 0.25 * (t * 17.0).sin());
    }
    frame
}

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

#[test]
fn report_family_distinctness() {
    let Some(mut renderer) = headless() else {
        return;
    };
    let frame = fixed_frame();

    // THIS LIST IS CURATED, NOT EXHAUSTIVE. It is a plain array rather than a
    // match over `SystemKind`, so adding a scene does not force a decision here
    // — which is exactly why the reasoning has to be written down instead of
    // inferred from an absence.
    //
    // **As of Plan 0067 Phase 1c it happens to cover all nine families**, and
    // nothing about that is automatic: a new `SystemKind` will not appear here
    // and nothing will fail. If a family is ever taken back out, the mechanical
    // reason belongs in this comment.
    //
    // The report's unit is a PAIRWISE matrix within a family, so a family needs
    // at least two shipped presets before it can say anything at all. That —
    // and only that — is why `attractor`, `reaction_diffusion` and `emitter`
    // were absent from this list for so long. The premise had gone stale by a
    // wide margin before anyone re-read it: at the time they were added,
    // `attractor` had **eight** presets (28 pairs this report had never
    // measured), `reaction_diffusion` six (15 pairs) and `emitter` two (1) —
    // and `attractor` is the family with three plans of shape work behind it
    // (0057, 0059, 0063) and therefore the most likely in the library to have
    // converged. A count is a fine reason to leave a family out and a terrible
    // one to leave written down, because it stops being true silently.
    for (system, label) in [
        (SystemKind::FragmentField, "fragment_field"),
        (SystemKind::Swarm, "swarm"),
        (SystemKind::ParametricCurve, "parametric_curve"),
        (SystemKind::LSystem, "lsystem"),
        (SystemKind::StarPattern, "star_pattern"),
        (SystemKind::Spectrum, "spectrum"),
        (SystemKind::Attractor, "attractor"),
        (SystemKind::ReactionDiffusion, "reaction_diffusion"),
        (SystemKind::Emitter, "emitter"),
    ] {
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
        for i in 0..caps.len() {
            for j in (i + 1)..caps.len() {
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
    }
}
