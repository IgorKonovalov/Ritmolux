//! The named values a MilkDrop program hands back, as one table (Plan 0100
//! Phase 4).
//!
//! # Why this is a macro
//!
//! There are forty-odd of them, and each needs the same four things: a field on a
//! struct the scene reads, an EEL2 name, a default the host seeds before the
//! program runs, and whether it is a per-frame *rate* that has to be converted to
//! this engine's per-second vocabulary. Written by hand that is four parallel
//! lists, and the failure mode of four parallel lists is a value that silently
//! reads its neighbour — the exact defect `expr.rs`'s slot-base assertion exists
//! to catch one axis of.
//!
//! So the table below is the single source, and the macro derives all four from
//! it. Adding an output is one line, in one place, and it cannot be half-added.
//!
//! # What is not here
//!
//! The **nine warp outputs** (`zoom`, `rot`, `cx`, `cy`, `dx`, `dy`, `sx`, `sy`,
//! `warp`) are not, and deliberately: they are also read **per vertex**, where
//! these are per frame only, and they are positionally tied to
//! [`warp_mesh::PER_VERTEX_PARAMS`](crate::render::scenes::warp_mesh::PER_VERTEX_PARAMS)
//! in a way a struct of named fields would obscure rather than clarify.

// Hot-path panic-denial pragma (Plan 0002 Phase 2; `core/src/milk` is scanned by
// the hygiene guard). Read once per frame.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

use super::vm::VmState;

/// How a per-frame output converts to this engine's per-second vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rate {
    /// Not a rate at all — a flag, a colour, a position, a count. Passes through.
    Plain,
    /// A factor composed multiplicatively every frame, so it becomes `v^fps`.
    Factor,
}

/// Generate a named-output struct plus the three things a runtime needs to use
/// it: the name roster, the index resolution, and the read-back.
macro_rules! outputs {
    (
        $struct_name:ident, $slots_name:ident, $names:ident;
        $($(#[$meta:meta])* $field:ident : $name:literal = $default:expr, $rate:ident;)*
    ) => {
        /// The values a program left in its output registers this frame, already
        /// converted out of MilkDrop's per-frame vocabulary where that applies.
        ///
        /// [`Default`] is MilkDrop's own default for each, which is what the host
        /// seeds before running a program — so an output the preset never writes
        /// comes back as the reference's resting value rather than as zero.
        #[derive(Debug, Clone, Copy, PartialEq)]
        pub struct $struct_name {
            $($(#[$meta])* pub $field: f32,)*
        }

        impl Default for $struct_name {
            fn default() -> Self {
                Self { $($field: $default,)* }
            }
        }

        /// Every field's EEL2 name, in field order. Read by the converter's
        /// roster check, so a name here and a name there cannot drift.
        pub const $names: &[&str] = &[$($name,)*];

        /// The resolved register index of each field, or `None` for one the
        /// program never mentions. Built **once at load**.
        #[derive(Debug, Default, Clone)]
        pub struct $slots_name {
            $($field: Option<u16>,)*
        }

        impl $slots_name {
            /// Resolve every field against a roster.
            pub fn resolve(index: &impl Fn(&str) -> Option<u16>) -> Self {
                Self { $($field: index($name),)* }
            }

            /// Seed every field's register with MilkDrop's default, before the
            /// program runs.
            pub fn seed(&self, state: &mut VmState) {
                let d = $struct_name::default();
                $(if let Some(i) = self.$field { state.set(i, d.$field); })*
            }

            /// Read every field back, converting the rates.
            pub fn read(&self, state: &VmState) -> $struct_name {
                let d = $struct_name::default();
                $struct_name {
                    $($field: match self.$field {
                        Some(i) => convert(state.get(i), Rate::$rate, d.$field),
                        None => d.$field,
                    },)*
                }
            }
        }
    };
}

outputs! {
    FrameOutputs, FrameSlots, FRAME_OUTPUT_NAMES;

    /// How much of the previous frame survives — **per second** here, from the
    /// reference's per-frame factor.
    decay: "decay" = 0.98, Factor;
    /// Multiplies the light on the way out.
    gamma: "gamma" = 1.0, Plain;
    /// Past `0.5`, the past is toroidal.
    wrap: "wrap" = 1.0, Plain;
    /// Darkens a soft disc at the middle of the frame.
    darken_center: "darken_center" = 0.0, Plain;
    /// Past `0.5`, `sqrt` of the light.
    brighten: "brighten" = 0.0, Plain;
    /// Past `0.5`, the light squared.
    darken: "darken" = 0.0, Plain;
    /// Past `0.5`, `c * (1 - c) * 4`.
    solarize: "solarize" = 0.0, Plain;
    /// Past `0.5`, `1 - c`.
    invert: "invert" = 0.0, Plain;

    // --- the waveform ---
    /// Which of the eight `wave_mode` figures the waveform draws.
    wave_mode: "wave_mode" = 0.0, Plain;
    /// The waveform's centre, in uv with `y = 0` at the top.
    wave_x: "wave_x" = 0.5, Plain;
    /// See [`wave_x`](Self::wave_x).
    wave_y: "wave_y" = 0.5, Plain;
    /// The waveform's colour.
    wave_r: "wave_r" = 1.0, Plain;
    /// See [`wave_r`](Self::wave_r).
    wave_g: "wave_g" = 1.0, Plain;
    /// See [`wave_r`](Self::wave_r).
    wave_b: "wave_b" = 1.0, Plain;
    /// The waveform's alpha. Under this engine's additive draw it reads as
    /// intensity rather than as coverage — see the scene's draw layer.
    wave_a: "wave_a" = 1.0, Plain;
    /// How far the trace swings. MilkDrop's `fWaveScale`.
    wave_scale: "wave_scale" = 1.0, Plain;
    /// How much the trace is smoothed along its length, `0..1`.
    wave_smoothing: "wave_smoothing" = 0.75, Plain;
    /// The mode-dependent shape parameter, MilkDrop's `fWaveParam`. Means a
    /// different thing in every `wave_mode`, which is the reference's own design.
    wave_mystery: "wave_mystery" = 0.0, Plain;
    /// Past `0.5`, the trace is drawn as dots rather than a line.
    wave_usedots: "wave_usedots" = 0.0, Plain;
    /// Past `0.5`, the trace is drawn thick.
    wave_thick: "wave_thick" = 0.0, Plain;
    /// Past `0.5`, the trace adds rather than blends. Always true here — the
    /// draw seam is additive by construction (ADR-0056) — and read only so the
    /// converter does not have to warn about it.
    wave_additive: "wave_additive" = 0.0, Plain;
    /// Past `0.5`, the trace's colour is normalized to its brightest channel.
    wave_brighten: "wave_brighten" = 1.0, Plain;

    // --- the two borders ---
    /// The outer border's thickness, as a fraction of the frame.
    ob_size: "ob_size" = 0.01, Plain;
    /// The outer border's colour.
    ob_r: "ob_r" = 0.0, Plain;
    /// See [`ob_r`](Self::ob_r).
    ob_g: "ob_g" = 0.0, Plain;
    /// See [`ob_r`](Self::ob_r).
    ob_b: "ob_b" = 0.0, Plain;
    /// The outer border's alpha, read as intensity.
    ob_a: "ob_a" = 0.0, Plain;
    /// The inner border's thickness.
    ib_size: "ib_size" = 0.01, Plain;
    /// The inner border's colour.
    ib_r: "ib_r" = 0.25, Plain;
    /// See [`ib_r`](Self::ib_r).
    ib_g: "ib_g" = 0.25, Plain;
    /// See [`ib_r`](Self::ib_r).
    ib_b: "ib_b" = 0.25, Plain;
    /// The inner border's alpha, read as intensity.
    ib_a: "ib_a" = 0.0, Plain;

    // --- the motion-vector grid ---
    /// How many motion vectors across.
    mv_x: "mv_x" = 12.0, Plain;
    /// How many motion vectors down.
    mv_y: "mv_y" = 9.0, Plain;
    /// The grid's offset within a cell, `0..1`.
    mv_dx: "mv_dx" = 0.0, Plain;
    /// See [`mv_dx`](Self::mv_dx).
    mv_dy: "mv_dy" = 0.0, Plain;
    /// Each vector's length, as a multiple of the warp it samples.
    mv_l: "mv_l" = 0.9, Plain;
    /// The grid's colour.
    mv_r: "mv_r" = 1.0, Plain;
    /// See [`mv_r`](Self::mv_r).
    mv_g: "mv_g" = 1.0, Plain;
    /// See [`mv_r`](Self::mv_r).
    mv_b: "mv_b" = 1.0, Plain;
    /// The grid's alpha, read as intensity. `0` — the default — draws nothing.
    mv_a: "mv_a" = 0.0, Plain;
}

outputs! {
    WavePoint, WavePointSlots, WAVE_POINT_NAMES;

    /// The point's position, in uv with `y = 0` at the top.
    x: "x" = 0.5, Plain;
    /// See [`x`](Self::x).
    y: "y" = 0.5, Plain;
    /// The point's colour, which a custom wave may vary along its length.
    r: "r" = 1.0, Plain;
    /// See [`r`](Self::r).
    g: "g" = 1.0, Plain;
    /// See [`r`](Self::r).
    b: "b" = 1.0, Plain;
    /// The point's alpha, read as intensity.
    a: "a" = 1.0, Plain;
}

outputs! {
    ShapeInstance, ShapeInstanceSlots, SHAPE_INSTANCE_NAMES;

    /// The shape's centre, in uv with `y = 0` at the top.
    x: "x" = 0.5, Plain;
    /// See [`x`](Self::x).
    y: "y" = 0.5, Plain;
    /// The shape's radius, in frame-heights.
    rad: "rad" = 0.1, Plain;
    /// The shape's rotation, in radians.
    ang: "ang" = 0.0, Plain;
    /// The centre colour.
    r: "r" = 1.0, Plain;
    /// See [`r`](Self::r).
    g: "g" = 0.0, Plain;
    /// See [`r`](Self::r).
    b: "b" = 0.0, Plain;
    /// The centre alpha, read as intensity.
    a: "a" = 1.0, Plain;
    /// The edge colour, which the fill ramps toward.
    r2: "r2" = 0.0, Plain;
    /// See [`r2`](Self::r2).
    g2: "g2" = 1.0, Plain;
    /// See [`r2`](Self::r2).
    b2: "b2" = 0.0, Plain;
    /// The edge alpha.
    a2: "a2" = 0.0, Plain;
    /// The outline's colour.
    border_r: "border_r" = 1.0, Plain;
    /// See [`border_r`](Self::border_r).
    border_g: "border_g" = 1.0, Plain;
    /// See [`border_r`](Self::border_r).
    border_b: "border_b" = 1.0, Plain;
    /// The outline's alpha. `0` draws no outline.
    border_a: "border_a" = 0.1, Plain;
    /// Past `0.5`, the outline is drawn thick.
    thick_outline: "thickoutline" = 0.0, Plain;
    /// How many sides this instance has, overriding the structural count.
    sides: "sides" = 4.0, Plain;
}

/// One raw output as this engine's vocabulary.
///
/// A `Factor` is raised to [`NOMINAL_FPS`](super::NOMINAL_FPS) and saturated
/// rather than allowed to overflow to the identity — see
/// [`per_second_factor`](super::per_second_factor). Everything else passes
/// through, with a non-finite value falling back to the reference's default so a
/// single `NaN` cannot blank a frame.
fn convert(value: f32, rate: Rate, default: f32) -> f32 {
    match rate {
        Rate::Factor => super::per_second_factor(value),
        Rate::Plain => {
            if value.is_finite() {
                value
            } else {
                default
            }
        }
    }
}
