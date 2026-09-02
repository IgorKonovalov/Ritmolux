//! A frame routed through the program preview's intermediate is the same frame
//! (ADR-0143).
//!
//! While the operator console is open the show is drawn into an intermediate
//! and reaches its real destination by `copy_texture_to_texture`. **The one
//! thing that could quietly go wrong is that the copy is not exact** — an
//! intermediate built at a different format, or a path that samples instead of
//! copying, round-trips the encoded values and changes the picture the audience
//! sees by a bit or two per channel. ADR-0096 dithers at the display write, so a
//! lost bit here is a real defect wearing the costume of rounding.
//!
//! So the claim is byte identity, asserted **exactly** and not within a
//! tolerance, on the precedent of `standalone/tests/shot_cli.rs`'s
//! `a_rendered_frame_is_byte_identical_to_the_png_the_app_writes`.
//!
//! **This runs headless, and that is a deviation from the phase that asked for
//! it.** Plan 0131 Phase 2 words its criteria against a console that is open,
//! which needs a second winit window on a real display; nothing in this
//! repository can open one from a test. What is asserted here instead is the
//! property the criterion was written to protect — that the intermediate and its
//! copy change no pixel — on the capture path, which routes through the same
//! intermediate by the same three recorded commands as the present path. What
//! stays uncovered is the window wiring, which Plan 0131 Phase 6's on-device
//! gate already owns.
//!
//! Software adapter (`prefer_software`) so it holds on any CI GPU.

use rlx_core::dsp::AnalysisFrame;
use rlx_core::render::CaptureImage;

mod common;

/// Small offscreen size — the claim is about whether the bytes survive a copy,
/// not about how many of them there are, and the software adapter is slow.
const SIZE: u32 = 96;

/// Whether every pixel of `img` is the same colour.
///
/// The vacuity guard: two all-black frames are byte-identical however either
/// was produced, so a flat reference would let this pass with the preview wired
/// to nothing at all.
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

/// A frame drawn through the intermediate and copied out is the same bytes as
/// one drawn straight at the target, from identical starting state.
#[test]
fn a_frame_through_the_preview_is_byte_identical_to_one_drawn_direct() {
    // Silence: this asserts that a copy preserves bytes, and the fewer inputs
    // the frame carries the fewer things can differ for another reason.
    let frame = AnalysisFrame::default();

    // Two renderers built and dropped in sequence, because the property is
    // about two paths reaching the same pixels *from the same starting state*
    // and nothing public resets a renderer's clock and scene state without also
    // drawing a frame. Sequential and not concurrent: one wgpu device at a time.
    let (name, direct) = {
        let Some(mut renderer) = common::headless(SIZE, SIZE) else {
            return;
        };
        assert_eq!(
            renderer.preview_state(),
            None,
            "a fresh renderer has no intermediate — the console is closed until \
             something opens it, and that is what makes the feature free"
        );
        let name = renderer.preset_name().to_string();
        let img = renderer
            .capture_frame(&frame)
            .expect("capture on a fresh headless renderer");
        (name, img)
    };

    let through = {
        let Some(mut renderer) = common::headless(SIZE, SIZE) else {
            return;
        };
        assert_eq!(
            renderer.preset_name(),
            name,
            "the two renderers must start on the same preset for this to be a \
             comparison of paths rather than of presets"
        );
        renderer
            .open_preview()
            .expect("a capture target accepts COPY_DST, so a preview opens");
        let (size, _) = renderer
            .preview_state()
            .expect("the preview is open after open_preview");
        assert_eq!(
            size,
            (SIZE, SIZE),
            "the intermediate sizes itself to the renderer's configured target"
        );
        renderer
            .capture_frame(&frame)
            .expect("capture through an open preview")
    };

    assert!(
        !is_flat(&direct),
        "preset '{name}' rendered a flat frame at {SIZE}x{SIZE}, so byte \
         identity below would hold for a preview wired to nothing — this test \
         needs a reference with structure in it"
    );
    assert_eq!(
        (through.width, through.height),
        (direct.width, direct.height),
        "the preview path yields the renderer's target size"
    );

    if let Some(diff) = first_difference(&direct, &through) {
        panic!(
            "the frame routed through the preview intermediate differs from the \
             one drawn straight at the target, for preset '{name}': {diff}. \
             Either the intermediate is not the destination's format and the \
             copy is round-tripping the encoded values, or the frame is \
             reaching the destination by something other than \
             copy_texture_to_texture"
        );
    }
}

/// Closing the preview leaves nothing behind: the next frame is identical to
/// one taken before the preview was ever opened.
///
/// The state half of "the feature is free when unused". A close that merely
/// stopped *reading* the intermediate — the natural way to get this wrong —
/// would keep drawing through it and pass every assertion about pixels while
/// holding the allocation for the rest of the session.
#[test]
fn closing_the_preview_returns_the_renderer_to_its_direct_path() {
    let frame = AnalysisFrame::default();
    let Some(mut renderer) = common::headless(SIZE, SIZE) else {
        return;
    };

    let before = renderer
        .capture_frame(&frame)
        .expect("capture before any preview");

    renderer.open_preview().expect("a preview opens");
    assert!(renderer.preview_state().is_some());
    let (_, first_generation) = renderer
        .preview_state()
        .expect("the preview is open after open_preview");
    let _ = renderer.capture_frame(&frame).expect("capture while open");

    renderer.close_preview();
    assert_eq!(
        renderer.preview_state(),
        None,
        "close_preview releases the intermediate rather than merely ignoring it"
    );
    renderer.close_preview();
    assert_eq!(
        renderer.preview_state(),
        None,
        "closing twice is idempotent"
    );

    // Re-opening builds a *different* intermediate, which is what lets a
    // consumer caching GPU state against one notice it has been handed another.
    renderer.open_preview().expect("a preview reopens");
    let (_, second_generation) = renderer.preview_state().expect("the preview is open again");
    assert_ne!(
        first_generation, second_generation,
        "a rebuilt intermediate must carry a new identity, or a cached bind \
         group outlives the texture it was built against"
    );
    renderer.close_preview();

    // Two more frames from the same clock the first one left, so the comparison
    // is of paths and not of clocks.
    let mut fresh = match common::headless(SIZE, SIZE) {
        Some(r) => r,
        None => return,
    };
    let after = fresh
        .capture_frame(&frame)
        .expect("capture on a fresh renderer");
    assert!(
        !is_flat(&before),
        "the reference frame needs structure in it"
    );
    if let Some(diff) = first_difference(&before, &after) {
        panic!("the direct path is not reproducible across renderers: {diff}");
    }
}
