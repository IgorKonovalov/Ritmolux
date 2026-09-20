//! A preset rendered after another preset renders what it renders alone
//! (ADR-0222).
//!
//! The three preset sweeps hand one renderer a batch of presets and capture them
//! in turn, which is only sound if a capture is a pure function of its arguments.
//! `capture_preset` and `capture_audio_after_warmup` both claim to be: each
//! rebuilds every scene from its seed, resets the feedback, tonemap, ink and
//! blend resources, and returns the clock to zero. This file is where that claim
//! is measured rather than read.
//!
//! # What it would take for this to go red
//!
//! A change that leaves some piece of render state alive across a preset switch.
//! Concretely: a new GPU-integrated resource — a history texture, a particle
//! buffer, an accumulation target — added to a scene or to a post stage and
//! **not** rebuilt or cleared by `reset_for_capture`; a scene clock that is
//! advanced somewhere other than the capture entry points; a `OnceLock` or
//! `static` that caches something derived from the preset that rendered last.
//! Any of those makes a subject's batched frame depend on the subject before it,
//! and the batched and solo captures separate.
//!
//! It goes red on **nothing else**. The subjects are frozen fixtures, not shipped
//! presets, so tuning the library cannot reach it, and the threshold below is the
//! engine's own drift tolerance, so a rasterizer difference cannot either.
//!
//! # The threshold, and why it is not tighter
//!
//! `MEAN_TOL` and `MAX_OUTLIER` are the values `golden.rs` compares a fresh
//! render against its committed baseline with (ADR-0023) — the project's declared
//! rasterizer-drift floor. Asserting anything tighter here, bit-equality
//! included, would be asserting a property of WARP rather than of the engine, and
//! ADR-0071 is explicit that a threshold at or below the noise floor measures the
//! noise. The measured figures are printed on every run, so the headroom is
//! visible rather than assumed: if they sit at zero, that is a reading and not a
//! reason to move the bar.
//!
//! **The bar is not so loose that it admits everything.** Measured by comparing
//! each subject's batched frame against a *different* subject's solo frame — the
//! coarsest leak this could be asked to catch — the three separate at mean
//! 0.221, 0.464 and 0.548, which is 11x to 27x over `MEAN_TOL`, with outliers at
//! the byte range's ends. A leak has 11x of room before this stops seeing it.
//!
//! Software adapter, so it holds on any CI GPU that has one at all (ADR-0016).

use rlx_core::audio::AudioFormat;
use rlx_core::dsp::{HOP_SIZE, WARMUP_HOPS};
use rlx_core::preset::Preset;
use rlx_core::render::{CaptureImage, Renderer, metrics::frame_diff};
use rlx_core::signal::bass_sine;

use crate::common;

/// The sweeps' own capture size.
const SIZE: u32 = 96;

/// Frames advanced before the still capture. Short, because the question is
/// whether state *carried over*, not how far a scene evolves: a feedback field
/// or a particle cloud that started from someone else's leftovers is already a
/// different picture by here.
const FRAMES: u32 = 16;

/// Hops of stimulus past the analyzer's warm-up, for the audio-driven primitive.
const SIGNAL_HOPS: usize = 4;

/// Mean per-channel difference (0..1) — `golden.rs`'s own drift tolerance.
const MEAN_TOL: f32 = 0.02;

/// Largest single-channel byte difference at any pixel — `golden.rs`'s own.
const MAX_OUTLIER: u8 = 48;

/// 48 kHz stereo, the format the reactivity sweep analyzes at.
const FORMAT: AudioFormat = AudioFormat {
    sample_rate: 48_000,
    channels: 2,
};

/// The three subjects, in the order a batch would render them, spanning what
/// accumulates state: a feedback world whose field is the previous frame, a
/// particle world whose positions are a GPU buffer, and a plain one that
/// integrates nothing and so is the control — it must match whatever happens,
/// and a failure there would mean something outside the scenes is leaking.
const SUBJECTS: [(&str, &str); 3] = [
    (
        "reaction_diffusion",
        include_str!("../fixtures/reaction_diffusion.toml"),
    ),
    ("attractor", include_str!("../fixtures/attractor.toml")),
    (
        "fragment_field",
        include_str!("../fixtures/fragment_field.toml"),
    ),
];

/// Largest absolute single-channel (RGB) byte difference across the two images.
fn max_channel_outlier(a: &CaptureImage, b: &CaptureImage) -> u8 {
    a.rgba
        .chunks_exact(4)
        .zip(b.rgba.chunks_exact(4))
        .flat_map(|(pa, pb)| {
            pa.iter()
                .zip(pb.iter())
                .take(3)
                .map(|(x, y)| x.abs_diff(*y))
        })
        .max()
        .unwrap_or(0)
}

/// The compiled subjects and their display names, in `SUBJECTS` order.
fn roster() -> (Vec<Preset>, Vec<String>) {
    let presets: Vec<Preset> = SUBJECTS
        .iter()
        .map(|(stem, toml)| {
            Preset::from_toml_str(toml)
                .unwrap_or_else(|e| panic!("fixture {stem}.toml is invalid: {e}"))
        })
        .collect();
    let names = presets.iter().map(|p| p.name.clone()).collect();
    (presets, names)
}

/// What one subject produces through both capture primitives.
struct Pair {
    still: CaptureImage,
    audio: CaptureImage,
}

/// Capture `name` the way the `animation`/`sanity` sweeps do and the way
/// `reactivity` does, on whatever renderer is handed in.
fn capture(renderer: &mut Renderer, name: &str) -> Pair {
    let still = renderer
        .capture_preset(name, &common::fixed_frame(), FRAMES)
        .expect("capture the still");

    let hops = WARMUP_HOPS + SIGNAL_HOPS;
    let secs = (hops * HOP_SIZE) as f32 / FORMAT.sample_rate as f32;
    let pcm = bass_sine(60.0, secs, FORMAT);
    // The last rendered hop: the most state any subject in this clip accumulates,
    // and so the frame a leak has had the longest to reach.
    let at = [(hops - 1) as u32];
    let run = renderer
        .capture_audio_after_warmup(name, &pcm, FORMAT, &at, WARMUP_HOPS)
        .expect("capture through the analyzer");
    let audio = run
        .images
        .into_iter()
        .next()
        .expect("one requested hop yields one image");

    Pair { still, audio }
}

/// **A batched render equals a solo render** (ADR-0222).
///
/// Each subject is captured twice: once on a renderer that has already rendered
/// every subject before it in `SUBJECTS` order — which is exactly what a sweep
/// batch does — and once on a renderer of its own that has rendered nothing
/// else. The two are compared at the engine's declared drift tolerance.
#[test]
fn a_preset_renders_the_same_after_another_preset_as_it_does_alone() {
    let Some(mut batched) = common::headless(SIZE, SIZE) else {
        return;
    };
    let (presets, names) = roster();
    batched.set_presets(presets);

    // One renderer, every subject in order — the batch.
    let in_batch: Vec<Pair> = names
        .iter()
        .map(|name| capture(&mut batched, name))
        .collect();
    drop(batched);

    let mut failures: Vec<String> = Vec::new();
    for (name, batch) in names.iter().zip(in_batch.iter()) {
        // A renderer of this subject's own, built and dropped around the one
        // capture, so nothing but this preset has ever been drawn through it.
        // The same roster is loaded either way, so the only difference between
        // the two arms is what rendered before.
        let Some(mut alone) = common::headless(SIZE, SIZE) else {
            return;
        };
        let (presets, _) = roster();
        alone.set_presets(presets);
        let solo = capture(&mut alone, name);
        drop(alone);

        for (primitive, a, b) in [
            ("capture_preset", &batch.still, &solo.still),
            ("capture_audio_after_warmup", &batch.audio, &solo.audio),
        ] {
            let mean = frame_diff(a, b);
            let outlier = max_channel_outlier(a, b);
            println!(
                "{name:<20} {primitive:<26} mean {mean:.6} (tol {MEAN_TOL}) \
                 max_outlier {outlier} (tol {MAX_OUTLIER})"
            );
            if mean > MEAN_TOL || outlier > MAX_OUTLIER {
                failures.push(format!(
                    "{name} through {primitive}: mean {mean:.6} / outlier {outlier} — the \
                     batched render is not the solo render, so something survived the preset \
                     switch and the sweeps' batches are measuring the preset before"
                ));
            }
        }
    }

    assert!(failures.is_empty(), "{failures:#?}");
}
