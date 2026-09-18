//! **What the level coordinate costs the present pass, per frame** (Plan 0184
//! Phase 2, ADR-0197).
//!
//! `color_source = 1` moves `warp_mesh`'s palette read from the deposit pass —
//! one fullscreen pass over a field-sized target — to the **present** pass, at
//! display resolution, and adds the contour's four LUT samples on top when the
//! contour is on. ADR-0197 books that as its last negative; this is where it gets
//! a number.
//!
//! # This is a measurement, and it names its machine
//!
//! Per ADR-0071 a frame time is a fact about a GPU, a driver, a build profile and
//! a window size rather than about the code, so **there is no threshold here**.
//! The sweep renders the four configurations, prints what it saw, and asserts
//! only that it genuinely rendered different ones. It **skips on a software
//! rasterizer, with a notice**, for `collage_cost.rs`'s reason: a WARP frame time
//! says nothing about the iGPU floor `docs/nfr.md` §7 is written against.
//!
//! The four rungs vary only `color_source` and `palette_contour`, on one fixture
//! whose every other binding is a constant, so the differences between them are
//! the two switches and nothing else. They are **interleaved** inside one loop,
//! which is what keeps a GPU ramping its clocks from being read as a cost.
//!
//! One `#[test]` per file (its own binary → its own process), so the hardware
//! device never coexists with the other suites' WARP ones.

// The determinism gate bans wall-clock reads because analysis must be a pure
// function of its input (clippy.toml, NFR §6). This file is the deliberate
// exception the gate's own comment anticipates — the same shape `collage_cost.rs`
// carries — and nothing under test reads a clock: the renderer is driven by an
// injected frame count, and the timing happens strictly outside it.
#![allow(
    clippy::disallowed_methods,
    reason = "a frame-cost report deliberately times execution; the render under test stays clock-free"
)]

/// The shared ADR-0016 skip and headless constructors.
mod common;

use std::time::Instant;

use rlx_core::dsp::AnalysisFrame;
use rlx_core::preset::Preset;
use rlx_core::render::Renderer;

/// **1080p, because that is the size `docs/nfr.md` §1 states its budget at**, and
/// because the cost being measured is per display pixel: at a smaller target it
/// would understate itself proportionally.
const WIDTH: u32 = 1920;
const HEIGHT: u32 = 1080;

/// Frame counts either side of the slope. The short run pays the same fixed costs
/// as the long one — preset load, first-frame pipeline warm, the single readback
/// — so the difference divided by the gap is the per-frame cost with all of that
/// subtracted out.
const FRAMES_SHORT: u32 = 10;
const FRAMES_LONG: u32 = 110;

/// How many times each rung is measured; the **minimum** of each leg is kept, and
/// the two legs are minimized separately and subtracted once (ADR-0173).
const REPEATS: usize = 3;

/// The ladder fixture — the same text `core/tests/suite/warp_mesh.rs` asserts on,
/// so the cost is priced on the picture the behaviour was measured on.
const LADDER: &str = include_str!("fixtures/warp_mesh_ladder.toml");

/// The four rungs: both colour paths, with the contour off and on.
const RUNGS: [(u32, f32); 4] = [(0, 0.0), (0, 1.0), (1, 0.0), (1, 1.0)];

/// The ladder fixture at one rung. Only the two switches move; the deposit, the
/// decay, the zoom, the palette and the step count are the fixture's own.
fn probe(source: u32, contour: f32) -> Preset {
    let text = LADDER
        .replace(
            "color_source    = \"1\"",
            &format!("color_source = \"{source}\""),
        )
        .replace(
            "palette_contour = \"0\"",
            &format!("palette_contour = \"{contour}\""),
        )
        .replace(
            "name   = \"fixture_warp_mesh_ladder\"",
            &format!("name   = \"cost_{source}_{contour}\""),
        );
    Preset::from_toml_str(&text).expect("the cost probe preset parses")
}

/// Per-frame milliseconds for each rung, interleaved.
fn per_frame_ms(renderer: &mut Renderer) -> Vec<f64> {
    let presets: Vec<Preset> = RUNGS.iter().map(|&(s, c)| probe(s, c)).collect();
    let names: Vec<String> = presets.iter().map(|p| p.name.clone()).collect();
    renderer.set_presets(presets);
    let frame = AnalysisFrame::default();
    let run = |renderer: &mut Renderer, name: &str, frames: u32| -> f64 {
        let start = Instant::now();
        renderer
            .capture_preset(name, &frame, frames)
            .expect("capture the cost probe");
        start.elapsed().as_secs_f64() * 1000.0
    };

    // One untimed pass each, so shader compilation and first-use allocation are
    // behind us before anything is measured.
    for name in &names {
        run(renderer, name, FRAMES_SHORT);
    }

    let mut best_short = vec![f64::INFINITY; names.len()];
    let mut best_long = vec![f64::INFINITY; names.len()];
    for _ in 0..REPEATS {
        for (index, name) in names.iter().enumerate() {
            let short = run(renderer, name, FRAMES_SHORT);
            let long = run(renderer, name, FRAMES_LONG);
            best_short[index] = best_short[index].min(short);
            best_long[index] = best_long[index].min(long);
        }
    }
    best_long
        .iter()
        .zip(best_short.iter())
        .map(|(long, short)| (long - short) / f64::from(FRAMES_LONG - FRAMES_SHORT))
        .collect()
}

/// **The level coordinate, priced against the deposit-angle one.**
///
/// Reports; does not gate. See the module docs.
#[test]
fn the_level_coordinate_is_priced_against_the_deposit_angle_one() {
    let Some(mut renderer) =
        common::headless_hardware_for(WIDTH, HEIGHT, None, common::NEEDS_HARDWARE_FOR_TIMING)
    else {
        return;
    };
    let adapter = renderer.adapter_description().to_string();
    let times = per_frame_ms(&mut renderer);
    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    const BUDGET_MS: f64 = 1000.0 / 60.0;

    println!(
        "warp_mesh colour path at {WIDTH}x{HEIGHT}, {} frames of slope, best of \
         {REPEATS}, interleaved (ADR-0071 report)",
        FRAMES_LONG - FRAMES_SHORT
    );
    println!("  adapter: {adapter}");
    println!("  profile: {profile}, fixture warp_mesh_ladder.toml");
    let base = times.first().copied().unwrap_or(0.0);
    for (rung, ms) in RUNGS.iter().zip(times.iter()) {
        let (source, contour) = *rung;
        println!(
            "  color_source {source}, palette_contour {contour:.1}   {ms:>7.3} ms/frame \
             {:>6.1} % of {BUDGET_MS:.2} ms   ({:+.3} ms vs the angle path with no contour)",
            100.0 * ms / BUDGET_MS,
            ms - base
        );
    }

    // Non-vacuity: four readings of the same number would be a very stable report
    // about nothing.
    assert!(
        times.len() == RUNGS.len() && times.iter().all(|ms| *ms > 0.0),
        "the sweep produced {times:?}, which is not four positive frame times"
    );
}
