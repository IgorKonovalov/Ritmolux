//! Per-band reactivity (Plan 0013 Phase 3, HARD). For every embedded preset,
//! compare a silent baseline against one sustained single-band stimulus at a
//! time (bass / mid / treb / onset+beat) and record how much each moves the
//! render. A preset that moves for **no** band has a dead reaction — the floor
//! catches that. The per-band vector is printed so a dead *single* binding (e.g.
//! treble) is visible even when the preset passes on another band.
//!
//! Software adapter (`prefer_software`) so it holds on any CI GPU.

use lmv_core::dsp::AnalysisFrame;
use lmv_core::preset::{SystemKind, default_presets};
use lmv_core::render::{HeadlessOptions, RenderError, Renderer, metrics::frame_diff};

/// Small offscreen size — the differential signal doesn't need resolution, and
/// the software adapter is slow.
const SIZE: u32 = 96;
/// Frames of sustained stimulus before the comparison capture.
const FRAMES: u32 = 24;
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
    }
}

/// Build a headless `Renderer`, or `None` (a logged skip) when the runner
/// exposes no GPU adapter — macOS has no software Metal fallback (ADR-0016).
/// Any other build error still panics loudly.
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

/// The log-band index ranges each named band roughly occupies, so a stimulus
/// lights the part of the `spectrum` array its own scalar summarises (Plan 0034
/// Phase 2). A scalar band at `1.0` over 64 silent log-bands is not a frame any
/// audio could produce, and a preset reading the array through `bin()` would
/// correctly see nothing under it.
///
/// These are **approximate** — derived from the ~20 Hz–Nyquist log spacing and
/// the ~250 Hz / ~4 kHz band edges — and only need to be *distinct* regions for
/// the differential probe to mean something. They are not a contract with the
/// DSP's exact edges, and no assertion here depends on them being tight. No
/// pre-0034 scene reads `spectrum`, so every other preset's numbers are
/// unchanged.
const BASS_BANDS: std::ops::Range<usize> = 0..22;
const MID_BANDS: std::ops::Range<usize> = 22..48;
const TREB_BANDS: std::ops::Range<usize> = 48..64;

/// A sustained frame with one scalar band up and the matching slice of the log
/// spectrum lit.
fn band_stimulus(
    scalar: impl Fn(&mut AnalysisFrame),
    bands: std::ops::Range<usize>,
) -> AnalysisFrame {
    let mut frame = AnalysisFrame::default();
    scalar(&mut frame);
    for band in frame.spectrum.iter_mut().take(bands.end).skip(bands.start) {
        *band = 1.0;
    }
    frame
}

/// The four single-band stimuli, each a sustained constant frame.
fn stimuli() -> [(&'static str, AnalysisFrame); 4] {
    [
        ("bass", band_stimulus(|f| f.bass = 1.0, BASS_BANDS)),
        ("mid", band_stimulus(|f| f.mid = 1.0, MID_BANDS)),
        ("treb", band_stimulus(|f| f.treb = 1.0, TREB_BANDS)),
        (
            // A transient is broadband, so the onset stimulus lights the whole
            // array rather than a slice.
            "onset",
            AnalysisFrame {
                onset: 1.0,
                beat: true,
                spectrum: [1.0; lmv_core::dsp::SPECTRUM_BINS],
                ..Default::default()
            },
        ),
    ]
}

#[test]
fn every_preset_reacts_to_at_least_one_band() {
    let Some(mut renderer) = headless() else {
        return;
    };
    let silent = AnalysisFrame::default();

    let mut failures = Vec::new();
    for preset in default_presets() {
        let base = renderer
            .capture_preset(&preset.name, &silent, FRAMES)
            .expect("capture silent baseline");

        let mut vector = Vec::new();
        for (label, frame) in stimuli() {
            let lit = renderer
                .capture_preset(&preset.name, &frame, FRAMES)
                .expect("capture stimulus");
            vector.push((label, frame_diff(&base, &lit)));
        }
        let max = vector.iter().map(|&(_, d)| d).fold(0.0f32, f32::max);
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
