//! String-to-value parsing for `shot`'s flags, the synthesized `--signal` PCM,
//! and the band-level measurement reported alongside a filmstrip. Pure: every
//! function is a total map from its inputs to a value or an error message, and
//! the error strings are the CLI's user-facing text.

use lmv_core::audio::AudioFormat;
use lmv_core::dsp::{AnalysisFrame, Analyzer, HOP_SIZE};
use lmv_core::signal::{bass_sine, chord, click_track, dynamic_groove, noise, treble_tone};

use super::film::FILMSTRIP_WARMUP;

/// Duration synthesized for a `--signal` (enough for several 120 BPM beats).
pub const SIGNAL_SECS: f32 = 4.0;

/// Parse `--size WxH` (either separator case) into non-zero dimensions.
pub fn parse_size(spec: &str) -> Result<(u32, u32), String> {
    let (w, h) = spec
        .split_once(['x', 'X'])
        .ok_or_else(|| format!("--size expects WxH, got `{spec}`"))?;
    let w = w.parse().map_err(|_| format!("bad width in `{spec}`"))?;
    let h = h.parse().map_err(|_| format!("bad height in `{spec}`"))?;
    if w == 0 || h == 0 {
        return Err("--size dimensions must be non-zero".to_string());
    }
    Ok((w, h))
}

/// Apply a comma-separated `k=v` list onto the stimulus frame. Keys are the
/// **grammar** variable names an author writes in a binding, not the
/// [`AnalysisFrame`] field names — `tempo` is the frame's `bpm`. `beat` is
/// truthy for any non-zero value.
pub fn apply_set(frame: &mut AnalysisFrame, spec: &str) -> Result<(), String> {
    for pair in spec.split(',').filter(|s| !s.is_empty()) {
        let (key, value) = pair
            .split_once('=')
            .ok_or_else(|| format!("--set expects k=v, got `{pair}`"))?;
        let v: f32 = value
            .parse()
            .map_err(|_| format!("--set `{key}` value `{value}` is not a number"))?;
        match key {
            "bass" => frame.bass = v,
            "mid" => frame.mid = v,
            "treb" => frame.treb = v,
            "onset" => frame.onset = v,
            "bar" => frame.bar = v,
            "beat" => frame.beat = v != 0.0,
            // Grammar v2 (Plan 0019). Reachable from a preset since then but not
            // from here, which is a large part of why almost no shipped preset
            // used them: an author could not drive either one in a capture.
            "tempo" => frame.bpm = v,
            "novelty" => frame.novelty = v,
            // Analysis v2 (Plan 0048 / ADR-0049, ADR-0050). Added here in the
            // same breath as the variables themselves, precisely because the
            // comment above is the lesson: a variable an author cannot drive in
            // a capture is a variable no preset ends up using.
            "bass_raw" => frame.bass_raw = v,
            "mid_raw" => frame.mid_raw = v,
            "treb_raw" => frame.treb_raw = v,
            "onset_raw" => frame.onset_raw = v,
            "time_since_beat" => frame.time_since_beat = v,
            "bar_phase" => frame.bar_phase = v,
            // Counters are integers on the frame and floats in the grammar, so a
            // negative or fractional request is clamped rather than wrapping to
            // something enormous.
            "beat_index" => frame.beat_index = whole(v),
            "beat_in_bar" => frame.beat_in_bar = whole(v),
            "bar_index" => frame.bar_index = whole(v),
            other => return Err(format!("--set: unknown key `{other}`")),
        }
    }
    Ok(())
}

/// A grammar float as a frame counter: truncated, and clamped into `u32` so a
/// negative or absurd `--set beat_index=-3` cannot wrap.
fn whole(v: f32) -> u32 {
    if v.is_finite() {
        v.clamp(0.0, u32::MAX as f32) as u32
    } else {
        0
    }
}

/// One numeric `--signal` parameter (a BPM, a frequency), named in the error.
pub fn parse_param(param: &str, what: &str) -> Result<f32, String> {
    param
        .parse::<f32>()
        .map_err(|_| format!("--signal: expected a {what} value, got `{param}`"))
}

/// Parse `<kind:param>` into synthesized PCM. Zero committed asset — this is the
/// self-contained validation of the whole audio path.
pub fn synth_signal(spec: &str) -> Result<(Vec<f32>, AudioFormat), String> {
    let format = AudioFormat {
        sample_rate: 48_000,
        channels: 2,
    };
    let (kind, param) = spec.split_once(':').unwrap_or((spec, ""));
    let pcm = match kind {
        "click" => click_track(parse_param(param, "click BPM")?, SIGNAL_SECS, format),
        "bass" => bass_sine(parse_param(param, "bass Hz")?, SIGNAL_SECS, format),
        "treble" | "treb" => treble_tone(parse_param(param, "treble Hz")?, SIGNAL_SECS, format),
        "noise" => {
            let seed = param.parse::<u64>().unwrap_or(1);
            noise(seed, SIGNAL_SECS, 0.8, format)
        }
        "chord" => chord(&[220.0, 277.0, 330.0], SIGNAL_SECS, format),
        // The one kind with dynamics — everything above is a steady tone or
        // steady noise (Plan 0037 Phase 3).
        "dynamic" => dynamic_groove(parse_param(param, "dynamic BPM")?, SIGNAL_SECS, format),
        other => {
            return Err(format!(
                "--signal: unknown kind `{other}` (click|bass|treble|noise|chord|dynamic)"
            ));
        }
    };
    Ok((pcm, format))
}

// ---------------------------------------------------------------------------
// What real audio actually produces
// ---------------------------------------------------------------------------

/// Min / mean / max of one band across a clip's analysis hops.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BandStats {
    /// Quietest hop measured.
    pub min: f32,
    /// Arithmetic mean across the measured hops.
    pub mean: f32,
    /// Loudest hop measured.
    pub max: f32,
}

/// The band levels a piece of audio derives through the **real** analyzer.
///
/// This exists because `--set bass=0.8` is not a level real material reaches:
/// `--set` writes the band straight onto the frame, while loopback and `--audio`
/// arrive through the analyzer's normalization. An author who calibrates a gain
/// against a `--set` magnitude ships a preset that barely moves on music — which
/// is exactly what happened to the 2026-07-26 preset sweep. Reported on every
/// filmstrip run so the real numbers stop being a guess.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BandLevels {
    /// Analysis hops the statistics cover — warm-up hops excluded.
    pub hops: usize,
    /// Bass band (~20-250 Hz).
    pub bass: BandStats,
    /// Mid band (~250-4000 Hz).
    pub mid: BandStats,
    /// Treble band (~4-18 kHz).
    pub treb: BandStats,
}

/// Run `pcm` through a fresh [`Analyzer`] and summarize what `bass`/`mid`/`treb`
/// read, over the same hops a filmstrip of this clip would render.
///
/// The first [`FILMSTRIP_WARMUP`] hops are skipped for the same reason the strip
/// skips them: until the analyzer's window fills, every band reads zero, and
/// including those hops would drag the reported minimum to 0 and the mean below
/// anything the clip actually produces.
pub fn band_levels(pcm: &[f32], format: AudioFormat) -> Result<BandLevels, String> {
    let mut analyzer = Analyzer::new(format).map_err(|e| format!("band levels: {e}"))?;
    // Safe only after `Analyzer::new` has validated the format — a zero channel
    // count would make this a zero-length chunk.
    let hop_samples = HOP_SIZE * format.channels as usize;

    let mut bass = Vec::new();
    let mut mid = Vec::new();
    let mut treb = Vec::new();
    for (index, hop) in pcm.chunks(hop_samples).enumerate() {
        analyzer.push_interleaved(hop);
        let frame = analyzer.take_frame();
        if index < FILMSTRIP_WARMUP {
            continue;
        }
        bass.push(frame.bass);
        mid.push(frame.mid);
        treb.push(frame.treb);
    }
    if bass.is_empty() {
        return Err("audio too short to measure band levels".to_string());
    }
    Ok(BandLevels {
        hops: bass.len(),
        bass: band_stats(&bass),
        mid: band_stats(&mid),
        treb: band_stats(&treb),
    })
}

/// Min / mean / max of `values`; all zero for an empty slice so this is total.
fn band_stats(values: &[f32]) -> BandStats {
    let mut min = f32::INFINITY;
    let mut max = f32::NEG_INFINITY;
    // Accumulate in f64: a long clip is thousands of hops, and the mean is the
    // number an author calibrates a gain against.
    let mut sum = 0.0f64;
    for &v in values {
        min = min.min(v);
        max = max.max(v);
        sum += f64::from(v);
    }
    if values.is_empty() {
        return BandStats {
            min: 0.0,
            mean: 0.0,
            max: 0.0,
        };
    }
    BandStats {
        min,
        mean: (sum / values.len() as f64) as f32,
        max,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_size_accepts_either_separator_and_rejects_degenerate_specs() {
        assert_eq!(parse_size("1280x720"), Ok((1280, 720)));
        assert_eq!(parse_size("64X48"), Ok((64, 48)));
        // A zero dimension would build a renderer with no pixels; reject it here.
        assert!(parse_size("0x480").is_err());
        assert!(parse_size("640x0").is_err());
        assert!(parse_size("640").is_err(), "no separator");
        assert!(parse_size("axb").is_err(), "non-numeric");
        assert!(parse_size("-1x2").is_err(), "negative width");
    }

    #[test]
    fn apply_set_writes_each_band_and_rejects_an_unknown_name() {
        let mut frame = AnalysisFrame::default();
        apply_set(
            &mut frame,
            "bass=1,mid=0.5,treb=0.25,onset=0.75,bar=0.125,beat=1,tempo=128,novelty=0.7",
        )
        .expect("every documented key is accepted");
        assert_eq!(frame.bass, 1.0);
        assert_eq!(frame.mid, 0.5);
        assert_eq!(frame.treb, 0.25);
        assert_eq!(frame.onset, 0.75);
        assert_eq!(frame.bar, 0.125);
        assert!(frame.beat, "any non-zero value is a beat");
        // The key is the grammar's name, the field is the analyzer's: an author
        // writes `tempo` in a binding, so `--set tempo=` is what must work.
        assert_eq!(frame.bpm, 128.0, "`tempo` writes the BPM estimate");
        assert_eq!(frame.novelty, 0.7);

        // `beat=0` is the only way to un-set it, and it must not read as truthy.
        apply_set(&mut frame, "beat=0").expect("beat=0 is valid");
        assert!(!frame.beat);

        // The whole reason this is a `Result`: a typo must be an exit-1 error,
        // not a silently ignored stimulus that makes a capture look dead.
        let err = apply_set(&mut frame, "bas=1").expect_err("unknown band name");
        assert!(err.contains("unknown key `bas`"), "got {err}");
        assert!(apply_set(&mut frame, "bass").is_err(), "missing `=`");
        assert!(apply_set(&mut frame, "bass=loud").is_err(), "not a number");
        // Near-misses of the two new keys stay rejected — `bpm` is the struct
        // field, not the grammar name, so accepting it would invent a synonym.
        assert!(
            apply_set(&mut frame, "bpm=128").is_err(),
            "not the key name"
        );
        assert!(apply_set(&mut frame, "temp=128").is_err(), "typo");
    }

    /// An empty segment is skipped, so a trailing comma is not an error.
    #[test]
    fn apply_set_skips_empty_segments() {
        let mut frame = AnalysisFrame::default();
        apply_set(&mut frame, "bass=1,").expect("trailing comma is tolerated");
        assert_eq!(frame.bass, 1.0);
        apply_set(&mut frame, "").expect("an empty spec is a no-op");
    }

    #[test]
    fn synth_signal_covers_every_kind_and_names_an_unknown_one() {
        let expected = (SIGNAL_SECS * 48_000.0) as usize * 2;
        for spec in [
            "click:120",
            "bass:60",
            "treble:10000",
            "treb:8000",
            "noise:7",
            "chord",
            "dynamic:110",
        ] {
            let (pcm, format) = synth_signal(spec).unwrap_or_else(|e| panic!("{spec}: {e}"));
            assert_eq!(format.sample_rate, 48_000);
            assert_eq!(format.channels, 2);
            assert_eq!(
                pcm.len(),
                expected,
                "{spec} synthesizes {SIGNAL_SECS}s stereo"
            );
            assert!(
                pcm.iter().all(|s| s.is_finite()),
                "{spec} must not emit NaN/inf into the analyzer"
            );
        }
        // `noise` takes a seed, not a float, and falls back rather than failing.
        assert!(synth_signal("noise").is_ok(), "seedless noise defaults");
        let err = synth_signal("sawtooth:1").expect_err("unknown kind");
        assert!(err.contains("unknown kind `sawtooth`"), "got {err}");
        // A numeric kind still needs its number.
        assert!(synth_signal("click:fast").is_err());
        assert!(synth_signal("click").is_err(), "click needs a BPM");
        assert!(synth_signal("dynamic").is_err(), "dynamic needs a BPM");
        assert!(synth_signal("dynamic:fast").is_err());
    }

    /// The measurement has to put energy where the signal put it, or the numbers
    /// it reports are worse than no numbers at all.
    #[test]
    fn band_levels_locate_a_known_tone_in_its_own_band() {
        let (pcm, format) = synth_signal("bass:60").expect("a 60 Hz sine");
        let levels = band_levels(&pcm, format).expect("4 s is plenty of hops");
        assert!(levels.hops > 0, "warm-up did not consume the whole clip");
        assert!(
            levels.bass.mean > levels.treb.mean,
            "a 60 Hz sine must read louder in bass than in treble: {levels:?}"
        );

        let (pcm, format) = synth_signal("treble:10000").expect("a 10 kHz tone");
        let levels = band_levels(&pcm, format).expect("4 s is plenty of hops");
        assert!(
            levels.treb.mean > levels.bass.mean,
            "a 10 kHz tone must read louder in treble than in bass: {levels:?}"
        );

        for band in [levels.bass, levels.mid, levels.treb] {
            assert!(band.min <= band.mean && band.mean <= band.max, "{band:?}");
            assert!(band.min.is_finite() && band.max.is_finite(), "{band:?}");
        }
    }

    /// Too little audio is an error, not a division by zero or a bogus mean.
    #[test]
    fn band_levels_reject_a_clip_shorter_than_the_warmup() {
        let format = AudioFormat {
            sample_rate: 48_000,
            channels: 2,
        };
        let short = vec![0.0f32; HOP_SIZE * 2 * FILMSTRIP_WARMUP];
        let err = band_levels(&short, format).expect_err("nothing past warm-up");
        assert!(err.contains("too short"), "got {err}");
        assert!(band_levels(&[], format).is_err(), "empty PCM");

        // One hop past warm-up is the first length that measures anything.
        let ok = vec![0.0f32; HOP_SIZE * 2 * (FILMSTRIP_WARMUP + 1)];
        assert_eq!(band_levels(&ok, format).map(|l| l.hops), Ok(1));
    }

    /// Silence is a legitimate measurement, and the one an author most needs to
    /// recognize: all three bands read zero rather than the function failing.
    #[test]
    fn band_stats_of_a_constant_signal_collapse_to_that_constant() {
        let s = band_stats(&[0.25, 0.25, 0.25]);
        assert_eq!((s.min, s.mean, s.max), (0.25, 0.25, 0.25));
        let s = band_stats(&[0.0, 0.5, 1.0]);
        assert_eq!((s.min, s.max), (0.0, 1.0));
        assert!((s.mean - 0.5).abs() < 1e-6, "got {}", s.mean);
        // Total on an empty slice — `band_levels` guards this, but the helper
        // must not divide by zero if it is ever called elsewhere.
        assert_eq!(
            band_stats(&[]),
            BandStats {
                min: 0.0,
                mean: 0.0,
                max: 0.0
            }
        );
    }
}
