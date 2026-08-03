//! Shape sanity (Plan 0013 Phase 3, HARD). A newly-added scene that drew nothing
//! or a single dot should fail before it ships. Under a sustained *loud* frame
//! (so audio-gated brightness is up), assert each preset lights a minimum
//! fraction of the frame (`coverage`) and spreads across at least two quadrants
//! (`quadrant_spread`) — "not blank, not a dot".
//!
//! The background is sampled from a corner pixel, **not** assumed to be black:
//! `fragment_field` clears to black but `swarm` clears to a dark blue, so a
//! fixed black background would score every swarm frame as fully lit (a
//! tautology — Plan 0013 Risks). Measuring foreground against the frame's own
//! background makes a blank frame score 0 whatever colour it cleared to.
//!
//! Coverage floors are per-system: `fragment_field` fills the frame, while the
//! `swarm` is sparse points, so a single broad floor would be either tautological
//! for one or impossible for the other.
//!
//! **Plan 0056 Phase 5 adds a third question: does the shape have an interior?**
//! "Not blank, not a dot" is satisfied completely by a fully saturated
//! single-tone mass — a real figure, the right size, in every quadrant, and a
//! blot. That is how four attractor presets shipped flat behind this gate, and
//! `tonal_flatness` is the statistic that names it. It is general, not
//! attractor-specific: any drive that stacks past the additive ceiling produces
//! it.

use lmv_core::{
    dsp::AnalysisFrame,
    preset::{Preset, SystemKind, default_presets},
    render::{
        CaptureImage, HeadlessOptions, RenderError, Renderer,
        metrics::{TONE_BANDS, coverage, quadrant_spread, tonal_flatness},
    },
};

const SIZE: u32 = 96;
const FRAMES: u32 = 30;
/// A pixel counts as lit if any RGB channel differs from the sampled background
/// by more than this (shrugs off dark near-background dithering).
const EPS: u8 = 10;
/// Minimum lit quadrants — a dot in one corner fails.
const MIN_QUADRANTS: u8 = 2;

/// Maximum share of the lit figure that may sit inside one narrow luminance
/// band (Plan 0056 Phase 5, backlog 0047) — the point past which the picture has
/// no tonal structure left, only a mass of one tone.
///
/// `coverage` and `quadrant_spread` ask *is something there* and *is it more
/// than a dot*, and a fully saturated single-tone mass answers yes to both: it
/// is a real shape, the right size, in every quadrant, and it is also a blot.
/// This is the third question.
///
/// **Measured, from the shipped library's own values.**
/// `every_preset_draws_a_real_shape` prints the whole distribution on every run.
/// Today, past [`KNOWN_FLAT`], the highest is `0.830` (`Rose Trails`) and the
/// next is `0.765` (`Rose Web`) — both trails-heavy line looks, where most lit
/// pixels are faint tail sitting at one level. Everything else is at or below
/// `0.66`. The deliberately flattened fixture below reads `0.98`. `0.90` sits
/// between them with room on both sides.
///
/// A measured constant, so it has a shelf life: re-measure when the library
/// changes materially.
const MAX_TONAL_FLATNESS: f32 = 0.90;

/// Shipped presets that are flat **today**, tracked rather than gated.
///
/// A defect list, not a policy. `Spectrum Ridge` measures `1.000` here: under
/// [`loud`] every one of its 40 elements is driven to full, so its mirrored
/// contour is a straight line, and its own header already records the mechanism
/// — "two haloed strokes at the same spot add on an additive renderer". Its two
/// siblings drawing the *same data* read `0.31` and `0.44` under the identical
/// fixture, so this is not the stimulus being degenerate; it is one preset
/// saturating.
///
/// Repairing it is preset-authoring work and Plan 0056 is explicitly
/// test-and-harness only, so it is listed rather than fixed here. The entry is
/// asserted to *still* be flat below: when the preset is repaired this test says
/// to delete the line, rather than leaving a stale exemption behind.
const KNOWN_FLAT: &[&str] = &["Spectrum Ridge"];

/// Per-system minimum lit fraction. The full-screen field must fill most of the
/// frame; the sparse swarm need only paint a small but real footprint.
fn coverage_floor(system: SystemKind) -> f32 {
    match system {
        // Full-screen field fills most of the frame.
        SystemKind::FragmentField => 0.30,
        // Reaction-diffusion paints a real pattern across the frame, but the
        // present maps only the sparse V species, so the lit fraction is modest.
        SystemKind::ReactionDiffusion => 0.03,
        // Sparse line art / point swarm / attractor cloud / spectrum comb: a
        // small but real footprint.
        SystemKind::Swarm
        | SystemKind::ParametricCurve
        | SystemKind::LSystem
        | SystemKind::StarPattern
        | SystemKind::Attractor
        | SystemKind::Spectrum => 0.01,
    }
}

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

/// A sustained "loud" frame: every band up and a beat, so any audio-gated
/// brightness reaches its lit state.
///
/// "Every band up" now includes the `spectrum` array itself (Plan 0034 Phase 2).
/// A frame with `bass = mid = treb = 1.0` and 64 silent log-bands is not a frame
/// any audio could produce, and under it a spectrum readout would correctly draw
/// almost nothing — the floor would be measuring the fixture, not the scene. No
/// pre-0034 scene reads `spectrum`, so every other preset's capture is
/// unchanged.
fn loud() -> AnalysisFrame {
    AnalysisFrame {
        bass: 1.0,
        mid: 1.0,
        treb: 1.0,
        onset: 1.0,
        beat: true,
        bar: 0.5,
        spectrum: [1.0; lmv_core::dsp::SPECTRUM_BINS],
        ..Default::default()
    }
}

/// The top-left pixel, taken as the scene's background colour (the clear colour
/// each built-in scene paints its corners with).
fn background(img: &CaptureImage) -> [u8; 4] {
    [
        img.rgba.first().copied().unwrap_or(0),
        img.rgba.get(1).copied().unwrap_or(0),
        img.rgba.get(2).copied().unwrap_or(0),
        img.rgba.get(3).copied().unwrap_or(255),
    ]
}

#[test]
fn every_preset_draws_a_real_shape() {
    let Some(mut renderer) = headless() else {
        return;
    };
    let frame = loud();

    let mut failures = Vec::new();
    let mut flatness = Vec::new();
    for preset in default_presets() {
        let img = renderer
            .capture_preset(&preset.name, &frame, FRAMES)
            .expect("capture preset");
        let bg = background(&img);
        let cov = coverage(&img, bg, EPS);
        let spread = quadrant_spread(&img, bg, EPS);
        let flat = tonal_flatness(&img, bg, EPS);
        let floor = coverage_floor(preset.system);
        println!(
            "[{}] {:<12} coverage={cov:.4} (floor {floor:.2}) quadrants={spread} \
             flatness={flat:.4} (max {MAX_TONAL_FLATNESS:.2})",
            system_name(preset.system),
            preset.name,
        );
        let known_flat = KNOWN_FLAT.contains(&preset.name.as_str());
        flatness.push((flat, preset.name.clone(), known_flat));
        if cov < floor {
            failures.push(format!(
                "{} blank: coverage {cov:.4} < {floor:.2}",
                preset.name
            ));
        }
        if spread < MIN_QUADRANTS {
            failures.push(format!(
                "{} is a dot: {spread} quadrant(s) < {MIN_QUADRANTS}",
                preset.name
            ));
        }
        if flat > MAX_TONAL_FLATNESS && !known_flat {
            failures.push(format!(
                "{} is flat: {:.1}% of its lit pixels sit in one of {TONE_BANDS} luminance \
                 bands (max {:.0}%) — a real shape with no interior, which coverage and \
                 spread both score as healthy. Lower the drive, the glow or the \
                 accumulation until the figure has falloff again",
                preset.name,
                flat * 100.0,
                MAX_TONAL_FLATNESS * 100.0,
            ));
        }
        // The list must not outlive the defect. A repaired preset that is still
        // named here would silently exempt whatever it becomes next.
        if known_flat && flat <= MAX_TONAL_FLATNESS {
            failures.push(format!(
                "{} is listed in KNOWN_FLAT but now measures {flat:.4}, under the \
                 {MAX_TONAL_FLATNESS:.2} ceiling — it was repaired, so delete the entry",
                preset.name
            ));
        }
    }

    // The distribution the threshold above is set from, printed on every run so
    // the next re-measurement does not need a special one.
    flatness.sort_by(|a, b| b.0.total_cmp(&a.0));
    println!("flattest presets (share of lit pixels in one luminance band):");
    for (flat, name, known) in flatness.iter().take(8) {
        let mark = if *known { "  (KNOWN_FLAT)" } else { "" };
        println!("  {flat:.4}  {name}{mark}");
    }

    assert!(
        failures.is_empty(),
        "these presets failed shape sanity: {failures:#?}"
    );
}

/// A line scene driven far past the additive ceiling: strokes wide enough to
/// meet, a glow multiplier that saturates every core, and a long trail that
/// stacks the same light again — so the whole figure clips to one tone.
///
/// Deliberately built the way the *shipped* flat frames got there (an additive
/// stack, not an `exposure` stop), because that is the failure mode this gate
/// exists to name. Exposure alone will not do it: past the knee the background
/// blows out with the figure, and a background-relative metric correctly stops
/// finding anything lit.
fn blown_out() -> Preset {
    Preset::from_toml_str(
        r#"
system = "parametric_curve"
name   = "Blown Out"

[params]
scale      = "0.9"
glow       = "20"
brightness = "16"
thickness  = "44"
trails     = "0.97"
"#,
    )
    .expect("the flat fixture parses")
}

#[test]
fn a_frame_with_no_tonal_structure_is_reported_flat() {
    let Some(mut renderer) = headless() else {
        return;
    };
    renderer.set_presets(vec![blown_out()]);
    let img = renderer
        .capture_preset("Blown Out", &loud(), FRAMES)
        .expect("capture the flat fixture");
    let bg = background(&img);

    let cov = coverage(&img, bg, EPS);
    let spread = quadrant_spread(&img, bg, EPS);
    let flat = tonal_flatness(&img, bg, EPS);
    println!("[blown out] coverage={cov:.4} quadrants={spread} flatness={flat:.4}");

    // The fixture has to pass the two existing checks, or it demonstrates
    // nothing: the whole claim is that a blot satisfies both of them.
    assert!(
        cov >= coverage_floor(SystemKind::ParametricCurve),
        "the fixture must pass the coverage floor, or it proves nothing: {cov:.4}"
    );
    assert!(
        spread >= MIN_QUADRANTS,
        "the fixture must pass the spread floor, or it proves nothing: {spread}"
    );
    assert!(
        flat > MAX_TONAL_FLATNESS,
        "a figure stacked past the additive ceiling must read flat, got {flat:.4}"
    );
}
