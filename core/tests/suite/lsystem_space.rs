//! The `lsystem` space turtle at the pixel level (ADR-0258): a tree walked in
//! depth reaches the frame through the shared camera and turns with its `yaw`.
//!
//! Software adapter (`prefer_software`) so it holds on any CI GPU. The claims
//! are relative — one capture against another — so they hold on every
//! software rasterizer, not only on the one the golden baselines were blessed
//! on.

use rlx_core::preset::Preset;
use rlx_core::render::{CaptureImage, Renderer, metrics::frame_diff};

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
