//! The `parametric_curve` space families at the pixel level (ADR-0258): a
//! torus knot drawn through the shared camera reaches the frame, turns with
//! the camera's `yaw`, and under an open aperture strokes its far side wider
//! than its focal side.
//!
//! Software adapter (`prefer_software`) so it holds on any CI GPU. The claims
//! are relative — one capture against another, or one half of a frame against
//! the other — so they hold on every software rasterizer, not only on the one
//! the golden baselines were blessed on.

use rlx_core::preset::Preset;
use rlx_core::render::scenes::OverflowContext;
use rlx_core::render::{CaptureImage, Renderer, TierConfig, metrics::frame_diff};

use crate::common;

/// The committed fixture the knot's golden baseline is rendered from.
const FIXTURE: &str = include_str!("../fixtures/parametric_torus_knot.toml");

/// Frames warmed before capture. Nothing in these presets moves with time.
const FRAMES: u32 = 3;

/// A pixel's light: its brightest channel, in `0..=255`.
fn light(img: &CaptureImage, x: u32, y: u32) -> u8 {
    let at = ((y * img.width + x) * 4) as usize;
    img.rgba[at..at + 3].iter().copied().max().unwrap_or(0)
}

fn capture(renderer: &mut Renderer, toml: &str) -> CaptureImage {
    let preset = Preset::from_toml_str(toml).expect("the preset loads");
    let name = preset.name.clone();
    renderer.set_presets(vec![preset]);
    renderer
        .capture_preset(&name, &common::fixed_frame(), FRAMES)
        .expect("capture")
}

/// Plan 0236 Phase 2's done-when, the first half: the fixture draws a knot
/// that covers a real part of the frame, and turning the camera's `yaw` moves
/// it.
#[test]
fn a_torus_knot_reaches_the_frame_and_turns_with_yaw() {
    const SIZE: u32 = 128;
    let Some(mut renderer) = common::headless(SIZE, SIZE) else {
        return;
    };
    let knot = capture(&mut renderer, FIXTURE);
    let lit = (0..SIZE)
        .flat_map(|y| (0..SIZE).map(move |x| (x, y)))
        .filter(|&(x, y)| light(&knot, x, y) > 32)
        .count();
    assert!(
        lit > 400,
        "the knot lit only {lit} of {} pixels",
        SIZE * SIZE
    );

    let turned_toml = FIXTURE.replace("yaw           = \"0.4\"", "yaw           = \"1.4\"");
    assert_ne!(
        turned_toml, FIXTURE,
        "the yaw binding was not found to turn"
    );
    let turned = capture(&mut renderer, &turned_toml);
    let diff = frame_diff(&knot, &turned);
    println!("{lit} lit pixels; a radian of yaw moves the frame by {diff:.4}");
    assert!(
        diff > 0.002,
        "a whole radian of yaw moved the frame by only {diff}"
    );
}

/// A knot on a slim tube seen from above its plane: the ring's top on screen
/// is its near side, its bottom the far side. `focus = 0` puts the focal plane
/// at the volume's nearest extent.
fn ring_view(aperture: f32) -> String {
    format!(
        "system = \"parametric_curve\"\nname = \"ring_{aperture}\"\n\
         [curve]\nfamily = \"torus_knot\"\n\
         [params]\nn = \"2\"\nd = \"3\"\ntube = \"0.08\"\nsamples = \"720\"\n\
         scale = \"1\"\nspin = \"0\"\nthickness = \"2\"\nhue_spread = \"0\"\n\
         yaw = \"0\"\npitch = \"1.0\"\ndistance = \"3\"\nfov = \"0.9\"\n\
         focus = \"0\"\naperture = \"{aperture}\"\n"
    )
}

/// How many pixels of the centre column, in rows `rows`, carry at least a
/// quarter of that run's own peak light — the stroke's width where the knot
/// crosses the column, whatever its brightness.
fn crossing_width(img: &CaptureImage, rows: std::ops::Range<u32>) -> (usize, u8) {
    let x = img.width / 2;
    let peak = rows.clone().map(|y| light(img, x, y)).max().unwrap_or(0);
    let wide = rows
        .filter(|&y| u32::from(light(img, x, y)) * 4 >= u32::from(peak))
        .count();
    (wide, peak)
}

/// Plan 0236 Phase 2's done-when, the second half: with `aperture > 0` the
/// projected stroke is wider at the knot's far side than at its focal side,
/// measured on one frame. The pinhole control is what makes it the lens's
/// doing: perspective alone draws the far side **thinner**.
#[test]
fn an_open_aperture_strokes_the_far_side_wider_than_the_focal_side() {
    const SIZE: u32 = 256;
    let Some(mut renderer) = common::headless(SIZE, SIZE) else {
        return;
    };
    let half = SIZE / 2;

    let pinhole = capture(&mut renderer, &ring_view(0.0));
    let (near, near_peak) = crossing_width(&pinhole, 0..half);
    let (far, far_peak) = crossing_width(&pinhole, half..SIZE);
    assert!(
        near_peak > 32 && far_peak > 32,
        "both sides of the ring must cross the centre column: peaks {near_peak} / {far_peak}"
    );
    assert!(
        far <= near,
        "through a pinhole the far side is drawn {far} px wide against the near side's {near}"
    );

    println!("pinhole: near {near} px (peak {near_peak}), far {far} px (peak {far_peak})");

    let lens = capture(&mut renderer, &ring_view(12.0));
    let (near, near_peak) = crossing_width(&lens, 0..half);
    let (far, far_peak) = crossing_width(&lens, half..SIZE);
    println!("aperture 12: near {near} px (peak {near_peak}), far {far} px (peak {far_peak})");
    assert!(
        near_peak > 0 && far_peak > 0,
        "both sides of the ring must still draw: peaks {near_peak} / {far_peak}"
    );
    assert!(
        far >= 2 * near,
        "an aperture of 12 px draws the far side {far} px wide against the focal \
         side's {near}"
    );
}

/// Plan 0236 Phase 4's done-when: a space family's `samples` past the tier's
/// `seg3d_segments` is clamped **with a notice** that reaches the renderer,
/// names what was asked and carries the cap that bit; one within the cap
/// announces nothing.
#[test]
fn an_over_cap_space_walk_is_clamped_with_a_notice() {
    let Some(mut renderer) = common::headless(64, 64) else {
        return;
    };
    let cap = TierConfig::FLOOR.seg3d_segments;
    let asked = cap + 500;
    for family in ["torus_knot", "lissajous_3d"] {
        let preset = |samples: u32| {
            format!(
                "system = \"parametric_curve\"\nname = \"dense_{family}_{samples}\"\n\
                 [curve]\nfamily = \"{family}\"\n[params]\nsamples = \"{samples}\"\n"
            )
        };
        capture(&mut renderer, &preset(asked));
        let notice = renderer
            .cap_overflow()
            .copied()
            .unwrap_or_else(|| panic!("{family}: the clamp must reach the renderer"));
        assert_eq!(notice.context, OverflowContext::Samples(asked), "{family}");
        assert_eq!(notice.cap, cap as usize, "{family}");
        assert_eq!(notice.dropped, 500, "{family}");
        assert!(
            notice.to_string().contains("pin --tier rich"),
            "{family}: Floor's cap is under Rich's, so the remedy names it: {notice}"
        );

        capture(&mut renderer, &preset(cap));
        assert_eq!(
            renderer.cap_overflow(),
            None,
            "{family}: samples at the cap is not an overflow"
        );
    }
}
