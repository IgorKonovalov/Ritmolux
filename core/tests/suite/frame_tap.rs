//! The frame tap draws at the same stage every other capture path draws at
//! (Plan 0115 Phase 2).
//!
//! `Renderer::render_tapped` exists so a live source can pull frames out of the
//! engine without a window. The one thing that could quietly go wrong is that it
//! taps the *wrong stage* — reading back the composite before the tonemap, or
//! the scene before the terminal ink remap — and the picture in TouchDesigner
//! would then be plausibly wrong rather than obviously wrong.
//!
//! So the claim is byte identity against `capture_frame`, asserted **exactly**
//! and not within a tolerance, on the precedent of
//! `standalone/tests/suite/shot_cli.rs`'s
//! `a_rendered_frame_is_byte_identical_to_the_png_the_app_writes` and for its
//! reason: a tolerance would pass with the tap one stage too early, which is the
//! failure most likely to ship unnoticed.
//!
//! **Two renderers, built and dropped in sequence**, because the property is
//! about two paths reaching the same pixels *from the same starting state*, and
//! nothing public resets a renderer's clock and scene state without also drawing
//! a frame. Sequential and not concurrent: one wgpu device is alive at a time.
//!
//! Software adapter (`prefer_software`) so it holds on any CI GPU.

use rlx_core::dsp::AnalysisFrame;
use rlx_core::render::CaptureImage;

use crate::common;

/// Small offscreen size — the claim is about which stage the bytes come from,
/// not about how many of them there are, and the software adapter is slow.
const SIZE: u32 = 96;

/// The step `Renderer::capture_frame` advances the clock by before it draws:
/// `render::scenes::FALLBACK_DT`, which is crate-private, so the coupling is
/// restated here rather than imported. If that constant moves, this test fails
/// and the message below is what says why.
const CAPTURE_FRAME_DT: f32 = 1.0 / 60.0;

/// Whether every pixel of `img` is the same colour.
///
/// The vacuity guard: two all-black frames are byte-identical whatever stage
/// either was read from, so a flat reference would let this test pass with the
/// tap wired to nothing at all.
fn is_flat(img: &CaptureImage) -> bool {
    let mut pixels = img.rgba.chunks_exact(4);
    let Some(first) = pixels.next() else {
        return true;
    };
    pixels.all(|p| p == first)
}

/// The index of the first differing byte, and what the two frames hold there.
fn first_difference(a: &CaptureImage, b: &CaptureImage) -> Option<String> {
    a.rgba
        .iter()
        .zip(&b.rgba)
        .position(|(x, y)| x != y)
        .map(|i| {
            let (x, y) = (a.rgba.get(i), b.rgba.get(i));
            let (px, chan) = (i / 4, i % 4);
            let (row, col) = (px / a.width as usize, px % a.width as usize);
            format!("byte {i} (pixel {col},{row} channel {chan}): {x:?} vs {y:?}")
        })
}

/// A tapped frame and a `capture_frame` frame, taken from identical starting
/// state at the same clock, are the same bytes.
#[test]
fn a_tapped_frame_is_byte_identical_to_the_capture_at_the_same_clock() {
    // Silence: this asserts which stage the pixels come from, and the fewer
    // inputs the frame carries the fewer things can differ for another reason.
    let frame = AnalysisFrame::default();

    // A fresh headless renderer already holds the embedded roster with index 0
    // active, so both halves below start on the same preset without selecting
    // one — and the name is read out so a mismatch names the preset rather than
    // leaving it to be guessed from the pixels.
    let (name, reference) = {
        let Some(mut renderer) = common::headless(SIZE, SIZE) else {
            return;
        };
        let name = renderer.preset_name().to_string();
        let img = renderer
            .capture_frame(&frame)
            .expect("capture_frame on a fresh headless renderer");
        (name, img)
    };

    let tapped = {
        let Some(mut renderer) = common::headless(SIZE, SIZE) else {
            return;
        };
        assert_eq!(
            renderer.preset_name(),
            name,
            "the two renderers must start on the same preset for this to be a \
             comparison of paths rather than of presets"
        );
        let mut tap = renderer.open_tap();
        assert_eq!(
            tap.size(),
            (SIZE, SIZE),
            "the tap sizes itself to the renderer's configured target"
        );
        // Draw the frame, then drain the pipeline for it: the tap keeps one
        // frame in flight, so the call that draws frame 0 hands back nothing.
        assert!(
            renderer
                .render_tapped(&mut tap, &frame, CAPTURE_FRAME_DT)
                .expect("render_tapped on a fresh headless renderer")
                .is_none(),
            "the first tapped frame is one frame late"
        );
        renderer
            .drain_tap(&mut tap)
            .expect("the frame just drawn is in flight")
    };

    assert_eq!(
        (tapped.width, tapped.height),
        (reference.width, reference.height),
        "the tap yields the renderer's target size"
    );
    assert_eq!(
        tapped.rgba.len(),
        SIZE as usize * SIZE as usize * 4,
        "the tap yields tight RGBA with the row padding stripped"
    );
    assert!(
        !is_flat(&reference),
        "preset '{name}' rendered a flat frame at {SIZE}x{SIZE}, so byte \
         identity below would hold for a tap wired to nothing — this test needs \
         a reference with structure in it"
    );

    if let Some(diff) = first_difference(&reference, &tapped) {
        panic!(
            "the tapped frame differs from capture_frame's for preset '{name}' \
             at the same clock: {diff}. Either the tap reads back at a different \
             stage than capture_frame draws to, or capture_frame no longer steps \
             the clock by {CAPTURE_FRAME_DT} (render::scenes::FALLBACK_DT)"
        );
    }
}

/// **The tap publishes frame `N` while frame `N+1` is being drawn**, and the
/// first call publishes nothing at all.
///
/// Asserted on the pixels rather than on a counter: each frame is given its own
/// backdrop colour through a `bg_hue` override, so "the frame that came out is
/// the one before the frame that went in" is a fact about the bytes. A
/// counter-only version would pass against a tap that returned the *current*
/// frame and simply skipped one.
#[test]
fn the_tap_hands_back_the_previous_frame() {
    /// The two backdrops, as `bg_hue` in turns. Far apart so the two frames
    /// cannot be confused for one another at any rounding.
    const HUES: [f32; 2] = [0.0, 0.5];
    /// Bright enough that the backdrop is the dominant thing in the frame.
    const BRIGHT: f32 = 0.9;

    let frame = AnalysisFrame::default();

    // The reference: what each of the two frames looks like on its own, taken
    // through `capture_frame`, whose clock steps the same `FALLBACK_DT`.
    let references: Vec<CaptureImage> = {
        let Some(mut renderer) = common::headless(SIZE, SIZE) else {
            return;
        };
        HUES.iter()
            .map(|hue| {
                set_backdrop(&mut renderer, *hue, BRIGHT);
                renderer
                    .capture_frame(&frame)
                    .expect("capture_frame on a headless renderer")
            })
            .collect()
    };
    assert!(
        first_difference(
            references.first().expect("two references"),
            references.get(1).expect("two references"),
        )
        .is_some(),
        "the two backdrops rendered the same frame, so the order below could \
         not be read off the pixels"
    );

    let Some(mut renderer) = common::headless(SIZE, SIZE) else {
        return;
    };
    let mut tap = renderer.open_tap();

    // Frame 0 goes in and nothing comes out.
    set_backdrop(&mut renderer, HUES[0], BRIGHT);
    assert!(
        renderer
            .render_tapped(&mut tap, &frame, CAPTURE_FRAME_DT)
            .expect("render_tapped on a headless renderer")
            .is_none(),
        "the first tapped frame was published immediately; the tap is not \
         keeping a frame in flight"
    );

    // Frame 1 goes in and frame 0 comes out. The map is given the wait it
    // needs through `drain_tap`, so this is an order assertion and not a race.
    set_backdrop(&mut renderer, HUES[1], BRIGHT);
    let published = renderer
        .drain_tap(&mut tap)
        .expect("frame 0 is the frame in flight");
    if let Some(diff) = first_difference(references.first().expect("two references"), &published) {
        panic!(
            "the frame the tap published is not frame 0: {diff}. It matches \
             frame 1 instead, which means the tap is publishing the frame just \
             drawn rather than the one before it"
        );
    }
    // And it is genuinely frame 0 rather than a frame that happens to match:
    // frame 1's reference differs from what came out.
    assert!(
        first_difference(references.get(1).expect("two references"), &published).is_some(),
        "the published frame matches BOTH references, so the backdrop override \
         did not reach the tapped path and this asserts nothing"
    );
}

/// Hold a backdrop colour on the active preset, whatever it binds.
fn set_backdrop(renderer: &mut rlx_core::render::Renderer, hue: f32, bright: f32) {
    renderer
        .set_param_override("bg_hue", hue)
        .expect("`bg_hue` is in every system's vocabulary");
    renderer
        .set_param_override("bg_bright", bright)
        .expect("`bg_bright` is in every system's vocabulary");
}

/// A windowless run's frames reach the diagnostics clock, so the rate it reports
/// is the rate it draws at rather than zero — and a capture's frames do not,
/// because an offline frame has no rate to contribute.
#[test]
fn tapped_frames_feed_the_frame_clock_and_captures_do_not() {
    /// Enough deltas for a rate: the first frame only starts the chain.
    const FRAMES: u64 = 6;

    let Some(mut renderer) = common::headless(SIZE, SIZE) else {
        return;
    };
    renderer.enable_diagnostics(true);
    let frame = AnalysisFrame::default();

    for _ in 0..3 {
        renderer
            .capture_frame(&frame)
            .expect("capture_frame on a fresh headless renderer");
    }
    let after_captures = renderer.metrics();
    assert_eq!(
        (after_captures.frames_total, after_captures.fps),
        (0, 0.0),
        "capture_frame fed the frame clock; an offline frame has no rate"
    );

    let mut tap = renderer.open_tap();
    for _ in 0..FRAMES {
        let _ = renderer
            .render_tapped(&mut tap, &frame, CAPTURE_FRAME_DT)
            .expect("render_tapped on a fresh headless renderer");
    }
    let metrics = renderer.metrics();
    assert_eq!(
        metrics.frames_total,
        FRAMES - 1,
        "every tapped frame after the first should record one delta"
    );
    assert!(
        metrics.fps > 0.0 && metrics.fps.is_finite(),
        "{FRAMES} tapped frames left the reported rate at {}",
        metrics.fps
    );
    assert!(
        metrics.draw_calls > 0,
        "a tapped frame should report the passes it drew"
    );
}
