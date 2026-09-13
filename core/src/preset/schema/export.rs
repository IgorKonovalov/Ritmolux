//! What a preset may contain, as a machine-readable document (Plan 0158
//! Phase 4).
//!
//! A studio building a parameter panel, an expression editor or a table form has
//! to know two things the engine already knows: every **parameter** a system and
//! the engine stages accept, and every **key** a structural table accepts. Both
//! are declared in the engine and read by the loader; this renders them.
//!
//! ## The parameter half is generated, not restated
//!
//! It walks the same [`ParamSpec`] declarations the generated block in
//! `presets/README.md` is rendered from (ADR-0170) — the same rosters, in the
//! same order — so the published table and this document cannot disagree about a
//! name, a default or a range. `core/tests/preset.rs` renders both from one walk
//! and asserts they agree.
//!
//! ## The structural half is declared beside its serde struct
//!
//! Every `[table]` a preset may write carries a [`TableDesc`] next to the `Raw*`
//! struct the loader deserializes it into. That is a second statement of the
//! table's shape and it is one on purpose: serde carries a field's *name* and
//! *type*, and none of what an editor needs — the closed roster a string is
//! drawn from, the default the loader substitutes, the sentence that says what
//! the key does. What stops the two drifting is a test, not a construction:
//! `core/src/preset/schema/tests.rs` reads the field roster serde derived
//! (through a `Deserializer` that answers nothing and records what it was asked
//! for) and asserts it is exactly what the descriptor names.
//!
//! **No enumeration is written here.** A `KeyKind::Roster` names the type that
//! owns the closed set, and [`Roster::values`] asks it — so `[feedback] warp`'s
//! roster is `Warp::ALL` and there is nothing here to fall out of step with it.
//!
//! ## The hash
//!
//! [`hash`] is FNV-1a over the document **body** — everything the export
//! declares, without the hash field itself, which would otherwise be hashing its
//! own output. A studio compares it against the one a player reports in `hello`
//! to know whether the schema it built its panels from is the schema the player
//! is running.
//!
//! No JSON crate: the writer below is thirty lines and NFR section 4's
//! dependency gate asks for a justification longer than that.

// A continuation of one module split across several files, so it needs the
// names `preset/schema/mod.rs` has in scope.
use super::*;

use crate::render::scenes::ParamSpec;

/// The document format's own version, carried as `v` so a consumer can refuse a
/// shape it does not know. Independent of the workspace version and of the
/// event stream's `v`: this one moves when the *document* changes shape, which
/// is rarer than either.
pub const SCHEMA_VERSION: u32 = 1;

// ---------------------------------------------------------------------------
// The descriptor vocabulary
// ---------------------------------------------------------------------------

/// A closed roster of accepted spellings, named by the type that owns it.
///
/// The whole point of the indirection: a `KeyKind::Roster(Roster::Warp)` renders
/// `Warp::ALL`, so the export cannot list a warp the loader rejects or omit one
/// it accepts. Writing the strings here instead would be a second copy of every
/// roster in the engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Roster {
    /// `system` — the built-in scenes.
    System,
    /// `[curve] family`.
    CurveFamily,
    /// `[particles] family` — the four maps plus every IFS figure.
    AttractorFamily,
    /// `[particles] morph_to` — the IFS figures alone.
    IfsFigure,
    /// `[spectrum] layout`.
    SpectrumLayout,
    /// `[feedback] warp`.
    Warp,
    /// `[feedback] blend`.
    Deposit,
    /// `[palette] name`.
    Palette,
    /// `[layer] join`.
    LayerJoin,
    /// `[layer] blend`.
    LayerBlend,
    /// `[generator] rings` motif.
    Motif,
    /// `[generator] tiling`.
    Tiling,
    /// `[field] family`.
    FieldFamily,
    /// `[field] map`.
    EscapeMap,
    /// `[field] trap`.
    TrapShape,
    /// `[cellular] family`.
    CellularFamily,
}

impl Roster {
    /// The accepted spellings, asked of the type that owns the roster.
    pub fn values(self) -> Vec<&'static str> {
        use crate::render::feedback::{Deposit, Warp};
        use crate::render::palette::NamedPalette;
        use crate::render::scenes::lines::star::Motif;
        use crate::render::scenes::lines::{CurveFamily, SpectrumLayout};
        use crate::render::scenes::particles::AttractorFamily;
        use crate::render::scenes::particles::ifs::IfsFigure;
        match self {
            Roster::System => SystemKind::ALL.iter().map(|k| k.as_str()).collect(),
            Roster::CurveFamily => CurveFamily::ALL.iter().map(|f| f.as_str()).collect(),
            Roster::AttractorFamily => AttractorFamily::MAPS
                .iter()
                .map(|f| f.as_str())
                .chain(IfsFigure::ALL.iter().map(|f| f.name()))
                .collect(),
            Roster::IfsFigure => IfsFigure::ALL.iter().map(|f| f.name()).collect(),
            Roster::SpectrumLayout => SpectrumLayout::NAMES.to_vec(),
            Roster::Warp => Warp::ALL.iter().map(|w| w.as_str()).collect(),
            Roster::Deposit => Deposit::ALL.iter().map(|d| d.as_str()).collect(),
            Roster::Palette => NamedPalette::ALL.iter().map(|p| p.as_str()).collect(),
            Roster::LayerJoin => LayerJoin::ALL.iter().map(|j| j.as_str()).collect(),
            Roster::LayerBlend => LayerBlend::ALL.iter().map(|b| b.as_str()).collect(),
            Roster::Motif => Motif::ALL.iter().map(|m| m.name()).collect(),
            Roster::Tiling => crate::render::scenes::lines::hankin::TILINGS.to_vec(),
            Roster::FieldFamily => FieldFamily::ALL.iter().map(|f| f.as_str()).collect(),
            Roster::EscapeMap => EscapeMap::ALL.iter().map(|m| m.as_str()).collect(),
            Roster::TrapShape => TrapShape::ALL.iter().map(|t| t.as_str()).collect(),
            Roster::CellularFamily => CellularFamily::ALL.iter().map(|f| f.as_str()).collect(),
        }
    }

    /// Whether `name` is in this roster — the membership test the round-trip
    /// test uses against each type's own `from_name`.
    pub fn accepts(self, name: &str) -> bool {
        self.values().contains(&name)
    }
}

/// What one structural key accepts.
///
/// Deliberately coarser than serde's type: an editor needs to know that
/// `[path] d` is *free text* and `[curve] family` is *one of a closed set*, and
/// serde reports both as `String`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyKind {
    /// `true` / `false`.
    Bool,
    /// A whole number.
    Int,
    /// A real number.
    Float,
    /// Free text with no roster behind it — a path's `d`, an axiom, a shader
    /// body.
    Text,
    /// An expression in the preset grammar.
    Expr,
    /// One of a closed set.
    Roster(Roster),
    /// An easing constant: seconds as a number, or `{ attack, release }`.
    Easing,
    /// A salt: a number, or the string `"random"` (ADR-0051).
    Seed,
    /// A hold edge: `"beat"`, `"bar"`, or a positive number of seconds
    /// (ADR-0180 rule 2). Not a `Roster`, because the number is not one of a
    /// closed set and an editor offering only the two words would reject a
    /// legal entry.
    Hold,
    /// A colour: `"#rrggbb"` or `[r, g, b]` in `0..=1`.
    Colour,
    /// A table of author-chosen names to values of this kind.
    Map(&'static KeyKind),
    /// A list of values of this kind.
    List(&'static KeyKind),
    /// A nested table, named in [`TABLES`].
    Table(&'static str),
}

impl KeyKind {
    /// The document's spelling for this kind.
    fn tag(&self) -> &'static str {
        match self {
            KeyKind::Bool => "bool",
            KeyKind::Int => "int",
            KeyKind::Float => "float",
            KeyKind::Text => "text",
            KeyKind::Expr => "expr",
            KeyKind::Roster(_) => "enum",
            KeyKind::Easing => "easing",
            KeyKind::Seed => "seed",
            KeyKind::Hold => "hold",
            KeyKind::Colour => "colour",
            KeyKind::Map(_) => "map",
            KeyKind::List(_) => "list",
            KeyKind::Table(_) => "table",
        }
    }
}

/// One key of one structural table.
#[derive(Debug, Clone, Copy)]
pub struct KeyDesc {
    /// The key as it is written in the file.
    pub name: &'static str,
    /// What it accepts.
    pub kind: KeyKind,
    /// What the loader uses when the key is absent, written as it would appear
    /// in TOML. Empty where absence means something other than a value — a
    /// required key, or one whose absence turns a feature off rather than
    /// selecting a value.
    pub default: &'static str,
    /// One line: what it does.
    pub doc: &'static str,
}

/// One structural table a preset may write.
#[derive(Debug, Clone, Copy)]
pub struct TableDesc {
    /// The table's name — its TOML header, or the name a [`KeyKind::Table`]
    /// refers to it by. `preset` is the document root.
    pub name: &'static str,
    /// One line: what the table is for.
    pub doc: &'static str,
    /// Its keys, in declaration order.
    pub keys: &'static [KeyDesc],
}

/// Every table, root first. A [`KeyKind::Table`] names one of these.
///
/// Flat with references rather than nested inline, because `[layer]` carries
/// most of the root's tables and inlining would print each of them twice — and
/// a consumer building a form wants one definition per table, not one per site.
pub const TABLES: &[&TableDesc] = &[
    &super::raw::PRESET,
    &super::raw::LAYER,
    &super::raw::LATCH,
    &super::raw::CURVE,
    &super::raw::GENERATOR,
    &super::raw::RING,
    &super::raw::PARTICLES,
    &super::raw::PATH,
    &super::raw::SPECTRUM,
    &super::raw::MESH,
    &super::raw::FIELD,
    &super::raw::CELLULAR,
    &super::raw::MILK,
    &super::raw::MILK_ELEMENT,
    &super::raw::FEEDBACK,
    &super::raw::OCCUPANCY,
    &super::raw::PALETTE,
    &super::raw::STOP,
];

/// The table named `name`, or `None`.
pub fn table(name: &str) -> Option<&'static TableDesc> {
    TABLES.iter().copied().find(|t| t.name == name)
}

// ---------------------------------------------------------------------------
// The parameter half
// ---------------------------------------------------------------------------

/// The engine-wide stages, labelled as a reader meets them rather than as the
/// modules are named.
///
/// Zipped against [`GLOBAL_PARAMS`] rather than naming each module, because that
/// array is already the one statement of which stages a preset may bind whatever
/// its system. The labels are in its order, and
/// [`param_rosters`]'s length check is what holds them there.
pub const STAGE_LABELS: [&str; 7] = [
    "background",
    "trails",
    "kaleidoscope",
    "bloom",
    "composite",
    "tonemap",
    "ink",
];

/// Every parameter roster a preset may bind, labelled: one per system, then one
/// per engine stage.
///
/// **The one walk.** The generated reference in `presets/README.md` and the
/// document below are both rendered from this, so the two cannot state different
/// defaults for a name (ADR-0170).
pub fn param_rosters() -> Vec<(&'static str, &'static [ParamSpec])> {
    let mut out: Vec<(&'static str, &'static [ParamSpec])> = SystemKind::ALL
        .iter()
        .map(|kind| (kind.as_str(), kind.param_specs()))
        .collect();
    // A stage that joined `GLOBAL_PARAMS` without a label here would be printed
    // under its neighbour's heading, so it is dropped rather than mislabelled —
    // and `the_stage_labels_cover_every_global_roster` fails on it.
    out.extend(STAGE_LABELS.iter().copied().zip(GLOBAL_PARAMS));
    out
}

/// How many of the rosters are engine stages rather than systems — the tail of
/// [`param_rosters`].
pub fn stage_count() -> usize {
    STAGE_LABELS.len().min(GLOBAL_PARAMS.len())
}

// ---------------------------------------------------------------------------
// The document
// ---------------------------------------------------------------------------

/// The whole schema as JSON, hash included.
///
/// One line of output; a consumer parses it, and a human reading it reaches for
/// a formatter. Deterministic: every roster it walks is a `const` array in
/// declaration order, so two runs on one build produce identical bytes.
pub fn document() -> String {
    let body = body();
    format!(
        "{{\"v\":{SCHEMA_VERSION},\"hash\":\"{:016x}\",{body}",
        hash_str(&body)
    )
}

/// The document's stable hash.
///
/// FNV-1a over the **body** — the document without its own `v` and `hash` — so
/// the hash is a function of what the engine declares rather than of itself. It
/// changes when and only when that declaration changes.
pub fn hash() -> u64 {
    hash_str(&body())
}

/// The hash as the sixteen hex characters `document` prints and `hello` reports.
pub fn hash_hex() -> String {
    format!("{:016x}", hash())
}

/// Everything the export declares, as the tail of a JSON object — from
/// `"systems"` through the closing brace.
fn body() -> String {
    let rosters = param_rosters();
    let stages = stage_count();
    let split = rosters.len().saturating_sub(stages);
    let mut out = String::with_capacity(64 * 1024);

    out.push_str("\"systems\":[");
    for (i, (label, specs)) in rosters.iter().take(split).enumerate() {
        if i > 0 {
            out.push(',');
        }
        push_roster(&mut out, label, specs);
    }
    out.push_str("],\"stages\":[");
    for (i, (label, specs)) in rosters.iter().skip(split).enumerate() {
        if i > 0 {
            out.push(',');
        }
        push_roster(&mut out, label, specs);
    }
    out.push_str("],\"tables\":[");
    for (i, table) in TABLES.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        push_table(&mut out, table);
    }
    out.push_str("],\"grammar\":");
    push_grammar(&mut out);
    out.push('}');
    out
}

/// The expression grammar's three identifier rosters.
///
/// **Generated by walking the engine's own declarations**, the same discipline
/// the parameter half is held to (ADR-0170): `variables` is what the parser's
/// identifier lookup accepts out of `VAR_NAMES`, `functions` is the roster
/// `Func::from_name` resolves through, and `constants` is the table `constant`
/// resolves through. Nothing here is a second copy, so an editor colouring these
/// colours exactly what the engine knows.
///
/// The reserved `[latch]` placeholders are **absent** from `variables`, because
/// the parser refuses them by name: an author reaches a latch through the name
/// they declared for it, and offering `_latch0` would be offering a spelling
/// that does not compile. A preset's own latch names are in the preset, not in
/// the schema.
fn push_grammar(out: &mut String) {
    out.push_str("{\"variables\":");
    push_names(out, expr::variable_names());
    out.push_str(",\"functions\":");
    push_names(out, expr::function_names());
    out.push_str(",\"constants\":");
    push_names(out, expr::constant_names());
    out.push('}');
}

/// A JSON array of strings.
fn push_names(out: &mut String, names: impl Iterator<Item = &'static str>) {
    out.push('[');
    for (i, name) in names.enumerate() {
        if i > 0 {
            out.push(',');
        }
        push_string(out, name);
    }
    out.push(']');
}

/// One labelled parameter roster.
fn push_roster(out: &mut String, label: &str, specs: &[ParamSpec]) {
    out.push_str("{\"name\":");
    push_string(out, label);
    out.push_str(",\"params\":[");
    for (i, spec) in specs.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str("{\"name\":");
        push_string(out, spec.name);
        out.push_str(",\"default\":");
        push_number(out, spec.default);
        out.push_str(",\"range\":");
        match spec.range {
            Some([lo, hi]) => {
                out.push('[');
                push_number(out, lo);
                out.push(',');
                push_number(out, hi);
                out.push(']');
            }
            None => out.push_str("null"),
        }
        out.push_str(",\"doc\":");
        push_string(out, spec.doc);
        // ADR-0180 rule 4's distinction, so a studio can group its panel the
        // way the reference groups its tables. Additive: a consumer that does
        // not know the field ignores it, which is why `SCHEMA_VERSION` does
        // not move — the body hash does, and that is the staleness signal a
        // studio already compares.
        out.push_str(",\"kind\":");
        push_string(out, spec.kind.as_str());
        out.push('}');
    }
    out.push_str("]}");
}

/// One structural table.
fn push_table(out: &mut String, table: &TableDesc) {
    out.push_str("{\"name\":");
    push_string(out, table.name);
    out.push_str(",\"doc\":");
    push_string(out, table.doc);
    out.push_str(",\"keys\":[");
    for (i, key) in table.keys.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str("{\"name\":");
        push_string(out, key.name);
        out.push_str(",\"default\":");
        push_string(out, key.default);
        out.push_str(",\"doc\":");
        push_string(out, key.doc);
        out.push(',');
        push_kind(out, &key.kind);
        out.push('}');
    }
    out.push_str("]}");
}

/// One key's kind, as the fields it contributes to that key's object.
fn push_kind(out: &mut String, kind: &KeyKind) {
    out.push_str("\"kind\":");
    push_string(out, kind.tag());
    match kind {
        KeyKind::Roster(roster) => {
            out.push_str(",\"values\":[");
            for (i, value) in roster.values().iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                push_string(out, value);
            }
            out.push(']');
        }
        KeyKind::Map(of) | KeyKind::List(of) => {
            out.push_str(",\"of\":{");
            push_kind(out, of);
            out.push('}');
        }
        KeyKind::Table(name) => {
            out.push_str(",\"table\":");
            push_string(out, name);
        }
        _ => {}
    }
}

/// A JSON string, quotes included.
///
/// Escapes what RFC 8259 requires and nothing else: the quote, the backslash,
/// and every control character below `0x20`. Doc lines are hand-written ASCII
/// prose, so the `\u` arm is a guard against a future one rather than a live
/// case.
fn push_string(out: &mut String, text: &str) {
    out.push('"');
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

/// A JSON number.
///
/// `{:?}` on an `f32` prints the shortest decimal that round-trips, which keeps
/// `0.5` as `0.5` rather than `0.5000` and keeps the hash stable across the
/// values a spec can hold. A non-finite default cannot be written as JSON, so it
/// is emitted as `null` — the loader would refuse such a spec long before this,
/// and printing `NaN` would produce a document nothing can parse.
fn push_number(out: &mut String, value: f32) {
    if value.is_finite() {
        out.push_str(&format!("{value:?}"));
    } else {
        out.push_str("null");
    }
}

/// FNV-1a, 64-bit, over `text`'s bytes — the function [`hash`] uses, exposed so
/// a test can hash a deliberately perturbed copy of the document and assert the
/// hash moved with it.
///
/// Not cryptographic and not asked to be: the question it answers is "is the
/// studio's copy of this document the one this player is running", where the
/// adversary is a stale file rather than a person. Written out rather than taken
/// from `DefaultHasher`, whose output std explicitly does not promise to be
/// stable across releases — a hash a studio caches has to survive a toolchain
/// bump.
pub fn hash_str(text: &str) -> u64 {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut hash = OFFSET;
    for byte in text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}

// ---------------------------------------------------------------------------
// The editor schema (ADR-0190)
// ---------------------------------------------------------------------------

/// The JSON Schema an editor completes preset TOML from, as committed to
/// `presets/preset.schema.json` and associated with preset files by `.taplo.toml`.
///
/// **A second rendering of the declarations [`document`] prints**, not a second
/// copy of them: the structural half walks [`TABLES`], the parameter half walks
/// [`SystemKind::param_specs`] and [`GLOBAL_PARAMS`], and neither writes a name
/// or a roster of its own. `core/tests/preset_schema.rs` holds the committed file
/// to this function and validates the whole preset corpus against the *same*
/// declarations — so the file, the editor and the loader cannot disagree about
/// what a preset may contain.
///
/// Draft-07, because that is what Even Better TOML's Taplo backend implements —
/// in particular the `if`/`then` the per-system parameter sets need.
///
/// **This file validates and does not complete.** Taplo reads hover text and
/// completions only off unconditional properties, so a `[params]` key inside one
/// of the `if`/`then` cases below gets neither. It is the fallback `.taplo.toml`
/// applies to a file no family rule claims; [`system_json_schema`] is what a
/// library file named for its family gets.
///
/// ## What it does and does not enforce
///
/// `additionalProperties: false` appears **only** where the engine's declaration
/// is authoritative for the whole key set: the document root, each structural
/// table, and each system's `[params]`. It is deliberately absent from the
/// author-keyed maps — `[smoothing]`, `[hold]`, `[occupancy]`, `[latch]` — whose
/// keys the preset chooses.
///
/// **No range is enforced anywhere.** [`ParamSpec::range`] is documented as
/// neither a clamp nor a validation bound, and presets set a value outside it on
/// purpose; a range that underlined a correct preset would be worse than no range
/// at all. It is carried as prose in `markdownDescription`, where an author reads
/// it and nothing acts on it.
///
/// Every `[params]` value is `type: string`: a binding is an expression, and
/// `glow = 1.0` is a load error rather than a shorthand.
pub fn json_schema() -> String {
    let mut out = String::with_capacity(256 * 1024);
    out.push_str("{\n");
    out.push_str("  \"$schema\": \"http://json-schema.org/draft-07/schema#\",\n");
    out.push_str("  \"title\": \"Ritmolux preset\",\n");
    out.push_str("  \"description\": ");
    out.push_str(&json_string(
        "Generated from the engine's own parameter and table declarations. Do not \
         edit by hand: re-run the core test suite with RLX_UPDATE_PRESET_SCHEMA=1.",
    ));
    out.push_str(",\n  \"type\": \"object\",\n");
    out.push_str("  \"additionalProperties\": false,\n");

    out.push_str("  \"properties\": {\n");
    push_table_properties(&mut out, 2, &super::raw::PRESET);
    out.push_str("\n  },\n");

    // Each distinct parameter declaration is written **once**, under
    // `definitions`, and every system accepting it `$ref`s that one entry.
    // Without the indirection the 28 cases below carry some 1,250 copies of a
    // hover text, and a one-word edit to a parameter's doc line rewrites the file
    // in 28 places instead of one.
    let definitions = param_definitions();

    // The per-system parameter sets. One `if`/`then` per system per surface: the
    // root's `[params]` and a `[layer]`'s own accept different sets, because a
    // layer binds its scene's parameters and never the compositing stages.
    out.push_str("  \"allOf\": [\n");
    let mut first = true;
    for surface in [ParamSurface::Root, ParamSurface::Layer] {
        for kind in SystemKind::ALL {
            push_separator(&mut out, &mut first);
            push_system_case(&mut out, 2, kind, surface, &definitions);
        }
    }
    out.push_str("\n  ],\n");

    out.push_str("  \"definitions\": {\n");
    let mut first = true;
    for table in TABLES {
        // The root is the document itself rather than a definition, and nothing
        // `$ref`s it.
        if table.name == super::raw::PRESET.name {
            continue;
        }
        push_separator(&mut out, &mut first);
        push_definition(&mut out, 2, table);
    }
    for (key, spec) in &definitions {
        push_separator(&mut out, &mut first);
        push_param_definition(&mut out, 2, key, spec);
    }
    out.push_str("\n  }\n}\n");
    out
}

/// Where the per-system schemas are committed, relative to the repository root.
pub const SYSTEM_SCHEMA_DIR: &str = "presets/schema";

/// Where the generic schema is committed, relative to the repository root.
pub const GENERIC_SCHEMA_PATH: &str = "presets/preset.schema.json";

/// Where the editor association is committed, relative to the repository root.
pub const TAPLO_CONFIG_PATH: &str = ".taplo.toml";

/// The committed path of `kind`'s own schema, relative to the repository root.
pub fn system_schema_path(kind: SystemKind) -> String {
    format!("{SYSTEM_SCHEMA_DIR}/{}.schema.json", kind.as_str())
}

/// Every generated editor file, as `(path relative to the repository root,
/// content)`: the generic schema, one schema per system, and `.taplo.toml`.
///
/// **The one list** the drift test compares and the regenerate command writes,
/// so no file can be rendered and not checked, or checked and not rendered.
pub fn editor_files() -> Vec<(String, String)> {
    let mut out = vec![(GENERIC_SCHEMA_PATH.to_owned(), json_schema())];
    out.extend(
        SystemKind::ALL
            .iter()
            .map(|kind| (system_schema_path(*kind), system_json_schema(*kind))),
    );
    out.push((TAPLO_CONFIG_PATH.to_owned(), taplo_config()));
    out
}

/// The JSON Schema for a preset driving `kind`, self-contained, as committed
/// under `presets/schema/`.
///
/// **No `if`/`then` on the path from the root to a parameter.** Taplo validates
/// through a conditional but reads neither hover text nor completions through
/// one, so [`json_schema`]'s per-system cases complete nothing. Here `system` is
/// a `const` and `params` is this system's property set, unconditionally — the
/// set [`params_of`] gives the generic file's matching case, `$ref`ing the same
/// definition keys.
///
/// **`[layer]` is not narrowed.** A layer names its own system, and narrowing it
/// here would take one conditional per system and every system's parameter
/// definitions in every file. Its `params` stays what the table declares: any
/// name, bound to a string. The generic file still validates a layer's names.
///
/// `definitions` is pruned to what this file references — the structural tables
/// reachable from the root, and this system's parameter declarations. A `$ref`
/// into another file is not something Taplo resolves, so nothing is shared.
pub fn system_json_schema(kind: SystemKind) -> String {
    let definitions = param_definitions();
    let params = params_of(kind, ParamSurface::Root);
    let name = json_string(kind.as_str());

    let mut out = String::with_capacity(96 * 1024);
    out.push_str("{\n");
    out.push_str("  \"$schema\": \"http://json-schema.org/draft-07/schema#\",\n");
    out.push_str(&format!(
        "  \"title\": {},\n",
        json_string(&format!("Ritmolux preset: {}", kind.as_str()))
    ));
    out.push_str("  \"description\": ");
    out.push_str(&json_string(&format!(
        "The schema for a preset driving `{}`, selected by a library filename beginning \
         `{}_`. Generated from the engine's own parameter and table declarations. Do not \
         edit by hand: re-run the core test suite with RLX_UPDATE_PRESET_SCHEMA=1.",
        kind.as_str(),
        kind.family()
    )));
    out.push_str(",\n  \"type\": \"object\",\n");
    out.push_str("  \"additionalProperties\": false,\n");

    out.push_str("  \"properties\": {\n");
    push_table_properties_with(
        &mut out,
        2,
        &super::raw::PRESET,
        |out, depth, key| match key.name {
            "system" => {
                let pad = "  ".repeat(depth);
                out.push_str(&format!(
                    "{pad}\"type\": \"string\",\n{pad}\"const\": {name}"
                ));
                true
            }
            "params" => {
                push_params_body(out, depth, &params, &definitions);
                true
            }
            _ => false,
        },
    );
    out.push_str("\n  },\n");

    let tables = reachable_tables(&super::raw::PRESET);
    out.push_str("  \"definitions\": {\n");
    let mut first = true;
    for table in TABLES {
        if tables.contains(&table.name) {
            push_separator(&mut out, &mut first);
            push_definition(&mut out, 2, table);
        }
    }
    for (key, spec) in &definitions {
        if params
            .iter()
            .any(|accepted| same_declaration(accepted, spec))
        {
            push_separator(&mut out, &mut first);
            push_param_definition(&mut out, 2, key, spec);
        }
    }
    out.push_str("\n  }\n}\n");
    out
}

/// The name of every structural table a `$ref` reaches from `root`'s keys, at
/// any depth, `root` itself excluded unless something refers back to it.
fn reachable_tables(root: &TableDesc) -> Vec<&'static str> {
    fn visit(kind: &KeyKind, seen: &mut Vec<&'static str>) {
        match kind {
            KeyKind::Table(name) => {
                if !seen.contains(name) {
                    seen.push(*name);
                    for key in table(name).map_or(&[][..], |t| t.keys) {
                        visit(&key.kind, seen);
                    }
                }
            }
            KeyKind::Map(of) | KeyKind::List(of) => visit(of, seen),
            _ => {}
        }
    }
    let mut seen = Vec::new();
    for key in root.keys {
        visit(&key.kind, &mut seen);
    }
    seen
}

/// The three library directories, as the `.taplo.toml` globs spell them.
const LIBRARY_DIRS: [&str; 3] = ["presets", "presets/proposed", "presets/pending"];

/// The glob matching a library file of `family` in `dir`, one of
/// [`LIBRARY_DIRS`].
fn family_glob(dir: &str, family: &str) -> String {
    format!("/**/{dir}/{family}_*.toml")
}

/// `.taplo.toml`: which schema Even Better TOML applies to which preset file.
///
/// One rule per system, matching its family's filenames in the three library
/// directories, then one fallback rule on the generic schema that **excludes**
/// every family glob. The exclusion is what routes a family file to its own
/// schema: Taplo breaks a tie between two matching rules by taking the later one,
/// which is the fallback, so rule order alone would select the wrong file.
///
/// Every glob begins with `/`. Taplo joins a relative glob onto the workspace
/// root's filesystem path but matches it against the document URL with only the
/// scheme stripped, and on Windows those two differ by a leading `/`, so a
/// relative glob matches nothing there. The generated header says so too, for
/// the reader who opens the file rather than this function.
pub fn taplo_config() -> String {
    let mut out = String::with_capacity(8 * 1024);
    out.push_str(TAPLO_HEADER);
    for kind in SystemKind::ALL {
        out.push_str("\n[[rule]]\n");
        out.push_str(&format!("name = \"ritmolux-{}\"\n", kind.as_str()));
        push_toml_globs(
            &mut out,
            "include",
            LIBRARY_DIRS
                .iter()
                .map(|dir| family_glob(dir, kind.family())),
        );
        out.push_str(&format!("schema.path = \"{}\"\n", system_schema_path(kind)));
    }
    out.push_str("\n[[rule]]\n");
    out.push_str("name = \"ritmolux-preset\"\n");
    push_toml_globs(
        &mut out,
        "include",
        LIBRARY_DIRS
            .iter()
            .map(|dir| format!("/**/{dir}/*.toml"))
            .chain(std::iter::once("/**/docs/examples/**/*.toml".to_owned())),
    );
    push_toml_globs(
        &mut out,
        "exclude",
        SystemKind::ALL.iter().flat_map(|kind| {
            LIBRARY_DIRS
                .iter()
                .map(|dir| family_glob(dir, kind.family()))
        }),
    );
    out.push_str(&format!("schema.path = \"{GENERIC_SCHEMA_PATH}\"\n"));
    out
}

/// `key = [ ... ]`, one quoted glob per line.
fn push_toml_globs(out: &mut String, key: &str, globs: impl Iterator<Item = String>) {
    out.push_str(&format!("{key} = [\n"));
    for glob in globs {
        out.push_str(&format!("  \"{glob}\",\n"));
    }
    out.push_str("]\n");
}

/// The comment block `.taplo.toml` opens with. Written out rather than
/// rendered, because nothing in it is a declaration the engine owns.
const TAPLO_HEADER: &str = "\
# GENERATED - do not edit. core/tests/preset_schema.rs fails if this file, the
# generic schema or any per-system schema is stale. Regenerate all of them with
#
#   RLX_UPDATE_PRESET_SCHEMA=1 cargo nextest run -p rlx-core the_generated_editor_files_are_current
#
# Associates the generated JSON Schemas with the preset files they describe, so
# an editor with Even Better TOML installed completes parameter names, shows each
# one's documentation on hover, and flags a key the preset's system does not
# accept. No per-user setup: the extension finds this file at the workspace root.
#
# THE `[formatting]` TABLE IS DELIBERATELY ABSENT, AND MUST STAY ABSENT.
# This project does not format TOML (ADR-0190). The presets carry deliberate,
# local alignment no formatter models - column-padded inline tables, `=` columns
# aligned per block, a two-space gap before a trailing comment - and `taplo fmt`
# measured against this corpus rewrote 99 to 121 of 124 files under every
# configuration tried and panicked on eight shipped presets under its defaults.
# A `[formatting]` table here would switch on the one Taplo feature that was
# rejected on measurement. The extension still ships a formatter this file cannot
# disable; docs/developing.md says how to keep it off for TOML.
#
# ONE SCHEMA PER SYSTEM, SELECTED BY THE FILENAME. Taplo validates through a
# schema's `if`/`then` but reads neither hover text nor completions through one,
# so a single schema keyed on `system` completes nothing inside `[params]` (Plan
# 0169 Phase 4). Each system therefore has a self-contained schema under
# presets/schema/, and a library file reaches it through its filename's family
# prefix: `collage_*.toml` gets shape_collage's. `ritmolux --check` warns on a
# library file named off its family, which is exactly the file that would get no
# completion.
#
# The last rule is the fallback: presets/preset.schema.json, which validates
# every system's parameters and completes none. It covers the teaching files in
# docs/examples/ and any library file no family rule claims, and it EXCLUDES every
# family glob. That exclusion is what hands a family file its own schema, not
# rule order: Taplo breaks a tie between matching rules by taking the later one,
# which is the fallback.
#
# EVERY GLOB STARTS WITH `/`. Taplo joins a relative glob onto the workspace root
# path (`c:/Users/...` on Windows, no leading slash) but matches it against the
# document URL with only the scheme stripped (`/c:/Users/...`), so on Windows a
# relative glob never matches and the editor reports no schema. A leading `/`
# makes the glob count as absolute, which skips the join; `/**/` then matches the
# URL path on Windows and macOS alike.
#
# This repository's own manifests (`Cargo.toml`, `deny.toml`, `config.toml`) sit
# outside every glob, so no rule is needed to keep a preset schema off them.
# `schema.path` is relative to this file, so it resolves the same in a clone at
# any path.
";

/// Every **distinct** parameter declaration in the engine, each with the
/// `definitions` key the schema refs it by.
///
/// Deduplicated by declaration rather than by name, which is the whole point:
/// 29 of the 208 parameter names are declared differently by different systems —
/// `d` is a curve's sampling step and an attractor's fourth coefficient, with
/// different sentences and different defaults — so one entry per *name* would
/// show the wrong default and the wrong prose on hover for whichever system lost.
///
/// The key is `param.<name>` for a name with a single declaration, and
/// `param.<name>.<owner>` for each variant of one declared more than once, where
/// `owner` is the first roster to declare that variant. Readable rather than
/// numbered, and stable because both rosters are `const` arrays walked in
/// declaration order. A key that would collide anyway takes a numeric suffix, so
/// two variants can never silently land on one definition.
fn param_definitions() -> Vec<(String, &'static ParamSpec)> {
    let labelled = SystemKind::ALL
        .iter()
        .map(|kind| (kind.as_str(), kind.param_specs()))
        .chain(
            STAGE_LABELS
                .iter()
                .copied()
                .zip(GLOBAL_PARAMS.iter().copied()),
        );

    // (owner label, spec), one per distinct declaration, in walk order.
    let mut distinct: Vec<(&'static str, &'static ParamSpec)> = Vec::new();
    for (label, specs) in labelled {
        for spec in specs {
            if !distinct
                .iter()
                .any(|(_, seen)| same_declaration(seen, spec))
            {
                distinct.push((label, spec));
            }
        }
    }

    let mut out: Vec<(String, &'static ParamSpec)> = Vec::with_capacity(distinct.len());
    for (label, spec) in &distinct {
        let ambiguous = distinct
            .iter()
            .filter(|(_, other)| other.name == spec.name)
            .count()
            > 1;
        let mut key = if ambiguous {
            format!("param.{}.{label}", spec.name)
        } else {
            format!("param.{}", spec.name)
        };
        // Belt and braces: a key already taken would otherwise overwrite its
        // twin in the JSON object, which is a silently wrong hover rather than a
        // failure.
        let mut next = 2;
        while out.iter().any(|(taken, _)| *taken == key) {
            key = format!("param.{}.{label}.{next}", spec.name);
            next += 1;
        }
        out.push((key, spec));
    }
    out
}

/// Whether two specs are the same declaration — the same name saying the same
/// thing about the same default.
///
/// Bit equality on the floats rather than `==`, so the comparison is total: a
/// `NaN` default would otherwise never equal itself and would be written once per
/// roster that declares it.
fn same_declaration(a: &ParamSpec, b: &ParamSpec) -> bool {
    let bits = |range: Option<[f32; 2]>| range.map(|[lo, hi]| (lo.to_bits(), hi.to_bits()));
    a.name == b.name
        && a.doc == b.doc
        && a.default.to_bits() == b.default.to_bits()
        && bits(a.range) == bits(b.range)
        && a.kind.as_str() == b.kind.as_str()
}

/// The definition key for `spec`, found by declaration.
fn definition_key<'k>(
    definitions: &'k [(String, &'static ParamSpec)],
    spec: &ParamSpec,
) -> Option<&'k str> {
    definitions
        .iter()
        .find(|(_, candidate)| same_declaration(candidate, spec))
        .map(|(key, _)| key.as_str())
}

/// One parameter declaration as a `definitions` entry: a string, what it does,
/// and its default, range and kind as hover prose.
fn push_param_definition(out: &mut String, depth: usize, key: &str, spec: &ParamSpec) {
    let pad = "  ".repeat(depth);
    out.push_str(&format!("{pad}{}: {{\n", json_string(key)));
    out.push_str(&format!("{pad}  \"type\": \"string\",\n"));
    out.push_str(&format!(
        "{pad}  \"description\": {},\n",
        json_string(spec.doc)
    ));
    out.push_str(&format!(
        "{pad}  \"markdownDescription\": {}\n",
        json_string(&markdown_for(spec))
    ));
    out.push_str(&format!("{pad}}}"));
}

/// Which `[params]` surface a per-system case is about.
///
/// The two accept different sets, which is why the distinction exists in the
/// schema at all: [`is_known_param`] admits the compositing stages at the root,
/// and the layer surface admits only the layer system's own roster — the rule the
/// loader already enforces by warning.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ParamSurface {
    /// The document's own `[params]`.
    Root,
    /// A `[layer]`'s `[layer.params]`.
    Layer,
}

/// Every parameter one surface of `kind` accepts, in the order an author meets
/// them: the system's own roster first, then the engine stages'.
///
/// **The membership test this renders is the loader's**, read off the same two
/// rosters `is_known_param` and `compile_bindings` consult — so a key the editor
/// underlines is exactly a key the loader would warn about, and a key it accepts
/// is one the loader reads. `core/tests/preset_schema.rs` validates against this
/// same function, which is what makes the committed JSON and the corpus check one
/// model rather than two.
///
/// A name declared by the system and again by a stage appears **once**, at its
/// first occurrence — the order `kind_of_param` resolves in.
pub fn params_of(kind: SystemKind, surface: ParamSurface) -> Vec<&'static ParamSpec> {
    let mut out: Vec<&'static ParamSpec> = Vec::new();
    let rosters = kind.param_specs().iter().chain(
        GLOBAL_PARAMS
            .iter()
            .filter(|_| surface == ParamSurface::Root)
            .flat_map(|stage| stage.iter()),
    );
    for spec in rosters {
        if !out.iter().any(|seen| seen.name == spec.name) {
            out.push(spec);
        }
    }
    out
}

/// One `if`/`then` pair: when `system` is `kind`, that surface's `[params]` keys
/// are this system's.
///
/// The `if` **requires** `system` as well as matching it, because a subschema for
/// an absent key is vacuously satisfied — without the `required` every case would
/// fire on a document that declares no system at all, and the parameter set of
/// whichever fired last would be the one enforced.
fn push_system_case(
    out: &mut String,
    depth: usize,
    kind: SystemKind,
    surface: ParamSurface,
    definitions: &[(String, &'static ParamSpec)],
) {
    let pad = "  ".repeat(depth);
    let params = params_of(kind, surface);
    let name = json_string(kind.as_str());
    out.push_str(&format!("{pad}{{\n"));
    match surface {
        ParamSurface::Root => {
            out.push_str(&format!(
                "{pad}  \"if\": {{ \"required\": [\"system\"], \"properties\": {{ \"system\": {{ \"const\": {name} }} }} }},\n"
            ));
            out.push_str(&format!(
                "{pad}  \"then\": {{ \"properties\": {{ \"params\": "
            ));
            push_params_object(out, depth + 2, &params, definitions);
            out.push_str(&format!(" }} }}\n{pad}}}"));
        }
        ParamSurface::Layer => {
            out.push_str(&format!(
                "{pad}  \"if\": {{ \"required\": [\"layer\"], \"properties\": {{ \"layer\": {{ \"required\": [\"system\"], \"properties\": {{ \"system\": {{ \"const\": {name} }} }} }} }} }},\n"
            ));
            out.push_str(&format!(
                "{pad}  \"then\": {{ \"properties\": {{ \"layer\": {{ \"properties\": {{ \"params\": "
            ));
            push_params_object(out, depth + 2, &params, definitions);
            out.push_str(&format!(" }} }} }} }}\n{pad}}}"));
        }
    }
}

/// The `[params]` object for one system: every accepted name as a string
/// property, and nothing else admitted.
fn push_params_object(
    out: &mut String,
    depth: usize,
    params: &[&'static ParamSpec],
    definitions: &[(String, &'static ParamSpec)],
) {
    out.push_str("{\n");
    push_params_body(out, depth + 1, params, definitions);
    out.push_str(" }");
}

/// [`push_params_object`] without its braces: the lines a per-system file writes
/// inside its `params` property, beside that property's description.
fn push_params_body(
    out: &mut String,
    depth: usize,
    params: &[&'static ParamSpec],
    definitions: &[(String, &'static ParamSpec)],
) {
    let pad = "  ".repeat(depth);
    out.push_str(&format!("{pad}\"type\": \"object\",\n"));
    out.push_str(&format!("{pad}\"additionalProperties\": false,\n"));
    out.push_str(&format!("{pad}\"properties\": {{\n"));
    let mut first = true;
    for spec in params {
        // Every spec reached here came out of a roster `param_definitions` also
        // walked, so the lookup cannot miss. A miss would mean the two walks had
        // stopped reading the same declarations, and writing the parameter inline
        // instead would hide that behind a hover that merely looks right.
        let key = definition_key(definitions, spec)
            .expect("every parameter roster is walked by param_definitions");
        push_separator(out, &mut first);
        out.push_str(&format!(
            "{pad}  {}: {{ \"$ref\": \"#/definitions/{key}\" }}",
            json_string(spec.name)
        ));
    }
    out.push_str(&format!("\n{pad}}}"));
}

/// The hover text for one parameter: what it does, then its default, its range
/// and its kind.
///
/// The range is **prose here and a bound nowhere**, for the reason
/// [`json_schema`] gives: a preset may legitimately set a value outside it.
fn markdown_for(spec: &ParamSpec) -> String {
    let mut text = format!("{}\n\n", spec.doc);
    text.push_str(&format!("- default `{:?}`\n", spec.default));
    match spec.range {
        Some([lo, hi]) => text.push_str(&format!(
            "- typical range `{lo:?}` to `{hi:?}` — a guide, not a bound: the engine \
             neither clamps to it nor rejects a value outside it\n"
        )),
        None => text.push_str("- no typical range declared\n"),
    }
    text.push_str(&format!("- {} parameter\n", spec.kind.as_str()));
    // No system is named: one declaration is shared by every system that makes
    // it, so a sentence naming one of them would be wrong on the others.
    text.push_str("\nThe value is an expression, always quoted.");
    text
}

/// One structural table as a `definitions` entry.
fn push_definition(out: &mut String, depth: usize, table: &TableDesc) {
    let pad = "  ".repeat(depth);
    out.push_str(&format!("{pad}{}: {{\n", json_string(table.name)));
    out.push_str(&format!(
        "{pad}  \"description\": {},\n",
        json_string(table.doc)
    ));
    out.push_str(&format!("{pad}  \"type\": \"object\",\n"));
    // Authoritative: `core/src/preset/schema/tests.rs` reads the field roster
    // serde derived for this table and asserts it is exactly these keys, so a key
    // the editor refuses is a key the loader has no field for.
    out.push_str(&format!("{pad}  \"additionalProperties\": false,\n"));
    out.push_str(&format!("{pad}  \"properties\": {{\n"));
    push_table_properties(out, depth + 2, table);
    out.push_str(&format!("\n{pad}  }}\n{pad}}}"));
}

/// Every key of `table` as a JSON Schema property.
fn push_table_properties(out: &mut String, depth: usize, table: &TableDesc) {
    push_table_properties_with(out, depth, table, |_, _, _| false);
}

/// [`push_table_properties`], with `body` offered each key's type half first.
///
/// `body` returns `true` when it wrote that half itself, and the key's
/// [`KeyKind`] is then not rendered. Everything else about the property — its
/// name, its description, its default — is written the same way either way, so a
/// per-system file's `system` and `params` read on hover exactly as the generic
/// file's do.
fn push_table_properties_with(
    out: &mut String,
    depth: usize,
    table: &TableDesc,
    mut body: impl FnMut(&mut String, usize, &KeyDesc) -> bool,
) {
    let pad = "  ".repeat(depth);
    let mut first = true;
    for key in table.keys {
        push_separator(out, &mut first);
        out.push_str(&format!("{pad}{}: {{\n", json_string(key.name)));
        if !body(out, depth + 1, key) {
            push_kind_body(out, depth + 1, &key.kind);
        }
        out.push_str(&format!(
            ",\n{pad}  \"description\": {}",
            json_string(key.doc)
        ));
        // An empty `default` means absence denotes something other than a value —
        // a required key, or one whose absence turns a feature off — so there is
        // no default to quote.
        if !key.default.is_empty() {
            out.push_str(&format!(
                ",\n{pad}  \"markdownDescription\": {}",
                json_string(&format!("{}\n\nDefault `{}`.", key.doc, key.default))
            ));
        }
        out.push_str(&format!("\n{pad}}}"));
    }
}

/// The type half of one key's schema — everything but its description.
///
/// The mapping is [`KeyKind`]'s and nothing here invents a roster: a
/// [`KeyKind::Roster`] asks the type that owns the closed set, so an `enum` in the
/// schema is the same list the loader accepts.
fn push_kind_body(out: &mut String, depth: usize, kind: &KeyKind) {
    let pad = "  ".repeat(depth);
    match kind {
        KeyKind::Bool => out.push_str(&format!("{pad}\"type\": \"boolean\"")),
        KeyKind::Int => out.push_str(&format!("{pad}\"type\": \"integer\"")),
        KeyKind::Float => out.push_str(&format!("{pad}\"type\": \"number\"")),
        // Free text and an expression are both strings to an editor; the
        // difference is what the loader does with them, not what TOML may hold.
        KeyKind::Text | KeyKind::Expr => out.push_str(&format!("{pad}\"type\": \"string\"")),
        KeyKind::Roster(roster) => {
            out.push_str(&format!("{pad}\"type\": \"string\",\n{pad}\"enum\": ["));
            let mut first = true;
            for value in roster.values() {
                if !first {
                    out.push_str(", ");
                }
                first = false;
                out.push_str(&json_string(value));
            }
            out.push(']');
        }
        // Seconds, or the `{ attack, release }` pair ADR-0035 widened it to.
        KeyKind::Easing => out.push_str(&format!(
            "{pad}\"anyOf\": [\n{pad}  {{ \"type\": \"number\" }},\n{pad}  {{ \"type\": \"object\", \"additionalProperties\": false, \"properties\": {{ \"attack\": {{ \"type\": \"number\" }}, \"release\": {{ \"type\": \"number\" }} }} }}\n{pad}]"
        )),
        // A number, or the word `random` (ADR-0051).
        KeyKind::Seed => out.push_str(&format!(
            "{pad}\"anyOf\": [\n{pad}  {{ \"type\": \"number\" }},\n{pad}  {{ \"type\": \"string\", \"enum\": [\"random\"] }}\n{pad}]"
        )),
        // A named edge, or a period in seconds — which the loader accepts bare or
        // quoted. Written as an unconstrained string beside the number rather than
        // as an `enum` of the two words, because an `enum` would underline the
        // legal `"2.0"`; the two words are carried as `examples`, which an editor
        // offers and does not enforce.
        KeyKind::Hold => out.push_str(&format!(
            "{pad}\"anyOf\": [\n{pad}  {{ \"type\": \"number\" }},\n{pad}  {{ \"type\": \"string\", \"examples\": [\"beat\", \"bar\"] }}\n{pad}]"
        )),
        // `#rrggbb` (or a bare `rrggbb`), or an `[r, g, b]` array in 0..=1. The
        // string carries no `pattern`: the loader's message about a malformed hex
        // names the offending value, and a pattern would underline a half-typed
        // colour on every keystroke.
        KeyKind::Colour => out.push_str(&format!(
            "{pad}\"anyOf\": [\n{pad}  {{ \"type\": \"string\" }},\n{pad}  {{ \"type\": \"array\", \"items\": {{ \"type\": \"number\" }} }}\n{pad}]"
        )),
        // Author-chosen keys, so no `additionalProperties: false`: the engine is
        // authoritative for the value's shape and the preset for the key set.
        KeyKind::Map(of) => {
            out.push_str(&format!(
                "{pad}\"type\": \"object\",\n{pad}\"additionalProperties\": {{\n"
            ));
            push_kind_body(out, depth + 1, of);
            out.push_str(&format!("\n{pad}}}"));
        }
        KeyKind::List(of) => {
            out.push_str(&format!("{pad}\"type\": \"array\",\n{pad}\"items\": {{\n"));
            push_kind_body(out, depth + 1, of);
            out.push_str(&format!("\n{pad}}}"));
        }
        KeyKind::Table(name) => {
            out.push_str(&format!("{pad}\"$ref\": \"#/definitions/{name}\""));
        }
    }
}

/// Write `",\n"` before every element but the first.
fn push_separator(out: &mut String, first: &mut bool) {
    if !*first {
        out.push_str(",\n");
    }
    *first = false;
}

/// `text` as a JSON string literal, quotes included.
fn json_string(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    push_string(&mut out, text);
    out
}
