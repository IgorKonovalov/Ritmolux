//! Downbeat estimation: which beat is beat 1 (ADR-0050 Layer 2).
//!
//! No preset can build an 8-bar arc or land a drop without knowing where the bar
//! starts. Estimating that on real music has genuine failure modes —
//! syncopation, half-time feel, non-4/4 — and **a wrong downbeat is worse than
//! none**: a preset accenting beat 1 on beat 3 reads as broken in a way a plain
//! counter never does. So the aggressive capability is kept and its failure mode
//! is made the conservative one.
//!
//! The method: accumulate a bass-weighted accent per detected beat, fold that
//! history over the four candidate beat-1 alignments of a 4/4 hypothesis, and
//! take the strongest. Confidence is **how much of the accent variation the
//! alignment explains**, corrected for the share noise alone would explain, so a
//! pattern with no accent structure scores near zero rather than picking a winner
//! by coincidence. Below a threshold the estimator does not publish, and switching
//! alignment mid-stream is hysteretic so the bar line cannot hop beat to beat.
//!
//! **The gate chooses an alignment, nothing else.** Locked and unlocked output
//! come from one formula, differing only in whether the alignment is the
//! estimate or `0` — so the fallback is `beat_index`-derived counters by
//! construction, not by a parallel code path that could drift. That is what
//! makes "worst case, it behaves exactly like the counters" a structural claim.
//!
//! 4/4 is assumed and documented. Pure and allocation-free after construction:
//! state is fixed arrays and the only clock is the beat stream (NFR section 6).

// Hot-path panic-denial pragma (Plan 0002 Phase 2). Runs every analysis hop.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

/// Beats per bar. 4/4 is assumed (ADR-0050); a different meter falls back to the
/// counters rather than mis-accenting.
pub const BEATS_PER_BAR: u32 = 4;

/// Beats of accent history the fold runs over — eight bars, so each of the four
/// alignments averages eight observations. Long enough to be evidence, short
/// enough that "recent past" still means something.
const ACCENT_HISTORY: usize = 32;

/// Beats of evidence required before the estimator will publish at all. Two
/// bars: below this every alignment has one or two samples. The confidence
/// measure already discounts small-sample coincidence, so this is a floor rather
/// than the main defence.
const MIN_BEATS: usize = 8;

/// Confidence the effect size must clear to publish. Measured against the
/// synthesized patterns in this module's tests and against real accented audio in
/// `core/tests/dsp.rs`: a clean kick every fourth beat scores well above this, an
/// unaccented click train scores near zero.
const CONFIDENCE_THRESHOLD: f32 = 0.25;

/// A challenger must lead the incumbent by this fraction before it even starts
/// counting toward a switch, so ordinary beat-to-beat wobble never begins one.
const SWITCH_MARGIN: f32 = 0.15;

/// Consecutive beats a leading challenger must hold before the alignment moves —
/// three bars. This is the "flips take bars, not frames" requirement.
const HYSTERESIS_BEATS: u32 = 12;

/// How much the bass band contributes to an accent relative to broadband flux.
///
/// Both inputs are ADR-0049-normalized to 0..1, which is what makes a weighted
/// blend of them meaningful at all — on raw magnitudes bass outweighs flux about
/// twentyfold and the weight would be doing nothing. Bass-dominant because the
/// kick is the downbeat cue in most material this targets.
const BASS_WEIGHT: f32 = 0.7;

/// Bar-position output for one hop.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct BarClock {
    /// Which beat of the bar this is, `0..BEATS_PER_BAR`.
    pub beat_in_bar: u32,
    /// Bar counter — **monotone except across an alignment change**, since it is
    /// `(beat_index - alignment) / BEATS_PER_BAR` and `alignment` moves when the
    /// estimator locks, drops back, or is overtaken by a challenger. See
    /// `bar_index_steps_back_across_an_alignment_change`, which pins the size of
    /// that step at one bar.
    pub bar_index: u32,
    /// Position across the bar in `[0, 1)`, including the fraction through the
    /// current beat — the true "bar phase" the shipped `bar` variable is not.
    pub bar_phase: f32,
    /// Alignment confidence in `0..1` — the noise-corrected share of accent
    /// variation the alignment explains (see `effect_size`). **Diagnostics only**,
    /// deliberately not a grammar variable: authors get behavior, not homework.
    pub confidence: f32,
    /// Whether the published position came from the estimator (`true`) or the
    /// deterministic counter fallback (`false`).
    pub locked: bool,
}

/// A read-only decomposition of what the estimator currently believes
/// (Plan 0068's instrument).
///
/// The 1 Hz diagnostic column publishes the *outcome* — locked or not, and one
/// confidence number — which is why "the accent feature is weak", "eight bars is
/// the wrong window" and "the confidence measure under-reports" are three
/// stories that fit the same reading. This is the decomposition that tells them
/// apart.
///
/// **Diagnostics only.** It is not a grammar variable, not on the C ABI, and
/// nothing on the analysis path reads it — [`DownbeatTracker::terms`] recomputes
/// from state that already exists rather than caching anything, so the estimator
/// behaves identically whether or not anyone is looking.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct DownbeatTerms {
    /// Mean accent per candidate beat-1 alignment — the fold's own output.
    pub scores: [f32; BEATS_PER_BAR as usize],
    /// The alignment the fold currently favours (first `argmax` of `scores`).
    pub best: u32,
    /// The alignment actually held, which lags `best` by the hysteresis.
    pub held: u32,
    /// Between-group share of accent variance, **before** the noise correction.
    pub effect_raw: f32,
    /// The share four groups would explain by chance alone at this history
    /// length — what `effect_raw` is discounted by.
    pub null_share: f32,
    /// What the gate compares: `effect_raw` corrected for `null_share`.
    pub effect_corrected: f32,
    /// Accents recorded, against `MIN_BEATS` and saturating at `ACCENT_HISTORY`.
    pub beats_seen: u32,
    /// Whether these terms publish — the evidence floor **and** the gate.
    pub locked: bool,
}

/// The three numbers behind one confidence reading. Split out so the probe can
/// see the correction rather than only its result; `effect_size` is still the
/// one caller on the analysis path and still returns a single `f32`.
#[derive(Debug, Clone, Copy)]
struct Effect {
    raw: f32,
    null: f32,
    corrected: f32,
}

/// Folds per-beat accents over the four 4/4 alignments and publishes a bar
/// position when the winner is convincing enough.
pub struct DownbeatTracker {
    /// `beat_index % BEATS_PER_BAR` for each recorded accent.
    phases: [u32; ACCENT_HISTORY],
    /// Accent strength for each recorded accent, same order.
    values: [f32; ACCENT_HISTORY],
    /// Accents recorded, saturating at `ACCENT_HISTORY`.
    filled: usize,
    /// Write cursor into the ring.
    cursor: usize,
    /// The alignment currently held: which `beat_index % BEATS_PER_BAR` is beat 1.
    alignment: u32,
    /// Latest confidence in `alignment` — see `effect_size`.
    confidence: f32,
    /// A challenging alignment and how many consecutive beats it has led for.
    challenger: Option<(u32, u32)>,
}

impl DownbeatTracker {
    /// A tracker with no evidence: unlocked, aligned to 0, so it reproduces the
    /// plain counters until it has reason not to.
    pub fn new() -> Self {
        Self {
            phases: [0; ACCENT_HISTORY],
            values: [0.0; ACCENT_HISTORY],
            filled: 0,
            cursor: 0,
            alignment: 0,
            confidence: 0.0,
            challenger: None,
        }
    }

    /// Advance one hop.
    ///
    /// `beat` and `beat_index` come from the beat clock; `bass` and `onset` are
    /// the **normalized** levels; `beat_phase` is the `0..1` position through the
    /// current beat, which becomes the sub-beat part of `bar_phase`.
    pub fn process(
        &mut self,
        beat: bool,
        beat_index: u32,
        bass: f32,
        onset: f32,
        beat_phase: f32,
    ) -> BarClock {
        if beat {
            self.record(beat_index, accent(bass, onset));
            self.reconsider();
        }

        let locked = self.filled >= MIN_BEATS && self.confidence >= CONFIDENCE_THRESHOLD;
        // One formula for both paths: the gate only decides whether the alignment
        // is the estimate or zero.
        let alignment = if locked { self.alignment } else { 0 };
        let shifted = beat_index.saturating_sub(alignment);
        let beat_in_bar = shifted % BEATS_PER_BAR;
        let phase = beat_phase.clamp(0.0, 1.0);

        BarClock {
            beat_in_bar,
            bar_index: shifted / BEATS_PER_BAR,
            bar_phase: (beat_in_bar as f32 + phase) / BEATS_PER_BAR as f32,
            confidence: self.confidence,
            locked,
        }
    }

    /// Read the terms behind the current estimate — Plan 0068's instrument.
    ///
    /// **Changes nothing.** It takes `&self`, recomputes the fold and the effect
    /// size from state `process` already keeps, and returns them by value in
    /// fixed-size arrays: no heap allocation, no clock, no field written, and no
    /// branch inside `process` that differs depending on whether anyone calls
    /// this. The recomputation is a pure function of the same state that
    /// produced the last published `BarClock::confidence`, so between hops the
    /// two agree bit for bit — which is what makes this a reading of the gate
    /// rather than a second opinion about it.
    pub fn terms(&self) -> DownbeatTerms {
        let scores = self.scores();
        let mut best = 0u32;
        let mut best_score = f32::NEG_INFINITY;
        for (a, &s) in scores.iter().enumerate() {
            if s > best_score {
                best_score = s;
                best = a as u32;
            }
        }
        let effect = self.effect(&scores);
        DownbeatTerms {
            scores,
            best,
            held: self.alignment,
            effect_raw: effect.raw,
            null_share: effect.null,
            effect_corrected: effect.corrected,
            beats_seen: self.filled as u32,
            locked: self.filled >= MIN_BEATS && effect.corrected >= CONFIDENCE_THRESHOLD,
        }
    }

    /// Store one beat's accent in the ring.
    #[allow(
        clippy::indexing_slicing,
        reason = "cursor is kept modulo ACCENT_HISTORY, a valid index into both fixed arrays"
    )]
    fn record(&mut self, beat_index: u32, value: f32) {
        let value = if value.is_finite() {
            value.max(0.0)
        } else {
            0.0
        };
        self.phases[self.cursor] = beat_index % BEATS_PER_BAR;
        self.values[self.cursor] = value;
        self.cursor = (self.cursor + 1) % ACCENT_HISTORY;
        self.filled = (self.filled + 1).min(ACCENT_HISTORY);
    }

    /// Recompute the fold, the confidence, and any pending switch.
    fn reconsider(&mut self) {
        let scores = self.scores();
        let mut best = 0u32;
        let mut best_score = f32::NEG_INFINITY;
        for (a, &s) in scores.iter().enumerate() {
            if s > best_score {
                best_score = s;
                best = a as u32;
            }
        }
        self.confidence = self.effect_size(&scores);

        if best == self.alignment {
            self.challenger = None;
            return;
        }

        // A challenger has to lead by a real margin before it starts counting,
        // and then hold that lead for bars rather than beats.
        let incumbent = scores.get(self.alignment as usize).copied().unwrap_or(0.0);
        if best_score <= incumbent * (1.0 + SWITCH_MARGIN) {
            self.challenger = None;
            return;
        }
        let held = match self.challenger {
            Some((who, n)) if who == best => n + 1,
            _ => 1,
        };
        if held >= HYSTERESIS_BEATS {
            self.alignment = best;
            self.challenger = None;
        } else {
            self.challenger = Some((best, held));
        }
    }

    /// How much of the accent variation the alignment actually explains, `0..1`.
    ///
    /// **Not the best-versus-runner-up margin**, which was the first thing tried
    /// and is unusable: with four groups over `n` observations, pure noise already
    /// produces an expected margin around `(k-1)/(n-1)`, so an unaccented click
    /// train scored 0.257 against a 0.20 gate and locked onto nothing. A
    /// confidently wrong downbeat is the one failure ADR-0050 says an author
    /// cannot work around, so the measure has to know the difference between a
    /// pattern and a coincidence.
    ///
    /// This is the between-group share of variance (eta-squared), **corrected for
    /// the share noise alone would explain**. The correction is what makes the
    /// threshold mean the same thing at every history length: with little evidence
    /// the null share is large, so a weak pattern cannot clear the gate; as
    /// evidence accumulates the same effect size becomes significant. No separate
    /// "minimum evidence" tuning is doing that work.
    fn effect_size(&self, means: &[f32; BEATS_PER_BAR as usize]) -> f32 {
        self.effect(means).corrected
    }

    /// `effect_size` with its working shown — the raw between-group share, the
    /// null share it is discounted by, and the corrected value the gate reads.
    /// Same arithmetic on the same inputs; the split exists so [`Self::terms`]
    /// can report the correction separately from its result.
    fn effect(&self, means: &[f32; BEATS_PER_BAR as usize]) -> Effect {
        let n = self.filled;
        // Share four groups would explain by chance alone over n observations.
        let null = (BEATS_PER_BAR as f32 - 1.0) / (n as f32 - 1.0).max(1.0);
        let nothing = Effect {
            raw: 0.0,
            null,
            corrected: 0.0,
        };
        if n <= BEATS_PER_BAR as usize {
            return nothing;
        }
        let values = self.values.iter().take(n);
        let grand = values.clone().sum::<f32>() / n as f32;

        let mut between = 0.0f32;
        let mut within = 0.0f32;
        for (phase, value) in self.phases.iter().zip(self.values.iter()).take(n) {
            let slot = (*phase % BEATS_PER_BAR) as usize;
            let group = means.get(slot).copied().unwrap_or(0.0);
            let d = value - group;
            within += d * d;
        }
        for (a, &m) in means.iter().enumerate() {
            let count = self
                .phases
                .iter()
                .take(n)
                .filter(|p| (**p % BEATS_PER_BAR) as usize == a)
                .count();
            let d = m - grand;
            between += count as f32 * d * d;
        }

        let total = between + within;
        if total <= f32::EPSILON {
            return nothing;
        }
        let eta_sq = between / total;
        if null >= 1.0 {
            return Effect {
                raw: eta_sq,
                null,
                corrected: 0.0,
            };
        }
        Effect {
            raw: eta_sq,
            null,
            corrected: ((eta_sq - null) / (1.0 - null)).clamp(0.0, 1.0),
        }
    }

    /// Mean accent for each of the four alignments over the recorded history.
    fn scores(&self) -> [f32; BEATS_PER_BAR as usize] {
        let mut sums = [0.0f32; BEATS_PER_BAR as usize];
        let mut counts = [0u32; BEATS_PER_BAR as usize];
        for (phase, value) in self.phases.iter().zip(self.values.iter()).take(self.filled) {
            let slot = (*phase % BEATS_PER_BAR) as usize;
            if let (Some(sum), Some(count)) = (sums.get_mut(slot), counts.get_mut(slot)) {
                *sum += *value;
                *count += 1;
            }
        }
        std::array::from_fn(|a| {
            let n = counts.get(a).copied().unwrap_or(0);
            if n == 0 {
                0.0
            } else {
                sums.get(a).copied().unwrap_or(0.0) / n as f32
            }
        })
    }
}

impl Default for DownbeatTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// One beat's accent strength from the normalized bass and flux levels.
fn accent(bass: f32, onset: f32) -> f32 {
    BASS_WEIGHT * bass + (1.0 - BASS_WEIGHT) * onset
}

#[cfg(test)]
#[allow(
    clippy::indexing_slicing,
    reason = "the module's hot-path pragma also covers the tests; indices here are literals or loop bounds, and a panic is the intended failure"
)]
mod tests {
    use super::*;

    /// Drive `beats` beats with `accent_at(beat_index) -> (bass, onset)`, one hop
    /// per beat, and return the last reading.
    fn drive(
        tracker: &mut DownbeatTracker,
        beats: u32,
        start_index: u32,
        accent_at: impl Fn(u32) -> (f32, f32),
    ) -> BarClock {
        let mut last = BarClock::default();
        for i in 0..beats {
            let index = start_index + i;
            let (bass, onset) = accent_at(index);
            last = tracker.process(true, index, bass, onset, 0.0);
        }
        last
    }

    /// An accent every fourth beat, with beat 1 at `offset`.
    fn accented(offset: u32) -> impl Fn(u32) -> (f32, f32) {
        move |index| {
            if index % BEATS_PER_BAR == offset {
                (0.9, 0.8)
            } else {
                (0.25, 0.2)
            }
        }
    }

    /// ADR-0050's first pinned test: an accented click locks to the accented
    /// alignment, **in all four rotations**. One rotation passing would be
    /// consistent with the tracker simply always answering 0.
    #[test]
    fn an_accented_pattern_locks_in_every_rotation() {
        for offset in 0..BEATS_PER_BAR {
            let mut t = DownbeatTracker::new();
            let out = drive(&mut t, 32, 0, accented(offset));
            assert!(
                out.locked,
                "offset {offset} should lock (confidence {:.3})",
                out.confidence
            );
            assert_eq!(
                t.alignment, offset,
                "offset {offset} should be identified as beat 1, got {}",
                t.alignment
            );
            // And the published position agrees: the accented beat is beat 0 of
            // the bar. Checked on a beat whose index is the accented phase.
            let on_accent = t.process(false, offset + 4 * BEATS_PER_BAR, 0.0, 0.0, 0.0);
            assert_eq!(
                on_accent.beat_in_bar, 0,
                "offset {offset}: the accented beat should read beat_in_bar 0"
            );
        }
    }

    /// **`bar_index` is not monotone across an alignment change, and this is the
    /// size of the step.**
    ///
    /// Three places documented it as monotone and it is not: `bar_index` is
    /// `(beat_index - alignment) / BEATS_PER_BAR`, so the beat the estimator locks
    /// onto a non-zero alignment subtracts up to three beats and the counter can
    /// step back by one bar. Plan 0049 chose to soften those docs rather than
    /// publish a second never-decreasing counter — the counter would need
    /// history-dependent state on the determinism-sensitive path and would give up
    /// the "one formula for both paths" property that makes the lock and the
    /// fallback auditable against each other, all to buy immunity from a rare
    /// one-bar repeat that is already the soft failure the gate exists to prefer.
    ///
    /// Softening a doc is only honest if the behaviour it now describes is pinned,
    /// so: the step happens, and it is **exactly one bar**.
    #[test]
    fn bar_index_steps_back_across_an_alignment_change() {
        // Accent beat 2 of every bar, so the estimator's alignment is 2 rather
        // than the fallback's 0 — the case where locking shifts the counter.
        const OFFSET: u32 = 2;
        let mut t = DownbeatTracker::new();

        // Feed one beat short of the lock, then read the counter at a known beat
        // while still in fallback.
        let mut before = None;
        let mut after = None;
        for i in 0..40u32 {
            let (bass, onset) = accented(OFFSET)(i);
            let out = t.process(true, i, bass, onset, 0.0);
            if !out.locked {
                before = Some((i, out.bar_index));
            } else if after.is_none() {
                after = Some((i, out.bar_index));
            }
        }
        let (last_free_beat, free_bar) = before.expect("the tracker starts in fallback");
        let (first_locked_beat, locked_bar) =
            after.expect("an accented pattern eventually locks (see the test above)");
        assert_eq!(
            first_locked_beat,
            last_free_beat + 1,
            "the two readings must be consecutive beats for the step to be the lock's doing"
        );

        // In fallback the counter is beat_index / 4; locked it is
        // (beat_index - 2) / 4. Across the lock it therefore repeats a bar
        // whenever the beat index has already passed the alignment within its bar.
        assert_eq!(
            locked_bar,
            first_locked_beat.saturating_sub(OFFSET) / BEATS_PER_BAR,
            "the locked reading is the alignment-shifted counter"
        );
        assert_eq!(
            free_bar,
            last_free_beat / BEATS_PER_BAR,
            "the fallback reading is the plain counter"
        );
        // The claim the docs now make: it can fail to advance, and never by more
        // than one bar in either direction.
        let step = locked_bar as i64 - free_bar as i64;
        assert!(
            (-1..=1).contains(&step),
            "bar_index moved {step} bars across the lock at beat {first_locked_beat}"
        );
        assert!(
            step <= 0,
            "this fixture is the repeat case: locking to alignment {OFFSET} must not \
             advance the counter (free {free_bar} -> locked {locked_bar})"
        );
    }

    /// ADR-0050's second pinned test: an unaccented pattern must stay in
    /// fallback rather than crowning an alignment by noise.
    ///
    /// Run twice, and the **noisy** run is the one that matters. Perfectly equal
    /// accents are the easy case: between-group variance is exactly zero, so any
    /// measure reports nothing. Real material is never equal, and it was
    /// unstructured *variation* that made the first confidence measure — the
    /// best-versus-runner-up margin — lock onto an unaccented click train at
    /// 0.257. This is that regression.
    #[test]
    fn an_unaccented_pattern_stays_in_fallback() {
        for (label, jitter) in [("flat", 0.0f32), ("noisy", 0.35)] {
            let mut t = DownbeatTracker::new();
            let out = drive(&mut t, 32, 0, |index| {
                // Deterministic, unstructured, and deliberately NOT periodic in 4:
                // an irrational-ish step means no alignment is systematically
                // favoured, which is what "no downbeat" looks like.
                let wobble = jitter * (index as f32 * 2.399_963).sin();
                (
                    (0.5 + wobble).clamp(0.0, 1.0),
                    (0.5 - wobble).clamp(0.0, 1.0),
                )
            });
            assert!(
                !out.locked,
                "the {label} pattern must not lock (confidence {:.3})",
                out.confidence
            );
            assert!(
                out.confidence < CONFIDENCE_THRESHOLD,
                "{label}: confidence {:.3} should sit under the {CONFIDENCE_THRESHOLD} gate",
                out.confidence
            );
            // Fallback is the plain counter, exactly.
            assert_eq!(out.beat_in_bar, 31 % BEATS_PER_BAR, "{label}");
            assert_eq!(out.bar_index, 31 / BEATS_PER_BAR, "{label}");
        }
    }

    /// ADR-0050's third pinned test: a mid-stream alignment flip takes several
    /// bars, not one beat.
    #[test]
    fn an_alignment_flip_takes_bars_not_beats() {
        let mut t = DownbeatTracker::new();
        drive(&mut t, 32, 0, accented(0));
        assert_eq!(t.alignment, 0, "locked on the original alignment first");

        // The accent moves to beat 2. Count how many beats pass before the
        // tracker follows.
        let mut moved_after = None;
        for i in 0..40u32 {
            let index = 32 + i;
            let (bass, onset) = accented(2)(index);
            t.process(true, index, bass, onset, 0.0);
            if t.alignment == 2 && moved_after.is_none() {
                moved_after = Some(i + 1);
            }
        }
        let beats = moved_after.expect("the tracker should eventually follow the new accent");
        assert!(
            beats >= HYSTERESIS_BEATS,
            "the flip took {beats} beats, which is under the {HYSTERESIS_BEATS}-beat hysteresis"
        );
        assert!(
            beats <= 40,
            "the flip should still happen within a reasonable span, took {beats}"
        );
    }

    /// The fallback path is byte-deterministic — the phase's third done-when.
    #[test]
    fn the_fallback_path_is_byte_deterministic() {
        let run = || {
            let mut t = DownbeatTracker::new();
            (0..64u32)
                .map(|i| {
                    let c = t.process(i % 2 == 0, i, 0.5, 0.5, i as f32 * 0.01);
                    (
                        c.beat_in_bar,
                        c.bar_index,
                        c.bar_phase.to_bits(),
                        c.confidence.to_bits(),
                        c.locked,
                    )
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(run(), run());
    }

    #[test]
    fn bar_phase_spans_the_bar_and_beat_in_bar_stays_in_range() {
        let mut t = DownbeatTracker::new();
        let mut seen_low = false;
        let mut seen_high = false;
        for i in 0..32u32 {
            // Sweep the sub-beat phase so bar_phase covers the continuum, not
            // just the four beat boundaries.
            for step in 0..4 {
                let c = t.process(step == 0, i, 0.5, 0.5, step as f32 * 0.25);
                assert!(
                    c.beat_in_bar < BEATS_PER_BAR,
                    "beat_in_bar {} out of range",
                    c.beat_in_bar
                );
                assert!(
                    (0.0..1.0).contains(&c.bar_phase),
                    "bar_phase {} out of range",
                    c.bar_phase
                );
                if c.bar_phase < 0.1 {
                    seen_low = true;
                }
                if c.bar_phase > 0.9 {
                    seen_high = true;
                }
            }
        }
        assert!(
            seen_low && seen_high,
            "bar_phase should traverse the whole bar, saw low {seen_low} high {seen_high}"
        );
    }

    #[test]
    fn evidence_is_required_before_locking() {
        // A perfectly accented pattern still must not publish on one bar of
        // evidence: with two samples per alignment the margin is not a
        // measurement.
        let mut t = DownbeatTracker::new();
        let early = drive(&mut t, MIN_BEATS as u32 - 1, 0, accented(0));
        assert!(
            !early.locked,
            "{} beats is under the {MIN_BEATS}-beat evidence floor",
            MIN_BEATS - 1
        );
        let later = drive(&mut t, 24, MIN_BEATS as u32 - 1, accented(0));
        assert!(later.locked, "it should lock once the evidence arrives");
    }

    #[test]
    fn a_non_finite_accent_cannot_poison_the_fold() {
        let mut t = DownbeatTracker::new();
        drive(&mut t, 16, 0, accented(0));
        let before = t.confidence;
        t.process(true, 16, f32::NAN, f32::INFINITY, 0.0);
        assert!(
            t.confidence.is_finite(),
            "confidence went non-finite after a NaN accent (was {before})"
        );
        let after = drive(&mut t, 16, 17, accented(0));
        assert!(after.locked, "the tracker should recover and still lock");
    }
}
