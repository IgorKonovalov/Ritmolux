//! The `shot` CLI's **pure** helpers, hosted in the library so they are testable.
//!
//! `standalone/examples/shot.rs` is an `examples/` target, and `#[test]` in an
//! example does not run under `cargo test` — which is why a hand-rolled WAV
//! parser, a JSON emitter, filmstrip index math and a bitmap glyph table sat
//! untested for five plans (Plan 0031 Phase 1). Everything here is a pure
//! function of its arguments: no GPU, no filesystem, no `Args`, no process
//! state. The example keeps argument parsing, GPU capture, and file I/O.
//!
//! **`image` stays a dev-dependency.** The PNG codec is deliberately out of the
//! shipped `lmv.exe` (ADR-0011, ADR-0033 Alt E), so nothing here names an
//! `image` type: the filmstrip's *layout* arithmetic lives in [`film`] and the
//! pixel blit stays in the example, and [`glyph`] returns bitmap rows the
//! example rasterizes.

pub mod args;
pub mod film;
pub mod glyph;
pub mod json;
pub mod wav;
