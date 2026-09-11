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
