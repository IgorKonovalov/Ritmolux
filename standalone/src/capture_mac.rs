//! macOS loopback capture via ScreenCaptureKit (macOS 13+).
//!
//! ScreenCaptureKit is the only first-party way to tap system audio output;
//! it requires the user to grant the *screen recording* permission (the
//! stream captures a 2x2 px, 1 fps throwaway video alongside the audio —
//! SCK will not run audio-only). The documented fallback is a virtual
//! device (BlackHole): set it as the output and this capture path is not
//! needed — that route lands with the capture-device work in the
//! live-performance plan.
//!
//! Same contract as `capture_win`: samples flow into the core's SPSC ring,
//! and the sample-handler callback does zero heap allocation, zero locks,
//! zero logging, zero file I/O (NFR section 5). SCK delivers audio on a
//! dedicated serial dispatch queue, which serializes access to the producer.
//!
//! This file is the setup half — content enumeration, the filter, the stream
//! configuration and the handle. The callback and everything it reaches live in
//! [`rt`], which carries the panic-denial pragma the hygiene guard checks for.

mod rt;

use std::sync::mpsc;
use std::time::Duration;

use block2::RcBlock;
use dispatch2::{DispatchQueue, DispatchRetained};
use objc2::AllocAnyThread;
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_core_media::{CMTime, CMTimeFlags};
use objc2_foundation::{NSArray, NSError};
use objc2_screen_capture_kit::{
    SCContentFilter, SCShareableContent, SCStream, SCStreamConfiguration, SCStreamOutputType,
};
use rlx_core::audio::{AudioFormat, SampleConsumer, intake};

use rt::StreamOutput;

/// Same headroom as the Windows path (~340 ms @ 48 kHz stereo).
const RING_CAPACITY_FRAMES: usize = 16_384;
/// The format we ask SCK to deliver; it resamples internally.
const SAMPLE_RATE: u32 = 48_000;
const CHANNELS: u16 = 2;

#[derive(Debug)]
pub enum CaptureError {
    /// Shareable-content enumeration failed — usually the screen-recording
    /// permission was denied (System Settings > Privacy > Screen Recording).
    ShareableContent(String),
    NoDisplay,
    Stream(String),
    Format(rlx_core::audio::FormatError),
    Timeout,
}

impl std::fmt::Display for CaptureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CaptureError::ShareableContent(e) => {
                write!(
                    f,
                    "ScreenCaptureKit content enumeration failed: {e} (screen-recording permission?)"
                )
            }
            CaptureError::NoDisplay => write!(f, "no display available to capture"),
            CaptureError::Stream(e) => write!(f, "ScreenCaptureKit stream error: {e}"),
            CaptureError::Format(e) => write!(f, "capture format rejected by core: {e}"),
            CaptureError::Timeout => write!(f, "timed out waiting for ScreenCaptureKit"),
        }
    }
}

impl std::error::Error for CaptureError {}

pub struct CaptureHandle {
    stream: Retained<SCStream>,
    // Kept alive for the stream's callbacks; released after stop.
    _output: Retained<StreamOutput>,
    _queue: DispatchRetained<DispatchQueue>,
    format: AudioFormat,
}

impl CaptureHandle {
    pub fn format(&self) -> AudioFormat {
        self.format
    }
}

impl Drop for CaptureHandle {
    fn drop(&mut self) {
        // Fire-and-forget stop; the retained output/queue outlive any
        // in-flight callback because we hold them until self drops.
        unsafe { self.stream.stopCaptureWithCompletionHandler(None) };
    }
}

/// Start system-audio capture. Blocks briefly while ScreenCaptureKit
/// enumerates content and starts the stream (first run triggers the
/// screen-recording permission prompt). Capture stops when the handle drops.
pub fn start() -> Result<(CaptureHandle, SampleConsumer), CaptureError> {
    let format = AudioFormat {
        sample_rate: SAMPLE_RATE,
        channels: CHANNELS,
    };
    let (producer, consumer) =
        intake(format, RING_CAPACITY_FRAMES).map_err(CaptureError::Format)?;

    // 1. Enumerate shareable content (async -> block until the callback).
    let (content_tx, content_rx) = mpsc::channel();
    let content_block = RcBlock::new(
        move |content: *mut SCShareableContent, error: *mut NSError| {
            let result = if content.is_null() {
                Err(describe_error(error))
            } else {
                // Safety: non-null content from the callback is valid.
                unsafe { Retained::retain(content) }.ok_or_else(|| "retain failed".to_string())
            };
            let _ = content_tx.send(result);
        },
    );
    unsafe { SCShareableContent::getShareableContentWithCompletionHandler(&content_block) };
    let content = content_rx
        .recv_timeout(Duration::from_secs(15))
        .map_err(|_| CaptureError::Timeout)?
        .map_err(CaptureError::ShareableContent)?;

    // 2. Filter: the first display, excluding nothing - we only want audio.
    let displays = unsafe { content.displays() };
    let display = displays.firstObject().ok_or(CaptureError::NoDisplay)?;
    let filter = unsafe {
        SCContentFilter::initWithDisplay_excludingWindows(
            SCContentFilter::alloc(),
            &display,
            &NSArray::new(),
        )
    };

    // 3. Configuration: audio on, video minimized to a 2x2 px 1 fps stub.
    let config = unsafe { SCStreamConfiguration::new() };
    unsafe {
        config.setCapturesAudio(true);
        config.setExcludesCurrentProcessAudio(true);
        config.setSampleRate(SAMPLE_RATE as isize);
        config.setChannelCount(CHANNELS as isize);
        config.setWidth(2);
        config.setHeight(2);
        config.setMinimumFrameInterval(CMTime {
            value: 1,
            timescale: 1,
            flags: CMTimeFlags::Valid,
            epoch: 0,
        });
    }

    // 4. Stream + audio output on a dedicated serial queue.
    let output = StreamOutput::new(producer);
    let queue = DispatchQueue::new("rlx-sck-audio", None);
    let stream = unsafe {
        SCStream::initWithFilter_configuration_delegate(SCStream::alloc(), &filter, &config, None)
    };
    unsafe {
        stream.addStreamOutput_type_sampleHandlerQueue_error(
            ProtocolObject::from_ref(&*output),
            SCStreamOutputType::Audio,
            Some(&queue),
        )
    }
    .map_err(|e| CaptureError::Stream(e.localizedDescription().to_string()))?;

    // 5. Start and wait for the completion callback.
    let (start_tx, start_rx) = mpsc::channel();
    let start_block = RcBlock::new(move |error: *mut NSError| {
        let result = if error.is_null() {
            Ok(())
        } else {
            Err(describe_error(error))
        };
        let _ = start_tx.send(result);
    });
    unsafe { stream.startCaptureWithCompletionHandler(Some(&start_block)) };
    start_rx
        .recv_timeout(Duration::from_secs(15))
        .map_err(|_| CaptureError::Timeout)?
        .map_err(CaptureError::Stream)?;

    Ok((
        CaptureHandle {
            stream,
            _output: output,
            _queue: queue,
            format,
        },
        consumer,
    ))
}

fn describe_error(error: *mut NSError) -> String {
    if error.is_null() {
        return "unknown error".to_string();
    }
    // Safety: non-null NSError from the callback is valid for the call.
    unsafe { (*error).localizedDescription() }.to_string()
}
