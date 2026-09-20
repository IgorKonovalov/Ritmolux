//! **The wash bisect** — Plan 0111 Phase 2.
//!
//! One statistic, measured at every seam of the chain, for a washed conversion
//! and a clean control. The question is not "how bright is it" but **where along
//! the chain does the washed/control ratio depart from what it was at the
//! field**, because that names the stage that does it.
//!
//! # The statistic
//!
//! [`FieldTrace::edge`](super::scenes::warp_mesh)'s quantity: the mean over the
//! outermost ring of texels. That is the **background**, where a centred figure
//! puts nothing. One statistic at every seam is the whole design — the numbers
//! are then comparable across stages, and each comparison is linear light against
//! linear light (ADR-0074's same-kind requirement).
//!
//! **It reads the background only for a figure that does not fill the frame.**
//! *Fog Tunnel* qualifies **in the reference**, which draws a skeleton of
//! discrete concentric rings whose gaps *are* the background — but our conversion
//! draws the solid tube that is the defect, so on this render the outer ring may
//! be sampling the figure. That softens the absolute numbers and not the trend:
//! the trend is one statistic at every seam of one run, so whatever the ring
//! samples, it samples the same thing at each seam. Pointed at a preset that
//! fills the frame in *both* renderers this measures the figure and says nothing
//! about a wash.
//!
//! # The seams, and why there are three rather than five
//!
//! There are five in the general case: the field (A), the present pass
//! (B), the backdrop + `layer_blend` composite (C), bloom (D) and the
//! tonemap (E). **For these two subjects C and D do not exist**, and
//! that is a measurement rather than an assumption:
//!
//! - every post stage reports [`active`](super::post::PostStage::active) only
//!   when its own param exceeds zero, and neither converted fixture binds
//!   `bloom`, `trails` or any kaleidoscope param — their whole `[params]` table
//!   is `brightness`, `warp_scale`, `warp_speed`;
//! - with no stage active `PostChain::begin` returns the caller's `destination`
//!   itself, so the scene draws **directly** into the texture the tonemap reads;
//! - `bg_bright` defaults to `0.0` and neither fixture binds it, so the backdrop
//!   pre-pass contributes nothing.
//!
//! So B, C and D are one texture, and the chain this probe bisects is `field ->
//! present pass -> tonemap -> display`. Two stages. The probe reports the
//! collapsed seam once, named for what it is, rather than printing one read
//! three times as though it were three.
//!
//! # What a departure means
//!
//! Seam E is display-referred and every other seam is linear, so a seam-to-seam
//! comparison of *levels* would be meaningless. The comparison is between the two
//! **subjects** at one seam — same units on both sides — and then between those
//! dimensionless ratios across seams, which is what [`SeamTrace`] carries.
//!
//! # The reading is a settled band, not a frame
//!
//! The defect being measured is an **equilibrium**: the field converges to a
//! level over many frames, and a single frame is exactly what makes a seam look
//! clean. So every seam is read at each of [`CHECKPOINTS`] — independent runs
//! from frame zero, since `capture_preset` rebuilds the scenes and resets the
//! clock — and the reported level is the mean over the last [`BAND`] of them.
//!
//! **It is a band and not a point, and that is a property of the subject rather
//! than of the instrument.** A converged field still tracks whatever its preset's
//! per-frame program is doing, and a program driving the warp from slow sine
//! terms never stops moving it. Reading one frame would report a phase of that
//! wobble as though it were the level. What separates the two is that the
//! transient into the band is the larger motion by an order of magnitude, which
//! is what the first checkpoint is for and what [`SETTLED_SPREAD`] and the
//! transient assertion hold the instrument to.
//!
//! # The control reads zero, and what that buys and costs
//!
//! *Blur Mix 3*'s background sits at exactly `0.0` at every seam. Its warp shader
//! subtracts a constant `0.02` from every texel each frame and divides by `1.1`,
//! and its fixture binds the scene's own `deposit` to `0.0` — so nothing sustains
//! a background and the subtraction floors it. The washed/control **ratio** is
//! therefore undefined, and the bisect's departure test is unavailable in that
//! form.
//!
//! What a zero control gives instead is stronger against one class of stage and
//! blind to the other:
//!
//! - **An additive stage is ruled out at every seam of the chain.** Fed a field
//!   that is black at the frame edge, the present pass and the tonemap return
//!   black at the frame edge. No stage downstream of the field contributes a
//!   background floor of its own, for any subject — that is a statement about the
//!   chain, where a ratio is a statement about two subjects in it.
//! - **A multiplicative stage is invisible to it**, since a gain maps `0` to `0`.
//!   That class is bounded from the washed subject's own seam-to-seam gain, which
//!   the probe prints beside the levels: `B/A` is the present pass's gain on a
//!   real background, in linear light on both sides.
//!
//! A second control preset would restore the ratio, and it is not taken: the two
//! subjects are the pair four look gates were judged on, and a third fixture
//! bought for one column would not be one of them.

use super::{HeadlessOptions, RenderError, Renderer, capture, scene_for};
use crate::dsp::AnalysisFrame;
use crate::preset::Preset;

/// *Geiss - Fog Tunnel* — still washed at Plan 0109's Phase 5 gate, and the one
/// whose defect is legible: the reference draws discrete concentric rings where
/// this draws a solid tube.
const FOG_TUNNEL: &str = include_str!("../../tests/fixtures/milk_wash_fog_tunnel.toml");

/// *Geiss - Blur Mix 3* — the clean control. Its blur chain actively darkens, and
/// it was the one pair Plan 0100 Phase 7 judged genuinely good.
const BLUR_MIX_3: &str = include_str!("../../tests/fixtures/milk_wash_blur_mix_3.toml");

/// Square, so the outer ring is the same thickness on both axes and `edge` is not
/// weighted toward one of them.
const SIZE: u32 = 128;

/// The frame counts every seam is read at, ascending. The first is the
/// **transient probe** — one time constant of the field is roughly 100 frames, so
/// at 30 it is still filling from black — and the remaining [`BAND`] are the
/// **settled band**, spanning two more time constants.
const CHECKPOINTS: [u32; 4] = [30, 100, 200, 300];

/// How many of [`CHECKPOINTS`], counting from the end, form the settled band. The
/// settled level is that band's mean, and its spread is the honest uncertainty on
/// it: a converged field still rides whatever its preset's per-frame program is
/// doing, and nothing here averages that out.
const BAND: usize = 3;

/// How thick the ring `edge` averages over. Two texels, matching the scene-level
/// probe this statistic is borrowed from.
const RING: u32 = 2;

/// Mean of the three colour channels over the outermost `RING` texels of an
/// `RGBA f32` buffer in row-major order.
fn edge(rgba: &[f32], width: u32, height: u32) -> f32 {
    let (mut sum, mut n) = (0f64, 0u64);
    for y in 0..height {
        for x in 0..width {
            let on_ring = x < RING || y < RING || x + RING >= width || y + RING >= height;
            if !on_ring {
                continue;
            }
            let base = ((y * width + x) * 4) as usize;
            for c in 0..3 {
                if let Some(v) = rgba.get(base + c) {
                    sum += f64::from(*v);
                    n += 1;
                }
            }
        }
    }
    if n == 0 { 0.0 } else { (sum / n as f64) as f32 }
}

/// One subject's background level at each seam of the chain, in the units that
/// seam carries.
#[derive(Debug, Clone, Copy)]
struct SeamTrace {
    /// After warp/deposit/draw, before the present pass. Linear.
    a_field: f32,
    /// After the present pass — the echo mix, `x brightness`, `x gamma`, the four
    /// composite remaps and `x occlude`. Linear. **Also seams C and D**, which do
    /// not exist for these subjects (module docs).
    b_present: f32,
    /// After the tonemap's shoulder. Display-referred, so comparable only against
    /// the other subject's `e_display`.
    e_display: f32,
}

/// Picks one seam's level out of a [`SeamTrace`].
type Pick = fn(&SeamTrace) -> f32;

/// The seams in chain order, each with the reader that picks it out of a
/// [`SeamTrace`], so the report loops over seams rather than repeating one block
/// of formatting per field.
const SEAMS: [(&str, Pick); 3] = [
    ("A field", |t| t.a_field),
    ("B present*", |t| t.b_present),
    ("E display", |t| t.e_display),
];

/// Render `source` to each of [`CHECKPOINTS`] and read the background at every
/// seam of each run, in checkpoint order.
///
/// One renderer serves all three because `capture_preset` rebuilds the scenes and
/// resets the clock: each checkpoint is an independent run from frame zero and a
/// pure function of (fixture, frame count), not a continuation of the one before
/// it.
fn seam_traces(source: &str) -> Option<Vec<SeamTrace>> {
    let preset = Preset::from_toml_str(source).expect("the fixture parses");
    let name = preset.name.clone();

    let mut renderer = match Renderer::new_headless(HeadlessOptions {
        width: SIZE,
        height: SIZE,
        prefer_software: false,
    }) {
        Ok(r) => r,
        Err(RenderError::RequestAdapter(_)) => {
            eprintln!("skipped: no GPU adapter on this runner (ADR-0016)");
            return None;
        }
        Err(e) => panic!("headless renderer build failed: {e}"),
    };
    renderer.set_presets(vec![preset]);

    let mut traces = Vec::with_capacity(CHECKPOINTS.len());
    for frames in CHECKPOINTS {
        // Seam E, and the thing that drives the run. Everything read below
        // belongs to this run's LAST frame.
        let display = renderer
            .capture_preset(&name, &AnalysisFrame::default(), frames)
            .expect("the capture succeeds");
        let e_display = {
            let linear: Vec<f32> = display.rgba.iter().map(|b| f32::from(*b) / 255.0).collect();
            edge(&linear, display.width, display.height)
        };

        traces.push(SeamTrace {
            a_field: read_linear(&renderer, Seam::Field)?,
            b_present: read_linear(&renderer, Seam::Present)?,
            e_display,
        });
    }
    Some(traces)
}

enum Seam {
    Field,
    Present,
}

/// Copy one of the run's linear intermediates back and reduce it to `edge`.
///
/// Both textures are `COMPOSITE_FORMAT` and both already carried `COPY_SRC`
/// before this probe existed — the field for Plan 0109 Phase 4, the tonemap's
/// input for its own CPU-mirror test. Nothing here adds a render target, which is
/// what keeps this probe clear of the adapter hazard ADR-0058 records.
fn read_linear(renderer: &Renderer, seam: Seam) -> Option<f32> {
    let texture = match seam {
        Seam::Field => {
            let system = renderer.roster.active_preset()?.system;
            scene_for(&renderer.scenes, system)?.feedback_field()?
        }
        Seam::Present => renderer.tonemap.src_texture()?,
    };
    let device = &renderer.ctx.device;
    let (width, height) = (texture.width(), texture.height());
    let (buffer, padded_bpr) = capture::create_linear_readback(device, width, height);
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("milk-wash-seam"),
    });
    capture::record_copy(&mut encoder, texture, &buffer, padded_bpr, width, height);
    renderer.ctx.queue.submit(std::iter::once(encoder.finish()));
    let rgba = capture::read_back_linear(device, &buffer, width, height, padded_bpr)
        .expect("the seam reads back");
    Some(edge(&rgba, width, height))
}

/// **The wash bisect.** Reports the settled background at every seam for a washed
/// conversion and the clean control, the band that level is settled within, and
/// the present pass's gain on each subject.
///
/// This test **asserts no threshold on the ratio or on any level** (ADR-0071).
/// What separation is large enough to call a departure is exactly what the
/// measurement is for, and a number chosen before it would be tuning to the
/// instrument. What it does assert is that the instrument works: both subjects
/// render, every seam reads back at every checkpoint, and the reported level is
/// an equilibrium — a band narrower than the transient that reached it — rather
/// than a point on a climb.
///
/// # What it measured, 2026-09-18
///
/// Dev box, 128x128, `AnalysisFrame::default()`, quantizer at its
/// `DEFAULT_QUANTIZE_STEPS = 255` (neither fixture overrides it). Levels are the
/// `edge` mean at each of [`CHECKPOINTS`]; `settled` is the mean over the last
/// [`BAND`] of them and `spread` its half-spread as a fraction of that mean.
///
/// ```text
///   subject      seam               f30          f100          f200          f300       settled  spread
///   fog tunnel   A field     0.07266875    0.08851405    0.08849836    0.08170217    0.08623820   3.95%
///   fog tunnel   B present*  0.14117473    0.16968489    0.16830856    0.15579858    0.16459735   4.22%
///   fog tunnel   E display   0.29694083    0.33533174    0.33402455    0.31976217    0.32970616   2.36%
///   blur mix 3   A field     0.00000000    0.00000000    0.00000000    0.00000000    0.00000000   0.00%
///   blur mix 3   B present*  0.00000000    0.00000000    0.00000000    0.00000000    0.00000000   0.00%
///   blur mix 3   E display   0.00000000    0.00000000    0.00000000    0.00000000    0.00000000   0.00%
/// ```
///
/// **The washed subject reaches a band, and it is a band rather than a point.**
/// The transient from black is `0.073 -> 0.086` at the field, done by frame 100;
/// what remains after it is a `+-4.0 %` wobble that is not monotone — `f100` is
/// the highest of the three, by a hair over `f200` — and *Fog Tunnel*'s own
/// per-frame program is where it comes from, driving `cx`, `cy`, `warp` and `rot`
/// from four sine terms whose slowest period is longer than the whole run. So
/// there is no frame at which the level stops moving, and the settled quantity is
/// the band's mean.
///
/// **The control is at the floor at every seam and every checkpoint**, including
/// the transient one: *Blur Mix 3* never has a background to settle. The washed
/// subject's present-pass gain is `B/A = 1.909` — linear against linear, so a
/// real gain and not a domain artifact — and seam E is display-referred, so it is
/// comparable only against another subject's E.
///
/// Three things the numbers do not say, all worth carrying:
///
/// - **`edge` may not be reading background on this render of *Fog Tunnel*.** The
///   statistic reads background only for a figure that does not fill the frame,
///   and the reference's discrete rings are exactly that — but *our* conversion
///   draws the solid tube that is the defect, so at the frame edge this may be
///   reading the figure. What survives either way is the seam-to-seam trend,
///   because it is one statistic at every seam of one run; the absolute level
///   does not.
/// - **The washed/control ratio is gone, not small.** A control at exactly zero
///   divides into nothing. See the module docs for what replaces it and what that
///   cannot see.
/// - **Two dated boundaries sit behind this table, and a reading from before
///   either is a different measurement.** Before 2026-09-16 both fixtures carried
///   the scene's `DEFAULT_DEPOSIT`, so their field had a source term these now
///   bind to `0.0`. And before the converted path took the reference's own decay
///   — encoded domain, factor truncated to 8 bits, `warp_mesh`'s `milk_decay` and
///   `rlx_milk_decay` — the same instrument read `0.13559258 / 0.25316700 /
///   0.40450469` with a gain of `1.867`, which is the `1.572x` / `1.538x` /
///   `1.227x` fall that repair bought. Numbers from either side of either
///   boundary are not comparable, and the older ones are not evidence about this
///   tree.
#[test]
fn the_wash_bisect_reports_every_seam() {
    let Some(fog) = seam_traces(FOG_TUNNEL) else {
        return;
    };
    let blur = seam_traces(BLUR_MIX_3).expect("the second subject runs too");
    let subjects = [("fog tunnel", &fog), ("blur mix 3", &blur)];

    let head: String = CHECKPOINTS
        .iter()
        .map(|n| format!("  {:>12}", format!("f{n}")))
        .collect();
    println!("[wash] subject      seam      {head}       settled  spread");
    for (subject, traces) in subjects {
        for (name, pick) in SEAMS {
            let at = column(traces, pick);
            let cells: String = at.iter().map(|v| format!("  {v:>12.8}")).collect();
            let s = Settled::of(&at);
            println!(
                "[wash] {subject}   {name:<10}{cells}  {:>12.8}  {:>6.2}%",
                s.level,
                s.spread * 100.0
            );
        }
    }
    let gain = |traces: &[SeamTrace]| {
        let level = |pick: Pick| Settled::of(&column(traces, pick)).level;
        let (a, b) = (level(|t| t.a_field), level(|t| t.b_present));
        if a > 0.0 { b / a } else { f32::NAN }
    };
    println!(
        "[wash] present-pass gain B/A on the settled level: fog tunnel {:.3}, blur mix 3 {:.3}",
        gain(&fog),
        gain(&blur)
    );
    println!(
        "[wash] * B is also seams C and D: no post stage is active and the backdrop is unbound"
    );

    for (subject, traces) in subjects {
        for (name, pick) in SEAMS {
            let at = column(traces, pick);
            assert!(
                at.iter().all(|v| v.is_finite()),
                "{subject} must reach {name} at every checkpoint: {at:?}"
            );
            let s = Settled::of(&at);
            // A floor is settled by inspection, and a relative spread on zero is
            // not a quantity — so the band test applies to a level there is one for.
            if s.level <= 0.0 {
                continue;
            }
            assert!(
                s.spread < SETTLED_SPREAD,
                "{subject} at {name} must hold a band across the last {BAND} checkpoints: \
                 it spread {:.4} about {:.8}, from {at:?}",
                s.spread,
                s.level
            );
            // The transient has to be the larger motion, or "settled" is just a
            // slow climb measured over too short a run.
            let start = at.first().copied().unwrap_or_default();
            let transient = (s.level - start).abs();
            assert!(
                transient > s.spread * s.level,
                "{subject} at {name} must have settled from a transient larger than its \
                 own band: it moved {transient:.8} from the first checkpoint's {start:.8} \
                 to the band's {:.8}, whose half-spread is {:.8}",
                s.level,
                s.spread * s.level
            );
        }
    }
}

/// One seam's level at every checkpoint, in checkpoint order.
fn column(traces: &[SeamTrace], pick: Pick) -> Vec<f32> {
    traces.iter().map(pick).collect()
}

/// A settled reading: the mean over the last [`BAND`] checkpoints and the
/// half-spread about it, as a fraction of the mean.
///
/// **The spread is not noise.** This instrument is deterministic — the same
/// fixture at the same frame count reads bit-identically run to run. What the
/// band covers is that a converged field still tracks its preset's own per-frame
/// program, and the band is shorter than the slowest term in either fixture's,
/// so the residual is a phase of that program rather than anything averaging out.
struct Settled {
    level: f32,
    spread: f32,
}

impl Settled {
    fn of(at: &[f32]) -> Self {
        Self::over(&at.iter().rev().take(BAND).copied().collect::<Vec<f32>>())
    }

    /// Mean and half-spread over **every** sample handed in, where
    /// [`Settled::of`] first narrows a checkpoint column to its last [`BAND`].
    /// Same statistic, different window: the rate probe's tail is already the
    /// window it wants averaged.
    fn over(at: &[f32]) -> Self {
        let level = at.iter().sum::<f32>() / at.len().max(1) as f32;
        let lo = at.iter().copied().fold(f32::INFINITY, f32::min);
        let hi = at.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        Self {
            level,
            spread: if level > 0.0 {
                (hi - lo) / (2.0 * level)
            } else {
                0.0
            },
        }
    }
}

/// How wide the settled band may be, as a fraction of its own mean, and still be
/// called a band rather than a trend. Generous against the measurement, because
/// what the assertion protects is the instrument's claim to report an equilibrium
/// at all — not where the equilibrium is, which is what the phase measures
/// (ADR-0071).
const SETTLED_SPREAD: f32 = 0.20;

// ---------------------------------------------------------------------------
// The rate probe
// ---------------------------------------------------------------------------

/// **The rate probe** — Plan 0202 Phase 1.
///
/// The same subject as the bisect above, driven at three frame rates over the
/// **same wall clock**, so the only thing that differs between the runs is how
/// many frames the engine cut that second into. If a deposit on this path were
/// per frame and unconverted, the field's equilibrium would rise with the rate
/// as `1/(1 - d)` does — the shape both remaining washed pairs have.
///
/// # Why this is the probe and not a `shot --render` pipeline
///
/// `shot --render <clip> --fps N` drives exactly
/// [`Renderer::capture_stream`](super::Renderer::capture_stream) at `1/N`, and
/// nothing between that call and the Y4M writer touches the picture — the
/// encoder path is a colour-space conversion of frames already rendered. So
/// calling `capture_stream` directly at each rate is the same measurement
/// without a WAV, an encoder or a video file, and it is deterministic where a
/// clip is not: the [`AnalysisFrame`] is held constant for the whole run, so the
/// *only* difference between the three columns is `dt`.
///
/// Holding the analysis frame also removes the one confound a clip would add.
/// The audio hop clock and the frame clock are different clocks, so at 30 fps a
/// frame spans several hops and at 165 several frames share one — the light the
/// draw layer lays down would then differ between the runs for a reason that has
/// nothing to do with the rate conversion under test.
///
/// # A ladder, not the two endpoints
///
/// The candidate predicts a level **proportional to the rate**, so the two rates
/// it is stated at would settle it — but they would settle it only as a pair of
/// numbers, and a pair cannot tell a rate law from a wobble that happens to
/// straddle them. The ladder spans 11x and brackets both endpoints, which is what
/// makes "not monotone in the rate" a statement the reading can support.
const RATES: [f32; 7] = [15.0, 30.0, 45.0, 60.0, 90.0, 120.0, 165.0];

/// The rate every other row is reported against: MilkDrop's own nominal cadence
/// ([`NOMINAL_FPS`](crate::milk::NOMINAL_FPS)), which is the cadence a `.milk`
/// author tuned by eye against and the lower of the two rates Plan 0142's log
/// names.
const BASE_RATE: f32 = crate::milk::NOMINAL_FPS;

/// Wall-clock seconds each rate is run for. *Fog Tunnel*'s field settles within
/// about a second and a half at any of [`RATES`] (the bisect's transient is done
/// by its `f100` checkpoint), so eight seconds is several time constants and the
/// tail below is well past the climb.
const RUN_SECONDS: f32 = 8.0;

/// How much of the tail the reported level averages over. A band rather than a
/// frame, for the reason the bisect's [`BAND`] is one — and **seconds** rather
/// than frames, because the three runs must average the same stretch of the
/// preset's own per-frame program or they are not comparable.
const TAIL_SECONDS: f32 = 2.0;

/// What one rate's run read: the display ground over the tail, and the field's
/// own level at the last frame.
struct RateReading {
    fps: f32,
    frames: u32,
    /// Display-referred `edge`, meaned over the last [`TAIL_SECONDS`].
    display: Settled,
    /// Linear `edge` at seam A after the run's last frame — the field itself,
    /// which is where an unconverted deposit would show.
    field: f32,
}

/// Drive `source` for [`RUN_SECONDS`] at `fps` and read the ground level.
fn ground_at_rate(renderer: &mut Renderer, name: &str, fps: f32) -> Option<RateReading> {
    let dt = 1.0 / fps;
    let frames = (RUN_SECONDS * fps).round().max(1.0) as u32;
    let tail_from = frames.saturating_sub((TAIL_SECONDS * fps).round().max(1.0) as u32);

    let mut tail: Vec<f32> = Vec::new();
    renderer
        .capture_stream(
            name,
            frames,
            dt,
            &mut |_| AnalysisFrame::default(),
            &mut |index, img| {
                if index >= tail_from {
                    let linear: Vec<f32> = img.rgba.iter().map(|b| f32::from(*b) / 255.0).collect();
                    tail.push(edge(&linear, img.width, img.height));
                }
                Ok(())
            },
        )
        .expect("the stream capture succeeds");

    Some(RateReading {
        fps,
        frames,
        display: Settled::over(&tail),
        field: read_linear(renderer, Seam::Field)?,
    })
}

/// **Does the converted field's ground level depend on the frame rate?**
///
/// Plan 0142's log names a rate mismatch as the candidate behind the two pairs
/// that are still washed at the ground, and names this probe as the way to
/// settle it. It **asserts no threshold on the levels** (ADR-0071) — what the
/// levels are is the measurement, and the comparison between them is recorded in
/// Plan 0202's implementation log. What it does assert is that the instrument
/// works: every rate renders, every reading is finite, and each run's tail is a
/// settled band rather than a point on a climb.
///
/// # What it measured
///
/// The reading is **printed by the test rather than frozen here**: the question
/// is a ratio between the rows, and a frozen level would be a golden nobody
/// could move. What the run prints is the linear `edge` at seam A after the last
/// frame, the display-referred `edge` meaned over the last [`TAIL_SECONDS`] with
/// its half-spread, and each row against [`BASE_RATE`] beside the rate ratio
/// itself — so a level that tracked the rate would read as two matching columns.
#[test]
fn the_converted_ground_level_is_read_across_a_frame_rate_ladder() {
    let preset = Preset::from_toml_str(FOG_TUNNEL).expect("the fixture parses");
    let name = preset.name.clone();

    let mut renderer = match Renderer::new_headless(HeadlessOptions {
        width: SIZE,
        height: SIZE,
        prefer_software: false,
    }) {
        Ok(r) => r,
        Err(RenderError::RequestAdapter(_)) => {
            eprintln!("skipped: no GPU adapter on this runner (ADR-0016)");
            return;
        }
        Err(e) => panic!("headless renderer build failed: {e}"),
    };
    renderer.set_presets(vec![preset]);

    let mut readings = Vec::with_capacity(RATES.len());
    for fps in RATES {
        let Some(reading) = ground_at_rate(&mut renderer, &name, fps) else {
            panic!("the field seam must read back at {fps} fps");
        };
        readings.push(reading);
    }

    println!(
        "[rate] fog tunnel, {RUN_SECONDS}s of wall clock, {SIZE}x{SIZE}, tail {TAIL_SECONDS}s"
    );
    println!("[rate]    fps  frames       A field     E display  spread");
    for r in &readings {
        println!(
            "[rate] {:>6.0}  {:>6}  {:>12.8}  {:>12.8}  {:>5.2}%",
            r.fps,
            r.frames,
            r.field,
            r.display.level,
            r.display.spread * 100.0
        );
    }
    // The comparison the phase is about, printed as the dimensionless quantity
    // it is: every rate against [`BASE_RATE`], beside the rate ratio itself,
    // which is what the candidate predicts the level ratio would track.
    if let Some(base) = readings.iter().find(|r| r.fps == BASE_RATE) {
        println!("[rate]    fps   rate x    field x  display x");
        for r in &readings {
            println!(
                "[rate] {:>6.0}  {:>7.4}  {:>9.4}  {:>9.4}",
                r.fps,
                r.fps / base.fps,
                ratio(r.field, base.field),
                ratio(r.display.level, base.display.level),
            );
        }
    }

    for r in &readings {
        assert!(
            r.field.is_finite() && r.display.level.is_finite(),
            "every seam must read a number at {} fps: field {}, display {}",
            r.fps,
            r.field,
            r.display.level
        );
        assert!(
            r.display.level > 0.0,
            "the washed subject must reach a ground level at {} fps, got {}",
            r.fps,
            r.display.level
        );
        assert!(
            r.display.spread < SETTLED_SPREAD,
            "the tail at {} fps must be a settled band, not a climb: it spread {:.4} \
             about {:.8}",
            r.fps,
            r.display.spread,
            r.display.level
        );
    }
}

/// `a / b`, or `NaN` when there is no ratio to take — the same guard the bisect's
/// present-pass gain uses.
fn ratio(a: f32, b: f32) -> f32 {
    if b > 0.0 { a / b } else { f32::NAN }
}
