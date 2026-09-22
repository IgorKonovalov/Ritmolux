//! Framing a byte stream of interleaved native-endian `f32` samples into whole
//! frames, carrying a trailing partial frame into the next read.
//!
//! A blocking byte-oriented capture read (the Linux backend's `pa_simple_read`)
//! is sized in bytes, not frames. Whatever it returns, the ring must only ever
//! see whole interleaved frames — a truncated frame shifts every later sample
//! onto the wrong channel — and the bytes of a partial frame must not be
//! dropped, or the same shift happens one read later.
//!
//! Pure and platform-free, so its test runs on every CI arm rather than only on
//! the one platform whose backend calls it.

/// Bytes in one sample: the capture format is `f32`.
pub(crate) const SAMPLE_BYTES: usize = std::mem::size_of::<f32>();

/// What one [`drain_whole_frames`] call produced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Drained {
    /// Samples written to the head of `out` — always a whole number of frames.
    pub(crate) samples: usize,
    /// Bytes of a partial trailing frame, now moved to the head of `buf`. The
    /// next read appends after them.
    pub(crate) carry: usize,
}

/// Decode every whole frame in `buf[..filled]` into `out`, then move the bytes
/// of a trailing partial frame to the head of `buf`.
///
/// `out` must hold at least `(filled / (channels * SAMPLE_BYTES)) * channels`
/// samples, and `channels` must be non-zero; the caller sizes both once, before
/// its read loop. Allocates nothing.
pub(crate) fn drain_whole_frames(
    buf: &mut [u8],
    filled: usize,
    channels: usize,
    out: &mut [f32],
) -> Drained {
    let frame_bytes = channels * SAMPLE_BYTES;
    let whole = filled - filled % frame_bytes;
    let samples = whole / SAMPLE_BYTES;
    for (dst, src) in out[..samples]
        .iter_mut()
        .zip(buf[..whole].chunks_exact(SAMPLE_BYTES))
    {
        *dst = f32::from_ne_bytes([src[0], src[1], src[2], src[3]]);
    }
    buf.copy_within(whole..filled, 0);
    Drained {
        samples,
        carry: filled - whole,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bytes_of(samples: &[f32]) -> Vec<u8> {
        samples.iter().flat_map(|s| s.to_ne_bytes()).collect()
    }

    #[test]
    fn a_whole_number_of_frames_is_decoded_in_order_with_nothing_carried() {
        let src = [1.0f32, -1.0, 0.5, -0.5];
        let mut buf = bytes_of(&src);
        let mut out = [0.0f32; 4];
        let d = drain_whole_frames(&mut buf, 16, 2, &mut out);
        assert_eq!(
            d,
            Drained {
                samples: 4,
                carry: 0
            }
        );
        assert_eq!(out, src);
    }

    /// A read ending five bytes into a stereo frame pushes only the frame before
    /// it, keeps those five bytes, and completes the frame from the next read —
    /// so the sample stream on the far side is exactly the one that was sent.
    #[test]
    fn a_partial_frame_is_neither_pushed_nor_discarded() {
        let sent = [0.25f32, -0.25, 0.75, -0.75, 0.125, -0.125];
        let all = bytes_of(&sent);
        let mut buf = vec![0u8; all.len()];
        let mut received = Vec::new();
        let mut out = [0.0f32; 6];

        // First read: one whole frame (8 bytes) plus five bytes of the second.
        buf[..13].copy_from_slice(&all[..13]);
        let d = drain_whole_frames(&mut buf, 13, 2, &mut out);
        assert_eq!(
            d,
            Drained {
                samples: 2,
                carry: 5
            }
        );
        assert_eq!(buf[..5], all[8..13], "the partial frame moves to the head");
        received.extend_from_slice(&out[..d.samples]);

        // Second read appends the rest after the carried bytes.
        buf[d.carry..d.carry + 11].copy_from_slice(&all[13..]);
        let d = drain_whole_frames(&mut buf, d.carry + 11, 2, &mut out);
        assert_eq!(
            d,
            Drained {
                samples: 4,
                carry: 0
            }
        );
        received.extend_from_slice(&out[..d.samples]);

        assert_eq!(received, sent);
    }

    #[test]
    fn less_than_one_frame_pushes_nothing_and_keeps_every_byte() {
        let all = bytes_of(&[0.5f32, -0.5]);
        let mut buf = all.clone();
        let mut out = [9.0f32; 2];
        let d = drain_whole_frames(&mut buf, 7, 2, &mut out);
        assert_eq!(
            d,
            Drained {
                samples: 0,
                carry: 7
            }
        );
        assert_eq!(buf[..7], all[..7]);
        assert_eq!(out, [9.0, 9.0], "nothing was written");
    }
}
