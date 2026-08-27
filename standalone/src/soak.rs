//! Long-run soak instrumentation for the standalone (Plan 0009 Phase 5).
//!
//! With `--soak <path>`, appends one sample every few seconds — elapsed time,
//! fps, resident-set bytes, total frames, a heartbeat counter, and (since
//! Plan 0085 Phase 3) a switch counter with the two frame-time columns below —
//! so a multi-hour session yields a measurable fps/RSS trace to check for drift
//! or stalls (Phase 6's ≥4-hour run reads it). Off by default: when `--soak`
//! isn't passed there is no `SoakLog` at all, so the render loop is unchanged.
//!
//! Lives on the render/UI thread only (never the sacred audio callback). The
//! per-frame cost is a single elapsed-time comparison that returns immediately;
//! the actual file write happens only on the coarse sample tick, off the
//! per-frame hot path. RSS is reused from the diagnostics query (`rss.rs`) — no
//! new dependency, no second OS binding.
//!
//! ## What a preset switch does to the log (Plan 0085 Phase 3)
//!
//! Three columns are **appended** — never interleaved, the same frozen-prefix
//! rule `diagnostics.log` follows — because two open questions both turn on an
//! axis this log could not see:
//!
//! - **Resident set.** A three-minute run grew 385 → 663 MB while switching
//!   presets, and with no switch marker nothing separates *per-switch cost*
//!   (each switch builds a side's GPU resources) from *growth that does not
//!   stop*. `switches` is that marker.
//! - **The statistic the unbuilt quality governor is specified to read.** In the
//!   same run `frame_ms_p99` peaked at **25.037 ms** while `frame_ms_avg` never
//!   passed 8.749 and **zero of 28,698 frames dropped**. The spikes coincide
//!   with preset switches and a fullscreen toggle — GPU resource rebuilds, not
//!   steady-state cost. A governor reading p99 as it stands would demote a
//!   preset running at 165 fps, during the one event that is already visually
//!   disruptive. `frame_ms_p99_steady` is the same statistic with those windows
//!   left out (backlog 0082, ADR-0099).
//!
//! Nothing here is a gate, and none of it runs in CI.

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use lmv_core::diag::Metrics;

/// One soak sample every few seconds — coarse enough to stay off the per-frame
/// path, fine enough that a multi-hour run has thousands of points.
const SAMPLE_INTERVAL: Duration = Duration::from_secs(5);
/// Column header written when a fresh soak log is created. Append-only: a
/// consumer that knows the first five fields keeps working across this change.
const HEADER: &str = "elapsed_secs\tfps\trss_bytes\tframes_total\theartbeat\t\
                      switches\tframe_ms_p99\tframe_ms_p99_steady\n";

/// Frames after a switch or surface reconfigure during which `frame_ms_p99` is
/// **not** trusted for the steady column.
///
/// **The derivation, and the reason it is a frame count rather than a
/// duration.** `frame_ms_p99` is computed over the core's rolling ring of frame
/// times, so a rebuild spike keeps influencing it until the ring has turned
/// over — a property of *frames*, not of seconds, and one that would need a
/// different constant at every refresh rate if expressed in time. The ring's
/// length is `pub(crate)` in `lmv_core::diag` and deliberately not exported (it
/// is coupled to the quality governor's minimum series), so this constant cannot
/// simply reference it. It is instead **measured** from the public
/// [`FrameStats`](lmv_core::diag::FrameStats) in this module's tests, which fail
/// if the ring ever outgrows this window.
///
/// 300 is the measured 240-frame ring with 25 % margin. The cost of the margin
/// is the only thing it trades away: at 60 fps it withholds 5 s of steady
/// readings per switch — one sample — and at 165 fps under 2 s.
const SWITCH_EXCLUSION_FRAMES: u64 = 300;

/// Appends periodic soak samples to a log file. Constructed only when `--soak`
/// is requested, so its mere existence signals the mode is on.
pub struct SoakLog {
    path: PathBuf,
    file: Option<File>,
    /// Session start, for the elapsed-time column.
    start: Instant,
    /// Deadline pacing: the wall-clock time of the last written sample.
    last: Instant,
    /// Monotonic sample counter — a heartbeat that must keep climbing for the
    /// whole run (a frozen heartbeat in the log means the render thread stalled).
    heartbeat: u64,
    /// Preset changes and surface reconfigures since session start. Monotone, so
    /// the difference between two rows is how many happened between them.
    switches: u64,
    /// `frames_total` at the most recent switch, or `None` before the first —
    /// the start of the window `frame_ms_p99` is not trusted over.
    last_switch_frame: Option<u64>,
    /// The most recent p99 read **outside** an exclusion window. Carried forward
    /// while excluded rather than blanked: a numeric column stays parseable, and
    /// the `switches` counter beside it is what tells a reader the value is
    /// being held. Divergence from the raw column *is* the finding.
    steady_p99: f32,
}

impl SoakLog {
    /// Start a soak logger writing to `path`. The file is opened lazily on the
    /// first sample.
    pub fn new(path: PathBuf) -> Self {
        // Soak timing reads the wall clock in the shell; core analysis stays
        // clock-free (determinism).
        #[allow(
            clippy::disallowed_methods,
            reason = "soak-cadence start on the render thread; core analysis stays clock-free"
        )]
        let now = Instant::now();
        eprintln!("soak mode: logging to {}", path.display());
        Self {
            path,
            file: None,
            start: now,
            last: now,
            heartbeat: 0,
            switches: 0,
            last_switch_frame: None,
            steady_p99: 0.0,
        }
    }

    /// Note a preset change or surface reconfigure at frame `frames_total`
    /// (Plan 0085 Phase 3).
    ///
    /// Called from the event path, not the render loop — a switch is a rare
    /// event, so this adds nothing to the per-frame cost. It does two things:
    /// bumps the counter the RSS question needs, and opens the window over which
    /// `frame_ms_p99` is a measurement of a GPU resource rebuild rather than of
    /// what the preset costs to run.
    pub fn note_switch(&mut self, frames_total: u64) {
        self.switches += 1;
        self.last_switch_frame = Some(frames_total);
    }

    /// Whether `frames_total` still sits inside a post-switch exclusion window.
    fn excluded(&self, frames_total: u64) -> bool {
        self.last_switch_frame
            .is_some_and(|at| frames_total.saturating_sub(at) < SWITCH_EXCLUSION_FRAMES)
    }

    /// Write a sample if the interval has elapsed. Returns immediately otherwise,
    /// so the per-frame cost is just the elapsed check. `rss` is evaluated lazily
    /// — only when a sample is actually due — to avoid a per-frame OS query.
    #[allow(
        clippy::disallowed_methods,
        reason = "soak-cadence pacing on the render thread; core analysis stays clock-free"
    )]
    pub fn maybe_sample(&mut self, metrics: &Metrics, rss: impl FnOnce() -> Option<u64>) {
        if self.last.elapsed() < SAMPLE_INTERVAL {
            return;
        }
        self.last = Instant::now();

        if self.file.is_none() {
            self.open();
        }
        if self.file.is_none() {
            return;
        }
        let elapsed = self.start.elapsed().as_secs_f64();
        let line = self.sample_line(elapsed, metrics, rss().unwrap_or(0));
        if let Some(file) = self.file.as_mut() {
            let _ = writeln!(file, "{line}");
            let _ = file.flush();
        }
    }

    /// Build one row and advance the state it carries (the heartbeat, and the
    /// steady p99 when this sample is outside an exclusion window).
    ///
    /// Split out from [`maybe_sample`](Self::maybe_sample) so the claim the
    /// steady column makes is directly testable: without this, checking that the
    /// two frame-time columns separate across a switch would need a five-second
    /// wall-clock wait and a temporary file.
    fn sample_line(&mut self, elapsed: f64, metrics: &Metrics, rss: u64) -> String {
        self.heartbeat += 1;
        // Outside an exclusion window the two frame-time columns are the same
        // reading, which is the point: they agree in steady state and separate
        // only where a rebuild is in the window.
        if !self.excluded(metrics.frames_total) {
            self.steady_p99 = metrics.frame_ms_p99;
        }
        format!(
            "{elapsed:.1}\t{:.1}\t{rss}\t{}\t{}\t{}\t{:.3}\t{:.3}",
            metrics.fps,
            metrics.frames_total,
            self.heartbeat,
            self.switches,
            metrics.frame_ms_p99,
            self.steady_p99,
        )
    }

    /// Open (creating dirs and the file) for appending, writing the header if
    /// the file is new. A failure is reported once and leaves the log dormant.
    fn open(&mut self) {
        let path: &Path = &self.path;
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let is_new = !path.exists();
        match OpenOptions::new().create(true).append(true).open(path) {
            Ok(mut file) => {
                if is_new {
                    let _ = file.write_all(HEADER.as_bytes());
                }
                self.file = Some(file);
            }
            Err(err) => eprintln!("soak log unavailable ({err})"),
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use lmv_core::diag::FrameStats;

    use super::*;

    /// A steady-state metrics snapshot at `frames_total`, with `p99` ms.
    fn metrics_at(frames_total: u64, p99: f32) -> Metrics {
        Metrics {
            fps: 165.0,
            frame_ms_avg: 8.7,
            frame_ms_p99: p99,
            frames_total,
            frames_dropped: 0,
            gpu_bytes: 0,
            draw_calls: 12,
        }
    }

    fn log() -> SoakLog {
        SoakLog::new(PathBuf::from("unused-in-these-tests.log"))
    }

    /// Field `n` (0-based) of a tab-separated row.
    fn field(line: &str, n: usize) -> &str {
        line.split('\t').nth(n).unwrap_or_else(|| {
            panic!("row has no field {n}: {line}");
        })
    }

    /// **The whole claim of Plan 0085 Phase 3**: the two frame-time columns
    /// diverge across a switch and agree during steady state.
    ///
    /// The failure this guards is two-sided and the test is written that way. If
    /// the exclusion window is too short it keeps the rebuild spike it exists to
    /// exclude, and the governor's column is no better than the one it replaces;
    /// if it is too long it hides genuine sustained cost, and a preset that
    /// really did get slower reads clean.
    #[test]
    fn the_steady_column_holds_across_a_switch_and_agrees_once_it_is_past() {
        let mut log = log();

        // Before any switch the two columns are one reading.
        let quiet = log.sample_line(5.0, &metrics_at(800, 8.7), 0);
        assert_eq!(field(&quiet, 6), "8.700");
        assert_eq!(field(&quiet, 7), "8.700", "no switch, nothing to exclude");
        assert_eq!(field(&quiet, 5), "0", "no switches yet");

        // A switch, and a tick inside the window where p99 carries the rebuild
        // spike: the raw column reports it and the steady column does not.
        log.note_switch(900);
        let spike = log.sample_line(10.0, &metrics_at(1000, 25.037), 0);
        assert_eq!(field(&spike, 5), "1", "the switch counter did not climb");
        assert_eq!(
            field(&spike, 6),
            "25.037",
            "the raw column must still spike"
        );
        assert_eq!(
            field(&spike, 7),
            "8.700",
            "the steady column took the rebuild spike — the exclusion window is \
             too short, and a governor reading it would demote on a switch"
        );

        // Past the window, the columns agree again — including on a *genuine*
        // rise, which is the half a too-long window would hide.
        let past = log.sample_line(15.0, &metrics_at(900 + SWITCH_EXCLUSION_FRAMES, 12.5), 0);
        assert_eq!(field(&past, 6), "12.500");
        assert_eq!(
            field(&past, 7),
            "12.500",
            "the steady column is still holding past the exclusion window — \
             sustained cost would never reach it"
        );

        // ...and the boundary is exclusive on the low side, so the last excluded
        // frame really is excluded.
        let mut log = self::log();
        log.note_switch(900);
        let edge = log.sample_line(
            20.0,
            &metrics_at(900 + SWITCH_EXCLUSION_FRAMES - 1, 25.0),
            0,
        );
        assert_eq!(field(&edge, 7), "0.000", "one frame short of the boundary");
    }

    /// The counter is monotone and counts every switch, so the difference
    /// between two rows is how many happened between them — which is what makes
    /// per-switch RSS cost separable from growth that does not stop.
    #[test]
    fn the_switch_counter_climbs_once_per_switch_and_never_falls() {
        let mut log = log();
        let mut seen: Vec<u64> = Vec::new();
        for i in 0..5u64 {
            log.note_switch(i * 400);
            let line = log.sample_line(i as f64, &metrics_at(i * 400 + 10, 9.0), 0);
            seen.push(field(&line, 5).parse().expect("the counter is an integer"));
        }
        assert_eq!(seen, vec![1, 2, 3, 4, 5]);

        // A tick with no switch between it and the last one repeats the count
        // rather than inventing one.
        let line = log.sample_line(6.0, &metrics_at(9_000, 9.0), 0);
        assert_eq!(field(&line, 5), "5");
        // The heartbeat, meanwhile, climbs once per *sample* — the two counters
        // are independent, and a reader compares them.
        assert_eq!(field(&line, 4), "6");
    }

    /// The header only ever grows: a consumer that knows the original five
    /// fields keeps working, which is the same frozen-prefix rule
    /// `diagnostics.log` follows.
    #[test]
    fn the_header_appends_and_matches_the_rows_it_labels() {
        const ORIGINAL: &str = "elapsed_secs\tfps\trss_bytes\tframes_total\theartbeat";
        assert!(
            HEADER.starts_with(ORIGINAL),
            "the frozen prefix moved: {HEADER}"
        );
        for name in ["switches", "frame_ms_p99", "frame_ms_p99_steady"] {
            assert!(HEADER.contains(name), "no `{name}` column: {HEADER}");
        }

        // Every column is labelled, and no row carries a field the header does
        // not name.
        let columns = HEADER.trim_end().split('\t').count();
        let line = log().sample_line(5.0, &metrics_at(100, 8.0), 123);
        assert_eq!(
            line.split('\t').count(),
            columns,
            "row and header disagree:\n{HEADER}{line}"
        );
        assert_eq!(field(&line, 2), "123", "the RSS column moved");
    }

    /// **The coupling this file cannot express in code.**
    ///
    /// [`SWITCH_EXCLUSION_FRAMES`] has to outlast the ring `frame_ms_p99` is
    /// computed over, and that ring's length is `pub(crate)` in `lmv_core::diag`
    /// — deliberately, because it is coupled to the quality governor's minimum
    /// series. So it is measured here through the public API instead: feed one
    /// enormous frame, then ordinary ones, and count how many it takes before
    /// the mean returns to exactly the ordinary value. That is the point the
    /// spike has left the window.
    ///
    /// If the ring ever grows past this constant, this fails rather than the
    /// steady column quietly starting to report rebuild spikes again.
    #[test]
    fn the_exclusion_window_outlasts_the_ring_p99_is_taken_over() {
        const ORDINARY: f32 = 1.0 / 60.0;
        let mut stats = FrameStats::new();
        stats.record(1.0); // one enormous frame

        let mut frames = 0u64;
        loop {
            stats.record(ORDINARY);
            frames += 1;
            if (stats.frame_ms_avg() - ORDINARY * 1000.0).abs() < 1e-3 {
                break;
            }
            assert!(
                frames < 100_000,
                "the frame-time ring never turned over — has FrameStats changed \
                 shape? {frames} frames in"
            );
        }
        eprintln!("measured frame-time window: {frames} frames");
        assert!(
            SWITCH_EXCLUSION_FRAMES >= frames,
            "the core's frame-time window is {frames} frames but the soak log \
             stops excluding after {SWITCH_EXCLUSION_FRAMES} — a rebuild spike \
             would reach the steady column"
        );
    }
}
