//! **What an authored contour costs per frame, and what its arity ceiling is**
//! (ADR-0107).
//!
//! ADR-0107 chose a per-pixel distance field over a triangulated fill on an
//! arithmetic *construction*: 2.07 M pixels x `N` segments x ~10 ops against a
//! nominal integrated-GPU throughput, giving "~2 % of a current integrated GPU
//! at `N = 32`" and "still order 8 %" at `N = 128`. The ADR's own Notes label
//! that as an estimate with nothing rendered behind it, and say Plan 0092
//! measures it before the ceiling is set. This file is that measurement.
//!
//! # This is a measurement, and it names its machine
//!
//! Per ADR-0071 a numeric contract states a property or names the configuration
//! it was taken on. A frame time is the second kind, so **there is no threshold
//! here**: the test renders the cases, prints what it saw, and asserts only that
//! it measured genuinely different work. Nothing about `main` being green
//! depends on the numbers.
//!
//! It skips on a software rasterizer, with a notice — a WARP frame time says
//! nothing about `docs/nfr.md` §1's floor, and a reading taken there would look
//! like evidence without being one.
//!
//! # The reading, and what it did to the ceiling
//!
//! **On the machine Plan 0092 was implemented on** — Windows 10 19045, DX12,
//! AMD Radeon(TM) Graphics (IntegratedGpu), debug profile, **1920x1080** (§1's
//! own floor resolution), floor tier, best of three interleaved repeats:
//!
//! | case | ms/frame | of the 16.67 ms floor budget |
//! |---|---|---|
//! | `heart` (roster control, no path) | 1.03 | 6 % |
//! | path, `samples = 16` | 2.69 | 16 % |
//! | path, `samples = 32` | 4.33 | 26 % |
//! | path, `samples = 64` | 7.70 | 46 % |
//!
//! **The slope is the finding: ~0.105 ms per segment**, flat across the range,
//! against a construction estimate of ~2 % of such a GPU at 32 segments. The
//! measurement says 26 % there — an order of magnitude out, and the estimate is
//! what was wrong. Two readings taken before the ceiling came down and now out
//! of reach of this test, kept because they are what set it: `samples = 128`
//! measured 14.51 ms (87 %) and `samples = 192` measured 21.22 ms (127 %) — the
//! second over the whole floor budget on its own, with nothing else in the
//! frame.
//!
//! # What the ceiling is, and why it is a refusal rather than a cap
//!
//! [`MAX_SAMPLES`] is the arity a `[path]` may ask for, and exceeding it is a
//! **load error naming both numbers** rather than a silent decimation. The cost
//! this file measures is paid at every pixel of every frame whether or not the
//! figure is on screen — a fullscreen field evaluates everywhere — so an author
//! who pastes a 500-point traced logo has to be told, not quietly given a figure
//! they did not draw.
//!
//! Because the ceiling is a load error, **every arity this test can render is
//! one the ceiling allows**: the two readings above it are recorded in prose
//! rather than measured here, and the slope is what carries them. What the test
//! still checks is that the slope exists — that the arity, not something else,
//! is what it priced.

// The determinism gate bans wall-clock reads because analysis must be a pure
// function of its input (clippy.toml, NFR §6). This file is the deliberate
// exception the gate's own comment anticipates — `field_cost.rs` and
// `mark_cost.rs` carry the same one — and nothing under test reads a clock: the
// renderer is driven by an injected frame count, and the timing happens strictly
// outside it.
#![allow(
    clippy::disallowed_methods,
    reason = "a frame-cost report deliberately times execution; the render under test stays clock-free"
)]

/// The shared ADR-0016 skip and headless constructors.
mod common;

use std::time::Instant;

use rlx_core::dsp::AnalysisFrame;
use rlx_core::preset::Preset;
use rlx_core::preset::path::MAX_SAMPLES;
use rlx_core::render::{CaptureImage, Renderer, Tier};

/// **`docs/nfr.md` §1's own floor resolution**, not a convenient smaller one.
/// The ceiling this file sets is a claim about the floor commitment, so it is
/// measured at the size that commitment is written at.
const WIDTH: u32 = 1920;
const HEIGHT: u32 = 1080;

/// Frame counts either side of the slope. The short run pays the same fixed
/// costs as the long one — preset load, first-frame pipeline warm, the single
/// readback — so the difference divided by the gap is the per-frame cost with
/// all of that subtracted out.
const FRAMES_SHORT: u32 = 20;
const FRAMES_LONG: u32 = 140;

/// How many times each case is measured; the **minimum** of each leg is kept.
/// A scheduler hiccup can only add time, so the smallest reading of a duration
/// is the least contaminated. The two legs are minimized separately and
/// subtracted once, after the loop — a minimum over their *difference* would
/// select the repeat whose short leg was most inflated (ADR-0173).
const REPEATS: usize = 3;

/// The arities priced. `8` is a coarse polygon and [`MAX_SAMPLES`] is the
/// ceiling, which is also the `[path]` default — every value here loads, because
/// one over the ceiling is a load error and could not be rendered to time.
const ARITIES: [usize; 5] = [8, 16, 32, 48, MAX_SAMPLES];

/// A leaf: two cubics meeting at a cusp top and bottom. Not reachable by any arm
/// of the `marks` roster, which is what makes it the right figure to price — a
/// path that a `shape` selector could have drawn would be measuring a detour.
const LEAF: &str = "M 0,-1 C 0.9,-0.4 0.9,0.4 0,1 C -0.9,0.4 -0.9,-0.4 0,-1 Z";

/// The shared params, so every case below differs only in where its silhouette
/// comes from and how many segments it has.
const LOOK: &str = "scale = \"0.6\"\ncolor_span = \"0.35\"\npalette_steps = \"9\"\n\
                    palette_contour = \"0.6\"\n";

/// The roster control: the same scene drawing a closed-form arm, so the
/// difference between it and a path case is the contour walk and nothing else.
fn control() -> Preset {
    let toml = format!(
        "system = \"shape_field\"\nname = \"path_cost_roster\"\n[params]\nshape = \"4\"\n{LOOK}"
    );
    Preset::from_toml_str(&toml).expect("the roster control preset parses")
}

fn path_probe(samples: usize) -> Preset {
    let toml = format!(
        "system = \"shape_field\"\nname = \"path_cost_{samples}\"\n\
         [path]\nd = \"{LEAF}\"\nsamples = {samples}\n[params]\n{LOOK}"
    );
    Preset::from_toml_str(&toml).expect("the path cost probe preset parses")
}

/// Build a **hardware** headless renderer at the floor tier, or `None` (a logged
/// skip) when the runner has no adapter or only a software one.
fn hardware() -> Option<Renderer> {
    common::headless_hardware_for(
        WIDTH,
        HEIGHT,
        Some(Tier::Floor),
        common::NEEDS_HARDWARE_FOR_TIMING,
    )
}

/// **The per-pixel contour walk, priced against the closed-form roster it sits
/// beside, across the arity range the ceiling covers.**
///
/// Reports; does not gate. See the module docs for why there is no threshold.
#[test]
fn the_contour_arity_is_priced_against_the_floor_tier() {
    let Some(mut renderer) = hardware() else {
        return;
    };

    let mut presets = vec![control()];
    let mut names = vec![presets[0].name.clone()];
    for samples in ARITIES {
        let p = path_probe(samples);
        names.push(p.name.clone());
        presets.push(p);
    }
    renderer.set_presets(presets);

    let frame = AnalysisFrame {
        bass: 0.6,
        mid: 0.5,
        treb: 0.6,
        onset: 0.4,
        ..Default::default()
    };
    let run = |renderer: &mut Renderer, name: &str, frames: u32| -> (f64, CaptureImage) {
        let start = Instant::now();
        let image = renderer
            .capture_preset(name, &frame, frames)
            .expect("capture the cost probe");
        (start.elapsed().as_secs_f64() * 1000.0, image)
    };

    // One untimed pass each, so shader compilation and first-use allocation are
    // behind us before anything is measured.
    let images: Vec<CaptureImage> = names
        .iter()
        .map(|name| run(&mut renderer, name, FRAMES_SHORT).1)
        .collect();

    // Interleaved rather than one series after another: whichever case ran last
    // would otherwise inherit a GPU that has finished ramping its clocks, which
    // is worth more than the effect being measured (ADR-0071's "control taken in
    // the same run", applied to time).
    let mut best_short = vec![f64::INFINITY; names.len()];
    let mut best_long = vec![f64::INFINITY; names.len()];
    for _ in 0..REPEATS {
        for (index, name) in names.iter().enumerate() {
            let (short, _) = run(&mut renderer, name, FRAMES_SHORT);
            let (long, _) = run(&mut renderer, name, FRAMES_LONG);
            best_short[index] = best_short[index].min(short);
            best_long[index] = best_long[index].min(long);
        }
    }
    let best: Vec<f64> = best_long
        .iter()
        .zip(best_short.iter())
        .map(|(long, short)| (long - short) / f64::from(FRAMES_LONG - FRAMES_SHORT))
        .collect();

    let roster = best[0];
    let mut report = format!(
        "shape_field authored-contour cost at {WIDTH}x{HEIGHT}, floor tier, {} frames of \
         slope, best of {REPEATS}, interleaved, on {} (ADR-0071 report):\n  \
         {:<28} {:>12} {:>12} {:>12}",
        FRAMES_LONG - FRAMES_SHORT,
        renderer.adapter_description(),
        "case",
        "ms/frame",
        "vs roster",
        "of 16.67 ms"
    );
    report.push_str(&format!(
        "\n  {:<28} {roster:>9.3} ms {:>12} {:>11.1} %",
        "heart (roster control)",
        "—",
        roster / 16.67 * 100.0
    ));
    for (index, samples) in ARITIES.iter().enumerate() {
        let ms = best[index + 1];
        report.push_str(&format!(
            "\n  {:<28} {ms:>9.3} ms {:>+9.3} ms {:>11.1} %",
            format!("path, samples = {samples}"),
            ms - roster,
            ms / 16.67 * 100.0
        ));
    }
    eprintln!("{report}");

    // Non-vacuity, and the only thing asserted. Every case really rendered...
    for (index, name) in names.iter().enumerate() {
        assert!(
            best[index].is_finite() && best[index] > 0.0,
            "the {name} reading is not a time: {}",
            best[index]
        );
    }

    // ...every path case really drew a path rather than falling back to the
    // roster's heart...
    let total = (WIDTH * HEIGHT) as usize;
    for (index, samples) in ARITIES.iter().enumerate() {
        let differing = images[0]
            .rgba
            .chunks_exact(4)
            .zip(images[index + 1].rgba.chunks_exact(4))
            .filter(|(a, b)| a[..3] != b[..3])
            .count();
        assert!(
            differing * 50 > total,
            "samples = {samples} drew the roster's figure, so this timed the \
             control twice: {differing} of {total} pixels differ from it"
        );
    }

    // ...and the arity is genuinely what is being priced: the ceiling costs
    // measurably more than the coarsest case. Stated as an inequality between
    // two readings in the same run rather than as a threshold on either.
    let (coarse, ceiling) = (best[1], best[ARITIES.len()]);
    assert!(
        ceiling > coarse,
        "samples = {} ({ceiling:.3} ms) did not measure as more work than \
         samples = {} ({coarse:.3} ms), so the contour walk is not what this \
         priced",
        MAX_SAMPLES,
        ARITIES[0]
    );
}
