//! **What a shaped mark costs, per frame** (Plan 0070 Phase 4,
//! ADR-0084).
//!
//! The silhouette roster puts a branch in the hottest fragment shader in the
//! engine: the swarm draws its whole tier of sprites every frame, and every
//! fragment of every one of them now selects a distance function. ADR-0084 books
//! that as a known negative and this file is where it gets a number.
//!
//! # This is a measurement, and it names its machine
//!
//! Per ADR-0071 a numeric contract states a property or names the configuration
//! it was taken on. A frame time is the second kind — it is a fact about a GPU, a
//! driver and a window size, not about the code — so **there is no threshold
//! here.** The test renders the cases, prints what it saw, and asserts only that
//! it measured genuinely different shaders. Nothing about `main` being green
//! depends on the numbers, which is exactly the point: a threshold on this would
//! be a re-measurement waiting for the next runner image.
//!
//! It also skips on a software rasterizer, with a notice. WARP's frame time says
//! nothing about the iGPU floor in `docs/nfr.md` §7, and a reading taken there
//! would be a number that looks like evidence and is not.
//!
//! # Three probes, because two would not have separated the two costs
//!
//! A shaped mark changes two things at once: it adds arithmetic to the fragment,
//! and it lights **less of the sprite quad**. Measured as disc-against-star those
//! are confounded, and the confound is not small — a seven-pointed star at
//! `STAR_INNER = 0.45` covers 34 % of the quad where a disc covers 78 %, and a
//! swarm frame at this size is dominated by blend traffic through 10 000
//! overdrawing quads rather than by ALU. So the star reads **faster** than the
//! disc, reproducibly, which says nothing at all about the branch.
//!
//! The third probe is the isolate: a **12-sided polygon**, which covers 75 % of
//! the quad — within 5 % of the disc's coverage — and takes the full `atan2` +
//! `floor` + `cos` path. Disc against polygon-12 is the arithmetic priced at
//! matched coverage; disc against star-7 is what a shaped silhouette actually
//! costs in a frame.
//!
//! **The reading, on the machine Plan 0070 was implemented on** — Windows 10
//! 19045, DX12, NVIDIA GeForce RTX 3080 Laptop GPU, **release** profile,
//! 1280x720, the floor tier's 10 000 swarm particles at `size = 3`, best of
//! three interleaved repeats, itself the median of three such runs:
//!
//! | mark | quad coverage | per-frame | vs `disc` |
//! |---|---|---|---|
//! | `disc` — the default, `length(local)` | 78 % | **0.877 ms** | — |
//! | `polygon`, 12 sides (matched coverage) | 75 % | **0.858 ms** | **-0.019 ms, -2 %** |
//! | `star`, 7 points (the shaped figure) | 34 % | **0.710 ms** | **-0.167 ms, -19 %** |
//!
//! The run-to-run spread on the polygon delta is about `0.01 ms` (-1.4 % to -2.4
//! % across three runs), so **the branch's arithmetic is below the resolution of
//! this measurement** — even at matched coverage the shaped arm is not slower
//! than the disc, and a shaped silhouette is a clear net saving because it
//! lights a third of the quad instead of three quarters. In the **debug**
//! profile the same probes read 1.974 / 1.965 / 1.981 ms, i.e. all three within
//! 0.5 % of each other, because the 10 000-particle CPU update dominates there.
//!
//! Against `docs/nfr.md` §7's 16.7 ms budget none of this is a number worth
//! tuning for, and ADR-0084's fallback — separate pipelines per shape, with its
//! own WARP hazard under ADR-0058 — stays unneeded. Two things make that
//! unsurprising rather than suspicious: the selection is **per draw**, so the
//! branch is uniform across a warp and the hardware takes one arm rather than
//! both; and a swarm frame at this size is bandwidth-bound through 10 000
//! overdrawing quads long before it is ALU-bound.
//!
//! One `#[test]` per file (its own binary → its own process), so the hardware
//! device never coexists with the other suites' WARP ones.

// The determinism gate bans wall-clock reads because analysis must be a pure
// function of its input (clippy.toml, NFR §6). This file is the deliberate
// exception the gate's own comment anticipates — the same shape
// `one_hop_analyzes_well_under_the_hop_interval` in `tests/dsp.rs` carries — and
// nothing under test reads a clock: the renderer is driven by an injected frame
// count, and the timing happens strictly outside it.
#![allow(
    clippy::disallowed_methods,
    reason = "a frame-cost report deliberately times execution; the render under test stays clock-free"
)]

/// The shared ADR-0016 skip and headless constructors.
mod common;

use std::time::Instant;

use lmv_core::dsp::AnalysisFrame;
use lmv_core::preset::Preset;
use lmv_core::render::{CaptureImage, Renderer};

/// A realistic frame for this scene, not a stress test: the size the standalone
/// opens at, where the swarm is one of the heavier families.
const WIDTH: u32 = 1280;
const HEIGHT: u32 = 720;

/// Frame counts either side of the slope. The short run pays the same fixed
/// costs as the long one — preset load, first-frame pipeline warm, the single
/// readback — so the difference divided by the gap is the per-frame cost with
/// all of that subtracted out.
const FRAMES_SHORT: u32 = 30;
const FRAMES_LONG: u32 = 270;

/// How many times each case is measured. The **minimum** is kept, not the mean:
/// a scheduler hiccup can only add time, so the smallest reading is the one
/// least contaminated by everything that is not the render.
const REPEATS: usize = 3;

/// The probes, in report order: the default, the matched-coverage isolate, and
/// the shaped figure. `(label, shape, points)`.
const CASES: [(&str, &str, &str); 3] = [
    ("disc      ", "0", "7"),
    ("polygon12 ", "2", "12"),
    ("star(7)   ", "3", "7"),
];

/// A swarm preset differing from its siblings **only** in its mark silhouette.
fn probe(index: usize) -> Preset {
    let (_, shape, points) = CASES[index];
    let toml = format!(
        "system = \"swarm\"\nname = \"cost_{index}\"\n[params]\n\
         force = \"0.8\"\nspin = \"0.1\"\nburst = \"0.3\"\nhue = \"0.6\"\n\
         brightness = \"0.9\"\nsize = \"3.0\"\npoints = \"{points}\"\nshape = \"{shape}\"\n"
    );
    Preset::from_toml_str(&toml).expect("the cost probe preset parses")
}

/// Build a **hardware** headless renderer, or `None` (a logged skip) when the
/// runner has no adapter or only a software one.
fn hardware() -> Option<Renderer> {
    common::headless_hardware_for(WIDTH, HEIGHT, None, common::NEEDS_HARDWARE_FOR_TIMING)
}

/// Per-frame milliseconds for each probe, plus the frame each drew.
///
/// **The cases are interleaved inside one loop rather than measured one series
/// after another**, and that is not tidiness: whichever case runs last otherwise
/// inherits a GPU that has finished ramping its clocks, which is worth more than
/// the effect being measured. Alternating puts every case on the same drift,
/// which is ADR-0071's "control taken in the same run" applied to time instead of
/// to pixels.
fn per_frame_ms(renderer: &mut Renderer) -> (Vec<f64>, Vec<CaptureImage>) {
    let presets: Vec<Preset> = (0..CASES.len()).map(probe).collect();
    let names: Vec<String> = presets.iter().map(|p| p.name.clone()).collect();
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
        .map(|name| run(renderer, name, FRAMES_SHORT).1)
        .collect();

    let mut best = vec![f64::INFINITY; names.len()];
    for _ in 0..REPEATS {
        for (slot, name) in best.iter_mut().zip(names.iter()) {
            let (short, _) = run(renderer, name, FRAMES_SHORT);
            let (long, _) = run(renderer, name, FRAMES_LONG);
            *slot = slot.min((long - short) / f64::from(FRAMES_LONG - FRAMES_SHORT));
        }
    }
    (best, images)
}

/// **The shaped-mark branch, priced against the disc case it replaces.**
///
/// Reports; does not gate. See the module docs for the reading this was written
/// against, for the third probe's job, and for why there is no threshold.
#[test]
fn a_shaped_mark_is_priced_against_the_disc() {
    let Some(mut renderer) = hardware() else {
        return;
    };
    let (times, frames) = per_frame_ms(&mut renderer);
    let disc = times[0];

    let mut report = format!(
        "mark cost at {WIDTH}x{HEIGHT}, {} frames of slope, best of {REPEATS}, \
         interleaved (ADR-0071 report):",
        FRAMES_LONG - FRAMES_SHORT
    );
    for (index, (label, shape, points)) in CASES.iter().enumerate() {
        let ms = times[index];
        report.push_str(&format!(
            "\n  {label} shape={shape} points={points}  {ms:.3} ms/frame  \
             ({:+.3} ms, {:+.1} % vs disc)",
            ms - disc,
            (ms / disc - 1.0) * 100.0
        ));
    }
    eprintln!("{report}");

    // Non-vacuity, and the only thing asserted: every case really rendered, and
    // each really rendered a *different* shader — three readings off one arm
    // would be very stable numbers about nothing.
    for (index, (label, _, _)) in CASES.iter().enumerate() {
        let ms = times[index];
        assert!(
            ms.is_finite() && ms > 0.0,
            "the {} reading is not a time: {ms}",
            label.trim()
        );
    }
    let total = (WIDTH * HEIGHT) as usize;
    for (index, (label, _, _)) in CASES.iter().enumerate().skip(1) {
        let differing = frames[0]
            .rgba
            .chunks_exact(4)
            .zip(frames[index].rgba.chunks_exact(4))
            .filter(|(a, b)| a[..3] != b[..3])
            .count();
        eprintln!(
            "  {} differs from disc in {differing} of {total} pixels",
            label.trim()
        );
        assert!(
            differing * 50 > total,
            "{} drew the same picture as the disc, so this measured one shader twice: \
             {differing} of {total} pixels differ",
            label.trim()
        );
    }
}
