//! **The in-frame geometry fraction** (Plan 0069, ADR-0083) — the share
//! of drawn segment length that lands inside the render target, measured
//! at `LineRenderer::draw` and covering the four line-family scenes
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
//! Two duties, in this order:
//!
//! 1. [`the_diagnostic_changes_nothing_about_the_picture`] — the switch is
//!    inert, asserted as **byte-identical** captures rather than a tolerance.
//!    ADR-0083's Negative section asks for exactly this: a second code path
//!    through the hot draw call, and the only way to know it changed nothing is
//!    to assert it. Get that wrong and the instrument measures itself.
//! 2. [`an_over_scaled_figure_measures_below_its_repaired_counterpart`] — the
//!    gate, calibrated against the two frozen defective configurations that
//!    motivated it. **Paired, not thresholded**: read that test's own doc before
//!    reaching for an absolute floor over the library, because two shipped
//!    presets deliberately leave the frame and sit right beside the defect.
//!
//! Software adapter (`prefer_software`) so it holds on any CI GPU, and every
//! capture in a duty comes from **one** renderer: a second
//! `Renderer::new_headless` in this binary would be a second GPU resource build
//! mid-run, which `composite.rs` documents as changing what the software adapter
//! resolves.

use rlx_core::{
    dsp::{AnalysisFrame, SPECTRUM_BINS},
    preset::{Preset, SystemKind, default_presets},
    render::{
        Renderer,
        metrics::{set_extent_diagnostic, take_draw_extent},
    },
};

mod common;

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
    let Some(mut renderer) = common::headless(WIDTH, HEIGHT) else {
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

// ---------------------------------------------------------------------------
// The gate (Plan 0069 Phase 3)
// ---------------------------------------------------------------------------

/// The four families this instrument reaches — the ones that build a CPU-side
/// segment list and stroke it through `LineRenderer`. **Exhaustive with no
/// wildcard arm**, like the golden roster: a new `SystemKind` fails to compile
/// here until someone says which side of the split it is on, because the split
/// does not follow a line an author would guess.
fn draws_segments(system: SystemKind) -> bool {
    match system {
        SystemKind::ParametricCurve
        | SystemKind::LSystem
        | SystemKind::StarPattern
        | SystemKind::Spectrum => true,
        SystemKind::FragmentField
        | SystemKind::Swarm
        | SystemKind::ReactionDiffusion
        | SystemKind::Attractor
        | SystemKind::Emitter
        // `shape_field` draws a figure and still belongs on this side: it is a
        // per-pixel distance with no CPU segment list at all, which is precisely
        // the property ADR-0105 chose it for.
        | SystemKind::ShapeField
        // The warp mesh draws a grid of triangles through its own pipeline, not
        // a CPU segment list through `LineRenderer`, so this instrument does not
        // reach it either.
        | SystemKind::WarpMesh
        // `shape_collage` is `shape_field`'s case exactly: its elements are
        // signed distances evaluated per pixel against a CPU-side element array,
        // and there is no segment list for this instrument to measure.
        | SystemKind::ShapeCollage => false,
    }
}

/// **`Spectrum Comb` and `Spectrum Corona` exactly as they shipped**, frozen
/// here when Plan 0075's cohort four retired the files (recover the commented
/// originals with `git log --diff-filter=D -- presets/spectrum_comb.toml` and
/// `-- presets/spectrum_corona.toml`). The paired comparison below needs the
/// repaired counterpart each over-scaled fixture was recovered from, and a
/// comparison instrument's subjects must stay comparable across runs — the
/// same reasoning that froze the animation ladder's `SQUALL_SRC` and
/// `ROSETTE_SRC`.
const COMB_REPAIRED_SRC: &str = r#"
system = "spectrum"
name   = "Spectrum Comb"
[spectrum]
elements = 26
layout   = "bars"
smoothing = { attack = 0.025, release = 0.22 }
[params]
base = "0.09 + sin(time * 1.5) * 0.075"
rotation = "sin(time * 0.9) * 0.135"
scale = "1.20"
curve = "0.55"
span  = "1.72"
thickness  = "13 + clamp(bass * 4.12, 0, 3.5)"
glow       = "0.75"
brightness = "0.72 + clamp(mid * 0.306, 0, 0.26)"
hue = "mod(time * 0.09 + clamp(treb * 0.233, 0, 0.14), 1)"
zoom  = "1.05 + sin(time * 1.1) * 0.125 + clamp(bass * 0.118, 0, 0.10)"
pan_y = "0.06 + sin(time * 0.17) * 0.03"
bg_hue      = "0.58 + sin(time * 0.011) * 0.05"
bg_bright   = "0.022 + clamp(mid * 0.0141, 0, 0.012)"
bg_vignette = "0.75"
[smoothing]
thickness = { attack = 0.03, release = 0.30 }
brightness = { attack = 0.04, release = 0.28 }
hue       = 0.45
zoom      = 0.22
bg_bright = 0.40
"#;

const CORONA_REPAIRED_SRC: &str = r#"
system = "spectrum"
name   = "Spectrum Corona"
[spectrum]
elements = 44
layout   = "radial_ring"
smoothing = { attack = 0.05, release = 0.40 }
[palette]
stops = [
  { at = 0.00, color = [0.247801, 0.099853, 0.000000] },
  { at = 0.25, color = [0.797738, 0.381092, 0.151704] },
  { at = 0.45, color = [1.000000, 0.679976, 0.271700] },
  { at = 0.55, color = [1.000000, 0.935741, 0.748379] },
  { at = 0.78, color = [0.854306, 0.484529, 0.189748] },
  { at = 1.00, color = [0.247801, 0.099853, 0.000000] },
]
[palette_b]
stops = [
  { at = 0.00, color = [0.000000, 0.247801, 0.461356] },
  { at = 0.28, color = [0.331830, 0.679976, 0.865042] },
  { at = 0.50, color = [0.665185, 0.880825, 0.963976] },
  { at = 0.60, color = [0.930925, 0.977692, 1.000000] },
  { at = 0.82, color = [0.381092, 0.701411, 0.896244] },
  { at = 1.00, color = [0.000000, 0.247801, 0.461356] },
]
[params]
rotation = "time * 0.42 + sin(time * 0.05) * 0.30 + clamp(bass * 0.212, 0, 0.18)"
radius = "0.34 + sin(time * 0.27) * 0.05 + clamp(bass * 0.165, 0, 0.14)"
base  = "0.09 + index * 0.04 + sin(time * 1.1) * 0.07"
scale = "0.45"
curve = "0.58"
hue         = "mod(time * 0.03, 1)"
hue_spread  = "1.0"
saturation  = "1.0"
palette_mix = "0.5 + sin(time * 0.18) * 0.5"
thickness  = "2.4 + clamp(treb * 3, 0, 1.8)"
glow       = "0.68"
brightness = "0.78 + clamp(mid * 0.282, 0, 0.24)"
zoom  = "1.05 + bar * 0.05"
pan_y = "0.0"
bg_hue      = "0.06 + sin(time * 0.009) * 0.05"
bg_bright   = "0.024 + clamp(bass * 0.0165, 0, 0.014)"
bg_vignette = "0.86"
trails = "0.52 + clamp(mid * 0.094, 0, 0.08)"
[smoothing]
rotation    = 0.15
radius      = 0.35
thickness   = { attack = 0.02, release = 0.32 }
brightness  = { attack = 0.04, release = 0.28 }
hue         = 0.40
palette_mix = 0.60
zoom        = 0.25
bg_bright   = 0.40
trails      = 0.55
"#;

/// The two frozen defective configurations, recovered from `2efb80e^`, each
/// paired with **the frozen source of the shipped preset it was recovered
/// from** (frozen above at cohort four, when those files retired; before that
/// the pairing looked the live library up by name). Their fixture headers say
/// what the defect is and why pixel coverage could not see it.
///
/// The pairing is load-bearing: the two differ in exactly one binding
/// (`scale`), so the comparison below isolates the defect rather than
/// comparing two unrelated figures.
const OVER_SCALED: [(&str, &str, &str); 2] = [
    (
        "spectrum_comb_over_scaled",
        include_str!("fixtures/spectrum_comb_over_scaled.toml"),
        COMB_REPAIRED_SRC,
    ),
    (
        "spectrum_corona_over_scaled",
        include_str!("fixtures/spectrum_corona_over_scaled.toml"),
        CORONA_REPAIRED_SRC,
    ),
];

/// **The margin pixel coverage had, and could not use** (ADR-0083): a gate at
/// `0.80` on the loud/moderate coverage ratio would have sat `0.055` from De
/// Jong, the lowest *legitimate* content in the library — and convicted none of
/// the three known-defective configurations, which scored *above* it. It is the
/// benchmark this instrument exists to beat, quoted rather than recomputed
/// because it belongs to a measurement that has already been taken.
const PIXEL_COVERAGE_MARGIN: f32 = 0.055;

/// How much better than that margin repairing an over-scaled figure has to read
/// on this measure. **A property against a quoted benchmark, not a tuned
/// threshold** (ADR-0071): the absolute fractions below are printed and never
/// asserted, because they are measurements of specific presets and content is
/// allowed to move.
///
/// **Set at five times the benchmark, against a target of ten. Measured, the
/// two pairs read `9.0x` (comb) and `14.2x` (corona)** — so the corona clears
/// that target and the comb does not, and the reason is geometry rather than
/// tuning. A comb roots every bar on a shared baseline, so a fully-driven bar
/// at `scale = 3.80` stands `3.97` world units tall from `-0.85` and keeps the
/// `1.85` below the top edge **in frame whatever else is done to it**: about
/// `0.47` of its own length, which bounds the achievable separation near `0.53`
/// before the repaired preset's own losses are counted. That is the same
/// baseline-rooted fact that made this figure invisible to pixel coverage,
/// showing up here as a much weaker version of itself.
///
/// The bar therefore sits at `5x` — a factor of `1.8` below the smaller measured
/// separation, so it is not fitted to either, and still emphatic against an axis
/// whose best available separation was **negative**.
const MIN_SEPARATION: f32 = PIXEL_COVERAGE_MARGIN * 5.0;

/// A sustained fully-driven frame: every band up and a beat. The drive the
/// repair commit reasoned about ("a fully-driven bar stood base + 3.80 = 3.97
/// tall"), and the one an over-scaled figure is furthest out of frame under.
fn loud() -> AnalysisFrame {
    AnalysisFrame {
        bass: 1.0,
        mid: 1.0,
        treb: 1.0,
        onset: 1.0,
        beat: true,
        bar: 0.5,
        spectrum: [1.0; SPECTRUM_BINS],
        ..Default::default()
    }
}

/// Capture `preset` and return the in-frame geometry fraction of its last drawn
/// frame, or `None` when it drew nothing at all (the *total* case, which
/// `sanity.rs` owns).
fn fraction_of(renderer: &mut Renderer, preset: Preset) -> Option<f32> {
    let name = preset.name.clone();
    renderer.set_presets(vec![preset]);
    set_extent_diagnostic(true);
    renderer
        .capture_preset(&name, &loud(), FRAMES)
        .unwrap_or_else(|e| panic!("capture {name}: {e}"));
    let extent = take_draw_extent();
    set_extent_diagnostic(false);
    extent
        .unwrap_or_else(|| panic!("{name} drew through no line renderer at all"))
        .fraction()
}

/// **The gate: an over-scaled figure measures below the repaired preset it was
/// recovered from, by an order of magnitude more than pixel coverage had to work
/// with** (Plan 0069 Phase 3).
///
/// # What is asserted, and what is only printed
///
/// The absolute fraction of any given preset is **printed, not asserted**
/// (ADR-0071): those are measurements of specific content, and content is
/// allowed to move. What is asserted is a *relation* — that correcting the one
/// binding that was wrong moves this measure decisively, in the right direction,
/// on both known defects — plus the structural facts that keep the sweep from
/// going vacuous (every line-family preset draws segments; the roster still
/// covers the library).
///
/// # The measurement, fully driven, at 16:9
///
/// ```text
///   0.1639  Spectrum Corona (pre-repair)  <- OVER-SCALED
///   0.3492  Rose Zoom
///   0.3563  Spectrum Comb (pre-repair)    <- OVER-SCALED
///   0.3659  Rose Overflow
///   0.5633  Rose Web
///   0.7693  Spectrum Ridge
///   0.8268  Rose Trails
///   0.8538  Spectrum Comb
///   0.9167  Rose Draw
///   0.9193  Arrowhead
///   0.9428  Spectrum Corona
///   0.9644  Star Rosette
///   0.9705  Cathedral
///   0.9820  Fern Grow
///   1.0000  Star Lantern
/// ```
///
/// # Read the third and fourth lines before adding a threshold
///
/// **No absolute threshold orders this library either, and that is not a defect
/// in the measure.** `Rose Zoom` sits *below* the over-scaled comb and `Rose
/// Overflow` a hair above it, and both are doing on purpose what the comb did by
/// accident: `Rose Zoom` binds `zoom` to `2.15..3.09` and `Rose Overflow` drives
/// `scale` to `2.84` — a figure flown into and a figure that outgrows the frame
/// are the preset names, not side effects. A length fraction cannot distinguish
/// "deliberately inside the figure" from "accidentally outside the frame",
/// because they are the same picture.
///
/// So this is a **paired** instrument, not a threshold: it convicts a
/// configuration against *its own repair*, which is the question a content pass
/// actually asks. Anyone reaching for `assert!(fraction > 0.5)` over the library
/// would fail two shipped presets that are working as authored — which is
/// exactly the mistake ADR-0083 catalogues pixel coverage making, one axis over.
#[test]
fn an_over_scaled_figure_measures_below_its_repaired_counterpart() {
    let Some(mut renderer) = common::headless(WIDTH, HEIGHT) else {
        return;
    };

    let shipped: Vec<Preset> = default_presets()
        .into_iter()
        .filter(|p| draws_segments(p.system))
        .collect();
    // Re-derived twice, and the number is the shipped count rather than a
    // claim: 8 at Plan 0075 cohort four, then 6 -> 5 when `lsystem_wildwood`
    // was retired (`7596d56`). The library now draws segments from five
    // presets — `curve_ionwake`, `curve_nightbloom`, `lsystem_vellum`,
    // `star_rosewindow` and `spectrum_halo` — so a floor above that fails a
    // library that is fully covered. The guard still catches the real failure
    // it exists for: the filter matching nothing.
    //
    // `7596d56` re-derived the lsystem floor in `sanity.rs` and missed this
    // second guard, which is the shape to expect — a retirement moves every
    // count keyed on the family, and `lsystem` is now a one-member family, so
    // the next preset in it moves this number again.
    assert!(
        shipped.len() >= 5,
        "only {} shipped line-family presets — this sweep has stopped covering \
         the library",
        shipped.len()
    );

    let mut measured: Vec<(String, f32)> = Vec::new();
    for preset in shipped {
        let name = preset.name.clone();
        let Some(fraction) = fraction_of(&mut renderer, preset) else {
            panic!("{name} drew no segment length under a fully-driven frame");
        };
        measured.push((name, fraction));
    }
    // The two frozen defects, each measured against the shipped preset it was
    // recovered from.
    let mut defective: Vec<(String, f32)> = Vec::new();
    let mut separations: Vec<(String, f32, f32, f32)> = Vec::new();
    for (stem, toml, repaired_toml) in OVER_SCALED {
        let preset =
            Preset::from_toml_str(toml).unwrap_or_else(|e| panic!("{stem}.toml is invalid: {e}"));
        let name = preset.name.clone();
        let repaired_preset = Preset::from_toml_str(repaired_toml)
            .unwrap_or_else(|e| panic!("{stem}'s frozen repaired counterpart is invalid: {e}"));
        let repaired_name = repaired_preset.name.clone();
        let repaired = fraction_of(&mut renderer, repaired_preset).unwrap_or_else(|| {
            panic!("{repaired_name} drew no segment length under a fully-driven frame")
        });
        let Some(fraction) = fraction_of(&mut renderer, preset) else {
            panic!(
                "{name} drew no segment length at all, so it is the TOTAL case and \
                 sanity.rs is its instrument, not this one"
            );
        };
        separations.push((name.clone(), fraction, repaired, repaired - fraction));
        defective.push((name, fraction));
    }

    // --- The report. Absolute fractions are printed, never asserted. ---
    let mut report: Vec<(&str, f32, bool)> = measured
        .iter()
        .map(|(n, f)| (n.as_str(), *f, false))
        .chain(defective.iter().map(|(n, f)| (n.as_str(), *f, true)))
        .collect();
    report.sort_by(|a, b| a.1.total_cmp(&b.1));
    eprintln!("in-frame geometry fraction, fully driven, at {WIDTH}x{HEIGHT}:");
    for (name, fraction, is_defective) in &report {
        let mark = if *is_defective {
            "  <- OVER-SCALED"
        } else {
            ""
        };
        eprintln!("  {fraction:.4}  {name}{mark}");
    }
    for (name, defect, repaired, separation) in &separations {
        eprintln!(
            "  separation {separation:.4} ({:.1}x the {PIXEL_COVERAGE_MARGIN} pixel \
             coverage had): {name} {defect:.4} against its repair {repaired:.4}",
            separation / PIXEL_COVERAGE_MARGIN
        );
    }

    // --- The property. ---
    for (name, defect, repaired, separation) in &separations {
        assert!(
            defect < repaired,
            "{name} measures {defect:.4} against its repaired counterpart's \
             {repaired:.4} — the over-scaled configuration is not scoring below \
             the correct one, which is the whole claim of this instrument. Under \
             pixel coverage it scored ABOVE it, and that is why this measure exists"
        );
        assert!(
            *separation >= MIN_SEPARATION,
            "repairing {name} moves the in-frame geometry fraction by only \
             {separation:.4} ({:.1}x the {PIXEL_COVERAGE_MARGIN} margin pixel \
             coverage had and could not use). A defect this measure can barely \
             separate from its own repair is a defect it is not really convicting",
            separation / PIXEL_COVERAGE_MARGIN
        );
    }
}
