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
    /// The optional `[path]` structural-config table (ADR-0107): an authored
    /// silhouette as inline SVG path data, for the shape field. Absent means the
    /// scene draws the `marks` roster, exactly as it did before paths existed.
    #[serde(default)]
    pub(in crate::preset::schema) path: Option<RawPath>,
    /// The optional `[spectrum]` structural-config table (Plan 0034): the element
    /// count, layout and per-element easing of the spectrum readout.
    #[serde(default)]
    pub(in crate::preset::schema) spectrum: Option<RawSpectrum>,
    /// The optional `[mesh]` structural-config table (Plan 0100): the warp
    /// mesh's grid, in cells.
    #[serde(default)]
    pub(in crate::preset::schema) mesh: Option<RawMesh>,
    /// The optional `[field]` structural-config table (ADR-0180 rule 1): which
    /// closed-form family the analytic field draws.
    #[serde(default)]
    pub(in crate::preset::schema) field: Option<RawField>,
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
    /// The optional `[hold]` table (ADR-0180 rule 2): the musical edge each
    /// listed binding re-samples on, each `"beat"`, `"bar"` or a period in
    /// seconds. Absent means every binding is read every frame.
    #[serde(default)]
    pub(in crate::preset::schema) hold: BTreeMap<String, RawHold>,
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
    /// Per-parameter sample-and-hold for the layer's bindings —
    /// `[layer.hold]`, the same vocabulary as the top-level table (ADR-0180
    /// rule 2), against the layer's own bindings.
    #[serde(default)]
    pub(in crate::preset::schema) hold: BTreeMap<String, RawHold>,
    /// The layer's structural tables, per system (ADR-0007) — the same shapes
    /// as the top level's.
    #[serde(default)]
    pub(in crate::preset::schema) curve: Option<RawCurve>,
    #[serde(default)]
    pub(in crate::preset::schema) generator: Option<RawGenerator>,
    #[serde(default)]
    pub(in crate::preset::schema) particles: Option<RawParticles>,
    #[serde(default)]
    pub(in crate::preset::schema) path: Option<RawPath>,
    #[serde(default)]
    pub(in crate::preset::schema) spectrum: Option<RawSpectrum>,
    #[serde(default)]
    pub(in crate::preset::schema) mesh: Option<RawMesh>,
    #[serde(default)]
    pub(in crate::preset::schema) field: Option<RawField>,
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

// ---------------------------------------------------------------------------
// The schema descriptor (Plan 0158 Phase 4). A second statement of this table's
// shape, and one on purpose: serde carries a field's name and type and none of
// what an editor needs -- the closed roster a string is drawn from, the default
// the loader substitutes, the sentence saying what the key does. What holds the
// two together is `schema::tests`, which reads the field roster serde derived
// and asserts it is exactly what the rows below name.
// ---------------------------------------------------------------------------

/// The document root.
pub(in crate::preset::schema) const PRESET: TableDesc = TableDesc {
    name: "preset",
    doc: "The preset document: which scene it drives, what drives its parameters, \
          and the structural tables that shape both.",
    keys: &[
        KeyDesc {
            name: "system",
            kind: KeyKind::Roster(Roster::System),
            default: "",
            doc: "Which built-in scene this preset drives. Required.",
        },
        KeyDesc {
            name: "name",
            kind: KeyKind::Text,
            default: "",
            doc: "Human-readable name; absent means the system's own name.",
        },
        KeyDesc {
            name: "representative",
            kind: KeyKind::Bool,
            default: "false",
            doc: "Whether this preset is one of its family's samples for the narrowed test tier.",
        },
        KeyDesc {
            name: "params",
            kind: KeyKind::Map(&KeyKind::Expr),
            default: "",
            doc: "One expression per named parameter; an unbound parameter keeps its default.",
        },
        KeyDesc {
            name: "per_vertex",
            kind: KeyKind::Map(&KeyKind::Expr),
            default: "",
            doc: "Bindings evaluated once per mesh vertex, with x/y/rad/ang in scope.",
        },
        KeyDesc {
            name: "smoothing",
            kind: KeyKind::Map(&KeyKind::Easing),
            default: "",
            doc: "Per-parameter easing in seconds; an unlisted parameter is applied instantly.",
        },
        KeyDesc {
            name: "hold",
            kind: KeyKind::Map(&KeyKind::Hold),
            default: "",
            doc: "Per-parameter sample-and-hold; an unlisted parameter is read every frame.",
        },
        KeyDesc {
            name: "latch",
            kind: KeyKind::Map(&KeyKind::Table("latch")),
            default: "",
            doc: "Named armed-and-fired events an expression can read as a variable.",
        },
        KeyDesc {
            name: "curve",
            kind: KeyKind::Table("curve"),
            default: "",
            doc: "The parametric curve's family.",
        },
        KeyDesc {
            name: "generator",
            kind: KeyKind::Table("generator"),
            default: "",
            doc: "The L-system's grammar or the star pattern's tiling, plus the preset's seed.",
        },
        KeyDesc {
            name: "particles",
            kind: KeyKind::Table("particles"),
            default: "",
            doc: "The attractor family, its density, and what morph travels towards.",
        },
        KeyDesc {
            name: "path",
            kind: KeyKind::Table("path"),
            default: "",
            doc: "An authored silhouette for the shape field, as inline SVG path data.",
        },
        KeyDesc {
            name: "spectrum",
            kind: KeyKind::Table("spectrum"),
            default: "",
            doc: "How the readout divides the frequency axis and what figure it forms.",
        },
        KeyDesc {
            name: "mesh",
            kind: KeyKind::Table("mesh"),
            default: "",
            doc: "The warp mesh's grid, in cells.",
        },
        KeyDesc {
            name: "field",
            kind: KeyKind::Table("field"),
            default: "",
            doc: "Which closed-form world the analytic field draws.",
        },
        KeyDesc {
            name: "milk",
            kind: KeyKind::Table("milk"),
            default: "",
            doc: "A converted MilkDrop preset's compiled programs and shaders.",
        },
        KeyDesc {
            name: "feedback",
            kind: KeyKind::Table("feedback"),
            default: "",
            doc: "Which warp the accumulation buffers resample their past through, and how \
                  this frame is deposited onto it.",
        },
        KeyDesc {
            name: "palette",
            kind: KeyKind::Table("palette"),
            default: "",
            doc: "The colour gradient; absent means the built-in spectrum.",
        },
        KeyDesc {
            name: "palette_b",
            kind: KeyKind::Table("palette"),
            default: "",
            doc: "The crossfade target a bindable palette_mix travels towards.",
        },
        KeyDesc {
            name: "occupancy",
            kind: KeyKind::Table("occupancy"),
            default: "",
            doc: "Parameters whose clamp bounds are meant to pin, exempted from the saturation gate.",
        },
        KeyDesc {
            name: "layer",
            kind: KeyKind::Table("layer"),
            default: "",
            doc: "A second scene drawn with the first.",
        },
    ],
};

/// The `[layer]` sub-preset: everything a preset has except the tables that
/// belong to the frame as a whole.
pub(in crate::preset::schema) const LAYER: TableDesc = TableDesc {
    name: "layer",
    doc: "A second scene drawn with the first, with its own system, bindings and structure.",
    keys: &[
        KeyDesc {
            name: "system",
            kind: KeyKind::Roster(Roster::System),
            default: "",
            doc: "Which built-in scene the layer draws. Required.",
        },
        KeyDesc {
            name: "join",
            kind: KeyKind::Roster(Roster::LayerJoin),
            default: "under",
            doc: "Where the layer joins: into the main scene's target, or into the chain \
                  as its own image.",
        },
        KeyDesc {
            name: "blend",
            kind: KeyKind::Roster(Roster::LayerBlend),
            default: "screen",
            doc: "How an over-joined layer blends into the chain. Inert on an under join.",
        },
        KeyDesc {
            name: "mix",
            kind: KeyKind::Expr,
            default: "",
            doc: "The over junction's amount; absent means full strength.",
        },
        KeyDesc {
            name: "params",
            kind: KeyKind::Map(&KeyKind::Expr),
            default: "",
            doc: "The layer's own parameter bindings, namespaced to its scene.",
        },
        KeyDesc {
            name: "per_vertex",
            kind: KeyKind::Map(&KeyKind::Expr),
            default: "",
            doc: "The layer's per-vertex bindings, for a warp-mesh layer.",
        },
        KeyDesc {
            name: "smoothing",
            kind: KeyKind::Map(&KeyKind::Easing),
            default: "",
            doc: "Per-parameter easing for the layer's bindings.",
        },
        KeyDesc {
            name: "hold",
            kind: KeyKind::Map(&KeyKind::Hold),
            default: "",
            doc: "Per-parameter sample-and-hold for the layer's bindings.",
        },
        KeyDesc {
            name: "curve",
            kind: KeyKind::Table("curve"),
            default: "",
            doc: "The layer's parametric curve family.",
        },
        KeyDesc {
            name: "generator",
            kind: KeyKind::Table("generator"),
            default: "",
            doc: "The layer's L-system grammar or star tiling.",
        },
        KeyDesc {
            name: "particles",
            kind: KeyKind::Table("particles"),
            default: "",
            doc: "The layer's attractor family.",
        },
        KeyDesc {
            name: "path",
            kind: KeyKind::Table("path"),
            default: "",
            doc: "The layer's authored silhouette.",
        },
        KeyDesc {
            name: "spectrum",
            kind: KeyKind::Table("spectrum"),
            default: "",
            doc: "The layer's readout configuration.",
        },
        KeyDesc {
            name: "mesh",
            kind: KeyKind::Table("mesh"),
            default: "",
            doc: "The layer's warp-mesh grid.",
        },
        KeyDesc {
            name: "field",
            kind: KeyKind::Table("field"),
            default: "",
            doc: "The layer's analytic-field family.",
        },
    ],
};

/// One `[latch]` entry.
pub(in crate::preset::schema) const LATCH: TableDesc = TableDesc {
    name: "latch",
    doc: "A gate armed on one condition and fired by the first rising edge of another \
          inside the arming window.",
    keys: &[
        KeyDesc {
            name: "arm",
            kind: KeyKind::Expr,
            default: "",
            doc: "While this holds above 0.5 the latch is armed; its fall re-arms it. Required.",
        },
        KeyDesc {
            name: "fire",
            kind: KeyKind::Expr,
            default: "",
            doc: "The rising edge that fires an armed latch. Required.",
        },
        KeyDesc {
            name: "hold",
            kind: KeyKind::Float,
            default: "0",
            doc: "How long the fired latch reads 1, in seconds. 0 is a single frame.",
        },
    ],
};
