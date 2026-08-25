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

/// The share of a window a case may spend on an octave other than its own
/// median's before it counts as flickering rather than merely wrong. Stated
/// rather than zero because the reading is a float comparison on two machines
/// (x86_64 here, aarch64 in CI); every case currently reads exactly 0.
const MAX_JUMP_PCT: f32 = 2.0;

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

    // (2) **Every case is stable** — no meaningful share of any window is spent
    //     on an octave other than the one that case's own median sits on. This
    //     is Phase 2's done-when and it is deliberately not a claim that the
    //     octave is *right*: three trap rungs sit an octave off and stay there.
    //     Before the hold, the off-beat rung at 0.50 — where the two peaks
    //     cross — spent 15 % of its window on the other octave, trading the
    //     estimate back and forth hop by hop.
    for r in ladder.iter().chain(traps.iter()) {
        assert!(
            r.jump_pct <= MAX_JUMP_PCT,
            "truth {:.0} BPM, median {:.1}: {:.0}% of the window read a different octave \
             from the median's, past the stated {MAX_JUMP_PCT}% — the estimate is flickering",
            r.truth,
            r.median,
            r.jump_pct
        );
    }

    // (3) Determinism, pinned inside this run rather than against a stored
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

// ---------------------------------------------------------------------------
// Phase 2 — why the repair is a hold and not an octave rule
// ---------------------------------------------------------------------------

/// Hops of onset envelope the tracker autocorrelates. Restated here rather than
/// exported: if `ENV_HISTORY` ever moves, this file's curve stops being the
/// tracker's curve in an obvious way rather than a subtle one.
const ENV_HISTORY: usize = 384;

/// The correlation at the winning lag's two octave neighbours, as a share of
/// the peak's.
struct Octaves {
    /// Winning lag's BPM.
    bpm: f32,
    /// Share at **twice** the winning lag — the slower reading. Asks "is the
    /// true beat half this rate?"
    halving: Option<f32>,
    /// Share at **half** the winning lag — the faster reading. Asks "is the
    /// true beat twice this rate?"
    doubling: Option<f32>,
}

/// Recompute the tracker's own mean-subtracted autocorrelation from the onset
/// envelope and read it at the winning lag's two octave neighbours.
fn octave_shares(pcm: &[f32]) -> Octaves {
    let hop_sec = HOP_SIZE as f32 / SR as f32;
    let min_lag = (60.0 / (200.0 * hop_sec)).floor() as usize;
    let max_lag = ((60.0 / (60.0 * hop_sec)).ceil() as usize).min(ENV_HISTORY - 1);

    let mut analyzer = Analyzer::new(FORMAT).expect("valid format");
    let hop_samples = HOP_SIZE * FORMAT.channels as usize;
    let mut env = Vec::new();
    for samples in pcm.chunks(hop_samples) {
        analyzer.push_interleaved(samples);
        env.push(analyzer.take_frame().onset_raw);
    }
    let tail: Vec<f32> = env.iter().rev().take(ENV_HISTORY).rev().copied().collect();
    let mean = tail.iter().sum::<f32>() / tail.len() as f32;
    let corr = |lag: usize| -> f32 {
        tail.iter()
            .zip(tail.iter().skip(lag))
            .map(|(x, y)| (x - mean) * (y - mean))
            .sum::<f32>()
    };

    let mut best_lag = min_lag;
    let mut peak = f32::NEG_INFINITY;
    for lag in min_lag..=max_lag {
        let c = corr(lag);
        if c > peak {
            peak = c;
            best_lag = lag;
        }
    }
    let share = |lag: usize| -> Option<f32> {
        if (min_lag..=max_lag).contains(&lag) {
            Some(corr(lag) / peak * 100.0)
        } else {
            None
        }
    };
    Octaves {
        bpm: 60.0 / (best_lag as f32 * hop_sec),
        halving: share(best_lag * 2),
        doubling: share(best_lag / 2),
    }
}

/// Phase 2 — the octave ambiguity is **one-sided**, which is why the repair is
/// a hold rather than an octave-preference rule.
///
/// The plan named three candidate repairs and said it would not pick one blind.
/// This is the reading that picked. An autocorrelation cannot tell a clean click
/// train from material whose accent period is twice its click period, because
/// *every* periodic signal correlates strongly at twice its own lag — so the
/// question "is the true beat slower than this?" has no discriminating evidence
/// in the curve, and a rule that preferred the slower reading takes the fast end
/// of the ladder down an octave. The mirrored question does have evidence: a
/// clean train's correlation at *half* its winning lag is negative, because
/// there is nothing between its clicks, while a half-time feel reads strongly
/// positive there.
///
/// That asymmetry is asserted below as an **overlap** and a **separation**, both
/// comparisons taken inside this run (ADR-0071). It is also why correcting the
/// doubling direction alone was refused: the material such a rule fires on is a
/// 60-100 BPM track with events between the beats, which describes most of the
/// hip-hop in the capture set this plan is repairing.
#[test]
fn the_octave_ambiguity_is_one_sided() {
    println!("\nPlan 0095 Phase 2 - correlation at the winning lag's octave neighbours");
    println!(
        "(as a share of the peak; halving asks whether the beat is slower, doubling whether faster)\n"
    );
    println!(
        "{:<38} {:>12}  {:>14}  {:>16}",
        "case", "winner", "halving (2L)", "doubling (L/2)"
    );

    let cell = |s: Option<f32>| match s {
        Some(v) => format!("{v:.1}%"),
        None => "out of range".to_string(),
    };
    let mut clean_halving: Vec<f32> = Vec::new();
    let mut clean_doubling: Vec<f32> = Vec::new();
    let mut offbeat_halving: Vec<f32> = Vec::new();
    let mut halftime_doubling: Vec<f32> = Vec::new();

    for &bpm in LADDER.iter() {
        let o = octave_shares(&signal::click_track(bpm, CLIP_SECS, FORMAT));
        println!(
            "{:<38} {:>8.1} BPM  {:>14}  {:>16}",
            format!("click train, {bpm:.0} BPM"),
            o.bpm,
            cell(o.halving),
            cell(o.doubling)
        );
        clean_halving.extend(o.halving);
        clean_doubling.extend(o.doubling);
    }
    println!();
    for &offbeat in [0.5f32, 0.8].iter() {
        let o = octave_shares(&signal::offbeat_click_track(
            OFFBEAT_BPM,
            CLIP_SECS,
            offbeat,
            FORMAT,
        ));
        println!(
            "{:<38} {:>8.1} BPM  {:>14}  {:>16}",
            format!("off-beat clicks at {offbeat:.2}, {OFFBEAT_BPM:.0} BPM"),
            o.bpm,
            cell(o.halving),
            cell(o.doubling)
        );
        offbeat_halving.extend(o.halving);
    }
    for &weak in [0.7f32, 0.25].iter() {
        let o = octave_shares(&signal::halftime_click_track(
            HALFTIME_BPM,
            CLIP_SECS,
            weak,
            FORMAT,
        ));
        println!(
            "{:<38} {:>8.1} BPM  {:>14}  {:>16}",
            format!("half-time, beats 2/4 at {weak:.2}, {HALFTIME_BPM:.0} BPM"),
            o.bpm,
            cell(o.halving),
            cell(o.doubling)
        );
        halftime_doubling.extend(o.doubling);
    }
    println!();

    // --- properties, not measurements -------------------------------------
    //
    // (1) **The halving direction does not separate.** The weakest clean rung
    //     reads below the strongest trap rung, so no threshold on this column
    //     can tell "a periodic signal" from "a signal whose accent period is
    //     twice its click period". Both bounds come from this same run.
    let clean_min = clean_halving.iter().copied().fold(f32::INFINITY, f32::min);
    let trap_max = offbeat_halving
        .iter()
        .copied()
        .fold(f32::NEG_INFINITY, f32::max);
    println!(
        "halving: clean rungs read down to {clean_min:.1}% of peak, the off-beat traps up to \
         {trap_max:.1}% - the ranges overlap, so no threshold separates them"
    );
    assert!(
        clean_min < trap_max,
        "the clean floor ({clean_min:.1}%) should sit below the trap ceiling ({trap_max:.1}%): \
         if these ever separate, an octave-preference rule becomes possible and this test is \
         the reason to revisit it"
    );

    // (2) **The doubling direction does separate**, and by a wide margin: a
    //     clean train has nothing between its clicks, so the correlation there
    //     is negative rather than merely small. Separable and refused anyway —
    //     the doc comment above says why.
    let clean_max = clean_doubling
        .iter()
        .copied()
        .fold(f32::NEG_INFINITY, f32::max);
    let trap_min = halftime_doubling
        .iter()
        .copied()
        .fold(f32::INFINITY, f32::min);
    println!(
        "doubling: clean rungs read up to {clean_max:.1}% of peak, the half-time traps down to \
         {trap_min:.1}% - separable, and refused anyway\n"
    );
    assert!(
        clean_max < 0.0,
        "a clean click train has nothing between its clicks, so the correlation at half its \
         winning lag should be negative, not {clean_max:.1}% of peak"
    );
    assert!(
        trap_min > clean_max,
        "the half-time traps ({trap_min:.1}%) should read above the clean rungs ({clean_max:.1}%)"
    );
}
