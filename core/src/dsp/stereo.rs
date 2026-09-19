//! The stereo field: where the mix sits between the two channels, and how far
//! apart they are (ADR-0215).
//!
//! **Absolute, never levelled.** Nothing here is divided by a running peak, the
//! way [`gain`](super::gain) divides the bands: `balance = 0` means genuinely
//! centred, so a source carrying no stereo information reads still instead of
//! stretching its noise floor into a confident wandering pan. The price is a
//! narrow usable range on real material, which `shot --report` publishes.
//!
//! **Nothing upstream reads this module, and it reads nothing but the hop's two
//! channels.** The mono analysis stays a function of the channel average alone,
//! so every field that existed before this module reads bit-identically for a
//! stereo stream and for the mono-duplicated stream carrying its channel
//! average — `core/tests/dsp.rs` holds that as an assertion.

// Hot-path panic-denial pragma (Plan 0002 Phase 2). Runs every analysis hop.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

use super::gain::WAVE_FLOOR;

/// One hop's whole-mix stereo field.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct StereoField {
    /// Where the mix sits, `-1` hard left to `+1` hard right, `0` centred.
    pub balance: f32,
    /// How decorrelated the two channels are: `0` identical, `0.5` fully
    /// decorrelated, `1` polarity-inverted.
    pub spread: f32,
}

impl StereoField {
    /// Measure one hop from channels 0 and 1.
    ///
    /// `balance` is the ratio of the two channels' RMS through [`ratio`], and
    /// `spread` is `(1 - corr) / 2` over the normalized correlation
    /// `Σ(l·r) / sqrt(Σl² · Σr²)`. Both read exactly `0` when the larger
    /// channel's RMS sits below [`WAVE_FLOOR`], and identical channels read
    /// exactly `0` for both: the correlation's numerator and denominator are
    /// then the same sum, and `sqrt(a·a)` is exactly `a` under round-to-nearest.
    ///
    /// Only the overlapping prefix of the two slices is read, so a caller that
    /// has one channel shorter measures what it has rather than reaching past
    /// it. The sums stay far from overflow at hop length: every sample of a
    /// validated stream is finite and the squares of a `±1` hop total at most
    /// its own sample count.
    pub fn measure(left: &[f32], right: &[f32]) -> Self {
        let n = left.len().min(right.len());
        if n == 0 {
            return Self::default();
        }
        let mut sum_ll = 0.0f32;
        let mut sum_rr = 0.0f32;
        let mut sum_lr = 0.0f32;
        for (l, r) in left.iter().zip(right).take(n) {
            sum_ll += l * l;
            sum_rr += r * r;
            sum_lr += l * r;
        }
        let inv_n = 1.0 / n as f32;
        let rms_l = (sum_ll * inv_n).sqrt();
        let rms_r = (sum_rr * inv_n).sqrt();
        if rms_l.max(rms_r) < WAVE_FLOOR {
            // Below the floor the ratio of two noise floors is noise. Zero is
            // the truth about a silent hop's position, and the only value that
            // cannot be mistaken for one.
            return Self::default();
        }
        // Floored rather than guarded: the branch above already covers silence,
        // and this keeps the division total on its own terms.
        let corr = sum_lr / (sum_ll * sum_rr).sqrt().max(f32::MIN_POSITIVE);
        Self {
            balance: ratio(rms_l, rms_r),
            // One channel under the floor beside a loud one leaves the
            // correlation undefined; the floored denominator reads it as `0`,
            // which is the fully-decorrelated midpoint — a hard pan is a wide
            // image, not a narrow one.
            spread: (1.0 - corr.clamp(-1.0, 1.0)) * 0.5,
        }
    }
}

/// The absolute left/right ratio of two non-negative magnitudes, `-1..=1`,
/// negative to the left.
///
/// **The one place the floor is applied**, so the whole-mix field and the
/// per-band one cannot disagree about what silence reads: exactly `0` when the
/// larger of the two sits below [`WAVE_FLOOR`], rather than a ratio of noise.
/// The division itself is floored as well as guarded, so it is total even if
/// the guard above it ever moves.
pub fn ratio(left: f32, right: f32) -> f32 {
    if left.max(right) < WAVE_FLOOR {
        return 0.0;
    }
    (right - left) / (left + right).max(WAVE_FLOOR)
}

/// The per-band field: [`ratio`] taken over each band's own per-channel energy,
/// `(bass, mid, treb)` in and out.
///
/// The two triples come from [`BandSplitter::split`](super::bands::BandSplitter)
/// run on each channel's spectrum, so the band edges are **by construction**
/// the ones `bass`/`mid`/`treb` already use rather than a second set that
/// happens to agree. A band whose louder channel sits below [`WAVE_FLOOR`]
/// reads exactly `0`: the treble of a bass-only source has no position, and a
/// ratio of two leakage floors would invent one.
pub fn band_balance(left: (f32, f32, f32), right: (f32, f32, f32)) -> (f32, f32, f32) {
    (
        ratio(left.0, right.0),
        ratio(left.1, right.1),
        ratio(left.2, right.2),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::AudioFormat;
    use crate::dsp::{Analyzer, HOP_SIZE, WARMUP_HOPS};

    /// A 512-sample hop of a sine, so a pair can be built at two gains.
    fn hop(amp: f32) -> Vec<f32> {
        (0..HOP_SIZE)
            .map(|i| amp * (std::f32::consts::TAU * 8.0 * i as f32 / HOP_SIZE as f32).sin())
            .collect()
    }

    /// The far landmark no `--signal` kind can reach: two channels that cancel
    /// exactly. `spread` is the only quantity that distinguishes it from a
    /// centred mono signal — `balance` reads `0` for both.
    #[test]
    fn a_polarity_inverted_pair_reads_the_top_of_the_spread_range() {
        let left = hop(0.5);
        let right: Vec<f32> = left.iter().map(|s| -s).collect();
        let field = StereoField::measure(&left, &right);
        assert!(
            (field.spread - 1.0).abs() < 1e-4,
            "an inverted pair must read the top of the range, got {}",
            field.spread
        );
        assert!(
            field.balance.abs() < 1e-4,
            "an inverted pair is still centred, got {}",
            field.balance
        );
    }

    /// Identical channels are the whole of the compatibility case: every
    /// pre-existing `--signal` kind and every mono stream lands here, and both
    /// quantities have to read **exactly** zero rather than near it.
    #[test]
    fn identical_channels_read_exactly_zero_for_both() {
        let left = hop(0.5);
        let field = StereoField::measure(&left, &left);
        assert_eq!(field, StereoField::default());
    }

    /// A one-waveform pair at two gains: the RMS ratio is the gain ratio, so
    /// `balance` is algebraic rather than fitted, and the correlation is 1.
    #[test]
    fn a_gain_ratio_is_the_balance_and_costs_no_spread() {
        // `p = -0.8`'s own gains — the stimulus the harness synthesizes.
        let left = hop(0.9);
        let right: Vec<f32> = left.iter().map(|s| s * (0.2 / 1.8)).collect();
        let field = StereoField::measure(&left, &right);
        assert!(
            (field.balance + 0.8).abs() < 1e-3,
            "gains 1.0 / 0.111 should read -0.800, got {}",
            field.balance
        );
        assert!(
            field.spread.abs() < 1e-4,
            "one waveform at two gains is perfectly correlated, got {}",
            field.spread
        );
    }

    /// Silence is a position nothing can be inferred from, and the floor says so
    /// rather than dividing one noise floor by another.
    #[test]
    fn a_sub_floor_hop_reads_zero_rather_than_a_ratio_of_noise() {
        let silent = vec![0.0f32; HOP_SIZE];
        assert_eq!(
            StereoField::measure(&silent, &silent),
            StereoField::default()
        );
        // A whisper an eighth of the floor, hard panned: still nothing.
        let whisper: Vec<f32> = hop(WAVE_FLOOR * 0.125);
        let field = StereoField::measure(&silent, &whisper);
        assert_eq!(field, StereoField::default());
        assert!(field.balance.is_finite() && field.spread.is_finite());
        // ...and an empty hop is a measurement of nothing, not a division by it.
        assert_eq!(StereoField::measure(&[], &[]), StereoField::default());
    }

    /// The per-band field shares the whole-mix field's floor, which is the only
    /// thing standing between `treb_balance` on a bass-only source and a
    /// confident position derived from two spectral leakage floors.
    #[test]
    fn a_band_below_the_floor_has_no_position() {
        let loud = (0.05, 0.02, WAVE_FLOOR * 0.5);
        let quiet = (0.01, 0.02, WAVE_FLOOR * 0.1);
        let (bass, mid, treb) = band_balance(loud, quiet);
        assert!(
            bass < -0.5,
            "a band with energy keeps its ratio, got {bass}"
        );
        assert_eq!(mid, 0.0, "equal energy is centred");
        assert_eq!(treb, 0.0, "a band under the floor reads exactly zero");
    }

    /// The claim a one-channel stream makes: channel 1 is filled from channel 0
    /// exactly as `waveform_pair` fills it, so the field reads `0` — the truth
    /// about a stream that carries no position at all.
    #[test]
    fn a_one_channel_stream_reads_a_centred_narrow_field() {
        let format = AudioFormat {
            sample_rate: 48_000,
            channels: 1,
        };
        let mut analyzer = Analyzer::new(format).expect("valid format");
        let pcm = crate::signal::noise(3, 1.0, 0.8, format);
        let mut hops = 0usize;
        for chunk in pcm.chunks(HOP_SIZE) {
            analyzer.push_interleaved(chunk);
            let frame = analyzer.take_frame();
            hops += 1;
            if hops <= WARMUP_HOPS {
                continue;
            }
            assert_eq!(
                (
                    frame.balance,
                    frame.spread,
                    frame.bass_balance,
                    frame.mid_balance,
                    frame.treb_balance
                ),
                (0.0, 0.0, 0.0, 0.0, 0.0),
                "a one-channel stream has no stereo field to read"
            );
        }
        assert!(hops > WARMUP_HOPS, "the clip never cleared warm-up");
    }
}
