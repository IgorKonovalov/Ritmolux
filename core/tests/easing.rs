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

use rlx_core::dsp::AnalysisFrame;
use rlx_core::preset::{Easing, Preset};
use rlx_core::render::metrics::{
    StepResponse, frame_diff, frames_to_settle, segment_settled, step_response,
};

mod common;

/// The evenness a pure exponential fall reads, and therefore what a one-pole
/// measures whichever side of it a `curve` sits on: `ln 2 / ln 10`, which is
/// `log10(2)` and so already in `std` rather than spelled out as a literal.
const LN2_OVER_LN10: f32 = std::f32::consts::LOG10_2;
use rlx_core::render::{CaptureImage, Renderer};

const SIZE: u32 = 96;

/// Frames of silence before the step, so the response starts settled.
const PRE: usize = 12;
/// Frames held on each side of the step. The rise and fall windows are the
/// **same length** on purpose: each is normalized against its own final frame,
/// so unequal windows would give the two directions different truncation bias.
///
/// **180 frames is 3 s, and the previous 96 was not enough** (Plan 0038 Phase 8).
/// The slowest constant either fixture uses is `easing_asymmetric`'s
/// `release = 0.5`, and 1.6 s of window is only 3.2 τ — a **4.1 %** residual,
/// twice [`SETTLE_TOL`]. That truncation was live: the asymmetric fall measured
/// **61** frames where the settled answer is `-0.5·ln(0.1)` = 1.151 s ≈ **69**,
/// which is the number its own fixture header predicts. 3 s is 6 τ, a 0.25 %
/// residual — the margin the scalar arm (`0.25`, 12 τ) always had.
///
/// Do not shorten this without checking `segment_settled` still passes
/// on both arms; nothing else here would tell you.
const WINDOW: usize = 180;

/// Fraction of a segment's travel that may remain unfinished at its last frame
/// before the measurement is refused (Plan 0038 Phase 7). Two percent is well
/// inside the pixel quantum at these sizes and far outside the ~20-45 %
/// truncation that produced Phase 3's original numbers.
///
/// **Gates every probe in this file**, not just the curve one — Phase 8 found
/// the shared probe above was itself truncated, and nothing in the file could
/// have said so.
const SETTLE_TOL: f32 = 0.02;

const SCALAR: &str = include_str!("fixtures/easing_scalar.toml");
const ASYMMETRIC: &str = include_str!("fixtures/easing_asymmetric.toml");
const SPECTRUM: &str = include_str!("fixtures/spectrum.toml");

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
    let Some(mut r) = common::headless(SIZE, SIZE) else {
        return;
    };

    let (scalar, scalar_imgs) = probe(&mut r, SCALAR);
    let (asym, asym_imgs) = probe(&mut r, ASYMMETRIC);
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
    for (label, r, imgs) in [
        ("scalar", scalar, &scalar_imgs),
        ("asymmetric", asym, &asym_imgs),
    ] {
        assert!(
            r.rise_frames > 0 && r.fall_frames > 0,
            "{label} reported no transient at all: {r:?}"
        );
        // Plan 0038 Phase 8. This replaces a `fall_frames < WINDOW` guard that
        // could never fire: `frames_to_settle` normalizes against the segment's
        // own last frame, so it always answers inside the segment and that
        // comparison was a tautology (Phase 7). The real question is whether the
        // last frame had arrived anywhere, and only `segment_settled` asks it.
        let (rise, fall) = segments(imgs);
        for (dir, seg) in [("rise", rise), ("fall", fall)] {
            assert!(
                segment_settled(seg, SETTLE_TOL),
                "{label} {dir}: still travelling at the end of its {WINDOW}-frame \
                 window, so `frames_to_settle` normalized against a moving target \
                 and returned a plausible but short count ({r:?}). Widen `WINDOW` \
                 — the numbers below are not measurements until this passes"
            );
        }
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
    let Some(mut r) = common::headless(SIZE, SIZE) else {
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
    let mut bands = [0.0f32; rlx_core::dsp::SPECTRUM_BINS];
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
/// A perfectly linear ramp reads `0.5/0.9 = 0.56`; a pure exponential reads
/// `ln 2 / ln 10 = 0.30`. Higher is more even.
///
/// **Only meaningful on a segment that actually settled** (Plan 0038 Phase 7).
/// Both frame counts are normalized against the segment's last frame, so on a
/// truncated window the 0.9 threshold is pulled in harder than the 0.5 one and
/// the ratio climbs — which reads exactly like a more even fall and is not one.
/// Gate on [`segment_settled`] before believing a number from here.
fn fall_evenness(fall: &[CaptureImage]) -> f32 {
    frames_to_settle(fall, 0.5) as f32 / frames_to_settle(fall, 0.9).max(1) as f32
}

/// Frames the **curve** probe holds each side of its step, replacing the shared
/// `PRE`/`WINDOW` for that measurement only (Plan 0038 Phase 7).
///
/// The rejected arm's effective time constant is `release / curve` = 1.0 s, so
/// the shared 96-frame (1.6 s) window leaves a fifth of its travel undone and
/// `frames_to_settle` answers with a plausible wrong number rather than a
/// failure. 600 frames is 10 s — ten of those time constants.
///
/// The two windows are deliberately **unequal**, which `step_response`'s contract
/// warns against. That rule exists so truncation bias cancels between the two
/// directions; once both segments are measured to settlement there is no bias
/// left to cancel, and asserting settlement is strictly stronger than balancing
/// two errors against each other.
const CURVE_PRE: usize = 12;
const CURVE_RISE: usize = 96;
const CURVE_FALL: usize = 600;

/// `CURVE_PRE` frames of silence, `CURVE_RISE` held loud, `CURVE_FALL` silent.
fn curve_stimulus() -> Vec<AnalysisFrame> {
    let silent = AnalysisFrame::default();
    let mut frames = vec![silent; CURVE_PRE];
    frames.extend(std::iter::repeat_n(loud(), CURVE_RISE));
    frames.extend(std::iter::repeat_n(silent, CURVE_FALL));
    frames
}

/// Split a [`curve_stimulus`] capture, each segment starting at the last frame
/// *before* its step — the settled state the response departs from.
fn curve_segments(images: &[CaptureImage]) -> (&[CaptureImage], &[CaptureImage]) {
    let rise = &images[CURVE_PRE - 1..CURVE_PRE + CURVE_RISE];
    let fall = &images[CURVE_PRE + CURVE_RISE - 1..];
    (rise, fall)
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
    let Some(mut r) = common::headless(SIZE, SIZE) else {
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

/// Plan 0038 Phase 3 done-when 6, **re-measured under Phase 7's fixed instrument**.
///
/// The original run of this test read the two orderings as differing in the
/// *shape* of their fall, which is what routed ADR-0040 back to `architect`. Half
/// of that was right and half was the instrument: the rejected arm never settled
/// inside the window it was measured in, so its numbers came from a truncated
/// normalization. See the ADR's Outcome. What survives, and what this now asserts:
///
/// - both falls have the **same shape** — for a step to silence each is an
///   exponential in the displayed level, so their evenness agrees and neither is
///   the "even fall" ADR-0040 originally claimed for the shipped order;
/// - they differ in **effective speed**, the rejected arm taking about `1/curve`
///   times as long, because curving after the smoother stretches the decay.
#[test]
fn the_two_curve_orderings_differ_in_speed_and_not_in_shape() {
    let Some(mut r) = common::headless(SIZE, SIZE) else {
        return;
    };
    let raw = curve_stimulus();
    let curve_then_ease = capture_arm(&mut r, CURVE_EASED, &raw);
    let ease_then_curve = capture_arm(&mut r, CURVE_INSTANT, &pre_eased(&raw));

    let mut reported = Vec::new();
    for (label, images) in [
        ("curve-then-ease (ADR-0040)", &curve_then_ease),
        ("ease-then-curve (rejected)", &ease_then_curve),
    ] {
        let (rise, fall) = curve_segments(images);
        // The gate Phase 7 exists for, and it runs BEFORE any number below is
        // read. `frames_to_settle` cannot fail on a truncated window — it always
        // answers inside the segment — so without this the numbers look fine.
        assert!(
            segment_settled(fall, SETTLE_TOL),
            "{label}: the fall had not settled by the end of its {CURVE_FALL}-frame \
             window, so every frame count below would be normalized against a \
             moving target — widen the window rather than reading the numbers"
        );
        let response = step_response(rise, fall);
        let evenness = fall_evenness(fall);
        println!(
            "{label:<28} rise {:>3} fall {:>3} fall-evenness {evenness:.3}",
            response.rise_frames, response.fall_frames
        );
        assert!(
            response.rise_frames > 0 && response.fall_frames > 0,
            "{label} reported no transient at all: {response:?} — the arm is not \
             responding to the stimulus"
        );
        reported.push((label, response, evenness));
    }

    let (a, b) = (reported[0].1, reported[1].1);
    let (even_a, even_b) = (reported[0].2, reported[1].2);

    // Same shape. Both are exponentials, so both sit at ln2/ln10 = 0.301 and,
    // more to the point, at the SAME value as each other. The tolerance is earned
    // from the measurement's own granularity: evenness is a ratio of two integer
    // frame counts, so at the shipped arm's ~70-frame fall one frame of rounding
    // is already ~0.015, and 0.05 is a few frames of slack rather than a fitted
    // number.
    assert!(
        (even_a - even_b).abs() <= 0.05,
        "the two orderings measured different fall SHAPES ({even_a:.3} vs \
         {even_b:.3}) — the closed form says both are exponentials of identical \
         shape, so this is either a truncated window that slipped the gate above \
         or a real finding that routes to architect"
    );
    assert!(
        (even_a - LN2_OVER_LN10).abs() <= 0.05,
        "the shipped arm's fall measured {even_a:.3}, not the pure-exponential \
         {LN2_OVER_LN10:.3} — a one-pole should produce nothing else"
    );

    // Different speed, by about 1/curve. The fixtures carry curve = 0.5, so the
    // rejected arm's decay is stretched twofold.
    let speedup = b.fall_frames as f32 / a.fall_frames.max(1) as f32;
    println!("fall-length ratio (rejected / shipped) {speedup:.2}, expected ~2.0");
    assert!(
        (1.7..=2.3).contains(&speedup),
        "the rejected ordering's fall should run about 1/curve = 2.0x the \
         shipped one's; measured {speedup:.2} from {} and {} frames",
        b.fall_frames,
        a.fall_frames
    );
}

/// `capture_preset_over` is the same renderer as `capture_preset`, not a second
/// path that might drift from it: holding one stimulus for N frames through the
/// new method reproduces the old method's Nth frame exactly.
#[test]
fn holding_one_stimulus_reproduces_capture_preset() {
    let Some(mut r) = common::headless(SIZE, SIZE) else {
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
