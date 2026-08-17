//! The HLSL→WGSL translator (Plan 0100 Phase 6).
//!
//! # The language this covers is MilkDrop's, not HLSL's
//!
//! Censused over all 430 854 shader source lines in the corpus (2026-08-16): ~30
//! intrinsics cover essentially everything, no preset declares a struct, none
//! uses a derivative (`ddx`/`ddy`), none touches a bitwise operator, and only
//! 9 % contain any loop. What remains is expression code over `float2`/`float3`/
//! `float4`/`float2x2`-`float4x4`, swizzles, `if`, bounded loops, parameterless
//! `#define`s and plain helper functions — a bounded language this hand-written
//! frontend covers whole. Anything outside it **rejects that preset by name**
//! (the phase's rule), with a class Phase 5's ranking can count.
//!
//! # The three deliberate translation choices
//!
//! 1. **Every scalar is `f32`.** HLSL promotes `int` to `float` implicitly and
//!    the corpus leans on it; WGSL does neither. Declaring every scalar —
//!    including loop counters — as `f32` makes HLSL's promotion the ambient
//!    rule again, and no shader here counts past a few thousand, well inside
//!    exact-float range. (No preset uses a bitwise operator, so nothing needs
//!    integers back.)
//! 2. **Matrix constructors go through `transpose`.** HLSL `floatNxN(...)`
//!    fills rows; WGSL `matNxN(...)` fills columns. Emitting
//!    `transpose(matNxN(rows...))` keeps the *mathematical* matrix identical, so
//!    `mul(a, b)` translates positionally to `(a) * (b)` — WGSL's `v * m` is
//!    the row-vector product and `m * v` the column one, exactly HLSL's pair.
//! 3. **`tex2D` becomes `textureSampleLevel(..., 0.0)`,** never `textureSample`:
//!    presets sample inside `if`s and loops freely, and naga's uniformity
//!    analysis (correctly) refuses implicit-derivative sampling there. No
//!    MilkDrop texture has mips, so level 0 *is* the texture.
//!
//! # Every loop is bounded
//!
//! Each `for`/`while` gets its own counter capped at [`LOOP_CAP`] iterations,
//! nesting is capped at [`MAX_LOOP_DEPTH`], and the emitted module's static op
//! count is capped at [`MAX_OPS`] — the one lever this project holds against a
//! converted shader tripping a driver watchdog (ADR-0113's stated residual
//! risk). The counts are recorded on [`Translated`] so the bundle can carry
//! them.

use std::fmt;

mod emit;
mod lex;
mod parse;

/// Which of MilkDrop 2's two pixel shaders is being translated. They differ in
/// what `uv` means (warped vs. plain), which bind group the surface sits at,
/// and what the epilogue returns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// The warp shader: runs over the mesh, `uv` is the warped source uv, and
    /// its output *is* the new field — so it is clamped to `[0, 1]` exactly as
    /// the reference's 8-bit target clamped it, or a `decay >= 1` preset would
    /// integrate without bound in a float field.
    Warp,
    /// The composite shader: fullscreen, `uv == uv_orig`, output goes to the
    /// screen over the backdrop.
    Comp,
}

impl Stage {
    /// The section name, for error messages and the report's ranking.
    pub fn name(self) -> &'static str {
        match self {
            Stage::Warp => "warp",
            Stage::Comp => "comp",
        }
    }
}

/// The most iterations one translated loop may run. The corpus's loops are
/// tens of iterations; this is the watchdog bound, not a tuning value.
pub const LOOP_CAP: u32 = 1024;
/// The deepest loop nesting accepted — past two, the worst case multiplies
/// beyond what any driver watchdog survives.
pub const MAX_LOOP_DEPTH: u32 = 2;
/// The most static operations one translated shader may hold.
pub const MAX_OPS: u32 = 16384;

/// A translated shader: the complete WGSL fragment module and the numbers the
/// bound-every-loop rule says to record.
#[derive(Debug, Clone)]
pub struct Translated {
    /// The whole module — prelude, the preset's helper functions, `fs_main`.
    pub wgsl: String,
    /// The deepest blur level referenced (`GetBlurN` or `sampler_blurN`), `0..=3`.
    pub blur_level: u8,
    /// Static operation count of the emitted code.
    pub ops: u32,
    /// How many loops were emitted, each bounded at [`LOOP_CAP`].
    pub loops: u32,
}

/// Why a shader did not translate. `class` is the short machine-readable bucket
/// Phase 5's ranking counts; `message` names the construct.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShaderError {
    /// One of `disk-texture`, `unknown-name`, `unsupported`, `parse`, `too-big`.
    pub class: &'static str,
    /// What, exactly, in the preset's own vocabulary.
    pub message: String,
}

impl ShaderError {
    pub(crate) fn new(class: &'static str, message: impl Into<String>) -> Self {
        Self {
            class,
            message: message.into(),
        }
    }
}

impl fmt::Display for ShaderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.class, self.message)
    }
}

impl std::error::Error for ShaderError {}

/// Translate one MilkDrop 2 shader block to a complete WGSL fragment module.
///
/// `tex_wrap` is the preset's `bTexWrap`, which decides what `sampler_main`
/// aliases to — the wrap or the clamp sampler — exactly as the reference binds
/// it. (A preset animating `wrap` per frame keeps its initial choice; none in
/// the corpus does.)
pub fn translate(stage: Stage, source: &str, tex_wrap: bool) -> Result<Translated, ShaderError> {
    let tokens = lex::lex(source)?;
    let unit = parse::parse(&tokens)?;
    emit::emit(stage, &unit, tex_wrap)
}
