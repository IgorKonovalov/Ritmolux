//! `palette_contour` fires where the **ink** changes, not at every band edge
//! (Plan 0121 Phase 5, ADR-0133).
//!
//! ADR-0078 drew the contour from the fractional position within the band grid,
//! which is a pure function of position and never reads the LUT. On the smooth
//! gradients it was written against every band boundary *is* a colour change, so
//! the distinction never came up. A limited-ink look is written as **plateaus**
//! — runs of bands holding one colour, the only way to get flat ink out of
//! `palette_steps` — and on those, most boundaries are white-meets-white, where
//! the contour drew exactly the grey shading a two-ink print is defined by not
//! having. Four of the nine presets naming the parameter set it to `0`.
//!
//! Every assertion here is a **differential between two captures through the
//! same pipeline**, identical but for `palette_contour`. The vignette, the
//! tonemap and the 8-bit display write are downstream of the contour and equal
//! on both sides, so they cancel; what is left is where the line was drawn.
//!
//! # What this suite does NOT assert, and where that lives instead
//!
//! *"A smooth palette is unchanged"* is not testable here, because there is no
//! second implementation to compare against. Its evidence is the **golden
//! suite**: the five shipped presets carrying a non-zero `palette_contour` —
//! `fragment_mandala`, `fragment_strata`, `fragment_tiled`, `fragment_vitrail`,
//! `shape_pulse` — are all on smooth palettes, and none of their baselines moved.
//! That is a byte-level claim this file could only weaken. What this file adds is
//! the half a golden cannot see: that the suppression is driven by *colour* and
//! not by step count, checked at a **high** step count as well as a low one — the
//! failure mode ADR-0133's Alternative B was rejected for appears only when
//! adjacent band centres sit close together.
//!
//! **Software adapter** (`prefer_software`), like the other GPU suites
//! (ADR-0016), so this runs on CI rather than only where a GPU happens to be.

use lmv_core::dsp::AnalysisFrame;
use lmv_core::preset::Preset;
use lmv_core::render::{CaptureImage, HeadlessOptions, RenderError, Renderer};

/// Capture size. Small on purpose: every assertion counts pixels over the whole
/// frame, and none of the properties vary with resolution.
const SIZE: u32 = 96;

/// Frames per capture. `warp = 0` leaves a closed-form field, so a handful of
/// frames is a settled picture; a few keeps the clock off zero.
const FRAMES: u32 = 6;

/// The contour strength every "on" capture uses. Mid-range: strong enough that a
/// drawn line clears the dither by a wide margin, well short of the `1.0` that
/// would black out an edge entirely.
const CONTOUR: f32 = 0.5;

/// A `fragment_field` probe with **no fold** (`warp = 0`), so the palette
/// coordinate is a plain diagonal sinusoid sweeping the whole gradient once —
/// every band edge is crossed, and crossed cleanly.
fn probe(palette: &str, steps: u32, contour: f32) -> Preset {
    let toml = format!(
        "system = \"fragment_field\"\nname = \"probe\"\n{palette}\n[params]\n\
         warp = \"0\"\nzoom = \"1.6\"\nglow = \"1.0\"\nflash = \"0\"\n\
         hue = \"0\"\ncolor_span = \"1.0\"\ncolor_center = \"0\"\n\
         saturation = \"1.0\"\nbloom_amount = \"0\"\ntrails = \"0\"\n\
         palette_steps = \"{steps}\"\npalette_contour = \"{contour}\"\n"
    );
    Preset::from_toml_str(&toml).unwrap_or_else(|e| panic!("the probe preset parses: {e}"))
}

/// A palette of one colour end to end — a single plateau, so **no** band edge in
/// it is an ink change however many bands it is cut into.
fn one_run() -> String {
    "[palette]\nstops = [{ at = 0.0, color = [0.55, 0.55, 0.55] }, \
     { at = 1.0, color = [0.55, 0.55, 0.55] }]"
        .to_string()
}

/// Two plateaus meeting once in the middle. The stops sit `0.01` either side of
/// the seam so no band centre lands inside the transition — the way a limited-ink
/// palette is actually authored.
fn two_runs() -> String {
    "[palette]\nstops = [{ at = 0.0, color = [0.20, 0.20, 0.20] }, \
     { at = 0.49, color = [0.20, 0.20, 0.20] }, \
     { at = 0.51, color = [0.85, 0.85, 0.85] }, \
     { at = 1.0, color = [0.85, 0.85, 0.85] }]"
        .to_string()
}

/// A smooth ramp: every band centre sits at a different place on it, so every
/// band edge separates two different colours at any step count.
fn smooth() -> String {
    "[palette]\nstops = [{ at = 0.0, color = [0.05, 0.05, 0.05] }, \
     { at = 1.0, color = [0.95, 0.95, 0.95] }]"
        .to_string()
}

/// Build a software headless renderer, or `None` (a logged skip) when the runner
/// exposes no adapter at all (ADR-0016).
fn renderer() -> Option<Renderer> {
    match Renderer::new_headless(HeadlessOptions {
        width: SIZE,
        height: SIZE,
        prefer_software: true,
    }) {
        Ok(renderer) => Some(renderer),
        Err(RenderError::RequestAdapter(_)) => {
            eprintln!("skipped: no GPU adapter on this runner (ADR-0016)");
            None
        }
        Err(e) => panic!("headless renderer build failed: {e}"),
    }
}

fn capture(renderer: &mut Renderer, palette: &str, steps: u32, contour: f32) -> CaptureImage {
    renderer.set_presets(vec![probe(palette, steps, contour)]);
    renderer
        .capture_preset("probe", &AnalysisFrame::default(), FRAMES)
        .unwrap_or_else(|e| panic!("capture the contour probe: {e}"))
}

/// Pixels whose colour the contour moved by at least two 8-bit levels.
///
/// Two, not one: the display write dithers (ADR-0096), so a sub-level shift
/// anywhere else in the frame could flip a single level. A drawn contour at
/// `CONTOUR` strength moves the pixels it touches by far more than that.
fn darkened(off: &CaptureImage, on: &CaptureImage) -> usize {
    assert_eq!(off.rgba.len(), on.rgba.len(), "the captures differ in size");
    off.rgba
        .chunks_exact(4)
        .zip(on.rgba.chunks_exact(4))
        .filter(|(a, b)| (0..3).any(|c| a[c].abs_diff(b[c]) >= 2))
        .count()
}

/// **Inside a run, no line at all.** A palette that holds one colour end to end
/// has no ink change anywhere in it, so a non-zero `palette_contour` must be an
/// exact identity — not "nearly", not "below a threshold": the shader returns
/// `1.0` on every pixel and the two captures are the same bytes.
///
/// Asserted at four step counts, because the rule is about *colour* and must not
/// acquire a dependence on how finely the gradient is cut.
#[test]
fn a_single_plateau_draws_no_contour_at_any_step_count() {
    let Some(mut r) = renderer() else { return };
    for steps in [2u32, 5, 20, 32] {
        let off = capture(&mut r, &one_run(), steps, 0.0);
        let on = capture(&mut r, &one_run(), steps, CONTOUR);
        assert_eq!(
            off.rgba,
            on.rgba,
            "at palette_steps = {steps} a one-colour palette drew a contour: \
             {} pixels moved. Every band edge in it is flat-meets-flat, which is \
             the grey hairline a limited-ink print is defined by not having",
            darkened(&off, &on)
        );
    }
}

/// **At a run boundary, a line.** Two plateaus meeting once: the contour has to
/// survive there, or ADR-0133 would have replaced an unusable parameter with an
/// inert one.
///
/// The second half is the one that makes the first mean something — the same
/// step count on a *smooth* palette draws at every edge, so the plateau's count
/// must be a small fraction of it rather than merely non-zero.
#[test]
fn a_run_boundary_draws_and_the_interior_does_not() {
    let Some(mut r) = renderer() else { return };
    const STEPS: u32 = 20;

    let plateau = darkened(
        &capture(&mut r, &two_runs(), STEPS, 0.0),
        &capture(&mut r, &two_runs(), STEPS, CONTOUR),
    );
    assert!(
        plateau > 0,
        "a two-run palette drew no contour anywhere — the run boundary is a real \
         ink change and must still be outlined"
    );

    let ramp = darkened(
        &capture(&mut r, &smooth(), STEPS, 0.0),
        &capture(&mut r, &smooth(), STEPS, CONTOUR),
    );
    // The palette wraps (repeat addressing), so the two runs meet twice: once
    // mid-gradient and once across the seam. Two edges of twenty, against
    // twenty of twenty — a quarter is a generous margin around that ratio.
    assert!(
        plateau * 4 < ramp,
        "the plateau palette darkened {plateau} pixels against the smooth \
         palette's {ramp} at the same {STEPS} steps. It should be drawing at its \
         two run boundaries only, not at all twenty band edges"
    );
}

/// The rule is driven by colour and **not** by step count — the failure mode
/// ADR-0133's Alternative B (scale the darkening by how different the two
/// colours are) was rejected for.
///
/// At 32 steps adjacent band centres on a smooth ramp are close together, which
/// is precisely where a "how different" test would fade the line out. Equality at
/// half a code value cannot: two distinct centres on a ramp differ by at least
/// one code value, so the contour is as present at 32 steps as at 4, while the
/// flat palette stays silent at both.
#[test]
fn a_smooth_palette_still_contours_where_its_band_centres_are_closest() {
    let Some(mut r) = renderer() else { return };
    for steps in [4u32, 32] {
        let ramp = darkened(
            &capture(&mut r, &smooth(), steps, 0.0),
            &capture(&mut r, &smooth(), steps, CONTOUR),
        );
        assert!(
            ramp > 0,
            "at palette_steps = {steps} a smooth ramp drew no contour at all — \
             every one of its band edges separates two different colours"
        );
        let flat = darkened(
            &capture(&mut r, &one_run(), steps, 0.0),
            &capture(&mut r, &one_run(), steps, CONTOUR),
        );
        assert_eq!(
            flat, 0,
            "at palette_steps = {steps} the flat palette drew {flat} pixels of \
             contour while the ramp drew {ramp}; the two must separate at every \
             step count"
        );
    }
}
