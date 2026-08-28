//! Running normalization: turns raw magnitudes into "loud relative to this
//! track's recent past" (ADR-0049).
//!
//! Raw band means on real music sit at 0.006-0.040 while the authoring stimuli
//! reached 0.187-0.8, so every threshold in the shipped library was a magic
//! number against a table that moved three times in one week. Dividing each
//! signal by its own slowly-decaying running peak makes `> 0.5` mean the same
//! thing on every track, at every gain setting, on every stimulus.
//!
//! Three properties, each pinned by a test rather than by a comment:
//!
//! - **Instant attack.** A new peak is adopted on the hop it arrives, so a hit
//!   reads high immediately instead of fading in.
//! - **Slow release.** The peak decays with a seconds-scale time constant, so a
//!   quiet passage lifts gradually rather than pumping bar to bar.
//! - **Silence floor.** Below a floor the output is *zero*, not amplified noise
//!   — the difference between a quiet room and a loud one must not be a
//!   full-scale visual.
//!
//! Pure and allocation-free after construction: state is a fixed set of floats
//! and every step is arithmetic on the input, so the same sequence always yields
//! the same output (NFR section 6).
//!
//! **Where this sits matters.** Normalization is applied at the *published*
//! frame boundary only. The onset detector, the tempo tracker and the novelty
//! detector all keep reading raw values, because each is tuned against raw
//! magnitudes and would be actively harmed by an AGC: autocorrelating a
//! peak-normalized envelope distorts the very periodicity the tempo tracker
//! looks for, and per-band normalization flattens exactly the spectral-shape
//! difference novelty exists to measure.

// Hot-path panic-denial pragma (Plan 0002 Phase 2). Runs every analysis hop.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

use super::{HOP_SIZE, SPECTRUM_BINS, WAVE_SAMPLES};

/// Release time constant of the running peak, in seconds.
///
/// **Provisional**: ADR-0049 fixes the *properties* and leaves the feel to Plan
/// 0048 Phase 6's listening test, which is the phase allowed to move this. Too
/// fast reads as pumping (the level chases every bar), too slow as numbness (a
/// quiet section never recovers). 2.5 s is a few musical bars at ordinary
/// tempos, which is the scale "recent past" should mean.
const RELEASE_TAU_SECS: f32 = 2.5;

/// Silence floor for the band scalars and the 64-band array.
///
/// **The floor is the one place an absolute magnitude survives**, so it is the
/// one place gain-portability can break: a band whose running peak sits under
/// the floor reads 0 where a louder copy of the same track reads 1. That makes
/// the margin, not the value, the thing to get right.
///
/// Measured against `signal::dynamic_groove`: the three band scalars peak at
/// 0.020-0.109 and **every one** of the 64 bands peaks above 0.011. At 1e-4 that
/// is a 110x-1000x margin, so the same material still clears the floor a decade
/// *below* -20 dB. An earlier draft used 1e-3 and was wrong for exactly this
/// reason — only ~6x over real mid/treb means, so a -20 dB track lost its mid
/// and treble entirely and read as bass-only, defeating the portability this
/// whole change exists to buy.
///
/// Downward, a -80 dBFS room noise floor spreads its energy across bins and
/// lands well under 1e-4, so it is still suppressed rather than amplified.
pub const BAND_FLOOR: f32 = 1e-4;

/// Silence floor for `onset`. An order of magnitude lower because spectral flux
/// is an order of magnitude smaller: the same groove peaks at 0.0167 with a
/// 0.0016 mean, so this keeps the same ~1000x margin.
pub const ONSET_FLOOR: f32 = 1e-5;

/// Per-hop release coefficient for `RELEASE_TAU_SECS` at `sample_rate`.
fn release_per_hop(sample_rate: u32) -> f32 {
    let hop_dt = HOP_SIZE as f32 / sample_rate.max(1) as f32;
    (-hop_dt / RELEASE_TAU_SECS).exp()
}

/// One signal's running peak, and the normalized reading it produces.
///
/// Instant attack, exponential release, floored. Kept a struct rather than a
/// closure so the 64-band variant can share the step exactly.
pub struct PeakNormalizer {
    peak: f32,
    release: f32,
    floor: f32,
}

impl PeakNormalizer {
    /// A normalizer for `sample_rate`, reporting zero until the signal clears
    /// `floor`.
    pub fn new(sample_rate: u32, floor: f32) -> Self {
        Self {
            peak: 0.0,
            release: release_per_hop(sample_rate),
            floor,
        }
    }

    /// Advance one hop and return `raw` as a 0..1 fraction of its recent peak.
    pub fn normalize(&mut self, raw: f32) -> f32 {
        step(&mut self.peak, raw, self.release, self.floor)
    }
}

/// The 64-band array's normalizer: **one** running peak, tracking the loudest
/// band, applied as a uniform gain across the whole array.
///
/// One shared peak rather than 64 independent ones, and the reason is that the
/// array is a *spectrum* — its values only mean anything relative to each other.
/// Normalizing each band against its own peak makes every band that is not
/// literally silent climb to full scale: a pure tone's Hann leakage four bands
/// out, some 60 dB down, would report 1.0 because that leakage is its own recent
/// maximum. Two things break at once. The array stops describing spectral shape,
/// and how many bands light up becomes a function of the silence floor — an
/// absolute magnitude, which is the exact dependence ADR-0049 exists to remove.
///
/// It would also have silently destroyed the timbre idiom two shipped presets
/// are built on: `attractor_clifford` and `fragment_aurora` both read
/// `bin(0.84) - bin(0.14)` as a contrast between two probes. Per-band
/// normalization leaves both terms near their own peaks, so the difference
/// degenerates to noise — information destroyed in the analyzer, where no
/// preset-level retune could recover it.
///
/// A uniform gain keeps every ratio in the array exact while still making
/// thresholds portable: `bin(x) > 0.5` means "half as loud as this track's
/// recent loudest band", on any track at any gain.
///
/// This is a deliberate deviation from Plan 0048 Phase 2's "per-band and
/// per-scalar" wording and ADR-0049's diagram. The four *scalars* do keep
/// independent peaks — they are separate signals, not one distribution.
pub struct BandNormalizer {
    peak: f32,
    release: f32,
    floor: f32,
}

impl BandNormalizer {
    /// A normalizer for the whole band array at `sample_rate`.
    pub fn new(sample_rate: u32) -> Self {
        Self {
            peak: 0.0,
            release: release_per_hop(sample_rate),
            floor: BAND_FLOOR,
        }
    }

    /// Advance one hop, normalizing `bands` in place against their shared peak.
    pub fn normalize(&mut self, bands: &mut [f32; SPECTRUM_BINS]) {
        let loudest = bands
            .iter()
            .copied()
            .filter(|v| v.is_finite())
            .fold(0.0f32, f32::max);
        match advance(&mut self.peak, loudest, self.release, self.floor) {
            Some(peak) => {
                for band in bands.iter_mut() {
                    let raw = if band.is_finite() { band.max(0.0) } else { 0.0 };
                    *band = (raw / peak).clamp(0.0, 1.0);
                }
            }
            // Under the floor the whole array is silence, not something to
            // amplify into a full-scale display of a quiet room.
            None => *bands = [0.0; SPECTRUM_BINS],
        }
    }
}

/// Advance a running peak one hop: adopt a louder value instantly, release
/// exponentially otherwise. `None` while the peak sits at or under `floor`,
/// which is the caller's cue to report silence rather than divide.
///
/// Non-finite input is treated as silence rather than propagated. A NaN reaching
/// `peak` would be absorbing — `raw > released` is false for every subsequent
/// `raw`, so the peak would never recover and the signal would be dead for the
/// rest of the run. Plan 0038 Phase 9 paid for that lesson in `Easing::step`.
fn advance(peak: &mut f32, raw: f32, release: f32, floor: f32) -> Option<f32> {
    let raw = if raw.is_finite() { raw.max(0.0) } else { 0.0 };
    let released = *peak * release;
    *peak = if raw > released { raw } else { released };
    if *peak <= floor { None } else { Some(*peak) }
}

/// One signal's normalized reading, sharing [`advance`]'s state machine.
fn step(peak: &mut f32, raw: f32, release: f32, floor: f32) -> f32 {
    let clean = if raw.is_finite() { raw.max(0.0) } else { 0.0 };
    match advance(peak, raw, release, floor) {
        Some(p) => (clean / p).clamp(0.0, 1.0),
        None => 0.0,
    }
}

/// Silence floor for the waveform trace, in **signal amplitude** — a different
/// quantity from [`BAND_FLOOR`], which is a band mean, and deliberately an order
/// of magnitude above it.
///
/// A band mean spreads a signal's energy across bins, so a -80 dBFS room lands
/// well under `1e-4` there. A time-domain peak does no such spreading: -80 dBFS
/// **is** an amplitude of `1e-4`, so `BAND_FLOOR` copied here would sit exactly
/// at room noise and amplify it to a full-scale trace — the one failure a floor
/// exists to prevent.
///
/// Measured against `signal::dynamic_groove` at 48 kHz, per 120 bpm beat: the
/// trace peaks at `0.900` through the loud phrase and at `0.206` in the two
/// resting beats, whose `0.04` phrase scale is the quietest material the
/// stimulus contains. So `1e-3` (-60 dBFS) leaves the quietest real material
/// **206x** clear of the floor, and the same material a decade down the fader
/// still **21x** clear — levelled rather than zeroed. Downward it suppresses a
/// -80 dBFS room by a decade and 16-bit dither (LSB `3e-5`) by two.
///
/// `the_floor_clears_real_material_by_two_orders_of_magnitude` is the margin,
/// asserted as a ratio rather than as this paragraph's readings.
pub const WAVE_FLOOR: f32 = 1e-3;

/// The waveform's normalizer: **one** running peak of the trace's magnitude,
/// applied as a uniform gain to the whole trace.
///
/// One shared peak for the same reason [`BandNormalizer`] uses one — the samples
/// of a trace only mean anything relative to each other, and a per-sample gain
/// would erase the shape the trace exists to show. Two differences from that
/// type, both forced by the quantity:
///
/// - **The peak tracks `|x|` and the output stays signed.** A trace is a signal,
///   not a magnitude, so it is divided rather than rectified and clamps to
///   `-1..=1` instead of `0..=1`.
/// - **The divisor is returned rather than discarded**, because it is published
///   as `AnalysisFrame::waveform_gain` (ADR-0139): the raw amplitude is
///   `waveform[i] * waveform_gain`, which is the escape hatch a consumer that
///   genuinely wants absolute level reaches for.
pub struct TraceNormalizer {
    peak: f32,
    release: f32,
    floor: f32,
}

impl TraceNormalizer {
    /// A normalizer for the trace at `sample_rate`.
    pub fn new(sample_rate: u32) -> Self {
        Self {
            peak: 0.0,
            release: release_per_hop(sample_rate),
            floor: WAVE_FLOOR,
        }
    }

    /// Advance one hop, normalizing `trace` in place, and return the divisor
    /// removed — `0.0` while the tracked peak sits under the floor, where the
    /// trace is zeroed rather than amplified.
    pub fn normalize(&mut self, trace: &mut [f32; WAVE_SAMPLES]) -> f32 {
        let loudest = trace
            .iter()
            .copied()
            .filter(|v| v.is_finite())
            .fold(0.0f32, |m, v| m.max(v.abs()));
        match advance(&mut self.peak, loudest, self.release, self.floor) {
            Some(peak) => {
                for sample in trace.iter_mut() {
                    let raw = if sample.is_finite() { *sample } else { 0.0 };
                    *sample = (raw / peak).clamp(-1.0, 1.0);
                }
                peak
            }
            None => {
                *trace = [0.0; WAVE_SAMPLES];
                0.0
            }
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    const SR: u32 = 48_000;
    /// Hops per second at `SR` — the release is specified in seconds, so the
    /// tests count in seconds too.
    const HOPS_PER_SEC: usize = SR as usize / HOP_SIZE;

    #[test]
    fn a_new_peak_is_adopted_on_the_hop_it_arrives() {
        let mut n = PeakNormalizer::new(SR, BAND_FLOOR);
        // Instant attack: the very first loud hop already reads full scale, so a
        // kick is not a fade-in.
        assert_eq!(n.normalize(0.5), 1.0);
        // And a *louder* hop still reads 1.0 rather than overshooting.
        assert_eq!(n.normalize(0.9), 1.0);
    }

    #[test]
    fn the_peak_releases_over_seconds_not_hops() {
        let mut n = PeakNormalizer::new(SR, BAND_FLOOR);
        n.normalize(1.0);
        // A quiet-but-audible signal right after a peak reads low...
        let immediately = n.normalize(0.2);
        assert!(
            immediately < 0.25,
            "just after a peak, 0.2 should still read low, got {immediately}"
        );
        // ...and the same signal reads high once the peak has released. One tau
        // is a factor of e, so hold for a few and it must be most of the way.
        for _ in 0..(4 * HOPS_PER_SEC) {
            n.normalize(0.2);
        }
        let later = n.normalize(0.2);
        assert!(
            later > 0.9,
            "after 4 s of 0.2 the peak should have released to it, got {later}"
        );

        // Non-vacuity on the *time scale*: the release must not be so fast that
        // it happens within a fraction of a second, which is what "seconds" is
        // guarding against. A fresh normalizer, one peak, then a tenth of a
        // second of quiet.
        let mut fast = PeakNormalizer::new(SR, BAND_FLOOR);
        fast.normalize(1.0);
        for _ in 0..(HOPS_PER_SEC / 10) {
            fast.normalize(0.2);
        }
        let after_100ms = fast.normalize(0.2);
        assert!(
            after_100ms < 0.3,
            "a seconds-scale release must barely move in 100 ms, got {after_100ms}"
        );
    }

    #[test]
    fn silence_reads_zero_and_room_noise_is_not_amplified() {
        let mut n = PeakNormalizer::new(SR, BAND_FLOOR);
        // True silence: zero in, zero out, for as long as you like.
        for _ in 0..(3 * HOPS_PER_SEC) {
            assert_eq!(n.normalize(0.0), 0.0);
        }
        // Low-level noise well under the floor stays at zero rather than being
        // lifted to full scale — the whole point of the floor.
        let mut noisy = PeakNormalizer::new(SR, BAND_FLOOR);
        let mut seen: f32 = 0.0;
        for i in 0..(3 * HOPS_PER_SEC) {
            // Deterministic pseudo-noise around 1e-5, an order under the floor.
            let dust = 1e-5 * (1.0 + 0.5 * (i as f32 * 0.7).sin());
            seen = seen.max(noisy.normalize(dust));
        }
        assert_eq!(
            seen, 0.0,
            "sub-floor dust must never be amplified, peaked at {seen}"
        );

        // Counter-assertion: the floor is not simply swallowing everything.
        // Content an order of magnitude above it normalizes as usual.
        let mut real = PeakNormalizer::new(SR, BAND_FLOOR);
        assert_eq!(real.normalize(1e-3), 1.0);
    }

    #[test]
    fn the_same_dynamics_normalize_alike_at_any_absolute_level() {
        // The portability property, and the reason the whole change is worth a
        // library retune: the *shape* of the level over time is what survives,
        // not the gain it arrived at. A steady tone would pass this trivially
        // (everything steady reads 1.0), so the fixture has real dynamics.
        let pattern: Vec<f32> = (0..(6 * HOPS_PER_SEC))
            .map(|i| {
                let t = i as f32 / HOPS_PER_SEC as f32;
                // A slow swell with a beat riding on it.
                (0.35 + 0.3 * (t * 0.8).sin()) * (1.0 + 0.6 * (t * 6.0).sin().max(0.0))
            })
            .collect();

        let run = |gain: f32| -> Vec<f32> {
            let mut n = PeakNormalizer::new(SR, BAND_FLOOR);
            pattern.iter().map(|v| n.normalize(v * gain)).collect()
        };

        let full = run(1.0);
        // -20 dB is a factor of 10 in amplitude.
        let quiet = run(0.1);
        let worst = full
            .iter()
            .zip(quiet.iter())
            .map(|(a, b)| (a - b).abs())
            .fold(0.0f32, f32::max);
        assert!(
            worst < 1e-5,
            "a -20 dB copy must normalize to the same series, worst divergence {worst}"
        );

        // And the series is genuinely varied, so the agreement above is not two
        // constant runs matching.
        let spread = full.iter().copied().fold(0.0f32, f32::max)
            - full.iter().copied().fold(1.0f32, f32::min);
        assert!(
            spread > 0.4,
            "fixture should exercise a real range, got {spread}"
        );
    }

    #[test]
    fn the_array_keeps_its_shape_under_one_shared_peak() {
        let mut n = BandNormalizer::new(SR);
        // Band 1 loud, band 2 a hundredth of it. The ratio between them is the
        // spectrum's whole content, so it has to survive normalization exactly.
        let mut bands = [0.0f32; SPECTRUM_BINS];
        for _ in 0..HOPS_PER_SEC {
            bands = [0.0; SPECTRUM_BINS];
            if let Some(loud) = bands.get_mut(1) {
                *loud = 0.5;
            }
            if let Some(quiet) = bands.get_mut(2) {
                *quiet = 0.005;
            }
            n.normalize(&mut bands);
        }
        assert_eq!(
            bands.get(1).copied(),
            Some(1.0),
            "the loudest band anchors at 1.0"
        );
        assert_eq!(
            bands.get(2).copied(),
            Some(0.01),
            "a band a hundredth as loud must still read a hundredth — per-band \
             normalization would have lifted it to 1.0 and destroyed the contrast"
        );
        assert_eq!(
            bands.get(3).copied(),
            Some(0.0),
            "a silent band stays silent"
        );
    }

    #[test]
    fn a_bin_contrast_survives_normalization() {
        // The property two shipped presets depend on: `bin(hi) - bin(lo)` as a
        // timbre signal. Under a shared peak the difference is preserved up to
        // the gain; under per-band peaks it would collapse toward zero because
        // both probes would sit at their own maxima.
        let mut n = BandNormalizer::new(SR);
        let mut last = [0.0f32; SPECTRUM_BINS];
        for _ in 0..HOPS_PER_SEC {
            last = [0.0; SPECTRUM_BINS];
            // A bright frame: high probe well above the low one.
            if let Some(lo) = last.get_mut(10) {
                *lo = 0.02;
            }
            if let Some(hi) = last.get_mut(50) {
                *hi = 0.08;
            }
            n.normalize(&mut last);
        }
        let contrast = last.get(50).copied().unwrap_or(0.0) - last.get(10).copied().unwrap_or(0.0);
        assert!(
            (contrast - 0.75).abs() < 1e-6,
            "the 0.08-vs-0.02 contrast should normalize to 0.75, got {contrast}"
        );
    }

    #[test]
    fn a_non_finite_input_cannot_poison_the_peak() {
        let mut n = PeakNormalizer::new(SR, BAND_FLOOR);
        n.normalize(0.5);
        assert_eq!(n.normalize(f32::NAN), 0.0);
        assert_eq!(n.normalize(f32::INFINITY), 0.0);
        // Recovery is the real claim: a poisoned peak would leave every later
        // hop dead for the rest of the run.
        assert_eq!(n.normalize(0.5), 1.0);
    }

    /// A trace of `WAVE_SAMPLES` at `peak`, shaped so it has both signs and a
    /// structure a uniform gain must preserve.
    fn trace_at(peak: f32) -> [f32; WAVE_SAMPLES] {
        std::array::from_fn(|i| {
            let t = i as f32 / WAVE_SAMPLES as f32;
            peak * (std::f32::consts::TAU * 3.0 * t).sin() * (0.4 + 0.6 * t)
        })
    }

    /// The floor is an **amplitude**, so the margin that matters is against the
    /// quietest passage of real material rather than against a band mean.
    ///
    /// `dynamic_groove`'s resting beats are its `0.04` phrase scale — the
    /// quietest thing it contains — and they must stay far enough over the floor
    /// that the same material a decade down the fader is still levelled instead
    /// of zeroed. Asserted as ratios; the readings themselves are in
    /// [`WAVE_FLOOR`]'s derivation.
    #[test]
    fn the_floor_clears_real_material_by_two_orders_of_magnitude() {
        let format = crate::audio::AudioFormat {
            sample_rate: SR,
            channels: 1,
        };
        let pcm = crate::signal::dynamic_groove(120.0, 8.0, format);
        let beat = SR as usize / 2;
        let quietest = pcm
            .chunks(beat)
            .map(|c| c.iter().fold(0.0f32, |m, v| m.max(v.abs())))
            .fold(f32::INFINITY, f32::min);
        assert!(
            quietest > 100.0 * WAVE_FLOOR,
            "the quietest beat peaks at {quietest}, only {}x the floor",
            quietest / WAVE_FLOOR
        );
        // ...and a decade down the fader it is still material, not silence.
        assert!(
            quietest * 0.1 > 10.0 * WAVE_FLOOR,
            "a decade down, the quietest beat is {}x the floor",
            quietest * 0.1 / WAVE_FLOOR
        );
    }

    /// The whole point: the input gain cancels.
    ///
    /// The normalizer divides by a peak that scales with its input, so it is
    /// homogeneous of degree zero above the floor — the same signal at any
    /// fader position produces the same trace. Not bit-exact for an arbitrary
    /// `k`, because `(k*x)/(k*p)` rounds differently from `x/p`; exact for a
    /// power of two, which is asserted separately.
    #[test]
    fn a_trace_normalizes_to_the_same_shape_at_any_input_gain() {
        let reference = {
            let mut t = trace_at(0.9);
            TraceNormalizer::new(SR).normalize(&mut t);
            t
        };
        for k in [0.18f32, 0.4, 3.0] {
            let mut scaled = trace_at(0.9 * k);
            let gain = TraceNormalizer::new(SR).normalize(&mut scaled);
            assert!(
                gain > 0.0,
                "gain 0 at k={k}: the stimulus fell under the floor"
            );
            let worst = reference
                .iter()
                .zip(scaled.iter())
                .fold(0.0f32, |m, (a, b)| m.max((a - b).abs()));
            assert!(worst < 1e-6, "k={k} moved the trace by {worst}");
        }
        let mut halved = trace_at(0.45);
        TraceNormalizer::new(SR).normalize(&mut halved);
        assert_eq!(
            halved, reference,
            "a power-of-two gain change must cancel exactly"
        );
    }

    /// Silence is reported as silence rather than amplified into a full-scale
    /// display of a quiet room — the same rule [`BAND_FLOOR`] holds for the band
    /// array, at an amplitude the floor's own derivation names.
    #[test]
    fn a_sub_floor_trace_reads_as_silence_and_publishes_no_gain() {
        let mut silent = [0.0f32; WAVE_SAMPLES];
        assert_eq!(TraceNormalizer::new(SR).normalize(&mut silent), 0.0);
        assert!(silent.iter().all(|v| *v == 0.0));

        let mut whisper = trace_at(WAVE_FLOOR * 0.5);
        assert_eq!(TraceNormalizer::new(SR).normalize(&mut whisper), 0.0);
        assert!(
            whisper.iter().all(|v| *v == 0.0),
            "a sub-floor trace must be zeroed, not scaled up"
        );
    }

    /// The published divisor is a real escape hatch: multiplying it back gives
    /// the amplitude the analyzer read (ADR-0139).
    #[test]
    fn the_published_gain_reconstructs_the_raw_trace() {
        let raw = trace_at(0.62);
        let mut normalized = raw;
        let gain = TraceNormalizer::new(SR).normalize(&mut normalized);
        let worst = raw
            .iter()
            .zip(normalized.iter())
            .fold(0.0f32, |m, (r, n)| m.max((r - n * gain).abs()));
        assert!(worst < 1e-6, "reconstruction is off by {worst}");
    }

    /// A NaN sample must not become an absorbing peak — the trap `advance`'s
    /// doc names, reached here through the trace's own magnitude scan.
    #[test]
    fn a_non_finite_sample_does_not_poison_the_running_peak() {
        let mut n = TraceNormalizer::new(SR);
        let mut poisoned = trace_at(0.5);
        poisoned[7] = f32::NAN;
        let gain = n.normalize(&mut poisoned);
        assert!(gain.is_finite() && gain > 0.0, "gain went bad: {gain}");
        assert!(poisoned.iter().all(|v| v.is_finite()));
        let mut after = trace_at(0.5);
        assert!(n.normalize(&mut after) > 0.0, "the peak never recovered");
    }
}
