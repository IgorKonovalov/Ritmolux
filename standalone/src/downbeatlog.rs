//! Per-beat downbeat decomposition log for the standalone (Plan 0086 Phase 1).
//!
//! With `--downbeat-log <path>`, appends **one tab-separated row per detected
//! beat**: the four alignment scores the 4/4 fold produces, which one it favours,
//! which one it holds, the effect size before and after its noise correction, the
//! evidence count, whether it publishes, and the four band levels for context.
//!
//! ## Why a second log
//!
//! `diagnostics.log` samples at 1 Hz and records the estimator's **outcome** —
//! locked or not, and one confidence number. That is what measured the 6.0 %
//! publish rate (ADR-0082), and it is also why the *cause* of that rate is still
//! inferred: three different failures fit the same outcome, and telling them apart
//! needs the terms, per beat, on real material. [`DownbeatTerms`] exists for
//! exactly this (Plan 0068) and nothing outside the tests reads it.
//!
//! ## What it costs the estimator: nothing
//!
//! [`Analyzer::downbeat_terms`](rlx_core::dsp::Analyzer::downbeat_terms) takes
//! `&self`, allocates nothing and reads no clock, so being observed cannot change
//! what is observed. The write itself lives on the render/UI thread — never the
//! capture callback — and is event-paced rather than clock-paced: at 120 BPM that
//! is ~2 rows/s, far coarser than the 1 Hz logger's worst case, and a frame with
//! no beat on it costs one bool test. Off by default: without the flag there is no
//! `DownbeatLog` at all and the loop is unchanged.
//!
//! Rows are written from the frame path, so a **hidden window logs nothing** — the
//! render loop returns before it takes a frame. A capture run wants the window up.

use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rlx_core::diag::DownbeatTerms;
use rlx_core::dsp::AnalysisFrame;

/// Column header written when a fresh log is created.
///
/// Tab-separated with a named header, the same shape `diagnostics.log` and
/// `soak.log` use, so a run of these lines is something a distribution can be
/// computed from: split on `\t`, take the column by name, no regex tuned to one
/// value's formatting.
///
/// The `s0..s3` block is the fold's own output and is as wide as
/// [`BEATS_PER_BAR`](rlx_core::dsp::downbeat::BEATS_PER_BAR) — a test pins that
/// rather than leaving it to whoever changes the meter assumption.
///
/// **Columns are appended, never interleaved** — the frozen-prefix rule
/// `diagnostics.log` follows — so a capture taken before a column existed stays
/// parseable by name. Three were added the same day as the first two runs,
/// because the reading needed all three and the file carried none of them:
///
/// - **`bpm`** is the tempo tracker's own estimate, and it is here to be compared
///   against the rate these very rows arrive at. The beat flag this log is paced
///   by comes from the onset detector with **no tempo gating** (`onset.rs`), so
///   "how many detections per musical beat" is a real question about the fold's
///   unit — and it was answerable only by counting rows against a wall clock and a
///   BPM the listener reported by ear.
/// - **`unix_ms`** is that time axis. Deltas between consecutive rows give the
///   inter-detection interval directly, and it lets a capture be lined up against
///   a `diagnostics.log` from the same session.
/// - **`time_since_beat`** is how stale the row's own band levels are. The bands
///   here come from the latest analysis hop, not necessarily the hop the beat
///   fired on, and this column is the size of that gap rather than an assurance
///   there isn't one.
///
/// Two more were appended by Plan 0117, and they are the reason the
/// `beat` column alone is not enough to read the alignment block
/// against:
///
/// - **`fold_beat`** is the counter the fold actually buckets by. Since
///   Plan 0095 that is the grid's tempo-driven beat count, not `beat_index` —
///   so `s0..s3`, `best` and `held` are indexed in `fold_beat % 4`, and were
///   read against `beat % 4` for as long as nothing published the difference.
/// - **`grid_bar_phase`** is where that counter sits across the bar, ungated.
///   It is a *grid* reading only where `bpm > 0`; before the grid starts the
///   fold is handed the tempo tracker's onset-reset phase instead.
const HEADER: &str = "beat\ts0\ts1\ts2\ts3\tbest\theld\teffect_raw\tnull_share\t\
                      effect_corrected\tbeats_seen\tlocked\tbass\tmid\ttreb\tonset\t\
                      bpm\ttime_since_beat\tunix_ms\tfold_beat\tgrid_bar_phase\n";

/// Appends one row per detected beat. Constructed only when `--downbeat-log` is
/// requested, so its mere existence signals the mode is on.
pub struct DownbeatLog {
    path: PathBuf,
    file: Option<File>,
}

impl DownbeatLog {
    /// Start a per-beat logger writing to `path`. The file is opened lazily, on
    /// the first beat.
    pub fn new(path: PathBuf) -> Self {
        eprintln!("downbeat log: one row per beat to {}", path.display());
        Self { path, file: None }
    }

    /// Write this frame's row, if a beat fired on it.
    ///
    /// `terms` is evaluated **lazily** — only on the frames that carry a beat —
    /// for the reason the 1 Hz logger's closures exist: this runs every frame, so
    /// nothing here may become per-frame work as the row grows. The timestamp is
    /// read on the same branch, for the same reason.
    #[allow(
        clippy::disallowed_methods,
        reason = "row timestamp on the render thread, on beat frames only; core analysis stays clock-free"
    )]
    pub fn maybe_log(&mut self, frame: &AnalysisFrame, terms: impl FnOnce() -> DownbeatTerms) {
        if !frame.beat {
            return;
        }
        if self.file.is_none() {
            self.open();
        }
        let Some(file) = self.file.as_mut() else {
            return;
        };
        let terms = terms();
        let unix_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let _ = writeln!(
            file,
            "{}",
            Row {
                frame,
                terms: &terms,
                unix_ms,
            }
        );
        let _ = file.flush();
    }

    /// Open (creating dirs and the file) for appending, writing the
    /// header when the file did not exist. A failure is reported once
    /// and leaves the log dormant.
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
            Err(err) => eprintln!("downbeat log unavailable ({err})"),
        }
    }
}

/// One row of the log, in header order.
///
/// A `Display` newtype rather than a `format!` inline, so the column order and
/// the formatting are one thing a unit test can exercise without a running app —
/// and so the score block is written by iterating the fold's array rather than by
/// four hand-written arms that a meter change would silently leave short.
///
/// **`locked` is written as `0`/`1` deliberately**, the same choice
/// `diagnostics.log` made: the publish *rate* over a run is then the mean of the
/// column, with no string matching.
struct Row<'a> {
    frame: &'a AnalysisFrame,
    terms: &'a DownbeatTerms,
    /// Wall-clock stamp for this row, read once by the caller on the beat branch.
    unix_ms: u128,
}

impl fmt::Display for Row<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let t = self.terms;
        write!(f, "{}", self.frame.beat_index)?;
        for score in &t.scores {
            write!(f, "\t{score:.4}")?;
        }
        write!(
            f,
            "\t{}\t{}\t{:.4}\t{:.4}\t{:.4}\t{}\t{}\t{:.4}\t{:.4}\t{:.4}\t{:.4}\t{:.2}\t{:.4}\t{}\
             \t{}\t{:.4}",
            t.best,
            t.held,
            t.effect_raw,
            t.null_share,
            t.effect_corrected,
            t.beats_seen,
            u8::from(t.locked),
            self.frame.bass,
            self.frame.mid,
            self.frame.treb,
            self.frame.onset,
            self.frame.bpm,
            self.frame.time_since_beat,
            self.unix_ms,
            t.fold_beat,
            t.grid_bar_phase,
        )
    }
}

#[cfg(test)]
mod tests {
    //! The claim this file has to earn is that the rows come from the **estimator**
    //! rather than from a default, so the central test drives synthesized audio
    //! through the real `Analyzer` — a suite that only ever feeds itself
    //! hand-made `DownbeatTerms` would be silence, not evidence, about the wiring.

    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

    use std::f32::consts::TAU;
    use std::io::Read as _;

    use rlx_core::audio::AudioFormat;
    use rlx_core::dsp::downbeat::BEATS_PER_BAR;
    use rlx_core::dsp::{Analyzer, HOP_SIZE};

    use super::*;

    /// 48 kHz stereo, the format the synthesized `--signal` clips use.
    const FORMAT: AudioFormat = AudioFormat {
        sample_rate: 48_000,
        channels: 2,
    };
    /// Tempo and length of the synthesized pattern: 48 beats is six times the
    /// eight-beat evidence floor and one and a half times the 32-beat history, so
    /// the ring holds nothing but the pattern by the end.
    const BPM: f32 = 120.0;
    const BEATS: usize = 48;
    /// The bar phase the pattern's kick lands on, in **pattern** beats. What the
    /// estimator sees is this phase shifted by however many beats its own detector
    /// missed during warm-up, which is why the assertions below derive the
    /// expected alignment from the log's own `bass` column rather than from here.
    const ACCENT_ON: usize = 0;

    /// The columns, in header order.
    fn columns() -> Vec<&'static str> {
        HEADER.trim_end().split('\t').collect()
    }

    /// Index of the named column, by header rather than by a hardcoded position.
    fn index(name: &str) -> usize {
        columns()
            .iter()
            .position(|c| *c == name)
            .unwrap_or_else(|| panic!("no `{name}` column"))
    }

    /// A scratch path that cleans itself up, keyed by the caller so parallel tests
    /// in this binary never share a file.
    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("lmv-downbeatlog-{name}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create the scratch dir");
        dir.join("downbeat.log")
    }

    fn read(path: &Path) -> String {
        let mut s = String::new();
        File::open(path)
            .expect("open the log")
            .read_to_string(&mut s)
            .expect("read the log");
        s
    }

    /// A deterministic broadband source — a plain xorshift, since the point is a
    /// transient the onset detector fires on, not spectral quality. Seeded, so the
    /// clip is identical on every run and every machine (NFR section 6).
    struct Noise(u32);

    impl Noise {
        fn next(&mut self) -> f32 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 17;
            self.0 ^= self.0 << 5;
            (self.0 as f32 / u32::MAX as f32) * 2.0 - 1.0
        }
    }

    /// A 4/4 pattern with a real downbeat: a short broadband click on **every**
    /// beat, and on every fourth beat a decaying low sine as well.
    ///
    /// The click is what makes each beat detectable; the kick is what makes one of
    /// the four alignments different from the other three. Both matter — a pattern
    /// accented by *presence* alone would be found by any beat counter, which is
    /// not what the fold is being asked to do.
    fn accented_pattern() -> Vec<f32> {
        let sr = FORMAT.sample_rate as f32;
        let period = ((60.0 / BPM) * sr) as usize;
        let click_len = (0.012 * sr) as usize;
        let kick_len = (0.18 * sr) as usize;
        let mut mono = vec![0.0f32; period * BEATS];
        let mut noise = Noise(0x1234_5678);

        for beat in 0..BEATS {
            let start = beat * period;
            for i in 0..click_len {
                let env = (-(i as f32) / click_len as f32 * 6.0).exp();
                let sample = noise.next() * env * 0.4;
                if let Some(slot) = mono.get_mut(start + i) {
                    *slot += sample;
                }
            }
            if beat % (BEATS_PER_BAR as usize) == ACCENT_ON {
                for i in 0..kick_len {
                    let t = i as f32 / sr;
                    let sample = (TAU * 55.0 * t).sin() * (-t * 16.0).exp() * 0.9;
                    if let Some(slot) = mono.get_mut(start + i) {
                        *slot += sample;
                    }
                }
            }
        }

        let mut pcm = Vec::with_capacity(mono.len() * FORMAT.channels as usize);
        for sample in mono {
            for _ in 0..FORMAT.channels {
                pcm.push(sample.clamp(-1.0, 1.0));
            }
        }
        pcm
    }

    /// Drive `pcm` through a real analyzer, logging every beat, and return the
    /// rows the file came back with (header stripped, split on tabs).
    fn rows_over(pcm: &[f32], path: &Path) -> Vec<Vec<String>> {
        let mut analyzer = Analyzer::new(FORMAT).expect("the synthesized format is valid");
        let mut log = DownbeatLog::new(path.to_path_buf());
        let hop_samples = HOP_SIZE * FORMAT.channels as usize;
        for hop in pcm.chunks(hop_samples) {
            analyzer.push_interleaved(hop);
            let frame = analyzer.take_frame();
            let terms = &analyzer;
            log.maybe_log(&frame, || terms.downbeat_terms());
        }
        drop(log);

        let text = read(path);
        let mut lines = text.lines();
        assert_eq!(lines.next(), Some(HEADER.trim_end()), "header first");
        lines
            .map(|line| line.split('\t').map(str::to_owned).collect())
            .collect()
    }

    /// A numeric field of a row, by column name.
    fn field(row: &[String], name: &str) -> f32 {
        let raw = &row[index(name)];
        raw.parse()
            .unwrap_or_else(|e| panic!("`{name}` = `{raw}` does not parse: {e}"))
    }

    /// **The non-vacuity check.** A synthesized 4/4 with a kick on one beat of
    /// four, through the real analyzer, produces rows whose fold favours the
    /// alignment the pattern was built with.
    ///
    /// The expected alignment is derived from the log's **own `bass` column**
    /// rather than from [`ACCENT_ON`], and that is deliberate: the analyzer's beat
    /// detector cannot fire until its window fills, so the first pattern beat or
    /// two are not in the stream at all and the estimator's beat 0 is not the
    /// pattern's. What has to hold is that the fold picks out the phase the loud
    /// beats actually landed on — which is a claim about the wiring, and is false
    /// for a log wired to a default, to a stale snapshot, or to the wrong tracker.
    ///
    /// **Which phase means which counter.** The bass is bucketed by `fold_beat`,
    /// because that is the counter `s0..s3`, `best` and `held` are indexed in
    /// (Plan 0095, ADR-0109). Bucketed by `beat` instead — as this test did until
    /// Plan 0117 gave it somewhere else to read — the accent is unambiguously on
    /// phase 0 and the fold reports 3, and the two readings are simply not
    /// commensurable rather than one of them being wrong.
    #[test]
    fn a_synthesized_4_4_favours_the_alignment_it_was_built_with() {
        let path = scratch("accented");
        let rows = rows_over(&accented_pattern(), &path);

        // A row per beat: no rows at all would make every assertion below vacuous.
        assert!(
            rows.len() > BEATS / 2,
            "expected roughly one row per beat over {BEATS} beats, got {}",
            rows.len()
        );
        for row in &rows {
            assert_eq!(
                row.len(),
                columns().len(),
                "row and header disagree: {row:?}"
            );
        }

        // Rows past the grid handover only. The fold count is `beat_index`
        // until the grid starts and the grid's count plus a whole-bar offset
        // after, and the two advance at different rates, so rows spanning the
        // handover mix two bucketings; `bpm > 0` marks it and they are skipped,
        // not deleted.
        let settled: Vec<&Vec<String>> = rows.iter().filter(|r| field(r, "bpm") > 0.0).collect();
        assert!(
            settled.len() > BEATS / 4,
            "only {} of {} rows are past the grid handover, which is too few to \
             read a fold that buckets in grid space",
            settled.len(),
            rows.len()
        );

        // Which phase the kick landed on, measured from the rows themselves —
        // bucketed by `fold_beat`, which is what the fold buckets by, and not by
        // `beat`, which counts transients and has not been the same number since
        // Plan 0095.
        let mut bass_by_phase = [(0.0f32, 0u32); BEATS_PER_BAR as usize];
        for row in &settled {
            let phase = (field(row, "fold_beat") as u32 % BEATS_PER_BAR) as usize;
            bass_by_phase[phase].0 += field(row, "bass");
            bass_by_phase[phase].1 += 1;
        }
        let means: Vec<f32> = bass_by_phase
            .iter()
            .map(|(sum, n)| if *n == 0 { 0.0 } else { sum / *n as f32 })
            .collect();
        let accent_phase = means
            .iter()
            .enumerate()
            .fold((0usize, f32::NEG_INFINITY), |best, (i, &m)| {
                if m > best.1 { (i, m) } else { best }
            })
            .0;
        eprintln!("mean bass per phase: {means:?} -> accent on {accent_phase}");
        let quiet = means
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != accent_phase)
            .map(|(_, m)| *m)
            .fold(f32::NEG_INFINITY, f32::max);
        assert!(
            means[accent_phase] > quiet * 1.2,
            "the clip did not deliver a bass accent on one phase of four ({means:?}) — \
             the test's own premise failed before the estimator was asked anything"
        );

        // The fold, read at the end of the run, agrees with it.
        let last = *settled.last().expect("at least one settled row");
        let best = field(last, "best") as usize;
        assert_eq!(
            best, accent_phase,
            "the fold favours alignment {best}, but the accent is on phase \
             {accent_phase}: {last:?}"
        );
        let winning = field(last, &format!("s{accent_phase}"));
        for phase in 0..BEATS_PER_BAR as usize {
            if phase == accent_phase {
                continue;
            }
            assert!(
                winning > field(last, &format!("s{phase}")),
                "s{accent_phase} does not lead s{phase}: {last:?}"
            );
        }
        // ...and it says so with evidence behind it: a run this long fills the
        // history, and a clean accent clears the gate.
        assert!(
            field(last, "beats_seen") >= 8.0,
            "the evidence count never climbed: {last:?}"
        );
        assert_eq!(
            field(last, "locked"),
            1.0,
            "a clean accented 4/4 should publish (corrected effect {}): {last:?}",
            field(last, "effect_corrected")
        );

        // **The `bpm` column is the estimator's own tempo, and the row rate is
        // what it has to be read against.** That comparison is the whole reason
        // the column exists: the beat flag pacing these rows is un-gated by tempo
        // (`onset.rs`), so "detections per musical beat" is a real question, and
        // on a clip built at a known tempo it has a known answer.
        let bpm = field(last, "bpm");
        eprintln!(
            "tempo estimate {bpm:.1} BPM against a clip built at {BPM}; \
             {} rows over {BEATS} beats",
            rows.len()
        );
        assert!(
            bpm > 0.0,
            "the tempo estimate never warmed, so the row cannot be read against \
             it: {last:?}"
        );
        let per_beat = rows.len() as f32 / BEATS as f32;
        assert!(
            (0.5..2.0).contains(&per_beat),
            "{per_beat:.2} detections per built beat — the clip has one transient \
             per beat by construction, so this is the log disagreeing with its own \
             stimulus"
        );

        let _ = fs::remove_dir_all(path.parent().expect("the scratch dir"));
    }

    /// The columns the first captures were taken with are unchanged and still
    /// lead the row, so those logs stay parseable by name; the three added after
    /// reading them are appended, never interleaved. Asserted as a **frozen
    /// prefix** rather than as the whole header, so the next appended column
    /// should have to move nothing here.
    #[test]
    fn the_original_columns_are_unchanged_and_still_lead() {
        const ORIGINAL: [&str; 16] = [
            "beat",
            "s0",
            "s1",
            "s2",
            "s3",
            "best",
            "held",
            "effect_raw",
            "null_share",
            "effect_corrected",
            "beats_seen",
            "locked",
            "bass",
            "mid",
            "treb",
            "onset",
        ];
        let cols = columns();
        assert_eq!(
            cols.get(..ORIGINAL.len()),
            Some(&ORIGINAL[..]),
            "the frozen prefix moved or was renamed: {cols:?}"
        );
        assert_eq!(
            cols.get(ORIGINAL.len()..),
            Some(
                &[
                    "bpm",
                    "time_since_beat",
                    "unix_ms",
                    "fold_beat",
                    "grid_bar_phase"
                ][..]
            ),
            "the added columns are not the appended tail, in the order they were \
             added: {cols:?}"
        );
    }

    /// The three effect columns are the estimator's own arithmetic, not three
    /// copies of one number: the correction discounts the raw share, and what the
    /// gate compares is the discounted one.
    #[test]
    fn the_effect_columns_carry_the_correction_the_gate_applies() {
        let path = scratch("effect");
        let rows = rows_over(&accented_pattern(), &path);
        let last = rows.last().expect("at least one row");

        let raw = field(last, "effect_raw");
        let null = field(last, "null_share");
        let corrected = field(last, "effect_corrected");
        assert!(raw > 0.0, "no between-alignment variance at all: {last:?}");
        assert!(
            null > 0.0,
            "the null share is the thing `effect_raw` is discounted by; zero \
             would mean the correction is not in the row: {last:?}"
        );
        assert!(
            corrected < raw,
            "the corrected effect ({corrected}) is not below the raw one ({raw}) \
             — the row is carrying the same number twice"
        );

        let _ = fs::remove_dir_all(path.parent().expect("the scratch dir"));
    }

    /// A frame with no beat writes nothing — the log is event-paced, and a row per
    /// *frame* would be 60 Hz of file I/O and a different instrument entirely.
    #[test]
    fn a_frame_without_a_beat_writes_no_row() {
        let path = scratch("quiet");
        let mut log = DownbeatLog::new(path.clone());
        for _ in 0..100 {
            log.maybe_log(&AnalysisFrame::default(), DownbeatTerms::default);
        }
        assert!(
            !path.exists(),
            "a run with no beats created a file: {}",
            path.display()
        );

        // ...and one with a beat does, header included.
        let beat = AnalysisFrame {
            beat: true,
            beat_index: 7,
            ..Default::default()
        };
        log.maybe_log(&beat, DownbeatTerms::default);
        let text = read(&path);
        assert!(
            text.starts_with(HEADER),
            "no header on a fresh log: {text:?}"
        );
        assert_eq!(
            text.lines().count(),
            2,
            "expected header + one row: {text:?}"
        );

        let _ = fs::remove_dir_all(path.parent().expect("the scratch dir"));
    }

    /// Every column is labelled, no row carries a field the header does not name,
    /// and the score block is as wide as the meter the fold assumes.
    #[test]
    fn the_header_labels_the_row_and_the_score_block_is_the_meter() {
        let cols = columns();
        let scores = cols
            .iter()
            .filter(|c| c.len() == 2 && c.starts_with('s'))
            .count();
        assert_eq!(
            scores, BEATS_PER_BAR as usize,
            "the header names {scores} score columns but the fold produces \
             {BEATS_PER_BAR}: {cols:?}"
        );

        let frame = AnalysisFrame {
            beat_index: 412,
            bass: 0.812,
            bpm: 128.5,
            time_since_beat: 0.0213,
            ..Default::default()
        };
        let terms = DownbeatTerms {
            scores: [0.7314, 0.2201, 0.7108, 0.1994],
            best: 0,
            held: 0,
            effect_raw: 0.5512,
            null_share: 0.0930,
            effect_corrected: 0.4582,
            beats_seen: 32,
            locked: true,
            fold_beat: 337,
            grid_bar_phase: 0.3125,
        };
        let line = Row {
            frame: &frame,
            terms: &terms,
            unix_ms: 1_755_000_000_123,
        }
        .to_string();
        let row: Vec<String> = line.split('\t').map(str::to_owned).collect();
        assert_eq!(row.len(), cols.len(), "row and header disagree: {line}");
        assert_eq!(field(&row, "beat"), 412.0);
        assert_eq!(field(&row, "s0"), 0.7314);
        assert_eq!(field(&row, "s3"), 0.1994);
        assert_eq!(field(&row, "bpm"), 128.5);
        assert_eq!(field(&row, "time_since_beat"), 0.0213);
        // Milliseconds, unrounded: the deltas between consecutive rows are the
        // inter-detection interval, so losing precision here would cost the one
        // measurement this column was added for.
        assert_eq!(row[index("unix_ms")], "1755000000123");
        assert_eq!(field(&row, "bass"), 0.812);
        // 0/1, so the publish RATE over a run is the mean of this column.
        assert_eq!(field(&row, "locked"), 1.0);
        // The two fold-space columns come off `terms`, not off the frame: 337
        // against the frame's `beat_index` of 412 is the whole point of them.
        assert_eq!(field(&row, "fold_beat"), 337.0);
        assert_eq!(field(&row, "grid_bar_phase"), 0.3125);
    }
}
