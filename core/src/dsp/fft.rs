//! Windowed FFT producing linear magnitudes plus a log-frequency band
//! spectrum for scenes.
//!
//! **Two windows, one axis** (ADR-0049). A single 2048 window cannot carry a
//! 64-band log axis: its bins are 23.4 Hz apart at 48 kHz, while the lowest log
//! bands are 3.6 Hz wide, so the bottom 20 bands were narrower than one bin and
//! the old collapse fix-up spread them across single linear bins instead. The
//! kick-and-sub region — the most-bound part of the axis — was its worst
//! resolved.
//!
//! So the short [`WINDOW_SIZE`] window keeps feeding every band it can actually
//! resolve, and a longer [`LOW_WINDOW_SIZE`] window feeds the bands below the
//! crossover. **The crossover is derived, not chosen:** it is the first band
//! whose width reaches one short-window bin (band 20, ~246 Hz at 48 kHz — which
//! lands within 2 % of the independently-chosen `BASS_HI_HZ`). Above it nothing
//! about the layout changed; below it the axis is genuinely logarithmic for the
//! first time.
//!
//! The low bands inherit the long window's slower time response — 85 ms of
//! Hann group delay at 8192 — and that is physics, stated rather than
//! compensated away (ADR-0049). It does **not** move NFR section 3's
//! beat-to-reaction budget: onset, beat and tempo all still read the short
//! window's magnitudes, untouched.

// Hot-path panic-denial pragma (Plan 0002 Phase 2).
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

use std::sync::Arc;

use rustfft::num_complex::Complex;
use rustfft::{Fft, FftPlanner};

use super::{LOW_WINDOW_SIZE, SPECTRUM_BINS, WINDOW_SIZE};

const MAG_BINS: usize = WINDOW_SIZE / 2;
const LOW_MAG_BINS: usize = LOW_WINDOW_SIZE / 2;
/// Log band range. The top is clamped below Nyquist for low sample rates.
const BAND_LO_HZ: f32 = 35.0;
const BAND_HI_HZ: f32 = 18_000.0;

/// Which analysis window a band's magnitudes come from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BandSource {
    /// The long [`LOW_WINDOW_SIZE`] window — bands below the crossover.
    Long,
    /// The short [`WINDOW_SIZE`] window — bands at or above the crossover.
    Short,
}

/// The 64-band log axis resolved against both analysis windows.
///
/// Pure: a function of the sample rate alone, built once at construction. Kept
/// separate from [`SpectrumAnalyzer`] so the layout's properties are testable
/// without planning an FFT.
pub struct BandLayout {
    /// Half-open bin range per band, within that band's own source window.
    bins: [(usize, usize); SPECTRUM_BINS],
    /// Band edge frequencies in Hz; band `k` spans `edges_hz[k]..edges_hz[k+1]`.
    edges_hz: [f32; SPECTRUM_BINS + 1],
    /// Bands `[0, crossover_band)` read the long window; the rest the short one.
    crossover_band: usize,
    /// Highest long-window bin any band needs — the long FFT's magnitudes are
    /// only converted this far, since nothing above the crossover reads them.
    long_bins_used: usize,
    /// Low bands even the long window cannot resolve, widened to a one-bin floor
    /// so they stay non-empty. Reported rather than hidden: at 8192 and 48 kHz
    /// this is 8 bands, all below 76 Hz.
    starved: usize,
}

impl BandLayout {
    /// Lay the axis out for `sample_rate` across the two shipped window sizes.
    pub fn new(sample_rate: u32) -> Self {
        Self::with_windows(sample_rate as f32, WINDOW_SIZE, LOW_WINDOW_SIZE)
    }

    /// The layout proper, parameterized on both window lengths so the tests can
    /// measure a candidate window without a rebuild.
    #[allow(
        clippy::indexing_slicing,
        reason = "edges_hz is a fixed SPECTRUM_BINS+1 array and k+1 <= SPECTRUM_BINS in every loop; bins is SPECTRUM_BINS long and k stays below it"
    )]
    fn with_windows(sr: f32, short_window: usize, long_window: usize) -> Self {
        let hi = BAND_HI_HZ.min(sr * 0.45);
        let ratio = hi / BAND_LO_HZ;
        let mut edges_hz = [0.0f32; SPECTRUM_BINS + 1];
        for (k, edge) in edges_hz.iter_mut().enumerate() {
            *edge = BAND_LO_HZ * ratio.powf(k as f32 / SPECTRUM_BINS as f32);
        }

        let short_bin_hz = sr / short_window as f32;
        let long_bin_hz = sr / long_window as f32;
        let short_mags = short_window / 2;
        let long_mags = long_window / 2;

        // The crossover: the first band the short window resolves on its own.
        // Band width grows monotonically with k, so the unresolvable bands are
        // a prefix and counting them is the same as finding the boundary.
        let mut crossover_band = 0;
        for k in 0..SPECTRUM_BINS {
            if edges_hz[k + 1] - edges_hz[k] < short_bin_hz {
                crossover_band = k + 1;
            }
        }

        let to_bin =
            |f: f32, bin_hz: f32, mags: usize| ((f / bin_hz).round() as usize).clamp(1, mags);

        let mut bins = [(1usize, 2usize); SPECTRUM_BINS];
        let mut starved = 0usize;

        // Each region chains its own `prev_hi` so bands stay contiguous; the
        // two chains are independent because their bins index different windows.
        let mut fill = |range: std::ops::Range<usize>, bin_hz: f32, mags: usize| {
            let mut prev_hi = to_bin(edges_hz[range.start], bin_hz, mags);
            let mut widened = 0usize;
            for k in range {
                let lo = prev_hi.max(to_bin(edges_hz[k], bin_hz, mags));
                let mut hi = to_bin(edges_hz[k + 1], bin_hz, mags);
                if hi <= lo {
                    // A band narrower than one bin. Widened to stay non-empty;
                    // above the crossover this cannot fire, which is what the
                    // crossover *means* and what `short_region_is_never_starved`
                    // asserts.
                    hi = (lo + 1).min(mags);
                    widened += 1;
                }
                bins[k] = (lo, hi);
                prev_hi = hi;
            }
            widened
        };

        starved += fill(0..crossover_band, long_bin_hz, long_mags);
        let short_widened = fill(crossover_band..SPECTRUM_BINS, short_bin_hz, short_mags);
        debug_assert_eq!(
            short_widened, 0,
            "the crossover guarantees every short-window band is at least one bin wide"
        );

        let long_bins_used = if crossover_band == 0 {
            0
        } else {
            bins[crossover_band - 1].1
        };

        Self {
            bins,
            edges_hz,
            crossover_band,
            long_bins_used,
            starved,
        }
    }

    /// Where a band's magnitudes come from.
    pub fn source(&self, band: usize) -> BandSource {
        if band < self.crossover_band {
            BandSource::Long
        } else {
            BandSource::Short
        }
    }

    /// First band that reads the short window.
    pub fn crossover_band(&self) -> usize {
        self.crossover_band
    }

    /// Lower edge of the crossover band, in Hz.
    pub fn crossover_hz(&self) -> f32 {
        self.edges_hz
            .get(self.crossover_band)
            .copied()
            .unwrap_or(BAND_HI_HZ)
    }

    /// Low bands the long window still cannot resolve — see [`Self::starved`]'s
    /// field docs. Surfaced so the docs can quote a measurement.
    pub fn starved(&self) -> usize {
        self.starved
    }

    /// The log-frequency band that contains `hz`: the last band whose lower
    /// edge is at or below it.
    pub fn band_for_freq(&self, hz: f32) -> usize {
        (0..SPECTRUM_BINS)
            .rev()
            .find(|&k| self.edges_hz.get(k).is_some_and(|&e| e <= hz))
            .unwrap_or(0)
    }
}

/// Windowed FFT plus a fixed log-frequency band mapping, reused every hop.
pub struct SpectrumAnalyzer {
    short_fft: Arc<dyn Fft<f32>>,
    long_fft: Arc<dyn Fft<f32>>,
    short_hann: [f32; WINDOW_SIZE],
    /// The long window's Hann taper and buffers live on the heap: at 8192 they
    /// are 32 KB apiece, and `Analyzer` is constructed and moved by value.
    long_hann: Vec<f32>,
    short_buf: Vec<Complex<f32>>,
    short_scratch: Vec<Complex<f32>>,
    long_buf: Vec<Complex<f32>>,
    long_scratch: Vec<Complex<f32>>,
    mags: [f32; MAG_BINS],
    long_mags: Vec<f32>,
    layout: BandLayout,
    /// Scales a Hann-windowed peak magnitude back to sine amplitude
    /// (Hann coherent gain 1/2, one-sided spectrum 2/N => 4/N). Per window, so
    /// a tone reads the same amplitude on either side of the crossover.
    short_norm: f32,
    long_norm: f32,
}

impl SpectrumAnalyzer {
    /// Plan both FFTs and precompute the Hann windows and band layout for
    /// `sample_rate`.
    pub fn new(sample_rate: u32) -> Self {
        let mut planner = FftPlanner::new();
        let short_fft = planner.plan_fft_forward(WINDOW_SIZE);
        let long_fft = planner.plan_fft_forward(LOW_WINDOW_SIZE);
        let short_scratch_len = short_fft.get_inplace_scratch_len();
        let long_scratch_len = long_fft.get_inplace_scratch_len();

        let mut short_hann = [0.0f32; WINDOW_SIZE];
        for (i, w) in short_hann.iter_mut().enumerate() {
            *w = hann_at(i, WINDOW_SIZE);
        }
        let long_hann: Vec<f32> = (0..LOW_WINDOW_SIZE)
            .map(|i| hann_at(i, LOW_WINDOW_SIZE))
            .collect();

        Self {
            short_fft,
            long_fft,
            short_hann,
            long_hann,
            short_buf: vec![Complex::new(0.0, 0.0); WINDOW_SIZE],
            short_scratch: vec![Complex::new(0.0, 0.0); short_scratch_len],
            long_buf: vec![Complex::new(0.0, 0.0); LOW_WINDOW_SIZE],
            long_scratch: vec![Complex::new(0.0, 0.0); long_scratch_len],
            mags: [0.0; MAG_BINS],
            long_mags: vec![0.0; LOW_MAG_BINS],
            layout: BandLayout::new(sample_rate),
            short_norm: 4.0 / WINDOW_SIZE as f32,
            long_norm: 4.0 / LOW_WINDOW_SIZE as f32,
        }
    }

    /// FFT both windows and return the log-frequency band spectrum. Band value
    /// is the peak bin in the band, so a pure tone reads near its amplitude
    /// regardless of band width — and regardless of which window resolved it.
    ///
    /// `long` is expected to be [`LOW_WINDOW_SIZE`] samples; a shorter slice
    /// simply leaves the tail of the transform zeroed rather than panicking.
    #[allow(
        clippy::indexing_slicing,
        reason = "buf/mags are indexed within their own iterators' bounds, and every (lo, hi) comes from BandLayout, which clamps both to the source window's magnitude count"
    )]
    pub fn analyze(&mut self, short: &[f32; WINDOW_SIZE], long: &[f32]) -> [f32; SPECTRUM_BINS] {
        for (i, (s, w)) in short.iter().zip(self.short_hann.iter()).enumerate() {
            self.short_buf[i] = Complex::new(s * w, 0.0);
        }
        self.short_fft
            .process_with_scratch(&mut self.short_buf, &mut self.short_scratch);
        for (i, m) in self.mags.iter_mut().enumerate() {
            *m = self.short_buf[i].norm() * self.short_norm;
        }

        // Only the bins below the crossover are ever read, so the magnitude
        // conversion stops there — ~42 of 4096 bins at 48 kHz.
        let used = self.layout.long_bins_used;
        if used > 0 {
            for (i, slot) in self.long_buf.iter_mut().enumerate() {
                let s = long.get(i).copied().unwrap_or(0.0);
                let w = self.long_hann.get(i).copied().unwrap_or(0.0);
                *slot = Complex::new(s * w, 0.0);
            }
            self.long_fft
                .process_with_scratch(&mut self.long_buf, &mut self.long_scratch);
            for (i, m) in self.long_mags.iter_mut().enumerate().take(used) {
                *m = self.long_buf[i].norm() * self.long_norm;
            }
        }

        let mut bands = [0.0f32; SPECTRUM_BINS];
        for (k, band) in bands.iter_mut().enumerate() {
            let (lo, hi) = self.layout.bins[k];
            let src = match self.layout.source(k) {
                BandSource::Long => &self.long_mags[..],
                BandSource::Short => &self.mags[..],
            };
            *band = src[lo..hi].iter().fold(0.0f32, |a, &b| a.max(b));
        }
        bands
    }

    /// Normalized linear magnitudes of the most recent `analyze` call, from the
    /// **short** window — consumed by onset detection, whose transient response
    /// is deliberately not slowed by the long window.
    pub fn magnitudes(&self) -> &[f32; MAG_BINS] {
        &self.mags
    }

    /// The band layout this analyzer resolved for its sample rate.
    pub fn layout(&self) -> &BandLayout {
        &self.layout
    }

    /// The log-frequency band index that contains `hz`.
    pub fn band_for_freq(&self, hz: f32) -> usize {
        self.layout.band_for_freq(hz)
    }
}

/// Hann taper value at sample `i` of an `n`-long window.
fn hann_at(i: usize, n: usize) -> f32 {
    let phase = i as f32 / (n.max(2) - 1) as f32;
    0.5 - 0.5 * (std::f32::consts::TAU * phase).cos()
}

#[cfg(test)]
#[allow(
    clippy::indexing_slicing,
    reason = "the module's hot-path pragma also covers the tests; here every index is a literal or a loop bound inside a fixed SPECTRUM_BINS(+1) array, and a panic is the intended failure anyway"
)]
mod tests {
    use super::*;

    const SR: f32 = 48_000.0;

    /// The layout as it was before ADR-0049: one window, natural log edges, and
    /// a cumulative fix-up that forced every collapsed edge to `previous + 1`.
    /// Kept here as the reference the "nothing above the crossover moved" claim
    /// is measured against, rather than as a description in a comment.
    fn v1_edges(sr: f32) -> [usize; SPECTRUM_BINS + 1] {
        let hi = BAND_HI_HZ.min(sr * 0.45);
        let ratio = hi / BAND_LO_HZ;
        let bin_hz = sr / WINDOW_SIZE as f32;
        let mut edges = [0usize; SPECTRUM_BINS + 1];
        for (k, edge) in edges.iter_mut().enumerate() {
            let f = BAND_LO_HZ * ratio.powf(k as f32 / SPECTRUM_BINS as f32);
            *edge = ((f / bin_hz).round() as usize).clamp(1, MAG_BINS);
        }
        for k in 1..edges.len() {
            if edges[k] <= edges[k - 1] {
                edges[k] = (edges[k - 1] + 1).min(MAG_BINS);
            }
        }
        edges
    }

    #[test]
    fn crossover_is_where_the_short_window_stops_resolving() {
        let layout = BandLayout::new(48_000);
        assert_eq!(
            layout.crossover_band(),
            20,
            "at 48 kHz the 2048 window resolves from band 20 up"
        );
        // ~246 Hz: derived from the axis, yet it lands within 2 % of the
        // independently chosen BASS_HI_HZ = 250. Worth pinning as a fact, not
        // as a coincidence someone might "tidy".
        let hz = layout.crossover_hz();
        assert!(
            (240.0..250.0).contains(&hz),
            "crossover should sit just under the 250 Hz bass split, got {hz}"
        );

        // Non-vacuity: every band below the crossover really is narrower than a
        // short-window bin, and every band above really is at least as wide.
        let short_bin_hz = SR / WINDOW_SIZE as f32;
        for k in 0..SPECTRUM_BINS {
            let width = layout.edges_hz[k + 1] - layout.edges_hz[k];
            if k < layout.crossover_band() {
                assert!(
                    width < short_bin_hz,
                    "band {k} width {width} should be under one {short_bin_hz} Hz bin"
                );
            } else {
                assert!(
                    width >= short_bin_hz,
                    "band {k} width {width} should reach one {short_bin_hz} Hz bin"
                );
            }
        }
    }

    #[test]
    fn above_the_chain_every_edge_is_bit_identical_to_v1() {
        let layout = BandLayout::new(48_000);
        let v1 = v1_edges(SR);

        // The v1 fix-up chain overshot the log curve and only died at band 32,
        // so v1's edges for bands 20..31 were artifacts of the collapse
        // handling rather than of the layout. From 32 up, v1 *was* the natural
        // curve — and there this plan changes nothing at all. Half the axis,
        // bit-identical.
        for k in 32..SPECTRUM_BINS {
            let (lo, hi) = layout.bins[k];
            assert_eq!(
                (lo, hi),
                (v1[k], v1[k + 1]),
                "band {k} sits above the v1 fix-up chain and must not have moved"
            );
        }

        // And the counter-assertion that makes the above mean something: bands
        // 20..31 *did* move, and by far more than rounding. Without this, the
        // test would pass just as well if the crossover had swallowed the whole
        // axis, or if the layout had simply reproduced v1.
        let moved: Vec<usize> = (layout.crossover_band()..SPECTRUM_BINS)
            .filter(|&k| layout.bins[k] != (v1[k], v1[k + 1]))
            .collect();
        assert_eq!(
            moved,
            (20..32).collect::<Vec<_>>(),
            "exactly bands 20 to 31 should have left their v1 fix-up positions"
        );
        assert_eq!(
            layout.bins[20],
            (11, 12),
            "band 20 should sit at its natural bins 11..12, not v1's forced 21..22"
        );
    }

    #[test]
    fn short_region_is_never_starved_and_the_low_region_is_measured() {
        let layout = BandLayout::new(48_000);
        // Every band is non-empty, whichever window it came from.
        for k in 0..SPECTRUM_BINS {
            let (lo, hi) = layout.bins[k];
            assert!(
                hi > lo,
                "band {k} must span at least one bin, got {lo}..{hi}"
            );
        }
        // The long window resolves all but the bottom handful; those are the
        // ones physics does not allow at this window length, and the number is
        // quoted in the docs rather than left vague.
        assert_eq!(
            layout.starved(),
            8,
            "at 8192 and 48 kHz exactly 8 sub-76 Hz bands stay one bin wide"
        );
    }

    #[test]
    fn the_long_window_was_chosen_by_measurement() {
        // The plan's rule: 4096 first, 8192 only if 4096 still leaves sub-bass
        // bands bin-starved. This records the measurement that decided it, so a
        // later reader can re-derive the choice instead of trusting a commit
        // message.
        let starved_at = |long: usize| BandLayout::with_windows(SR, WINDOW_SIZE, long).starved();
        // 4096 widens *all twenty* low bands — it buys nothing the 2048 window
        // did not already fail at, which is what made the choice unambiguous
        // rather than a judgement call.
        assert_eq!(starved_at(4096), 20, "4096 resolves none of the low region");
        assert_eq!(
            starved_at(8192),
            8,
            "8192 pulls the boundary down to ~76 Hz"
        );
        assert_eq!(
            starved_at(16_384),
            0,
            "16384 would resolve all of it, at 171 ms of group delay"
        );
        assert!(
            starved_at(LOW_WINDOW_SIZE) < starved_at(4096),
            "the shipped window must beat the one the plan tried first"
        );
    }

    #[test]
    fn a_tone_reads_its_amplitude_on_either_side_of_the_crossover() {
        // The two windows carry different norms (4/N each); if they disagreed,
        // the axis would step in level at the crossover. 120 Hz is long-window
        // territory, 400 Hz short-window, and a 0.8 sine must read ~0.8 in both.
        //
        // Read as the spectrum's peak rather than the band containing the tone:
        // near the crossover a band is only one or two bins wide, so a tone
        // sitting at a band edge legitimately splits its energy with its
        // neighbour. The level is the claim here, not the placement — that is
        // `band_for_freq_agrees_with_the_edge_table`'s job.
        let mut readings = Vec::new();
        for freq in [120.0f32, 400.0] {
            let mut an = SpectrumAnalyzer::new(48_000);
            let tone = |i: usize| 0.8 * (std::f32::consts::TAU * freq * i as f32 / SR).sin();
            let short: [f32; WINDOW_SIZE] = std::array::from_fn(tone);
            let long: Vec<f32> = (0..LOW_WINDOW_SIZE).map(tone).collect();
            let bands = an.analyze(&short, &long);

            let (peak_band, peak) = bands
                .iter()
                .enumerate()
                .max_by(|a, b| a.1.total_cmp(b.1))
                .map(|(k, &v)| (k, v))
                .unwrap_or((0, 0.0));
            assert!(
                (0.6..=1.0).contains(&peak),
                "a 0.8 sine at {freq} Hz should peak near 0.8, got {peak} in band {peak_band}"
            );
            // The peak is where the tone is, give or take the edge-splitting
            // above — so this stays a real placement check without being brittle.
            let expected = an.band_for_freq(freq);
            assert!(
                peak_band.abs_diff(expected) <= 1,
                "a {freq} Hz tone should peak at or beside band {expected}, got {peak_band}"
            );
            readings.push(peak);
        }
        // The continuity claim proper: the two windows agree on level to within
        // Hann edge-splitting, so nothing steps at the crossover.
        let (lo, hi) = (readings[0], readings[1]);
        assert!(
            (lo - hi).abs() < 0.2,
            "the two windows must agree on a 0.8 tone's level: long read {lo}, short read {hi}"
        );
    }

    /// Plan 0048 Phase 1 / ADR-0049: the 808-collapse reproduction, inverted.
    ///
    /// A tone stepped across the sub-bass and bass region must climb the axis one
    /// band at a time instead of parking in one or two. The bound it has to beat
    /// is **derived, not chosen**: before the dual-resolution axis every band down
    /// here was a single short-window bin, so the region could only ever resolve
    /// as many distinct bands as there are `sample_rate / WINDOW_SIZE` bins in it.
    ///
    /// Two deliberate choices about the instrument. **Stepped tones, not a glide:**
    /// the long window integrates 171 ms, so a fast sweep is genuinely smeared
    /// across it — the physics ADR-0049 accepts — and measuring the *axis* means
    /// holding each frequency steady. **Read here rather than through `Analyzer`:**
    /// this reads the raw band array, because the analyzer publishes a per-band
    /// normalized one on which an argmax is meaningless — a tone's own band and
    /// both neighbours all saturate at 1.0, so the peak index is arbitrary among
    /// them. Placement is a property of the layout, and this is the layout's test.
    #[test]
    fn a_tone_stepped_through_the_bass_region_climbs_distinct_bands() {
        const LO_HZ: f32 = 40.0;
        const HI_HZ: f32 = 200.0;

        let mut an = SpectrumAnalyzer::new(48_000);
        let peak_band_at = |an: &mut SpectrumAnalyzer, freq: f32| -> usize {
            let tone = |i: usize| 0.8 * (std::f32::consts::TAU * freq * i as f32 / SR).sin();
            let short: [f32; WINDOW_SIZE] = std::array::from_fn(tone);
            let long: Vec<f32> = (0..LOW_WINDOW_SIZE).map(tone).collect();
            an.analyze(&short, &long)
                .iter()
                .enumerate()
                .max_by(|a, b| a.1.total_cmp(b.1))
                .map(|(k, _)| k)
                .unwrap_or(0)
        };

        let steps = 64;
        let visited: Vec<usize> = (0..=steps)
            .map(|i| LO_HZ + (HI_HZ - LO_HZ) * i as f32 / steps as f32)
            .map(|f| peak_band_at(&mut an, f))
            .collect();

        // Smooth: the peak band never walks backwards as the tone rises.
        for pair in visited.windows(2) {
            if let [a, b] = pair {
                assert!(
                    b >= a,
                    "the peak band must not fall as frequency rises: {visited:?}"
                );
            }
        }

        let distinct = {
            let mut v = visited.clone();
            v.dedup();
            v.len()
        };

        // The v1 ceiling: one short-window bin per band meant at most this many
        // distinct bands were reachable between LO_HZ and HI_HZ, however many log
        // bands nominally sat there.
        let bin_hz = SR / WINDOW_SIZE as f32;
        let v1_ceiling = (HI_HZ / bin_hz).floor() as usize - (LO_HZ / bin_hz).floor() as usize + 1;
        assert_eq!(
            v1_ceiling, 8,
            "fixture sanity: the old axis could show 8 bands across this span"
        );
        assert!(
            distinct > v1_ceiling,
            "the dual-resolution axis should resolve more than the {v1_ceiling} bands a single \
             {WINDOW_SIZE} window could, got {distinct}: {visited:?}"
        );

        // And the span really is spread across the axis rather than nudged: most
        // of the log bands nominally covering 40-200 Hz should appear.
        let layout = BandLayout::new(48_000);
        let nominal = layout.band_for_freq(HI_HZ) - layout.band_for_freq(LO_HZ) + 1;
        assert!(
            distinct * 2 >= nominal,
            "40-200 Hz spans {nominal} log bands and should light up most of them, \
             got {distinct}: {visited:?}"
        );
    }

    #[test]
    fn band_for_freq_agrees_with_the_edge_table() {
        let layout = BandLayout::new(48_000);
        for k in 0..SPECTRUM_BINS {
            // A frequency just inside band k's span must map back to k.
            let lo = layout.edges_hz[k];
            let hi = layout.edges_hz[k + 1];
            let mid = (lo + hi) * 0.5;
            assert_eq!(
                layout.band_for_freq(mid),
                k,
                "midpoint of band {k} ({mid} Hz)"
            );
        }
        // Below the axis clamps to band 0 rather than wrapping or panicking.
        assert_eq!(layout.band_for_freq(1.0), 0);
    }
}
