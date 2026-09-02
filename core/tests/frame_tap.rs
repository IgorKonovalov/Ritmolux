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
//! `standalone/tests/shot_cli.rs`'s
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

mod common;

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
        renderer
            .render_tapped(&mut tap, &frame, CAPTURE_FRAME_DT)
            .expect("render_tapped on a fresh headless renderer")
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
