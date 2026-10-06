//! The `lsystem` space turtle at the pixel level (ADR-0258): a tree walked in
//! depth reaches the frame through the shared camera and turns with its `yaw`.
//!
//! Software adapter (`prefer_software`) so it holds on any CI GPU. The claims
//! are relative — one capture against another — so they hold on every
//! software rasterizer, not only on the one the golden baselines were blessed
//! on.

use rlx_core::preset::Preset;
use rlx_core::render::scenes::OverflowContext;
use rlx_core::render::{CaptureImage, Renderer, TierConfig, metrics::frame_diff};

use crate::common;

/// The committed fixture the space tree's golden baseline is rendered from.
const FIXTURE: &str = include_str!("../fixtures/lsystem_space.toml");

/// Frames warmed before capture. Nothing in the fixture moves with time.
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

/// Plan 0237 Phase 3's done-when: a space depth whose walk passes the tier's
/// `seg3d_segments` is reported through [`OverflowContext::Depth`], naming the
/// depth, the segments dropped and the cap that bit, rather than drawing a
/// truncated tree without notice; a grammar inside the cap reports nothing.
#[test]
fn a_space_depth_past_the_seg3d_cap_is_reported() {
    let Some(mut renderer) = common::headless(64, 64) else {
        return;
    };
    let cap = TierConfig::FLOOR.seg3d_segments as usize;
    // Every `F` rewrites to four, so depth `d` walks `4^d` segments: depth 6
    // is 4,096, inside Floor's cap, and depth 7 is 16,384, past it.
    let preset = |max_depth: u32| {
        format!(
            "system = \"lsystem\"\nname = \"dense_space_{max_depth}\"\n\
             [generator]\naxiom = \"F\"\nrules = {{ F = \"F[&F][^F]/F\" }}\n\
             angle_deg = 30\nmax_depth = {max_depth}\nturtle = \"space\"\n\
             [params]\nvisible_depth = \"{max_depth}\"\n"
        )
    };
    assert!(
        4usize.pow(6) <= cap && 4usize.pow(7) > cap,
        "the probe straddles the cap"
    );

    capture(&mut renderer, &preset(7));
    let notice = renderer
        .cap_overflow()
        .copied()
        .expect("the truncated depth must reach the renderer");
    assert_eq!(notice.context, OverflowContext::Depth(7));
    assert_eq!(notice.cap, cap, "the cap that bit is seg3d_segments");
    assert_eq!(notice.dropped, 4usize.pow(7) - cap);

    capture(&mut renderer, &preset(6));
    assert_eq!(
        renderer.cap_overflow(),
        None,
        "a tree inside the cap is not an overflow"
    );
}

/// The flat fixture the `lsystem` golden is rendered from.
const FLAT_FIXTURE: &str = include_str!("../fixtures/lsystem.toml");

/// Plan 0237 Phase 2 (ADR-0258): what each turtle declares inert does not
/// move a pixel. The space tree ignores the in-plane transform, the mirror and
/// the opaque stroke path; the flat figure ignores the camera block.
#[test]
fn each_turtle_ignores_what_it_declares_inert() {
    const SIZE: u32 = 96;
    let Some(mut renderer) = common::headless(SIZE, SIZE) else {
        return;
    };
    let space = capture(&mut renderer, FIXTURE);
    let space_moved = capture(
        &mut renderer,
        &format!(
            "{FIXTURE}rotation = \"1.3\"\nscale = \"0.4\"\nstroke_blend = \"1\"\n\
             mirror_order = \"3\"\nmirror_reflect = \"1\"\n"
        ),
    );
    assert!(
        space.rgba == space_moved.rgba,
        "a flat-only parameter moved the space tree by {}",
        frame_diff(&space, &space_moved)
    );

    let flat = capture(&mut renderer, FLAT_FIXTURE);
    let flat_moved = capture(
        &mut renderer,
        &format!(
            "{FLAT_FIXTURE}yaw = \"1.1\"\npitch = \"-0.7\"\ndistance = \"2\"\nfov = \"1.6\"\n\
             focus = \"0.9\"\naperture = \"9\"\nfog = \"1\"\nsolid = \"1\"\n"
        ),
    );
    assert!(
        flat.rgba == flat_moved.rgba,
        "a camera parameter moved the flat figure by {}",
        frame_diff(&flat, &flat_moved)
    );

    // And the controls: the same parameters do move the mode that reads them.
    let turned = capture(
        &mut renderer,
        &FLAT_FIXTURE.replace("rotation      = \"0\"", "rotation      = \"1.3\""),
    );
    assert!(
        flat.rgba != turned.rgba,
        "rotation must turn the flat figure"
    );
    let orbited = capture(
        &mut renderer,
        &FIXTURE.replace("yaw           = \"0.4\"", "yaw           = \"1.1\""),
    );
    assert!(space.rgba != orbited.rgba, "yaw must turn the space tree");
}

/// Plan 0237 Phase 1's done-when: the space fixture, a branching grammar
/// that pitches with `&` and rolls with `/`, draws a tree that covers a real
/// part of the frame, and turning the camera's `yaw` moves it.
#[test]
fn a_space_tree_reaches_the_frame_and_turns_with_yaw() {
    const SIZE: u32 = 128;
    let Some(mut renderer) = common::headless(SIZE, SIZE) else {
        return;
    };
    assert!(
        FIXTURE.contains('&') && FIXTURE.contains('/') && FIXTURE.contains("\"space\""),
        "the fixture must pitch, roll and walk in space, or this proves nothing"
    );
    let tree = capture(&mut renderer, FIXTURE);
    let lit = (0..SIZE)
        .flat_map(|y| (0..SIZE).map(move |x| (x, y)))
        .filter(|&(x, y)| light(&tree, x, y) > 32)
        .count();
    assert!(
        lit > 300,
        "the tree lit only {lit} of {} pixels",
        SIZE * SIZE
    );

    let turned_toml = FIXTURE.replace("yaw           = \"0.4\"", "yaw           = \"1.4\"");
    assert_ne!(
        turned_toml, FIXTURE,
        "the yaw binding was not found to turn"
    );
    let turned = capture(&mut renderer, &turned_toml);
    let diff = frame_diff(&tree, &turned);
    println!("{lit} lit pixels; a radian of yaw moves the frame by {diff:.4}");
    assert!(
        diff > 0.002,
        "a whole radian of yaw moved the frame by only {diff}"
    );
}
