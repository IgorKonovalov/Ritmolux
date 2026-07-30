//! ~1 Hz structured diagnostics logger for the standalone (Plan 0011).
//!
//! Appends one tab-separated sample per second to a rotating `diagnostics.log`
//! under the per-user app dir. Lives on the render/UI thread only — never the
//! capture/audio thread (the audio callback is sacred). File I/O at 1 Hz on the
//! render thread is well within budget.

use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use lmv_core::diag::{AnalysisMetrics, Metrics};

/// One sample per second.
const LOG_INTERVAL: Duration = Duration::from_secs(1);
/// Rotate the active log once it passes this size (keeps one `.1` backup).
const MAX_LOG_BYTES: u64 = 1024 * 1024;
/// Column header written when a fresh log is created.
///
/// **The eight leading columns and their names are frozen** — anything already
/// parsing this file reads them by index and must keep working. The analysis
/// columns (Plan 0049 Phase 4) are appended, never interleaved.
///
/// Tab-separated with a named header is what makes a run of these lines
/// something a lock **rate** can be computed from: split on `\t`, take the
/// column, no regex tuned to one value's formatting.
const HEADER: &str = "unix_ms\tfps\tframe_ms_avg\tframe_ms_p99\tframes_total\tframes_dropped\tgpu_bytes\trss_bytes\tbass\tmid\ttreb\tonset\tdownbeat_confidence\tdownbeat_locked\n";

/// Appends diagnostics samples to a rotating log file at ~1 Hz.
pub struct DiagLog {
    path: Option<PathBuf>,
    file: Option<File>,
    last: Instant,
}

impl DiagLog {
    /// A logger writing to `path` (an unresolved `None` path silently no-ops, so
    /// a machine without a resolvable data dir still runs — degrade, never crash).
    pub fn new(path: Option<PathBuf>) -> Self {
        // Log-cadence pacing reads the wall clock in the shell; core analysis
        // stays clock-free (determinism).
        #[allow(
            clippy::disallowed_methods,
            reason = "log-cadence start on the render thread; core analysis stays clock-free"
        )]
        let last = Instant::now();
        Self {
            path,
            file: None,
            last,
        }
    }

    /// Write a sample if a second has elapsed since the last.
    ///
    /// `analysis` and `rss` are both evaluated **lazily** — only on the seconds a
    /// sample is actually due. This runs every frame, so nothing here may become
    /// per-frame work; the closures are what keeps that true as the row grows.
    #[allow(
        clippy::disallowed_methods,
        reason = "log-cadence + sample timestamp reads on the render thread; core analysis stays clock-free"
    )]
    pub fn maybe_log(
        &mut self,
        metrics: &Metrics,
        analysis: impl FnOnce() -> AnalysisMetrics,
        rss: impl FnOnce() -> Option<u64>,
    ) {
        if self.last.elapsed() < LOG_INTERVAL {
            return;
        }
        self.last = Instant::now();

        let Some(path) = self.path.clone() else {
            return;
        };
        self.rotate_if_large(&path);
        if self.file.is_none() {
            self.open(&path);
        }
        let Some(file) = self.file.as_mut() else {
            return;
        };

        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let rss = rss().unwrap_or(0);
        let a = analysis();
        let _ = writeln!(
            file,
            "{ts}\t{:.1}\t{:.3}\t{:.3}\t{}\t{}\t{}\t{}\t{}",
            metrics.fps,
            metrics.frame_ms_avg,
            metrics.frame_ms_p99,
            metrics.frames_total,
            metrics.frames_dropped,
            metrics.gpu_bytes,
            rss,
            AnalysisFields(a),
        );
        let _ = file.flush();
    }

    /// Open (creating dirs and the file) for appending, writing the header if the
    /// file is new. A failure is reported once and leaves the logger dormant.
    fn open(&mut self, path: &Path) {
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        // A log left behind by an older build names a different set of columns,
        // and the header is only ever written to a *new* file — so appending
        // would give one file rows of two different widths under a header that
        // describes neither. Phase 6 computes a lock rate by column index off
        // exactly this file, so rotate the stale one instead of growing it.
        if path.exists() && !header_is_current(path) {
            self.rotate(path);
        }
        let is_new = !path.exists();
        match OpenOptions::new().create(true).append(true).open(path) {
            Ok(mut file) => {
                if is_new {
                    let _ = file.write_all(HEADER.as_bytes());
                }
                self.file = Some(file);
            }
            Err(err) => eprintln!("diagnostics log unavailable ({err})"),
        }
    }

    /// Back-date the cadence clock so the next [`maybe_log`](DiagLog::maybe_log)
    /// writes. The interval is a real second; a test that slept for it would be a
    /// second slower for nothing.
    #[cfg(test)]
    fn force_due(&mut self) {
        self.last = self.last.checked_sub(LOG_INTERVAL).unwrap_or(self.last);
    }

    /// Rotate to a single `.1` backup once the active log grows past the cap.
    fn rotate_if_large(&mut self, path: &Path) {
        let too_big = fs::metadata(path)
            .map(|m| m.len() > MAX_LOG_BYTES)
            .unwrap_or(false);
        if too_big {
            self.rotate(path);
        }
    }

    /// Close the active file and rename `path` to its single `.1` backup.
    fn rotate(&mut self, path: &Path) {
        self.file = None; // close before renaming
        let backup = path.with_extension("log.1");
        let _ = fs::rename(path, &backup);
    }
}

/// Whether `path`'s first line is the header this build writes. An unreadable or
/// empty file answers `false`, which starts a fresh one — the conservative
/// direction, since the alternative is appending rows nothing can index.
fn header_is_current(path: &Path) -> bool {
    let Ok(file) = File::open(path) else {
        return false;
    };
    let mut first = String::new();
    match BufReader::new(file).read_line(&mut first) {
        Ok(_) => first == HEADER,
        Err(_) => false,
    }
}

/// The analysis columns of one row, in header order.
///
/// A `Display` newtype rather than six more arms inline, so the column order and
/// their formatting are one thing a unit test can exercise without a file or a
/// running app.
///
/// **`downbeat_locked` is written as `0`/`1` deliberately.** A run of these lines
/// is what Plan 0048 Phase 6 computes a lock **rate** from, and with 0/1 that rate
/// is the mean of the column — no string matching, no regex, the same arithmetic
/// as every other column.
struct AnalysisFields(AnalysisMetrics);

impl fmt::Display for AnalysisFields {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let a = self.0;
        write!(
            f,
            "{:.4}\t{:.4}\t{:.4}\t{:.4}\t{:.4}\t{}",
            a.bass,
            a.mid,
            a.treb,
            a.onset,
            a.downbeat_confidence,
            u8::from(a.downbeat_locked),
        )
    }
}

#[cfg(test)]
mod tests {
    //! The log is the half of Plan 0048 Phase 6 that is **recorded** rather than
    //! watched: a run of these lines is what a lock rate is computed from. So
    //! these drive the real analyzer over real synthesized PCM rather than a
    //! hand-made `AnalysisFrame` — a suite that only ever feeds itself
    //! synthesized frames is silence, not evidence, about the thing being logged.

    use std::io::Read as _;

    use lmv_core::dsp::{Analyzer, HOP_SIZE, WARMUP_HOPS};

    use super::*;

    /// The eight columns that existed before Plan 0049, in order. **Frozen** —
    /// anything already parsing this file reads them by index.
    const LEGACY_COLUMNS: [&str; 8] = [
        "unix_ms",
        "fps",
        "frame_ms_avg",
        "frame_ms_p99",
        "frames_total",
        "frames_dropped",
        "gpu_bytes",
        "rss_bytes",
    ];
    const ANALYSIS_COLUMNS: [&str; 6] = [
        "bass",
        "mid",
        "treb",
        "onset",
        "downbeat_confidence",
        "downbeat_locked",
    ];

    fn columns() -> Vec<&'static str> {
        HEADER.trim_end().split('\t').collect()
    }

    fn read(path: &Path) -> String {
        let mut s = String::new();
        File::open(path)
            .expect("open the log")
            .read_to_string(&mut s)
            .expect("read the log");
        s
    }

    /// A scratch directory that cleans itself up, keyed by the caller so parallel
    /// tests in this binary never share a path.
    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("lmv-diaglog-{name}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create the scratch dir");
        dir.join("diagnostics.log")
    }

    /// The analysis a real `--signal` clip produces, taken past warm-up.
    fn analysis_over_signal(spec: &str) -> AnalysisMetrics {
        let (pcm, format) = standalone::shot::args::synth_signal(spec).expect("synthesize signal");
        let mut analyzer = Analyzer::new(format).expect("the synthesized format is valid");
        let hop_samples = HOP_SIZE * format.channels as usize;
        let mut latest = None;
        for (index, hop) in pcm.chunks(hop_samples).enumerate() {
            analyzer.push_interleaved(hop);
            let frame = analyzer.take_frame();
            if index >= WARMUP_HOPS {
                latest = Some(AnalysisMetrics::from(&frame));
            }
        }
        latest.expect("the clip outlasts warm-up")
    }

    /// **The six values reach the file, over real audio, and come back out by
    /// column index.** No regex, no formatting-specific parse: split on tab, take
    /// the column the header names, parse a number.
    #[test]
    fn a_signal_run_logs_all_six_analysis_values() {
        let path = scratch("signal");
        let analysis = analysis_over_signal("dynamic:120");
        // A steady tone would leave every band pinned and prove nothing about the
        // columns tracking their frame, so the one signal kind with dynamics.
        assert!(
            analysis.bass > 0.0 || analysis.mid > 0.0 || analysis.treb > 0.0,
            "the clip produced silence, so this test would pass on any wiring: {analysis:?}"
        );

        let mut log = DiagLog::new(Some(path.clone()));
        log.force_due();
        log.maybe_log(
            &Metrics {
                fps: 60.0,
                frames_total: 120,
                ..Metrics::default()
            },
            || analysis,
            || Some(1234),
        );

        let text = read(&path);
        let mut lines = text.lines();
        assert_eq!(lines.next(), Some(HEADER.trim_end()), "header first");
        let row: Vec<&str> = lines.next().expect("one sample row").split('\t').collect();
        assert_eq!(row.len(), columns().len(), "row width matches the header");

        let column = |name: &str| -> f32 {
            let i = columns()
                .iter()
                .position(|c| *c == name)
                .unwrap_or_else(|| panic!("no `{name}` column"));
            row[i]
                .parse()
                .unwrap_or_else(|e| panic!("`{name}` = `{}` does not parse: {e}", row[i]))
        };
        assert_eq!(column("bass"), round4(analysis.bass));
        assert_eq!(column("mid"), round4(analysis.mid));
        assert_eq!(column("treb"), round4(analysis.treb));
        assert_eq!(column("onset"), round4(analysis.onset));
        assert_eq!(
            column("downbeat_confidence"),
            round4(analysis.downbeat_confidence)
        );
        // 0/1, so the lock RATE Phase 6 records is the mean of this column.
        assert_eq!(
            column("downbeat_locked"),
            f32::from(analysis.downbeat_locked)
        );

        let _ = fs::remove_dir_all(path.parent().expect("the scratch dir"));
    }

    /// What `{:.4}` does to a value, so the assertions above compare like with
    /// like instead of asserting the formatter is lossless.
    fn round4(v: f32) -> f32 {
        (v * 10_000.0).round() / 10_000.0
    }

    /// The existing columns and their names are unchanged and still lead the row,
    /// so anything already parsing these lines keeps working. The new columns are
    /// appended, never interleaved.
    #[test]
    fn the_legacy_columns_are_unchanged_and_still_lead() {
        let cols = columns();
        assert_eq!(
            cols.get(..LEGACY_COLUMNS.len()),
            Some(&LEGACY_COLUMNS[..]),
            "the pre-Plan-0049 columns moved or were renamed: {cols:?}"
        );
        assert_eq!(
            cols.get(LEGACY_COLUMNS.len()..),
            Some(&ANALYSIS_COLUMNS[..]),
            "the analysis columns are not the appended tail: {cols:?}"
        );
    }

    /// **A log from an older build is rotated, not appended to.** Its header names
    /// eight columns and the rows this build writes carry fourteen; the header is
    /// only ever written to a new file, so appending would leave one file with two
    /// row widths under a header describing neither — and Phase 6 reads this file
    /// by column index.
    #[test]
    fn a_stale_header_rotates_instead_of_appending() {
        let path = scratch("stale");
        let stale = "unix_ms\tfps\trss_bytes\n1\t60.0\t99\n";
        fs::write(&path, stale).expect("seed a stale log");

        let mut log = DiagLog::new(Some(path.clone()));
        log.force_due();
        log.maybe_log(&Metrics::default(), AnalysisMetrics::default, || None);

        let text = read(&path);
        assert!(
            text.starts_with(HEADER),
            "the fresh log does not lead with the current header: {text:?}"
        );
        assert!(
            !text.contains("\t99\n"),
            "the stale rows were appended to rather than rotated away: {text:?}"
        );
        // Rotated, not destroyed — the old samples are still on disk.
        let backup = path.with_extension("log.1");
        assert_eq!(read(&backup), stale, "the stale log was not preserved");

        let _ = fs::remove_dir_all(path.parent().expect("the scratch dir"));
    }

    /// A log this build wrote is appended to, not rotated on every open — the
    /// check above must be a header comparison, not "rotate whenever it exists".
    #[test]
    fn a_current_header_is_appended_to() {
        let path = scratch("append");
        let mut log = DiagLog::new(Some(path.clone()));
        for _ in 0..3 {
            log.force_due();
            log.maybe_log(&Metrics::default(), AnalysisMetrics::default, || None);
        }
        let text = read(&path);
        assert_eq!(
            text.lines().count(),
            4,
            "expected one header + three samples: {text:?}"
        );
        assert!(
            !path.with_extension("log.1").exists(),
            "a current log was rotated"
        );

        let _ = fs::remove_dir_all(path.parent().expect("the scratch dir"));
    }
}
