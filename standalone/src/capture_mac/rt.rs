//! The ScreenCaptureKit backend's real-time half: the stream-output object SCK
//! calls back on, and everything that call reaches.
//!
//! Split out of the setup half so the panic-denial pragma below covers the
//! callback and only the callback. Setup enumerates shareable content, formats
//! error descriptions and blocks on channels; a file-level pragma over both
//! halves would have to be escaped there, and an allow-riddled file satisfies
//! the hygiene guard's sentinel without meaning anything.
//!
//! SCK invokes the output callback on the single serial dispatch queue passed to
//! `addStreamOutput`, so access to the producer is exclusive without a lock (NFR
//! section 5 forbids locking here anyway). The callback does zero heap
//! allocation, zero locks, zero logging and zero file I/O: the interleave
//! scratch is sized once when this object is built, and the AudioBufferList
//! header lives on the stack.

// Hot-path panic-denial pragma (Plan 0002 Phase 2). The audio callback and
// ring must never panic in production; violations fail the build.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

use std::cell::UnsafeCell;
use std::ptr::NonNull;

use objc2::rc::Retained;
use objc2::{AllocAnyThread, DefinedClass, define_class, msg_send};
use objc2_core_audio_types::AudioBufferList;
use objc2_core_foundation::CFRetained;
use objc2_core_media::{
    CMBlockBuffer, CMSampleBuffer, kCMSampleBufferFlag_AudioBufferList_Assure16ByteAlignment,
};
use objc2_foundation::{NSObject, NSObjectProtocol};
use objc2_screen_capture_kit::{SCStream, SCStreamOutput, SCStreamOutputType};
use rlx_core::audio::SampleProducer;

/// Interleave scratch: whole frames only, preallocated once.
const SCRATCH_SAMPLES: usize = 32_768;
/// SCK audio arrives planar with one buffer per channel; cap what we read.
const MAX_PLANES: usize = 8;
/// Stack storage for the AudioBufferList header + up to MAX_PLANES buffers.
const ABL_STORAGE_BYTES: usize = 256;

/// Interleave planar channel data into `out`. Pure - unit tested below.
/// Returns the number of samples written (frames * planes).
///
/// A short `out` or a short plane stops the copy at the last whole sample it
/// could write rather than panicking: this module denies indexing, and the
/// caller sizes both so the bound is never reached.
fn interleave_planar(
    out: &mut [f32],
    planes: &[&[f32]],
    frame_offset: usize,
    frames: usize,
) -> usize {
    let channels = planes.len();
    let mut written = 0;
    for f in 0..frames {
        for plane in planes {
            let Some(src) = plane.get(frame_offset + f) else {
                return written;
            };
            let Some(dst) = out.get_mut(written) else {
                return written;
            };
            *dst = *src;
            written += 1;
        }
    }
    debug_assert_eq!(written, frames * channels);
    written
}

struct AudioState {
    producer: SampleProducer,
    scratch: Box<[f32]>,
}

/// Ivar wrapper. Safety: SCK invokes the output callback on the single
/// serial dispatch queue passed to addStreamOutput, so access is exclusive
/// without a lock (NFR section 5 forbids locking here anyway).
///
/// `define_class!` emits `impl DefinedClass for StreamOutput { type Ivars =
/// OutputIvars; }`, an interface as visible as `StreamOutput` itself, so this
/// type must be at least `pub(super)` too or the build fails with E0446.
pub(super) struct OutputIvars(UnsafeCell<AudioState>);
unsafe impl Send for OutputIvars {}
unsafe impl Sync for OutputIvars {}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = AllocAnyThread]
    #[name = "RlxStreamOutput"]
    #[ivars = OutputIvars]
    pub(super) struct StreamOutput;

    unsafe impl NSObjectProtocol for StreamOutput {}

    unsafe impl SCStreamOutput for StreamOutput {
        #[unsafe(method(stream:didOutputSampleBuffer:ofType:))]
        fn stream_did_output_sample_buffer_of_type(
            &self,
            _stream: &SCStream,
            sample_buffer: &CMSampleBuffer,
            output_type: SCStreamOutputType,
        ) {
            if output_type == SCStreamOutputType::Audio {
                // Safety: serial queue - see OutputIvars.
                unsafe { self.handle_audio(sample_buffer) };
            }
        }
    }
);

impl StreamOutput {
    pub(super) fn new(producer: SampleProducer) -> Retained<Self> {
        let this = Self::alloc().set_ivars(OutputIvars(UnsafeCell::new(AudioState {
            producer,
            scratch: vec![0.0f32; SCRATCH_SAMPLES].into_boxed_slice(),
        })));
        unsafe { msg_send![super(this), init] }
    }

    /// Real-time path: stack ABL storage, no heap, no locks, no logging.
    unsafe fn handle_audio(&self, sample_buffer: &CMSampleBuffer) {
        let frames = unsafe { sample_buffer.num_samples() };
        if frames <= 0 {
            return;
        }
        let frames = frames as usize;

        #[repr(C, align(16))]
        struct AblStorage([u8; ABL_STORAGE_BYTES]);
        let mut storage = AblStorage([0; ABL_STORAGE_BYTES]);
        let abl_ptr = storage.0.as_mut_ptr().cast::<AudioBufferList>();
        let mut block_buffer: *mut CMBlockBuffer = std::ptr::null_mut();
        let status = unsafe {
            sample_buffer.audio_buffer_list_with_retained_block_buffer(
                std::ptr::null_mut(),
                abl_ptr,
                ABL_STORAGE_BYTES,
                None,
                None,
                kCMSampleBufferFlag_AudioBufferList_Assure16ByteAlignment,
                &mut block_buffer,
            )
        };
        if status != 0 {
            return;
        }
        // Owns the block buffer; dropping releases the sample data reference.
        let _block_guard = NonNull::new(block_buffer).map(|p| unsafe { CFRetained::from_raw(p) });

        let abl = unsafe { &*abl_ptr };
        let plane_count = (abl.mNumberBuffers as usize).min(MAX_PLANES);
        if plane_count == 0 {
            return;
        }
        let buffers = unsafe { std::slice::from_raw_parts(abl.mBuffers.as_ptr(), plane_count) };

        let state = unsafe { &mut *self.ivars().0.get() };
        if plane_count == 1 {
            // Mono or already interleaved - push as-is.
            let Some(buf) = buffers.first() else {
                return;
            };
            if buf.mData.is_null() {
                return;
            }
            let samples = unsafe {
                std::slice::from_raw_parts(
                    buf.mData.cast::<f32>(),
                    buf.mDataByteSize as usize / std::mem::size_of::<f32>(),
                )
            };
            state.producer.push_samples(samples);
            return;
        }

        // Planar: interleave through the fixed scratch, chunked to its size.
        let mut planes: [&[f32]; MAX_PLANES] = [&[]; MAX_PLANES];
        for (slot, buf) in planes.iter_mut().zip(buffers.iter()) {
            if buf.mData.is_null() {
                return;
            }
            *slot = unsafe {
                std::slice::from_raw_parts(
                    buf.mData.cast::<f32>(),
                    buf.mDataByteSize as usize / std::mem::size_of::<f32>(),
                )
            };
        }
        let Some(planes) = planes.get(..plane_count) else {
            return;
        };
        let frames = frames.min(planes.iter().map(|p| p.len()).min().unwrap_or(0));
        let frames_per_chunk = state.scratch.len() / plane_count;
        let mut offset = 0;
        while offset < frames && frames_per_chunk > 0 {
            let n = (frames - offset).min(frames_per_chunk);
            let written = interleave_planar(&mut state.scratch, planes, offset, n);
            if let Some(ready) = state.scratch.get(..written) {
                state.producer.push_samples(ready);
            }
            offset += n;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::interleave_planar;

    #[test]
    fn interleaves_planar_channels_in_frame_order() {
        let left = [1.0f32, 3.0, 5.0];
        let right = [2.0f32, 4.0, 6.0];
        let mut out = [0.0f32; 6];
        let written = interleave_planar(&mut out, &[&left, &right], 0, 3);
        assert_eq!(written, 6);
        assert_eq!(out, [1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
    }

    #[test]
    fn interleaves_with_frame_offset() {
        let left = [0.0f32, 0.0, 7.0, 9.0];
        let right = [0.0f32, 0.0, 8.0, 10.0];
        let mut out = [0.0f32; 4];
        let written = interleave_planar(&mut out, &[&left, &right], 2, 2);
        assert_eq!(written, 4);
        assert_eq!(out, [7.0, 8.0, 9.0, 10.0]);
    }
}
