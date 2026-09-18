//! Deterministic tempo (BPM) and beat-phase (`bar`) from the onset envelope.
//!
//! The time base is the analyzer's hop count, never the wall clock: BPM is the
//! lag of the strongest mean-subtracted autocorrelation of the recent onset
//! envelope (parabolically refined for sub-hop precision, and **held against
//! challengers** so two near-tied peaks cannot trade it hop by hop — Plan 0095
//! Phase 2, see `TempoTracker::hold`), and `bar` is a 0..1
//! phase advanced each hop by the current BPM and snapped to 0 on every
//! detected beat. Pure and allocation-free after construction — the envelope
//! history is a fixed array and every pass is iterator-based, so the same
//! `(onset, beat)` sequence always yields the same `(bpm, bar)` sequence
//! (NFR 6, hot-path discipline §5).

// Hot-path panic-denial pragma (Plan 0002 Phase 2). Runs every analysis hop.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

use super::HOP_SIZE;

/// Onset-envelope history (~4.1 s at a 10.7 ms hop): long enough to resolve
/// tempos down to `MIN_BPM` with several beat periods of overlap.
const ENV_HISTORY: usize = 384;
/// Tempo search range. Sub/‑super-harmonics outside this are ignored.
const MIN_BPM: f32 = 60.0;
const MAX_BPM: f32 = 200.0;

/// Bottom of the octave [`fold`] reports in, inclusive.
pub const FOLD_MIN_BPM: f32 = 70.0;

/// Top of that octave, exclusive. **Exactly twice [`FOLD_MIN_BPM`]**, which is
/// what makes the fold total: halving until below the top leaves a value under
/// it, and doubling until at or above the bottom cannot then pass it, so every
/// positive input lands in the window in at most a few steps.
pub const FOLD_MAX_BPM: f32 = 2.0 * FOLD_MIN_BPM;

/// `bpm` moved by whole octaves into `[FOLD_MIN_BPM, FOLD_MAX_BPM)`; `0` for a
/// cold or non-finite estimate.
///
/// **This settles the octave by fiat, and that is the whole of what it does.**
/// The estimator declines to settle it because the autocorrelation carries no
/// evidence either way (see [`TempoTracker::hold`]), and nothing here supplies
/// that evidence — it picks one octave so that every consumer picks the *same*
/// one. The result is a power-of-two multiple of the input and never a musical
/// claim: a piece genuinely at 60 BPM folds to 120, and a consumer counting
/// beats from it counts two per bar-line.
///
/// It is published beside [`BeatClock::bpm`] and never instead of it, because
/// presets and the OSC contract are bound to the raw estimate.
pub fn fold(bpm: f32) -> f32 {
    if !bpm.is_finite() || bpm <= 0.0 {
        return 0.0;
    }
    let mut folded = bpm;
    // Each iteration strictly halves or doubles a positive finite value, so both
    // loops terminate; the estimator's own range needs at most one step of each.
    while folded >= FOLD_MAX_BPM {
        folded *= 0.5;
    }
    while folded < FOLD_MIN_BPM {
        folded *= 2.0;
    }
    folded
}

/// A challenging lag must lead the held one's correlation by this fraction
/// before it even starts counting toward a switch, so two near-tied peaks
/// cannot trade the estimate back and forth hop by hop. Named and sized after
/// [`downbeat`](super::downbeat)'s alignment switch, which solves the same
/// problem one layer up.
const SWITCH_MARGIN: f32 = 0.15;

/// Consecutive hops a leading challenger must hold before the estimate moves —
/// ~0.5 s at a 10.7 ms hop. The envelope history is 4.1 s, so a real tempo
/// change arrives gradually as the window fills and leads for far longer than
/// this; what it excludes is the flicker at a tie.
const SWITCH_HOPS: u32 = 48;

/// Rolling onset-envelope autocorrelator producing a BPM estimate and a beat
/// phase.
pub struct TempoTracker {
    /// Seconds per hop — the fixed conversion between lag (hops) and BPM.
    hop_sec: f32,
    /// Envelope history, oldest at index 0, newest at the end.
    env: [f32; ENV_HISTORY],
    /// Hops seen so far, saturating at `ENV_HISTORY` (estimation waits until
    /// the buffer is full so the autocorrelation has full context).
    filled: usize,
    /// Lag search bounds (hops) derived from `MAX_BPM`/`MIN_BPM`.
    min_lag: usize,
    max_lag: usize,
    /// Latest BPM estimate (0 until warm / when no periodicity is found).
    bpm: f32,
    /// The lag the estimate is currently published at; `0` until the first
    /// positive periodicity is found. See [`TempoTracker::hold`].
    held_lag: usize,
    /// A challenging lag and how many consecutive hops it has led for.
    challenger_lag: usize,
    challenger_hops: u32,
    /// Beat phase in [0, 1): 0 at each beat, ramping toward the next.
    phase: f32,
    /// Beats detected so far. `beat_index` publishes this less one, so the first
    /// detected beat reads 0 and [`Layer 2`](super::downbeat)'s counter fallback
    /// starts its bar on a beat rather than a beat and a bit.
    beats_seen: u32,
    /// Hops since the last detected beat, the integer `time_since_beat` is
    /// derived from. Counted in hops rather than accumulated in seconds so it
    /// cannot drift: the hop clock is the only clock here (NFR section 6).
    hops_since_beat: u32,
}

/// One hop's beat-clock reading (ADR-0050 Layer 1 plus the pre-existing tempo
/// pair), returned together because they all derive from the same beat stream.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct BeatClock {
    /// Tempo estimate in BPM; 0 until the tracker warms.
    pub bpm: f32,
    /// [`bpm`](Self::bpm) folded into `[FOLD_MIN_BPM, FOLD_MAX_BPM)`, or 0 while
    /// the tracker is cold. Beside the estimate, never instead of it.
    pub bpm_folded: f32,
    /// Beat phase in [0, 1) — the shipped `bar` variable, whose name is a
    /// documented misnomer (ADR-0050).
    pub bar: f32,
    /// Monotone count of beats seen, starting at 0 on the first one.
    pub beat_index: u32,
    /// Seconds since the last detected beat; exactly 0 on a beat hop.
    pub time_since_beat: f32,
}

impl TempoTracker {
    /// Build a tracker for `sample_rate`, precomputing the lag search bounds.
    pub fn new(sample_rate: u32) -> Self {
        let hop_sec = HOP_SIZE as f32 / sample_rate as f32;
        // lag_hops = 60 / (bpm * hop_sec); faster tempo => shorter lag.
        let min_lag = (60.0 / (MAX_BPM * hop_sec)).floor() as usize;
        let max_lag = ((60.0 / (MIN_BPM * hop_sec)).ceil() as usize).min(ENV_HISTORY - 1);
        Self {
            hop_sec,
            env: [0.0; ENV_HISTORY],
            filled: 0,
            min_lag: min_lag.max(1),
            max_lag,
            bpm: 0.0,
            held_lag: 0,
            challenger_lag: 0,
            challenger_hops: 0,
            phase: 0.0,
            beats_seen: 0,
            hops_since_beat: 0,
        }
    }

    /// Advance one hop and return the whole beat clock.
    pub fn process(&mut self, onset: f32, beat: bool) -> BeatClock {
        // Slide the newest onset into the tail (oldest falls off the front).
        self.env.copy_within(1.., 0);
        if let Some(last) = self.env.last_mut() {
            *last = onset;
        }
        self.filled = (self.filled + 1).min(ENV_HISTORY);

        if self.filled >= ENV_HISTORY {
            self.bpm = self.estimate_bpm();
        }

        // Beat phase: hard-reset on a detected beat so the ramp stays locked
        // to the music; otherwise advance by the current tempo.
        if beat {
            self.phase = 0.0;
            self.beats_seen = self.beats_seen.saturating_add(1);
            self.hops_since_beat = 0;
        } else {
            if self.bpm > 0.0 {
                self.phase += self.bpm * self.hop_sec / 60.0;
                self.phase -= self.phase.floor(); // wrap into [0, 1)
            }
            self.hops_since_beat = self.hops_since_beat.saturating_add(1);
        }

        BeatClock {
            bpm: self.bpm,
            bpm_folded: fold(self.bpm),
            bar: self.phase,
            beat_index: self.beats_seen.saturating_sub(1),
            time_since_beat: self.hops_since_beat as f32 * self.hop_sec,
        }
    }

    /// Lag of the strongest mean-subtracted autocorrelation peak in the search
    /// range, held against beat-to-beat challengers, refined to sub-hop
    /// precision and converted to BPM. Keeps the last estimate if no positive
    /// periodicity is present.
    fn estimate_bpm(&mut self) -> f32 {
        let mean = self.env.iter().sum::<f32>() / ENV_HISTORY as f32;

        let mut best_lag = self.min_lag;
        let mut best = f32::NEG_INFINITY;
        for lag in self.min_lag..=self.max_lag {
            let c = self.corr_at(lag, mean);
            if c > best {
                best = c;
                best_lag = lag;
            }
        }
        if best <= 0.0 {
            return self.bpm;
        }

        let lag = self.hold(best_lag, best, mean);
        60.0 / (self.refine(lag, mean) * self.hop_sec)
    }

    /// Which lag the estimate actually publishes: the argmax once it has led the
    /// incumbent by a margin for long enough, and the incumbent until then.
    ///
    /// **This does not settle the octave, and it is not trying to** (Plan 0095
    /// Phase 1). The probe measured both directions of the ambiguity on
    /// synthesized clips with known truth — the numbers below are what
    /// `the_octave_ambiguity_is_one_sided` prints, in `core/tests/suite/tempo_probe.rs`,
    /// which is where to re-read them rather than trusting this comment: a clean
    /// click train's correlation at *twice* the winning lag reads 80.0-88.5 % of
    /// the peak — a plain property of any periodic signal — against 75.2-90.7 %
    /// for material whose accent period really is twice the click period, so the
    /// two overlap and no threshold separates them, and a rule that preferred the
    /// slower reading dragged the 140, 165 and 200 BPM rungs down an octave. That
    /// overlap is asserted there, not just printed. What is separable is
    /// *stability*:
    /// the estimator recomputes an argmax from scratch every hop and has no
    /// memory, so two near-tied peaks make it flicker hop to hop (measured at
    /// 15 % of the window on the off-beat rung where the two peaks cross). A
    /// margin plus a hold turns a flickering answer into a stable one, which is
    /// the property a bar grid needs from it (ADR-0109).
    fn hold(&mut self, best_lag: usize, best: f32, mean: f32) -> usize {
        // Cold start, or a held lag left stale by a rebuild of the bounds.
        if self.held_lag < self.min_lag || self.held_lag > self.max_lag {
            self.held_lag = best_lag;
            self.clear_challenger();
            return best_lag;
        }
        // Adjacent lags are the same answer drifting, not a challenger: follow
        // them, so a slowly-moving tempo is tracked rather than resisted.
        if best_lag.abs_diff(self.held_lag) <= 1 {
            self.held_lag = best_lag;
            self.clear_challenger();
            return best_lag;
        }
        // A challenger has to lead by a real margin before it starts counting,
        // and then hold that lead for a stretch rather than a hop.
        let incumbent = self.corr_at(self.held_lag, mean);
        if best <= incumbent * (1.0 + SWITCH_MARGIN) {
            self.clear_challenger();
            return self.held_lag;
        }
        let hops = if self.challenger_lag.abs_diff(best_lag) <= 1 {
            self.challenger_hops.saturating_add(1)
        } else {
            1
        };
        self.challenger_lag = best_lag;
        if hops >= SWITCH_HOPS {
            self.held_lag = best_lag;
            self.clear_challenger();
            best_lag
        } else {
            self.challenger_hops = hops;
            self.held_lag
        }
    }

    fn clear_challenger(&mut self) {
        self.challenger_lag = 0;
        self.challenger_hops = 0;
    }

    /// Parabolic interpolation across `lag`'s neighbors for sub-hop precision
    /// (keeps the estimate off the coarse integer-lag grid).
    fn refine(&self, lag: usize, mean: f32) -> f32 {
        if lag > self.min_lag && lag < self.max_lag {
            let y = self.corr_at(lag, mean);
            let yl = self.corr_at(lag - 1, mean);
            let yr = self.corr_at(lag + 1, mean);
            let denom = yl - 2.0 * y + yr;
            let delta = if denom.abs() > f32::EPSILON {
                (0.5 * (yl - yr) / denom).clamp(-0.5, 0.5)
            } else {
                0.0
            };
            lag as f32 + delta
        } else {
            lag as f32
        }
    }

    /// Mean-subtracted autocorrelation at `lag`, iterator-based (no indexing).
    fn corr_at(&self, lag: usize, mean: f32) -> f32 {
        self.env
            .iter()
            .zip(self.env.iter().skip(lag))
            .map(|(x, y)| (x - mean) * (y - mean))
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsp::HOP_SIZE;

    /// The fastest lag the search reaches at 48 kHz, and the BPM it reports
    /// there.
    ///
    /// `min_lag` is `floor(60 / (MAX_BPM * hop_sec))`, and the floor is what
    /// makes the wall sit *above* `MAX_BPM`: 28 hops rather than 28.125, which
    /// reads 200.89 BPM. `refine` leaves it alone because it declines to
    /// interpolate at either end of the range, so the wall is reported as that
    /// one exact repeated value rather than as a neighbourhood around it.
    #[test]
    fn the_top_of_the_search_range_is_a_wall_at_200_9_bpm() {
        let hop_sec = HOP_SIZE as f32 / 48_000.0;
        let min_lag = (60.0 / (MAX_BPM * hop_sec)).floor();
        assert_eq!(min_lag, 28.0);
        let at_the_wall = 60.0 / (min_lag * hop_sec);
        assert!(
            (at_the_wall - 200.9).abs() < 0.05,
            "the fastest searchable lag reads {at_the_wall:.2} BPM"
        );
        assert!(
            at_the_wall > MAX_BPM,
            "the wall sits above MAX_BPM, so a reading over {MAX_BPM} is the range's \
             edge and not an estimate past it"
        );
    }

    /// Every octave of one rate folds to the same answer, and that answer is in
    /// the window.
    #[test]
    fn the_fold_lands_in_its_window_and_is_octave_invariant() {
        for base in [70.0f32, 99.5, 100.45, 120.0, 139.9] {
            let folded = fold(base);
            assert!(
                (folded - base).abs() < 1e-3,
                "{base} is already in the window and should not move, got {folded}"
            );
            for octave in [0.25f32, 0.5, 2.0, 4.0] {
                let shifted = fold(base * octave);
                assert!(
                    (shifted - base).abs() < 1e-2,
                    "{base} BPM at {octave}x folded to {shifted}, not back to {base}"
                );
            }
        }
    }

    /// The window is closed at the bottom and open at the top, over the whole
    /// range the estimator can report plus an octave either side of it.
    #[test]
    fn the_fold_covers_the_estimators_whole_range() {
        let hop_sec = HOP_SIZE as f32 / 48_000.0;
        let wall = 60.0 / ((60.0 / (MAX_BPM * hop_sec)).floor() * hop_sec);
        for step in 0..=400 {
            let bpm = MIN_BPM + (wall - MIN_BPM) * step as f32 / 400.0;
            let folded = fold(bpm);
            assert!(
                (FOLD_MIN_BPM..FOLD_MAX_BPM).contains(&folded),
                "{bpm} BPM folded to {folded}, outside [{FOLD_MIN_BPM}, {FOLD_MAX_BPM})"
            );
            // A power-of-two multiple, asserted rather than assumed: the ratio's
            // base-2 logarithm must be a whole number.
            let octaves = (bpm / folded).log2();
            assert!(
                (octaves - octaves.round()).abs() < 1e-4,
                "{bpm} BPM to {folded} is {octaves} octaves, which is not a whole one"
            );
        }
    }

    /// A cold or nonsense estimate folds to zero rather than looping.
    #[test]
    fn a_cold_estimate_folds_to_zero() {
        assert_eq!(fold(0.0), 0.0);
        assert_eq!(fold(-120.0), 0.0);
        assert_eq!(fold(f32::NAN), 0.0);
        assert_eq!(fold(f32::INFINITY), 0.0);
    }

    /// The published clock carries the fold beside the estimate rather than
    /// instead of it.
    #[test]
    fn the_clock_publishes_both_tempos() {
        let mut tracker = TempoTracker::new(48_000);
        let clock = tracker.process(0.0, false);
        assert_eq!(clock.bpm, 0.0);
        assert_eq!(clock.bpm_folded, 0.0, "a cold tracker folds nothing");
    }
}
