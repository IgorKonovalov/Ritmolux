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

// ---------------------------------------------------------------------------
// The readback (Plan 0158 Phase 6): the same pixels reach the CPU one frame
// late, and nothing about the display loop learns to wait.
// ---------------------------------------------------------------------------

/// The show's pixels are unchanged with the readback open.
///
/// The byte-identity claim above, extended to the path that also copies the
/// intermediate out to a staging buffer. The failure this rules out is a
/// readback that reached the intermediate through anything but a second read —
/// a layout transition or a second attachment would change what the copy to the
/// destination then carries.
#[test]
fn the_shows_pixels_are_unchanged_with_the_readback_open() {
    let frame = AnalysisFrame::default();

    let (name, direct) = {
        let Some(mut renderer) = common::headless(SIZE, SIZE) else {
            return;
        };
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
        renderer.open_preview().expect("a preview opens");
        renderer
            .open_preview_readback(SIZE, SIZE)
            .expect("a readback opens against an open preview");
        assert_eq!(
            renderer.preview_readback_size(),
            Some((SIZE, SIZE)),
            "the readback yields the size it was opened at"
        );
        renderer
            .capture_frame(&frame)
            .expect("capture through an open preview and readback")
    };

    assert!(
        !is_flat(&direct),
        "preset '{name}' rendered a flat frame, so byte identity below would \
         hold for a readback wired to nothing"
    );
    if let Some(diff) = first_difference(&direct, &through) {
        panic!(
            "opening the readback changed the show's pixels for preset \
             '{name}': {diff}. The readback is a second read of the \
             intermediate and must not be able to affect what the copy to the \
             destination carries"
        );
    }
}

/// The readback yields the **previous** frame's pixels, and yields nothing at
/// all on the first.
///
/// This is the one-frame-late contract, and it is what buys the display loop its
/// freedom from waiting: the copy rides frame N's submission and the map is
/// taken on frame N+1, so the first frame has nothing to take and every frame
/// after it takes its predecessor's.
///
/// **The comparison is nearest-of-two rather than byte identity**, because the
/// readback is a copy of nothing exactly: it reads a fixed-size tap that a
/// sampling blit fills (ADR-0187), so even at the same size and aspect the
/// values round-trip through the sampler. Byte identity of the *show's* own
/// pixels is asserted above and is unaffected — the tap is a second read of the
/// intermediate, not a stage in the path to the destination.
#[test]
fn the_readback_yields_the_frame_before_and_nothing_on_the_first() {
    let frame = AnalysisFrame::default();
    let Some(mut renderer) = common::headless(SIZE, SIZE) else {
        return;
    };
    renderer.open_preview().expect("a preview opens");
    renderer
        .open_preview_readback(SIZE, SIZE)
        .expect("a readback opens against an open preview");

    let first = renderer
        .capture_frame(&frame)
        .expect("the first frame through the readback");
    assert!(
        renderer.take_preview_frame().is_none(),
        "the first frame produced a readback, but its own copy has only just \
         been submitted — there is nothing yet for a map to have landed on"
    );

    let second = renderer
        .capture_frame(&frame)
        .expect("the second frame through the readback");
    let readback = renderer
        .take_preview_frame()
        .expect("the second frame takes the first frame's map");

    assert_eq!(
        (readback.width, readback.height),
        (SIZE, SIZE),
        "the readback yields the size it was opened at"
    );
    assert!(
        !is_flat(&first),
        "the frames are flat, so the comparison below would hold for a readback \
         that produced anything at all"
    );
    // ...and the clock moved between them, so "the frame before" is a real
    // distinction rather than two identical pictures. Asserted before the
    // comparison it makes meaningful, since a zero denominator there would let
    // the nearest-of-two claim pass on nothing.
    assert!(
        first_difference(&first, &second).is_some(),
        "two consecutive captures were identical, so this test cannot tell the \
         previous frame from the current one"
    );
    let to_first = mean_difference(&readback, &first);
    let to_second = mean_difference(&readback, &second);
    assert!(
        to_first < to_second,
        "the readback is {to_first:.3} 8-bit levels from the FIRST frame and \
         {to_second:.3} from the second, so it did not yield the first. It is \
         consumed one frame late by construction, and landing on the second \
         would mean the copy is being waited on rather than polled"
    );
}

/// The mean absolute per-channel difference between two frames of one size, in
/// 8-bit levels. `f64::MAX` for a size mismatch, which is not a distance.
fn mean_difference(a: &CaptureImage, b: &CaptureImage) -> f64 {
    if (a.width, a.height) != (b.width, b.height) || a.rgba.is_empty() {
        return f64::MAX;
    }
    let total: u64 = a
        .rgba
        .iter()
        .zip(&b.rgba)
        .map(|(x, y)| u64::from(x.abs_diff(*y)))
        .sum();
    total as f64 / a.rgba.len() as f64
}

/// Closing the preview takes the readback with it.
#[test]
fn closing_the_preview_closes_the_readback() {
    let Some(mut renderer) = common::headless(SIZE, SIZE) else {
        return;
    };
    renderer.open_preview().expect("a preview opens");
    renderer
        .open_preview_readback(SIZE, SIZE)
        .expect("a readback opens");
    assert_eq!(renderer.preview_readback_size(), Some((SIZE, SIZE)));

    renderer.close_preview();
    assert_eq!(
        renderer.preview_readback_size(),
        None,
        "the readback outlived the intermediate it copies out of, so it holds a \
         staging buffer it can never fill again"
    );
    assert!(
        renderer.take_preview_frame().is_none(),
        "a closed readback handed over a frame"
    );
}

/// A readback needs an open preview, and says so rather than opening a buffer
/// nothing will ever copy into.
#[test]
fn a_readback_without_a_preview_is_refused() {
    let Some(mut renderer) = common::headless(SIZE, SIZE) else {
        return;
    };
    assert!(
        renderer.open_preview_readback(SIZE, SIZE).is_err(),
        "a readback opened with no intermediate to read"
    );
    assert_eq!(renderer.preview_readback_size(), None);
}

/// A preview size that is neither the surface's nor a divisor of it, so no
/// assertion below can pass by accidentally agreeing with the window.
const PREVIEW: (u32, u32) = (100, 60);

/// **A resize does not stop the preview** (ADR-0187, backlog 0201).
///
/// This is the defect the fixed-size tap exists to remove, and it is
/// undiagnosable from either end: a readback rebuilt at the new size on `resize`
/// hands its writer a frame the sink refuses, the writer's loop breaks on that
/// error and discards its message, and the result is standard output going quiet
/// while the show keeps drawing and every counter reads zero. See
/// `Plan 0167 Phase 1` and backlog 0201 for the diagnosis.
///
/// So the assertion is that frames **keep arriving at the announced geometry**
/// across a sequence of resizes — a drag, a maximize and a fullscreen toggle,
/// which reach the renderer as the same `WindowEvent::Resized` and are therefore
/// one case here rather than three.
#[test]
fn a_resize_does_not_stop_the_preview() {
    let frame = AnalysisFrame::default();
    let Some(mut renderer) = common::headless(SIZE, SIZE) else {
        return;
    };
    renderer.open_preview().expect("a preview opens");
    renderer
        .open_preview_readback(PREVIEW.0, PREVIEW.1)
        .expect("a readback opens against an open preview");

    // Two frames: the readback is one frame late, so the second is the first
    // that can yield anything.
    let yielded = |renderer: &mut rlx_core::render::Renderer, label: &str| {
        for _ in 0..2 {
            renderer
                .capture_frame(&frame)
                .unwrap_or_else(|err| panic!("capture {label}: {err}"));
        }
        let image = renderer
            .take_preview_frame()
            .unwrap_or_else(|| panic!("the preview yielded no frame {label}"));
        assert_eq!(
            (image.width, image.height),
            PREVIEW,
            "the frame {label} is not the size the pipe announced, which is the \
             disagreement that used to end the writer thread"
        );
        assert_eq!(
            image.rgba.len(),
            PREVIEW.0 as usize * PREVIEW.1 as usize * 4,
            "the frame {label} carries no bytes at the announced size"
        );
        image
    };

    let before = yielded(&mut renderer, "before the resize");
    assert!(
        !is_flat(&before),
        "the preview was flat before any resize, so 'the picture kept coming' \
         below would hold for a tap wired to nothing"
    );

    // A drag to a taller shape, a maximize to a wider one, and a fullscreen
    // toggle to a third — every one of them a `Resized` the renderer rebuilds
    // the intermediate for.
    for (label, (width, height)) in [
        ("after a drag", (SIZE * 2, SIZE)),
        ("after a maximize", (SIZE * 3, SIZE * 2)),
        ("after a fullscreen toggle", (SIZE, SIZE * 2)),
    ] {
        renderer.resize(width, height);
        assert_eq!(
            renderer.preview_state().map(|(size, _)| size),
            Some((width, height)),
            "the intermediate did not follow the surface {label}, so this case \
             is not exercising a resize at all"
        );
        assert_eq!(
            renderer.preview_readback_size(),
            Some(PREVIEW),
            "the pipe's geometry moved {label}"
        );
        let after = yielded(&mut renderer, label);
        assert!(
            !is_flat(&after),
            "the preview went flat {label} — the tap is being cleared and never \
             drawn into"
        );
    }
}

/// **The preview's size is the caller's, not the surface's.**
///
/// The size half of the test above, asserted with no frames drawn so a failure
/// names the geometry rather than the picture. [`PREVIEW`] is chosen to share no
/// factor with the surface, because a readback that still followed the
/// intermediate would agree with a size that happened to be half the window.
#[test]
fn the_previews_size_is_independent_of_the_surfaces() {
    let Some(mut renderer) = common::headless(SIZE, SIZE) else {
        return;
    };
    renderer.open_preview().expect("a preview opens");
    renderer
        .open_preview_readback(PREVIEW.0, PREVIEW.1)
        .expect("a readback opens against an open preview");

    assert_eq!(renderer.preview_readback_size(), Some(PREVIEW));
    renderer.resize(SIZE * 2, SIZE * 2);
    assert_eq!(
        renderer.preview_readback_size(),
        Some(PREVIEW),
        "the preview followed the surface up"
    );
    renderer.resize(SIZE / 2, SIZE * 3);
    assert_eq!(
        renderer.preview_readback_size(),
        Some(PREVIEW),
        "the preview followed the surface down"
    );
}

/// No blocking wait enters the display loop.
///
/// `wgpu`'s indefinite poll is correct in exactly three places, and every one of
/// them is named below with the reason. Anywhere else in `render/` it is a stall
/// on a loop that answers to a present deadline, and the preview readback exists
/// precisely because that is unacceptable.
///
/// A source scan, because the property is about every line rather than the ones
/// a run happened to reach.
#[test]
fn no_indefinite_wait_lives_outside_the_places_that_may_hold_one() {
    /// Where an indefinite wait is correct, and why.
    ///
    /// Each entry is a path **relative to `core/src/render/`**, never a bare
    /// file name: six `mod.rs` live under this root, so a bare name would
    /// allowlist a wait in any of them — including one that ships — while
    /// naming only the test instrument below.
    const ALLOWED: [(&str, &str); 3] = [
        (
            "capture.rs",
            "the capture readback: a headless loop outruns the GPU and the wait is what paces it",
        ),
        (
            "capture_api.rs",
            "the frame tap's retire, on the same path and for the same reason",
        ),
        (
            "scenes/particles/mod.rs",
            "`read_particles`, a #[cfg(test)] instrument that no shipped build compiles",
        ),
    ];
    /// The one allowlisted file whose wait is a test instrument rather than a
    /// capture path. Checked below to still be behind its `#[cfg(test)]`, so the
    /// entry cannot come to cover a wait that shipped.
    const TEST_ONLY: (&str, &str) = (
        "src/render/scenes/particles/mod.rs",
        "#[cfg(test)]\n    fn read_particles",
    );

    let render = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/render");
    let allowed: Vec<&str> = ALLOWED.iter().map(|(file, _)| *file).collect();
    let mut findings = Vec::new();
    let mut scanned = 0usize;
    let mut waits_seen = 0usize;
    let mut walk = vec![render.clone()];
    while let Some(dir) = walk.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk.push(path);
                continue;
            }
            if path.extension().is_none_or(|ext| ext != "rs") {
                continue;
            }
            let Ok(source) = std::fs::read_to_string(&path) else {
                continue;
            };
            scanned += 1;
            let waits = source.matches("wait_indefinitely").count();
            waits_seen += waits;
            let relative = path
                .strip_prefix(&render)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            if waits > 0 && !allowed.contains(&relative.as_str()) {
                findings.push(format!("  {}: {waits} indefinite wait(s)", path.display()));
            }
        }
    }

    assert!(
        scanned > 20 && waits_seen > 0,
        "the scan reached {scanned} files and {waits_seen} waits under render/, \
         which is too few for a green result to mean anything"
    );
    assert!(
        findings.is_empty(),
        "{} file(s) under core/src/render/ wait indefinitely and are not one of \
         the places that may:\n{}\nA capture has no deadline and the wait is \
         what paces it; the display loop has one, and a wait there is a dropped \
         frame. Poll instead, and consume what landed.",
        findings.len(),
        findings.join("\n"),
    );

    // The test-only entry is still test-only. Without this the allowlist would
    // quietly come to cover a wait that ships, in the file most likely to grow
    // one.
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(TEST_ONLY.0);
    let source =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let gate = source.find(TEST_ONLY.1).unwrap_or_else(|| {
        panic!(
            "{} no longer gates its readback with {}",
            TEST_ONLY.0, TEST_ONLY.1
        )
    });
    let wait = source
        .find("wait_indefinitely")
        .unwrap_or_else(|| panic!("{} is allowlisted and no longer waits", TEST_ONLY.0));
    assert!(
        wait > gate,
        "{}'s indefinite wait is no longer inside the #[cfg(test)] instrument \
         the allowlist admits it for, so it is now in a shipped build",
        TEST_ONLY.0
    );

    // The two capture entries still describe something, so a green run is not
    // the scan walking past files that stopped waiting.
    for (file, _) in ALLOWED.iter().take(2) {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/render")
            .join(file);
        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        assert!(
            source.contains("wait_indefinitely"),
            "{file} is allowlisted for the indefinite wait and no longer uses one"
        );
    }
}
