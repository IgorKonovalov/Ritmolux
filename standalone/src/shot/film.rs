//! Filmstrip arithmetic: which analysis frames to capture along a clip, and the
//! canvas the captures tile into.
//!
//! The **layout** is here and the pixel blit is not, deliberately: `image` is a
//! dev-dependency so the PNG codec stays out of the shipped `ritmolux.exe` (ADR-0011,
//! ADR-0033 Alt E). Dimensions and offsets are the part that can be wrong in a
//! way a human would not notice, so they are the part worth testing.

use rlx_core::audio::AudioFormat;
use rlx_core::dsp::{HOP_SIZE, WARMUP_HOPS};

/// Hops skipped at the start so the strip samples past the analyzer's warm-up.
///
/// **Derived, not chosen.** [`WARMUP_HOPS`] is when the analyzer publishes its
/// first frame (ADR-0049); the original four hops of slack past that boundary are
/// kept. A bare `8` against a 2048-sample window is what Plan 0048 Phase 1
/// silently invalidated — deriving the constant means changing a window size
/// cannot leave the harness sampling zero frames again.
pub const FILMSTRIP_WARMUP: usize = WARMUP_HOPS + 4;

/// Rendered height of each frame in a strip.
pub const STRIP_H: u32 = 200;
/// Gutter between (and around) the tiled frames.
pub const STRIP_PAD: u32 = 4;

/// Analysis hops a clip of `pcm_len` interleaved samples yields.
///
/// The one place this division lives. `--strip`'s even spacing, `--at`'s and
/// `--frame-at`'s range checks and the level table all count hops from hop 0 of
/// the clip, so they have to agree on how many there are; a second copy of this
/// expression is how they would stop agreeing.
pub fn total_hops(pcm_len: usize, format: AudioFormat) -> usize {
    let hop_samples = HOP_SIZE * format.channels.max(1) as usize;
    pcm_len / hop_samples.max(1)
}

/// Reject any hop that lands past the end of the clip, naming the flag that
/// asked for it.
///
/// Validated against the clip rather than in the parser because a hop the
/// capture never reaches is silently *not captured* — it records only the
/// indices it passes — which shortens a strip, and empties a `--frame-at`
/// capture, without either one saying why.
pub fn check_hops(
    hops: &[u32],
    pcm_len: usize,
    format: AudioFormat,
    flag: &str,
) -> Result<(), String> {
    let total = total_hops(pcm_len, format);
    match hops.iter().find(|h| **h as usize >= total) {
        Some(past) => Err(format!(
            "{flag} {past}: the clip is only {total} analysis hops long"
        )),
        None => Ok(()),
    }
}

/// `--strip` frame indices, evenly spaced from just past warm-up to the last
/// analysis frame the PCM produces.
pub fn filmstrip_indices(
    pcm_len: usize,
    format: AudioFormat,
    strip: u32,
) -> Result<Vec<u32>, String> {
    let total = total_hops(pcm_len, format);
    if total <= FILMSTRIP_WARMUP + 1 {
        return Err("audio too short for a filmstrip".to_string());
    }
    let start = FILMSTRIP_WARMUP;
    let end = total - 1;
    let n = strip.max(1);
    if n == 1 {
        return Ok(vec![start as u32]);
    }
    let span = (end - start) as f32;
    Ok((0..n)
        .map(|i| (start as f32 + span * i as f32 / (n - 1) as f32).round() as u32)
        .collect())
}

/// Where each captured frame lands on the strip canvas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StripLayout {
    /// Aspect-preserved thumbnail width.
    pub thumb_w: u32,
    /// Thumbnail height — always [`STRIP_H`].
    pub thumb_h: u32,
    /// Full canvas width, gutters included.
    pub canvas_w: u32,
    /// Full canvas height, gutters included.
    pub canvas_h: u32,
    /// The gutter, so a caller placing pixels doesn't re-derive it.
    pub pad: u32,
}

impl StripLayout {
    /// Left edge of the `i`th thumbnail. The top edge is always [`Self::pad`].
    pub fn x_of(&self, i: usize) -> u32 {
        self.pad + i as u32 * (self.thumb_w + self.pad)
    }
}

/// Lay out `count` frames of `frame_w x frame_h` left-to-right, each scaled to
/// [`STRIP_H`] with the source aspect preserved. An empty strip is an error —
/// a zero-width canvas is not a picture, and the caller wants exit 1.
pub fn filmstrip_layout(frame_w: u32, frame_h: u32, count: usize) -> Result<StripLayout, String> {
    if count == 0 {
        return Err("no frames captured".to_string());
    }
    let thumb_w = (frame_w * STRIP_H / frame_h.max(1)).max(1);
    let n = count as u32;
    Ok(StripLayout {
        thumb_w,
        thumb_h: STRIP_H,
        canvas_w: n * thumb_w + (n + 1) * STRIP_PAD,
        canvas_h: STRIP_H + 2 * STRIP_PAD,
        pad: STRIP_PAD,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stereo(rate: u32) -> AudioFormat {
        AudioFormat {
            sample_rate: rate,
            channels: 2,
        }
    }

    /// Four seconds of 48 kHz stereo is ~375 hops, so a default strip of 8 spans
    /// from just past warm-up to the last analyzable frame.
    #[test]
    fn filmstrip_indices_span_the_available_hops() {
        let format = stereo(48_000);
        let pcm_len = 48_000 * 2 * 4; // 4 s interleaved stereo
        let total = pcm_len / (HOP_SIZE * 2);
        let at = filmstrip_indices(pcm_len, format, 8).expect("4 s is plenty");
        assert_eq!(at.len(), 8);
        assert_eq!(at[0], FILMSTRIP_WARMUP as u32, "starts past warm-up");
        assert_eq!(
            *at.last().unwrap(),
            (total - 1) as u32,
            "ends on the last analysis frame the PCM produces"
        );
        // Monotonic and strictly increasing — a repeated index would silently
        // duplicate a tile and read as a frozen visual.
        assert!(
            at.windows(2).all(|w| w[0] < w[1]),
            "evenly spaced and distinct: {at:?}"
        );
        // Even spacing: consecutive gaps differ by at most the rounding slack.
        let gaps: Vec<i64> = at.windows(2).map(|w| w[1] as i64 - w[0] as i64).collect();
        let (lo, hi) = (gaps.iter().min().unwrap(), gaps.iter().max().unwrap());
        assert!(hi - lo <= 1, "gaps {gaps:?} are even to within rounding");
    }

    #[test]
    fn a_single_frame_strip_samples_just_past_warmup() {
        let format = stereo(48_000);
        let pcm_len = 48_000 * 2 * 2;
        assert_eq!(
            filmstrip_indices(pcm_len, format, 1),
            Ok(vec![FILMSTRIP_WARMUP as u32])
        );
        // `--strip 0` is rejected at parse time, but the floor keeps this total.
        assert_eq!(
            filmstrip_indices(pcm_len, format, 0),
            Ok(vec![FILMSTRIP_WARMUP as u32])
        );
    }

    /// The error case the CLI depends on: asking for a strip of audio that cannot
    /// produce enough analysis frames fails rather than capturing warm-up noise.
    #[test]
    fn filmstrip_indices_error_when_the_audio_is_too_short() {
        let format = stereo(48_000);
        // Exactly warm-up + 1 hops is still too short (the span would be empty).
        let boundary = (FILMSTRIP_WARMUP + 1) * HOP_SIZE * 2;
        let err = filmstrip_indices(boundary, format, 8).expect_err("no usable span");
        assert!(err.contains("too short"), "got {err}");
        assert!(filmstrip_indices(0, format, 8).is_err(), "empty PCM");
        assert!(
            filmstrip_indices(HOP_SIZE * 2, format, 8).is_err(),
            "one hop"
        );
        // One hop past the boundary is the first length that works.
        let ok = boundary + HOP_SIZE * 2;
        let at = filmstrip_indices(ok, format, 4).expect("just long enough");
        assert_eq!(at.len(), 4);
        assert!(at.iter().all(|i| *i as usize >= FILMSTRIP_WARMUP));

        // Mono halves the samples per hop, so the same byte count goes further.
        let mono = AudioFormat {
            sample_rate: 48_000,
            channels: 1,
        };
        assert!(
            filmstrip_indices(boundary, mono, 8).is_ok(),
            "mono gets twice the hops from the same sample count"
        );
    }

    /// `--frame-at` and `--at` share one range check, and it has to agree with
    /// the hop numbering `filmstrip_indices` produces — the last index that
    /// function returns is the last index `check_hops` accepts.
    #[test]
    fn check_hops_rejects_past_the_end_and_agrees_with_the_strip_numbering() {
        let format = stereo(48_000);
        let pcm_len = 48_000 * 2 * 4; // 4 s interleaved stereo
        let total = total_hops(pcm_len, format);
        assert_eq!(total, pcm_len / (HOP_SIZE * 2));

        let last = filmstrip_indices(pcm_len, format, 8).expect("4 s is plenty");
        let last = *last.last().unwrap();
        assert_eq!(last as usize, total - 1);
        assert_eq!(check_hops(&[last], pcm_len, format, "--frame-at"), Ok(()));

        // One past that last index is the error, and the message names the flag
        // that asked so `--at` and `--frame-at` do not report each other's.
        let err = check_hops(&[last + 1], pcm_len, format, "--frame-at")
            .expect_err("one hop past the clip");
        assert!(err.starts_with("--frame-at "), "got {err}");
        assert!(
            err.contains(&format!("only {total} analysis hops")),
            "got {err}"
        );
        let err = check_hops(&[0, 9_999], pcm_len, format, "--at").expect_err("second hop is past");
        assert!(
            err.contains("--at 9999"),
            "the offending hop is named: {err}"
        );

        // Mono halves the samples per hop, so the same byte count reaches twice
        // as far — the check has to follow the format, not a fixed divisor.
        let mono = AudioFormat {
            sample_rate: 48_000,
            channels: 1,
        };
        assert_eq!(total_hops(pcm_len, mono), total * 2);
        assert!(check_hops(&[last + 1], pcm_len, mono, "--frame-at").is_ok());

        // An empty clip has no valid hop at all, including hop 0.
        assert!(check_hops(&[0], 0, format, "--frame-at").is_err());
        // Asking for nothing is vacuously fine; the parser rejects an empty spec.
        assert_eq!(check_hops(&[], pcm_len, format, "--at"), Ok(()));
    }

    #[test]
    fn filmstrip_layout_sizes_the_canvas_for_a_known_frame_set() {
        // 16:9 frames scaled to STRIP_H=200 give 355px-wide thumbs (integer math).
        let l = filmstrip_layout(1280, 720, 8).expect("eight frames");
        assert_eq!(l.thumb_h, STRIP_H);
        assert_eq!(l.thumb_w, 1280 * STRIP_H / 720);
        assert_eq!(l.canvas_w, 8 * l.thumb_w + 9 * STRIP_PAD);
        assert_eq!(l.canvas_h, STRIP_H + 2 * STRIP_PAD);
        // Offsets tile without overlap and the last thumb ends inside the canvas.
        assert_eq!(l.x_of(0), STRIP_PAD);
        assert_eq!(l.x_of(1), STRIP_PAD + l.thumb_w + STRIP_PAD);
        assert_eq!(l.x_of(7) + l.thumb_w + STRIP_PAD, l.canvas_w);

        // A square frame is square at strip height.
        let sq = filmstrip_layout(256, 256, 1).expect("one frame");
        assert_eq!(sq.thumb_w, STRIP_H);
        assert_eq!(sq.canvas_w, STRIP_H + 2 * STRIP_PAD);
    }

    #[test]
    fn filmstrip_layout_rejects_an_empty_frame_set_and_survives_a_zero_height() {
        let err = filmstrip_layout(1280, 720, 0).expect_err("nothing to tile");
        assert!(err.contains("no frames captured"), "got {err}");
        // A degenerate capture must not divide by zero, and never lays out a
        // zero-width thumbnail (which would make an unopenable PNG).
        let l = filmstrip_layout(1280, 0, 2).expect("clamped, not fatal");
        assert!(l.thumb_w >= 1);
        let thin = filmstrip_layout(1, 4096, 2).expect("clamped, not fatal");
        assert_eq!(thin.thumb_w, 1, "floor keeps the canvas non-empty");
    }
}
