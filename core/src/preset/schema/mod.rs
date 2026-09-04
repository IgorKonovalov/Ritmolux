//! TOML preset schema: which built-in system a preset drives and the
//! expression bound to each of its named parameters.
//!
//! Parsing happens once at load: the raw TOML is deserialized, each parameter
//! expression is compiled (a malformed one is rejected with a surfaced error),
//! and the result is an in-memory [`Preset`] whose bindings are ready to
//! evaluate. A bad preset returns `Err` — it never panics, so the caller can
//! degrade to the last good preset (ADR-0002 / NFR 10).

use std::collections::BTreeMap;
use std::fmt;

use serde::Deserialize;

use super::expr::{self, Expr, ExprError};
use crate::render::feedback::{Deposit, FeedbackConfig, Warp};
use crate::render::palette::{NamedPalette, PaletteConfig};
use crate::render::scenes::lines::star::{DEFAULT_RING_SCALE, MAX_RING_COUNT, Motif, RingSpec};
use crate::render::scenes::lines::{
    CurveFamily, GeneratorConfig, MAX_LSYSTEM_DEPTH, SpectrumLayout, hankin,
};
use crate::render::scenes::particles::AttractorFamily;
use crate::render::scenes::particles::ifs::IfsFigure;

// The five concerns this file holds apart. `system` is the roster of built-in
// systems, `easing` the attack/release pair, `raw` the on-disk tables, `load`
// the TOML-to-`Preset` path, `error` the failure enum. What stays here is the
// compiled shape a preset becomes.
mod easing;
mod error;
mod load;
mod raw;
mod system;

pub use easing::Easing;
pub use error::PresetError;
pub use system::{GLOBAL_PARAMS, SystemKind, is_known_param};

use raw::*;

/// Where a preset's second scene joins the composite (ADR-0090): before the
/// post chain, sharing every stage with the main scene, or between the
/// kaleidoscope and bloom in its own offscreen (Phase 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LayerJoin {
    /// The layer draws into the same scene target as the main scene, before the
    /// chain — one substance, shared trails/fold/bloom. The default.
    #[default]
    Under,
    /// The layer renders into its own offscreen and blends into the chain
    /// between the kaleidoscope and bloom — crisp geometry, shared glow.
    Over,
}

impl LayerJoin {
    /// Parse the canonical `join = "..."` value, or `None` if unknown.
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "under" => LayerJoin::Under,
            "over" => LayerJoin::Over,
            _ => return None,
        })
    }
}

/// How an `over` layer blends into the chain (ADR-0090): fixed at load, like
/// every structural key, and applied in linear light within the layer's
/// premultiplied-alpha footprint. Parsed now; consumed by the blend pass
/// (Plan 0076 Phase 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LayerBlend {
    /// Linear-light addition — the engine's native compositing idiom.
    Add,
    /// `1 - (1-a)(1-b)`: bounded brightening. The default — ADR-0090's
    /// illustrative mode, and the one that cannot blow out.
    #[default]
    Screen,
    /// Darkens where the layer has coverage.
    Multiply,
    /// Multiply below mid-grey, screen above.
    Overlay,
}

impl LayerBlend {
    /// Every mode, for the load error's "expected one of" listing.
    pub const ALL: [LayerBlend; 4] = [
        LayerBlend::Add,
        LayerBlend::Screen,
        LayerBlend::Multiply,
        LayerBlend::Overlay,
    ];

    /// Parse the canonical `blend = "..."` value, or `None` if unknown.
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "add" => LayerBlend::Add,
            "screen" => LayerBlend::Screen,
            "multiply" => LayerBlend::Multiply,
            "overlay" => LayerBlend::Overlay,
            _ => return None,
        })
    }

    /// The canonical name — [`from_name`](Self::from_name)'s inverse.
    pub fn as_str(self) -> &'static str {
        match self {
            LayerBlend::Add => "add",
            LayerBlend::Screen => "screen",
            LayerBlend::Multiply => "multiply",
            LayerBlend::Overlay => "overlay",
        }
    }
}

/// The optional second scene layer (ADR-0090 / Plan 0076): a full authoring
/// surface — its own system, params, bindings, `[layer.smoothing]` and
/// structural tables — joined to the composite at [`LayerJoin`]. The preset's
/// single `[palette]` serves both layers (one colour language, one baked LUT),
/// and layer params are **namespaced to the layer**: they reach the layer's
/// scene only, never the main scene's first-owner-wins routing and never the
/// compositing stages, which belong to the preset as a whole.
#[derive(Debug)]
pub struct Layer {
    /// The built-in system this layer drives.
    pub system: SystemKind,
    /// Where the layer joins the composite.
    pub join: LayerJoin,
    /// How an `over` layer blends in (ignored, with a load warning, on an
    /// `under` join — there is no junction for it to apply at).
    pub blend: LayerBlend,
    /// The bindable mix amount at the `over` join (ADR-0090): how much of the
    /// layer the blend applies, evaluated per frame like any binding so audio
    /// can surge the second layer. `None` — the default — is full strength.
    pub mix: Option<Binding>,
    /// The layer's parameter bindings, name-sorted like the preset's own.
    pub params: Vec<Binding>,
    /// The layer's `[layer.per_vertex]` bindings — see
    /// [`Preset::per_vertex`](Preset::per_vertex).
    pub per_vertex: Vec<Binding>,
    /// The layer's declarative structural config (ADR-0007), from its own
    /// `[layer.curve]` / `[layer.generator]` / `[layer.particles]` /
    /// `[layer.spectrum]` tables — validated by the same per-system rules as
    /// the top level.
    pub config: Option<GeneratorConfig>,
}

/// A named parameter bound to a compiled expression.
#[derive(Debug)]
pub struct Binding {
    /// The system parameter this drives (e.g. `warp`, `hue`).
    pub name: String,
    /// The compiled expression producing its per-frame value.
    pub expr: Expr,
    /// This binding's easing constants (ADR-0019 / ADR-0035), read out of the
    /// preset's `[smoothing]` table **once, here at load**.
    /// [`Easing::INSTANT`] — the default for an unlisted param — means no
    /// smoothing. Resolved at parse time rather than looked up per binding per
    /// frame (Plan 0031 Phase 3); it is a fact about the preset, and the preset
    /// does not change while it renders.
    pub tau: Easing,
}

/// One `[latch]` entry, compiled (ADR-0137): a gate armed on one condition and
/// fired by the first rising edge of another inside the arming window.
///
/// Its **slot is its position in [`Preset::latches`]**, and that is the only
/// place the mapping exists: the loader resolved the author's name onto a
/// reserved variable slot while compiling the bindings, so nothing per-frame
/// looks a latch up by name. The same reasoning that keeps `[smoothing]` off
/// [`Preset`] as a table — a fact about the preset, resolved once, and the
/// preset does not change while it renders.
///
/// The two expressions are compiled **without** any latch name in scope, so a
/// latch cannot read a latch. That is not a restriction waiting to be lifted: it
/// is what makes "evaluate every latch, then the params that read them" a
/// complete order rather than one with a dependency graph inside it.
#[derive(Debug)]
pub struct Latch {
    /// The author's name for it, which its bindings reference.
    pub name: String,
    /// While this holds (`> 0.5`), the latch is armed. Its fall re-arms.
    pub arm: Expr,
    /// The rising edge that fires an armed latch.
    pub fire: Expr,
    /// How long the fired latch reads `1.0`, in seconds. `0` is one frame.
    pub hold: f32,
}

/// A loaded, ready-to-evaluate preset.
#[derive(Debug)]
pub struct Preset {
    /// Human-readable name (defaults to the system name if omitted).
    pub name: String,
    /// Which built-in system this preset drives.
    pub system: SystemKind,
    /// Parameter bindings, sorted by name for deterministic iteration.
    pub params: Vec<Binding>,
    /// The `[per_vertex]` table's bindings (Plan 0100 Phase 1): the warp mesh's
    /// per-vertex program, evaluated once **per mesh vertex** per frame with
    /// `x`/`y`/`rad`/`ang` bound to that vertex's position.
    ///
    /// A separate table rather than a naming convention inside `[params]`,
    /// because the cost is categorically different: one of these is `N`
    /// evaluations where an ordinary binding is one, and an author has to be able
    /// to see which of their bindings they are paying `N` for. Empty for every
    /// system but the warp mesh, and for a warp-mesh preset that accepts the
    /// identity transform.
    ///
    /// Never eased: like a per-element binding, a per-vertex one has no single
    /// value for the smoother to hold. A `[smoothing]` entry naming one is a
    /// load warning.
    pub per_vertex: Vec<Binding>,
    /// The `[latch]` table's entries (ADR-0137), in slot order — the one part of
    /// the preset surface whose value depends on frame history.
    ///
    /// Empty for a preset declaring no table, which is the overwhelmingly common
    /// case and costs exactly what it cost before latches existed: the render
    /// layer's bank advances nothing and every reserved slot stays at its rest
    /// value of `0.0`.
    pub latches: Vec<Latch>,
    /// Declarative structural config for a line scene (ADR-0007), applied once
    /// at preset load via `Scene::configure`. `None` for the fragment/swarm
    /// systems and for curve presets that accept the family default.
    pub config: Option<GeneratorConfig>,
    // The `[smoothing]` table itself is deliberately **not** kept: it is validated
    // at load and folded into each binding's `tau` there (Plan 0031 Phase 3), so
    // there is nothing left for a frame to look up. An entry naming a param this
    // preset does not bind was inert before and is inert now.
    /// Optional color palette selection (ADR-0021 / Plan 0020), from a `[palette]`
    /// table — a built-in `name` or custom `stops`, validated and baked-ready at
    /// this boundary. `None` means the default `spectrum` (the exact current
    /// cosine), so a preset without `[palette]` is visually unchanged. The
    /// renderer bakes it into a LUT and hands it to the active scene via
    /// `Scene::set_palette` on each preset switch.
    pub palette: Option<PaletteConfig>,
    /// The `[feedback]` structural table (ADR-0048): which curated warp the
    /// accumulation buffers resample their past through, and how this frame's
    /// light is deposited onto it.
    ///
    /// Not an `Option`: the absent table and the all-defaults table mean the same
    /// thing, and a plain value is what lets the renderer hand it over on **every**
    /// preset switch — so the outgoing preset's warp can never survive into the
    /// incoming one. Load-time by `[curve] family`'s reasoning; the strength that
    /// rides on it is the bindable `fb_warp`.
    pub feedback: FeedbackConfig,
    /// Optional **second** palette (ADR-0021 / Plan 0020 Phase 4), from a
    /// `[palette_b]` table. When present, the renderer bakes an A/B pair and a
    /// bindable `palette_mix` param crossfades between them per frame. `None`
    /// means no crossfade (palette A only).
    pub palette_b: Option<PaletteConfig>,
    /// The salt this preset's `hash()`/`noise()` calls mix into their argument
    /// **in the live app** (ADR-0051): folded at load from the `[generator] seed`
    /// key (Plan 0010 reserved it, Plan 0047 gave it meaning), or drawn once from
    /// OS entropy where the preset declares `seed = "random"`. `0` when it
    /// declares nothing — a perfectly good salt, and the one the whole shipped
    /// library used before any preset asked for another.
    ///
    /// A load-time constant. Nothing per-frame recomputes it, and no expression
    /// can read it except through the two functions it salts.
    pub salt: u32,
    /// The salt every **capture** path uses in place of [`salt`](Self::salt):
    /// the declared number, or `0` for `seed = "random"`.
    ///
    /// Equal to `salt` unless the preset opted into per-run variety — the whole
    /// point of the pair (ADR-0051, following ADR-0045's tier pinning). The live
    /// app varies and the harness pins, so `shot`, the goldens, `--report` and
    /// the behavioral gates stay pure functions of their inputs while a preset
    /// can still be different every time the user starts the app.
    ///
    /// It is the *renderer* that chooses between the two, not the loader, and
    /// deliberately: `default_presets()` feeds both the live C-ABI path and the
    /// capture gates, so a decision taken at load would be wrong for one of them.
    pub pinned_salt: u32,
    /// Parameters whose `clamp()` bounds are **meant** to pin, from an
    /// `[occupancy] exempt = [...]` table (ADR-0062). Sorted and deduplicated at
    /// load.
    ///
    /// A safety rail exists to bind at peak, and the saturation gate would
    /// otherwise convict it of the defect it was written to prevent. The
    /// exemption silences `core/tests/saturation.rs`, and **only** that: the
    /// binding still appears in `--report`'s `occ` count and `SAT` lines,
    /// because an exemption is a place to hide and the one mitigation available
    /// is that it stays visible.
    ///
    /// A preset-level table naming params rather than a per-expression
    /// annotation, deliberately: the grammar stays a pure expression language
    /// (ADR-0020), and this is metadata *about* a binding rather than part of
    /// it. Harness-only — nothing per-frame reads it.
    pub occupancy_exempt: Vec<String>,
    /// Whether this preset is one of its family's **representatives** — the
    /// sample the `dev` lane's per-phase test tier renders (ADR-0157).
    ///
    /// Absent means `false`. Harness-only, like `occupancy_exempt`: nothing
    /// per-frame reads it, and it changes nothing about how the preset looks or
    /// what the close and CI render, which is the whole library either way. It
    /// is **declared, not derived** — a first-N or hash-rotation rule would
    /// either never sample a newly landed preset or make the same tree gate
    /// differently on different commits.
    ///
    /// A floor is enforced in `core/tests/preset.rs`: every family carries at
    /// least two. That catches a sample decayed to nothing; it cannot catch two
    /// representatives that have stopped representing a family that grew around
    /// them, which is a curation duty with no gate behind it.
    pub representative: bool,
    /// The optional second scene layer (ADR-0090 / Plan 0076), from a `[layer]`
    /// table. `None` — the overwhelmingly common case — takes exactly the code
    /// path a preset took before layers existed: no new pass, no new target.
    pub layer: Option<Layer>,
    /// Non-fatal problems found while loading — today, bindings naming a
    /// parameter this system does not consume (ADR-0020). The preset loaded and
    /// its good bindings apply; these are surfaced so a typo stops failing
    /// silently. Empty for a clean preset. Load-time only — never read per
    /// frame.
    pub warnings: Vec<String>,
}

#[cfg(test)]
mod tests;
