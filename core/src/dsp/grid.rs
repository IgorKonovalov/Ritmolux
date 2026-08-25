//! The bar-scale grid Layer 2 folds over (ADR-0109, Plan 0095 Phase 3).
//!
//! [`beat_index`](super::AnalysisFrame::beat_index) counts **onset events**, not
//! musical beats — 1.73x / 1.35-2.10x / 1.76x of them per beat on three genres,
//! against a synthesized control that reads exactly 1.00. Folding an accent
//! history over `beat_index % 4` therefore spans well under a bar at a ratio
//! that is not even a stable integer within one track, so a bar-locked accent
//! precesses across all four alignments instead of accumulating in one. This
//! module is the unit that repairs it: a beat clock driven by the **tempo
//! estimate** rather than by the transient stream.
//!
//! Two things it deliberately is not:
//!
//! - **Not a second `beat_index`.** Nothing outside the analysis path reads it.
//!   `beat`, `beat_index` and `time_since_beat` keep their present behaviour bit
//!   for bit, so no preset's flash timing moves (ADR-0109's Alternatives A and B
//!   record why that was chosen over repairing them in place).
//! - **Not a downbeat estimate.** It says where the beat *grid* is, not which of
//!   its beats is beat 1. Which one that is stays [`downbeat`](super::downbeat)'s
//!   job, and its four-alignment fold is what absorbs this grid's arbitrary bar
//!   offset.
//!
//! **The phase is locked, not free-running**, which is the design question the
//! plan flagged as most likely to need a second attempt. A free accumulator
//! walks off the music at any tempo error, and — worse than drifting — it can
//! settle with its beat boundary sitting *on* the music's transients, which
//! splits every accent across two adjacent cells and smears the fold it exists
//! to sharpen. The lock is a phase-locked loop over the onset **envelope**, not
//! the beat flag: two exponentially-decayed quadrature accumulators give the
//! energy-weighted mean phase of the recent envelope relative to the grid, and
//! the grid is nudged toward it by a small per-hop gain. That is the plan's
//! "correct toward the onset stream's aggregate phase over a window rather than
//! snapping to each event", and it is what keeps the over-firing detector from
//! yanking the grid the way it yanks [`tempo`](super::tempo)'s `bar` phase.
//!
//! Pure and allocation-free after construction: state is five scalars, the only
//! clock is the hop counter, and the same `(bpm, onset)` sequence always yields
//! the same positions (NFR section 6).

// Hot-path panic-denial pragma (Plan 0002 Phase 2). Runs every analysis hop.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

use std::f32::consts::TAU;

use super::HOP_SIZE;
use super::downbeat::BEATS_PER_BAR;

/// Time constant of the phase lock's window, in seconds. Two seconds is a few
/// bars at any tempo in the search range: long enough that one loud transient in
/// the wrong place cannot move the grid, short enough to follow a real shift
/// within a phrase.
const LOCK_TAU_SECS: f32 = 2.0;

/// Fraction of the measured phase error corrected per hop.
///
/// Sized so the correction can never exceed the advance: at [`MIN_BPM`] the grid
/// advances 0.0107 beats per hop and the largest possible correction is
/// `0.02 * 0.5 = 0.01`, so the grid cannot run backwards even at the slowest
/// tempo with the worst error. The step is clamped non-negative anyway — a
/// structural guarantee is worth more here than an arithmetic one, because
/// `bar_index` going backwards would be visible in a preset as a repeated bar.
///
/// [`MIN_BPM`]: super::tempo
const LOCK_GAIN: f32 = 0.02;

/// Where the grid stands this hop.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct GridPosition {
    /// Which beat of the bar this is, `0..BEATS_PER_BAR`. **Which one is beat 1
    /// is not decided here** — [`downbeat`](super::downbeat) folds over this and
    /// finds that.
    pub beat_in_bar: u32,
    /// Monotone bar counter since the grid started running.
    pub bar_index: u32,
    /// Position across the bar in `[0, 1)`, including the fraction through the
    /// current beat.
    pub bar_phase: f32,
    /// Position across the current beat in `[0, 1)`.
    pub beat_phase: f32,
    /// Whether the grid is advancing. False until the tempo tracker warms up,
    /// and the position is held rather than reset while it is false.
    pub running: bool,
}

/// A beat clock driven by the tempo estimate, phase-locked to the onset
/// envelope.
pub struct BarGrid {
    /// Seconds per hop — the fixed conversion between BPM and phase per hop.
    hop_sec: f32,
    /// Per-hop decay of the quadrature accumulators, derived from
    /// [`LOCK_TAU_SECS`] at construction so nothing on the hot path calls `exp`.
    decay: f32,
    /// Position within the current grid beat, `[0, 1)`.
    beat_phase: f32,
    /// Grid beats completed. Split from the phase rather than accumulated as one
    /// float, so an hour-long session loses no sub-beat resolution.
    beats: u32,
    /// Quadrature accumulators: the envelope's energy projected onto the grid's
    /// own phase, exponentially weighted. Their argument is the mean phase the
    /// lock steers toward.
    cos_acc: f32,
    sin_acc: f32,
}

impl BarGrid {
    /// A grid for `sample_rate`, stopped until the first positive tempo.
    pub fn new(sample_rate: u32) -> Self {
        let hop_sec = HOP_SIZE as f32 / sample_rate as f32;
        Self {
            hop_sec,
            decay: (-hop_sec / LOCK_TAU_SECS).exp(),
            beat_phase: 0.0,
            beats: 0,
            cos_acc: 0.0,
            sin_acc: 0.0,
        }
    }

    /// Advance one hop against the current tempo estimate and onset envelope.
    ///
    /// `bpm` is the tempo tracker's estimate and `onset` the **raw** envelope —
    /// raw for the same reason the tempo tracker reads it raw (see
    /// [`gain`](super::gain)): peak normalization is a slow AGC that would
    /// reweight the history the lock averages over.
    pub fn process(&mut self, bpm: f32, onset: f32) -> GridPosition {
        if !bpm.is_finite() || bpm <= 0.0 || !onset.is_finite() {
            return self.position(false);
        }

        // Where the recent envelope's energy sits relative to this grid, as a
        // phase in [-0.5, 0.5) beats. Accumulated before the advance so the
        // reading and the projection use the same phase.
        let angle = TAU * self.beat_phase;
        self.cos_acc = self.cos_acc * self.decay + onset.max(0.0) * angle.cos();
        self.sin_acc = self.sin_acc * self.decay + onset.max(0.0) * angle.sin();
        let error = self.sin_acc.atan2(self.cos_acc) / TAU;

        // Advance by the tempo, pulled toward that energy. Clamped non-negative
        // so the grid can only ever stall, never reverse.
        let advance = bpm * self.hop_sec / 60.0;
        let step = (advance - LOCK_GAIN * error).max(0.0);
        self.beat_phase += step;
        while self.beat_phase >= 1.0 {
            self.beat_phase -= 1.0;
            self.beats = self.beats.saturating_add(1);
        }

        self.position(true)
    }

    /// The current position, without advancing.
    fn position(&self, running: bool) -> GridPosition {
        let beat_in_bar = self.beats % BEATS_PER_BAR;
        GridPosition {
            beat_in_bar,
            bar_index: self.beats / BEATS_PER_BAR,
            bar_phase: (beat_in_bar as f32 + self.beat_phase) / BEATS_PER_BAR as f32,
            beat_phase: self.beat_phase,
            running,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: u32 = 48_000;

    /// Hops in `secs` at the analyzer's hop rate.
    fn hops(secs: f32) -> usize {
        (secs * SR as f32 / HOP_SIZE as f32).round() as usize
    }

    /// The envelope every test here drives the grid with: a narrow spike once
    /// per musical beat, offset by `beat_offset` beats of phase.
    fn envelope(hop: usize, bpm: f32, beat_offset: f32) -> f32 {
        let t = hop as f32 * HOP_SIZE as f32 / SR as f32;
        let beat_secs = 60.0 / bpm;
        let since = (t / beat_secs - beat_offset).rem_euclid(1.0) * beat_secs;
        (-since * 40.0).exp()
    }

    /// Drive `secs` of that envelope from `cursor`, advancing it, so successive
    /// calls are one continuous stimulus rather than two that both start on a
    /// beat.
    fn drive(
        grid: &mut BarGrid,
        cursor: &mut usize,
        bpm: f32,
        secs: f32,
        beat_offset: f32,
    ) -> GridPosition {
        let mut last = GridPosition::default();
        for _ in 0..hops(secs) {
            last = grid.process(bpm, envelope(*cursor, bpm, beat_offset));
            *cursor += 1;
        }
        last
    }

    /// The rate claim: one bar per four musical beats, counted against the
    /// clip's own construction rather than against any detector's output.
    #[test]
    fn the_grid_advances_one_bar_per_four_musical_beats() {
        for bpm in [60.0f32, 90.0, 120.0, 175.0] {
            let mut grid = BarGrid::new(SR);
            // Measured as a rate across a window *after* the lock has pulled the
            // phase in, so the lock-in transient is not charged to the rate.
            let settle = 8.0;
            let window = 24.0;
            let mut cursor = 0usize;
            let start = drive(&mut grid, &mut cursor, bpm, settle, 0.0);
            let end = drive(&mut grid, &mut cursor, bpm, window, 0.0);
            let counted =
                (end.bar_index as f32 + end.bar_phase) - (start.bar_index as f32 + start.bar_phase);
            let expected = window / (60.0 / bpm) / BEATS_PER_BAR as f32;
            assert!(
                (counted - expected).abs() <= 0.05,
                "{bpm} BPM over {window} s is {expected:.3} bars by construction; \
                 the grid counted {counted:.3}"
            );
            assert!(end.running, "{bpm} BPM: the grid should be running");
        }
    }

    /// The lock claim: wherever the grid's phase starts, the envelope's spikes
    /// end up near the start of a grid beat rather than straddling one.
    ///
    /// The offsets are the cases that matter — 0.5 is the one a free-running
    /// accumulator can settle into, where every transient lands exactly on a
    /// cell boundary and the fold downstream splits each accent in two.
    #[test]
    fn the_phase_locks_onto_the_envelope_wherever_it_starts() {
        for offset in [0.0f32, 0.25, 0.5, 0.75] {
            let mut grid = BarGrid::new(SR);
            let mut cursor = 0usize;
            drive(&mut grid, &mut cursor, 120.0, 20.0, offset);
            // Then read where the grid says the loudest spike of the next few
            // seconds landed.
            let mut at_spike = 0.0f32;
            let mut peak = f32::NEG_INFINITY;
            for _ in 0..hops(4.0) {
                let onset = envelope(cursor, 120.0, offset);
                let pos = grid.process(120.0, onset);
                cursor += 1;
                if onset > peak {
                    peak = onset;
                    at_spike = pos.beat_phase;
                }
            }
            // Distance from the start of a grid beat, wrapped: a spike at 0.98
            // is as well aligned as one at 0.02.
            let from_beat = at_spike.min(1.0 - at_spike);
            assert!(
                from_beat <= 0.15,
                "starting {offset} beats off, the envelope's spike should sit near a grid \
                 beat, not {from_beat:.3} of a beat away (phase {at_spike:.3})"
            );
        }
    }

    /// The grid stalls rather than reversing, at every tempo in the search
    /// range and against an envelope actively pulling its phase backwards.
    #[test]
    fn the_bar_counter_never_runs_backwards() {
        for bpm in [60.0f32, 120.0, 200.0] {
            let mut grid = BarGrid::new(SR);
            let mut prev = (0u32, 0.0f32);
            let hop_sec = HOP_SIZE as f32 / SR as f32;
            for hop in 0..hops(20.0) {
                // Deliberately adversarial: a spike wherever the grid is not.
                let t = hop as f32 * hop_sec;
                let onset = (t * 7.3).sin().max(0.0);
                let pos = grid.process(bpm, onset);
                let now = (pos.bar_index, pos.bar_phase);
                assert!(
                    now.0 > prev.0 || (now.0 == prev.0 && now.1 >= prev.1),
                    "{bpm} BPM, hop {hop}: the grid went backwards, {prev:?} then {now:?}"
                );
                prev = now;
            }
        }
    }

    /// Before the tempo tracker warms up there is no grid, and the position is
    /// held rather than reset when it stops.
    #[test]
    fn a_stopped_grid_holds_its_position() {
        let mut grid = BarGrid::new(SR);
        for _ in 0..hops(1.0) {
            let pos = grid.process(0.0, 0.5);
            assert!(!pos.running, "no tempo means no grid");
            assert_eq!(pos.bar_index, 0);
            assert_eq!(pos.beat_phase, 0.0);
        }
        let mut cursor = 0usize;
        drive(&mut grid, &mut cursor, 120.0, 8.0, 0.0);
        let running = grid.process(120.0, 0.5);
        let stopped = grid.process(0.0, 0.5);
        assert!(!stopped.running);
        assert_eq!(
            stopped.bar_index, running.bar_index,
            "a stopped grid holds its bar rather than resetting it"
        );
        assert_eq!(stopped.beat_phase.to_bits(), running.beat_phase.to_bits());
    }

    /// **Layer 1 does not move.** The property every shipped preset's timing
    /// rests on (ADR-0109): `beat`, `beat_index` and `time_since_beat` are
    /// exactly what they were before this module existed.
    ///
    /// Asserted two ways, because they fail differently. The first is a
    /// same-run comparison: one analyzer with a grid driven off its own output
    /// every hop, one with no grid at all, and the three series must match bit
    /// for bit — which catches any feedback from the grid into Layer 1. The
    /// second is structural and outlives Phase 3, when the grid moves inside the
    /// analyzer and the first arm can no longer be built: the three are exactly
    /// the counter and the timer derived from the beat-flag stream, so anything
    /// that re-times them shows up as a broken derivation rather than as a
    /// number nobody has a reference for.
    #[test]
    fn the_grid_does_not_move_layer_1() {
        use crate::audio::AudioFormat;
        use crate::dsp::Analyzer;

        let format = AudioFormat {
            sample_rate: SR,
            channels: 1,
        };
        let pcm = crate::signal::click_track(120.0, 12.0, format);
        let hop_samples = HOP_SIZE * format.channels as usize;

        let run = |with_grid: bool| {
            let mut analyzer = Analyzer::new(format).expect("valid format");
            let mut grid = BarGrid::new(SR);
            let mut series = Vec::new();
            for samples in pcm.chunks(hop_samples) {
                analyzer.push_interleaved(samples);
                let f = analyzer.take_frame();
                if with_grid {
                    grid.process(f.bpm, f.onset_raw);
                }
                series.push((f.beat, f.beat_index, f.time_since_beat));
            }
            series
        };

        let with = run(true);
        let without = run(false);
        assert_eq!(with.len(), without.len());
        for (hop, (a, b)) in with.iter().zip(without.iter()).enumerate() {
            assert_eq!(a.0, b.0, "hop {hop}: the beat flag moved");
            assert_eq!(a.1, b.1, "hop {hop}: beat_index moved");
            assert_eq!(
                a.2.to_bits(),
                b.2.to_bits(),
                "hop {hop}: time_since_beat moved ({} then {})",
                a.2,
                b.2
            );
        }

        // The derivation, restated as an assertion. `beat_index` is one less
        // than the number of flags seen, and `time_since_beat` is hops since the
        // last flag on the hop clock — no wall clock, no grid.
        //
        // Read from the first *analyzed* hop: until the low window is full the
        // analyzer publishes its default frame, which is not a reading of
        // anything (`filled` reaches `LOW_WINDOW_SIZE` on hop `WARMUP_HOPS - 1`).
        let hop_sec = HOP_SIZE as f32 / SR as f32;
        let mut flags = 0u32;
        let mut since = 0u32;
        let mut saw_a_beat = false;
        for (hop, &(beat, index, time)) in with
            .iter()
            .enumerate()
            .skip(crate::dsp::WARMUP_HOPS.saturating_sub(1))
        {
            if beat {
                flags += 1;
                since = 0;
                saw_a_beat = true;
            } else {
                since += 1;
            }
            assert_eq!(
                index,
                flags.saturating_sub(1),
                "hop {hop}: beat_index must be the flag count less one"
            );
            assert_eq!(
                time.to_bits(),
                (since as f32 * hop_sec).to_bits(),
                "hop {hop}: time_since_beat must be hops-since-flag on the hop clock"
            );
        }
        assert!(saw_a_beat, "the clip should have produced beats at all");
    }

    /// Determinism: the same `(bpm, onset)` sequence twice, bit for bit.
    #[test]
    fn the_grid_is_deterministic() {
        let run = || {
            let mut grid = BarGrid::new(SR);
            let mut series = Vec::new();
            for hop in 0..hops(10.0) {
                let onset = ((hop as f32) * 0.37).sin().abs();
                let pos = grid.process(128.0, onset);
                series.push((pos.bar_index, pos.bar_phase.to_bits()));
            }
            series
        };
        assert_eq!(
            run(),
            run(),
            "the grid must be a pure function of its input"
        );
    }
}
