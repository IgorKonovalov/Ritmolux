//! Plan 0095 Phase 1 — the tempo estimate, measured before it is touched.
//!
//! [ADR-0109] gives Layer 2 its own bar grid, built from the autocorrelated
//! tempo. A grid is only as good as the rate under it, and the live captures
//! ([Plan 0086] Phase 2) read a p10 of 64.0 against a 128.0 median and a p90 of
//! 200.9 against a 100.2 median — both at the `MIN_BPM`/`MAX_BPM` search bounds.
//! That is a *field* reading on material with no ground truth. This file is the
//! bench reading: synthesized clips at **known** tempos through the real
//! [`Analyzer`], printing the estimate against the truth.
//!
//! **The deliverable is the printed table, not the pass/fail.** Phase 2 chooses
//! its repair against these numbers, so what matters is which rungs are wrong
//! and in what way. To read it:
//!
//! ```text
//! cargo nextest run -p lmv-core --test tempo_probe --no-capture
//! cargo test -p lmv-core --test tempo_probe -- --nocapture
//! ```
//!
//! (Unlike `downbeat_probe.rs` this file carries no `.config/nextest.toml`
//! override, so a passing run hides the table unless it is asked for.)
//!
//! Per [ADR-0071] every number printed here is a **measurement** and every
//! number asserted is a **property**. The one absolute tolerance below is
//! stated, not discovered: the plan's done-when asks for the estimate to land
//! within a stated tolerance of the truth *or of an exact octave of it*, and
//! **the octave is its own column** rather than folded into the error — an
//! estimator that reports half the true tempo is making a different mistake
//! from one that reports 0.94x of it, and a single error column cannot say
//! which.
//!
//! [ADR-0109]: ../../docs/adrs/0109-the-beat-clock-counts-onsets-not-beats.md
//! [ADR-0071]: ../../docs/adrs/0071-a-numeric-test-contract-states-a-property-or-names-its-machine.md
//! [Plan 0086]: ../../docs/plans/done/0086-the-downbeat-finds-a-cue-that-is-not-the-kick.md

use lmv_core::audio::AudioFormat;
use lmv_core::dsp::{Analyzer, HOP_SIZE};
use lmv_core::signal;

const SR: u32 = 48_000;

const FORMAT: AudioFormat = AudioFormat {
    sample_rate: SR,
    channels: 1,
};

/// Clip length. The tracker's envelope history is 384 hops (~4.1 s) and it
/// declines to estimate until that is full, so a clip has to be several times
/// that before "the estimate over the clip" means anything.
const CLIP_SECS: f32 = 14.0;

/// Readings start here — past the ~4.3 s of warmup with margin, so every number
/// below comes from a tracker whose history holds nothing but the clip.
const MEASURE_FROM_SECS: f32 = 6.0;

/// The stated tolerance, as a percentage of the octave-folded truth. Wide
/// enough that the coarse lag grid at the slow end (a 60 BPM beat is 94 hops,
/// and the parabolic refinement is skipped at the search bound) is not a
/// finding, narrow enough that a dotted or triplet misread is.
const TOL_PCT: f32 = 4.0;

/// The tempo ladder: both search bounds and six rungs between them.
const LADDER: [f32; 8] = [60.0, 75.0, 90.0, 105.0, 120.0, 140.0, 165.0, 200.0];

/// The double-time trap's base tempo. 90 rather than 120 so that the octave
/// above (180) is **inside** the 60-200 search range — an estimator cannot make
/// an octave error the search bounds already forbid, and a case that cannot
/// fail is not a case.
const OFFBEAT_BPM: f32 = 90.0;

/// The half-time trap's base tempo, chosen the same way: 75 is inside the range.
const HALFTIME_BPM: f32 = 150.0;

/// What one clip reads over the measurement window.
struct Reading {
    /// Notated tempo of the stimulus.
    truth: f32,
    p10: f32,
    median: f32,
    p90: f32,
    /// Octave the median sits on relative to the truth, as a signed power of
    /// two: `0` is the true octave, `-1` half-time, `+1` double-time.
    octave: i32,
    /// Error of the median against the truth folded onto `octave`, in percent.
    /// **Not** the error against the truth: the octave column carries that.
    err_pct: f32,
    /// Share of the window whose reading sat on a different octave from the
    /// median's — the within-clip instability the field capture saw as a p10
    /// and p90 at the search bounds.
    jump_pct: f32,
}

/// Drive `pcm` through a real analyzer and collect the BPM series over the
/// measurement window.
fn bpm_series(pcm: &[f32]) -> Vec<f32> {
    let mut analyzer = Analyzer::new(FORMAT).expect("valid format");
    let hop_samples = HOP_SIZE * FORMAT.channels as usize;
    let from_hop = (MEASURE_FROM_SECS * SR as f32 / HOP_SIZE as f32) as usize;
    let mut series = Vec::new();
    for (hop, samples) in pcm.chunks(hop_samples).enumerate() {
        analyzer.push_interleaved(samples);
        let frame = analyzer.take_frame();
        if hop >= from_hop {
            series.push(frame.bpm);
        }
    }
    series
}

/// Nearest-rank percentile of an already-sorted slice.
fn percentile(sorted: &[f32], q: f32) -> f32 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = ((q * sorted.len() as f32).ceil() as usize).saturating_sub(1);
    sorted
        .get(idx.min(sorted.len() - 1))
        .copied()
        .unwrap_or(0.0)
}

/// Which octave of `truth` an estimate sits on, as a signed power of two.
fn octave_of(estimate: f32, truth: f32) -> i32 {
    if estimate <= 0.0 || truth <= 0.0 {
        return 0;
    }
    (estimate / truth).log2().round() as i32
}

fn read(pcm: &[f32], truth: f32) -> Reading {
    let series = bpm_series(pcm);
    let mut sorted = series.clone();
    sorted.sort_by(f32::total_cmp);
    let median = percentile(&sorted, 0.5);
    let octave = octave_of(median, truth);
    let folded = truth * 2f32.powi(octave);
    let jumps = series
        .iter()
        .filter(|&&b| octave_of(b, truth) != octave)
        .count();
    Reading {
        truth,
        p10: percentile(&sorted, 0.10),
        median,
        p90: percentile(&sorted, 0.90),
        octave,
        err_pct: (median - folded) / folded * 100.0,
        jump_pct: jumps as f32 / series.len().max(1) as f32 * 100.0,
    }
}

fn header() -> String {
    format!(
        "{:<38} {:>7}  {:>7} {:>7} {:>7}  {:>6}  {:>8}  {:>8}",
        "case", "truth", "p10", "median", "p90", "octave", "err", "oct-jump"
    )
}

fn row(label: &str, r: &Reading) -> String {
    format!(
        "{:<38} {:>7.1}  {:>7.1} {:>7.1} {:>7.1}  {:>6}  {:>7.1}% {:>8.0}%",
        label,
        r.truth,
        r.p10,
        r.median,
        r.p90,
        match r.octave {
            0 => "x1".to_string(),
            n if n > 0 => format!("x{}", 1 << n),
            n => format!("/{}", 1 << -n),
        },
        r.err_pct,
        r.jump_pct
    )
}

/// Phase 1 — the tempo estimate against known truth.
///
/// The ladder is the unambiguous half: a metronome click train has exactly one
/// periodicity and the only question is whether the estimator finds it and on
/// which octave. The two traps are the ambiguous half, and they are the cases
/// the live captures were full of — an arrangement with strong off-beat energy
/// reads as double-time to an autocorrelator, and a half-time feel reads as
/// half. Both are printed at three strengths so the table shows a gradient rather
/// than a verdict.
#[test]
fn the_tempo_estimate_is_measured_against_known_truth() {
    println!("\nPlan 0095 Phase 1 - the tempo estimate against known truth");
    println!(
        "({CLIP_SECS} s clips at {SR} Hz, read from {MEASURE_FROM_SECS} s; \
         search range 60-200 BPM; err is against the truth folded onto the octave column)\n"
    );
    println!("{}", header());

    let mut ladder = Vec::new();
    for &bpm in LADDER.iter() {
        let pcm = signal::click_track(bpm, CLIP_SECS, FORMAT);
        let r = read(&pcm, bpm);
        println!("{}", row(&format!("click train, {bpm:.0} BPM"), &r));
        ladder.push(r);
    }

    println!();
    let mut traps = Vec::new();
    for &offbeat in [0.3f32, 0.5, 0.8].iter() {
        let pcm = signal::offbeat_click_track(OFFBEAT_BPM, CLIP_SECS, offbeat, FORMAT);
        let r = read(&pcm, OFFBEAT_BPM);
        println!(
            "{}",
            row(
                &format!("off-beat clicks at {offbeat:.2}, {OFFBEAT_BPM:.0} BPM"),
                &r
            )
        );
        traps.push(r);
    }
    for &weak in [0.7f32, 0.5, 0.25].iter() {
        let pcm = signal::halftime_click_track(HALFTIME_BPM, CLIP_SECS, weak, FORMAT);
        let r = read(&pcm, HALFTIME_BPM);
        println!(
            "{}",
            row(
                &format!("half-time feel, beats 2/4 at {weak:.2}, {HALFTIME_BPM:.0} BPM"),
                &r
            )
        );
        traps.push(r);
    }
    println!();

    // --- properties, not measurements -------------------------------------
    //
    // (1) The unambiguous ladder lands on *some* octave of the truth, within a
    //     stated tolerance. This is the plan's done-when, and the split into
    //     two columns is what makes it meaningful: an estimator reading 59.8
    //     for 60 is refining correctly on the coarse lag grid, and one reading
    //     30 is on the wrong octave — the second is Phase 2's problem and the
    //     first is not a problem at all.
    let wrong_octave = ladder.iter().filter(|r| r.octave != 0).count();
    println!(
        "ladder: {}/{} rungs on the true octave, worst |err| {:.1}% of the folded truth",
        LADDER.len() - wrong_octave,
        LADDER.len(),
        ladder
            .iter()
            .map(|r| r.err_pct.abs())
            .fold(0.0f32, f32::max)
    );
    for r in &ladder {
        assert!(
            r.err_pct.abs() <= TOL_PCT,
            "click train at {:.0} BPM: median {:.1} is {:.1}% off the truth folded onto \
             its own octave ({:.1}), past the stated {TOL_PCT}% tolerance",
            r.truth,
            r.median,
            r.err_pct,
            r.truth * 2f32.powi(r.octave)
        );
    }

    // (2) Determinism, pinned inside this run rather than against a stored
    //     number: the same clip fed twice produces a bit-identical series. Phase
    //     2 changes this estimator, and a repair that made it path-dependent
    //     would be a hot-path regression no visual could reveal.
    let pcm = signal::click_track(120.0, CLIP_SECS, FORMAT);
    let first = bpm_series(&pcm);
    let second = bpm_series(&pcm);
    assert_eq!(first.len(), second.len(), "same clip, same hop count");
    for (i, (a, b)) in first.iter().zip(second.iter()).enumerate() {
        assert_eq!(
            a.to_bits(),
            b.to_bits(),
            "hop {i}: the same clip must estimate bit-identically ({a} then {b})"
        );
    }
    println!(
        "determinism: {} hops of the 120 BPM clip, bit-identical across two runs\n",
        first.len()
    );
}
