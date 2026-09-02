//! rlx-core — the shared, source-agnostic brain of ritmolux.
//!
//! Takes PCM frames in (it never knows whether they came from loopback capture
//! or foobar2000), runs DSP (spectrum, onset/beat), and renders scenes via wgpu.
//! See ADR-0001 for the architecture and the layering rules in CLAUDE.md:
//! no audio-source or platform types in this crate, ever.

// core is the shared public-API crate; its surface stays documented. Binding
// under CI's `-D warnings` by design (Plan 0002 Phase 0).
#![warn(missing_docs)]

/// The `wgpu` this crate was built against, re-exported.
///
/// A caller that hands [`render::Renderer::new_from_surface_target`] a surface
/// target has to build it from **this** `wgpu`, not from its own copy: two
/// versions in one tree are two distinct types, and the mismatch is a confusing
/// error at the seam rather than at the dependency. `core-cabi` builds the
/// Win32 handle through this, which is what keeps the platform out of `core`
/// (ADR-0001, ADR-0072) without giving the ABI crate a second `wgpu`.
pub use wgpu;

pub mod audio;
pub mod diag;
pub mod dsp;
pub mod milk;
pub mod preset;
pub mod render;
pub mod signal;
