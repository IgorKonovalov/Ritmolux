//! The `shot` CLI's helpers, hosted in the library so they are testable.
//!
//! `standalone/examples/shot.rs` is an `examples/` target, and `#[test]` in an
//! example does not run under `cargo test` — which is why a hand-rolled WAV
//! parser, a JSON emitter, filmstrip index math and a bitmap glyph table sat
//! untested for five plans (Plan 0031 Phase 1). [`args`], [`film`], [`glyph`],
//! [`json`] and [`wav`] are pure functions of their arguments: no GPU, no
//! filesystem, no `Args`, no process state.
//!
//! [`report`], [`horizon`] and [`render`] are the exceptions and all three are
//! deliberate (Plan 0061 Phase 4, Plan 0085 Phase 1, Plan 0101 Phase 1). The
//! `--report`, `--horizon` and `--render`
//! machinery *does* drive a renderer, so none is pure — but leaving `report`
//! in the example meant a thousand lines of table generation, gate reachability
//! and transient analysis whose only coverage was a subprocess asserting that
//! the JSON's braces balanced. Their pure halves are directly testable here;
//! the GPU half of each is one function it calls.
//!
//! **`image` stays a dev-dependency.** The PNG codec is deliberately out of the
//! shipped `ritmolux.exe` (ADR-0011, ADR-0033 Alt E), so nothing here names an
//! `image` type: the filmstrip's *layout* arithmetic lives in [`film`] and the
//! pixel blit stays in the example, and [`glyph`] returns bitmap rows the
//! example rasterizes.

pub mod args;
pub mod film;
pub mod glyph;
pub mod horizon;
pub mod json;
pub mod render;
pub mod report;
pub mod wav;

use rlx_core::preset::Preset;
use rlx_core::render::{HeadlessOptions, Renderer, Tier};

/// A headless renderer over `presets`, using the real GPU at full quality (the
/// CLI wants speed and true output, not the tests' software reproducibility).
///
/// Shared rather than duplicated: the example's capture modes and [`report`]
/// both need it, and they need it configured identically or a report would
/// describe a different renderer than a capture of the same preset.
pub fn renderer(
    width: u32,
    height: u32,
    presets: Vec<Preset>,
    tier: Tier,
) -> Result<Renderer, String> {
    let mut r = Renderer::new_headless_tiered(
        HeadlessOptions {
            width,
            height,
            prefer_software: false,
        },
        tier,
    )
    .map_err(|e| format!("headless renderer: {e}"))?;
    r.set_presets(presets);
    Ok(r)
}
