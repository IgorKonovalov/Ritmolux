//! The `rlx-core` integration tests that need no test binary of their own, linked
//! as one (ADR-0204).
//!
//! Each file beside this one is a module, and a test's name carries its module as a
//! prefix, `easing::<test>`. Select one file's tests with
//! `cargo nextest run -p rlx-core --test suite easing::`.
//!
//! **A file stays a top-level `tests/*.rs`, outside this binary, when any of these
//! holds:**
//!
//! 1. a `binary()` selector in `.config/nextest.toml` names it — the GPU suites the
//!    `fast` profile excludes, and the run-alone override's binaries;
//! 2. it reads the clock, which is to say it carries a `clippy::disallowed_methods`
//!    exemption. `hygiene::every_clock_reading_test_is_scheduled_alone` fails on one
//!    anywhere under `tests/suite/`;
//! 3. it reads a process-level quantity another test in the same process could
//!    move: memory, the environment, the working directory.
//!
//! Under nextest every test still runs in its own process. Under plain `cargo test`
//! the modules here share one, which rule 3 is what makes harmless.
//!
//! `common` is the directory module the separate binaries also use, reached here by
//! `#[path]` so there is one copy.
//!
//! `preset.rs` declares this binary's `#[global_allocator]`, a per-thread
//! allocation counter, so every module here allocates through it and no second
//! module can declare one.

#[path = "../common/mod.rs"]
mod common;

mod analytic_field;
mod attractor_trails;
mod backdrop_palette;
mod backdrop_ramp;
mod batch_independence;
mod beat;
mod bloom;
mod capture_advance;
mod cellular;
mod chain;
mod collage_layout;
mod composite;
mod console_preview;
mod downbeat_probe;
mod easing;
mod feedback;
mod frame_tap;
mod geometry_extent;
mod hygiene;
mod kaleidoscope;
mod layer;
mod line_joints;
mod r#override;
mod palette_contour;
mod palette_srgb;
mod preset;
mod preset_schema;
mod saturation;
mod seed;
mod spectrum;
mod tempo_probe;
mod tier_switch;
mod transition;
mod warp_mesh;
mod warp_mesh_wide;
