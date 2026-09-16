//! The `standalone` integration tests that need no test binary of their own,
//! linked as one (ADR-0204).
//!
//! Each file beside this one is a module, and a test's name carries its module.
//! Select one file's tests with `cargo nextest run -p standalone --test suite shot_cli::`.
//!
//! The rule for what stays a top-level `tests/*.rs` is `core/tests/suite/main.rs`'s:
//! a `binary()` selector in `.config/nextest.toml` names it, it reads the clock, or
//! it reads a process-level quantity such as its own memory.
//!
//! `common` is the directory module the separate binaries also use, reached here by
//! `#[path]` so there is one copy.

#[path = "../common/mod.rs"]
mod common;

mod configuration_doc;
mod preset_check;
mod shot_cli;
mod show_is_the_only_owner;
mod stream_split;
