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

/// The shared ADR-0016 skip and headless constructors.
use crate::common;

use rlx_core::dsp::AnalysisFrame;
use rlx_core::preset::Preset;
use rlx_core::render::{CaptureImage, Renderer};

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

/// The four contour controls one capture is taken with (ADR-0197). Grouped
/// because every assertion below is a differential across them and a positional
/// argument list of four would read as noise at the call sites.
#[derive(Clone, Copy)]
struct Contour {
    /// `palette_contour` — how strongly the line is drawn.
    amount: f32,
    /// `palette_contour_style` — 0 soft black, 1 hard black, 2 soft ink, 3 hard ink.
    style: u32,
    /// `palette_contour_ink` — the absolute palette coordinate an ink style reads.
    ink: f32,
    /// `palette_mix` — which of the two palettes the whole frame, including the
    /// ink, is sampled from.
    mix: f32,
}

impl Contour {
    /// No line at all: the baseline every differential is taken against.
    const OFF: Self = Self {
        amount: 0.0,
        style: 0,
        ink: 0.0,
        mix: 0.0,
    };

    /// The line at `amount` in `style`, on palette A.
    const fn styled(amount: f32, style: u32) -> Self {
        Self {
            amount,
            style,
            ink: 0.0,
            mix: 0.0,
        }
    }
}

/// A `fragment_field` probe with **no fold** (`warp = 0`), so the palette
/// coordinate is a plain diagonal sinusoid sweeping the whole gradient once —
/// every band edge is crossed, and crossed cleanly.
fn probe_with(palette: &str, steps: u32, c: Contour) -> Preset {
    let Contour {
        amount,
        style,
        ink,
        mix,
    } = c;
    let toml = format!(
        "system = \"fragment_field\"\nname = \"probe\"\n{palette}\n[params]\n\
         warp = \"0\"\nzoom = \"1.6\"\nglow = \"1.0\"\nflash = \"0\"\n\
         hue = \"0\"\ncolor_span = \"1.0\"\ncolor_center = \"0\"\n\
         saturation = \"1.0\"\nbloom_amount = \"0\"\ntrails = \"0\"\n\
         palette_steps = \"{steps}\"\npalette_contour = \"{amount}\"\n\
         palette_contour_style = \"{style}\"\npalette_contour_ink = \"{ink}\"\n\
         palette_mix = \"{mix}\"\n"
    );
    Preset::from_toml_str(&toml).unwrap_or_else(|e| panic!("the probe preset parses: {e}"))
}

/// A palette of one colour end to end — a single plateau, so **no** band edge in
/// it is an ink change however many bands it is cut into.
fn one_run() -> String {
    "[palette]\nstops = [{ at = 0.0, color = [0.767376, 0.767376, 0.767376] }, \
     { at = 1.0, color = [0.767376, 0.767376, 0.767376] }]"
        .to_string()
}

/// Two plateaus meeting once in the middle. The stops sit `0.01` either side of
/// the seam so no band centre lands inside the transition — the way a limited-ink
/// palette is actually authored.
fn two_runs() -> String {
    "[palette]\nstops = [{ at = 0.0, color = [0.484529, 0.484529, 0.484529] }, \
     { at = 0.49, color = [0.484529, 0.484529, 0.484529] }, \
     { at = 0.51, color = [0.930925, 0.930925, 0.930925] }, \
     { at = 1.0, color = [0.930925, 0.930925, 0.930925] }]"
        .to_string()
}

/// The three inks of [`three_runs`], as sRGB greys. The third is the one an ink
/// contour is asked to draw in, and the fourth is palette B's replacement for it.
const INK_ONE: f32 = 0.20;
const INK_TWO: f32 = 0.55;
const INK_THREE: f32 = 0.90;
const INK_THREE_B: f32 = 0.35;

/// Where along the palette [`three_runs`]' third ink sits — the middle of its
/// plateau, well clear of both transitions, so the LUT there is exactly the ink.
const INK_COORD: f32 = 0.80;

/// The step count every plateau assertion uses. Twenty bands puts the band
/// centres at `0.025 + 0.05k`, and [`three_runs`]' two transitions are placed on
/// band *edges* (`0.35`, `0.65`) so no centre can land inside one.
const PLATEAU_STEPS: u32 = 20;

/// One flat colour end to end, as a `[palette]` table — the reference a contour
/// drawn in that colour is compared against, pixel for pixel.
fn flat(ink: f32) -> String {
    format!(
        "[palette]\nstops = [{{ at = 0.0, color = [{ink}, {ink}, {ink}] }}, {{ at = 1.0, color = [{ink}, {ink}, {ink}] }}]"
    )
}

/// The three-plateau stop list, with `third` as its last ink.
fn three_run_stops(third: f32) -> String {
    format!(
        "stops = [{{ at = 0.0, color = [{one}, {one}, {one}] }}, \
         {{ at = 0.34, color = [{one}, {one}, {one}] }}, \
         {{ at = 0.36, color = [{two}, {two}, {two}] }}, \
         {{ at = 0.64, color = [{two}, {two}, {two}] }}, \
         {{ at = 0.66, color = [{third}, {third}, {third}] }}, \
         {{ at = 1.0, color = [{third}, {third}, {third}] }}]",
        one = INK_ONE,
        two = INK_TWO,
    )
}

/// Three plateaus, so a contour drawn in the third ink is drawn in a colour the
/// frame already carries **and** one the two boundaries it fires at do not.
fn three_runs() -> String {
    format!("[palette]\n{}", three_run_stops(INK_THREE))
}

/// The same three plateaus as palette A, with a different third ink as palette B
/// — so `palette_mix = 1` moves the whole frame *and* the ink it is keyed in,
/// while leaving every run boundary where palette A put it.
fn three_runs_ab() -> String {
    format!(
        "[palette]\n{}\n[palette_b]\n{}",
        three_run_stops(INK_THREE),
        three_run_stops(INK_THREE_B)
    )
}

/// A smooth ramp: every band centre sits at a different place on it, so every
/// band edge separates two different colours at any step count.
fn smooth() -> String {
    "[palette]\nstops = [{ at = 0.0, color = [0.247801, 0.247801, 0.247801] }, \
     { at = 1.0, color = [0.977692, 0.977692, 0.977692] }]"
        .to_string()
}

/// Build a software headless renderer, or `None` (a logged skip) when the runner
/// exposes no adapter at all (ADR-0016).
fn renderer() -> Option<Renderer> {
    common::headless(SIZE, SIZE)
}

fn capture(renderer: &mut Renderer, palette: &str, steps: u32, contour: f32) -> CaptureImage {
    capture_with(renderer, palette, steps, Contour::styled(contour, 0))
}

fn capture_with(
    renderer: &mut Renderer,
    palette: &str,
    steps: u32,
    contour: Contour,
) -> CaptureImage {
    renderer.set_presets(vec![probe_with(palette, steps, contour)]);
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
    darkened_set(off, on).len()
}

/// The indices of those pixels, so one capture's footprint can be compared
/// against another's rather than only counted.
fn darkened_set(off: &CaptureImage, on: &CaptureImage) -> std::collections::BTreeSet<usize> {
    assert_eq!(off.rgba.len(), on.rgba.len(), "the captures differ in size");
    off.rgba
        .chunks_exact(4)
        .zip(on.rgba.chunks_exact(4))
        .enumerate()
        .filter(|(_, (a, b))| (0..3).any(|c| a[c].abs_diff(b[c]) >= 2))
        .map(|(i, _)| i)
        .collect()
}

/// The indices of every pixel the two captures disagree about **at all**.
///
/// One byte, not two: the display write's dither is a pure function of the pixel
/// coordinate and the value (ADR-0096), so two captures through the same pipeline
/// at the same size agree byte-for-byte wherever the light reaching the write is
/// the same. Any difference is the change under test.
fn changed_set(off: &CaptureImage, on: &CaptureImage) -> std::collections::BTreeSet<usize> {
    assert_eq!(off.rgba.len(), on.rgba.len(), "the captures differ in size");
    off.rgba
        .chunks_exact(4)
        .zip(on.rgba.chunks_exact(4))
        .enumerate()
        .filter(|(_, (a, b))| a[..3] != b[..3])
        .map(|(i, _)| i)
        .collect()
}

/// One pixel's RGB triple.
fn pixel(img: &CaptureImage, i: usize) -> [u8; 3] {
    let px = img
        .rgba
        .get(i * 4..i * 4 + 3)
        .unwrap_or_else(|| panic!("pixel {i} is inside the capture"));
    [px[0], px[1], px[2]]
}

/// Every distinct RGB triple in a capture.
fn colours(img: &CaptureImage) -> std::collections::BTreeSet<[u8; 3]> {
    img.rgba
        .chunks_exact(4)
        .map(|px| [px[0], px[1], px[2]])
        .collect()
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

// --- The style and the ink (ADR-0197) --------------------------------------

/// **A hard contour adds no colour the frame did not already have.** That is the
/// whole point of the style: backlog 0140 measured a limited-ink print going from
/// 9 distinct colours to 684 because the soft ramp writes one intermediate value
/// per step of the falloff.
///
/// At `palette_contour = 1` the hard line resolves to `mix(col, black, 1)`, which
/// is black exactly — and the dither fades to zero within one encoded level of a
/// rail, so black stays byte-zero. The soft line at the same strength is the
/// non-vacuity: it *does* invent values the contour-off frame lacks.
#[test]
fn a_hard_contour_adds_no_colour_the_frame_did_not_have() {
    let Some(mut r) = renderer() else { return };
    let off = capture_with(&mut r, &two_runs(), PLATEAU_STEPS, Contour::OFF);
    let soft = capture_with(&mut r, &two_runs(), PLATEAU_STEPS, Contour::styled(1.0, 0));
    let hard = capture_with(&mut r, &two_runs(), PLATEAU_STEPS, Contour::styled(1.0, 1));

    assert!(
        darkened(&off, &hard) > 0,
        "the hard contour drew nothing at all, so what follows would be vacuous"
    );

    let off_colours = colours(&off);
    let soft_new: Vec<[u8; 3]> = colours(&soft).difference(&off_colours).copied().collect();
    assert!(
        !soft_new.is_empty(),
        "the soft contour invented no colour the contour-off frame lacked, so the \
         hard contour's not doing so says nothing"
    );

    let hard_new: Vec<[u8; 3]> = colours(&hard).difference(&off_colours).copied().collect();
    let strays: Vec<[u8; 3]> = hard_new
        .iter()
        .copied()
        .filter(|c| *c != [0, 0, 0])
        .collect();
    assert!(
        strays.is_empty(),
        "the hard contour added {} colours the frame did not have besides black: \
         {strays:?}. The soft one added {} — that difference is the parameter's \
         reason to exist",
        strays.len(),
        soft_new.len()
    );
}

/// **A hard contour covers the soft one's footprint, and barely more.**
///
/// Both styles fire over exactly `d < w`, and ADR-0133's equality rule gates both
/// — so inside a single plateau run the hard line still draws nothing at all, and
/// that half is an exact byte identity.
///
/// The footprint half is asserted as containment **plus a bound**, rather than as
/// the set equality the shapes suggest. The soft ramp's outermost sliver darkens
/// by less than one 8-bit level and so is invisible to any differential, while the
/// step paints it at full strength: the hard set is the soft set plus that fringe,
/// which is a fraction of the line's own width.
#[test]
fn a_hard_contour_draws_where_the_soft_one_does_and_barely_wider() {
    let Some(mut r) = renderer() else { return };
    let off = capture_with(&mut r, &two_runs(), PLATEAU_STEPS, Contour::OFF);
    let soft = capture_with(&mut r, &two_runs(), PLATEAU_STEPS, Contour::styled(1.0, 0));
    let hard = capture_with(&mut r, &two_runs(), PLATEAU_STEPS, Contour::styled(1.0, 1));

    let soft_pixels = darkened_set(&off, &soft);
    let hard_pixels = darkened_set(&off, &hard);
    assert!(
        !hard_pixels.is_empty(),
        "the hard contour darkened nothing at a run boundary"
    );
    let missed: Vec<usize> = soft_pixels.difference(&hard_pixels).copied().collect();
    assert!(
        missed.is_empty(),
        "the hard contour missed {} pixels the soft one darkens, starting at {:?} \
         — the two share one footprint and only differ in how it is filled",
        missed.len(),
        missed.first()
    );
    assert!(
        hard_pixels.len() * 2 <= soft_pixels.len() * 3,
        "the hard contour darkened {} pixels against the soft one's {}. Beyond the \
         sub-level fringe the step fills and the ramp does not, they are the same \
         line",
        hard_pixels.len(),
        soft_pixels.len()
    );

    // ADR-0133's rule holds for every style: no ink change, no line.
    let flat_off = capture_with(&mut r, &one_run(), PLATEAU_STEPS, Contour::OFF);
    let flat_hard = capture_with(&mut r, &one_run(), PLATEAU_STEPS, Contour::styled(1.0, 1));
    assert_eq!(
        flat_off.rgba,
        flat_hard.rgba,
        "a one-colour palette drew a HARD contour: {} pixels moved. The style \
         changes what the line is drawn in, never where it fires",
        darkened(&flat_off, &flat_hard)
    );
}

/// **An ink contour draws in the palette's own colour, to the code value.**
///
/// The comparison is against a capture of the *same probe* whose palette is that
/// one ink end to end: the vignette, the tonemap and the dither are all functions
/// of the pixel's position and its light, so at `palette_contour = 1` in style 3 —
/// where the line resolves to `mix(col, ink, 1)`, the ink exactly — the two frames
/// must agree byte for byte on every pixel the line touches.
///
/// The A/B half is the same claim through `palette_mix = 1`: the ink is crossfaded
/// like every other sample, so the line takes palette B's colour at the same
/// coordinate.
///
/// The footprint is taken from a **style-1** capture of the same probe rather than
/// from the ink capture itself, because an ink the frame already carries is
/// invisible where it is laid over its own run: the line fires at the ink2/ink3
/// boundary on both sides, and only the ink2 side moves. That is the property the
/// style exists for, so the claim is put as *the whole footprint renders as the
/// ink* rather than as *the line changes every pixel it fires at*.
#[test]
fn an_ink_contour_draws_in_the_palettes_own_colour() {
    let Some(mut r) = renderer() else { return };
    let inked = Contour {
        amount: 1.0,
        style: 3,
        ink: INK_COORD,
        mix: 0.0,
    };

    let off = capture_with(&mut r, &three_runs(), PLATEAU_STEPS, Contour::OFF);
    let black = capture_with(
        &mut r,
        &three_runs(),
        PLATEAU_STEPS,
        Contour::styled(1.0, 1),
    );
    let ink = capture_with(&mut r, &three_runs(), PLATEAU_STEPS, inked);
    let reference = capture_with(&mut r, &flat(INK_THREE), PLATEAU_STEPS, Contour::OFF);

    // The whole line, taken from the style that is visible against every ink.
    let footprint = darkened_set(&off, &black);
    assert!(
        !footprint.is_empty(),
        "the contour fired nowhere on a three-plateau palette"
    );
    for &i in &footprint {
        assert_eq!(
            pixel(&ink, i),
            pixel(&reference, i),
            "pixel {i} of the ink contour is {:?}, but the third ink renders \
             {:?} at that position in the same probe. A hard ink line at full \
             strength is that ink and nothing between",
            pixel(&ink, i),
            pixel(&reference, i)
        );
    }
    let strays: Vec<usize> = changed_set(&off, &ink)
        .difference(&footprint)
        .copied()
        .collect();
    assert!(
        strays.is_empty(),
        "the ink contour changed {} pixels outside the line's own footprint, \
         starting at {:?}",
        strays.len(),
        strays.first()
    );

    // ...and the ink is crossfaded A/B like every other sample.
    let b_base = Contour {
        mix: 1.0,
        ..Contour::OFF
    };
    let b_off = capture_with(&mut r, &three_runs_ab(), PLATEAU_STEPS, b_base);
    let b_black = capture_with(
        &mut r,
        &three_runs_ab(),
        PLATEAU_STEPS,
        Contour {
            amount: 1.0,
            style: 1,
            ..b_base
        },
    );
    let b_ink = capture_with(
        &mut r,
        &three_runs_ab(),
        PLATEAU_STEPS,
        Contour { mix: 1.0, ..inked },
    );
    let b_reference = capture_with(&mut r, &flat(INK_THREE_B), PLATEAU_STEPS, Contour::OFF);
    let b_footprint = darkened_set(&b_off, &b_black);
    assert!(
        !b_footprint.is_empty(),
        "at palette_mix = 1 the contour fired nowhere"
    );
    for &i in &b_footprint {
        assert_eq!(
            pixel(&b_ink, i),
            pixel(&b_reference, i),
            "at palette_mix = 1 pixel {i} is {:?} and palette B's ink renders \
             {:?} there. The contour's ink crossfades with everything else",
            pixel(&b_ink, i),
            pixel(&b_reference, i)
        );
    }
    assert_ne!(
        pixel(&b_reference, 0),
        pixel(&reference, 0),
        "palette B's third ink renders the same bytes as palette A's, so the \
         crossfade above would pass without moving"
    );
}

/// **The two new names are scoped, where `palette_contour` is not.**
///
/// `palette_contour` is declared by every palette-coloured scene and is simply
/// inert on the ones that cannot draw it, which is a trap an author falls into
/// silently (`presets/README.md` has to say so beside it). These two are declared
/// only by the six scenes that carry the shader function, so binding one anywhere
/// else is an unknown-parameter warning — the loader saying so rather than the
/// picture not changing.
#[test]
fn the_contour_style_and_ink_are_unknown_off_the_six_scenes() {
    for name in ["palette_contour_style", "palette_contour_ink"] {
        let src =
            format!("system = \"attractor\"\nname = \"scope probe\"\n[params]\n{name} = \"1\"\n");
        let preset = Preset::from_toml_str(&src).expect("an unknown param only warns");
        assert!(
            preset.warnings.iter().any(|w| w.contains(name)),
            "`{name}` bound on `attractor` produced no warning: {:?}. \
             `palette_contour`'s silent inertness is the trap these two do not \
             inherit",
            preset.warnings
        );

        // ...and it IS known on a scene that draws it, or the above would pass
        // for a name the engine has simply never heard of.
        let src = format!(
            "system = \"fragment_field\"\nname = \"scope probe\"\n[params]\n{name} = \"1\"\n"
        );
        let preset = Preset::from_toml_str(&src).expect("the probe parses");
        assert!(
            preset.warnings.is_empty(),
            "`{name}` warned on `fragment_field`, which draws the contour: {:?}",
            preset.warnings
        );
    }
}
