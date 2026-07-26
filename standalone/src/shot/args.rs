//! String-to-value parsing for `shot`'s flags, plus the synthesized `--signal`
//! PCM. Pure: every function is a total map from its `&str` input to a value or
//! an error message, and the error strings are the CLI's user-facing text.

use lmv_core::audio::AudioFormat;
use lmv_core::dsp::AnalysisFrame;
use lmv_core::signal::{bass_sine, chord, click_track, noise, treble_tone};

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
/// scalar analysis bands; `beat` is truthy for any non-zero value.
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
            other => return Err(format!("--set: unknown key `{other}`")),
        }
    }
    Ok(())
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
        other => {
            return Err(format!(
                "--signal: unknown kind `{other}` (click|bass|treble|noise|chord)"
            ));
        }
    };
    Ok((pcm, format))
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
            "bass=1,mid=0.5,treb=0.25,onset=0.75,bar=0.125,beat=1",
        )
        .expect("every documented key is accepted");
        assert_eq!(frame.bass, 1.0);
        assert_eq!(frame.mid, 0.5);
        assert_eq!(frame.treb, 0.25);
        assert_eq!(frame.onset, 0.75);
        assert_eq!(frame.bar, 0.125);
        assert!(frame.beat, "any non-zero value is a beat");

        // `beat=0` is the only way to un-set it, and it must not read as truthy.
        apply_set(&mut frame, "beat=0").expect("beat=0 is valid");
        assert!(!frame.beat);

        // The whole reason this is a `Result`: a typo must be an exit-1 error,
        // not a silently ignored stimulus that makes a capture look dead.
        let err = apply_set(&mut frame, "bas=1").expect_err("unknown band name");
        assert!(err.contains("unknown key `bas`"), "got {err}");
        assert!(apply_set(&mut frame, "bass").is_err(), "missing `=`");
        assert!(apply_set(&mut frame, "bass=loud").is_err(), "not a number");
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
    }
}
