//! The preset-level raw tables: the document itself, its `[layer]` sub-preset,
//! one `[latch]` entry, and the `[generator] seed` scalar.
//!
//! Everything here describes the *preset* rather than one subsystem, which is
//! why it sits apart from the eight subsystem tables beside it.
// A continuation of one module split across several files: `super` is the other
// raw tables, `super::super` the compiled shapes and the loader they validate
// into.
use super::super::*;
use super::*;

/// The on-disk shape, before expressions are compiled.
#[derive(Deserialize)]
pub(in crate::preset::schema) struct RawPreset {
    pub(in crate::preset::schema) system: String,
    #[serde(default)]
    pub(in crate::preset::schema) name: Option<String>,
    /// The `representative` flag (ADR-0157). Absent means `false`; a non-boolean
    /// value is a `toml` type error, which is the same load-time rejection every
    /// other mistyped scalar in this struct gets.
    #[serde(default)]
    pub(in crate::preset::schema) representative: bool,
    #[serde(default)]
    pub(in crate::preset::schema) params: BTreeMap<String, String>,
    /// The optional `[curve]` structural-config table (ADR-0007), present on
    /// parametric-curve presets.
    #[serde(default)]
    pub(in crate::preset::schema) curve: Option<RawCurve>,
    /// The optional `[generator]` structural-config table (ADR-0007), present on
    /// generator presets (L-system, star pattern).
    #[serde(default)]
    pub(in crate::preset::schema) generator: Option<RawGenerator>,
    /// The optional `[particles]` structural-config table (Plan 0016), selecting
    /// the attractor family for the compute-particle scene.
    #[serde(default)]
    pub(in crate::preset::schema) particles: Option<RawParticles>,
    /// The optional `[spectrum]` structural-config table (Plan 0034): the element
    /// count, layout and per-element easing of the spectrum readout.
    #[serde(default)]
    pub(in crate::preset::schema) spectrum: Option<RawSpectrum>,
    /// The optional `[mesh]` structural-config table (Plan 0100): the warp
    /// mesh's grid, in cells.
    #[serde(default)]
    pub(in crate::preset::schema) mesh: Option<RawMesh>,
    /// The optional `[per_vertex]` table (Plan 0100): bindings evaluated once
    /// per mesh vertex, with `x`/`y`/`rad`/`ang` in scope.
    #[serde(default)]
    pub(in crate::preset::schema) per_vertex: BTreeMap<String, String>,
    /// The optional `[milk]` table (Plan 0100 Phase 2): a converted MilkDrop
    /// preset's three compiled EEL2 programs, as assembly text.
    #[serde(default)]
    pub(in crate::preset::schema) milk: Option<RawMilk>,
    /// The optional `[feedback]` structural-config table (ADR-0048): the warp
    /// kind and deposit blend both accumulation buffers read their past through.
    #[serde(default)]
    pub(in crate::preset::schema) feedback: Option<RawFeedback>,
    /// The optional `[smoothing]` table (ADR-0019, ADR-0035): per-parameter
    /// easing time constants in seconds, each a scalar or an
    /// `{ attack, release }` pair. Absent means every param is applied instantly.
    #[serde(default)]
    pub(in crate::preset::schema) smoothing: BTreeMap<String, RawSmoothing>,
    /// The optional `[latch]` table (ADR-0137): named armed-and-fired events,
    /// each an `arm`/`fire` pair plus a `hold` in seconds. Absent means the
    /// preset holds no state between frames in its expression layer.
    #[serde(default)]
    pub(in crate::preset::schema) latch: BTreeMap<String, RawLatch>,
    /// The optional `[palette]` color table (ADR-0021): a built-in `name` or
    /// custom `stops`. Absent means the default `spectrum` cosine.
    #[serde(default)]
    pub(in crate::preset::schema) palette: Option<RawPalette>,
    /// The optional `[palette_b]` table (ADR-0021 / Phase 4): the crossfade
    /// target for a bindable `palette_mix`. Same shape as `[palette]`.
    #[serde(default)]
    pub(in crate::preset::schema) palette_b: Option<RawPalette>,
    /// The optional `[occupancy]` table (ADR-0062): params whose `clamp()`
    /// bounds are meant to pin, exempted from the saturation gate. Absent means
    /// every clamp in this preset is held to it.
    #[serde(default)]
    pub(in crate::preset::schema) occupancy: Option<RawOccupancy>,
    /// The optional `[layer]` table (ADR-0090 / Plan 0076): the second scene
    /// layer — its system, join point, blend, bindable mix, params, smoothing
    /// and structural tables.
    #[serde(default)]
    pub(in crate::preset::schema) layer: Option<RawLayer>,
}

/// The `[layer]` table, before validation (ADR-0090 / Plan 0076). Everything a
/// top-level preset has except a palette (the preset's single `[palette]`
/// serves both layers) and the compositing tables (`[feedback]`, `[occupancy]`,
/// `[smoothing]` for global params), which belong to the preset as a whole.
#[derive(Deserialize)]
pub(in crate::preset::schema) struct RawLayer {
    /// The layer's built-in system.
    pub(in crate::preset::schema) system: String,
    /// `under` | `over`; absent means `under`.
    #[serde(default)]
    pub(in crate::preset::schema) join: Option<String>,
    /// `add` | `screen` | `multiply` | `overlay`; absent means `screen`.
    /// `over`-join only — warned as ignored on `under`.
    #[serde(default)]
    pub(in crate::preset::schema) blend: Option<String>,
    /// The bindable mix expression at the `over` join; absent means full
    /// strength.
    #[serde(default)]
    pub(in crate::preset::schema) mix: Option<String>,
    /// The layer's parameter bindings — `[layer.params]`.
    #[serde(default)]
    pub(in crate::preset::schema) params: BTreeMap<String, String>,
    /// Per-parameter easing for the layer's bindings — `[layer.smoothing]`,
    /// the same vocabulary as the top-level table (ADR-0019 / ADR-0035).
    #[serde(default)]
    pub(in crate::preset::schema) smoothing: BTreeMap<String, RawSmoothing>,
    /// The layer's structural tables, per system (ADR-0007) — the same shapes
    /// as the top level's.
    #[serde(default)]
    pub(in crate::preset::schema) curve: Option<RawCurve>,
    #[serde(default)]
    pub(in crate::preset::schema) generator: Option<RawGenerator>,
    #[serde(default)]
    pub(in crate::preset::schema) particles: Option<RawParticles>,
    #[serde(default)]
    pub(in crate::preset::schema) spectrum: Option<RawSpectrum>,
    #[serde(default)]
    pub(in crate::preset::schema) mesh: Option<RawMesh>,
    /// `[layer.per_vertex]` — the same per-vertex surface as the top level, for
    /// a layer whose system is the warp mesh (Plan 0100 Phase 1).
    #[serde(default)]
    pub(in crate::preset::schema) per_vertex: BTreeMap<String, String>,
}

/// One `[latch]` entry, before validation (ADR-0137).
///
/// `hold` defaults to `0`, which is a **single frame** at whatever rate the
/// display runs — the shortest pulse a binding can read, and the right default
/// for an edge-triggered consumer like `recompose`, which acts on the rise and
/// ignores the rest.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::preset::schema) struct RawLatch {
    pub(in crate::preset::schema) arm: String,
    pub(in crate::preset::schema) fire: String,
    #[serde(default)]
    pub(in crate::preset::schema) hold: f32,
}

/// One `[generator] seed` value, before resolution (ADR-0051): a number, or the
/// literal string `"random"`.
///
/// Hand-deserialized rather than `#[serde(untagged)]` for the same reason
/// [`RawSmoothing`] is: an untagged enum reports every failure as "data did not
/// match any variant", where a misspelled `seed = "randmo"` deserves to be told
/// what the accepted forms are.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::preset::schema) enum RawSeed {
    /// `seed = 7` — a fixed salt, the same in the live app and in a capture.
    Fixed(u64),
    /// `seed = "random"` — drawn once per preset load in the live app, and
    /// pinned to the numeric fallback (`0`) on every capture path.
    Random,
}

impl RawSeed {
    /// The **declared** number: the value itself, or `0` for `"random"`. This is
    /// what a capture resolves to, and what the L-system's inert seed field keeps
    /// receiving.
    pub(in crate::preset::schema) fn numeric(self) -> u64 {
        match self {
            Self::Fixed(n) => n,
            Self::Random => 0,
        }
    }
}

impl<'de> Deserialize<'de> for RawSeed {
    fn deserialize<D: serde::Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        de.deserialize_any(RawSeedVisitor)
    }
}

pub(in crate::preset::schema) struct RawSeedVisitor;

impl serde::de::Visitor<'_> for RawSeedVisitor {
    type Value = RawSeed;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a non-negative integer, or the string \"random\"")
    }

    fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<Self::Value, E> {
        Ok(RawSeed::Fixed(v))
    }

    /// TOML has one integer type and it is **signed**, so a plain `seed = 7`
    /// arrives here rather than at `visit_u64`. A negative seed is rejected
    /// rather than reinterpreted as a huge unsigned one — the author meant
    /// something, and it was not `18446744073709551609`.
    fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<Self::Value, E> {
        u64::try_from(v)
            .map(RawSeed::Fixed)
            .map_err(|_| E::invalid_value(serde::de::Unexpected::Signed(v), &self))
    }

    fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Self::Value, E> {
        match v {
            "random" => Ok(RawSeed::Random),
            other => Err(E::invalid_value(serde::de::Unexpected::Str(other), &self)),
        }
    }
}
