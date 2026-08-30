//! Per-band reactivity (Plan 0013 Phase 3, HARD). For every embedded preset,
//! compare a silent baseline against one sustained single-band stimulus at a
//! time (bass / mid / treb / onset+beat) and record how much each moves the
//! render. A preset that moves for **no** band has a dead reaction — the floor
//! catches that. The per-band vector is printed so a dead *single* binding (e.g.
//! treble) is visible even when the preset passes on another band.
//!
//! **The stimuli are PCM, and every number here comes out of the real analyzer**
//! (Plan 0067 Phase 1). The four clips are `core::signal` generators pushed
//! hop-by-hop through [`Analyzer`](lmv_core::dsp::Analyzer) — FFT, band split,
//! onset detector, the normalizers of ADR-0049 — and each published
//! `AnalysisFrame` past the analyzer's warm-up drives one rendered frame (Plan
//! 0084 Phase 4: the warm-up hops advance the same analyzer and rasterize
//! nothing). Before this the four stimuli were
//! hand-built frames, which made a green run evidence that the *renderer* does
//! something with numbers the test made up, rather than evidence that the audio
//! path reaches the picture. This gate is the one ADR-0081 leans on to authorize
//! shipped content, so it is the one that had to stop synthesizing; the other
//! four preset gates ask questions about the frame and are correct as they stand
//! (`docs/capturing.md`).
//!
//! **The four columns are not orthogonal, and that is the analyzer being
//! honest.** A hand-built frame can set exactly one band; real signals cannot. A
//! click track is broadband, so it raises `bass` as well as `onset`, and a
//! steady tone's attack raises `onset` once — under ADR-0049's normalization,
//! which scales each level against its own recent peak, "once" is enough to read
//! `1.0`. So a preset bound only to `bass` scores on the `onset` column too. The
//! columns still separate where it matters (a bass tone reads exactly `0.000` in
//! mid and treble), and the gate's own question — does *anything* move — is
//! unaffected; but read the vector as "which stimuli move this preset", not as
//! four independent bindings.
//!
//! Determinism is unchanged (`CLAUDE.md`): every `core::signal` generator is a
//! pure function of its arguments — no wall clock, seeded noise only — and the
//! analysis is a pure function of its window, so the vector printed below is
//! reproducible run to run and machine to machine.
//!
//! Software adapter (`prefer_software`) so it holds on any CI GPU.

use lmv_core::audio::AudioFormat;
use lmv_core::dsp::{HOP_SIZE, WARMUP_HOPS};
use lmv_core::preset::{Preset, SystemKind, default_presets};
use lmv_core::render::{CaptureImage, Renderer, metrics::frame_diff};
use lmv_core::signal::{bass_sine, chord, click_track, treble_tone};

mod common;

/// Small offscreen size — the differential signal doesn't need resolution, and
/// the software adapter is slow.
const SIZE: u32 = 96;

/// Format the stimuli are synthesized and analyzed at: 48 kHz stereo, the same
/// format `shot --signal` uses, so these clips are the ones the authoring tool
/// would play.
const FORMAT: AudioFormat = AudioFormat {
    sample_rate: 48_000,
    channels: 2,
};

/// Hops of stimulus past the analyzer's own warm-up. Matches the 24 held frames
/// the synthesized version rendered, so a preset with a slow `[smoothing]` tau
/// gets the same settling time it always got.
///
/// **This is the gate's cost knob and it was chosen by measurement, not taste.**
/// It has not moved since Plan 0067 and Plan 0084 did not renegotiate it — only
/// the wasted rendering around it went. **16 was tried and rejected** back when
/// every hop rasterized: it brought that sweep to 120 s, but the tightest preset
/// in the library then (`Squall`) fell from 0.0270 to 0.0220 against a 0.020
/// floor — 10 % headroom, one content tweak from a false failure. Those two
/// figures are pre-Plan-0084 and are kept as the reason the knob sits at 24, not
/// as current readings.
///
/// **What Plan 0084 Phase 4 actually bought, measured on this Windows
/// development box through the DX12 software adapter (ADR-0071 — this is a
/// measurement, not a contract): `cargo test --test reactivity --
/// --test-threads=1` over 36 presets went 136.3 s -> 100.2 s, a 26 % cut.** The
/// older 86 s -> 167 s arithmetic in this docstring's previous form was taken on
/// a 41-preset library and does not difference against these; and its ~15 %
/// attributable to the wider readback window **does not come back** — the
/// readbacks are the measurement, and only the warm-up renders were removed.
///
/// **The gate's numbers moved, and that was accepted rather than discovered.**
/// The warm-up renders were expected to be pure waste; they were also the scene
/// warm-up ([`HOPS`] below). Removing them left 35 of the 36 per-band vectors
/// different — every one of them *larger* — with `spectrum/Halo`, the only
/// preset in the set with no accumulating state, bit-identical. Nothing
/// regressed: the lowest max across the library went 0.0287 -> 0.0504 against
/// the 0.020 floor, so the tightest headroom in the set roughly doubled. Read a
/// figure recorded before 2026-08-13 as a different measurement, not as drift.
const SIGNAL_HOPS: usize = 24;

/// Hops of PCM each capture is driven through. The first [`WARMUP_HOPS`]
/// publish nothing at all — the analyzer's long window has not filled, so it
/// holds a zeroed frame — so they are fed through
/// [`Renderer::capture_audio_after_warmup`], which advances the analyzer and the
/// clock without rasterizing anything. Keeping the clip this short is
/// deliberate: the analyzer needs enough window to fill, not a musical phrase,
/// and this gate already sweeps the whole shipped set.
///
/// **These hops advance the analyzer without rendering.** Rendering
/// them at silence would double as a scene warm-up, so every preset
/// carrying GPU-integrated state — trails, particles, a
/// reaction-diffusion field — meets the measured window [`WARMUP_HOPS`]
/// steps colder for their being skipped. Scenes driven only by time and
/// audio are unaffected, because the clock advances either way.
const HOPS: usize = WARMUP_HOPS + SIGNAL_HOPS;

/// Clip length in seconds, derived so the generators produce exactly [`HOPS`]
/// whole hops and no ragged tail.
const CLIP_SECS: f32 = (HOPS * HOP_SIZE) as f32 / FORMAT.sample_rate as f32;

/// Clicks per minute in the onset stimulus. Fast enough that a transient lands
/// inside the measured window rather than decaying before it: at 240 the clicks
/// fall every ~23 hops, and the window below is the whole post-warm-up span, so
/// the comparison sees the strike, the response and the return to rest.
const CLICK_BPM: f32 = 240.0;

/// A preset must move at least this much (mean-abs RGB, 0..1) for its most
/// reactive band. Catches a *dead* preset, not a merely subtle one.
const REACTIVITY_FLOOR: f32 = 0.02;

fn system_name(system: SystemKind) -> &'static str {
    match system {
        SystemKind::FragmentField => "fragment_field",
        SystemKind::Swarm => "swarm",
        SystemKind::ParametricCurve => "parametric_curve",
        SystemKind::LSystem => "lsystem",
        SystemKind::StarPattern => "star_pattern",
        SystemKind::ReactionDiffusion => "reaction_diffusion",
        SystemKind::Attractor => "attractor",
        SystemKind::Spectrum => "spectrum",
        SystemKind::Emitter => "emitter",
        SystemKind::ShapeField => "shape_field",
        SystemKind::WarpMesh => "warp_mesh",
        SystemKind::ShapeCollage => "shape_collage",
    }
}

/// Digital silence, the baseline every stimulus is measured against — the PCM
/// spelling of the zeroed frame this gate used before.
fn silence() -> Vec<f32> {
    vec![0.0; HOPS * HOP_SIZE * FORMAT.channels as usize]
}

/// The four single-band stimuli, as PCM.
///
/// Each is the generator `shot --signal` offers for that band, so a number here
/// and a number an author reads off a filmstrip come from the same clip. What
/// they are *not* is one scalar set to `1.0` over 64 silent log-bands: a real
/// 60 Hz tone lights the bass bands its scalar summarises, so a preset reading
/// the array through `bin()` sees what the scalar claims. The label is asserted
/// rather than assumed — see `each_stimulus_lands_in_the_band_it_is_named_for`.
fn stimuli() -> [(&'static str, Vec<f32>); 4] {
    [
        ("bass", bass_sine(60.0, CLIP_SECS, FORMAT)),
        // Above the ~250 Hz band edge and spread across the mid span, with
        // harmonics rather than a single partial: the mid scalar is a *mean*
        // over ~250 Hz-4 kHz, so a lone tone reads as a trickle.
        ("mid", chord(&[440.0, 660.0, 990.0], CLIP_SECS, FORMAT)),
        ("treb", treble_tone(12_000.0, CLIP_SECS, FORMAT)),
        // A transient is broadband, so the onset stimulus is a click track: real
        // onsets, and a `beat` flag raised by the detector rather than by the
        // test asserting one.
        ("onset", click_track(CLICK_BPM, CLIP_SECS, FORMAT)),
    ]
}

/// The hops read back for comparison: every hop past the analyzer's warm-up.
///
/// A window rather than one settled frame, because a transient is an *event*.
/// Holding `onset = 1.0` forever made the old stimulus comparable at any single
/// frame; a click track's response peaks a few hops after the strike and decays,
/// so a single-frame read would score a beat-latched preset by where the strike
/// happened to fall. Reading back more hops costs readbacks, **not renders** —
/// every hop past the warm-up is rasterized whether or not it is read.
fn measured_hops() -> Vec<u32> {
    (WARMUP_HOPS as u32..HOPS as u32).collect()
}

/// Drive `pcm` through the gate's capture shape: the analyzer's warm-up hops
/// advanced without pixels, the measured window rendered.
///
/// The count is asserted rather than trusted: the one thing that could
/// silently undo this is a warm-up that starts drawing again,
/// rasterizing every hop in the clip and throwing all but the measured
/// window away.
fn capture(renderer: &mut Renderer, name: &str, pcm: &[f32], hops: &[u32]) -> Vec<CaptureImage> {
    let run = renderer
        .capture_audio_after_warmup(name, pcm, FORMAT, hops, WARMUP_HOPS)
        .expect("capture through the analyzer");
    assert_eq!(
        run.rendered, SIGNAL_HOPS,
        "{name}: the measured window and nothing else should reach a rasterizer"
    );
    run.images
}

/// The largest per-hop difference between the silent baseline and `pcm`, over
/// the measured window.
fn response(
    renderer: &mut Renderer,
    name: &str,
    baseline: &[CaptureImage],
    pcm: &[f32],
    hops: &[u32],
) -> f32 {
    let lit = capture(renderer, name, pcm, hops);
    baseline
        .iter()
        .zip(lit.iter())
        .map(|(a, b)| frame_diff(a, b))
        .fold(0.0f32, f32::max)
}

/// The per-band vector for one preset in the renderer's current roster.
fn measure(renderer: &mut Renderer, name: &str) -> Vec<(&'static str, f32)> {
    let hops = measured_hops();
    let baseline = capture(renderer, name, &silence(), &hops);
    stimuli()
        .into_iter()
        .map(|(label, pcm)| (label, response(renderer, name, &baseline, &pcm, &hops)))
        .collect()
}

fn max_of(vector: &[(&'static str, f32)]) -> f32 {
    vector.iter().map(|&(_, d)| d).fold(0.0f32, f32::max)
}

#[test]
fn every_preset_reacts_to_at_least_one_band() {
    let Some(mut renderer) = common::headless(SIZE, SIZE) else {
        return;
    };

    let mut failures = Vec::new();
    for preset in default_presets() {
        let vector = measure(&mut renderer, &preset.name);
        let max = max_of(&vector);
        println!(
            "[{}] {:<10} bass={:.4} mid={:.4} treb={:.4} onset={:.4}  (max {:.4})",
            system_name(preset.system),
            preset.name,
            vector[0].1,
            vector[1].1,
            vector[2].1,
            vector[3].1,
            max,
        );
        if max < REACTIVITY_FLOOR {
            failures.push(format!("{} (per-band {:?})", preset.name, vector));
        }
    }

    assert!(
        failures.is_empty(),
        "these presets react to no band above {REACTIVITY_FLOOR}: {failures:#?}"
    );
}

// ---------------------------------------------------------------------------
// The two properties that make the numbers above mean what they claim
// ---------------------------------------------------------------------------

/// The stimulus labels are a claim about the analyzer, so they are checked
/// through it. CPU only — no renderer, no GPU.
///
/// A hand-built frame could not get this wrong, because it *was* the answer.
/// PCM can: a chord chosen a third of an octave too low lands in bass and the
/// `mid` column would silently be measuring bass reactivity.
#[test]
fn each_stimulus_lands_in_the_band_it_is_named_for() {
    use lmv_core::dsp::Analyzer;

    let hop_samples = HOP_SIZE * FORMAT.channels as usize;
    for (label, pcm) in stimuli() {
        let mut analyzer = Analyzer::new(FORMAT).expect("48 kHz stereo is a valid format");
        let mut peak = [0.0f32; 4]; // bass, mid, treb, onset
        for (hop, chunk) in pcm.chunks(hop_samples).enumerate() {
            // Every hop is pushed — the window only fills if it is fed — but the
            // warm-up frames are not read, because until it fills every band is
            // zero and the maximum would be the analyzer starting up.
            analyzer.push_interleaved(chunk);
            let f = analyzer.take_frame();
            if hop < WARMUP_HOPS {
                continue;
            }
            for (slot, v) in peak.iter_mut().zip([f.bass, f.mid, f.treb, f.onset]) {
                *slot = slot.max(v);
            }
        }
        println!(
            "{label:<6} peak bass={:.3} mid={:.3} treb={:.3} onset={:.3}",
            peak[0], peak[1], peak[2], peak[3]
        );
        let [bass, mid, treb, onset] = peak;
        match label {
            "bass" => assert!(
                bass > mid && bass > treb,
                "the bass stimulus does not lead in bass: {peak:?}"
            ),
            "mid" => assert!(
                mid > bass && mid > treb,
                "the mid stimulus does not lead in mid: {peak:?}"
            ),
            "treb" => assert!(
                treb > bass && treb > mid,
                "the treble stimulus does not lead in treble: {peak:?}"
            ),
            "onset" => assert!(
                onset > 0.5,
                "the click track produced no transient: {peak:?}"
            ),
            other => panic!("unlabelled stimulus `{other}`"),
        }
    }
}

/// Twins that differ in exactly one character of one binding: `glow` reads the
/// bass level in one and is the constant it rests at in the other. Nothing else
/// in either reads audio — `fragment_field::update` ignores its `AnalysisFrame`
/// entirely — so the pair isolates the question this gate exists to ask.
const PROBE_BOUND: &str = r#"
system = "fragment_field"
name   = "probe_band_bound"

[params]
warp  = "0.5"
hue   = "0.15"
zoom  = "0.9"
glow  = "0.25 + 0.75 * bass"
flash = "0.2"
"#;

const PROBE_UNBOUND: &str = r#"
system = "fragment_field"
name   = "probe_band_deleted"

[params]
warp  = "0.5"
hue   = "0.15"
zoom  = "0.9"
glow  = "0.25"
flash = "0.2"
"#;

/// **The property that makes this gate evidence rather than ceremony.** Delete
/// a preset's only band binding and the gate must fail it.
///
/// It is worth a test of its own because the failure it guards against is
/// invisible from a green run: if the analyzer path were disconnected — a
/// stimulus that produces no signal, a capture that never reaches the scene —
/// every preset would score whatever its own animation drifts to, the sweep
/// above would still pass on the loud presets, and nothing would say the audio
/// had stopped arriving. The unbound twin is the control that cannot pass.
#[test]
fn a_preset_whose_only_band_binding_is_deleted_fails_the_gate() {
    let Some(mut renderer) = common::headless(SIZE, SIZE) else {
        return;
    };
    let parse = |src: &str| {
        Preset::from_toml_str(src).unwrap_or_else(|e| panic!("probe fixture parses: {e}"))
    };
    renderer.set_presets(vec![parse(PROBE_BOUND), parse(PROBE_UNBOUND)]);

    let bound = measure(&mut renderer, "probe_band_bound");
    let deleted = measure(&mut renderer, "probe_band_deleted");
    println!("bound   {bound:?}");
    println!("deleted {deleted:?}");

    assert!(
        max_of(&bound) >= REACTIVITY_FLOOR,
        "the bound twin does not clear the floor, so the control below proves \
         nothing: {bound:?}"
    );
    assert!(
        max_of(&deleted) < REACTIVITY_FLOOR,
        "a preset with no band binding at all cleared the reactivity floor — \
         the gate is measuring something other than the audio: {deleted:?}"
    );
}
