//! A minimal hand-rolled 16-bit-PCM WAV reader (no decoder dependency).
//!
//! Only the **parse** lives here — the file read stays in the example, so this is
//! a pure function of a byte slice and can be tested against hand-built headers
//! and truncated garbage. Every read goes through the bounds-checked [`le_u16`] /
//! [`le_u32`] helpers or `slice::get`, so a malformed file is an `Err` and never
//! a panic: `shot` is the harness the `preset-author` lane self-verifies through,
//! and a panic there reads as a broken preset.

use rlx_core::audio::AudioFormat;

/// Decode uncompressed 16-bit PCM (`fmt ` format tag 1) from a whole WAV file's
/// bytes into interleaved `-1.0..=1.0` samples plus the header's declared format.
/// Any channel count / sample rate the core accepts is fine; other encodings are
/// a documented followup.
///
/// `label` names the source in the not-a-WAV message (the example passes the
/// path), so the CLI's text is unchanged by this split.
pub fn parse_wav_16bit(bytes: &[u8], label: &str) -> Result<(Vec<f32>, AudioFormat), String> {
    if bytes.get(0..4) != Some(b"RIFF") || bytes.get(8..12) != Some(b"WAVE") {
        return Err(format!("{label} is not a RIFF/WAVE file"));
    }

    let mut channels = 0u16;
    let mut sample_rate = 0u32;
    let mut data: Option<&[u8]> = None;
    let mut pos = 12usize;
    while pos + 8 <= bytes.len() {
        let id = bytes.get(pos..pos + 4).unwrap_or(&[]);
        let size = le_u32(bytes, pos + 4).unwrap_or(0) as usize;
        let body = pos + 8;
        let end = body.saturating_add(size).min(bytes.len());
        match id {
            b"fmt " => {
                let audio_format = le_u16(bytes, body).unwrap_or(0);
                let bits = le_u16(bytes, body + 14).unwrap_or(0);
                if audio_format != 1 {
                    return Err("only uncompressed PCM (format 1) WAV is supported".to_string());
                }
                if bits != 16 {
                    return Err(format!(
                        "only 16-bit PCM WAV is supported (found {bits}-bit)"
                    ));
                }
                channels = le_u16(bytes, body + 2).unwrap_or(0);
                sample_rate = le_u32(bytes, body + 4).unwrap_or(0);
            }
            b"data" => data = bytes.get(body..end),
            _ => {}
        }
        // Chunks are word-aligned. `saturating_add` because a bogus 32-bit size
        // would otherwise overflow the cursor on a 32-bit host.
        pos = body.saturating_add(size).saturating_add(size & 1);
    }

    let data = data.ok_or("WAV has no data chunk")?;
    let samples: Vec<f32> = data
        .chunks_exact(2)
        .map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / 32768.0)
        .collect();
    let format = AudioFormat {
        sample_rate,
        channels,
    }
    .validate()
    .map_err(|e| format!("unusable WAV format: {e}"))?;
    Ok((samples, format))
}

/// Little-endian `u16` at `at`, or `None` when the slice is too short.
pub fn le_u16(b: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes([*b.get(at)?, *b.get(at + 1)?]))
}

/// Little-endian `u32` at `at`, or `None` when the slice is too short.
pub fn le_u32(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes([
        *b.get(at)?,
        *b.get(at + 1)?,
        *b.get(at + 2)?,
        *b.get(at + 3)?,
    ]))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a canonical 44-byte-header WAV around `samples` (already interleaved
    /// `i16`), so the tests exercise the real parse rather than a stub.
    fn wav(
        channels: u16,
        sample_rate: u32,
        bits: u16,
        format_tag: u16,
        samples: &[i16],
    ) -> Vec<u8> {
        let block_align = channels * bits / 8;
        let byte_rate = sample_rate * block_align as u32;
        let data_len = (samples.len() * 2) as u32;
        let mut b = Vec::new();
        b.extend_from_slice(b"RIFF");
        b.extend_from_slice(&(36 + data_len).to_le_bytes());
        b.extend_from_slice(b"WAVE");
        b.extend_from_slice(b"fmt ");
        b.extend_from_slice(&16u32.to_le_bytes());
        b.extend_from_slice(&format_tag.to_le_bytes());
        b.extend_from_slice(&channels.to_le_bytes());
        b.extend_from_slice(&sample_rate.to_le_bytes());
        b.extend_from_slice(&byte_rate.to_le_bytes());
        b.extend_from_slice(&block_align.to_le_bytes());
        b.extend_from_slice(&bits.to_le_bytes());
        b.extend_from_slice(b"data");
        b.extend_from_slice(&data_len.to_le_bytes());
        for s in samples {
            b.extend_from_slice(&s.to_le_bytes());
        }
        b
    }

    #[test]
    fn a_16bit_stereo_wav_decodes_to_the_declared_format_and_sample_count() {
        // Four interleaved stereo frames = eight samples.
        let pcm = [0i16, -32768, 32767, 16384, -16384, 0, 1, -1];
        let bytes = wav(2, 44_100, 16, 1, &pcm);
        let (samples, format) = parse_wav_16bit(&bytes, "clip.wav").expect("well-formed WAV");
        assert_eq!(format.sample_rate, 44_100);
        assert_eq!(format.channels, 2);
        assert_eq!(samples.len(), 8, "interleaved samples, not frames");
        // Exact scaling: i16 / 32768, so -32768 is exactly -1.0 and +32767 is
        // just under +1.0. A sign or endianness slip would show here.
        assert_eq!(samples[0], 0.0);
        assert_eq!(samples[1], -1.0);
        assert!((samples[2] - 0.999_969_5).abs() < 1e-6, "{}", samples[2]);
        assert_eq!(samples[3], 0.5);
        assert_eq!(samples[4], -0.5);
        assert!(samples.iter().all(|s| (-1.0..=1.0).contains(s)));
    }

    #[test]
    fn a_mono_wav_reports_one_channel() {
        let bytes = wav(1, 48_000, 16, 1, &[100, -100, 200]);
        let (samples, format) = parse_wav_16bit(&bytes, "mono.wav").expect("well-formed WAV");
        assert_eq!(format.channels, 1);
        assert_eq!(format.sample_rate, 48_000);
        assert_eq!(samples.len(), 3);
    }

    /// The claim this whole module exists for: garbage in, clean `Err` out —
    /// never a panic, and never a plausible-but-wrong sample count.
    #[test]
    fn malformed_input_is_a_clean_error_and_never_a_panic() {
        let good = wav(2, 44_100, 16, 1, &[1, 2, 3, 4]);
        /// Canonical header length: 12 (RIFF/WAVE) + 24 (`fmt `) + 8 (`data`).
        const HEADER: usize = 44;
        assert_eq!(
            good.len(),
            HEADER + 8,
            "four stereo samples after the header"
        );

        // Not a RIFF/WAVE container at all.
        let err = parse_wav_16bit(b"not audio at all", "junk.bin").expect_err("not RIFF");
        assert!(
            err.contains("junk.bin is not a RIFF/WAVE file"),
            "got {err}"
        );
        assert!(parse_wav_16bit(&[], "empty.wav").is_err(), "empty input");

        // Every prefix that cuts into the *header* is an error — not a panic, and
        // not a partially-filled format that would mislead the analyzer.
        for cut in 0..HEADER {
            assert!(
                parse_wav_16bit(&good[..cut], "cut.wav").is_err(),
                "a {cut}-byte prefix truncates the header and must not parse"
            );
        }

        // A prefix that cuts into the *sample data* is a short read, and the
        // count must match the bytes actually present — never the declared size.
        for cut in HEADER..=good.len() {
            let (samples, format) = parse_wav_16bit(&good[..cut], "short.wav")
                .unwrap_or_else(|e| panic!("{cut}-byte file: {e}"));
            assert_eq!(format.channels, 2);
            assert_eq!(
                samples.len(),
                (cut - HEADER) / 2,
                "{cut}-byte file decodes only whole samples that are present"
            );
        }

        // A `data` chunk claiming more bytes than the file holds is clamped to
        // what is there, not read past the end.
        let mut lying = good.clone();
        let data_size_at = HEADER - 8;
        lying[data_size_at + 4..data_size_at + 8].copy_from_slice(&9_999u32.to_le_bytes());
        let (samples, _) = parse_wav_16bit(&lying, "lying.wav").expect("clamped, not fatal");
        assert_eq!(samples.len(), 4, "only the bytes actually present decode");

        // A chunk header with no body at all: the loop must end, not spin.
        let mut headerless = good[..12].to_vec();
        headerless.extend_from_slice(b"data");
        assert!(parse_wav_16bit(&headerless, "stub.wav").is_err());

        // A chunk declaring a size near `usize::MAX` must not overflow the cursor.
        let mut huge = good[..12].to_vec();
        huge.extend_from_slice(b"junk");
        huge.extend_from_slice(&u32::MAX.to_le_bytes());
        huge.extend_from_slice(&[0u8; 8]);
        assert!(parse_wav_16bit(&huge, "huge.wav").is_err());
    }

    #[test]
    fn unsupported_encodings_are_named_rather_than_guessed() {
        // IEEE float (tag 3) and 24-bit are both rejected with their own message.
        let float_tag = wav(2, 44_100, 32, 3, &[1, 2]);
        let err = parse_wav_16bit(&float_tag, "f32.wav").expect_err("format tag 3");
        assert!(err.contains("uncompressed PCM (format 1)"), "got {err}");

        let deep = wav(2, 44_100, 24, 1, &[1, 2]);
        let err = parse_wav_16bit(&deep, "24bit.wav").expect_err("24-bit");
        assert!(err.contains("found 24-bit"), "got {err}");

        // A header the core's boundary validation refuses (4 kHz is below
        // MIN_SAMPLE_RATE) fails at the boundary, not silently downstream.
        let slow = wav(2, 4_000, 16, 1, &[1, 2]);
        let err = parse_wav_16bit(&slow, "slow.wav").expect_err("4 kHz");
        assert!(err.contains("unusable WAV format"), "got {err}");

        // No `data` chunk: a `fmt `-only file has no samples to decode.
        let fmt_only = &wav(2, 44_100, 16, 1, &[])[..36];
        let err = parse_wav_16bit(fmt_only, "fmtonly.wav").expect_err("no data chunk");
        assert!(err.contains("no data chunk"), "got {err}");
    }

    #[test]
    fn odd_sized_chunks_stay_word_aligned() {
        // An odd-length `LIST` chunk before `data` carries a pad byte; missing
        // the alignment would desynchronize the cursor and lose `data`.
        let mut b = Vec::new();
        b.extend_from_slice(b"RIFF");
        b.extend_from_slice(&0u32.to_le_bytes());
        b.extend_from_slice(b"WAVE");
        b.extend_from_slice(b"LIST");
        b.extend_from_slice(&3u32.to_le_bytes());
        b.extend_from_slice(&[b'I', b'N', b'F', 0]); // 3 bytes + 1 pad
        let tail = wav(2, 44_100, 16, 1, &[7, -7]);
        b.extend_from_slice(&tail[12..]); // fmt + data
        let (samples, format) = parse_wav_16bit(&b, "list.wav").expect("padded chunk skipped");
        assert_eq!(format.channels, 2);
        assert_eq!(samples.len(), 2);
    }

    #[test]
    fn the_little_endian_readers_are_bounds_checked() {
        let b = [0x01, 0x02, 0x03, 0x04, 0x05];
        assert_eq!(le_u16(&b, 0), Some(0x0201));
        assert_eq!(le_u16(&b, 3), Some(0x0504));
        assert_eq!(le_u16(&b, 4), None, "one byte short");
        assert_eq!(le_u16(&b, 99), None);
        assert_eq!(le_u32(&b, 0), Some(0x0403_0201));
        assert_eq!(le_u32(&b, 1), Some(0x0504_0302));
        assert_eq!(le_u32(&b, 2), None, "two bytes short");
        assert_eq!(le_u32(&[], 0), None);
    }
}
