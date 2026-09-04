//! The on-disk tables, before validation.
//!
//! One file per subsystem table, named for the TOML table it deserializes, plus
//! `preset.rs` for the preset-level ones. Nothing here is a runtime shape: a
//! `Raw*` type is what `serde` produces, and each carries the `into_*` that
//! validates it into the real thing -- so an invalid document is rejected at
//! exactly one place per table.

mod feedback;
mod generator;
mod mesh;
mod milk;
mod palette;
mod particles;
mod preset;
mod smoothing;
mod spectrum;

pub(super) use feedback::*;
pub(super) use generator::*;
pub(super) use mesh::*;
pub(super) use milk::*;
pub(super) use palette::*;
pub(super) use particles::*;
pub(super) use preset::*;
pub(super) use smoothing::*;
pub(super) use spectrum::*;
