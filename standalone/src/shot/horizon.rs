//! The `shot --horizon` long-run mode: N **simulated** minutes at capture
//! cadence, one drift row per interval (Plan 0085 Phase 1, ADR-0099).
//!
//! **What this instrument is for.** Everything else this repo measures lives in
//! the first seconds of a preset's life — the four behavioral gates capture 30
//! frames, half a second — while the live-show use case runs for hours. A world
//! whose mechanism accumulates (sustained forces, feedback with net gain, a
//! population that can migrate or pile up) can drift over *minutes* with the
//! whole suite green, which is exactly what happened to Plan 0075 cohort 4's
//! Shatter: three live collapses, nothing red.
//!
//! **What it is not.** It is never a gate (ADR-0099 rejects that on measured
//! cost), it asserts no threshold, and it prints a **trend** rather than a
//! verdict. The verdict is the reader's, and it belongs in the world's own
//! header. The statistics are image-domain proxies for a simulation-domain
//! event — particles piling onto attractors *reads* as coverage falling and
//! concentration rising, and that correlation is strong without being identity.
//! Running a **static control world** beside the subject is what keeps the proxy
//! honest: a flat row series there is what makes a sloped one here mean
//! something.
//!
//! **What it cannot see.** GPU resource churn and the frame-time spike on a
//! preset switch. This is a headless render loop that never rebuilds a surface,
//! so neither is reproducible here by construction; `--soak` is the instrument
//! for that half (ADR-0099).
//!
//! **The resident set is the exception**, and Plan 0099 is why this paragraph no
//! longer lists it among them. A horizon cannot reproduce a *switching* app's
//! memory behaviour, but the growth of its own render loop is precisely what a
//! run of tens of thousands of frames is in a position to see — and reading the
//! cost block's RSS column is what found a capture path retaining 950 KB a
//! frame.
//!
//! Deterministic on the same terms as every other capture: injected `dt`, seeded
//! randomness, scenes rebuilt to their seed. The same world at the same interval
//! produces the same rows on every machine, which is what makes a recorded
//! header verdict worth anything. The wall clock and resident set it prints are
//! the exception and are labelled as such — those are properties of the box.

use std::fmt::Write as _;
use std::time::Instant;

use rlx_core::dsp::AnalysisFrame;
use rlx_core::preset::Preset;
use rlx_core::render::metrics::{coverage, footprint_diff, peak_to_mean};
use rlx_core::render::{CaptureImage, Tier};

use crate::shot::json::{json_string, num};

/// The capture path's fixed step — `core`'s `FALLBACK_DT`, one 60 Hz frame.
/// Every headless entry point advances by exactly this, so a horizon's
/// *simulated* seconds are frames over this rate regardless of how long the run
/// takes in wall-clock terms.
pub const CAPTURE_HZ: f32 = 60.0;

/// Longest horizon accepted, in simulated minutes. Ten hours is far past any
/// spot-check and well short of what would overflow the frame counter; the point
/// of the bound is that a mistyped `--horizon 6000` fails immediately instead of
/// wedging a session for a week.
pub const MAX_MINUTES: f32 = 600.0;

/// "Lit" tolerance for [`coverage`] and [`peak_to_mean`], matching `--report`'s
/// so the two instruments call the same pixels lit.
const COVERAGE_EPS: u8 = 10;

/// Lower bound on the footprint reading's denominator, as a fraction of the
/// frame — the same guard at the same value `--report` and the animation gate
/// use (ADR-0091), where its two-sided derivation lives. Without it a
/// nearly-empty frame's one-pixel flicker reads as strong motion, which over a
/// long run would print as drift.
const FOOTPRINT_MIN_FRAC: f32 = 0.015;

/// What the CLI asked for. The example owns the flags; this owns the shape, so
/// the library never depends on how a command line is spelled.
pub struct HorizonRequest {
    /// Preset to run, or `None` when the roster names itself (one entry).
    pub preset: Option<String>,
    /// Held stimulus, exactly as `--set` builds it for every other capture.
    pub stimulus: AnalysisFrame,
    /// Simulated minutes to render.
    pub minutes: f32,
    /// Simulated seconds between rows.
    pub interval_secs: f32,
    pub width: u32,
    pub height: u32,
    pub tier: Tier,
    pub json: bool,
}

/// One interval of a horizon run — the row shape.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HorizonSample {
    /// Frame index within the run (frame 0 is the first rendered frame).
    pub frame: u32,
    /// **Simulated** time, from the injected `dt` — not wall clock.
    pub elapsed_secs: f32,
    /// Lit fraction of the frame (ADR-0067 semantics).
    pub coverage: f32,
    /// Motion over the figure's own footprint since the **previous** row
    /// (ADR-0091). `None` on the first row, which has no predecessor — printed
    /// as `-` rather than as a zero that would read as a stalled world.
    pub footprint_diff: Option<f32>,
    /// Concentration: has the population piled onto a few places?
    pub peak_to_mean: f32,
}

/// The shape of one statistic's series across the run.
///
/// Deliberately four plain numbers and no verdict. `delta` says how far the
/// statistic travelled end to end and `monotone` says how much of that travel
/// went one way — a world grinding steadily into a corner reads a large `delta`
/// at a `monotone` near 1.0, while a world breathing around a stable mean reads
/// a `delta` near zero whatever its `monotone`. Where the line falls between
/// "drifting" and "alive" is a judgement about the look, so the tool declines to
/// make it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Trend {
    pub first: f32,
    pub last: f32,
    /// `last - first`.
    pub delta: f32,
    /// Share of the consecutive steps that moved in `delta`'s direction, in
    /// `0.0..=1.0`. Zero for a series that ended where it started, and for a
    /// series too short to have a step.
    pub monotone: f32,
}

/// What the run cost on **this** box. Reported, never asserted (ADR-0071):
/// every other number here is a property of the simulation and travels, these
/// two are properties of the machine and do not.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RunCost {
    /// Frames rendered, sampled and unsampled alike.
    pub frames: u32,
    pub wall_secs: f32,
    /// Resident set before and after the run. **Not a sampled peak** — the run
    /// is a single call, so there is nowhere to sample from. It is a usable
    /// stand-in because the only thing that grows across a horizon is the vector
    /// of sampled images, one per row, which is never freed until the end.
    pub rss_before_bytes: Option<u64>,
    pub rss_after_bytes: Option<u64>,
    /// Whether the adapter was a software rasterizer, which is a several-fold
    /// difference in what the wall clock above means.
    pub software_adapter: bool,
}

/// A finished run: everything the presentations need, and nothing that needs a
/// GPU to produce. Splitting it out this way is what makes the table and the
/// JSON directly testable.
pub struct HorizonRun {
    pub preset: String,
    pub tier: Tier,
    pub width: u32,
    pub height: u32,
    pub minutes: f32,
    pub interval_secs: f32,
    pub samples: Vec<HorizonSample>,
    pub cost: RunCost,
}

// ---------------------------------------------------------------------------
// Pure arithmetic
// ---------------------------------------------------------------------------

/// Frame indices to sample, given a horizon in simulated minutes and an interval
/// in simulated seconds.
///
/// Row 0 sits at frame 0 and is the **reference** the first interval's motion is
/// measured against, so a request for *k* intervals returns *k + 1* indices.
/// They are exact multiples of the interval's frame count, which is the property
/// that makes row *k* independent of the horizon requested: a ten-minute run and
/// a two-minute run agree on every index they share, so their rows are
/// comparable.
pub fn sample_frames(minutes: f32, interval_secs: f32) -> Result<Vec<u32>, String> {
    if !minutes.is_finite() || minutes <= 0.0 {
        return Err(format!(
            "--horizon expects positive minutes, got `{minutes}`"
        ));
    }
    if minutes > MAX_MINUTES {
        return Err(format!(
            "--horizon {minutes} is past the {MAX_MINUTES}-minute ceiling \
             ({:.0} renders); a spot-check is minutes, not days",
            minutes * 60.0 * CAPTURE_HZ
        ));
    }
    if !interval_secs.is_finite() || interval_secs <= 0.0 {
        return Err(format!(
            "--interval expects positive seconds, got `{interval_secs}`"
        ));
    }
    let step = (interval_secs * CAPTURE_HZ).round();
    if step < 1.0 {
        return Err(format!(
            "--interval {interval_secs}s is under one frame at {CAPTURE_HZ} Hz"
        ));
    }
    let intervals = (minutes * 60.0 / interval_secs).floor();
    if intervals < 1.0 {
        return Err(format!(
            "--interval {interval_secs}s is longer than the {minutes}-minute \
             horizon, so there is nothing to sample"
        ));
    }
    let step = step as u32;
    let intervals = intervals as u32;
    Ok((0..=intervals).map(|k| k * step).collect())
}

/// Simulated seconds at a frame index. The clock advances *before* each frame is
/// drawn, so frame 0 sits one step in rather than at zero.
pub fn elapsed_secs(frame: u32) -> f32 {
    (frame + 1) as f32 / CAPTURE_HZ
}

/// The shape of a statistic's series — see [`Trend`].
pub fn trend(values: &[f32]) -> Trend {
    let (Some(&first), Some(&last)) = (values.first(), values.last()) else {
        return Trend {
            first: 0.0,
            last: 0.0,
            delta: 0.0,
            monotone: 0.0,
        };
    };
    let delta = last - first;
    let steps = values.len().saturating_sub(1);
    let monotone = if steps == 0 || delta == 0.0 {
        0.0
    } else {
        let agreeing = values
            .windows(2)
            .filter(|w| match w {
                [a, b] => (b - a) * delta > 0.0,
                _ => false,
            })
            .count();
        agreeing as f32 / steps as f32
    };
    Trend {
        first,
        last,
        delta,
        monotone,
    }
}

/// The three series a run produces, in table order.
fn series(samples: &[HorizonSample]) -> [(&'static str, Vec<f32>); 3] {
    [
        (
            "coverage",
            samples.iter().map(|s| s.coverage).collect::<Vec<_>>(),
        ),
        (
            "peak/mean",
            samples.iter().map(|s| s.peak_to_mean).collect::<Vec<_>>(),
        ),
        (
            "footprint",
            // The first row has no predecessor, so the motion series is one
            // shorter than the other two rather than carrying a fabricated zero.
            samples
                .iter()
                .filter_map(|s| s.footprint_diff)
                .collect::<Vec<_>>(),
        ),
    ]
}

// ---------------------------------------------------------------------------
// Presentations
// ---------------------------------------------------------------------------

/// Simulated seconds the run's **last row** actually sits at.
///
/// Not the same as the requested horizon, which is why it exists (Plan 0099).
/// Rows are exact multiples of the interval — the property that makes row *k*
/// comparable between a two-minute run and a ten-minute one — so a request the
/// interval does not divide is silently rounded **down** to the last whole
/// interval. Nothing else here could tell you that had happened.
pub fn reached_secs(run: &HorizonRun) -> f32 {
    run.samples.last().map_or(0.0, |s| s.elapsed_secs)
}

/// Simulated seconds the run fell short of what `--horizon` asked for, or `None`
/// when it reached the request.
///
/// The tolerance is one frame: `elapsed_secs` counts the clock forward *before*
/// each frame draws, so an exactly-divided request lands one step past the
/// nominal length rather than on it, and that is not a shortfall.
pub fn shortfall_secs(run: &HorizonRun) -> Option<f32> {
    let short = run.minutes * 60.0 - reached_secs(run);
    (short > 1.0 / CAPTURE_HZ).then_some(short)
}

/// The horizon table, as a string. Returns rather than prints so the formatting
/// is directly testable.
pub fn text_table(source: &str, run: &HorizonRun) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "horizon: {} over {:.1} simulated minutes, one row per {:.0}s \
         [{source}] tier {} at {}x{}",
        run.preset,
        reached_secs(run) / 60.0,
        run.interval_secs,
        run.tier.as_str(),
        run.width,
        run.height
    );
    // The header states what the run *reached*, so when that is short of what
    // was asked for the difference is stated too — right above the table rather
    // than on stderr, because the table is what gets read and copied into a
    // world's header (Plan 0099).
    if let Some(short) = shortfall_secs(run) {
        let _ = writeln!(
            out,
            "  SHORT of the {:.1} minutes --horizon asked for, by {short:.1}s: rows are \
             exact multiples\n  of the {:.0}s interval, so the request was rounded down to \
             the last whole one.",
            run.minutes, run.interval_secs
        );
    }
    let _ = writeln!(
        out,
        "  {:>10} {:>10} {:>10} {:>10}",
        "sim_secs", "coverage", "peak/mean", "footprint"
    );
    for s in &run.samples {
        let motion = match s.footprint_diff {
            Some(v) => format!("{v:>10.4}"),
            // No predecessor. A zero here would read as a frozen world.
            None => format!("{:>10}", "-"),
        };
        let _ = writeln!(
            out,
            "  {:>10.1} {:>10.4} {:>10.3} {motion}",
            s.elapsed_secs, s.coverage, s.peak_to_mean
        );
    }

    let _ = writeln!(
        out,
        "\ntrend (first -> last, and how much of the travel went one way):"
    );
    for (name, values) in series(&run.samples) {
        let t = trend(&values);
        let _ = writeln!(
            out,
            "  {name:<10} {:>9.4} -> {:>9.4}   delta {:>+9.4}   monotone {:.2}",
            t.first, t.last, t.delta, t.monotone
        );
    }
    let _ = writeln!(
        out,
        "\nno threshold is applied and this is not a gate (ADR-0099). Read the \
         trend against a\nstatic control world run at the same horizon; record \
         the verdict in the world's own header."
    );

    let _ = writeln!(
        out,
        "\ncost on this machine ({} {}, {} adapter) — reported, not asserted \
         (ADR-0071):",
        std::env::consts::OS,
        std::env::consts::ARCH,
        if run.cost.software_adapter {
            "software"
        } else {
            "hardware"
        }
    );
    let _ = writeln!(
        out,
        "  {} frames in {:.1}s wall clock ({:.0} frames/s)",
        run.cost.frames,
        run.cost.wall_secs,
        run.cost.frames as f32 / run.cost.wall_secs.max(f32::EPSILON)
    );
    let _ = writeln!(out, "  resident set {}", rss_line(&run.cost));
    out
}

/// The resident-set half of the cost block, in MB, or a note when the OS query
/// is unavailable (the module is a no-op outside Windows and macOS).
fn rss_line(cost: &RunCost) -> String {
    const MB: f64 = 1024.0 * 1024.0;
    match (cost.rss_before_bytes, cost.rss_after_bytes) {
        (Some(before), Some(after)) => format!(
            "{:.0} -> {:.0} MB (end of run, not a sampled peak)",
            before as f64 / MB,
            after as f64 / MB
        ),
        _ => "unavailable on this platform".to_string(),
    }
}

/// The same run as JSON, in the hand-rolled style `--report --json` uses.
pub fn json_report(source: &str, run: &HorizonRun) -> String {
    let mut out = String::from("{");
    let _ = write!(out, "\"source\":{},", json_string(source));
    let _ = write!(out, "\"preset\":{},", json_string(&run.preset));
    let _ = write!(out, "\"tier\":{},", json_string(run.tier.as_str()));
    let _ = write!(out, "\"width\":{},\"height\":{},", run.width, run.height);
    let _ = write!(out, "\"minutes\":{},", num(run.minutes));
    // What the run reached, beside what it was asked for — a consumer must be
    // able to tell a short run from a whole one without re-deriving it from the
    // last sample (Plan 0099).
    let _ = write!(out, "\"reached_secs\":{},", num(reached_secs(run)));
    let _ = write!(
        out,
        "\"shortfall_secs\":{},",
        shortfall_secs(run).map_or("null".to_string(), num)
    );
    let _ = write!(out, "\"truncated\":false,");
    let _ = write!(out, "\"interval_secs\":{},", num(run.interval_secs));

    out.push_str("\"samples\":[");
    for (i, s) in run.samples.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        let motion = match s.footprint_diff {
            Some(v) => num(v),
            // Explicitly null: a consumer must be able to tell "no predecessor"
            // from "did not move".
            None => "null".to_string(),
        };
        let _ = write!(
            out,
            "{{\"frame\":{},\"elapsed_secs\":{},\"coverage\":{},\
             \"peak_to_mean\":{},\"footprint_diff\":{motion}}}",
            s.frame,
            num(s.elapsed_secs),
            num(s.coverage),
            num(s.peak_to_mean)
        );
    }
    out.push_str("],");

    out.push_str("\"trend\":{");
    for (i, (name, values)) in series(&run.samples).into_iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        let t = trend(&values);
        let _ = write!(
            out,
            "{}:{{\"first\":{},\"last\":{},\"delta\":{},\"monotone\":{}}}",
            json_string(name),
            num(t.first),
            num(t.last),
            num(t.delta),
            num(t.monotone)
        );
    }
    out.push_str("},");

    let _ = write!(
        out,
        "\"cost\":{{\"frames\":{},\"wall_secs\":{},\"software_adapter\":{},\
         \"os\":{},\"arch\":{},\"rss_before_bytes\":{},\"rss_after_bytes\":{}}}",
        run.cost.frames,
        num(run.cost.wall_secs),
        run.cost.software_adapter,
        json_string(std::env::consts::OS),
        json_string(std::env::consts::ARCH),
        run.cost
            .rss_before_bytes
            .map_or("null".to_string(), |v| v.to_string()),
        run.cost
            .rss_after_bytes
            .map_or("null".to_string(), |v| v.to_string()),
    );
    out.push('}');
    out.push('\n');
    out
}

/// What to print, on **stdout**, when the render died before reaching the
/// requested length (Plan 0099).
///
/// The point is the channel. Before this, a horizon that could not finish
/// returned an error and printed nothing where the table goes — the whole result
/// existed only as one stderr line, which a reader piping stdout to a file or
/// scrolling to the trend block never saw. A truncated run is a *result* about
/// the machine, so it is reported where results are reported, and it still exits
/// non-zero.
fn truncation_report(source: &str, run: &TruncatedRun, json: bool) -> String {
    if json {
        let mut out = String::from("{");
        let _ = write!(out, "\"source\":{},", json_string(source));
        let _ = write!(out, "\"preset\":{},", json_string(&run.preset));
        let _ = write!(out, "\"minutes\":{},", num(run.minutes));
        let _ = write!(out, "\"interval_secs\":{},", num(run.interval_secs));
        let _ = write!(out, "\"requested_frames\":{},", run.requested_frames);
        let _ = write!(out, "\"truncated\":true,");
        let _ = write!(out, "\"samples\":[],");
        let _ = write!(out, "\"error\":{},", json_string(&run.error));
        let _ = write!(
            out,
            "\"cost\":{{\"wall_secs\":{},\"rss_at_failure_bytes\":{}}}",
            num(run.wall_secs),
            run.rss_bytes.map_or("null".to_string(), |v| v.to_string())
        );
        out.push('}');
        out.push('\n');
        return out;
    }

    const MB: f64 = 1024.0 * 1024.0;
    let mut out = String::new();
    let _ = writeln!(
        out,
        "horizon: {} TRUNCATED — asked for {:.1} simulated minutes ({} frames) and did \
         not finish.\n  There is no table: the run failed at `{}` after {:.1}s wall clock.",
        run.preset, run.minutes, run.requested_frames, run.error, run.wall_secs
    );
    if let Some(rss) = run.rss_bytes {
        let _ = writeln!(out, "  Resident set at failure: {:.0} MB.", rss as f64 / MB);
    }
    let _ = writeln!(
        out,
        "\n  This is a limit of this machine, not of the world. The capture path retains\n  \
         nothing per frame (Plan 0099), so --interval is NOT a lever on it: what is left\n  \
         is the per-frame cost itself, which a smaller --size or a shorter --horizon\n  \
         lowers. See docs/capturing.md for the verified ceiling and the box it was\n  \
         verified on."
    );
    out
}

/// The half of a run that exists when it did not finish — what
/// [`truncation_report`] needs, and nothing that requires rows.
struct TruncatedRun {
    preset: String,
    minutes: f32,
    interval_secs: f32,
    requested_frames: u32,
    wall_secs: f32,
    rss_bytes: Option<u64>,
    error: String,
}

// ---------------------------------------------------------------------------
// The run itself
// ---------------------------------------------------------------------------

/// Measure one run's rows from the captured interval images.
///
/// The background is sampled **once**, from the first image, and held for every
/// row. Re-sampling per row would let the mask move under the statistics, so a
/// world whose backdrop brightens would report a coverage change that is really
/// a change of ruler.
fn measure(frames: &[u32], images: &[CaptureImage]) -> Vec<HorizonSample> {
    let bg = images.first().map_or([0, 0, 0, 255], corner);
    let mut samples = Vec::with_capacity(images.len());
    for (i, img) in images.iter().enumerate() {
        samples.push(HorizonSample {
            frame: frames.get(i).copied().unwrap_or(0),
            elapsed_secs: elapsed_secs(frames.get(i).copied().unwrap_or(0)),
            coverage: coverage(img, bg, COVERAGE_EPS),
            footprint_diff: i
                .checked_sub(1)
                .and_then(|prev| images.get(prev))
                .map(|prev| footprint_diff(prev, img, bg, COVERAGE_EPS, FOOTPRINT_MIN_FRAC)),
            peak_to_mean: peak_to_mean(img, bg, COVERAGE_EPS),
        });
    }
    samples
}

/// The frame's own ground, sampled at the top-left pixel — the same convention
/// `--report`'s coverage column uses, so "lit" means one thing across the two.
fn corner(img: &CaptureImage) -> [u8; 4] {
    [
        img.rgba.first().copied().unwrap_or(0),
        img.rgba.get(1).copied().unwrap_or(0),
        img.rgba.get(2).copied().unwrap_or(0),
        255,
    ]
}

/// Render the horizon and print it.
///
/// The wall-clock reads here are the shell's own instrumentation, off any hot
/// path and outside the core — analysis stays clock-free.
#[allow(
    clippy::disallowed_methods,
    reason = "reporting a headless CLI's own wall-clock cost; core analysis stays clock-free"
)]
pub fn run(presets: Vec<Preset>, source: &str, req: &HorizonRequest) -> Result<(), String> {
    let frames = sample_frames(req.minutes, req.interval_secs)?;
    let name = match (&req.preset, presets.as_slice()) {
        (Some(name), _) => name.clone(),
        (None, [only]) => only.name.clone(),
        (None, _) => {
            return Err("--horizon runs one world: name it with --preset <name>".to_string());
        }
    };

    let total = frames.last().copied().unwrap_or(0) + 1;
    eprintln!(
        "horizon: rendering {total} frames ({:.1} simulated minutes) of `{name}` — \
         this is slow by construction (ADR-0099)",
        req.minutes
    );

    let mut r = super::renderer(req.width, req.height, presets, req.tier)?;
    let software_adapter = r.adapter_is_software();
    let rss_before = crate::rss::current_rss_bytes();
    let started = Instant::now();
    let images = match r.capture_preset_at(&name, &req.stimulus, &frames) {
        Ok(images) => images,
        Err(e) => {
            // Report the truncation where the table would have been, then still
            // fail — the caller's non-zero exit is what a script reads, and this
            // block is what a person reads (Plan 0099).
            print!(
                "{}",
                truncation_report(
                    source,
                    &TruncatedRun {
                        preset: name.clone(),
                        minutes: req.minutes,
                        interval_secs: req.interval_secs,
                        requested_frames: total,
                        wall_secs: started.elapsed().as_secs_f32(),
                        rss_bytes: crate::rss::current_rss_bytes(),
                        error: e.to_string(),
                    },
                    req.json,
                )
            );
            return Err(format!("horizon `{name}`: {e}"));
        }
    };
    let wall_secs = started.elapsed().as_secs_f32();
    let rss_after = crate::rss::current_rss_bytes();

    let run = HorizonRun {
        preset: name,
        tier: req.tier,
        width: req.width,
        height: req.height,
        minutes: req.minutes,
        interval_secs: req.interval_secs,
        samples: measure(&frames, &images),
        cost: RunCost {
            frames: total,
            wall_secs,
            rss_before_bytes: rss_before,
            rss_after_bytes: rss_after,
            software_adapter,
        },
    };

    if req.json {
        print!("{}", json_report(source, &run));
    } else {
        print!("{}", text_table(source, &run));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
