//! The preset-reachable spectrum (Plan 0034 / ADR-0036), end to end.
//!
//! The unit-level claims about `bin(x)` — endpoints, interpolation, totality —
//! live beside the rest of the grammar in `preset.rs`. What this file asserts is
//! the part no unit test can: that the band array actually travels from the
//! analysis frame, through `Variables`, through a compiled expression, into a
//! scene parameter, and out as different pixels.
//!
//! Software adapter (`prefer_software`) so it holds on any CI GPU.

use lmv_core::dsp::{AnalysisFrame, SPECTRUM_BINS};
use lmv_core::preset::Preset;
use lmv_core::render::{HeadlessOptions, RenderError, Renderer, metrics::frame_diff};

const SIZE: u32 = 96;
const FRAMES: u32 = 12;
/// Mean-abs RGB difference (0..1) that counts as "materially different". Well
/// above the software adapter's rasterization noise, well below a full recolor.
const MATERIAL: f32 = 0.05;

/// Build a headless `Renderer`, or `None` (a logged skip) when the runner
/// exposes no GPU adapter — macOS has no software Metal fallback (ADR-0016).
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

/// A frame whose band energy sits in one narrow region: `lit` bands starting at
/// `from`, every other band silent.
///
/// The scalar fields carry a **fixed** mid-energy level, identical across every
/// stimulus this file builds — enough that an audio-gated scene is actually lit
/// (a black frame differs from a black frame by nothing), while leaving the band
/// array as the only thing that varies. Any pixel change between two of these is
/// therefore attributable to `bin`.
fn banded(from: usize, lit: usize) -> AnalysisFrame {
    let mut frame = AnalysisFrame {
        bass: 0.6,
        mid: 0.5,
        treb: 0.6,
        onset: 0.4,
        bar: 0.25,
        ..Default::default()
    };
    for band in frame.spectrum.iter_mut().skip(from).take(lit) {
        *band = 1.0;
    }
    frame
}

/// Plan 0034 Phase 1 done-when 4. A parameter bound through `bin()` renders
/// materially differently under energy in the region it samples than under
/// energy elsewhere — and the *control*, the same preset with the same param
/// bound to a constant, renders identically under both. The control is what
/// makes this a test of `bin` rather than of the two frames.
#[test]
fn a_bin_bound_param_reaches_a_scene_and_tracks_its_own_region() {
    let Some(mut renderer) = headless() else {
        return;
    };

    // Energy at the bottom of the log-spaced range vs. energy at the top.
    let low = banded(0, 8);
    let high = banded(SPECTRUM_BINS - 8, 8);

    // `bin(0.05)` samples ~3/63 of the way up — inside `low`'s lit region and
    // far outside `high`'s. It drives `glow` (not `hue`: the palette is cyclic,
    // so hue 0 and hue 1 are the same colour and would hide the swing) over a
    // full-frame field, where a brightness change is unmissable.
    let probe = Preset::from_toml_str(
        "system = \"fragment_field\"\nname = \"Bin Probe\"\n\
         [params]\nhue = \"0.5\"\nglow = \"0.1 + bin(0.05) * 0.9\"\n",
    )
    .expect("probe preset compiles");
    let control = Preset::from_toml_str(
        "system = \"fragment_field\"\nname = \"Bin Control\"\n\
         [params]\nhue = \"0.5\"\nglow = \"0.55\"\n",
    )
    .expect("control preset compiles");
    renderer.set_presets(vec![probe, control]);

    let probe_low = renderer
        .capture_preset("Bin Probe", &low, FRAMES)
        .expect("capture probe under low-band energy");
    let probe_high = renderer
        .capture_preset("Bin Probe", &high, FRAMES)
        .expect("capture probe under high-band energy");
    let moved = frame_diff(&probe_low, &probe_high);

    let control_low = renderer
        .capture_preset("Bin Control", &low, FRAMES)
        .expect("capture control under low-band energy");
    let control_high = renderer
        .capture_preset("Bin Control", &high, FRAMES)
        .expect("capture control under high-band energy");
    let control_moved = frame_diff(&control_low, &control_high);

    println!("bin-bound {moved:.4} vs constant-bound {control_moved:.4}");
    assert!(
        moved > MATERIAL,
        "a bin()-bound param must track its region: moved only {moved:.4}"
    );
    assert!(
        control_moved < 1e-6,
        "the two stimuli differ only in the band array, so a preset that does \
         not call bin() must render identically: moved {control_moved:.4}"
    );
}
