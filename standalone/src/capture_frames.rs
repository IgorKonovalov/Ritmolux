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
//!
//! **It runs on the capture thread.** The Linux backend's real-time loop calls
//! [`drain_whole_frames`] once per read, so this file carries the same
//! panic-denial pragma as the `capture_*/` loops, and the hygiene guard lists it
//! beside them.

// Hot-path panic-denial pragma (Plan 0002 Phase 2). The audio callback and
// ring must never panic in production; violations fail the build.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

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
/// The caller sizes `out` once, before its read loop, to hold every whole frame
/// a read can produce: `(filled / (channels * SAMPLE_BYTES)) * channels`
/// samples. Allocates nothing and
/// **cannot panic**: a `filled` past the end of `buf` is clamped to it, an `out`
/// too small for every whole frame decodes only the frames it holds and carries
/// the rest, and zero `channels` decodes nothing and carries every byte. Each of
/// those is a caller bug the caller then sees as a carry larger than a frame,
/// never as an unwind through the capture thread.
pub(crate) fn drain_whole_frames(
    buf: &mut [u8],
    filled: usize,
    channels: usize,
    out: &mut [f32],
) -> Drained {
    let frame_bytes = channels.saturating_mul(SAMPLE_BYTES);
    let filled = filled.min(buf.len());
    // Whole frames `out` can take, in bytes; zero when `channels` is zero.
    let room = out
        .len()
        .checked_div(channels)
        .map_or(0, |frames| frames.saturating_mul(frame_bytes));
    let whole = filled
        .checked_rem(frame_bytes)
        .map_or(0, |partial| filled - partial)
        .min(room);
    let samples = whole / SAMPLE_BYTES;
    let src = buf.get(..whole).unwrap_or_default();
    for (dst, bytes) in out.iter_mut().zip(src.chunks_exact(SAMPLE_BYTES)) {
        if let Ok(sample) = bytes.try_into() {
            *dst = f32::from_ne_bytes(sample);
        }
    }
    // In bounds by construction: `whole <= filled <= buf.len()`.
    buf.copy_within(whole..filled, 0);
    Drained {
        samples,
        carry: filled - whole,
    }
}

#[cfg(test)]
mod tests {
    // Tests index fixed-size arrays and known-length buffers freely; the
    // hot-path pragma above cascades here, so re-allow indexing for tests.
    #![allow(clippy::indexing_slicing)]

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

    /// The three caller bugs the contract names end in a carry, not a panic:
    /// zero channels, an `out` too small for every whole frame, and a `filled`
    /// past the end of `buf`.
    #[test]
    fn a_caller_bug_is_a_carry_and_never_a_panic() {
        let all = bytes_of(&[1.0f32, -1.0, 0.5, -0.5]);

        let mut buf = all.clone();
        let mut out = [0.0f32; 4];
        let d = drain_whole_frames(&mut buf, 16, 0, &mut out);
        assert_eq!(
            d,
            Drained {
                samples: 0,
                carry: 16
            },
            "zero channels decode nothing"
        );

        let mut buf = all.clone();
        let mut out = [0.0f32; 2];
        let d = drain_whole_frames(&mut buf, 16, 2, &mut out);
        assert_eq!(
            d,
            Drained {
                samples: 2,
                carry: 8
            },
            "only the frame `out` holds"
        );
        assert_eq!(out, [1.0, -1.0]);
        assert_eq!(
            buf[..8],
            all[8..16],
            "the frame that did not fit is carried"
        );

        let mut buf = all.clone();
        let mut out = [0.0f32; 4];
        let d = drain_whole_frames(&mut buf, 64, 2, &mut out);
        assert_eq!(
            d,
            Drained {
                samples: 4,
                carry: 0
            },
            "`filled` clamps to `buf`"
        );
    }
}
