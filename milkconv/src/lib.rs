//! `milkconv` — the MilkDrop converter (Plan 0100 / ADR-0113).
//!
//! **Nothing in this crate ships.** It reads `.milk` text ahead of time and emits
//! a bundle the engine loads; what a released binary carries is the VM and the
//! bundle loader in `lmv_core::milk`, which contain no parser and no translator.
//! That is the whole of ADR-0113's decision, and it is what keeps untrusted
//! program text and a large translator out of a process that is sometimes
//! foobar2000's.
//!
//! The crate is outside the workspace's `default-members` for the same reason
//! `lmv-core-cabi` is (ADR-0072): a developer tool no shipped artifact depends on
//! should not be in the everyday build loop. `--workspace` selects it, so CI and
//! the pre-push hook still run its conformance suite and its clippy.

#![warn(missing_docs)]

pub mod eel;
