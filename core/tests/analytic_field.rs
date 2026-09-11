//! The analytic field (ADR-0180 rule 1): one fullscreen pass whose output is a
//! closed-form function of position.
//!
//! # What is compared against what
//!
//! The Chladni plate's figure is a closed form, so the test states it
//! independently: [`chladni`] below is the textbook formula written out on the
//! CPU, and a capture is held to the mask it predicts. The two agree only where
//! the pass takes its coordinates the way the formula does — the short axis
//! spanning `[-1, 1]` and the aspect taken from the render target (ADR-0037) —
//! which is the property under test, not an incidental one.
//!
//! Pixels within one pixel of the band's edge are left out of every mask
//! comparison: which side of a threshold an antialiased edge lands on is the
//! rasterizer's business, and a claim about it would be a claim about WARP.
//!
//! **Software adapter**, like the rest of the GPU suites (ADR-0016).

mod common;

use rlx_core::dsp::AnalysisFrame;
use rlx_core::preset::{Preset, PresetError, SystemKind};
use rlx_core::render::{CaptureImage, Renderer};

/// Frames per capture. The field holds no state, so one frame is the whole
/// picture; two keeps the clock off zero.
const FRAMES: u32 = 2;

/// A palette with no gradient in it, so every lit pixel is the same colour and
/// a capture's brightness is the field's light and nothing else.
const WHITE: &str = "[palette]\nstops = [\
    { at = 0.0, color = \"#ffffff\" }, \
    { at = 1.0, color = \"#ffffff\" }]\n";

/// The plate the done-when names.
const MODES: (f32, f32) = (3.0, 5.0);

/// The band's full width in plate units, as the fixture binds it.
const LINE_WIDTH: f32 = 0.05;

/// A Chladni preset over [`WHITE`], lines only.
fn plate(n: f32, m: f32, extra: &str) -> String {
    format!(
        "system = \"analytic_field\"\nname = \"plate\"\n{WHITE}\
         [field]\nfamily = \"chladni\"\n\
         [params]\nmode_n = \"{n}\"\nmode_m = \"{m}\"\n\
         line_width = \"{LINE_WIDTH}\"\nplate_mix = \"0\"\n{extra}"
    )
}

/// One capture of `toml` on `renderer`.
fn capture(renderer: &mut Renderer, toml: &str) -> CaptureImage {
    let preset = Preset::from_toml_str(toml).expect("the probe preset loads");
    let name = preset.name.clone();
    renderer.set_presets(vec![preset]);
    renderer
        .capture_preset(&name, &AnalysisFrame::default(), FRAMES)
        .expect("capture the probe")
}

/// Rec. 601 luma of pixel `(x, y)`, `0..=255`.
fn luma(img: &CaptureImage, x: u32, y: u32) -> f32 {
    let i = ((y * img.width + x) * 4) as usize;
    0.299 * f32::from(img.rgba[i])
        + 0.587 * f32::from(img.rgba[i + 1])
        + 0.114 * f32::from(img.rgba[i + 2])
}

/// The textbook plate, `cos(n pi x) cos(m pi y) - cos(m pi x) cos(n pi y)` over
/// the unit plate `[0, 1]^2`, and its first-order distance to the nodal set in
/// the field's `[-1, 1]` units.
fn chladni(px: f32, py: f32, n: f32, m: f32) -> f32 {
    use std::f32::consts::PI;
    let (qx, qy) = ((px + 1.0) * 0.5, (py + 1.0) * 0.5);
    let f = (n * PI * qx).cos() * (m * PI * qy).cos() - (m * PI * qx).cos() * (n * PI * qy).cos();
    let gx = 0.5
        * PI
        * (-n * (n * PI * qx).sin() * (m * PI * qy).cos()
            + m * (m * PI * qx).sin() * (n * PI * qy).cos());
    let gy = 0.5
        * PI
        * (-m * (n * PI * qx).cos() * (m * PI * qy).sin()
            + n * (m * PI * qx).cos() * (n * PI * qy).sin());
    f.abs() / gx.hypot(gy).max(1e-4)
}

/// The field coordinate of pixel `(x, y)`'s centre on a `w` x `h` target at
/// `zoom`: the short axis spans `[-1, 1]`, y up.
fn field_at(x: u32, y: u32, w: u32, h: u32, zoom: f32) -> (f32, f32) {
    let nx = (x as f32 + 0.5) / w as f32 * 2.0 - 1.0;
    let ny = 1.0 - (y as f32 + 0.5) / h as f32 * 2.0;
    (nx * w as f32 / h as f32 / zoom, ny / zoom)
}

/// How a capture's lit pixels compare with the mask the formula predicts.
struct MaskReading {
    /// Pixels far enough from the band's edge to be decided.
    decided: usize,
    /// Of those, how many the capture and the formula agree on.
    agree: usize,
    /// The fraction of decided pixels the formula lights.
    lit: f32,
}

/// Hold `img` to the plate `(n, m)` predicts at `zoom`.
fn read_mask(img: &CaptureImage, n: f32, m: f32, zoom: f32) -> MaskReading {
    let peak = (0..img.height)
        .flat_map(|y| (0..img.width).map(move |x| (x, y)))
        .map(|(x, y)| luma(img, x, y))
        .fold(0.0f32, f32::max);
    assert!(peak > 32.0, "the plate drew nothing (peak luma {peak})");
    let pixel = 2.0 / (zoom * img.height as f32);
    let half = 0.5 * LINE_WIDTH;
    let (mut decided, mut agree, mut lit) = (0usize, 0usize, 0usize);
    for y in 0..img.height {
        for x in 0..img.width {
            let (px, py) = field_at(x, y, img.width, img.height, zoom);
            let dist = chladni(px, py, n, m);
            if (dist - half).abs() < pixel {
                continue;
            }
            decided += 1;
            let predicted = dist < half;
            if predicted {
                lit += 1;
            }
            if (luma(img, x, y) > 0.5 * peak) == predicted {
                agree += 1;
            }
        }
    }
    MaskReading {
        decided,
        agree,
        lit: lit as f32 / decided.max(1) as f32,
    }
}

/// **`mode_n = 3`, `mode_m = 5` draws that plate's nodal figure** — held to the
/// formula pixel by pixel, with a control proving the comparison can fail.
#[test]
fn modes_three_and_five_draw_that_plates_nodal_figure() {
    const SIZE: u32 = 128;
    let Some(mut renderer) = common::headless(SIZE, SIZE) else {
        return;
    };
    let img = capture(&mut renderer, &plate(MODES.0, MODES.1, ""));
    let reading = read_mask(&img, MODES.0, MODES.1, 1.0);
    let agreement = reading.agree as f32 / reading.decided as f32;
    println!(
        "3,5 plate: {} decided pixels, {:.4} agree, {:.3} lit",
        reading.decided, agreement, reading.lit
    );
    assert!(
        reading.lit > 0.03 && reading.lit < 0.6,
        "the predicted figure lights {:.3} of the frame — a blank or a flooded \
         plate cannot tell a right figure from a wrong one",
        reading.lit
    );
    assert!(
        agreement > 0.99,
        "only {agreement:.4} of the decided pixels match the 3,5 plate's nodal set"
    );

    // The control: the same capture against a different plate must disagree
    // widely, or the agreement above is measuring nothing.
    let other = read_mask(&img, 2.0, 4.0, 1.0);
    let control = other.agree as f32 / other.decided as f32;
    println!("the 3,5 capture read as a 2,4 plate: {control:.4} agree");
    assert!(
        control < 0.9,
        "the 3,5 capture also matches the 2,4 plate at {control:.4}, so the mask \
         comparison does not discriminate between figures"
    );

    // And a binding really is what picks the figure: the 2,4 plate rendered is
    // the 2,4 plate predicted.
    let img24 = capture(&mut renderer, &plate(2.0, 4.0, ""));
    let reading24 = read_mask(&img24, 2.0, 4.0, 1.0);
    assert!(
        reading24.agree as f32 / reading24.decided as f32 > 0.99,
        "the 2,4 plate does not match its own formula"
    );
}

/// **The figure is square on a non-square target** (ADR-0037): at 1280x800 the
/// central 800x800 is the same picture as an 800x800 render, and the 3,5
/// plate's own symmetry — `|f(x, y)| = |f(y, x)|` — holds across the diagonal
/// in pixels.
///
/// A field that took its aspect from anything but the target would stretch the
/// plate horizontally at 16:10 and fail both halves; at a square target it would
/// pass them, which is why the size is 1280x800 and not 16:9's more common
/// square-ish captures.
#[test]
fn the_plate_is_square_on_a_non_square_target() {
    let Some(mut wide) = common::headless(1280, 800) else {
        return;
    };
    let toml = plate(MODES.0, MODES.1, "");
    let wide_img = capture(&mut wide, &toml);
    drop(wide);
    let Some(mut square) = common::headless(800, 800) else {
        return;
    };
    let square_img = capture(&mut square, &toml);

    // The wide capture also matches the formula over its whole frame, edges
    // included, which is the aspect claim on its own.
    let reading = read_mask(&wide_img, MODES.0, MODES.1, 1.0);
    let agreement = reading.agree as f32 / reading.decided as f32;
    println!("1280x800 against the formula: {agreement:.4}");
    assert!(
        agreement > 0.99,
        "at 1280x800 only {agreement:.4} of the frame matches the plate — the \
         figure is not being drawn in square units"
    );

    // Centre crop against the square render.
    let offset = (1280 - 800) / 2;
    let (mut differing, mut total) = (0usize, 0usize);
    for y in 0..800 {
        for x in 0..800 {
            total += 1;
            let (a, b) = (luma(&wide_img, x + offset, y), luma(&square_img, x, y));
            if (a - b).abs() > 24.0 {
                differing += 1;
            }
        }
    }
    let share = differing as f32 / total as f32;
    println!("centre crop vs 800x800: {share:.5} of pixels differ");
    assert!(
        share < 0.005,
        "the centre of the 1280x800 frame differs from the 800x800 render on \
         {share:.5} of its pixels — the plate's scale depends on the target's \
         width"
    );

    // The diagonal symmetry, read in pixels of the wide frame's central square.
    let (mut asymmetric, mut compared) = (0usize, 0usize);
    for y in 0..800u32 {
        for x in (y + 1)..800u32 {
            compared += 1;
            let (a, b) = (
                luma(&wide_img, x + offset, y),
                luma(&wide_img, (799 - y) + offset, 799 - x),
            );
            if (a - b).abs() > 24.0 {
                asymmetric += 1;
            }
        }
    }
    let share = asymmetric as f32 / compared as f32;
    println!("diagonal asymmetry: {share:.5}");
    assert!(
        share < 0.005,
        "the 3,5 plate is not symmetric about its diagonal in pixels ({share:.5} \
         of pairs differ), so one field unit is not the same number of pixels on \
         both axes"
    );
}

/// The mean absolute luma difference between two same-sized captures, and the
/// fraction of pixels where `lit` is brighter than `dark` by more than two
/// code values.
fn brighter(lit: &CaptureImage, dark: &CaptureImage) -> (f32, f32) {
    let (mut sum, mut raised, mut n) = (0.0f32, 0usize, 0usize);
    for y in 0..lit.height {
        for x in 0..lit.width {
            let (a, b) = (luma(lit, x, y), luma(dark, x, y));
            sum += (a - b).abs();
            if a > b + 2.0 {
                raised += 1;
            }
            n += 1;
        }
    }
    (sum / n as f32, raised as f32 / n as f32)
}

/// What lighting the backdrop does to one preset at the two ends of `occlude`:
/// how far it moves the frame at `occlude = 1`, and how much of the frame it
/// raises at `occlude = 0`.
fn occlude_reading(renderer: &mut Renderer, system: &str, body: &str, stage: &str) -> (f32, f32) {
    let at = |occlude: f32, sky: f32| {
        format!(
            "system = \"{system}\"\nname = \"occ\"\n{body}{stage}\
             occlude = \"{occlude}\"\nbg_bright = \"{sky}\"\nbg_hue = \"0.6\"\n"
        )
    };
    let covered_lit = capture(renderer, &at(1.0, 0.8));
    let covered_dark = capture(renderer, &at(1.0, 0.0));
    let (covered_moved, _) = brighter(&covered_lit, &covered_dark);
    let adding_lit = capture(renderer, &at(0.0, 0.8));
    let adding_dark = capture(renderer, &at(0.0, 0.0));
    let (_, adding_raised) = brighter(&adding_lit, &adding_dark);
    (covered_moved, adding_raised)
}

/// **`occlude` behaves as it does on `fragment_field`** — on both of the two
/// paths it can reach, asserted as a comparison between the two systems rather
/// than as a recollection of what the reference does.
///
/// - **A post stage active** (`trails` bound): the chain's last fold owns the
///   seam (ADR-0085). At `occlude = 1` lighting the sky changes nothing; at `0`
///   it raises the frame. Asserted outright on both systems, which is what makes
///   the comparison non-vacuous.
/// - **No stage active**: the scene lands on the backdrop itself, and here the
///   two fields must simply agree. Both draw with a replacing blend, so the sky
///   under them does not show at either end — the comparison holds the new
///   field to what the reference does, whatever that is.
#[test]
fn occlude_behaves_as_it_does_on_fragment_field() {
    const SIZE: u32 = 64;
    let Some(mut renderer) = common::headless(SIZE, SIZE) else {
        return;
    };
    let field = "[field]\nfamily = \"chladni\"\n[params]\nmode_n = \"3\"\nmode_m = \"5\"\n";
    let fold = "[params]\nwarp = \"0.5\"\n";
    for (path, stage) in [("stage active", "trails = \"0.5\"\n"), ("no stage", "")] {
        let analytic = occlude_reading(&mut renderer, "analytic_field", field, stage);
        let reference = occlude_reading(&mut renderer, "fragment_field", fold, stage);
        println!(
            "{path}: analytic_field moves {:.4} at occlude 1 and raises {:.3} at 0; \
             fragment_field moves {:.4} and raises {:.3}",
            analytic.0, analytic.1, reference.0, reference.1
        );
        let covers = |reading: (f32, f32)| reading.0 < 0.05;
        let adds = |reading: (f32, f32)| reading.1 > 0.3;
        assert_eq!(
            (covers(analytic), adds(analytic)),
            (covers(reference), adds(reference)),
            "{path}: the analytic field's occlude ({analytic:?}) does not behave \
             as fragment_field's does ({reference:?})"
        );
        if !stage.is_empty() {
            assert!(
                covers(reference) && adds(reference),
                "{path}: fragment_field no longer covers at occlude 1 and adds at \
                 0 ({reference:?}), so the comparison above compares nothing"
            );
        }
    }
}

/// **An unknown family name is a load error**, naming the roster; an absent
/// `[field]` table is the default family, as an absent `[particles]` is De Jong.
#[test]
fn an_unknown_family_is_a_load_error() {
    let err = Preset::from_toml_str("system = \"analytic_field\"\n[field]\nfamily = \"chladny\"\n")
        .expect_err("a misspelled family must not load");
    match err {
        PresetError::Config(message) => {
            assert!(
                message.contains("chladny") && message.contains("chladni"),
                "the error must name the bad value and the roster: {message}"
            );
        }
        other => panic!("expected a config error, got {other:?}"),
    }

    let bare = Preset::from_toml_str("system = \"analytic_field\"\n")
        .expect("an analytic field with no [field] table loads");
    assert_eq!(bare.system, SystemKind::AnalyticField);
    assert!(bare.warnings.is_empty(), "{:?}", bare.warnings);
    let named = Preset::from_toml_str(&plate(3.0, 5.0, "")).expect("the plate loads");
    assert!(named.warnings.is_empty(), "{:?}", named.warnings);
}

// ---------------------------------------------------------------------------
// Escape time
// ---------------------------------------------------------------------------

/// A Julia constant inside the Mandelbrot set: the main cardioid's
/// `mu / 2 - mu^2 / 4` at `mu = 0.7 e^(2i)`, well inside `|mu| < 1`, so its
/// filled Julia set is one quasi-disk with no pinch point a grid could split.
const C_INSIDE: (f32, f32) = (-0.066, 0.411);

/// A constant outside it: its own orbit leaves `|z| = 2` by the fourth step,
/// so its Julia set is a Cantor dust of measure zero.
const C_OUTSIDE: (f32, f32) = (0.534, 0.411);

/// The escape-time view every test here shares: the plane `[-1.67, 1.67]^2`,
/// wide enough to hold either set whole.
const ESCAPE_ZOOM: f32 = 0.6;

/// An escape-time preset over `palette`, the set's interior black.
fn julia(c_re: &str, c_im: &str, palette: &str, extra: &str) -> String {
    format!(
        "system = \"analytic_field\"\nname = \"julia\"\n{palette}\
         [field]\nfamily = \"escape_time\"\nmap = \"julia\"\n\
         [params]\nc_re = \"{c_re}\"\nc_im = \"{c_im}\"\niterations = \"64\"\n\
         escape_radius = \"16\"\ninterior = \"0\"\nzoom = \"{ESCAPE_ZOOM}\"\n{extra}"
    )
}

/// The integer step at which `z -> z^2 + c` from `p` passes radius 16, or
/// `None` within 64 steps — the textbook iteration, written independently of
/// the shader.
fn escape_step(p: (f32, f32), c: (f32, f32)) -> Option<u32> {
    let (mut x, mut y) = p;
    for n in 1..=64 {
        (x, y) = (x * x - y * y + c.0, 2.0 * x * y + c.1);
        if x * x + y * y > 256.0 {
            return Some(n);
        }
    }
    None
}

/// The dark pixels of `img` — the set's interior — as a mask.
fn interior_mask(img: &CaptureImage) -> Vec<bool> {
    (0..img.height)
        .flat_map(|y| (0..img.width).map(move |x| (x, y)))
        .map(|(x, y)| luma(img, x, y) < 64.0)
        .collect()
}

/// The sizes of `mask`'s 4-connected components, largest first.
fn components(mask: &[bool], width: usize) -> Vec<usize> {
    let mut seen = vec![false; mask.len()];
    let mut sizes = Vec::new();
    for start in 0..mask.len() {
        if !mask[start] || seen[start] {
            continue;
        }
        let (mut size, mut stack) = (0usize, vec![start]);
        seen[start] = true;
        while let Some(i) = stack.pop() {
            size += 1;
            let (x, y) = (i % width, i / width);
            let mut visit = |j: usize| {
                if mask[j] && !seen[j] {
                    seen[j] = true;
                    stack.push(j);
                }
            };
            if x > 0 {
                visit(i - 1);
            }
            if x + 1 < width {
                visit(i + 1);
            }
            if y > 0 {
                visit(i - width);
            }
            if i + width < mask.len() {
                visit(i + width);
            }
        }
        sizes.push(size);
    }
    sizes.sort_unstable_by(|a, b| b.cmp(a));
    sizes
}

/// The interior's share of the frame, and the share of the interior its
/// largest connected piece holds.
fn topology(img: &CaptureImage) -> (f32, f32) {
    let mask = interior_mask(img);
    let sizes = components(&mask, img.width as usize);
    let interior: usize = sizes.iter().sum();
    let largest = sizes.first().copied().unwrap_or(0);
    (
        interior as f32 / mask.len() as f32,
        largest as f32 / interior.max(1) as f32,
    )
}

/// **A Julia preset renders a connected filled set for `c` inside the
/// Mandelbrot set and a dust for `c` outside it** — and the set drawn is the
/// textbook one, held to an independent iteration pixel by pixel.
#[test]
fn a_julia_set_is_connected_for_c_inside_the_mandelbrot_set_and_dust_outside() {
    const SIZE: u32 = 128;
    let Some(mut renderer) = common::headless(SIZE, SIZE) else {
        return;
    };
    let inside = capture(
        &mut renderer,
        &julia(&C_INSIDE.0.to_string(), &C_INSIDE.1.to_string(), WHITE, ""),
    );
    let outside = capture(
        &mut renderer,
        &julia(
            &C_OUTSIDE.0.to_string(),
            &C_OUTSIDE.1.to_string(),
            WHITE,
            "",
        ),
    );
    let (inside_share, inside_largest) = topology(&inside);
    let (outside_share, _) = topology(&outside);
    println!(
        "c inside: interior {inside_share:.4} of the frame, its largest piece {inside_largest:.4}; \
         c outside: interior {outside_share:.5}"
    );
    assert!(
        inside_share > 0.05,
        "c inside the Mandelbrot set filled only {inside_share:.4} of the frame"
    );
    assert!(
        inside_largest > 0.99,
        "c inside the Mandelbrot set drew a set in pieces: the largest holds only \
         {inside_largest:.4} of the interior"
    );
    assert!(
        outside_share < 0.002,
        "c outside the Mandelbrot set left {outside_share:.5} of the frame interior — \
         its Julia set is dust and should be almost nowhere"
    );

    // The set drawn is the set the textbook iteration defines.
    let (mut agree, mut decided) = (0usize, 0usize);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let p = field_at(x, y, SIZE, SIZE, ESCAPE_ZOOM);
            let step = escape_step(p, C_INSIDE);
            // Orbits that escape on the last couple of steps sit on the
            // boundary, where the last rounding decides; leave them out.
            if step.is_some_and(|n| n > 62) {
                continue;
            }
            decided += 1;
            if (luma(&inside, x, y) < 64.0) == step.is_none() {
                agree += 1;
            }
        }
    }
    let agreement = agree as f32 / decided as f32;
    println!("the drawn set against the textbook iteration: {agreement:.4}");
    assert!(
        agreement > 0.99,
        "only {agreement:.4} of the frame matches the textbook Julia set for c inside"
    );
}

/// A grey ramp: palette coordinate `t` is grey level `t`, so a capture's
/// brightness reads the palette coordinate back through the tonemap.
const GREY: &str = "[palette]\nstops = [\
    { at = 0.0, color = \"#000000\" }, \
    { at = 1.0, color = \"#ffffff\" }]\n";

/// **The palette across the escape boundary is free of integer banding at
/// `palette_steps = 0`.**
///
/// `color_span = 4` makes one iteration an eighth of the palette — about thirty
/// grey levels — so an integer count would print a hard edge wherever the
/// escape step changes. Those places are found independently (the textbook
/// iteration), and at each the capture's step across the pair is compared with
/// its steps across the pairs either side: a smooth count moves across an
/// integer step no faster than the slope around it.
///
/// **The control is a banded picture by construction.** The smooth count is a
/// whole number exactly where the escape step changes (`|z_n| = R`), so at
/// `color_span = 4` a `palette_steps = 8` cuts the palette at every one of
/// those places — the integer count's picture, drawn through the same pass.
/// The same statistic must convict it, or it could not convict banding.
#[test]
fn the_escape_boundary_is_free_of_integer_banding() {
    const SIZE: u32 = 256;
    let Some(mut renderer) = common::headless(SIZE, SIZE) else {
        return;
    };
    let smooth = capture(
        &mut renderer,
        &julia(
            &C_INSIDE.0.to_string(),
            &C_INSIDE.1.to_string(),
            GREY,
            "color_span = \"4\"\npalette_steps = \"0\"\n",
        ),
    );
    let banded = capture(
        &mut renderer,
        &julia(
            &C_INSIDE.0.to_string(),
            &C_INSIDE.1.to_string(),
            GREY,
            "color_span = \"4\"\npalette_steps = \"8\"\n",
        ),
    );
    let (judged, worst_excess, span) = worst_step_excess(&smooth);
    let (_, banded_excess, _) = worst_step_excess(&banded);
    println!(
        "{judged} integer steps judged; the smooth palette exceeds its local slope by at \
         most {worst_excess:.2} grey levels over a span {:.0}..{:.0}; the banded control by \
         {banded_excess:.2}",
        span.0, span.1
    );
    assert!(
        judged >= 20,
        "only {judged} integer steps were judged, too few to show banding"
    );
    assert!(
        span.1 - span.0 > 40.0,
        "the judged pixels span only {:.0} grey levels, so a band edge could hide in them",
        span.1 - span.0
    );
    assert!(
        banded_excess > 16.0,
        "the banded control exceeds its slope by only {banded_excess:.2}, so this \
         statistic cannot see an integer band edge"
    );
    assert!(
        worst_excess < 8.0,
        "across an integer escape step the palette jumps {worst_excess:.2} grey levels past \
         its local slope — the boundary is banded"
    );
}

/// Over every place the escape step changes by one between two exterior
/// neighbours, how far the capture's jump across the pair exceeds its jumps
/// across the pairs either side: `(places judged, worst excess, luma span)`.
fn worst_step_excess(img: &CaptureImage) -> (usize, f32, (f32, f32)) {
    let size = img.width;
    let (mut judged, mut worst_excess, mut span) = (0usize, 0.0f32, (255.0f32, 0.0f32));
    for y in 0..size {
        let row: Vec<Option<u32>> = (0..size)
            .map(|x| escape_step(field_at(x, y, size, size, ESCAPE_ZOOM), C_INSIDE))
            .collect();
        for x in 2..size - 1 {
            let (a, b) = (row[(x - 1) as usize], row[x as usize]);
            // Exterior on both sides, one integer step apart, and early
            // enough that 4 * nu / 32 has not wrapped the repeat-addressed
            // palette.
            let (Some(a), Some(b)) = (a, b) else { continue };
            if a == b || a.max(b) > 6 {
                continue;
            }
            // The two neighbouring pairs must each sit inside one step.
            if row[(x - 2) as usize] != Some(a) || row[(x + 1) as usize] != Some(b) {
                continue;
            }
            let l = |i: u32| luma(img, i, y);
            let jump = (l(x) - l(x - 1)).abs();
            let slope = (l(x - 1) - l(x - 2)).abs().max((l(x + 1) - l(x)).abs());
            judged += 1;
            worst_excess = worst_excess.max(jump - slope);
            span = (span.0.min(l(x)), span.1.max(l(x)));
        }
    }
    (judged, worst_excess, span)
}

/// **A bass-driven `c_re` visibly morphs the set's topology**: silence holds
/// `c` inside the Mandelbrot set and the set is one piece; full bass carries it
/// outside and the set falls to dust. The same preset, two analysis frames.
#[test]
fn a_bass_driven_c_re_morphs_the_topology() {
    const SIZE: u32 = 128;
    let Some(mut renderer) = common::headless(SIZE, SIZE) else {
        return;
    };
    let c_re = format!("{} + {} * bass", C_INSIDE.0, C_OUTSIDE.0 - C_INSIDE.0);
    let toml = julia(&c_re, &C_INSIDE.1.to_string(), WHITE, "");
    let preset = Preset::from_toml_str(&toml).expect("the bass-driven Julia loads");
    renderer.set_presets(vec![preset]);
    let at = |renderer: &mut Renderer, bass: f32| {
        let frame = AnalysisFrame {
            bass,
            ..Default::default()
        };
        renderer
            .capture_preset("julia", &frame, FRAMES)
            .expect("capture the bass-driven Julia")
    };
    let quiet = topology(&at(&mut renderer, 0.0));
    let loud = topology(&at(&mut renderer, 1.0));
    println!(
        "bass 0: interior {:.4}, largest piece {:.4}; bass 1: interior {:.5}",
        quiet.0, quiet.1, loud.0
    );
    assert!(
        quiet.0 > 0.05 && quiet.1 > 0.99,
        "at silence the set should be one filled piece: {quiet:?}"
    );
    assert!(
        loud.0 < 0.002,
        "at full bass the set should be dust: interior {:.5}",
        loud.0
    );
}

/// `[field] map` is escape time's alone, and names its roster when it is
/// misspelled — both load errors, never a silent default.
#[test]
fn the_escape_map_is_validated_at_load() {
    let on_plate = Preset::from_toml_str(
        "system = \"analytic_field\"\n[field]\nfamily = \"chladni\"\nmap = \"julia\"\n",
    )
    .expect_err("a map on the plate must not load");
    assert!(
        on_plate.to_string().contains("escape_time"),
        "the error must say which family a map belongs to: {on_plate}"
    );
    let unknown = Preset::from_toml_str(
        "system = \"analytic_field\"\n[field]\nfamily = \"escape_time\"\nmap = \"fatou\"\n",
    )
    .expect_err("an unknown map must not load");
    let message = unknown.to_string();
    assert!(
        message.contains("fatou") && message.contains("mandelbrot"),
        "the error must name the bad value and the roster: {message}"
    );
    for map in ["julia", "mandelbrot"] {
        Preset::from_toml_str(&format!(
            "system = \"analytic_field\"\n[field]\nfamily = \"escape_time\"\nmap = \"{map}\"\n"
        ))
        .unwrap_or_else(|e| panic!("map = {map} must load: {e}"));
    }
}
