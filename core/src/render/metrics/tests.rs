// Test bodies index and unwrap freely — not the hot path.
#![allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used)]

use super::*;

const BLACK: [u8; 4] = [0, 0, 0, 255];

/// Build a `w`×`h` image by painting each pixel from `f(x, y) -> [r,g,b,a]`.
fn image(w: u32, h: u32, f: impl Fn(u32, u32) -> [u8; 4]) -> CaptureImage {
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            rgba.extend_from_slice(&f(x, y));
        }
    }
    CaptureImage {
        width: w,
        height: h,
        rgba,
    }
}

fn solid(w: u32, h: u32, color: [u8; 4]) -> CaptureImage {
    image(w, h, |_, _| color)
}

/// A vertical bar in `color` over black covering `x < w/2`.
fn left_bar(w: u32, h: u32, color: [u8; 4]) -> CaptureImage {
    image(w, h, |x, _| if x < w / 2 { color } else { BLACK })
}

/// A horizontal bar in `color` over black covering `y < h/2`.
fn top_bar(w: u32, h: u32, color: [u8; 4]) -> CaptureImage {
    image(w, h, |_, y| if y < h / 2 { color } else { BLACK })
}

#[test]
fn frame_diff_bounds() {
    let black = solid(32, 32, BLACK);
    let white = solid(32, 32, [255, 255, 255, 255]);
    assert_eq!(frame_diff(&black, &black), 0.0);
    assert_eq!(frame_diff(&black, &white), 1.0);
    // Mismatched sizes read as fully different.
    assert_eq!(frame_diff(&black, &solid(16, 16, BLACK)), 1.0);
}

#[test]
fn footprint_diff_measures_over_the_mask_not_the_frame() {
    let w = 32;
    // A 4-pixel white dot at x=0..4 in row 0, then the same dot moved to
    // x=8..12: the union mask is 8 pixels, each fully swung on 3 channels.
    let dot_at = |x0: u32| {
        image(w, w, |x, y| {
            if y == 0 && x >= x0 && x < x0 + 4 {
                [255, 255, 255, 255]
            } else {
                BLACK
            }
        })
    };
    let (a, b) = (dot_at(0), dot_at(8));

    // Whole-frame: 8 changed pixels diluted by 1024. Masked (zero floor): the
    // mean over exactly the 8 mask pixels is a full swing.
    let whole = frame_diff(&a, &b);
    assert!(whole < 0.01, "whole-frame dilutes the dot: {whole}");
    assert_eq!(footprint_diff(&a, &b, BLACK, 8, 0.0), 1.0);

    // The denominator floor caps what a tiny mask can read: at a floor of half
    // the frame, 8 fully-swung pixels over 512 counted ones.
    let floored = footprint_diff(&a, &b, BLACK, 8, 0.5);
    assert!((floored - 8.0 / 512.0).abs() < 1e-6, "floored: {floored}");

    // Identical frames have a zero numerator whatever the mask — the ADR-0091
    // safety argument — and an all-black pair reads 0.0, not a division.
    assert_eq!(footprint_diff(&a, &a, BLACK, 8, 0.0), 0.0);
    let black = solid(w, w, BLACK);
    assert_eq!(footprint_diff(&black, &black, BLACK, 8, 0.0), 0.0);

    // Mismatched sizes read as fully different, like `frame_diff`.
    assert_eq!(
        footprint_diff(&a, &solid(16, 16, BLACK), BLACK, 8, 0.0),
        1.0
    );
}

/// The concentration reading has to move the right way when light *piles up*,
/// which is the whole reason it exists (Plan 0085 Phase 1): the same total
/// energy gathered into fewer pixels must read higher, and spread back out must
/// read lower.
#[test]
fn peak_to_mean_rises_as_the_same_light_concentrates() {
    let w = 32;
    let total = (w * w) as f32;
    // `n` lit pixels at `value`, black elsewhere — the energy is `n * value`.
    let spread = |n: u32, value: u8| {
        image(w, w, |x, y| {
            if y * w + x < n {
                [value, value, value, 255]
            } else {
                BLACK
            }
        })
    };

    // Same energy (64 x 60 = 3840 vs 16 x 240 = 3840), four times the
    // concentration. Both are well below saturation, so the numerator is free
    // to move.
    let wide = peak_to_mean(&spread(64, 60), BLACK, 8);
    let tight = peak_to_mean(&spread(16, 240), BLACK, 8);
    assert!(
        tight > wide * 3.5,
        "concentrating the same light must raise the ratio: {wide} -> {tight}"
    );

    // The two endpoints of the range, exactly. A uniform lit frame is 1.0 (peak
    // equals mean); one lit pixel is the pixel count (all the light in 1/N).
    let uniform = peak_to_mean(&solid(w, w, [200, 200, 200, 255]), BLACK, 8);
    assert!((uniform - 1.0).abs() < 1e-4, "uniform frame: {uniform}");
    let single = peak_to_mean(&spread(1, 255), BLACK, 8);
    assert!((single - total).abs() < 0.5, "one lit pixel: {single}");

    // An empty frame makes no claim rather than dividing by zero, and neither
    // does one that is lit *by* its own background.
    assert_eq!(peak_to_mean(&solid(w, w, BLACK), BLACK, 8), 0.0);
    let grey = [90, 90, 90, 255];
    assert_eq!(peak_to_mean(&solid(w, w, grey), grey, 8), 0.0);
    assert_eq!(
        peak_to_mean(&image(0, 0, |_, _| BLACK), BLACK, 8),
        0.0,
        "a zero-pixel frame is not a division"
    );

    // Measured against the frame's *own* ground, not absolute black: the same
    // figure over a lifted backdrop reads the same, which is what lets a
    // vignetted world be compared with a dark one.
    let on_black = image(w, w, |x, y| {
        if y * w + x < 16 {
            [240, 240, 240, 255]
        } else {
            BLACK
        }
    });
    let on_grey = image(w, w, |x, y| {
        if y * w + x < 16 {
            [255, 255, 255, 255]
        } else {
            grey
        }
    });
    let (a, b) = (
        peak_to_mean(&on_black, BLACK, 8),
        peak_to_mean(&on_grey, grey, 8),
    );
    assert!(
        (a - b).abs() < 0.5,
        "the ground must cancel: {a} on black, {b} on grey"
    );

    // A **dark** figure on a light ground is the same event, and reads the same
    // number. Without the absolute value an ink world (ADR-0106) would report
    // 0.0 for every frame of a horizon and its drift would be invisible.
    let white = [255, 255, 255, 255];
    let ink = image(w, w, |x, y| {
        if y * w + x < 16 {
            [15, 15, 15, 255]
        } else {
            white
        }
    });
    let dark = peak_to_mean(&ink, white, 8);
    assert!(
        (dark - a).abs() < 0.5,
        "a dark figure on a light ground must read like its inverse: {a} vs {dark}"
    );

    // The deadband keeps near-ground noise out of the denominator: a dithered
    // ground one code value off black must not read as spread-out light.
    let dithered = image(w, w, |x, y| {
        if y * w + x < 16 {
            [240, 240, 240, 255]
        } else if (x + y) % 2 == 0 {
            [1, 1, 1, 255]
        } else {
            BLACK
        }
    });
    let noisy = peak_to_mean(&dithered, BLACK, 8);
    assert!(
        (noisy - a).abs() < 1e-4,
        "sub-eps dither changed the reading: {a} -> {noisy}"
    );
}

#[test]
fn coverage_and_spread_extremes() {
    let black = solid(32, 32, BLACK);
    let white = solid(32, 32, [255, 255, 255, 255]);
    assert_eq!(coverage(&black, BLACK, 8), 0.0);
    assert_eq!(coverage(&white, BLACK, 8), 1.0);
    assert_eq!(quadrant_spread(&black, BLACK, 8), 0);
    assert_eq!(quadrant_spread(&white, BLACK, 8), 4);

    // A single lit pixel in the top-left quadrant hits exactly one quadrant.
    let dot = image(32, 32, |x, y| {
        if x == 2 && y == 2 {
            [255, 255, 255, 255]
        } else {
            BLACK
        }
    });
    assert_eq!(quadrant_spread(&dot, BLACK, 8), 1);
}

#[test]
fn radial_shell_occupancy_extremes_and_geometry() {
    let black = solid(96, 96, BLACK);
    let white = solid(96, 96, [255, 255, 255, 255]);
    // Nothing lit occupies nothing; everything lit occupies every shell.
    assert_eq!(radial_shell_occupancy(&black, BLACK, 8), 0);
    assert_eq!(radial_shell_occupancy(&white, BLACK, 8), RADIAL_SHELLS);

    // Normalized radius from the frame centre, in units of the inscribed disc.
    let r_norm = |x: u32, y: u32| {
        let (dx, dy) = (x as f32 + 0.5 - 48.0, y as f32 + 0.5 - 48.0);
        (dx * dx + dy * dy).sqrt() / 48.0
    };

    // A thin ring occupies exactly the one shell its radius lives in — the
    // structural claim: a figure at one radius is one shell, however bright.
    let ring = image(96, 96, |x, y| {
        let r = r_norm(x, y);
        if (0.52..0.58).contains(&r) {
            [255, 255, 255, 255]
        } else {
            BLACK
        }
    });
    assert_eq!(radial_shell_occupancy(&ring, BLACK, 8), 1);

    // Light outside the inscribed disc (the corners) is at radii the measure
    // deliberately does not score.
    let corners = image(96, 96, |x, y| {
        if r_norm(x, y) >= 1.0 {
            [255, 255, 255, 255]
        } else {
            BLACK
        }
    });
    assert_eq!(radial_shell_occupancy(&corners, BLACK, 8), 0);
}

/// One frame at 60 Hz — the clock `capture_preset_over` advances by.
const DT: f32 = 1.0 / 60.0;

/// sRGB-encode a linear value into the byte a capture would hold, the
/// inverse of [`srgb_decode_lut`].
fn encode(linear: f32) -> u8 {
    let c = if linear <= 0.003_130_8 {
        linear * 12.92
    } else {
        1.055 * linear.powf(1.0 / 2.4) - 0.055
    };
    (c.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// A flat image whose **linear** grey level is `linear`.
fn solid_linear(linear: f32) -> CaptureImage {
    let v = encode(linear);
    solid(16, 16, [v, v, v, 255])
}

/// A one-pole segment: `n` frames easing from `from` to `to` with time
/// constant `tau`, rendered as linear grey.
fn one_pole(from: f32, to: f32, tau: f32, n: usize) -> Vec<CaptureImage> {
    (0..n)
        .map(|i| {
            let p = 1.0 - (-(i as f32 * DT) / tau).exp();
            solid_linear(from + (to - from) * p)
        })
        .collect()
}

/// The measurement's contract against the arithmetic ADR-0019 is quoted in:
/// a one-pole reaches 90 % of a step at `t = tau * ln(10)`, and it does so in
/// the same number of frames going down as going up.
#[test]
fn frames_to_settle_matches_the_one_pole_arithmetic_in_both_directions() {
    let tau = 0.25f32;
    // 2.303 * 0.25 s = 0.576 s = 34.5 frames at 60 Hz.
    let expected = (tau * 10.0f32.ln() / DT).round() as i32;
    // Long enough (2 s = 8 tau) that the segment's own last frame is settled,
    // so normalizing against it costs nothing.
    let n = 120;

    let rise = frames_to_settle(&one_pole(0.0, 1.0, tau, n), SETTLE_FRAC) as i32;
    let fall = frames_to_settle(&one_pole(1.0, 0.0, tau, n), SETTLE_FRAC) as i32;
    println!("one-pole tau={tau}: rise {rise}, fall {fall}, expected {expected}");
    assert!(
        (rise - expected).abs() <= 2,
        "rise {rise} should be within 2 frames of {expected}"
    );
    assert!(
        (fall - expected).abs() <= 2,
        "fall {fall} should be within 2 frames of {expected}"
    );
    assert_eq!(rise, fall, "a symmetric ease must measure symmetric");

    // A faster constant settles proportionally sooner — the measure tracks
    // tau rather than reporting a fixed fraction of the window.
    let quick = frames_to_settle(&one_pole(0.0, 1.0, 0.05, n), SETTLE_FRAC) as i32;
    assert!(quick * 3 < rise, "tau 0.05 ({quick}) vs tau 0.25 ({rise})");
}

/// Plan 0038 Phase 7 done-when 2. The case the test above **cannot** reach,
/// and the reason a truncated measurement shipped as a finding: at `tau = 0.25`
/// over 120 frames the window is 8 tau, where the unsettled residual is 0.03 %
/// and normalizing against the last frame costs nothing. Every easing
/// measurement in this repo has been made at a configuration like that.
///
/// Push tau past the window and [`frames_to_settle`] does not fail, complain,
/// or clamp — it returns a plausible number that is simply wrong.
/// [`segment_settled`] is what tells the two apart.
#[test]
fn a_response_slower_than_its_window_is_reported_unsettled_not_clamped() {
    const TOL: f32 = 0.02;
    let n = 120; // 2 s at 60 Hz.

    // Settled: 8 tau of window. Both directions, since the estimator reads
    // per-frame deltas and must not care which way the ramp goes.
    for (from, to) in [(0.0f32, 1.0f32), (1.0, 0.0)] {
        let fast = one_pole(from, to, 0.25, n);
        assert!(
            segment_settled(&fast, TOL),
            "tau 0.25 over {n} frames is 8 tau — must read settled ({from} -> {to})"
        );
    }

    // Unsettled: tau 2.0 s over the same 2 s window leaves ~37 % of the
    // travel undone, yet `frames_to_settle` still answers inside the segment.
    let slow = one_pole(0.0, 1.0, 2.0, n);
    let reported = frames_to_settle(&slow, SETTLE_FRAC);
    assert!(
        reported < n as u32,
        "the premise of this test: frames_to_settle always answers inside \
         the segment, which is why it cannot self-detect truncation — got \
         {reported} for a response that has barely started"
    );
    assert!(
        !segment_settled(&slow, TOL),
        "tau 2.0 over {n} frames has ~37 % of its travel left — must read \
         unsettled, otherwise `{reported}` would pass for a measurement"
    );

    // The boundary is a property of the residual, not of a frame count: the
    // same tau reads settled once the window is long enough to earn it.
    let slow_but_long = one_pole(0.0, 1.0, 2.0, 8 * n);
    assert!(
        segment_settled(&slow_but_long, TOL),
        "tau 2.0 over {} frames is 8 tau — the same response, measured to \
         settlement, must read settled",
        8 * n
    );
}

/// Why [`linear_diff`] exists rather than reusing [`frame_diff`]: on the very
/// same symmetric ramp, measuring in the stored sRGB bytes reports a rise and
/// a fall that differ by more than 2x, so a scalar `[smoothing]` entry would
/// read as asymmetric and the probe would be measuring the transfer curve.
#[test]
fn measuring_in_srgb_would_fake_an_asymmetry() {
    let tau = 0.25f32;
    let n = 120;
    let srgb_settle = |seg: &[CaptureImage]| -> usize {
        let total = frame_diff(&seg[0], seg.last().unwrap());
        seg.iter()
            .position(|img| frame_diff(&seg[0], img) >= SETTLE_FRAC * total)
            .unwrap_or(0)
    };
    let rise = srgb_settle(&one_pole(0.0, 1.0, tau, n));
    let fall = srgb_settle(&one_pole(1.0, 0.0, tau, n));
    println!("same ramp measured in sRGB: rise {rise}, fall {fall}");
    assert!(
        fall > rise * 2,
        "sRGB rise {rise} / fall {fall} should be badly skewed — if they are \
         now close, linear_diff is no longer earning its keep"
    );
}

/// A stimulus a preset does not respond to must not divide by zero or invent
/// a transient: both directions read 0 and the ratio is 0.
#[test]
fn a_frame_that_never_moves_reports_no_transient() {
    let flat: Vec<CaptureImage> = (0..30).map(|_| solid_linear(0.4)).collect();
    let r = step_response(&flat, &flat);
    assert_eq!(
        r,
        StepResponse {
            rise_frames: 0,
            fall_frames: 0
        }
    );
    assert_eq!(r.ratio(), 0.0);
    // ...and an empty capture is total too.
    assert_eq!(frames_to_settle(&[], SETTLE_FRAC), 0);
}

#[test]
fn step_response_ratio_reports_the_asymmetry() {
    // A 3-frame rise against a 69-frame fall is the shape
    // `{ attack = 0.02, release = 0.5 }` produces at 60 Hz.
    let r = StepResponse {
        rise_frames: 3,
        fall_frames: 69,
    };
    assert!((r.ratio() - 23.0).abs() < 1e-6, "got {}", r.ratio());
    let sym = StepResponse {
        rise_frames: 35,
        fall_frames: 35,
    };
    assert_eq!(sym.ratio(), 1.0);
}

#[test]
fn struct_diff_is_recolor_robust_but_shape_sensitive() {
    // Same shape (left bar), different colors: low structural difference.
    let red_bar = left_bar(64, 64, [220, 30, 30, 255]);
    let blue_bar = left_bar(64, 64, [30, 30, 220, 255]);
    let recolor = struct_diff(&red_bar, &blue_bar);

    // Different shape (left bar vs top bar), same color: high difference.
    let red_top = top_bar(64, 64, [220, 30, 30, 255]);
    let reshape = struct_diff(&red_bar, &red_top);

    assert!(
        recolor < reshape,
        "a recolor ({recolor:.3}) must read as more similar than a reshape ({reshape:.3})"
    );
    assert!(
        recolor < 0.05,
        "same-shape recolor is near-zero ({recolor:.3})"
    );
    assert!(
        reshape > 0.10,
        "a genuine shape change is clearly nonzero ({reshape:.3})"
    );
}

// ---------------------------------------------------------------------------
// The frame's own ground (Plan 0116 Phase 3)
// ---------------------------------------------------------------------------

#[test]
fn modal_ground_finds_the_dark_ground_a_light_figure_sits_on() {
    // The world the lens was built for: a figure on a ground it does not own.
    // The estimator must land on that ground, so it gives the answer the
    // hardcoded BLACK gave and the dark-world library does not move.
    let lit = image(
        32,
        32,
        |x, _| {
            if x < 8 { [255, 255, 255, 255] } else { BLACK }
        },
    );
    assert_eq!(modal_ground(&lit), BLACK);
    assert_eq!(coverage(&lit, modal_ground(&lit), 8), 0.25);
    assert_eq!(coverage(&lit, BLACK, 8), 0.25);
}

#[test]
fn modal_ground_finds_the_paper_an_ink_figure_sits_on() {
    // ADR-0126's motivating world, inverted: black ink on off-white paper.
    //
    // Under BLACK the *paper* is the figure — 75 % of the frame is "lit" and
    // the 25 % that carries the drawing is excluded, which is the whole of the
    // `fragment_tiledmono` false positive. Under the derived ground the ink is
    // the figure and coverage reads the share it actually occupies.
    let paper = [245, 245, 245, 255];
    let ink = image(32, 32, |x, _| if x < 8 { BLACK } else { paper });
    assert_eq!(modal_ground(&ink), paper);
    assert_eq!(coverage(&ink, BLACK, 8), 0.75);
    assert_eq!(coverage(&ink, modal_ground(&ink), 8), 0.25);

    // And the residue Plan 0116 Phase 1 measured, asserted rather than
    // remembered: the ground does **not** repair `tonal_flatness` on a duotone.
    // Whichever of the two populations `is_lit` removes, the other one holds
    // all of what remains and reads 1.0 either way. That is ADR-0128's job.
    assert_eq!(tonal_flatness(&ink, BLACK, 8), 1.0);
    assert_eq!(tonal_flatness(&ink, modal_ground(&ink), 8), 1.0);
}

#[test]
fn modal_ground_returns_the_bands_own_mean_not_its_centre() {
    // A specific off-white, not the middle of the 16-level band holding it: at
    // EPS-scale tolerances the difference is a ground versus a second figure.
    let paper = [250, 248, 246, 255];
    assert_eq!(modal_ground(&solid(16, 16, paper)), paper);
}

#[test]
fn a_frame_with_no_dominant_band_has_no_ground() {
    // The boundary case, defined rather than discovered (Plan 0116 Phase 3).
    // A perfectly flat luminance histogram puts exactly MIN_GROUND_SHARE in
    // every band, so no band is dominant and the estimator declines: 256 gray
    // levels, each on exactly one column of a 256-wide frame.
    let flat = image(256, 8, |x, _| {
        let v = x as u8;
        [v, v, v, 255]
    });
    let mut counts = [0u64; TONE_BANDS];
    for px in flat.rgba.chunks_exact(4) {
        counts[(((luma(px) / 256.0) * TONE_BANDS as f32) as usize).min(TONE_BANDS - 1)] += 1;
    }
    let total: u64 = counts.iter().sum();
    let modal = *counts.iter().max().unwrap();
    assert_eq!(
        modal as f32 / total as f32,
        MIN_GROUND_SHARE,
        "the fixture must be exactly flat, or it does not test the boundary"
    );
    assert_eq!(modal_ground(&flat), NO_GROUND);

    // Uniform *noise* is not this case, and a reader who expects it to be
    // should find out here. The largest of TONE_BANDS counts is at least their
    // mean, so any sampled distribution lands above the line: the rule fires on
    // exact flatness, and on nothing the renderer produces.
    let noise = image(96, 96, |x, y| {
        // splitmix64 over the pixel index — seeded, so the frame is the same
        // every run (the determinism rule in CLAUDE.md applies to fixtures too).
        let mut z = (u64::from(y) * 96 + u64::from(x)).wrapping_add(0x9E37_79B9_7F4A_7C15);
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        let v = ((z ^ (z >> 31)) >> 33) as u8;
        [v, v, v, 255]
    });
    assert_ne!(modal_ground(&noise), NO_GROUND);
}

#[test]
fn an_empty_frame_has_no_ground() {
    assert_eq!(modal_ground(&image(0, 0, |_, _| BLACK)), NO_GROUND);
}

#[test]
fn a_frame_of_one_tone_is_all_ground_and_nothing_is_lit() {
    // The property Plan 0116 Phase 6 rests on: a bare sheet of paper is its own
    // ground, so nothing departs from it. Under BLACK the same frame reads
    // fully lit, which is the false negative ADR-0126 exists to close.
    let bare = solid(32, 32, [250, 249, 247, 255]);
    assert_eq!(coverage(&bare, modal_ground(&bare), 8), 0.0);
    assert_eq!(quadrant_spread(&bare, modal_ground(&bare), 8), 0);
    assert_eq!(coverage(&bare, BLACK, 8), 1.0);
}
