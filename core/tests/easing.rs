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
use lmv_core::preset::{Easing, Preset};
use lmv_core::render::metrics::{StepResponse, frame_diff, frames_to_settle, step_response};
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

// ---------------------------------------------------------------------------
// Plan 0038 Phase 3 done-when 6 — ADR-0040's curve-vs-easing ordering, measured
// at the pixel level rather than argued.
//
// The unit test in the scene pins that the ordering is IMPLEMENTED as specified.
// It cannot show that the chosen order produces the motion the ADR claims, which
// is a statement about a decaying element on screen. That needs the probe.
//
// The trick that makes "both ways round" measurable without shipping a second
// ordering: with `[spectrum] smoothing` absent the scene's easing is INSTANT, so
// the drawn length is `curve(input)` and nothing else. Pre-ease the stimulus on
// the CPU and that same scene draws `curve(ease(raw))` — the rejected order —
// through the identical renderer, shader and metric as the engine's own
// `ease(curve(raw))`. The control test below proves the construction is exact.
// ---------------------------------------------------------------------------

const CURVE_EASED: &str = include_str!("fixtures/spectrum_curve_eased.toml");
const CURVE_INSTANT: &str = include_str!("fixtures/spectrum_curve_instant.toml");

/// The constants the eased arm's `[spectrum] smoothing` declares, mirrored here
/// so the instant arm's stimulus can be pre-eased with the same envelope.
const PROBE_EASING: Easing = Easing {
    attack: 0.02,
    release: 0.5,
};

/// The clock `capture_preset_over` advances per stimulus frame — `FALLBACK_DT`,
/// which is `pub(crate)` and so not nameable from an integration test.
///
/// **Not trusted on faith:** `at_curve_one_the_pre_eased_arm_matches_the_engine`
/// is its drift guard. At `curve = 1.0` the two orders are algebraically
/// identical, so the two arms must agree frame for frame — which they cannot if
/// this number stops matching the engine's.
const DT: f32 = 1.0 / 60.0;

/// The `curve` line both probe fixtures carry, and the handle the control arm
/// uses to build its `curve = 1.0` variant.
const CURVE_LINE: &str = "curve      = \"0.5\"";

/// Re-ease a stimulus on the CPU with [`PROBE_EASING`], band by band.
///
/// Easing the **bands** and letting the scene downsample is exactly equal to the
/// scene easing the **downsampled levels**, which is what makes this arm a fair
/// comparison rather than an approximation: `downsample` is a mean, the one-pole
/// step is linear in `(raw - held)`, and every band inside one element moves in
/// the same direction under this stimulus, so all of them select the same
/// constant. Mean-then-ease and ease-then-mean therefore coincide.
fn pre_eased(stimulus: &[AnalysisFrame]) -> Vec<AnalysisFrame> {
    let mut bands = [0.0f32; lmv_core::dsp::SPECTRUM_BINS];
    let mut bass = 0.0f32;
    stimulus
        .iter()
        .map(|frame| {
            for (held, &raw) in bands.iter_mut().zip(frame.spectrum.iter()) {
                *held = PROBE_EASING.step(*held, raw, DT);
            }
            bass = PROBE_EASING.step(bass, frame.bass, DT);
            let mut out = *frame;
            out.spectrum = bands;
            out.bass = bass;
            out
        })
        .collect()
}

fn capture_arm(r: &mut Renderer, src: &str, stimulus: &[AnalysisFrame]) -> Vec<CaptureImage> {
    let p = preset(src);
    let name = p.name.clone();
    r.set_presets(vec![p]);
    r.capture_preset_over(&name, stimulus)
        .unwrap_or_else(|e| panic!("probe `{name}`: {e}"))
}

/// How **even** a fall is: the share of its settling time spent covering the
/// first half of its travel.
///
/// This is the quantity ADR-0040 actually argues about — "a perceptually even
/// fall" against "a fast start and a long crawl" — and `StepResponse` alone
/// cannot express it, because a response can settle quickly and still be
/// lopsided. A perfectly linear ramp reads `0.5/0.9 = 0.56`; a pure exponential
/// reads `ln 2 / ln 10 = 0.30`. Higher is more even.
fn fall_evenness(fall: &[CaptureImage]) -> f32 {
    frames_to_settle(fall, 0.5) as f32 / frames_to_settle(fall, 0.9).max(1) as f32
}

/// The two arms must be twins apart from `name` and the `smoothing` key —
/// otherwise the probe is comparing two scenes, not two orderings.
#[test]
fn the_two_curve_arms_differ_only_in_their_spectrum_smoothing() {
    fn body(src: &str) -> Vec<&str> {
        src.lines()
            .map(str::trim)
            .filter(|l| {
                !l.is_empty()
                    && !l.starts_with('#')
                    && !l.starts_with("name")
                    && !l.starts_with("smoothing")
            })
            .collect()
    }
    let (a, b) = (body(CURVE_EASED), body(CURVE_INSTANT));
    assert!(!a.is_empty(), "the fixture body did not parse into lines");
    assert_eq!(
        a, b,
        "the curve probe fixtures have drifted apart — the measurement would be \
         of the difference between two scenes, not between two orderings"
    );
    for (label, src) in [("eased", CURVE_EASED), ("instant", CURVE_INSTANT)] {
        assert!(
            src.contains(CURVE_LINE),
            "the {label} arm no longer carries `{CURVE_LINE}`, so the curve arms \
             would be measuring a linear readout and agree vacuously"
        );
    }
    // Against the comment-stripped body, not the raw text: both files *mention*
    // smoothing in their headers, and only one may declare it.
    let declares = |src: &str| {
        src.lines()
            .map(str::trim)
            .any(|l| !l.starts_with('#') && l.starts_with("smoothing"))
    };
    assert!(
        declares(CURVE_EASED) && !declares(CURVE_INSTANT),
        "the eased arm must declare [spectrum] smoothing and the instant arm \
         must not — that difference is the whole experiment"
    );
}

/// The control, and the drift guard on [`DT`] and on [`pre_eased`] alike.
///
/// At `curve = 1.0` the exponent is the identity, so `ease(curve(raw))` and
/// `curve(ease(raw))` are the *same function*. The two arms must therefore land
/// on identical pixels, every frame. If `FALLBACK_DT` changes, if `pre_eased`
/// stops mirroring the scene's smoother, or if the mean/ease commutation
/// argument is wrong, this is what fails — before the measurement below is
/// allowed to mean anything.
#[test]
fn at_curve_one_the_pre_eased_arm_matches_the_engine() {
    let Some(mut r) = headless() else {
        return;
    };
    let linear = |src: &str| src.replace(CURVE_LINE, "curve      = \"1.0\"");

    let raw = step_stimulus();
    let engine = capture_arm(&mut r, &linear(CURVE_EASED), &raw);
    let rebuilt = capture_arm(&mut r, &linear(CURVE_INSTANT), &pre_eased(&raw));

    // Non-vacuity: a pair of all-black captures would match trivially.
    let (rise, _) = segments(&engine);
    let travel = frame_diff(&rise[0], rise.last().unwrap_or(&rise[0]));
    assert!(
        travel > 0.02,
        "the probe fixture barely moved under the step ({travel:.4}) — the \
         agreement below would be between two static frames"
    );

    let mismatched = engine
        .iter()
        .zip(&rebuilt)
        .enumerate()
        .find(|(_, (a, b))| a.rgba != b.rgba);
    assert!(
        mismatched.is_none(),
        "at curve = 1.0 the CPU-eased arm diverged from the engine's own easing \
         at frame {:?} — DT or `pre_eased` no longer mirrors the scene, so the \
         ordering measurement would be meaningless",
        mismatched.map(|(i, _)| i)
    );
}

/// Plan 0038 Phase 3 done-when 6. Measure the fall **both ways round** under a
/// non-unit `curve` and record what each does.
///
/// **No threshold is asserted on the ordering claim** — this plan has not earned
/// one, and inventing a number here is the Plan 0033 mistake. The test asserts
/// only that the measurement is non-vacuous and that the two orders are
/// genuinely distinguishable at the pixel level; the numbers are printed for the
/// commit body and for ADR-0040 to be judged against.
#[test]
fn the_two_curve_orderings_measure_differently_in_the_frame() {
    let Some(mut r) = headless() else {
        return;
    };
    let raw = step_stimulus();
    let curve_then_ease = capture_arm(&mut r, CURVE_EASED, &raw);
    let ease_then_curve = capture_arm(&mut r, CURVE_INSTANT, &pre_eased(&raw));

    let mut reported = Vec::new();
    for (label, images) in [
        ("curve-then-ease (ADR-0040)", &curve_then_ease),
        ("ease-then-curve (rejected)", &ease_then_curve),
    ] {
        let (rise, fall) = segments(images);
        let response = step_response(rise, fall);
        let evenness = fall_evenness(fall);
        println!(
            "{label:<28} rise {:>3} fall {:>3} ratio {:>5.2} fall-evenness {evenness:.3}",
            response.rise_frames,
            response.fall_frames,
            response.ratio()
        );
        assert!(
            response.rise_frames > 0 && response.fall_frames > 0,
            "{label} reported no transient at all: {response:?} — the arm is not \
             responding to the stimulus"
        );
        assert!(
            response.fall_frames < WINDOW as u32,
            "{label} never settled inside the {WINDOW}-frame window: {response:?} \
             — the measurement is clamped rather than measured"
        );
        reported.push((label, response, evenness));
    }

    // The one property that must hold for the experiment to have run at all: a
    // non-unit curve makes the two orders visibly different. If they measured
    // the same, either arm could be mislabelled and nothing above would notice.
    let (a, b) = (reported[0].1, reported[1].1);
    assert_ne!(
        (a.fall_frames, reported[0].2.to_bits()),
        (b.fall_frames, reported[1].2.to_bits()),
        "the two orderings measured identically, so this probe cannot tell them \
         apart and its numbers say nothing about ADR-0040"
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
