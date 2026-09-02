//! **What an element costs, per frame** (Plan 0113 Phase 2, ADR-0123).
//!
//! `shape_collage` draws every element **in every pixel**. Each fragment walks
//! the whole live array, and the axis-aligned bounding box removes the distance
//! evaluation but not the loop step — a wavefront still iterates. ADR-0123 books
//! that as the decision's one real negative and prices the rejection alone at
//! roughly `6N` operations a pixel; this file is where it gets a number, and
//! Plan 0113 Phase 3 is the human gate that reads it.
//!
//! # This is a measurement, and it names its machine
//!
//! Per ADR-0071 a numeric contract states a property or names the configuration
//! it was taken on. A frame time is the second kind — a fact about a GPU, a
//! driver, a build profile and a window size, not about the code — so **there
//! is no threshold here.** The sweep renders the rungs, prints what it saw, and
//! asserts only that it genuinely measured different configurations. Nothing
//! about `main` being green depends on the numbers, which is the point: a
//! threshold on this would be a re-measurement waiting for the next runner
//! image.
//!
//! It also **skips on a software rasterizer, with a notice**. A WARP frame time
//! says nothing about the iGPU floor in `docs/nfr.md` §7, and a reading taken
//! there would be a number that looks like evidence and is not.
//!
//! # What the sweep separates, and what it cannot
//!
//! The rungs vary **only** the live element count, at a fixed 1080p, on a
//! canvas whose elements are the same shapes at the same sizes — so the slope
//! across them is the per-element cost and the intercept is everything else the
//! frame does. Two things it deliberately does not separate, because Phase 3
//! does not need them separated:
//!
//! - **Loop step against distance evaluation.** More elements means both more
//!   iterations and more covered pixels. ADR-0123's escape (Alternative A,
//!   instanced quads) is chosen on *which* of those is the wall, and the honest
//!   way to find out is to compare against that implementation rather than to
//!   infer it from one curve. Phase 3 can commission that.
//! - **Overdraw.** Elements past the authored fourteen are scattered by the
//!   scene's seeded filler, so a high rung has more overlap than a low one.
//!   That is what a dense canvas actually looks like.
//!
//! # The reading
//!
//! **Taken 2026-08-25 on the development box** — Windows 10 19045, DX12,
//! **AMD Radeon(TM) Graphics, an INTEGRATED GPU**, driver 30.0.13002.1001,
//! 1920x1080, floor tier, release, 100 frames of slope, best of three
//! interleaved repeats. Two runs of the shipped rungs, and the pre-Phase-3 sweep
//! that ran to 128 against a provisional cap:
//!
//! | elements | run A | run B | share of 16.67 ms | over 8 elements |
//! |---|---|---|---|---|
//! | 8  | **2.113 ms** | 2.368 ms | 12.7-14.2 % | — |
//! | 16 | **2.794 ms** | 3.209 ms | 16.8-19.3 % | +0.68 / +0.84 ms |
//! | 24 | **3.597 ms** | 3.953 ms | 21.6-23.7 % | +1.48 / +1.59 ms |
//! | 32 | **4.383 ms** | 4.458 ms | 26.3-26.7 % | +2.27 / +2.09 ms |
//! | 40 | **4.824 ms** | 5.137 ms | 28.9-30.8 % | +2.71 / +2.77 ms |
//!
//! **The element cost is linear at roughly 0.09 ms an element** (0.085-0.105
//! across the rungs and the two runs), over an intercept of about **1.4-1.6 ms**
//! that is the frame's own fixed cost at this size — the backdrop pass, the
//! linear-light composite's bandwidth, the tonemap — and *not* this scene's to
//! be charged for.
//!
//! ## Re-measured after the full roster landed, and it got *cheaper*
//!
//! Plan 0113 Phase 7 requires this sweep be re-run once the eight-kind roster is
//! in, because the roster puts a branch in the painter's innermost loop — the
//! hazard `mark_cost.rs` exists for. Same box, same day, probe now at the shipped
//! configuration (`layout = 2`, `roster = 1`, the full vocabulary):
//!
//! | elements | ms/frame | share of 16.67 ms | over 8 elements |
//! |---|---|---|---|
//! | 8  | **1.366 ms** | 8.2 % | — |
//! | 16 | **1.778 ms** | 10.7 % | +0.41 ms |
//! | 24 | **2.353 ms** | 14.1 % | +0.99 ms |
//! | 32 | **2.725 ms** | 16.3 % | +1.36 ms |
//! | 40 | **3.226 ms** | 19.4 % | +1.86 ms |
//!
//! **0.058 ms an element against 0.09, and a full canvas at 19 % of the budget
//! against 29 %.** The branch is not what this loop costs — *coverage* is, the
//! same conclusion `mark_cost.rs` reached about the shaped-mark branch. The
//! eight-kind roster draws rings, sectors and checker patches, which light far
//! less of their own bounding box than a quad does, so the added branch is
//! comfortably repaid by the pixels it does not shade.
//!
//! The two tables are **not** a controlled before/after: Phase 4 replaced the
//! element source between them (a cycling authored roster, then a seeded layout
//! grammar), so the canvases differ in size distribution as well as in kind.
//! What both support is the shape of the answer — linear in element count, with
//! an intercept that belongs to the frame rather than to the scene.
//!
//! ## The sweep's own total load moves its absolute numbers, on this GPU
//!
//! **This is the instrument's one real trap and it is worth stating before the
//! numbers are quoted anywhere.** The pre-Phase-3 sweep ran rungs of 8/16/32/64/
//! 128 and read **6.034 / 6.627 / 7.565 / 10.149 / 14.962 ms** — nearly 4 ms
//! higher at the 8-element rung than the same rung reads above. Nothing about
//! the 8-element case changed. What changed is that the *sweep* went from about
//! two seconds of GPU work to about twenty, and this is a **power-shared
//! integrated GPU**: sustained load drops its clocks, and the light rungs were
//! measured on a throttled part alongside the heavy ones.
//!
//! Interleaving (see [`per_frame_ms`]) is what keeps that from corrupting the
//! *comparison* — every rung rides the same drift, which is why both sweeps
//! agree that the cost is linear — but it cannot keep the **absolute** numbers
//! from moving with how heavy the heaviest rung is. So:
//!
//! - **Quote the shipped-rung table, not the old one.** A canvas at the floor
//!   cap costs about 30 % of the 60 Hz budget, not 90 %. The old sweep priced
//!   its light rungs on a part that was throttled by work no shipped preset can
//!   ask for, since the cap is 40.
//! - **A rung above the cap is not just uninformative, it is misleading**, for
//!   this reason as well as the clamping one.
//!
//! ## The adapter is the integrated one, and that is the useful accident
//!
//! `Renderer::new_headless` asks for no power preference, so on this box it gets
//! the iGPU rather than the discrete GPU — and `docs/nfr.md` §1's floor tier
//! targets a ~2015-iGPU-class machine, so a modern iGPU is much the closer model
//! of it than the RTX 3080 `mark_cost.rs` recorded its table on. Read the
//! numbers as an *optimistic* floor-tier reading rather than as a desktop one:
//! this iGPU is a decade newer than the one the tier is quoted against, and it
//! throttles, as the paragraph above found the hard way.
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

use rlx_core::dsp::AnalysisFrame;
use rlx_core::preset::Preset;
use rlx_core::render::{CaptureImage, Renderer, TierConfig};

/// **1080p, because that is the size `docs/nfr.md` §1 states its budget at.**
/// This scene's cost is per pixel per element, so the render size is half the
/// measurement and a smaller one would understate it proportionally.
const WIDTH: u32 = 1920;
const HEIGHT: u32 = 1080;

/// The rungs, spanning the floor tier's shipped range: the sparse end a preset
/// is actually authored at, up to
/// [`collage_elements`](TierConfig::collage_elements)' own value.
///
/// **The top rung tracks the cap and must stay at or below it.** Past the cap
/// `applied_count` clamps, so two rungs above it would render the same canvas —
/// very stable numbers about nothing, which is exactly what the non-vacuity
/// assertion at the bottom of this file exists to catch. The pre-Phase-3 sweep
/// ran to 128 against a provisional cap; that reading is preserved in the table
/// below because it is the evidence Phase 3 was decided on.
const COUNTS: [usize; 5] = [8, 16, 24, 32, 40];

/// Frame counts either side of the slope. The short run pays the same fixed
/// costs as the long one — preset load, first-frame pipeline warm, the single
/// readback — so the difference divided by the gap is the per-frame cost with
/// all of that subtracted out.
const FRAMES_SHORT: u32 = 10;
const FRAMES_LONG: u32 = 110;

/// How many times each rung is measured. The **minimum** is kept, not the mean:
/// a scheduler hiccup can only add time, so the smallest reading is the one
/// least contaminated by everything that is not the render.
const REPEATS: usize = 3;

/// A `shape_collage` preset differing from its siblings **only** in how many
/// elements are live. Everything else — the canvas, the palette, the paper — is
/// held, so the slope across the rungs is the element count and nothing else.
fn probe(count: usize) -> Preset {
    let toml = format!(
        "system = \"shape_collage\"\nname = \"cost_{count}\"\n\
         [palette]\nstops = [\n\
         {{ at = 0.0, color = \"#111111\" }},\n\
         {{ at = 0.5, color = \"#8a1420\" }},\n\
         {{ at = 1.0, color = \"#d9d5c8\" }},\n\
         ]\n\
         [params]\ncount = \"{count}\"\nscale = \"1.0\"\npaper = \"1.0\"\n\
         layout = \"2\"\nroster = \"1\"\nseed = \"29\"\nsize_hierarchy = \"0.62\"\n"
    );
    Preset::from_toml_str(&toml).expect("the cost probe preset parses")
}

/// Build a **hardware** headless renderer, or `None` (a logged skip) when the
/// runner has no adapter or only a software one.
fn hardware() -> Option<Renderer> {
    common::headless_hardware_for(WIDTH, HEIGHT, None, common::NEEDS_HARDWARE_FOR_TIMING)
}

/// Per-frame milliseconds for each rung, plus the frame each drew.
///
/// **The rungs are interleaved inside one loop rather than measured one series
/// after another**, and that is not tidiness: whichever rung runs last otherwise
/// inherits a GPU that has finished ramping its clocks, which on a five-rung
/// sweep is worth more than the effect being measured. Alternating puts every
/// rung on the same drift — ADR-0071's "control taken in the same run", applied
/// to time instead of to pixels.
fn per_frame_ms(renderer: &mut Renderer) -> (Vec<f64>, Vec<CaptureImage>) {
    let presets: Vec<Preset> = COUNTS.iter().map(|&n| probe(n)).collect();
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

/// **The painter's loop, priced against element count.**
///
/// Reports; does not gate. See the module docs for why there is no threshold,
/// for what the sweep separates, and for where the reading goes.
#[test]
fn a_canvas_is_priced_against_its_element_count() {
    let Some(mut renderer) = hardware() else {
        return;
    };
    let adapter = renderer.adapter_description().to_string();
    let (times, frames) = per_frame_ms(&mut renderer);
    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    // The frame budget NFR §1 commits to at this size, for the share column. Not
    // a threshold — the share is printed so Phase 3 reads a fraction of a budget
    // rather than a bare millisecond count.
    const BUDGET_MS: f64 = 1000.0 / 60.0;

    let mut report = format!(
        "collage cost at {WIDTH}x{HEIGHT}, {} frames of slope, best of {REPEATS}, \
         interleaved (ADR-0071 report)\n  adapter: {adapter}\n  profile: {profile}, \
         floor-tier element cap {}",
        FRAMES_LONG - FRAMES_SHORT,
        TierConfig::FLOOR.collage_elements,
    );
    let base = times.first().copied().unwrap_or(f64::NAN);
    for (index, &count) in COUNTS.iter().enumerate() {
        let ms = times[index];
        let extra = ms - base;
        let per_element = if count > COUNTS[0] {
            extra / (count - COUNTS[0]) as f64
        } else {
            0.0
        };
        report.push_str(&format!(
            "\n  {count:>4} elements   {ms:8.3} ms/frame   {:5.1} % of {BUDGET_MS:.2} ms   \
             ({extra:+.3} ms vs {} elements, {per_element:+.4} ms/element)",
            ms / BUDGET_MS * 100.0,
            COUNTS[0],
        ));
    }
    eprintln!("{report}");

    // Non-vacuity, and the only thing asserted. Every rung really rendered...
    for (index, &count) in COUNTS.iter().enumerate() {
        let ms = times[index];
        assert!(
            ms.is_finite() && ms > 0.0,
            "the {count}-element reading is not a time: {ms}"
        );
    }
    // ...and every rung really drew a *different* canvas. Five readings off one
    // element count would be very stable numbers about nothing — which is
    // exactly what a `count` that stopped reaching the scene would produce.
    let total = (WIDTH * HEIGHT) as usize;
    for (index, &count) in COUNTS.iter().enumerate().skip(1) {
        let differing = frames[index - 1]
            .rgba
            .chunks_exact(4)
            .zip(frames[index].rgba.chunks_exact(4))
            .filter(|(a, b)| a[..3] != b[..3])
            .count();
        eprintln!(
            "  {count} elements differs from {} in {differing} of {total} pixels",
            COUNTS[index - 1]
        );
        assert!(
            differing * 1000 > total,
            "the {count}-element canvas drew the same picture as the {}-element one, so this \
             measured one configuration twice: {differing} of {total} pixels differ",
            COUNTS[index - 1]
        );
    }
}
