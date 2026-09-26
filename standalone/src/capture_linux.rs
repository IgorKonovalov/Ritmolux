//! Linux system-audio capture through PulseAudio's simple API (ADR-0131).
//!
//! Opens a record stream on `@DEFAULT_MONITOR@` — the monitor source of
//! whichever sink is the default, i.e. exactly what the machine is playing —
//! at 48 kHz stereo `f32`. `pipewire-pulse` serves the same protocol, so one
//! client covers both sound servers a current desktop runs.
//!
//! Same contract as `capture_win` and `capture_mac`: samples flow into the
//! core's SPSC ring. Unlike those two, nothing hands us a callback — the
//! simple API is a blocking read, and this module owns the thread it blocks
//! on. The read loop allocates nothing, takes no lock, opens no file and logs
//! nothing (NFR section 5): both buffers are sized once before it starts.
//!
//! This file is the setup half — connecting, format negotiation, the handle and
//! its shutdown. The loop itself lives in [`rt`], which carries the panic-denial
//! pragma the hygiene guard checks for.
//!
//! The simple API cannot enumerate sources, so there is no device selection
//! here; `[input] device` is inert on Linux.

mod rt;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::Duration;

use libpulse_binding::def::BufferAttr;
use libpulse_binding::error::{Code, PAErr};
use libpulse_binding::sample::{Format, Spec};
use libpulse_binding::stream::Direction;
use libpulse_simple_binding::Simple;
use rlx_core::audio::{AudioFormat, SampleConsumer, intake};

use crate::capture_frames::SAMPLE_BYTES;

/// Same headroom as the other two backends (~340 ms @ 48 kHz stereo).
const RING_CAPACITY_FRAMES: usize = 16_384;
const SAMPLE_RATE: u32 = 48_000;
const CHANNELS: u16 = 2;
/// The source this backend opens: the default sink's monitor, resolved by the
/// server. Also the endpoint name the verdict reports, since the simple API
/// cannot ask the server what it resolved to.
pub const DEVICE: &str = "@DEFAULT_MONITOR@";
/// Frames per blocking read: 10 ms at 48 kHz. It bounds both the latency one
/// read adds and how long the thread can take to notice `stop` while audio
/// flows.
const READ_FRAMES: usize = 480;
const FRAME_BYTES: usize = CHANNELS as usize * SAMPLE_BYTES;
const READ_BYTES: usize = READ_FRAMES * FRAME_BYTES;
/// How many 1 ms polls dropping the handle waits for the thread to finish its
/// read before detaching it — about 250 ms, counted rather than timed because
/// the shell reads no wall clock here. A read only outlives it when the server
/// has stopped delivering, and the shell's shutdown must not hang on that.
const STOP_POLLS: u32 = 250;
const APP_NAME: &str = "Ritmolux";

#[derive(Debug)]
pub enum CaptureError {
    /// No PulseAudio-protocol server could be reached — neither PulseAudio nor
    /// `pipewire-pulse` is running for this user. No source was reached.
    Connect(PAErr),
    /// The server was reached and refused the record stream.
    Stream(PAErr),
    Format(rlx_core::audio::FormatError),
    ThreadDied,
}

impl CaptureError {
    /// Whether this failure happened before any endpoint was reached — the same
    /// question `capture_win::CaptureError::is_activation` answers there.
    pub fn is_activation(&self) -> bool {
        matches!(self, CaptureError::Connect(_))
    }

    /// Sort a `pa_simple_new` error: a failure to reach the server at all is not
    /// a statement about the monitor source.
    fn from_open(err: PAErr) -> Self {
        match Code::try_from(err) {
            Ok(
                Code::ConnectionRefused
                | Code::ConnectionTerminated
                | Code::InvalidServer
                | Code::Access
                | Code::AuthKey,
            ) => CaptureError::Connect(err),
            _ => CaptureError::Stream(err),
        }
    }
}

impl std::fmt::Display for CaptureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CaptureError::Connect(e) => write!(
                f,
                "no PulseAudio server reachable (is pipewire-pulse running?): {e}"
            ),
            CaptureError::Stream(e) => write!(f, "PulseAudio record stream on {DEVICE}: {e}"),
            CaptureError::Format(e) => write!(f, "capture format rejected by core: {e}"),
            CaptureError::ThreadDied => write!(f, "capture thread died during setup"),
        }
    }
}

impl std::error::Error for CaptureError {}

pub struct CaptureHandle {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    /// Set by the capture thread when a read fails — the server went away. One
    /// store from that thread, one relaxed load per frame from the shell.
    lost: Arc<AtomicBool>,
    format: AudioFormat,
}

impl CaptureHandle {
    pub fn format(&self) -> AudioFormat {
        self.format
    }

    /// Whether the capture thread has reported its stream dead.
    pub fn lost(&self) -> bool {
        self.lost.load(Ordering::Relaxed)
    }
}

impl Drop for CaptureHandle {
    /// Stop the thread and close the stream, without hanging on a read.
    ///
    /// The thread checks `stop` between reads, so while audio flows it exits
    /// within one read. If it has not finished inside [`STOP_POLLS`] the server
    /// has stopped delivering, and the thread is detached rather than joined: it
    /// owns the stream and the producer, drops both when its read returns, and
    /// its ring has no reader: the shell drops the consumer with the handle. The
    /// shell's next `start` builds a new ring, so the single-producer invariant
    /// holds either way.
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        let Some(thread) = self.thread.take() else {
            return;
        };
        for _ in 0..STOP_POLLS {
            if thread.is_finished() {
                break;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        if thread.is_finished() {
            let _ = thread.join();
        }
    }
}

/// Start system-audio capture on the default sink's monitor. Blocks while the
/// stream connects; capture stops when the handle drops.
pub fn start() -> Result<(CaptureHandle, SampleConsumer), CaptureError> {
    let format = AudioFormat {
        sample_rate: SAMPLE_RATE,
        channels: CHANNELS,
    };
    // `intake` is the boundary: it validates the format, once, for the ring.
    let (producer, consumer) =
        intake(format, RING_CAPACITY_FRAMES).map_err(CaptureError::Format)?;

    let stop = Arc::new(AtomicBool::new(false));
    let lost = Arc::new(AtomicBool::new(false));
    let (thread_stop, thread_lost) = (Arc::clone(&stop), Arc::clone(&lost));
    // The stream is opened on the capture thread and never leaves it; the
    // opening result comes back once over this channel.
    let (setup_tx, setup_rx) = mpsc::channel();
    let thread = std::thread::Builder::new()
        .name("rlx-pulse-capture".into())
        .spawn(move || match open_stream() {
            Ok(stream) => {
                let _ = setup_tx.send(Ok(()));
                rt::read_loop(&stream, producer, &thread_stop, &thread_lost);
            }
            Err(e) => {
                let _ = setup_tx.send(Err(e));
            }
        })
        .expect("spawning the capture thread is an init-time invariant");

    match setup_rx.recv() {
        Ok(Ok(())) => Ok((
            CaptureHandle {
                stop,
                thread: Some(thread),
                lost,
                format,
            },
            consumer,
        )),
        Ok(Err(e)) => {
            let _ = thread.join();
            Err(e)
        }
        Err(_) => {
            let _ = thread.join();
            Err(CaptureError::ThreadDied)
        }
    }
}

fn open_stream() -> Result<Simple, CaptureError> {
    let spec = Spec {
        format: Format::FLOAT32NE,
        channels: CHANNELS as u8,
        rate: SAMPLE_RATE,
    };
    debug_assert!(spec.is_valid());
    // Without an explicit `fragsize` the server picks a record fragment of up
    // to about two seconds, and every read waits for one — the visuals would
    // trail the music by that much. One read's worth keeps the delivery
    // cadence at the loop's own. `u32::MAX` leaves the rest at the server's
    // default.
    let attr = BufferAttr {
        maxlength: u32::MAX,
        tlength: u32::MAX,
        prebuf: u32::MAX,
        minreq: u32::MAX,
        fragsize: READ_BYTES as u32,
    };
    Simple::new(
        None,
        APP_NAME,
        Direction::Record,
        Some(DEVICE),
        "system audio",
        &spec,
        None,
        Some(&attr),
    )
    .map_err(CaptureError::from_open)
}
