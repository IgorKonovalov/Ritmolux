//! One frame tap sustains a long run without growing (Plan 0115 Phase 2).
//!
//! `render_tapped` exists to be called hundreds of thousands of times — a
//! four-hour source at 60 fps is 864,000 frames — so the thing that has to hold
//! is that the loop allocates nothing per frame. The target, its view and the
//! readback buffer are built in `Renderer::open_tap` and taken by `&mut` on
//! every call, which is a claim about a call graph; **the observable is the
//! resident set**, and a per-frame GPU allocation of any size shows up there
//! long before 300 frames are out.
//!
//! Three hundred frames rather than the 864,000 the mode is for: Plan 0099's
//! per-pass retention grew *linearly with frame count*, so a few hundred frames
//! of a defect at that scale is hundreds of megabytes and a ceiling catches it,
//! while a four-hour run belongs in Phase 6's hand-run measurement. The ceiling
//! is a tripwire, not a budget — the number that is actually judged is the one
//! the run prints, on ADR-0071's rule that absolute figures are a property of
//! the box's driver stack and do not travel.
//!
//! **This test lives in `standalone/` and not in `core/`** because `ResidentSet`
//! and the per-OS working-set read underneath it do — `standalone/src/rss.rs` is
//! platform code, which is exactly what `core` may not hold, and `core` cannot
//! dev-depend on the standalone without a cycle. Plan 0115 Phase 2's file list
//! put it in `core/tests/`; the byte-identity half is there
//! (`core/tests/frame_tap.rs`) and this half is here, so the helper is reused
//! rather than written a second time.
//!
//! Software adapter (`prefer_software`) so it runs wherever `shot`'s
//! hardware-gated cases skip.

use lmv_core::dsp::AnalysisFrame;
use lmv_core::render::{HeadlessOptions, RenderError, Renderer};
use standalone::shot::render::ResidentSet;

/// Frames driven through one tap — 5 s at 60 fps.
const FRAMES: u32 = 300;

/// Small offscreen size. The claim is per-frame retention, which is about the
/// number of allocations and not their size, and the software adapter is slow.
const WIDTH: u32 = 64;
const HEIGHT: u32 = 48;

/// A plausible live cadence. Nothing here depends on its value — the tap takes
/// `dt` per call precisely so the caller may vary it — but a run at a real frame
/// interval exercises the scene advance the stream mode will drive.
const DT: f32 = 1.0 / 60.0;

/// Growth past which this is a leak, charged from the **warm** reading so the
/// pipeline compilation the first draw pays for is not in it. Sixty-four
/// megabytes matches the ceiling `shot_cli.rs` holds the render mode to, and is
/// generous against a run that should be flat.
const CEILING_MB: f64 = 64.0;

/// Samples across the run, matching the render mode's own cadence: a baseline
/// before the first frame, then one every `FRAMES / 20`, then one at the end.
/// Before-and-after alone cannot tell a run that grew steadily from one that
/// stepped once at startup.
const SAMPLE_POINTS: u32 = 20;

fn headless() -> Option<Renderer> {
    match Renderer::new_headless(HeadlessOptions {
        width: WIDTH,
        height: HEIGHT,
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

/// The signed megabyte figure out of a `ResidentSet::summary` line.
fn growth_mb(line: &str) -> f64 {
    line.split("growth ")
        .nth(1)
        .and_then(|rest| rest.split(" MB").next())
        .and_then(|mb| mb.parse().ok())
        .unwrap_or_else(|| panic!("no growth figure in: {line}"))
}

#[test]
fn one_tap_renders_three_hundred_consecutive_frames_without_growing() {
    let Some(mut renderer) = headless() else {
        return;
    };
    // Silence: the tap's memory behaviour is a property of the loop, not of what
    // the frame happens to draw.
    let frame = AnalysisFrame::default();

    let mut tap = renderer.open_tap();
    assert_eq!(
        tap.size(),
        (WIDTH, HEIGHT),
        "the tap sizes itself to the renderer's configured target"
    );

    let mut resident = ResidentSet::default();
    // Before the first frame, so the baseline holds the renderer and its driver
    // stack but none of the run.
    resident.sample();

    let every = (FRAMES / SAMPLE_POINTS).max(1);
    let mut drawn = 0u32;
    let mut any_content = false;
    for index in 0..FRAMES {
        let img = renderer
            .render_tapped(&mut tap, &frame, DT)
            .unwrap_or_else(|e| panic!("frame {index} of {FRAMES} through the tap: {e}"));
        assert_eq!(
            (img.width, img.height),
            (WIDTH, HEIGHT),
            "frame {index} came back a different size than the tap was opened at"
        );
        assert_eq!(
            img.rgba.len(),
            WIDTH as usize * HEIGHT as usize * 4,
            "frame {index} is not tight RGBA"
        );
        // A run of 300 blank readbacks would hold the resident set flat for the
        // wrong reason, so at least one frame has to carry a picture.
        any_content |= img.rgba.iter().any(|&b| b != 0);
        drawn += 1;
        if index > 0 && index.is_multiple_of(every) {
            resident.sample();
        }
    }
    resident.sample();

    assert_eq!(drawn, FRAMES, "every frame of the run completed");
    assert!(
        any_content,
        "all {FRAMES} tapped frames read back as pure black, so a flat resident \
         set below would say nothing about a loop that draws"
    );

    let line = resident.summary(FRAMES);
    eprintln!("{line}");
    if resident.samples.is_empty() {
        // No working-set query on this platform — the frames still completed,
        // which is the half of the claim that does not need an instrument.
        return;
    }
    assert!(
        resident.samples.len() > 10,
        "the resident set was sampled {} times across {FRAMES} frames, which \
         cannot separate steady growth from a single startup step: {line}",
        resident.samples.len()
    );

    let growth = growth_mb(&line);
    assert!(
        growth < CEILING_MB,
        "the resident set grew {growth:.1} MB across {FRAMES} tapped frames — a \
         tap that allocates per frame is a source that dies partway through a \
         set (NFR 12), and the target and readback buffer are supposed to be \
         built once in open_tap: {line}"
    );
}
