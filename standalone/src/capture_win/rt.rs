//! The WASAPI backend's real-time half: the polling loop that runs between
//! stream start and stream stop, and the scratch it needs.
//!
//! Split out of the setup half so the panic-denial pragma below covers the loop
//! and only the loop. Setup legitimately enumerates endpoints, formats strings,
//! writes to stderr and carries an init-time `expect` on the thread spawn; a
//! file-level pragma over both halves would have to be escaped there, and an
//! allow-riddled file satisfies the hygiene guard's sentinel without meaning
//! anything.
//!
//! The loop obeys NFR section 5: after stream start it performs zero heap
//! allocation, zero locks, zero logging, zero file I/O — it copies device
//! buffers into the SPSC ring and sleeps. It **reports and never decides**
//! (ADR-0142): a dead stream stores one `AtomicBool` and returns.

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
use std::time::Duration;

use rlx_core::audio::SampleProducer;
use windows::Win32::Media::Audio::{
    AUDCLNT_BUFFERFLAGS_SILENT, AUDCLNT_E_DEVICE_INVALIDATED, IAudioCaptureClient, IAudioClient,
};

/// WASAPI shared-mode periods are 10 ms; polling faster than the period keeps
/// delivery latency well under the 15 ms capture allocation in NFR section 3.
const POLL_INTERVAL: Duration = Duration::from_millis(4);

/// Scratch zeros pushed when a packet carries the SILENT flag (its data
/// pointer is not required to be valid then). Preallocated before the loop.
const SILENCE_CHUNK_SAMPLES: usize = 4096;

pub(super) struct Stream {
    pub(super) audio_client: IAudioClient,
    pub(super) capture_client: IAudioCaptureClient,
    pub(super) producer: SampleProducer,
    // Interleaving width of the captured stream. The ring producer does
    // not expose the format (Plan 0005), so carry the channel count
    // here.
    pub(super) channels: usize,
}

/// Whether a packet-call error means the stream is *gone* rather than merely
/// unhappy.
///
/// `AUDCLNT_E_DEVICE_INVALIDATED` is what WASAPI reports when the endpoint is
/// removed, disabled, or has its format changed underneath the client. Every
/// other code stays transient: the loop cannot tell a hiccup from a teardown,
/// and promoting both would tear capture down on noise.
fn is_device_lost(err: &windows::core::Error) -> bool {
    err.code() == AUDCLNT_E_DEVICE_INVALIDATED
}

/// The real-time loop. From here until `stop` flips: no allocation, no locks,
/// no logging, no I/O — copy packets into the ring, release, sleep.
///
/// A dead stream ends the loop rather than sleeping back into it: `lost` is
/// stored and the function returns, leaving the shell to decide what runs next.
/// Spinning on an invalidated device delivers nothing and says nothing, which is
/// the failure this exit exists to end.
pub(super) fn run_capture_loop(stream: &mut Stream, stop: &AtomicBool, lost: &AtomicBool) {
    // Preallocated so silent packets cost no heap work inside the loop.
    let silence = [0.0f32; SILENCE_CHUNK_SAMPLES];
    let channels = stream.channels;
    let Stream {
        capture_client,
        producer,
        ..
    } = stream;

    while !stop.load(Ordering::Acquire) {
        loop {
            let packet_frames = match unsafe { capture_client.GetNextPacketSize() } {
                Ok(n) => n,
                Err(e) => {
                    if is_device_lost(&e) {
                        lost.store(true, Ordering::Relaxed);
                        return;
                    }
                    break;
                }
            };
            if packet_frames == 0 {
                break;
            }
            let mut data: *mut u8 = std::ptr::null_mut();
            let mut frames_read: u32 = 0;
            let mut flags: u32 = 0;
            let got = unsafe {
                capture_client.GetBuffer(&mut data, &mut frames_read, &mut flags, None, None)
            };
            if let Err(e) = got {
                if is_device_lost(&e) {
                    lost.store(true, Ordering::Relaxed);
                    return;
                }
                break;
            }
            let sample_count = frames_read as usize * channels;
            if flags & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 != 0 {
                push_silence(producer, &silence, sample_count, channels);
            } else if !data.is_null() && sample_count > 0 {
                // Safety: WASAPI hands us frames_read frames of the mix
                // format we validated as f32 at setup.
                let samples =
                    unsafe { std::slice::from_raw_parts(data as *const f32, sample_count) };
                // Drop-on-full is the ring's policy; nothing to retry
                // without blocking.
                let _ = producer.push_samples(samples);
            }
            if let Err(e) = unsafe { capture_client.ReleaseBuffer(frames_read) } {
                if is_device_lost(&e) {
                    lost.store(true, Ordering::Relaxed);
                    return;
                }
                break;
            }
        }
        std::thread::sleep(POLL_INTERVAL);
    }
}

fn push_silence(
    producer: &mut SampleProducer,
    silence: &[f32],
    mut remaining: usize,
    channels: usize,
) {
    let chunk_max = silence.len() / channels * channels;
    while remaining > 0 && chunk_max > 0 {
        let n = remaining.min(chunk_max);
        // `n <= chunk_max <= silence.len()`, so the chunk is always there; asked
        // rather than sliced because this module denies slicing outright.
        let Some(chunk) = silence.get(..n) else {
            return;
        };
        let written = producer.push_samples(chunk);
        if written < n {
            break;
        }
        remaining -= n;
    }
}
