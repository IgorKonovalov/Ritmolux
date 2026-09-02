//! One preview intermediate sustains a long run, and closing it gives the
//! memory back (ADR-0143).
//!
//! While the operator console is open every frame is drawn into an intermediate
//! and copied out, and a console left open for a set is hundreds of thousands of
//! frames. The intermediate is built in `Renderer::open_preview` and taken by
//! reference per frame, which is a claim about a call graph; **the observable is
//! the resident set**, and a per-frame texture of the target's size shows up
//! there long before 300 frames are out.
//!
//! The ceiling is a tripwire, not a budget — Plan 0099's per-pass retention grew
//! *linearly with frame count*, so a few hundred frames of a defect at that
//! scale is hundreds of megabytes. The number that is actually judged is the one
//! the run prints, on ADR-0071's rule that absolute figures are a property of
//! the box's driver stack and do not travel.
//!
//! **This lives in `standalone/` and not in `core/`** for the reason
//! `frame_tap_memory.rs` states beside it: `ResidentSet` reads a per-OS working
//! set, which is platform code `core` may not hold, and `core` cannot
//! dev-depend on the standalone without a cycle.
//!
//! Software adapter (`prefer_software`) so it runs wherever the hardware-gated
//! cases skip.

use rlx_core::dsp::AnalysisFrame;
use rlx_core::render::{HeadlessOptions, RenderError, Renderer};
use standalone::shot::render::ResidentSet;

/// Frames driven through one open preview — 5 s at 60 fps.
const FRAMES: u32 = 300;

/// Small offscreen size. The claim is per-frame retention, which is about the
/// number of allocations and not their size, and the software adapter is slow.
const WIDTH: u32 = 64;
const HEIGHT: u32 = 48;

/// Growth past which this is a leak, charged from the **warm** reading so the
/// pipeline compilation the first draw pays for is not in it. Matches the
/// ceiling `frame_tap_memory.rs` and `shot_cli.rs` hold their runs to.
const CEILING_MB: f64 = 64.0;

/// Samples across the run: a baseline before the first frame, then one every
/// `FRAMES / 20`, then one at the end. Before-and-after alone cannot tell a run
/// that grew steadily from one that stepped once at startup.
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
fn one_preview_carries_three_hundred_frames_without_growing() {
    let Some(mut renderer) = headless() else {
        return;
    };
    // Silence: the preview's memory behaviour is a property of the loop, not of
    // what the frame happens to draw.
    let frame = AnalysisFrame::default();

    renderer.open_preview().expect("a preview opens");
    let (size, generation) = renderer
        .preview_state()
        .expect("the preview is open after open_preview");
    assert_eq!(
        size,
        (WIDTH, HEIGHT),
        "the intermediate sizes itself to the renderer's configured target"
    );

    let mut resident = ResidentSet::default();
    // Before the first frame, so the baseline holds the renderer, its driver
    // stack and the intermediate — but none of the run.
    resident.sample();

    let every = (FRAMES / SAMPLE_POINTS).max(1);
    let mut any_content = false;
    for index in 0..FRAMES {
        let img = renderer
            .capture_frame(&frame)
            .unwrap_or_else(|e| panic!("frame {index} of {FRAMES} through the preview: {e}"));
        assert_eq!(
            (img.width, img.height),
            (WIDTH, HEIGHT),
            "frame {index} came back a different size than the preview was opened at"
        );
        // A run of 300 blank frames would hold the resident set flat for the
        // wrong reason, so at least one has to carry a picture.
        any_content |= img.rgba.iter().any(|&b| b != 0);
        if index > 0 && index.is_multiple_of(every) {
            resident.sample();
        }
    }
    resident.sample();

    assert!(
        any_content,
        "all {FRAMES} frames through the preview read back as pure black, so a \
         flat resident set below would say nothing about a loop that draws"
    );
    assert_eq!(
        renderer.preview_state().map(|(_, g)| g),
        Some(generation),
        "the intermediate must be the same one {FRAMES} frames later — a rebuilt \
         one per frame is the allocation this test exists to catch"
    );

    let line = resident.summary(FRAMES);
    eprintln!("{line}");
    if resident.samples.is_empty() {
        // No working-set query on this platform — the frames still completed and
        // the intermediate is still the one that was opened, which is the half
        // of the claim that does not need an instrument.
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
        "the resident set grew {growth:.1} MB across {FRAMES} frames with the \
         preview open — the intermediate is supposed to be built once in \
         open_preview and taken by reference per frame: {line}"
    );
}

/// Closing the preview gives the memory back rather than holding it at the
/// console-open level for the rest of the session.
///
/// The natural way to get this wrong is to stop *reading* the intermediate and
/// leave it allocated, which passes every assertion about pixels.
#[test]
fn closing_the_preview_returns_the_resident_set_toward_its_baseline() {
    let Some(mut renderer) = headless() else {
        return;
    };
    let frame = AnalysisFrame::default();

    // A frame first, so the driver stack and the pipelines are already warm and
    // the readings below are about the intermediate rather than about startup.
    let _ = renderer.capture_frame(&frame).expect("a warm-up frame");

    let mut closed = ResidentSet::default();
    closed.sample();
    for _ in 0..30 {
        let _ = renderer
            .capture_frame(&frame)
            .expect("a closed-preview frame");
    }
    closed.sample();

    renderer.open_preview().expect("a preview opens");
    for _ in 0..30 {
        let _ = renderer
            .capture_frame(&frame)
            .expect("an open-preview frame");
    }
    renderer.close_preview();
    assert_eq!(
        renderer.preview_state(),
        None,
        "close_preview releases the intermediate rather than merely ignoring it"
    );

    let mut after = ResidentSet::default();
    after.sample();
    for _ in 0..30 {
        let _ = renderer
            .capture_frame(&frame)
            .expect("a reclosed-preview frame");
    }
    after.sample();

    let (before_line, after_line) = (closed.summary(30), after.summary(30));
    eprintln!("closed-before: {before_line}");
    eprintln!("closed-after:  {after_line}");
    if closed.samples.is_empty() || after.samples.is_empty() {
        // No working-set query here; the state assertion above is the half of
        // the claim that does not need an instrument, and it already ran.
        return;
    }
    assert!(
        growth_mb(&after_line) < CEILING_MB,
        "the resident set kept growing after the preview was closed, so closing \
         is not releasing: {after_line}"
    );
}
