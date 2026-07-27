//! The transient probe (Plan 0037 Phase 1, ADR-0039): `[smoothing]` is
//! observable, and an `{ attack, release }` pair measures differently from a
//! scalar one.
//!
//! Until this file existed, nothing automated could tell the two apart. Every
//! capture in the suite rides `capture_preset`, which holds ONE `AnalysisFrame`
//! for every frame it renders, so every smoother converges before the pixels are
//! read and the result is identical for any easing constant — an identity, not a
//! coincidence. Twenty presets adopted ADR-0035's pair on the strength of a human
//! watching the app.
//!
//! So this drives `capture_preset_over` with a step — silence, a held stimulus,
//! silence again — reads back every frame, and measures how many frames the
//! **frame** takes to settle each way. The fixtures are purpose-built
//! (`fixtures/easing_scalar.toml` and its twin) precisely so the measurement is
//! of the easing rather than of a scene's saturation curve; see their headers.
//!
//! What this does NOT prove: that the probe can see through a *shipped* preset's
//! visual response. It cannot, in general — the probe measures the frame, not the
//! parameter, and a preset whose response saturates reads flat regardless of its
//! easing (`rose_trails` is the worked example, documented in
//! `docs/capturing.md`). This file establishes the floor.
//!
//! Skips with no adapter per ADR-0016.

use lmv_core::dsp::AnalysisFrame;
use lmv_core::preset::Preset;
use lmv_core::render::metrics::{StepResponse, frame_diff, step_response};
use lmv_core::render::{CaptureImage, HeadlessOptions, RenderError, Renderer};

const SIZE: u32 = 96;

/// Frames of silence before the step, so the response starts settled.
const PRE: usize = 12;
/// Frames held on each side of the step. The rise and fall windows are the
/// **same length** on purpose: each is normalized against its own final frame,
/// so unequal windows would give the two directions different truncation bias
/// and a symmetric ease would not measure as symmetric. 96 frames is 1.6 s, over
/// three time constants of the slowest constant either fixture uses.
const WINDOW: usize = 96;

const SCALAR: &str = include_str!("fixtures/easing_scalar.toml");
const ASYMMETRIC: &str = include_str!("fixtures/easing_asymmetric.toml");
const SPECTRUM: &str = include_str!("fixtures/spectrum.toml");

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

fn preset(src: &str) -> Preset {
    Preset::from_toml_str(src).unwrap_or_else(|e| panic!("easing fixture parses: {e}"))
}

/// The log-band slice a `bass = 1.0` claim implies, mirroring `--report`'s own
/// stimuli. A frame claiming a band is up over 64 silent bands is not a frame any
/// audio could produce, and a `spectrum`-system preset would correctly draw
/// nothing under it.
const BASS_BANDS: std::ops::Range<usize> = 0..22;

/// The held half of the step: bass up, and the part of the band array that
/// scalar summarises lit alongside it.
fn loud() -> AnalysisFrame {
    let mut frame = AnalysisFrame {
        bass: 1.0,
        ..Default::default()
    };
    for band in frame
        .spectrum
        .iter_mut()
        .take(BASS_BANDS.end)
        .skip(BASS_BANDS.start)
    {
        *band = 1.0;
    }
    frame
}

/// `PRE` frames of silence, `WINDOW` held loud, `WINDOW` silent again.
fn step_stimulus() -> Vec<AnalysisFrame> {
    let silent = AnalysisFrame::default();
    let mut frames = vec![silent; PRE];
    frames.extend(std::iter::repeat_n(loud(), WINDOW));
    frames.extend(std::iter::repeat_n(silent, WINDOW));
    frames
}

/// Split a probe capture into its rise and fall segments. Each segment starts at
/// the last frame *before* its step, which is the settled state the response
/// departs from.
fn segments(images: &[CaptureImage]) -> (&[CaptureImage], &[CaptureImage]) {
    let rise = &images[PRE - 1..PRE + WINDOW];
    let fall = &images[PRE + WINDOW - 1..PRE + 2 * WINDOW];
    (rise, fall)
}

fn probe(r: &mut Renderer, src: &str) -> (StepResponse, Vec<CaptureImage>) {
    let p = preset(src);
    let name = p.name.clone();
    r.set_presets(vec![p]);
    let images = r
        .capture_preset_over(&name, &step_stimulus())
        .unwrap_or_else(|e| panic!("probe `{name}`: {e}"));
    assert_eq!(
        images.len(),
        PRE + 2 * WINDOW,
        "one image per stimulus frame"
    );
    let (rise, fall) = segments(&images);
    (step_response(rise, fall), images)
}

/// The two fixtures must be twins apart from their `name` and their
/// `[smoothing]` table — otherwise the probe is comparing two different scenes
/// and the asymmetry it reports means nothing. Compares the significant lines up
/// to `[smoothing]`.
#[test]
fn the_two_fixtures_differ_only_in_their_smoothing_table() {
    fn body(src: &str) -> Vec<&str> {
        src.lines()
            .map(str::trim)
            .take_while(|l| !l.starts_with("[smoothing]"))
            .filter(|l| !l.is_empty() && !l.starts_with('#') && !l.starts_with("name"))
            .collect()
    }
    let (a, b) = (body(SCALAR), body(ASYMMETRIC));
    assert!(!a.is_empty(), "the fixture body did not parse into lines");
    assert_eq!(
        a, b,
        "easing_scalar.toml and easing_asymmetric.toml have drifted apart — the \
         probe would be measuring the difference between two scenes, not between \
         two easing tables"
    );
}

#[test]
fn a_scalar_smoothing_entry_measures_symmetric_and_an_asymmetric_one_does_not() {
    let Some(mut r) = headless() else {
        return;
    };

    let (scalar, scalar_imgs) = probe(&mut r, SCALAR);
    let (asym, _) = probe(&mut r, ASYMMETRIC);
    println!(
        "scalar     rise {:>3} fall {:>3} ratio {:.2}",
        scalar.rise_frames,
        scalar.fall_frames,
        scalar.ratio()
    );
    println!(
        "asymmetric rise {:>3} fall {:>3} ratio {:.2}",
        asym.rise_frames,
        asym.fall_frames,
        asym.ratio()
    );

    // --- Non-vacuity first: a probe that saw nothing move would report 0/0 and
    // sail through every ratio assertion below. ---
    let (rise, fall) = segments(&scalar_imgs);
    let travel = frame_diff(&rise[0], &fall[0]);
    assert!(
        travel > 0.02,
        "the step barely changed the frame ({travel:.4}) — the fixture is not \
         responding to the stimulus and the ratios below are meaningless"
    );
    for (label, r) in [("scalar", scalar), ("asymmetric", asym)] {
        assert!(
            r.rise_frames > 0 && r.fall_frames > 0,
            "{label} reported no transient at all: {r:?}"
        );
        assert!(
            r.fall_frames < WINDOW as u32,
            "{label} never settled inside the {WINDOW}-frame window: {r:?} — the \
             window is too short for its release constant, so the measurement is \
             clamped rather than measured"
        );
    }

    // --- The property. A scalar entry selects one constant in both directions,
    // so its fall/rise ratio is 1.0 by construction in the parameter domain; the
    // fixture is built to keep the pixel domain close to it. The pair snaps up
    // and glides down, so its two directions are far apart. Neither bound is a
    // tuned constant: the claim is "one is symmetric, the other is not". ---
    assert!(
        (0.6..=1.6).contains(&scalar.ratio()),
        "a scalar [smoothing] entry did not measure symmetric: rise {} fall {} \
         (ratio {:.2})",
        scalar.rise_frames,
        scalar.fall_frames,
        scalar.ratio()
    );
    assert!(
        asym.ratio() > 3.0,
        "{{ attack = 0.02, release = 0.5 }} did not measure asymmetric: rise {} \
         fall {} (ratio {:.2})",
        asym.rise_frames,
        asym.fall_frames,
        asym.ratio()
    );
    // ...and the separation is the point: swap the two fixtures' tables and this
    // is the assertion that fails.
    assert!(
        asym.ratio() > scalar.ratio() * 2.5,
        "the two tables are not separated: scalar {:.2} vs asymmetric {:.2}",
        scalar.ratio(),
        asym.ratio()
    );
    // The asymmetry is in the attack, not just a slower everything: the pair
    // reaches its target far sooner than the scalar entry does.
    assert!(
        asym.rise_frames * 2 < scalar.rise_frames,
        "the pair's fast attack is not visible: {} frames against the scalar's {}",
        asym.rise_frames,
        scalar.rise_frames
    );
}

/// Plan 0037 Phase 1 done-when 5. The time-varying path must carry a populated
/// `spectrum` array alongside the scalar bands, on the same convention `--report`
/// and `reactivity.rs` use — a frame claiming `bass = 1.0` over 64 silent
/// log-bands is not a frame any audio could produce.
///
/// Proven behaviorally rather than by inspection: the `spectrum` fixture reads
/// **nothing but** the band array, so if the step stimulus dropped it the frame
/// would be identical before and after the step.
#[test]
fn the_step_stimulus_lights_the_band_array_not_only_the_scalars() {
    let Some(mut r) = headless() else {
        return;
    };
    let p = preset(SPECTRUM);
    let name = p.name.clone();
    r.set_presets(vec![p]);
    let images = r
        .capture_preset_over(&name, &step_stimulus())
        .unwrap_or_else(|e| panic!("probe `{name}`: {e}"));

    let settled_silent = &images[PRE - 1];
    let settled_loud = &images[PRE + WINDOW - 1];
    let moved = frame_diff(settled_silent, settled_loud);
    println!("spectrum fixture: silent -> loud frame_diff {moved:.4}");
    assert!(
        moved > 0.02,
        "the spectrum readout did not move under the step ({moved:.4}): the \
         time-varying stimulus is not lighting the log-band array, only the \
         scalar bands"
    );
}

/// `capture_preset_over` is the same renderer as `capture_preset`, not a second
/// path that might drift from it: holding one stimulus for N frames through the
/// new method reproduces the old method's Nth frame exactly.
#[test]
fn holding_one_stimulus_reproduces_capture_preset() {
    let Some(mut r) = headless() else {
        return;
    };
    let p = preset(SCALAR);
    let name = p.name.clone();
    r.set_presets(vec![p]);

    const N: u32 = 40;
    let held = loud();
    let over = r
        .capture_preset_over(&name, &vec![held; N as usize])
        .unwrap_or_else(|e| panic!("capture_preset_over: {e}"));
    let once = r
        .capture_preset(&name, &held, N)
        .unwrap_or_else(|e| panic!("capture_preset: {e}"));
    assert_eq!(
        over.last().map(|i| &i.rgba),
        Some(&once.rgba),
        "the time-varying path's final frame differs from capture_preset's"
    );
}
