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
