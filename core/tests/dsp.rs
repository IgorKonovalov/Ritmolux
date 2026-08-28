//! Plan 0001 Phase 3 fixtures: known signals in, expected analysis out.

use lmv_core::audio::AudioFormat;
use lmv_core::dsp::{AnalysisFrame, Analyzer, HOP_SIZE, SPECTRUM_BINS, WAVE_SAMPLES, WINDOW_SIZE};

const SR: u32 = 48_000;

fn mono_analyzer() -> Analyzer {
    Analyzer::new(AudioFormat {
        sample_rate: SR,
        channels: 1,
    })
    .expect("valid format")
}

fn sine(freq: f32, amp: f32, len: usize) -> Vec<f32> {
    (0..len)
        .map(|i| amp * (std::f32::consts::TAU * freq * i as f32 / SR as f32).sin())
        .collect()
}

/// Sum of equal-amplitude sines (a chord), scaled so the peak stays within
/// `amp`. Two chords on disjoint frequency ranges give clearly distinct spectra.
fn chord(freqs: &[f32], amp: f32, len: usize) -> Vec<f32> {
    let scale = if freqs.is_empty() {
        0.0
    } else {
        amp / freqs.len() as f32
    };
    (0..len)
        .map(|i| {
            let t = i as f32 / SR as f32;
            freqs
                .iter()
                .map(|f| (std::f32::consts::TAU * f * t).sin())
                .sum::<f32>()
                * scale
        })
        .collect()
}

/// Click track: near-silence with a short 0.9-amplitude burst at every beat.
fn click_track(period_samples: usize, len: usize) -> Vec<f32> {
    let mut signal = vec![0.0f32; len];
    let mut pos = period_samples;
    while pos + 32 < len {
        for (i, s) in signal[pos..pos + 32].iter_mut().enumerate() {
            // Alternating-sign burst: broadband, deterministic, no RNG.
            *s = if i % 2 == 0 { 0.9 } else { -0.9 };
        }
        pos += period_samples;
    }
    signal
}

/// Energy lands in the band the frequency says it should, and stays there.
///
/// The concentration claim reads exactly as it did before ADR-0049, and that is
/// the point: normalizing the array against **one** shared peak is a uniform
/// gain, so every ratio inside the array is untouched and "distant bands sit far
/// below the peak" remains a meaningful statement. Per-band normalization would
/// have broken this test rather than merely shifted it — the leakage four bands
/// out would have climbed to 1.0 as its own maximum.
///
/// The absolute-amplitude half of the old assertion moved to
/// `dsp::fft::tests::a_tone_reads_its_amplitude_on_either_side_of_the_crossover`,
/// which reads the raw band array where a magnitude is still a magnitude.
#[test]
fn sine_energy_concentrates_in_expected_band() {
    let mut analyzer = mono_analyzer();
    let freq = 1_000.0;
    let signal = sine(freq, 0.8, SR as usize);
    analyzer.push_interleaved(&signal);
    let frame = analyzer.take_frame();

    let expected = analyzer.band_for_freq(freq);
    let peak_band = frame
        .spectrum
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.total_cmp(b.1))
        .map(|(k, _)| k)
        .expect("spectrum is non-empty");
    assert_eq!(
        peak_band, expected,
        "energy should peak in the band containing {freq} Hz"
    );

    // The loudest band anchors the shared peak, so it reads full scale.
    let peak = frame.spectrum[expected];
    assert_eq!(peak, 1.0, "the loudest band should anchor the array at 1.0");

    // The energy is concentrated: away from the peak's immediate neighbors
    // (Hann leakage), every band stays far below the peak.
    for (k, &v) in frame.spectrum.iter().enumerate() {
        if k.abs_diff(expected) > 1 {
            assert!(
                v < 0.1 * peak,
                "band {k} = {v} should be well below the {freq} Hz peak {peak}"
            );
        }
    }
}

/// Plan 0048 Phase 2 done-when: `*_raw` reproduce the pre-ADR-0049 values
/// **bit-exactly** — on x86_64, which is where they were measured.
///
/// The four literals below are not a re-derivation of this build's own output —
/// they were measured by running this exact fixture against `92579ef`, where
/// `bass` *was* the raw value. So the test states a fact about the pre-ADR-0049
/// code that this code has to match, which is the only form of "unchanged"
/// worth asserting.
///
/// Hop 200 is read deliberately: both builds have long since filled the short
/// window there, and the raw levels come from the **short** window's magnitudes
/// via `bands.rs`, which neither phase touched. The longer warm-up gate moves
/// which hop publishes *first*, not what the window contains at hop 200.
///
/// **This is a measurement, not a property** (ADR-0071). Those bits belong to one
/// configuration and cannot reproduce on another: the fixture builds its own
/// input with `f32::sin`, which lowers to the platform libm, and `rustfft`
/// dispatches NEON on aarch64 where it dispatches AVX/SSE here — two sets of
/// rounding applied to two slightly different inputs. On `macos-26-arm64`
/// `bass_raw` lands about 71 ULP away and always has. So the comparison names the
/// architecture it came from and does not run outside it; elsewhere it prints
/// every observed level with its relative error, in the ADR-0016 skip-with-notice
/// shape, so the configuration it declines to gate is still visible in the log.
///
/// The counter-assertion below — that normalization is not a no-op — needs no
/// frozen number and runs **everywhere**, so the test is not vacuous where the
/// bit comparison is pinned out.
///
/// **What the other architecture actually reads**, off `macos-26-arm64` on
/// 2026-08-04 (Plan 0060 Phase 2, CI run 30903871856) — observed and printed
/// there, never asserted:
///
/// | level | observed bits | relative error vs the x86_64 reference |
/// |---|---|---|
/// | `bass_raw` | `0x386597d6` | `8.44e-6` |
/// | `mid_raw` | `0x3bd581b5` | bit-identical |
/// | `treb_raw` | `0x35f3168e` | `1.15e-5` |
/// | `onset_raw` | `0x348652cd` | `1.86e-5` |
///
/// All four sit within `2e-5` relative and one reproduces exactly. `onset_raw` is
/// the one worth naming: at `2.5e-7` it is small enough that a difference-derived
/// value could have lost relative precision by orders of magnitude, and it does
/// not — 2.2x `bass_raw`'s divergence, the same order. None of this argues for a
/// cross-architecture tolerance; it is recorded so the next reader knows the size
/// of what the pin hides instead of inferring it from a skip.
#[test]
fn raw_levels_are_bit_identical_to_the_pre_normalization_build() {
    let mut analyzer = mono_analyzer();
    let signal = {
        let mut s = sine(440.0, 0.5, 4 * SR as usize);
        let clicks = click_track(12_000, s.len());
        for (a, b) in s.iter_mut().zip(clicks.iter()) {
            *a += b;
        }
        s
    };

    let mut at_200 = None;
    for (i, hop) in signal.chunks_exact(HOP_SIZE).enumerate() {
        analyzer.push_interleaved(hop);
        let frame = analyzer.take_frame();
        if i == 200 {
            at_200 = Some(frame);
        }
    }
    let frame = at_200.expect("the fixture is long enough to reach hop 200");

    // The reference reading, as `92579ef` produced it on x86_64.
    let reference = [
        ("bass_raw", frame.bass_raw, 0x3865_9855u32),
        ("mid_raw", frame.mid_raw, 0x3bd5_81b5),
        ("treb_raw", frame.treb_raw, 0x35f3_1745),
        ("onset_raw", frame.onset_raw, 0x3486_5371),
    ];

    if cfg!(target_arch = "x86_64") {
        for (name, actual, expected) in reference {
            assert_eq!(
                actual.to_bits(),
                expected,
                "{name} = {actual} must reproduce the pre-ADR-0049 value bit-for-bit"
            );
        }
    } else {
        eprintln!(
            "skipped: the frozen raw-level bits are a measurement taken on x86_64 \
             against 92579ef and do not reproduce on {} (ADR-0071). Observed here:",
            std::env::consts::ARCH
        );
        // Printed in full rather than compared: reporting every level, not just the
        // first divergent one, is the whole point of not asserting them here.
        for (name, actual, expected) in reference {
            let want = f32::from_bits(expected);
            let rel = if want == 0.0 {
                if actual == 0.0 { 0.0 } else { f32::INFINITY }
            } else {
                ((actual - want) / want).abs()
            };
            eprintln!(
                "  {name}: observed 0x{:08x} = {actual:e}, reference 0x{expected:08x} = {want:e}, \
                 relative error {rel:e}",
                actual.to_bits()
            );
        }
    }

    // The counter-assertion that stops this from being a tautology: the
    // *normalized* values genuinely differ from the raw ones on this fixture. If
    // normalization were a no-op the block above would pass just as happily.
    assert_ne!(
        frame.bass.to_bits(),
        frame.bass_raw.to_bits(),
        "normalization must actually do something (bass {} vs raw {})",
        frame.bass,
        frame.bass_raw
    );
}

/// Plan 0048 Phase 2 done-when, through the analyzer rather than the normalizer:
/// the same music at two gains converges to the same normalized reading, and
/// silence stays at zero.
#[test]
fn normalized_levels_are_portable_across_absolute_gain() {
    // Real broadband content, at two gains. `dynamic_groove` rather than a tone
    // plus clicks: the property is about music, and a fixture whose treble sits
    // near the silence floor at full scale would cross it at -20 dB and diverge
    // for a reason that is the floor working correctly, not the property failing.
    let format = AudioFormat {
        sample_rate: SR,
        channels: 2,
    };
    let groove = lmv_core::signal::dynamic_groove(120.0, 6.0, format);
    let hop_samples = HOP_SIZE * format.channels as usize;

    let read = |gain: f32| -> Vec<(f32, f32, f32)> {
        let mut analyzer = Analyzer::new(format).expect("valid format");
        groove
            .chunks(hop_samples)
            .map(|hop| {
                let scaled: Vec<f32> = hop.iter().map(|s| s * gain).collect();
                analyzer.push_interleaved(&scaled);
                let f = analyzer.take_frame();
                (f.bass, f.mid, f.treb)
            })
            .collect()
    };

    let full = read(1.0);
    // -20 dB: a factor of ten in amplitude.
    let quiet = read(0.1);
    let worst = full
        .iter()
        .zip(quiet.iter())
        .map(|(a, b)| {
            (a.0 - b.0)
                .abs()
                .max((a.1 - b.1).abs())
                .max((a.2 - b.2).abs())
        })
        .fold(0.0f32, f32::max);
    assert!(
        worst < 1e-4,
        "a -20 dB copy must produce the same normalized series, worst divergence {worst}"
    );

    // Non-vacuity: the series has real range, so the agreement is not two flat
    // runs matching each other.
    let spread = full.iter().map(|f| f.0).fold(0.0f32, f32::max)
        - full.iter().map(|f| f.0).fold(1.0f32, f32::min);
    assert!(
        spread > 0.3,
        "the fixture should exercise a real dynamic range, got {spread}"
    );

    // Silence stays silent: the floor means a quiet room is not amplified to
    // full scale.
    let mut analyzer = mono_analyzer();
    let mut peak = 0.0f32;
    for hop in vec![0.0f32; 3 * SR as usize].chunks_exact(HOP_SIZE) {
        analyzer.push_interleaved(hop);
        let f = analyzer.take_frame();
        peak = peak.max(f.bass).max(f.mid).max(f.treb).max(f.onset);
    }
    assert_eq!(peak, 0.0, "silence must read zero, peaked at {peak}");
}

/// Plan 0048 Phase 3 done-when: `beat_index` is monotone and `time_since_beat`
/// resets on each beat, under the click signal.
///
/// Driven through `signal::click_track` — the generator behind `shot`'s
/// `--signal click`, so this is the same stimulus the done-when names, measured
/// without a GPU.
#[test]
fn the_beat_clock_counts_monotonically_and_resets_on_every_beat() {
    let format = AudioFormat {
        sample_rate: SR,
        channels: 2,
    };
    let bpm = 120.0;
    let pcm = lmv_core::signal::click_track(bpm, 10.0, format);
    let mut analyzer = Analyzer::new(format).expect("valid format");
    let hop_samples = HOP_SIZE * format.channels as usize;

    let mut series = Vec::new();
    for hop in pcm.chunks(hop_samples) {
        analyzer.push_interleaved(hop);
        let f = analyzer.take_frame();
        series.push((f.beat, f.beat_index, f.time_since_beat));
    }

    // Monotone, and never skipping: a counter that jumped would break
    // `mod(beat_index, 4)` arithmetic in a way no visual would explain.
    for pair in series.windows(2) {
        if let [(_, a, _), (_, b, _)] = pair {
            assert!(b >= a, "beat_index must never fall: {a} then {b}");
            assert!(
                b - a <= 1,
                "beat_index must advance one beat at a time: {a} then {b}"
            );
        }
    }

    // Every beat hop resets the clock to exactly zero...
    let beat_hops: Vec<usize> = series
        .iter()
        .enumerate()
        .filter(|(_, (beat, _, _))| *beat)
        .map(|(i, _)| i)
        .collect();
    assert!(
        beat_hops.len() >= 15,
        "10 s at {bpm} BPM should give ~20 beats, got {}",
        beat_hops.len()
    );
    for &i in &beat_hops {
        assert_eq!(
            series[i].2, 0.0,
            "time_since_beat must be exactly 0 on the beat hop at {i}"
        );
    }

    // ...and it climbs in between, reaching most of a beat period before the
    // next reset. Without this the test would pass on a constant zero.
    let period = 60.0 / bpm;
    let longest = series.iter().map(|(_, _, t)| *t).fold(0.0f32, f32::max);
    assert!(
        longest > period * 0.5,
        "time_since_beat should climb toward the {period:.3} s beat period, peaked at {longest:.3}"
    );

    // The counter actually advanced across the clip, and by as many beats as
    // were detected — the two are separate claims and both matter.
    let final_index = series.last().map(|(_, i, _)| *i).unwrap_or(0);
    assert_eq!(
        final_index as usize,
        beat_hops.len() - 1,
        "the final beat_index should be one less than the number of beats seen"
    );
}

/// A click every beat, with a low thump on every `accent_every`-th beat starting
/// at `offset`. `accent_every = 0` means no accents at all.
fn accented_clicks(period: usize, len: usize, accent_every: usize, offset: usize) -> Vec<f32> {
    let mut signal = click_track(period, len);
    if accent_every == 0 {
        return signal;
    }
    let mut beat = 0usize;
    let mut pos = period;
    while pos + 4_000 < len {
        if beat % accent_every == offset {
            // A 60 Hz thump, coincident with the click so it reads as one onset
            // rather than a second beat. Decaying, like a kick.
            for i in 0..4_000 {
                let t = i as f32 / SR as f32;
                let env = (-t * 18.0).exp();
                signal[pos + i] += 0.85 * env * (std::f32::consts::TAU * 60.0 * t).sin();
            }
        }
        beat += 1;
        pos += period;
    }
    signal
}

/// Plan 0048 Phase 4, end to end: the downbeat estimator locks onto a kick
/// pattern in **real audio**, and stays in fallback without one.
///
/// The module's own tests drive the tracker with idealized accent numbers, which
/// says the fold and the gate are right but says nothing about whether the accent
/// *measure* can tell a kick from a click once it has been through an FFT, a
/// band split and two normalizers. This is that claim.
#[test]
fn the_downbeat_estimator_locks_onto_a_kick_pattern_in_real_audio() {
    let format = AudioFormat {
        sample_rate: SR,
        channels: 1,
    };
    // 120 BPM: a beat every 24000 samples. 24 s is 48 beats — comfortably past
    // the 8-beat evidence floor and the 12-beat hysteresis.
    let period = 24_000usize;
    let len = 24 * SR as usize;

    let run = |pcm: &[f32]| -> (bool, f32, Vec<(u32, u32)>) {
        let mut analyzer = Analyzer::new(format).expect("valid format");
        let mut pairs = Vec::new();
        let mut locked = false;
        let mut confidence = 0.0;
        for hop in pcm.chunks_exact(HOP_SIZE) {
            analyzer.push_interleaved(hop);
            let f = analyzer.take_frame();
            if f.beat {
                pairs.push((f.beat_index, f.beat_in_bar));
            }
            locked = f.downbeat_locked;
            confidence = f.downbeat_confidence;
        }
        (locked, confidence, pairs)
    };

    // Accent on every 4th beat, offset 2, so a tracker that always answers 0
    // cannot pass.
    let (locked, confidence, pairs) = run(&accented_clicks(period, len, 4, 2));
    assert!(
        locked,
        "a kick every 4th beat should lock (confidence {confidence:.3})"
    );
    assert!(
        confidence > 0.2,
        "confidence {confidence:.3} should clear the gate with margin"
    );

    // The accented beats are the ones whose beat_index % 4 == 2 in the tracker's
    // own numbering. Once locked, those must be beat 0 of the bar. Read from the
    // back half so the lock has settled.
    let settled: Vec<(u32, u32)> = pairs.iter().copied().skip(pairs.len() / 2).collect();
    assert!(settled.len() > 8, "expected plenty of settled beats");
    let accented_are_beat_one = settled
        .iter()
        .filter(|(index, _)| index % 4 == 2)
        .all(|(_, in_bar)| *in_bar == 0);
    assert!(
        accented_are_beat_one,
        "every accented beat should read beat_in_bar 0 once locked: {settled:?}"
    );

    // The counter-case: identical clicks with no kick must NOT lock. Without this
    // the test above would pass on a tracker that locks onto anything.
    let (flat_locked, flat_conf, _) = run(&accented_clicks(period, len, 0, 0));
    assert!(
        !flat_locked,
        "an unaccented click train must stay in fallback (confidence {flat_conf:.3})"
    );
}

#[test]
fn click_track_produces_onsets_on_the_beats() {
    let mut analyzer = mono_analyzer();
    // 120 BPM at 48 kHz: a click every 24000 samples, 5 seconds of signal.
    let period = 24_000usize;
    let signal = click_track(period, 5 * SR as usize);

    let mut beat_hops = Vec::new();
    for (hop_idx, hop) in signal.chunks_exact(HOP_SIZE).enumerate() {
        analyzer.push_interleaved(hop);
        if analyzer.take_frame().beat {
            beat_hops.push(hop_idx);
        }
    }

    // Clicks start at `period` and repeat while a full burst fits.
    let expected_clicks: Vec<usize> = (1..)
        .map(|k| k * period)
        .take_while(|&pos| pos + 32 < signal.len())
        .collect();
    assert_eq!(expected_clicks.len(), 9, "fixture sanity");

    // Every click produces exactly one beat within 3 hops of the hop that
    // first contains it, and there are no spurious beats elsewhere.
    let tolerance = 3;
    for &click_pos in &expected_clicks {
        let click_hop = click_pos / HOP_SIZE;
        let matches = beat_hops
            .iter()
            .filter(|&&h| h.abs_diff(click_hop) <= tolerance)
            .count();
        assert_eq!(
            matches, 1,
            "click at hop {click_hop} should produce exactly one beat, got {beat_hops:?}"
        );
    }
    assert_eq!(
        beat_hops.len(),
        expected_clicks.len(),
        "no spurious beats: {beat_hops:?}"
    );
}

#[test]
fn tempo_estimate_locks_onto_a_known_click_train() {
    let mut analyzer = mono_analyzer();
    // 120 BPM at 48 kHz: a click every 24000 samples. 12 s is well past the
    // tempo tracker's ~4 s envelope-history warmup.
    let period = 24_000usize;
    let signal = click_track(period, 12 * SR as usize);
    analyzer.push_interleaved(&signal);
    let bpm = analyzer.take_frame().bpm;

    // Hop-clock autocorrelation should land the tempo within a few BPM of the
    // true 120 (a determinism/correctness claim, not "the test runs").
    assert!(
        (bpm - 120.0).abs() <= 3.0,
        "estimated tempo {bpm} should be within 3 BPM of the 120 BPM click train"
    );
}

#[test]
fn band_split_is_frequency_correct() {
    // A pure low tone lands its energy in bass with ~none in treble...
    let mut low_an = mono_analyzer();
    low_an.push_interleaved(&sine(60.0, 0.8, SR as usize));
    let low = low_an.take_frame();
    assert!(
        low.bass > 0.01 && low.bass > low.mid && low.bass > low.treb,
        "60 Hz energy should dominate the bass band (bass={}, mid={}, treb={})",
        low.bass,
        low.mid,
        low.treb
    );
    assert!(
        low.treb < 0.1 * low.bass,
        "treble should be near-empty for a 60 Hz tone (treb={}, bass={})",
        low.treb,
        low.bass
    );

    // ...and the mirror holds for a pure high tone.
    let mut high_an = mono_analyzer();
    high_an.push_interleaved(&sine(6_000.0, 0.8, SR as usize));
    let high = high_an.take_frame();
    assert!(
        high.treb > 0.0 && high.treb > high.bass && high.treb > high.mid,
        "6 kHz energy should dominate the treble band (bass={}, mid={}, treb={})",
        high.bass,
        high.mid,
        high.treb
    );
    assert!(
        high.bass < 0.1 * high.treb,
        "bass should be near-empty for a 6 kHz tone (bass={}, treb={})",
        high.bass,
        high.treb
    );
}

#[test]
fn novelty_spikes_at_a_spectral_boundary() {
    let mut analyzer = mono_analyzer();
    // Two 3 s segments with disjoint spectra: a low chord then a high chord.
    let seg = 3 * SR as usize;
    let mut signal = chord(&[110.0, 220.0, 330.0], 0.8, seg);
    signal.extend_from_slice(&chord(&[4_000.0, 6_000.0, 8_000.0], 0.8, seg));

    let boundary_hop = seg / HOP_SIZE;
    let novelty: Vec<f32> = signal
        .chunks_exact(HOP_SIZE)
        .map(|hop| {
            analyzer.push_interleaved(hop);
            analyzer.take_frame().novelty
        })
        .collect();

    // Well inside segment A the spectrum sits on its own running mean: ~0.
    let steady_a = novelty[boundary_hop / 2];
    // Late in segment B the mean has caught up to the new spectrum: also low.
    let steady_b = *novelty.last().expect("frames were produced");
    // The boundary (plus the ~4-hop window transition) spikes.
    let peak = novelty[boundary_hop..(boundary_hop + 12).min(novelty.len())]
        .iter()
        .copied()
        .fold(0.0f32, f32::max);

    assert!(
        steady_a < 0.1,
        "steady segment-A novelty {steady_a} should be near zero"
    );
    assert!(peak > 0.4, "the boundary should spike novelty (got {peak})");
    assert!(
        peak > steady_a * 5.0 + 0.2,
        "boundary spike {peak} should stand out from steady {steady_a}"
    );
    assert!(
        steady_b < peak,
        "late segment-B novelty {steady_b} should decay below the spike {peak}"
    );
}

#[test]
fn analysis_is_deterministic() {
    let signal = {
        let mut s = sine(440.0, 0.5, 2 * SR as usize);
        let clicks = click_track(12_000, s.len());
        for (a, b) in s.iter_mut().zip(clicks.iter()) {
            *a += b;
        }
        s
    };

    #[allow(clippy::type_complexity)]
    let run = |mut analyzer: Analyzer| -> Vec<(Vec<u32>, Vec<u32>, bool, Vec<u32>, bool)> {
        signal
            .chunks_exact(HOP_SIZE)
            .map(|hop| {
                analyzer.push_interleaved(hop);
                let f = analyzer.take_frame();
                // Bit-exact comparison via raw f32 bits. ADR-0049's normalizers
                // and ADR-0050's beat clock are both new analyzer *state*, which
                // is exactly the kind of addition that can make analysis
                // history-dependent in a way a spot check would miss.
                //
                // Destructured rather than field-accessed, deliberately: naming
                // every field means **adding one to `AnalysisFrame` stops this
                // file compiling** until it is covered here. A `f.field` list
                // would have gone quietly out of date instead.
                let AnalysisFrame {
                    spectrum,
                    waveform,
                    waveform_gain,
                    onset,
                    beat,
                    bass,
                    mid,
                    treb,
                    bass_raw,
                    mid_raw,
                    treb_raw,
                    onset_raw,
                    bpm,
                    bar,
                    beat_index,
                    time_since_beat,
                    beat_in_bar,
                    bar_index,
                    bar_phase,
                    downbeat_confidence,
                    downbeat_locked,
                    novelty,
                } = f;
                (
                    spectrum
                        .iter()
                        .chain(waveform.iter())
                        .map(|v| v.to_bits())
                        .collect(),
                    vec![
                        waveform_gain.to_bits(),
                        onset.to_bits(),
                        bass.to_bits(),
                        mid.to_bits(),
                        treb.to_bits(),
                        bass_raw.to_bits(),
                        mid_raw.to_bits(),
                        treb_raw.to_bits(),
                        onset_raw.to_bits(),
                        bpm.to_bits(),
                        bar.to_bits(),
                        time_since_beat.to_bits(),
                        bar_phase.to_bits(),
                        downbeat_confidence.to_bits(),
                        novelty.to_bits(),
                    ],
                    beat,
                    vec![beat_index, beat_in_bar, bar_index],
                    downbeat_locked,
                )
            })
            .collect()
    };

    assert_eq!(run(mono_analyzer()), run(mono_analyzer()));
}

#[test]
#[allow(
    clippy::disallowed_methods,
    reason = "perf smoke test deliberately times execution; the analysis under test stays clock-free"
)]
fn one_hop_analyzes_well_under_the_hop_interval() {
    let mut analyzer = mono_analyzer();
    let signal = sine(440.0, 0.5, WINDOW_SIZE + 1000 * HOP_SIZE);
    analyzer.push_interleaved(&signal[..WINDOW_SIZE]);

    let start = std::time::Instant::now();
    for hop in signal[WINDOW_SIZE..].chunks_exact(HOP_SIZE) {
        analyzer.push_interleaved(hop);
    }
    let per_hop = start.elapsed() / 1000;

    // Printed, not just asserted: Plan 0048 Phase 1's done-when asks for the
    // measured per-hop cost against NFR section 3, and the dual-resolution axis
    // added a second FFT to this path. Run with `--release --nocapture` for a
    // number worth quoting.
    let hop_budget = std::time::Duration::from_secs_f32(HOP_SIZE as f32 / SR as f32);
    println!(
        "per-hop analysis: {per_hop:?} of a {hop_budget:?} hop ({:.2}% of budget)",
        per_hop.as_secs_f64() / hop_budget.as_secs_f64() * 100.0
    );

    // Hop interval is ~10.7 ms at 48 kHz; even unoptimized builds should sit
    // far below it (NFR section 3 / plan done-when).
    assert!(
        per_hop < std::time::Duration::from_millis(11),
        "one hop took {per_hop:?}, budget is ~11 ms"
    );
    // Sanity that the spectrum output stayed meaningful end-to-end.
    assert!(analyzer.take_frame().spectrum.iter().sum::<f32>() > 0.0);
    assert_eq!(SPECTRUM_BINS, 64);
}

/// **The waveform is the signal, in time order, levelled** (Plan 0100 Phase 4,
/// ADR-0139) — the four properties the warp mesh's `wave_mode` draw rests on.
///
/// It is not a fifth statistic. Everything else on the frame is a *measurement*
/// of the window; this is the window's own shape, scaled by one number the frame
/// publishes, and the assertions below are what distinguishes the two.
#[test]
fn the_waveform_is_the_recent_signal_levelled_rather_than_a_measurement_of_it() {
    // A sine at a frequency that fits a whole number of cycles into the tail, so
    // the trace is checkable sample by sample rather than statistically.
    let freq = SR as f32 / 64.0; // 750 Hz — 8 whole cycles across 512 samples
    let amp = 0.4;
    let mut analyzer = mono_analyzer();
    let signal = sine(freq, amp, WINDOW_SIZE * 8);
    for hop in signal.chunks_exact(HOP_SIZE) {
        analyzer.push_interleaved(hop);
    }
    let frame = analyzer.take_frame();

    // 1. It is the TAIL of the window, consecutive — so multiplying the
    //    published gain back round-trips against the generator, phase and all.
    //    That reconstruction IS the escape hatch, exercised where it is claimed.
    let tail = &signal[signal.len() - WAVE_SAMPLES..];
    for (i, (got, want)) in frame.waveform.iter().zip(tail).enumerate() {
        let raw = got * frame.waveform_gain;
        assert!(
            (raw - want).abs() < 1e-6,
            "sample {i}: waveform {got} * gain {} = {raw} != signal {want}",
            frame.waveform_gain
        );
    }

    // 2. It is LEVELLED, like every level on this frame. A 0.4-amplitude sine
    //    reaches 1.0 and the gain carries the 0.4 — which is what makes the
    //    trace the same picture at any fader position, on either frontend
    //    (ADR-0139). The shape is untouched; only the scale moved.
    let peak = frame.waveform.iter().fold(0.0f32, |m, v| m.max(v.abs()));
    assert!(
        (peak - 1.0).abs() < 0.01,
        "the levelled trace must reach full scale, got {peak}"
    );
    assert!(
        (frame.waveform_gain - amp).abs() < 0.01,
        "the gain must carry the signal's own amplitude, got {} for {amp}",
        frame.waveform_gain
    );

    // 3. It is bipolar — a spectrum is not. A trace that had been rectified or
    //    magnitude-taken would fail here and pass every other check.
    assert!(
        frame.waveform.iter().any(|v| *v > 0.1) && frame.waveform.iter().any(|v| *v < -0.1),
        "the trace must swing both ways"
    );

    // 4. Silence is a flat line, not a floor. Nothing normalizes it up.
    let mut quiet = mono_analyzer();
    for hop in vec![0.0f32; WINDOW_SIZE * 8].chunks_exact(HOP_SIZE) {
        quiet.push_interleaved(hop);
    }
    let quiet_frame = quiet.take_frame();
    assert!(
        quiet_frame.waveform.iter().all(|v| *v == 0.0),
        "silence must be a flat line"
    );
    assert_eq!(
        quiet_frame.waveform_gain, 0.0,
        "and it publishes no gain, so reconstructing it gives silence too"
    );
}

/// ADR-0139's whole point, through the analyzer: the same music at two gains
/// produces the same trace, so the OS volume slider stops being a visual
/// parameter and the two frontends stop disagreeing.
///
/// The standalone taps loopback *after* the endpoint volume and the plugin taps
/// the decoded stream *before* it; nothing else about the two paths differs by
/// the time samples reach the analyzer, so a gain sweep is the whole of that
/// asymmetry. Shaped after `normalized_levels_are_portable_across_absolute_gain`
/// because it is the same property one array over.
#[test]
fn the_trace_is_portable_across_absolute_gain() {
    let format = AudioFormat {
        sample_rate: SR,
        channels: 2,
    };
    let groove = lmv_core::signal::dynamic_groove(120.0, 6.0, format);
    let hop_samples = HOP_SIZE * format.channels as usize;

    let read = |gain: f32| -> Vec<[f32; WAVE_SAMPLES]> {
        let mut analyzer = Analyzer::new(format).expect("valid format");
        groove
            .chunks(hop_samples)
            .map(|hop| {
                let scaled: Vec<f32> = hop.iter().map(|s| s * gain).collect();
                analyzer.push_interleaved(&scaled);
                analyzer.take_frame().waveform
            })
            .collect()
    };

    // 18 % on the master slider is the reading design-backlog 0123 measured the
    // near-flat ribbon at; 1.0 is what `shot --audio` feeds from a file.
    let full = read(1.0);
    let quiet = read(0.18);
    let worst = full
        .iter()
        .zip(quiet.iter())
        .flat_map(|(a, b)| a.iter().zip(b.iter()))
        .fold(0.0f32, |m, (a, b)| m.max((a - b).abs()));
    assert!(
        worst < 1e-4,
        "a quieter copy must draw the same trace, worst divergence {worst}"
    );

    // Non-vacuity: the traces have real amplitude, so this is not two silent
    // runs agreeing with each other.
    let loudest = full
        .iter()
        .flat_map(|t| t.iter())
        .fold(0.0f32, |m, v| m.max(v.abs()));
    assert!(
        loudest > 0.5,
        "the fixture should drive the trace to real amplitude, peaked at {loudest}"
    );
}

/// The dynamics the un-normalized trace was defended for: a levelled trace still
/// draws the quiet passage smaller than the loud one.
///
/// `dynamic_groove`'s phrase rests for two of every eight beats at a `0.04`
/// scale, and the release is seconds-scale, so the rest sits well inside the
/// window where the loud phrase's peak still governs. Asserted as an ordering —
/// the ratio is a function of `RELEASE_TAU_SECS` and belongs to ADR-0049.
#[test]
fn a_quiet_passage_still_draws_a_smaller_trace_than_a_loud_one() {
    let format = AudioFormat {
        sample_rate: SR,
        channels: 1,
    };
    let groove = lmv_core::signal::dynamic_groove(120.0, 8.0, format);
    let mut analyzer = Analyzer::new(format).expect("valid format");
    // Peak trace amplitude per beat, at 120 bpm — beats 6 and 7 of each phrase
    // of eight are the rest.
    let hops_per_beat = (SR as usize / 2) / HOP_SIZE;
    let mut per_beat: Vec<f32> = Vec::new();
    for (index, hop) in groove.chunks_exact(HOP_SIZE).enumerate() {
        analyzer.push_interleaved(hop);
        let peak = analyzer
            .take_frame()
            .waveform
            .iter()
            .fold(0.0f32, |m, v| m.max(v.abs()));
        let beat = index / hops_per_beat;
        if beat >= per_beat.len() {
            per_beat.push(peak);
        } else if let Some(slot) = per_beat.get_mut(beat) {
            *slot = slot.max(peak);
        }
    }
    // The second phrase, so the running peak is warm and the first phrase's
    // silent start is not in the comparison.
    let loud = per_beat.get(12).copied().expect("12 beats of groove");
    let rest = per_beat.get(14).copied().expect("14 beats of groove");
    assert!(
        rest < loud,
        "the rest must draw smaller than the build: rest {rest}, loud {loud}"
    );
    assert!(
        rest > 0.0,
        "...and it must still draw: a rest is quieter, not silent"
    );
}

/// The waveform is **taken consecutively, not decimated**, and this is what that
/// buys — measured rather than argued.
///
/// A 4:1 decimation across the whole 2048-sample window would alias anything
/// above a quarter of Nyquist: a 12 kHz tone at 48 kHz would read as 12 kHz
/// sampled at 12 kHz, i.e. DC or a slow beat, and the trace would show a
/// near-straight line where the signal is a dense oscillation. The consecutive
/// tail shows the oscillation.
#[test]
fn a_high_tone_reads_as_an_oscillation_rather_than_aliasing_flat() {
    let mut analyzer = mono_analyzer();
    for hop in sine(12_000.0, 0.5, WINDOW_SIZE * 8).chunks_exact(HOP_SIZE) {
        analyzer.push_interleaved(hop);
    }
    let frame = analyzer.take_frame();

    // Sign changes across the trace: a 12 kHz tone at 48 kHz crosses zero every
    // other sample, so a 512-sample consecutive window holds hundreds of them.
    let crossings = frame
        .waveform
        .windows(2)
        .filter(|w| (w[0] < 0.0) != (w[1] < 0.0))
        .count();
    println!("[waveform] 12 kHz tone: {crossings} zero crossings in {WAVE_SAMPLES} samples");
    assert!(
        crossings > 200,
        "a 12 kHz tone must read as a dense oscillation, got {crossings} crossings \
         — a decimated trace would be nearly flat"
    );
}
