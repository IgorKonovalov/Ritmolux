//! **The in-frame geometry fraction** (Plan 0069, [ADR-0083]) — the share of
//! drawn segment length that lands inside the render target, measured at
//! `LineRenderer::draw` and covering the four line-family scenes
//! (`parametric_curve`, `lsystem`, `star_pattern`, `spectrum`) with one
//! implementation.
//!
//! Pixel coverage cannot see an over-scaled figure. A comb roots every bar on a
//! shared baseline and a corona roots every spoke at a centre, so clipping the
//! tips costs a rounding error of lit pixels — Plan 0058 measured it, and the
//! two defective presets scored **above** the legitimate content, which is why
//! that plan shipped a report rather than a gate. This file is the successor
//! measure, and it is deliberately narrow: it says nothing about
//! `fragment_field`, `reaction_diffusion`, `attractor`, `swarm` or `emitter`,
//! which build no segment list and keep pixel coverage.
//!
//! First duty: [`the_diagnostic_changes_nothing_about_the_picture`] — the switch
//! is inert, asserted as **byte-identical** captures rather than a tolerance.
//! ADR-0083's Negative section asks for exactly this: a second code path through
//! the hot draw call, and the only way to know it changed nothing is to assert
//! it. Get that wrong and the instrument measures itself.
//!
//! Software adapter (`prefer_software`) so it holds on any CI GPU, and every
//! capture in a duty comes from **one** renderer: a second
//! `Renderer::new_headless` in this binary would be a second GPU resource build
//! mid-run, which `composite.rs` documents as changing what the software adapter
//! resolves.
//!
//! [ADR-0083]: ../../docs/adrs/0083-in-frame-geometry-is-measured-at-the-line-renderers-draw-seam.md

use lmv_core::{
    dsp::AnalysisFrame,
    preset::Preset,
    render::{
        HeadlessOptions, RenderError, Renderer,
        scenes::lines::renderer::{set_extent_diagnostic, take_draw_extent},
    },
};

/// Capture size. Nothing here reads a pixel except the byte-identity check, and
/// that one only needs two frames of the same thing, so this is small on
/// purpose. **16:9 rather than square**: the measured rectangle is
/// `[-aspect, aspect] x [-1, 1]`, so a square frame would measure a shape no
/// shipped preset is ever seen in.
const WIDTH: u32 = 160;
const HEIGHT: u32 = 90;

/// Frames per capture — enough to clear a draw-in and settle any smoothing, at
/// the cost the sweep below pays 15 times over.
const FRAMES: u32 = 30;

/// A headless renderer, or `None` (a logged skip) when the runner exposes no GPU
/// adapter — macOS has no software Metal fallback (ADR-0016).
fn headless() -> Option<Renderer> {
    match Renderer::new_headless(HeadlessOptions {
        width: WIDTH,
        height: HEIGHT,
        prefer_software: true,
    }) {
        Ok(renderer) => Some(renderer),
        Err(RenderError::RequestAdapter(_)) => {
            eprintln!("skipped: no GPU adapter on this runner (ADR-0016)");
            None
        }
        Err(e) => panic!("headless renderer build failed: {e}"),
    }
}

/// The line fixture the inertness check captures, borrowed read-only from the
/// golden roster: a static 6-petal Maurer rose. Nothing here pins its pixels or
/// depends on its numbers — it is used because it is a frozen figure that draws
/// a few hundred segments through the renderer under test, and any edit to it
/// leaves this file's claim (two captures of the *same* fixture agree) intact.
const LINE_FIXTURE: &str = include_str!("fixtures/parametric_curve.toml");

/// A representative mid-energy frame with the band array populated, so a
/// band-reactive line scene draws a real figure rather than nothing.
fn lit_frame() -> AnalysisFrame {
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
        *band = 1.0 - (i as f32 / bands) * 0.8;
    }
    frame
}

/// **The diagnostic changes nothing about the picture** (Plan 0069 Phase 2).
///
/// The measurement is a per-frame CPU loop over the segment list on the draw
/// path, and the argument for admitting it at all is that it is off in the
/// shipped path and touches no GPU resource when on. That argument is only worth
/// what an assertion makes it worth.
///
/// **Byte-identical, not within a tolerance.** A tolerance would pass a
/// diagnostic that perturbed the instance buffer by a rounding error, and the
/// claim is stronger than that: the measurement reads the segment slice and
/// writes to a thread-local, so the bytes uploaded and the draw call issued are
/// the same ones either way. A tolerance here would be an admission that nobody
/// knows what the switch does.
///
/// Both arms are non-vacuous by assertion: with the diagnostic off nothing is
/// recorded *at all* (distinct from a recorded zero), and with it on a real
/// figure of positive length is measured. Without those, a switch wired to
/// nothing would pass this test trivially.
#[test]
fn the_diagnostic_changes_nothing_about_the_picture() {
    let Some(mut renderer) = headless() else {
        return;
    };
    let preset = Preset::from_toml_str(LINE_FIXTURE).expect("the line fixture parses");
    let name = preset.name.clone();
    renderer.set_presets(vec![preset]);
    let frame = lit_frame();

    // --- Off: the shipped path. ---
    set_extent_diagnostic(false);
    let dark = renderer
        .capture_preset(&name, &frame, FRAMES)
        .expect("capture with the diagnostic off");
    assert_eq!(
        take_draw_extent(),
        None,
        "the diagnostic recorded a measurement while switched off"
    );

    // --- On: the same scene, the same renderer, the same frame. ---
    set_extent_diagnostic(true);
    let measured = renderer
        .capture_preset(&name, &frame, FRAMES)
        .expect("capture with the diagnostic on");
    let extent = take_draw_extent().expect("the diagnostic measured the last draw");
    set_extent_diagnostic(false);

    assert!(
        extent.total_len > 0.0,
        "the fixture drew no segment length ({extent:?}), so the on arm measured \
         nothing and this comparison is two pictures of the same nothing"
    );
    eprintln!(
        "diagnostic on: total_len {:.4}, in_frame_len {:.4}, fraction {:?}",
        extent.total_len,
        extent.in_frame_len,
        extent.fraction()
    );

    assert_eq!(
        (dark.width, dark.height),
        (measured.width, measured.height),
        "the two captures differ in size"
    );
    let differing = dark
        .rgba
        .iter()
        .zip(&measured.rgba)
        .filter(|(a, b)| a != b)
        .count();
    assert_eq!(
        differing,
        0,
        "{differing} of {} bytes differ between the capture with the in-frame \
         geometry diagnostic off and the one with it on. The measurement reads \
         the segment slice and writes a thread-local; it must not reach the \
         instance buffer, the uniform, or the draw call",
        dark.rgba.len()
    );
}
