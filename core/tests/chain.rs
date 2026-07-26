//! Full-chain end-to-end: PCM in, pixels out (Plan 0032 Phase 1, ADR-0033 tier 4).
//!
//! Every other suite covers **one** box of the architecture. `lmv-ring`'s unit
//! tests and Miri cover the SPSC ring in isolation; `core/tests/dsp.rs` covers the
//! analyzer; `golden.rs` covers the renderer. Nothing followed a sample all the way
//! across, so the sentence CLAUDE.md opens with — *the ring buffer is the seam
//! between audio and render* — was defended by design review and by nothing that
//! runs.
//!
//! This suite joins the halves. Synthetic PCM is pushed into a real
//! [`lmv_core::audio::intake`] pair in capture-callback-sized bursts, drained
//! through `pop_samples` into a fixed scratch buffer, fed to a real `Analyzer`, and
//! rendered by a real `Renderer` to real pixels. Nothing is mocked and nothing is
//! shortcut: `Renderer::capture_audio` would be the convenient path, but it *starts*
//! at the analyzer, which is precisely the gap this file exists to close.
//!
//! **The drain policy here is a copy of the shell's** (`standalone/src/main.rs`'s
//! `pump_audio`), not a call into it — `core/` cannot depend on the standalone, and
//! extracting the loop into shared code is a deliberate followup rather than a
//! shell change smuggled into a testing plan (ADR-0033, Negative consequences). The
//! two can drift; that is the accepted cost, recorded here so a reader of either
//! side knows to check the other.
//!
//! WARP-only for the rendering claims: macOS has no software Metal fallback, so
//! those skip with a printed reason exactly as `golden.rs` does (ADR-0016). The
//! boundary-validation claim needs no adapter and runs everywhere.

use lmv_core::audio::{self, AudioFormat, FormatError, SampleConsumer};
use lmv_core::dsp::Analyzer;
use lmv_core::preset::Preset;
use lmv_core::render::{CaptureImage, HeadlessOptions, RenderError, Renderer, metrics::frame_diff};
use lmv_core::signal::{bass_sine, noise};

const SIZE: u32 = 128;

const FORMAT: AudioFormat = AudioFormat {
    sample_rate: 48_000,
    channels: 2,
};

/// Seconds of PCM driven through the chain. Long enough to clear the analyzer's
/// 2048-sample window warm-up many times over.
const SECS: f32 = 1.0;

/// Frames per push, i.e. what one capture callback hands over. 20 ms is a typical
/// WASAPI buffer, so the producer/consumer cadence here is the real one rather than
/// a single giant push the ring could never hold.
const BURST_FRAMES: usize = 960;

/// Ring headroom in frames. Deliberately far smaller than the whole signal, so the
/// test genuinely round-trips through a bounded ring instead of using it as a
/// vector.
const RING_FRAMES: usize = 8_192;

/// The shell's drain scratch (`standalone/src/main.rs`), matched so the copy of the
/// drain policy below is faithful in its buffer size too.
const SCRATCH_SAMPLES: usize = 32_768;

/// A probe preset that spends bass and treble on **different** visual axes: bass
/// moves geometry (`warp`, `zoom`), treble moves colour (`hue`, `glow`). A signal
/// with energy in one band therefore cannot accidentally render like a signal with
/// energy in the other, which is what makes the band-routing claim meaningful.
///
/// The per-band gains differ (×3 and ×20) because the bands are **not** normalized
/// against each other: `band_mean` averages over the bins a band spans, and the
/// treble band spans far more of them, so a given loudness reads roughly an order
/// of magnitude lower there than in the narrow bass band. Shipped presets
/// compensate the same way (`clamp(treb * 3, 0, 1)` and friends); this is preset
/// practice, not a thumb on the scale.
///
/// No `trails` / `kaleido_*` / `ink_*`: this file is testing the audio seam, and a
/// feedback stage would add a GPU-state variable that has nothing to do with it.
const PROBE: &str = r#"
system = "fragment_field"
name   = "chain_probe"

[params]
warp  = "0.2 + clamp(bass * 3, 0, 1) * 1.5"
zoom  = "0.7 + clamp(bass * 3, 0, 1) * 0.5"
hue   = "clamp(treb * 20, 0, 1)"
glow  = "0.15 + clamp(treb * 20, 0, 1) * 0.85"
flash = "0"
"#;

/// Build a headless renderer on the software adapter, or `None` (a logged skip)
/// when the runner exposes no adapter — the guard `golden.rs` already uses, reused
/// rather than reinvented.
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

fn probe_preset() -> Preset {
    Preset::from_toml_str(PROBE).expect("probe preset parses")
}

/// Drain everything the ring holds into the analyzer — the shell's `pump_audio`
/// policy: pop into a fixed scratch until a pop comes back empty, pushing each
/// chunk straight on. Allocation-free, exactly as the render thread needs it.
fn drain_into(consumer: &mut SampleConsumer, analyzer: &mut Analyzer, scratch: &mut [f32]) {
    loop {
        let n = consumer.pop_samples(scratch);
        if n == 0 {
            break;
        }
        analyzer.push_interleaved(&scratch[..n]);
    }
}

/// Drive `pcm` through ring -> drain -> analyzer -> renderer and return the last
/// rendered frame.
///
/// Takes a fresh `Renderer` by value because the renderer's scene clock and scene
/// state accumulate: a fresh one per run is what makes two runs comparable, and is
/// the same reset `capture_preset` performs internally for its own purity.
fn run_chain(mut renderer: Renderer, pcm: &[f32]) -> CaptureImage {
    renderer.set_presets(vec![probe_preset()]);
    assert!(
        renderer.select_preset_by_name("chain_probe"),
        "probe preset is in the roster"
    );

    let (mut producer, mut consumer) =
        audio::intake(FORMAT, RING_FRAMES).expect("48 kHz stereo is a valid intake format");
    let mut analyzer = Analyzer::new(FORMAT).expect("48 kHz stereo is a valid analyzer format");
    let mut scratch = vec![0.0f32; SCRATCH_SAMPLES];

    let burst_samples = BURST_FRAMES * FORMAT.channels as usize;
    let mut last = None;
    for burst in pcm.chunks(burst_samples) {
        // Whole frames only — `push_samples` debug-asserts the interleaving.
        let whole = burst.len() / FORMAT.channels as usize * FORMAT.channels as usize;
        let pushed = producer.push_samples(&burst[..whole]);
        assert_eq!(
            pushed, whole,
            "a {BURST_FRAMES}-frame burst fits in a {RING_FRAMES}-frame ring that is drained \
             every iteration; a short push here means the drain below stopped working"
        );

        drain_into(&mut consumer, &mut analyzer, &mut scratch);
        let analysis = analyzer.take_frame();
        last = Some(
            renderer
                .capture_frame(&analysis)
                .expect("headless capture succeeds"),
        );
    }

    last.expect("the signal produced at least one render frame")
}

/// Bass-dominant: a strong 60 Hz sine. Reads bass ~0.19, mid and treble ~0.
fn bass_pcm() -> Vec<f32> {
    bass_sine(60.0, SECS, FORMAT)
}

/// Bright/broadband: seeded white noise. Reads treble ~0.024 against bass ~0.025,
/// i.e. a genuinely different band *profile* rather than merely a quieter one.
///
/// A single high sine was the obvious choice here and is the wrong one: its energy
/// lands in one bin of a band that averages over many, so it reads treble ~0.003 —
/// weaker than noise does, and too weak to drive a visible reaction. Deterministic
/// (seeded, NFR section 6), so it is as reproducible as the sine.
fn bright_pcm() -> Vec<f32> {
    noise(7, SECS, 0.9, FORMAT)
}

/// Claim 1: band routing survives the seam. The *same* ring, the *same* preset and
/// the *same* code path, fed a bass-dominant and then a bright/broadband signal,
/// must render measurably different frames — otherwise band information is being
/// lost somewhere between `push_samples` and the pixels.
///
/// The floor is deliberately far above the noise: the companion determinism test
/// pins that an identical signal reproduces **byte-identically** (difference
/// exactly 0), so any non-zero difference here is signal, and this threshold only
/// guards against a *weak* reaction being read as a real one.
#[test]
fn band_routing_survives_the_ring_seam() {
    const FLOOR: f32 = 0.05;

    let Some(a) = headless() else { return };
    let bass = run_chain(a, &bass_pcm());

    let Some(b) = headless() else { return };
    let bright = run_chain(b, &bright_pcm());

    let diff = frame_diff(&bass, &bright);
    assert!(
        diff > FLOOR,
        "a bass-dominant and a bright signal rendered nearly the same frame \
         (diff {diff:.4} <= {FLOOR}) — band routing is not surviving the ring seam"
    );
}

/// Claim 2: determinism holds *through* the ring (NFR section 6). Two identical runs
/// of the same PCM produce byte-identical captures. The existing determinism checks
/// all start downstream of the intake; this one includes it.
///
/// This is also what makes the band-routing floor above non-vacuous: it establishes
/// that the chain's own run-to-run difference is exactly zero, so the difference
/// that test measures is entirely attributable to the stimulus.
#[test]
fn the_chain_is_deterministic_across_the_ring() {
    let Some(a) = headless() else { return };
    let first = run_chain(a, &bass_pcm());

    let Some(b) = headless() else { return };
    let second = run_chain(b, &bass_pcm());

    assert_eq!(
        (first.width, first.height),
        (second.width, second.height),
        "two runs of the same PCM captured at different sizes"
    );
    assert!(
        first.rgba == second.rgba,
        "two identical runs through the ring produced different pixels \
         (frame_diff {:.6}) — something in the chain is reading a clock or \
         unseeded randomness",
        frame_diff(&first, &second)
    );
}

/// Claim 3: ring overflow is lossy, never fatal. An audio callback that outruns the
/// consumer must drop samples and return — never block, never panic. The chain has
/// to keep producing frames afterwards, because a momentary overrun is an ordinary
/// event, not a failure state.
#[test]
fn ring_overflow_drops_samples_and_the_chain_keeps_running() {
    let (mut producer, mut consumer) =
        audio::intake(FORMAT, RING_FRAMES).expect("valid intake format");
    let mut analyzer = Analyzer::new(FORMAT).expect("valid analyzer format");
    let mut scratch = vec![0.0f32; SCRATCH_SAMPLES];

    // Four times the ring's headroom in one push, as an oversized callback would.
    let capacity_samples = RING_FRAMES * FORMAT.channels as usize;
    let oversized = vec![0.25f32; capacity_samples * 4];
    let pushed = producer.push_samples(&oversized);

    assert!(
        pushed < oversized.len(),
        "an oversized burst was accepted whole ({pushed} of {}) — the ring is not \
         bounded, or the producer blocked to make room",
        oversized.len()
    );
    assert_eq!(
        pushed, capacity_samples,
        "an overflowing push should fill the ring exactly, dropping only the excess"
    );
    assert_eq!(
        pushed % FORMAT.channels as usize,
        0,
        "a short push split a frame, which would desynchronize channel interleaving"
    );

    // The chain still works: drain the survivors, then push and drain real audio.
    drain_into(&mut consumer, &mut analyzer, &mut scratch);
    let pcm = bass_pcm();
    let burst_samples = BURST_FRAMES * FORMAT.channels as usize;
    let mut produced_a_frame = false;
    for burst in pcm.chunks(burst_samples) {
        let whole = burst.len() / FORMAT.channels as usize * FORMAT.channels as usize;
        producer.push_samples(&burst[..whole]);
        drain_into(&mut consumer, &mut analyzer, &mut scratch);
        let analysis = analyzer.take_frame();
        if analysis.bass > 0.0 {
            produced_a_frame = true;
        }
    }
    assert!(
        produced_a_frame,
        "after an overflow the chain never produced a frame with bass energy — \
         the drop was not recoverable"
    );
}

/// Claim 4: format validation rejects at the boundary and never panics. The intake
/// is the one place sample rate and channel count are checked (CLAUDE.md: validate
/// at the boundary, trust inside), so a bad format must come back as a typed error
/// rather than reaching the hot path or unwinding.
///
/// Needs no GPU — this one runs on every platform.
#[test]
fn intake_rejects_bad_formats_without_panicking() {
    fn intake_err(sample_rate: u32, channels: u16) -> FormatError {
        match audio::intake(
            AudioFormat {
                sample_rate,
                channels,
            },
            RING_FRAMES,
        ) {
            Ok(_) => panic!("intake accepted an invalid format ({sample_rate} Hz, {channels} ch)"),
            Err(e) => e,
        }
    }

    assert_eq!(
        intake_err(4_000, 2),
        FormatError::SampleRateOutOfRange(4_000),
        "4 kHz is below MIN_SAMPLE_RATE and must be rejected as such"
    );
    assert_eq!(
        intake_err(48_000, 0),
        FormatError::ChannelsOutOfRange(0),
        "a zero-channel stream must be rejected as such"
    );
    assert_eq!(
        intake_err(48_000, 9),
        FormatError::ChannelsOutOfRange(9),
        "9 channels exceeds MAX_CHANNELS and must be rejected as such"
    );

    // The valid case still succeeds, so the assertions above are rejecting the
    // format rather than the call.
    assert!(audio::intake(FORMAT, RING_FRAMES).is_ok());
}
