//! The Spout sender: the only Rust in this repository that names Spout
//! (Plan 0115 Phase 3, ADR-0125).
//!
//! `core` produces frames and knows nothing about where they go; the shell owns
//! the sink exactly as it owns the audio source. Spout is Windows, D3D11 and
//! third-party — everything the source-agnostic, GPU-abstract core rule forbids
//! — so it stops here, and the frame tap upstream stays transport-agnostic.
//!
//! Compiled only under the **`spout` cargo feature** and only on Windows; the
//! feature is off by default, so an ordinary build, CI and the macOS target
//! never reach this module, never need the SDK and never run a C++ step.

use std::ffi::{CStr, CString, c_char, c_int, c_uint};
use std::fmt;
use std::ptr::NonNull;

/// The opaque handle `shim.cpp` hands back. Never dereferenced on this side.
#[repr(C)]
struct LmvSpout {
    _private: [u8; 0],
}

unsafe extern "C" {
    fn lmv_spout_create(sender_name: *const c_char, width: c_uint, height: c_uint)
    -> *mut LmvSpout;
    fn lmv_spout_send(
        spout: *mut LmvSpout,
        rgba: *const u8,
        width: c_uint,
        height: c_uint,
    ) -> c_int;
    fn lmv_spout_name(spout: *mut LmvSpout) -> *const c_char;
    fn lmv_spout_destroy(spout: *mut LmvSpout);
}

/// Why a sender could not be created or a frame could not be published.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpoutError {
    /// The requested name held an interior NUL, so it cannot cross the C seam.
    NameNotCString,
    /// A zero width or height. Rejected here rather than passed on, because the
    /// SDK reports it the same way it reports a device failure.
    EmptySize { width: u32, height: u32 },
    /// The sender could not be created — no D3D11 device, or the shared texture
    /// or its registration failed.
    CreateFailed {
        name: String,
        width: u32,
        height: u32,
    },
    /// The pixel buffer is not `width * height * 4` bytes. Checked on this side
    /// because the C seam takes a bare pointer and would read past the end.
    PixelCount {
        got: usize,
        want: usize,
        width: u32,
        height: u32,
    },
    /// The SDK refused the frame.
    SendFailed { width: u32, height: u32 },
}

impl fmt::Display for SpoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SpoutError::NameNotCString => {
                write!(f, "the Spout sender name contains an interior NUL byte")
            }
            SpoutError::EmptySize { width, height } => {
                write!(f, "a Spout sender cannot be {width}x{height}")
            }
            SpoutError::CreateFailed {
                name,
                width,
                height,
            } => write!(
                f,
                "could not create the Spout sender '{name}' at {width}x{height} \
                 (no D3D11 device, or the shared texture could not be created)"
            ),
            SpoutError::PixelCount {
                got,
                want,
                width,
                height,
            } => write!(
                f,
                "a {width}x{height} Spout frame needs {want} bytes of RGBA, got {got}"
            ),
            SpoutError::SendFailed { width, height } => {
                write!(f, "the Spout sender refused a {width}x{height} frame")
            }
        }
    }
}

impl std::error::Error for SpoutError {}

/// A live Spout sender other applications on this machine can receive from.
///
/// Holds a raw pointer to a C++ object owning a D3D11 device and its immediate
/// context, which is what makes this type **neither `Send` nor `Sync`** — the
/// raw pointer gives that for free, and it is the property that matters: one
/// sender belongs to one thread.
pub struct SpoutSender {
    handle: NonNull<LmvSpout>,
    /// The name the sender was actually registered under, read back once at
    /// construction. Not the requested name where a stale registration forced
    /// an increment.
    name: String,
    width: u32,
    height: u32,
}

impl SpoutSender {
    /// Claim a sender named `name` for frames of `width` x `height`.
    ///
    /// The **name** is settled here — [`name`](Self::name) is answerable
    /// straight away, and may differ from `name` where a stale registration
    /// forced an increment. The sender itself becomes visible to a receiver on
    /// the first [`send`](Self::send), because the SDK's eager registration
    /// entry point is not public.
    pub fn new(name: &str, width: u32, height: u32) -> Result<Self, SpoutError> {
        if width == 0 || height == 0 {
            return Err(SpoutError::EmptySize { width, height });
        }
        let c_name = CString::new(name).map_err(|_| SpoutError::NameNotCString)?;
        // SAFETY: `c_name` is a valid NUL-terminated string that outlives the
        // call, and the dimensions are non-zero. The callee copies the name and
        // returns either null or a handle this type takes sole ownership of.
        let raw = unsafe { lmv_spout_create(c_name.as_ptr(), width, height) };
        let handle = NonNull::new(raw).ok_or_else(|| SpoutError::CreateFailed {
            name: name.to_string(),
            width,
            height,
        })?;
        // SAFETY: `handle` came from a successful create and has not been
        // destroyed. The returned pointer is the sender's own storage, valid
        // until destroy, so it is copied out before anything else can run.
        let registered = unsafe {
            let ptr = lmv_spout_name(handle.as_ptr());
            if ptr.is_null() {
                name.to_string()
            } else {
                CStr::from_ptr(ptr).to_string_lossy().into_owned()
            }
        };
        Ok(Self {
            handle,
            name: registered,
            width,
            height,
        })
    }

    /// Publish one frame: tight, row-major, top-to-bottom RGBA8, exactly as
    /// `lmv_core`'s `CaptureImage` returns it.
    ///
    /// A `width` or `height` differing from the last call is handled inside the
    /// SDK, which re-creates the shared texture under the same sender name, so
    /// there is no separate resize step.
    pub fn send(&mut self, rgba: &[u8], width: u32, height: u32) -> Result<(), SpoutError> {
        let want = width as usize * height as usize * 4;
        if rgba.len() != want {
            return Err(SpoutError::PixelCount {
                got: rgba.len(),
                want,
                width,
                height,
            });
        }
        // SAFETY: `handle` is owned and live; `rgba` is checked above to hold
        // exactly `width * height * 4` bytes, which is what the callee reads,
        // and it is borrowed for the duration of the call.
        let ok = unsafe { lmv_spout_send(self.handle.as_ptr(), rgba.as_ptr(), width, height) };
        if ok == 0 {
            return Err(SpoutError::SendFailed { width, height });
        }
        self.width = width;
        self.height = height;
        Ok(())
    }

    /// The name the sender is registered under — what a receiver lists, which
    /// is not necessarily the name that was requested.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The size of the last frame published, or the size the sender was created
    /// at before any frame.
    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }
}

impl Drop for SpoutSender {
    fn drop(&mut self) {
        // SAFETY: the handle came from a successful create, is destroyed exactly
        // once because this type is the sole owner and is not `Copy`, and is
        // never used afterwards.
        unsafe { lmv_spout_destroy(self.handle.as_ptr()) };
    }
}
