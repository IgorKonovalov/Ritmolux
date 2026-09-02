//! The attractor with the engine `trails` stage bound, pinned to a baseline
//! (Plan 0053 Phase 1, ADR-0058).
//!
//! **This combination was covered by no capture in the suite.** Two shipped
//! presets — `attractor_clifford` and `attractor_leviathan` — bind `trails` on
//! the attractor, which puts the attractor's four pipelines (compute, draw,
//! decay, present) and the trails stage's two into one command buffer. That is
//! the densest coexistence of pipelines any shipped preset produces, and
//! ADR-0058's hazard — the DX12 WARP software adapter handing a pipeline whose
//! bind-group layout matches another live one *the other pass's* resources —
//! keys on exactly that. `golden.rs`'s `attractor.toml` binds no trails, and
//! every `composite_*` fixture is a line scene, so nothing rendered it under a
//! pinned baseline.
//!
//! # What this pins, and what it does not
//!
//! It is **coverage, not evidence of correctness**. The baseline is blessed on
//! WARP like every other one in this repository, so if this configuration does
//! alias, the committed PNG is a picture of the wrong thing and every later run
//! agrees with it. That is the whole failure mode ADR-0058 exists for, and the
//! check for it is the hardware-vs-WARP comparison ADR-0058 requires — not this
//! file. What this file catches is *drift*: a change to the attractor, to the
//! trails stage, or to the seam between them that moves the picture.
//!
//! # A separate test binary, for two reasons
//!
//! **Blessing.** `RLX_BLESS` is not scoped to a fixture. Adding this to
//! `golden.rs`'s `EXTRA_FIXTURES` would mean `RLX_BLESS=1 … --test golden`
//! rewrites all 12 of that binary's baselines to add one, and three of those
//! twelve (`lsystem`, `parametric_curve`, `star_pattern`) re-encode differently
//! on this repository's dev box from a *clean* tree — so the diff would name
//! files the change never touched, which is how an unrelated drift gets blessed
//! in while someone reads it as noise. Its
//! own binary means `RLX_BLESS=1 cargo test -p rlx-core --test attractor_trails`
//! can reach nothing else. Same posture `line_joints.rs` documents.
//!
//! **Device state.** `composite.rs` keeps one stage per fixture and its own
//! process precisely because building GPU resources mid-run is documented to
//! change what the trails stage resolves to on the software adapter. This
//! fixture builds *six* pipelines. Putting it in either existing binary would
//! expose that binary's pre-existing baselines to a device state they have never
//! been rendered from.
//!
//! # The capture size is not a square
//!
//! 160x100, following `composite.rs`: the post stages round each grid axis up to
//! a 256 px step, so this takes a 256x256 grid against a target aspect of 1.6.
//! A square capture makes the grid's aspect and the target's identical and
//! cannot see an ADR-0037 confusion between them — which is the bug that has
//! shipped three times, once on this very scene.

use rlx_core::preset::Preset;
use rlx_core::render::{CaptureImage, metrics::frame_diff};

mod common;

/// Capture width/height — see the module docs. **Not interchangeable.**
const WIDTH: u32 = 160;
const HEIGHT: u32 = 100;
/// Frames warmed before capture. The attractor's own decay settles in a handful
/// at `fade = 0.6`; the stage's `trails = 0.98` tail needs far more than that
/// before what it holds stops growing.
const FRAMES: u32 = 40;
/// Mean per-channel difference (0..1) a fresh render may drift from baseline.
/// Same tolerance as `golden.rs` and `composite.rs`.
const MEAN_TOL: f32 = 0.02;
/// Largest single-channel byte difference tolerated at any pixel.
const MAX_OUTLIER: u8 = 48;

const STEM: &str = "attractor_trails";
const FIXTURE: &str = include_str!("fixtures/attractor_trails.toml");

/// Largest absolute single-channel (RGB) byte difference across the two images.
fn max_channel_outlier(a: &CaptureImage, b: &CaptureImage) -> u8 {
    a.rgba
        .chunks_exact(4)
        .zip(b.rgba.chunks_exact(4))
        .flat_map(|(pa, pb)| {
            pa.iter()
                .zip(pb.iter())
                .take(3)
                .map(|(x, y)| x.abs_diff(*y))
        })
        .max()
        .unwrap_or(0)
}

/// Capture the fixture once, or `None` on an adapterless runner.
fn capture() -> Option<CaptureImage> {
    let mut renderer = common::headless(WIDTH, HEIGHT)?;
    let preset =
        Preset::from_toml_str(FIXTURE).unwrap_or_else(|e| panic!("{STEM}.toml is invalid: {e}"));
    let name = preset.name.clone();
    renderer.set_presets(vec![preset]);
    Some(
        renderer
            .capture_preset(&name, &common::fixed_frame(), FRAMES)
            .expect("capture the attractor-with-trails fixture"),
    )
}

/// The scene's own accumulation and the engine stage's, composited and pinned.
///
/// `RLX_BLESS=1 cargo test -p rlx-core --test attractor_trails` rewrites this
/// baseline and — because this is its own binary — can reach no other.
#[test]
fn the_attractor_over_the_trails_stage_matches_its_baseline() {
    let Some(fresh) = capture() else {
        return;
    };
    std::fs::create_dir_all(common::golden_dir()).expect("create tests/golden");
    let path = common::golden_dir().join(format!("{STEM}.png"));

    // Checked ahead of the bless branch, the way `composite.rs` guards its
    // no-clip claim: a bless must not be able to write an empty frame and call
    // it the new truth. The fixture exists to put six pipelines in one command
    // buffer, and a frame with nothing in it does not do that whatever the
    // pipelines did.
    let lit = fresh
        .rgba
        .chunks_exact(4)
        .filter(|px| px[0] > 8 || px[1] > 8 || px[2] > 8)
        .count();
    let total = (WIDTH * HEIGHT) as usize;
    assert!(
        lit * 100 > total,
        "only {lit} of {total} pixels carry any light — the attractor drew \
         nothing, so this capture pins neither the scene nor the stage over it"
    );

    if std::env::var_os("RLX_BLESS").is_some() {
        common::encode(&fresh, &path);
        println!("blessed {}", path.display());
        return;
    }

    assert!(
        path.exists(),
        "missing baseline {} — run `RLX_BLESS=1 cargo test -p rlx-core --test attractor_trails`",
        path.display()
    );
    let baseline = common::decode(&path);
    let mean = frame_diff(&baseline, &fresh);
    let outlier = max_channel_outlier(&baseline, &fresh);
    println!(
        "{STEM:<18} mean {mean:.4} (tol {MEAN_TOL}) max_outlier {outlier} (tol {MAX_OUTLIER}), {lit} of {total} lit"
    );
    assert!(
        mean <= MEAN_TOL && outlier <= MAX_OUTLIER,
        "{STEM}: mean {mean:.4} / outlier {outlier} exceeds tolerance — the \
         attractor, the trails stage, or the seam between them has moved. Bless \
         with RLX_BLESS=1 only if intended"
    );
}

/// The fixture still describes the configuration this binary exists for.
///
/// Two values are load-bearing and neither is visible in the picture as such:
/// the engine stage's tail has to **outlast** the scene's own, or its
/// `max(cur, prev * trails)` is a bit-for-bit passthrough and the baseline pins
/// a frame the trails pipelines never wrote to; and the figure has to move, or a
/// trail is indistinguishable from the frame under it. A fixture edit that
/// quietly drops either shows up here rather than passing.
///
/// No GPU, so it runs everywhere — including on the adapterless runner where the
/// capture above skips.
#[test]
fn the_fixture_still_puts_both_accumulations_live() {
    let value = |key: &str| -> f32 {
        FIXTURE
            .lines()
            .find_map(|line| {
                let rest = line.trim_start().strip_prefix(key)?;
                let rest = rest.trim_start().strip_prefix('=')?;
                rest.trim().trim_matches('"').parse::<f32>().ok()
            })
            .unwrap_or(f32::NAN)
    };
    Preset::from_toml_str(FIXTURE).expect("the attractor-with-trails fixture parses");

    let (fade, trails, spin) = (value("fade"), value("trails"), value("spin"));
    assert!(
        trails > fade,
        "attractor_trails.toml has trails = {trails} against the scene's own \
         fade = {fade}. The engine stage only shows up when its tail OUTLASTS \
         the scene's — at or below it, `max(cur, prev * trails)` is exactly \
         `cur` at every pixel and this baseline pins a passthrough"
    );
    assert!(
        spin != 0.0,
        "attractor_trails.toml no longer turns the figure (spin = {spin}), so \
         neither accumulation holds anything the current frame does not already \
         show and a trail is indistinguishable from a passthrough"
    );
}
