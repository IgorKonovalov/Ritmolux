//! The Linux capture backend's real-time half: the loop that runs between
//! stream start and stream stop.
//!
//! Split out of the setup half so the panic-denial pragma below covers the loop
//! and only the loop. The parent module enumerates nothing, but it does format
//! strings, write to stderr and carry an init-time `expect` on the thread spawn;
//! a file-level pragma over both halves would have to be escaped there, and an
//! allow-riddled file satisfies the hygiene guard's sentinel without meaning
//! anything.
//!
//! Everything the loop touches is sized once before it starts: the byte buffer,
//! the sample buffer and the carried partial frame. The loop itself allocates
//! nothing, takes no lock, opens no file and logs nothing (NFR section 5).

// Hot-path panic-denial pragma (Plan 0002 Phase 2). The audio callback and
// ring must never panic in production; violations fail the build.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

use std::sync::atomic::{AtomicBool, Ordering};

use libpulse_simple_binding::Simple;
use rlx_core::audio::SampleProducer;

use crate::capture_frames::drain_whole_frames;

use super::{CHANNELS, FRAME_BYTES, READ_BYTES, READ_FRAMES};

/// The real-time path: block in a read, frame it, push it. Everything it
/// touches was allocated before the loop.
pub(super) fn read_loop(
    stream: &Simple,
    mut producer: SampleProducer,
    stop: &AtomicBool,
    lost: &AtomicBool,
) {
    // One frame of headroom past a read, for a carried partial frame.
    let mut bytes = vec![0u8; READ_BYTES + FRAME_BYTES].into_boxed_slice();
    let mut samples = vec![0.0f32; (READ_FRAMES + 1) * CHANNELS as usize].into_boxed_slice();
    let mut carry = 0;
    while !stop.load(Ordering::Acquire) {
        let filled = carry + READ_BYTES;
        // `carry` is a partial frame, so it is at most `FRAME_BYTES - 1` and
        // `filled` is always inside the buffer sized above. Asked rather than
        // indexed because this module denies slicing outright, and a loop that
        // ends is a better failure here than one that unwinds through the
        // PulseAudio stream it owns.
        let Some(window) = bytes.get_mut(carry..filled) else {
            return;
        };
        if stream.read(window).is_err() {
            lost.store(true, Ordering::Relaxed);
            return;
        }
        let drained = drain_whole_frames(&mut bytes, filled, CHANNELS as usize, &mut samples);
        if let Some(ready) = samples.get(..drained.samples) {
            producer.push_samples(ready);
        }
        carry = drained.carry;
    }
}
