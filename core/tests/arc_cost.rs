//! **What an arc-drawn ring costs, per frame** (Plan 0087 Phase 3,
//! ADR-0098).
//!
//! The arc primitive trades vertices for **fill**. A `circle` motif is one
//! instance instead of `SMOOTH_SAMPLES`, but that instance rasterizes a bounding
//! box around the whole circle and shades every fragment in it, where the
//! polyline shaded only thin quads along its chords. On a dense ornament those
//! boxes overlap, so the shaded area is larger than the stroke area — fill-rate
//! work on the hardware least able to pay it, which is ADR-0098's own named risk
//! and the stop condition this file measures against.
//!
//! # This is a measurement, and it names its machine
//!
//! Per ADR-0071 a numeric contract states a property or names the configuration
//! it was taken on. A frame time is the second kind, so **there is no threshold
//! here.** The test renders the cases, prints what it saw, and asserts only that
//! it measured genuinely different figures. It skips on a software rasterizer:
//! WARP's frame time says nothing about the iGPU floor in `docs/nfr.md` §1, and
//! a reading taken there would be a number that looks like evidence and is not.
//! Its shape is `mark_cost.rs`'s — interleaved cases, a two-length slope to
//! subtract the fixed costs, the minimum of several repeats — and that file's
//! header explains each of those choices.
//!
//! # Four probes, because the ring's cost is not one number
//!
//! The three ring probes share a rosette, and each is measured against the
//! **rosette alone**, so what is reported is the ring's own marginal cost rather
//! than a whole frame's.
//!
//! - `arcs x40` at motif scale 0.13 — the retired `star_mandala`'s outermost
//!   ring, and the figure the arc primitive exists to make shippable. At this scale the
//!   forty boxes barely touch.
//! - `arcs x40` at motif scale 0.46 — the top of the range
//!   ADR-0098
//!   quotes, where the boxes overlap heavily. This is the fill-rate case, and it
//!   is here because a measurement taken only at 0.13 would price the primitive
//!   at its cheapest and call it priced.
//! - `petals x40` at motif scale 0.13 — the nearest **polyline** stand-in the
//!   roster still offers: the same count, the same `SMOOTH_SAMPLES` vertices,
//!   the same scale, 960 segments against 40 arcs. It is a stand-in and not the
//!   control, and the difference matters: a petal is a flatter figure than a
//!   circle, so it lights less of the frame, and the comparison flatters the
//!   polyline slightly. A true control would need the engine to still be able to
//!   draw a circle as a polyline, which after this phase it cannot.
//!
//! # The reading
//!
//! **On the machine Plan 0087 was implemented on** — Windows 10 19045, DX12,
//! NVIDIA GeForce RTX 3080 Laptop GPU, **release** profile, 1920x1080, Floor
//! tier, best of three interleaved repeats. See the printed table for the run
//! this file was committed against; the numbers live in Plan 0087's
//! implementation log rather than here, so a re-run has one place to update.
//!
//! **The run-to-run spread is about 0.06 ms on the marginal figures**, measured
//! across three consecutive runs of this file. That is larger than the gap
//! between the sparse arc ring and the polyline stand-in, so **those two are not
//! separated by this measurement** — the honest statement is that an arc ring at
//! ornament scale costs no more than the polyline it replaces, not that it costs
//! less. The dense case's marginal cost is several times the spread and is a
//! real reading.
//!
//! **This box is not the floor tier's baseline hardware**, which `docs/nfr.md`
//! §1 puts at an iGPU. So a reading here can convict and cannot acquit: a
//! marginal cost that already eats the 16.67 ms budget on a discrete laptop GPU
//! would be a stop, while one that is a rounding error here still owes an
//! on-device check before anyone calls the floor safe. That asymmetry is the
//! honest reading of a number taken on the wrong machine, and it is why the
//! comparison against the polyline stand-in is reported beside the absolute
//! cost — a primitive that is *cheaper than what it replaces* is safe on any
//! machine the polyline was safe on.
//!
//! One `#[test]` per file (its own binary → its own process), so the hardware
//! device never coexists with the other suites' WARP ones.

// The determinism gate bans wall-clock reads because analysis must be a pure
// function of its input (clippy.toml, NFR §6). This file is the deliberate
// exception the gate's own comment anticipates — the same shape `mark_cost.rs`
// carries — and nothing under test reads a clock: the renderer is driven by an
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
use lmv_core::render::{CaptureImage, Renderer, Tier};

/// The floor commitment's own resolution (`docs/nfr.md` §1), not the size the
/// standalone happens to open at: this measurement exists to be read against
/// that commitment.
const WIDTH: u32 = 1920;
const HEIGHT: u32 = 1080;

/// The per-frame budget 60 fps buys, in milliseconds — the number the stop
/// condition is stated against. Printed, never asserted on: see the header.
const BUDGET_MS: f64 = 1000.0 / 60.0;

/// Frame counts either side of the slope. The short run pays the same fixed
/// costs as the long one — preset load, first-frame pipeline warm, the single
/// readback — so the difference divided by the gap is the per-frame cost with
/// all of that subtracted out.
const FRAMES_SHORT: u32 = 30;
const FRAMES_LONG: u32 = 210;

/// How many times each case is measured. The **minimum** is kept, not the mean:
/// a scheduler hiccup can only add time, so the smallest reading is the one
/// least contaminated by everything that is not the render.
const REPEATS: usize = 3;

/// Forty copies, which is what a mandala's outer ring carries and what
/// ADR-0079's budget arithmetic was quoted against.
const RING_COUNT: u32 = 40;

/// `(label, rings-table body)` — the baseline first, then the three rings.
const CASES: [(&str, &str); 4] = [
    ("rosette only ", ""),
    (
        "arcs x40 s.13",
        "rings = [{ motif = \"circle\", count = 40, radius = 0.70, scale = 0.13 }]\n",
    ),
    (
        "arcs x40 s.46",
        "rings = [{ motif = \"circle\", count = 40, radius = 0.70, scale = 0.46 }]\n",
    ),
    (
        "petal x40 s.13",
        "rings = [{ motif = \"petal\", count = 40, radius = 0.70, scale = 0.13 }]\n",
    ),
];

/// A `star_pattern` preset differing from its siblings **only** in its ring
/// roster, so the difference between two readings is the ring and nothing else.
fn probe(index: usize) -> Preset {
    let (label, rings) = CASES[index];
    let toml = format!(
        "system = \"star_pattern\"\nname = \"arc_cost_{index}\"\n\
         [generator]\ntiling = \"12\"\ncontact_angle_deg = 35\n{rings}\
         [params]\nvariant = \"0\"\nrotation = \"0\"\nhue = \"0.5\"\n\
         draw_progress = \"1\"\nthickness = \"3\"\nscale = \"0.9\"\nbrightness = \"0.9\"\n"
    );
    Preset::from_toml_str(&toml)
        .unwrap_or_else(|e| panic!("the cost probe preset for {label} parses: {e}"))
}

/// Build a **hardware** headless renderer at the Floor tier, or `None` (a logged
/// skip) when the runner has no adapter or only a software one.
fn hardware() -> Option<Renderer> {
    common::headless_hardware_for(
        WIDTH,
        HEIGHT,
        Some(Tier::Floor),
        common::NEEDS_HARDWARE_FOR_TIMING,
    )
}

/// Per-frame milliseconds for each probe, plus the frame each drew.
///
/// **The cases are interleaved inside one loop rather than measured one series
/// after another**: whichever case ran last would otherwise inherit a GPU that
/// has finished ramping its clocks, which is worth more than the effect being
/// measured. Alternating puts every case on the same drift — ADR-0071's "control
/// taken in the same run" applied to time instead of to pixels.
fn per_frame_ms(renderer: &mut Renderer) -> (Vec<f64>, Vec<CaptureImage>) {
    let presets: Vec<Preset> = (0..CASES.len()).map(probe).collect();
    let names: Vec<String> = presets.iter().map(|p| p.name.clone()).collect();
    renderer.set_presets(presets);
    let frame = AnalysisFrame::default();
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

/// Pixels carrying any light — the non-vacuity handle, and what says the three
/// ring probes really drew different figures rather than the same one thrice.
fn lit(image: &CaptureImage) -> usize {
    image.rgba.chunks_exact(4).filter(|p| p[0] > 8).count()
}

/// **A forty-member arc ring, priced at the floor tier's own resolution.**
///
/// Reports; does not gate. See the module docs for why there is no threshold,
/// for what each probe isolates, and for why a reading on this box can convict
/// the primitive but cannot acquit it.
#[test]
fn a_forty_member_arc_ring_is_priced_against_the_floor_budget() {
    let Some(mut renderer) = hardware() else {
        return;
    };
    let (ms, images) = per_frame_ms(&mut renderer);

    let baseline = ms.first().copied().unwrap_or(f64::NAN);
    println!(
        "\nA {RING_COUNT}-member ring at {WIDTH}x{HEIGHT}, Floor tier, best of {REPEATS} \
         interleaved repeats.\nThe 60 fps floor (docs/nfr.md §1) is {BUDGET_MS:.2} ms a frame.\n"
    );
    println!(
        "{:<15} {:>10} {:>12} {:>12} {:>10}",
        "case", "lit px", "per-frame", "vs rosette", "of budget"
    );
    for (index, (label, _)) in CASES.iter().enumerate() {
        let value = ms.get(index).copied().unwrap_or(f64::NAN);
        let pixels = images.get(index).map(lit).unwrap_or(0);
        if index == 0 {
            println!(
                "{label:<15} {pixels:>10} {value:>9.3} ms {:>12} {:>9.1} %",
                "—",
                value / BUDGET_MS * 100.0
            );
        } else {
            println!(
                "{label:<15} {pixels:>10} {value:>9.3} ms {:>+9.3} ms {:>9.1} %",
                value - baseline,
                value / BUDGET_MS * 100.0
            );
        }
    }
    println!();

    // --- Non-vacuity: the probes must be different figures, or every delta
    // above is a measurement of noise. ---
    let lit_counts: Vec<usize> = images.iter().map(lit).collect();
    let rosette = lit_counts.first().copied().unwrap_or(0);
    assert!(
        rosette > 0,
        "the rosette baseline drew nothing, so every delta is against an empty frame"
    );
    for (index, (label, _)) in CASES.iter().enumerate().skip(1) {
        let pixels = lit_counts.get(index).copied().unwrap_or(0);
        assert!(
            pixels > rosette,
            "{label} lit {pixels} pixels against the bare rosette's {rosette} — the ring \
             is not reaching the frame, so its measured cost is not its cost"
        );
    }
    // The dense case must genuinely be denser, or the fill-rate probe is not
    // probing fill rate.
    let (sparse, dense) = (
        lit_counts.get(1).copied().unwrap_or(0),
        lit_counts.get(2).copied().unwrap_or(0),
    );
    assert!(
        dense > sparse * 2,
        "the motif scale 0.46 ring lit {dense} pixels against 0.13's {sparse}; the probe \
         that exists to overlap bounding boxes is not overlapping them"
    );
}
