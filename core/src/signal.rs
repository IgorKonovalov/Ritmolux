//! Pure, deterministic PCM signal synthesis for the capture / visual-QA path
//! (Plan 0013). These generate synthetic *signals* by math over a sample clock —
//! they are **not** an audio *source* (no WASAPI, no file, no OS), so they live
//! in the source-agnostic core and can be fed straight through the real
//! [`Analyzer`](crate::dsp::Analyzer) to exercise the actual DSP.
//!
//! Every generator is a pure function of its arguments — no wall clock, seeded
//! randomness only (NFR section 6). Output is interleaved f32 frames matching
//! the given [`AudioFormat`], the same shape a frontend pushes into the intake.

use std::f32::consts::TAU;

use crate::audio::AudioFormat;

/// A pure sine at `freq_hz` and `amplitude` for `secs`, interleaved to
/// `format.channels`.
pub fn sine(freq_hz: f32, secs: f32, amplitude: f32, format: AudioFormat) -> Vec<f32> {
    let sr = format.sample_rate as f32;
    let n = frame_count(secs, format.sample_rate);
    let mono: Vec<f32> = (0..n)
        .map(|i| amplitude * (TAU * freq_hz * i as f32 / sr).sin())
        .collect();
    interleave(&mono, format.channels)
}

/// A strong low-frequency sine (bass band). Thin wrapper over [`sine`].
pub fn bass_sine(freq_hz: f32, secs: f32, format: AudioFormat) -> Vec<f32> {
    sine(freq_hz, secs, 0.9, format)
}

/// A strong high-frequency sine (treble band). Thin wrapper over [`sine`].
pub fn treble_tone(freq_hz: f32, secs: f32, format: AudioFormat) -> Vec<f32> {
    sine(freq_hz, secs, 0.9, format)
}

/// Seeded white noise in `[-amplitude, amplitude]`, deterministic per `seed`.
pub fn noise(seed: u64, secs: f32, amplitude: f32, format: AudioFormat) -> Vec<f32> {
    let n = frame_count(secs, format.sample_rate);
    let mut rng = SplitMix::new(seed);
    let mono: Vec<f32> = (0..n)
        .map(|_| (rng.next_f32() * 2.0 - 1.0) * amplitude)
        .collect();
    interleave(&mono, format.channels)
}

/// A sum of sines at `freqs` for `secs`, scaled so the peak stays within ±0.9.
pub fn chord(freqs: &[f32], secs: f32, format: AudioFormat) -> Vec<f32> {
    let sr = format.sample_rate as f32;
    let n = frame_count(secs, format.sample_rate);
    let scale = if freqs.is_empty() {
        0.0
    } else {
        0.9 / freqs.len() as f32
    };
    let mono: Vec<f32> = (0..n)
        .map(|i| {
            let t = i as f32 / sr;
            freqs.iter().map(|f| (TAU * f * t).sin()).sum::<f32>() * scale
        })
        .collect();
    interleave(&mono, format.channels)
}

/// A metronome click track at `bpm` for `secs`: a short decaying broadband burst
/// on each beat, silence between. Fed through the real analyzer it produces an
/// onset (and a beat flag) on each click, ~`60/bpm` seconds apart.
pub fn click_track(bpm: f32, secs: f32, format: AudioFormat) -> Vec<f32> {
    let sr = format.sample_rate as f32;
    let n = frame_count(secs, format.sample_rate);
    let period = ((60.0 / bpm.max(1.0)) * sr).round() as usize;
    let click_len = ((0.012 * sr).round() as usize).max(1); // ~12 ms
    let mut rng = SplitMix::new(0x1234_5678_9ABC_DEF0);
    let mut mono = vec![0.0f32; n];
    let mut start = 0usize;
    while start < n {
        for i in 0..click_len {
            let idx = start + i;
            if idx >= n {
                break;
            }
            let env = (-(i as f32) / click_len as f32 * 6.0).exp();
            let sample = (rng.next_f32() * 2.0 - 1.0) * env * 0.95;
            if let Some(slot) = mono.get_mut(idx) {
                *slot = sample;
            }
        }
        start += period.max(1);
    }
    interleave(&mono, format.channels)
}

/// An envelope-shaped, beat-gridded signal with **dynamics** at `bpm` for
/// `secs` — the one generator here that rises and falls (Plan 0037, ADR-0039).
///
/// Every other generator is a steady tone or steady noise: measured through the
/// band report, `bass:60` gives min/mean/max 0.187 / 0.187 / 0.187, zero
/// variance, and `chord` 0.058 / 0.059 / 0.060. A filmstrip of those exercises
/// the DSP with material that never changes, which is not what any preset is
/// authored against.
///
/// Three layers on a beat grid, each landing in a different band, plus a
/// **phrase envelope**: an 8-beat cycle that builds over six beats and rests for
/// two. The rest is what produces real dynamics — without a near-silent stretch
/// the running mean climbs to meet the peak and `max / mean` collapses toward 1.
///
/// - **kick**, every beat: a pitch-dropping low sine, ~105 Hz down to ~45 — the
///   bass band, and the transient the onset detector fires on.
/// - **hat**, on each off-beat: a very short broadband tick — the treble band.
/// - **pad**, continuous: a three-note chord around 220-330 Hz that swells across
///   each beat — the mid band.
///
/// **It exercises dynamics; it is not evidence about real loopback levels.**
/// Nothing synthesized here can be — only a measurement of real material through
/// `--audio` speaks to that (`docs/capturing.md`).
///
/// A pure function of its arguments like every generator here: no wall clock,
/// and the hat's noise comes from a fixed seed pulled once per sample, so the
/// sequence is identical on every run and every machine (NFR section 6).
pub fn dynamic_groove(bpm: f32, secs: f32, format: AudioFormat) -> Vec<f32> {
    let sr = format.sample_rate as f32;
    let n = frame_count(secs, format.sample_rate);
    let beat_secs = 60.0 / bpm.max(1.0);
    let beat_samples = ((beat_secs * sr).round() as usize).max(1);
    let mut rng = SplitMix::new(0x5EED_0037_D17A_71C5);
    // The kick's phase is integrated rather than evaluated at `t`, because its
    // frequency changes within the beat — `sin(TAU * f(t) * t)` would sweep the
    // wrong way.
    let mut kick_phase = 0.0f32;
    let mut prev_white = 0.0f32;
    let mut mono = vec![0.0f32; n];

    for (i, slot) in mono.iter_mut().enumerate() {
        let beat = i / beat_samples;
        let within = (i % beat_samples) as f32 / beat_samples as f32;
        let since = within * beat_secs;

        // The phrase: six beats building, two resting. The build is geometric
        // rather than linear because the crest factor is the whole point — a
        // ramp that spends half its beats near the top has a mean close to its
        // maximum, which is the flatness every other generator here suffers
        // from. `0.04` rather than zero so the rest is quiet rather than digital
        // silence, which is what music does and what keeps the onset detector's
        // floor honest.
        let phrase = match beat % 8 {
            6 | 7 => 0.04,
            b => 0.18 * 1.4f32.powi(b as i32),
        };

        if i % beat_samples == 0 {
            kick_phase = 0.0;
        }
        let kick_hz = 45.0 + 60.0 * (-since * 45.0).exp();
        kick_phase += TAU * kick_hz / sr;
        let kick = kick_phase.sin() * (-since * 26.0).exp() * 0.45;

        // Pulled every sample, not only inside a tick, so the noise sequence does
        // not depend on where the eighth-note grid lands. Differenced against the
        // previous sample, which is a one-tap high-pass: flat white noise spends
        // most of its amplitude below 4 kHz where the kick and pad already live,
        // so an un-brightened tick costs peak headroom to light a band it barely
        // reaches. A hat is a bright sound; this makes it one.
        let white = rng.next_f32() * 2.0 - 1.0;
        let tick = white - prev_white;
        prev_white = white;
        // Hats on every eighth, the off-beat louder. A single 6 ms tick per beat
        // was measurable but pointless: at a ~1 % duty cycle the treble band's
        // mean over a hop reads 0.0002, which is silence with a good crest
        // factor. 90 ms of decay twice a beat is what puts real energy up there.
        let eighth = beat_secs * 0.5;
        let hat_t = since - eighth * (since / eighth).floor();
        let hat = tick * (-hat_t * 16.0).exp() * if within >= 0.5 { 2.6 } else { 1.6 };

        // Two voices a fifth apart, each with five harmonics at 1/k. The
        // harmonics are the point: the mid band is ~250 Hz-4 kHz and its scalar
        // is a MEAN over that whole span, so a bare three-note chord at 220-330
        // lands mostly in bass and reads as a trickle in mid. The stack spreads
        // energy from 165 Hz to 1.65 kHz, which is where a mix's body sits.
        let t = i as f32 / sr;
        // The per-harmonic phase offset is not decoration: with every partial
        // starting at zero they all align once per period and the pad's crest
        // factor sets the whole signal's peak, so the normalization below pulls
        // the kick and hats down with it. Detuned phases cost nothing and buy
        // back most of the headroom.
        let mut voices = 0.0f32;
        for f0 in [165.0f32, 247.5] {
            for k in 1..=5 {
                let kf = k as f32;
                voices += (TAU * f0 * kf * t + kf * 1.7).sin() / kf;
            }
        }
        let pad = voices * 0.4 * (0.35 + 0.65 * (1.0 - (-since * 6.0).exp()));

        // Soft-clipped rather than peak-normalized to the 0.9 headroom the other
        // generators use. Dividing by the loudest sample would make the three
        // layers a zero-sum game — every increase in the hats pulls the kick and
        // pad down by the same factor, so no setting lights all three bands. A
        // tanh saturator bounds the peak while leaving the average alone, which
        // is what a mix bus does; the phrase multiplies BEFORE it, so the rest
        // stays in the curve's linear region and the dynamics survive.
        *slot = ((kick + hat + pad) * phrase * 1.2).tanh() * 0.9;
    }

    interleave(&mono, format.channels)
}

/// Interleave a mono buffer up to `channels` (the same sample on every channel).
fn interleave(mono: &[f32], channels: u16) -> Vec<f32> {
    let ch = channels.max(1) as usize;
    let mut out = Vec::with_capacity(mono.len() * ch);
    for &s in mono {
        for _ in 0..ch {
            out.push(s);
        }
    }
    out
}

/// Whole frames in `secs` at `sample_rate` (non-negative).
fn frame_count(secs: f32, sample_rate: u32) -> usize {
    (secs.max(0.0) * sample_rate as f32).round() as usize
}

/// splitmix64 — a tiny seeded PRNG so noise/click generation stays deterministic
/// without a dependency (mirrors the render side's `SeededRng`).
struct SplitMix(u64);

impl SplitMix {
    fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn next_f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsp::{Analyzer, HOP_SIZE};

    fn fmt() -> AudioFormat {
        AudioFormat {
            sample_rate: 48_000,
            channels: 2,
        }
    }

    /// Run PCM through the real analyzer, returning the latest frame after the
    /// whole buffer.
    fn analyze_all(pcm: &[f32]) -> crate::dsp::AnalysisFrame {
        let mut an = Analyzer::new(fmt()).expect("valid format");
        an.push_interleaved(pcm);
        an.take_frame()
    }

    #[test]
    fn bass_sine_lands_in_the_bass_band() {
        let frame = analyze_all(&bass_sine(60.0, 1.0, fmt()));
        assert!(
            frame.bass > frame.treb,
            "60 Hz: bass {} should exceed treb {}",
            frame.bass,
            frame.treb
        );
        assert!(frame.bass > 0.05, "60 Hz sine should light the bass band");
    }

    #[test]
    fn treble_tone_lands_in_the_treble_band() {
        let frame = analyze_all(&treble_tone(12_000.0, 1.0, fmt()));
        assert!(
            frame.treb > frame.bass,
            "12 kHz: treb {} should exceed bass {}",
            frame.treb,
            frame.bass
        );
    }

    /// Min / mean / max of each band across a clip's hops, skipping the hops
    /// before the analyzer's window fills (they read zero for every generator and
    /// would drag every minimum to 0).
    fn band_ranges(pcm: &[f32], warmup: usize) -> [(f32, f32, f32); 3] {
        let format = fmt();
        let mut an = Analyzer::new(format).expect("valid format");
        let hop = HOP_SIZE * format.channels as usize;
        let mut bands = [Vec::new(), Vec::new(), Vec::new()];
        for (i, chunk) in pcm.chunks(hop).enumerate() {
            an.push_interleaved(chunk);
            let f = an.take_frame();
            // Past the analyzer's own warm-up (derived — see `WARMUP_HOPS`) plus
            // whatever settling the caller asked for on top.
            if i < crate::dsp::WARMUP_HOPS + warmup {
                continue;
            }
            // The **raw** levels, deliberately. These measurements are claims
            // about the *generator* — does this PCM have dynamics — and ADR-0049's
            // normalization exists precisely to flatten absolute dynamics away, so
            // reading the normalized values here would measure the AGC's crest
            // factor instead of the signal's.
            bands[0].push(f.bass_raw);
            bands[1].push(f.mid_raw);
            bands[2].push(f.treb_raw);
        }
        std::array::from_fn(|i| {
            let v = &bands[i];
            let min = v.iter().copied().fold(f32::INFINITY, f32::min);
            let max = v.iter().copied().fold(f32::NEG_INFINITY, f32::max);
            let mean = v.iter().sum::<f32>() / v.len().max(1) as f32;
            (min, mean, max)
        })
    }

    /// The property Plan 0037 Phase 3 exists for: this generator has **real
    /// dynamics**, where every other kind here is flat. No honest absolute
    /// threshold exists yet, so the claim is relative — `max / mean` materially
    /// above 1 in every band, against `bass:60`'s exactly 1.000 and `chord`'s
    /// 1.017 — and it is asserted against a steady kind measured the same way in
    /// the same run rather than against a remembered number.
    #[test]
    fn dynamic_groove_has_dynamics_where_the_steady_kinds_have_none() {
        let format = fmt();
        let groove = band_ranges(&dynamic_groove(110.0, 4.0, format), 4);
        // The liveliest existing kind, measured in the same run rather than
        // quoted from memory: seeded noise, whose `max / mean` the plan records
        // at 1.77 in bass where `bass:60` is exactly 1.000 and `chord` 1.017.
        let liveliest = band_ranges(&noise(7, 4.0, 0.8, format), 4);
        let names = ["bass", "mid", "treb"];

        for (i, (min, mean, max)) in groove.iter().copied().enumerate() {
            let crest = max / mean.max(f32::EPSILON);
            let (_, noise_mean, noise_max) = liveliest[i];
            let noise_crest = noise_max / noise_mean.max(f32::EPSILON);
            println!(
                "{:<5} min {min:.4} mean {mean:.4} max {max:.4}  max/mean {crest:.2}  \
                 (noise:7 mean {noise_mean:.4}, max/mean {noise_crest:.2})",
                names[i]
            );
            // Energy in every band at all — a groove that only lit bass would
            // exercise a third of the DSP, and the `spectrum` scenes read the
            // whole array. The floor is deliberately low: what a band *should*
            // read is exactly the open question Phase 4 measures, so this asserts
            // "audible, not silence" rather than a level nobody has evidence for.
            assert!(
                mean > 0.004,
                "{} is effectively silent (mean {mean:.4})",
                names[i]
            );
            assert!(
                crest > 2.0,
                "{} has no dynamics: max/mean {crest:.2} (min {min:.4} mean \
                 {mean:.4} max {max:.4})",
                names[i]
            );
            assert!(
                crest > noise_crest,
                "{} is no livelier than seeded noise, the liveliest kind that \
                 already existed: {crest:.2} against {noise_crest:.2}",
                names[i]
            );
        }
    }

    /// Determinism (NFR section 6): the same arguments give the same samples, so
    /// a filmstrip of this is reproducible. The seeded hat is the only thing that
    /// could have broken it.
    #[test]
    fn dynamic_groove_is_a_pure_function_of_its_arguments() {
        let format = fmt();
        let a = dynamic_groove(110.0, 1.0, format);
        let b = dynamic_groove(110.0, 1.0, format);
        assert_eq!(a, b, "two calls with identical arguments differ");
        assert!(a.iter().all(|s| s.is_finite()), "NaN/inf into the analyzer");
        assert!(
            a.iter().all(|s| s.abs() <= 0.9001),
            "peak normalization did not hold the 0.9 headroom"
        );
        // ...and the BPM is a real argument, not decoration.
        assert_ne!(a, dynamic_groove(90.0, 1.0, format), "the BPM does nothing");
    }

    #[test]
    fn click_track_produces_periodic_onsets() {
        let format = fmt();
        let pcm = click_track(120.0, 3.0, format); // 120 BPM => 0.5 s apart
        let mut an = Analyzer::new(format).expect("valid format");
        let hop = HOP_SIZE * format.channels as usize;
        let secs_per_frame = HOP_SIZE as f32 / format.sample_rate as f32;

        let mut beat_secs = Vec::new();
        for (frame, chunk) in pcm.chunks(hop).enumerate() {
            an.push_interleaved(chunk);
            if an.take_frame().beat {
                beat_secs.push(frame as f32 * secs_per_frame);
            }
        }

        // ~6 beats over 3 s (allow warm-up to swallow the first, and slack).
        assert!(
            (4..=7).contains(&beat_secs.len()),
            "expected ~6 beats over 3 s, got {}: {beat_secs:?}",
            beat_secs.len()
        );
        // Consecutive beats sit near 0.5 s apart.
        for pair in beat_secs.windows(2) {
            let gap = pair[1] - pair[0];
            assert!(
                (0.35..=0.65).contains(&gap),
                "beat gap {gap:.3}s should be ~0.5s (beats {beat_secs:?})"
            );
        }
    }
}
