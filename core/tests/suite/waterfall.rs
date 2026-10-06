//! The waterfall system through the renderer: a `[waterfall] rows` past what the
//! tier's `seg3d_segments` draws is clamped with a notice that reaches the
//! renderer, and one within it announces nothing; a solid row is clamped at its
//! skirted cost, and hides the row behind it.

use rlx_core::preset::Preset;
use rlx_core::render::camera::Camera3d;
use rlx_core::render::scenes::OverflowContext;
use rlx_core::render::{CaptureImage, Renderer, TierConfig};

use crate::common;

fn load(renderer: &mut Renderer, toml: &str) {
    let preset = Preset::from_toml_str(toml).expect("the preset loads");
    let name = preset.name.clone();
    renderer.set_presets(vec![preset]);
    renderer
        .capture_preset(&name, &common::fixed_frame_spectrum(), 3)
        .expect("capture");
}

/// Plan 0238 Phase 2's done-when: an over-cap `rows` is clamped with a notice
/// naming what was asked and carrying the cap that bit.
#[test]
fn an_over_cap_row_count_is_clamped_with_a_notice() {
    let Some(mut renderer) = common::headless(64, 64) else {
        return;
    };
    let cap = TierConfig::FLOOR.seg3d_segments as usize;
    let per_row = 63usize;
    let fits = (cap / per_row) as u32;
    let preset = |rows: u32| {
        format!(
            "system = \"waterfall\"\nname = \"rows_{rows}\"\n\
             [waterfall]\nelements = 64\nrows = {rows}\n"
        )
    };

    load(&mut renderer, &preset(fits + 10));
    let notice = renderer
        .cap_overflow()
        .copied()
        .unwrap_or_else(|| panic!("the row clamp must reach the renderer"));
    assert_eq!(notice.context, OverflowContext::Rows(fits + 10, 63));
    assert_eq!(notice.cap, cap);
    assert_eq!(notice.dropped, 10 * per_row);
    let text = notice.to_string();
    assert!(
        text.contains(&format!("keeps {fits} rows")) && text.contains("pin --tier rich"),
        "Floor's cap is under Rich's, so the remedy names it: {text}"
    );

    load(&mut renderer, &preset(fits));
    assert_eq!(
        renderer.cap_overflow(),
        None,
        "rows at the cap is not an overflow"
    );
}

/// Plan 0248 Phase 4's done-when: **a solid row costs its skirts**, so the rows
/// that fit the tier's buffer as lines alone are held to half as many when the
/// preset is solid, and the clamp reaches the renderer as the same
/// `OverflowContext::Rows`, counting a row at its skirted cost. The same rows
/// in glow announce nothing.
#[test]
fn a_solid_row_count_is_clamped_at_its_skirted_cost() {
    let Some(mut renderer) = common::headless(64, 64) else {
        return;
    };
    let cap = TierConfig::FLOOR.seg3d_segments as usize;
    let fits = (cap / 63) as u32;
    let preset = |solid: u32| {
        format!(
            "system = \"waterfall\"\nname = \"skirted_{solid}\"\n\
             [waterfall]\nelements = 64\nrows = {fits}\n\
             [params]\nsolid = \"{solid}\"\n"
        )
    };

    load(&mut renderer, &preset(0));
    assert_eq!(renderer.cap_overflow(), None, "glow rows at the cap fit");

    load(&mut renderer, &preset(1));
    let notice = renderer
        .cap_overflow()
        .copied()
        .unwrap_or_else(|| panic!("the skirted row clamp must reach the renderer"));
    assert_eq!(notice.context, OverflowContext::Rows(fits, 126));
    assert_eq!(notice.cap, cap);
    let kept = cap / 126;
    assert_eq!(notice.dropped, (fits as usize - kept) * 126);
    let text = notice.to_string();
    assert!(text.contains(&format!("keeps {kept} rows")), "{text}");
}

/// The test target's side, in pixels.
const SIDE: u32 = 192;

/// A three-row landscape of eight bands, its camera fixed and its rows far
/// apart, at `solid`: no fade, so the farther rows keep the front edge's light.
fn terrain(solid: u32) -> String {
    format!(
        "system = \"waterfall\"\nname = \"terrain_{solid}\"\n\
         [waterfall]\nelements = 8\nrows = 3\nrow_period = 1.0\n\
         [params]\n\
         height = \"1.0\"\nrow_spacing = \"0.3\"\nfade = \"0\"\nline_width = \"3\"\n\
         yaw = \"0\"\npitch = \"0.35\"\ndistance = \"3.5\"\nfov = \"0.8\"\n\
         focus = \"0.5\"\naperture = \"0\"\nzoom = \"1\"\npan_x = \"0\"\npan_y = \"0\"\n\
         solid = \"{solid}\"\n"
    )
}

/// The landscape [`terrain`] draws after 177 frames at 60 Hz: two rows pushed
/// flat on the ground at 1 s and 2 s, then a live front row whose four middle
/// bands stand at full height — a peak in the near row with a flat row 0.95 of
/// a row behind it.
fn capture_terrain(renderer: &mut Renderer, solid: u32) -> CaptureImage {
    let preset = Preset::from_toml_str(&terrain(solid)).expect("the terrain preset loads");
    let name = preset.name.clone();
    renderer.set_presets(vec![preset]);
    let flat = {
        let mut frame = common::fixed_frame_spectrum();
        frame.spectrum.iter_mut().for_each(|band| *band = 0.0);
        frame
    };
    let peak = {
        let mut frame = flat;
        let bins = frame.spectrum.len();
        for (i, band) in frame.spectrum.iter_mut().enumerate() {
            if (2 * bins / 8..6 * bins / 8).contains(&i) {
                *band = 1.0;
            }
        }
        frame
    };
    let frames = 177;
    let mut last = None;
    renderer
        .capture_stream(
            &name,
            frames,
            1.0 / 60.0,
            &mut |i| if i < 120 { flat } else { peak },
            &mut |i, img| {
                if i == frames - 1 {
                    last = Some(img.clone());
                }
                Ok(())
            },
        )
        .expect("capture the terrain");
    last.expect("the stream reached its last frame")
}

/// Plan 0248 Phase 4's done-when: **a solid row hides what lies behind it**
/// (ADR-0263). Down the frame's centre column, the flat row behind the near
/// row's peak lies on screen between the peak's top and the near row's foot.
/// In glow its line is the brightest pixel there, as it always was; solid, the
/// same pixels show the near row's black skirt, not the far row's light.
#[test]
fn a_solid_near_peak_hides_the_row_behind_it() {
    let Some(mut renderer) = common::headless(SIDE, SIDE) else {
        return;
    };
    // The scene's own world: the front row stands at `z = 1` and each row
    // lies `row_spacing` behind the last, so the newest pushed row is 0.95 of
    // a row back at the capture's phase.
    let view = Camera3d {
        yaw: 0.0,
        pitch: 0.35,
        distance: 3.5,
        fov: 0.8,
        focus: 0.5,
        aperture: 0.0,
    }
    .view(1.0, 1.0, [0.0, 0.0]);
    let row_of = |p: [f32; 3]| {
        let c = view.clip(p);
        (0.5 - c[1] / c[3] * 0.5) * SIDE as f32
    };
    let (top, foot, behind) = (
        row_of([0.0, 1.0, 1.0]),
        row_of([0.0, 0.0, 1.0]),
        row_of([0.0, 0.0, 1.0 - 0.95 * 0.3]),
    );
    println!("centre column: peak top {top:.1}, near foot {foot:.1}, far row {behind:.1}");
    assert!(
        top + 6.0 < behind && behind + 6.0 < foot,
        "the far row must lie on screen between the peak and its foot"
    );

    let luma = |img: &CaptureImage, y: u32| {
        let i = ((y * SIDE + SIDE / 2) * 4) as usize;
        let px = &img.rgba[i..i + 3];
        (u32::from(px[0]) + u32::from(px[1]) + u32::from(px[2])) / 3
    };
    let window = (behind.round() as u32 - 3)..=(behind.round() as u32 + 3);
    let glow = capture_terrain(&mut renderer, 0);
    let solid = capture_terrain(&mut renderer, 1);
    let lit = |img: &CaptureImage| window.clone().map(|y| luma(img, y)).max().unwrap_or(0);
    let (glow_lit, solid_lit) = (lit(&glow), lit(&solid));
    println!("far row's light behind the peak: glow {glow_lit}, solid {solid_lit}");
    let brightest = ((top.max(0.0) as u32 + 3)..(foot as u32 - 3))
        .max_by_key(|&y| luma(&glow, y))
        .unwrap_or(0);
    assert!(
        window.contains(&brightest),
        "glow: the brightest pixel between the peak and its foot is the far row's line, at \
         {behind:.1}, not row {brightest}"
    );
    assert!(
        glow_lit >= 40,
        "glow: the far row shows its light ({glow_lit})"
    );
    assert!(
        solid_lit * 8 <= glow_lit,
        "solid: the far row behind the peak still shows {solid_lit} against glow's {glow_lit}"
    );
}
