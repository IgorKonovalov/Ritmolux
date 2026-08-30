//! **What the scaled-copy coordinate costs, per frame** (Plan 0098 Phase 5,
//! ADR-0111).
//!
//! ADR-0111 argues that the closed forms are "a handful of ALU ops, the same
//! order as the SDF they sit beside", and that this is what makes the second
//! coordinate a mode rather than a feature with a cost gate on it. That is an
//! argument, and this file is where it becomes a measurement.
//!
//! `shape_field` is fullscreen and unconditional — every pixel of every frame
//! evaluates the roster once — so a second shape evaluation is exactly the kind
//! of thing that could have been unaffordable. It is also the scene ADR-0105
//! already flagged as this family's weak point.
//!
//! # This is a measurement, and it names its machine
//!
//! Per ADR-0071 a numeric contract states a property or names the configuration it was taken
//! on. A frame time is the second kind — a fact about a GPU, a driver and a
//! window size — so **there is no threshold here.** The test renders the cases,
//! prints what it saw, and asserts only that it measured genuinely different
//! shaders. Nothing about `main` being green depends on the numbers.
//!
//! It skips on a software rasterizer, with a notice: WARP's frame time says
//! nothing about the floor in `docs/nfr.md` §1, and a reading taken there would
//! look like evidence and not be one.
//!
//! # The pairs, and the control
//!
//! Each figure is rendered under both modes and nothing else changes, so the
//! delta is the coordinate and not the shape. Three figures, chosen for what
//! they price:
//!
//! - **`heart`** — the reference construction, and a closed form on both sides.
//! - **`star`, straight** — the other closed form, with an angular fold in it.
//! - **`star`, curved and jittered** — the only arm with a **loop** on either
//!   side. The distance walks two polylines (the edge and the unjittered
//!   reference, Plan 0098 Phase 1); the radius walks one and computes only the
//!   ray crossing. This is the pair that could have gone either way.
//! - **`disc`** — the control. `mark_distance` is `length(p)` and the ratio is
//!   `length(p) / 1`, so the two modes are the same arithmetic and the delta is
//!   the measurement's own noise floor. A disc pair reading far from zero
//!   convicts the harness rather than the coordinate.
//!
//! # The reading, and the verdict on ADR-0111's claim
//!
//! **On the machine Plan 0098 was implemented on** — Windows 10 19045, DX12,
//! **AMD Radeon(TM) Graphics (IntegratedGpu)**, driver 30.0.13002.1001, debug
//! profile, 1280x720, floor tier, best of three interleaved repeats, and the
//! table below is the **median of three such runs**. (The integrated adapter is
//! the one `wgpu` selects here, and it is the apter of the two for a floor-tier
//! reading — `docs/nfr.md` §1's floor is an iGPU-class baseline.)
//!
//! | figure | distance | radius | delta | vs distance |
//! |---|---|---|---|---|
//! | `disc` — the control | 0.633 ms | 0.620 ms | -0.014 ms | **-1.1 %** |
//! | `heart` | 0.624 ms | 0.621 ms | -0.003 ms | **-0.5 %** |
//! | `star`, 7 points, straight | 0.697 ms | 0.640 ms | -0.033 ms | **-4.7 %** |
//! | `star`, curved + jittered | 1.217 ms | 0.902 ms | -0.315 ms | **-25.9 %** |
//!
//! **The control sets the noise floor at about ±3 %** (it read -1.1 %, -2.2 %
//! and +3.4 % across the three runs on arithmetic that is *identical* between
//! the two modes). Read the rest against that:
//!
//! - the closed-form arms cost **nothing measurable** — `heart` and `star`
//!   land inside the control's own spread, which is ADR-0111's "handful of ALU
//!   ops, the same order as the SDF they sit beside", confirmed rather than
//!   asserted;
//! - the curved star is **cheaper by a quarter**, reproducibly (-25.5 %,
//!   -25.9 %, -29.3 %), and the reason is structural rather than lucky: the
//!   distance walks **two** sampled polylines there — the edge, and the
//!   unjittered reference Plan 0098 Phase 1 added — while the radius walks one
//!   and computes only the ray crossing, with no point-to-segment distance at
//!   all.
//!
//! So the mode ships ungated, and nothing in `docs/nfr.md` moves: the largest
//! reading here is 1.2 ms against that file's 16.67 ms frame budget. A negative
//! result was a legitimate outcome of this phase and did not occur.
//!
//! One `#[test]` per file (its own binary → its own process), so the hardware
//! device never coexists with the other suites' WARP ones.

// The determinism gate bans wall-clock reads because analysis must be a pure
// function of its input (clippy.toml, NFR §6). This file is the deliberate
// exception the gate's own comment anticipates — `mark_cost.rs` carries the same
// one — and nothing under test reads a clock: the renderer is driven by an
// injected frame count, and the timing happens strictly outside it.
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
/// opens at. `shape_field` covers every pixel, so this is its whole cost.
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

/// `(label, shape params)` — the figure, without a coordinate mode. Each is
/// rendered under both modes and nothing else differs.
const FIGURES: [(&str, &str); 4] = [
    ("disc (control)", "shape = \"0\"\n"),
    ("heart         ", "shape = \"4\"\n"),
    (
        "star(7)       ",
        "shape = \"3\"\npoints = \"7\"\nstar_valley = \"0.45\"\n",
    ),
    (
        "star curved   ",
        "shape = \"3\"\npoints = \"7\"\nstar_valley = \"0.45\"\nstar_curve = \"0.5\"\nstar_jitter = \"0.3\"\n",
    ),
];

/// A `shape_field` preset differing from its partner **only** in `coord_mode`.
fn probe(index: usize, mode: u32) -> Preset {
    let (_, shape) = FIGURES[index];
    let toml = format!(
        "system = \"shape_field\"\nname = \"cost_{index}_{mode}\"\n[params]\n\
         {shape}scale = \"0.6\"\ncolor_span = \"0.35\"\npalette_steps = \"9\"\n\
         palette_contour = \"0.6\"\ncoord_mode = \"{mode}\"\n"
    );
    Preset::from_toml_str(&toml).expect("the cost probe preset parses")
}

/// Build a **hardware** headless renderer, or `None` (a logged skip) when the
/// runner has no adapter or only a software one.
fn hardware() -> Option<Renderer> {
    common::headless_hardware_for(WIDTH, HEIGHT, None, common::NEEDS_HARDWARE_FOR_TIMING)
}

/// **The scaled-copy coordinate, priced against the distance it sits beside.**
///
/// Reports; does not gate. See the module docs for why there is no threshold and
/// for what the `disc` pair is doing in the list.
#[test]
fn the_radius_coordinate_is_priced_against_the_distance() {
    let Some(mut renderer) = hardware() else {
        return;
    };

    let mut presets = Vec::new();
    let mut names = Vec::new();
    for index in 0..FIGURES.len() {
        for mode in 0..2u32 {
            let p = probe(index, mode);
            names.push(p.name.clone());
            presets.push(p);
        }
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

    // **Interleaved rather than one series after another**: whichever case ran
    // last would otherwise inherit a GPU that has finished ramping its clocks,
    // which is worth more than the effect being measured. Alternating puts every
    // case on the same drift — ADR-0071's "control taken in the same run",
    // applied to time instead of to pixels.
    let mut best = vec![f64::INFINITY; names.len()];
    for _ in 0..REPEATS {
        for (slot, name) in best.iter_mut().zip(names.iter()) {
            let (short, _) = run(&mut renderer, name, FRAMES_SHORT);
            let (long, _) = run(&mut renderer, name, FRAMES_LONG);
            *slot = slot.min((long - short) / f64::from(FRAMES_LONG - FRAMES_SHORT));
        }
    }

    let mut report = format!(
        "shape_field coordinate cost at {WIDTH}x{HEIGHT}, floor tier, {} frames of \
         slope, best of {REPEATS}, interleaved, on {} (ADR-0071 report):\n  \
         {:<16} {:>12} {:>12} {:>12} {:>10}",
        FRAMES_LONG - FRAMES_SHORT,
        renderer.adapter_description(),
        "figure",
        "distance",
        "radius",
        "delta",
        "pct"
    );
    for (index, (label, _)) in FIGURES.iter().enumerate() {
        let (d, r) = (best[index * 2], best[index * 2 + 1]);
        report.push_str(&format!(
            "\n  {label:<16} {d:>9.3} ms {r:>9.3} ms {:>+9.3} ms {:>+9.1} %",
            r - d,
            (r / d - 1.0) * 100.0
        ));
    }
    eprintln!("{report}");

    // Non-vacuity, and the only thing asserted. First: every case really
    // rendered.
    for (index, name) in names.iter().enumerate() {
        assert!(
            best[index].is_finite() && best[index] > 0.0,
            "the {name} reading is not a time: {}",
            best[index]
        );
    }

    // Second: each PAIR really rendered two different shaders — except the disc,
    // where the two modes are the same arithmetic and identical frames are the
    // claim rather than the failure.
    let total = (WIDTH * HEIGHT) as usize;
    for (index, (label, _)) in FIGURES.iter().enumerate() {
        let differing = images[index * 2]
            .rgba
            .chunks_exact(4)
            .zip(images[index * 2 + 1].rgba.chunks_exact(4))
            .filter(|(a, b)| a[..3] != b[..3])
            .count();
        eprintln!(
            "  {} differs between the modes in {differing} of {total} pixels",
            label.trim()
        );
        if index == 0 {
            assert_eq!(
                differing, 0,
                "the disc control must draw the SAME figure under both modes — \
                 `mark_distance` is `length(p)` there and the ratio is \
                 `length(p) / 1`. A difference convicts the harness"
            );
        } else {
            assert!(
                differing * 50 > total,
                "{} drew the same picture under both modes, so this timed one \
                 shader twice: {differing} of {total} pixels differ",
                label.trim()
            );
        }
    }
}
