//! `presets/preset.schema.json` is what the engine declares, and no shipped
//! preset violates it (ADR-0190).
//!
//! Three properties, and the second is the one this phase most has to get right:
//!
//!   1. **The committed file is current.** It is generated from
//!      [`export::json_schema`], so a hand edit or a stale checkout fails here.
//!      `RLX_UPDATE_PRESET_SCHEMA=1` rewrites it — the same shape
//!      `the_parameter_reference_block_is_current` uses for `presets/README.md`.
//!   2. **Nothing in the corpus violates it.** An editor squiggle on a correct
//!      shipped preset is the failure that would make the whole feature a
//!      nuisance, so every tracked `*.toml` under `presets/` and `docs/examples/`
//!      is validated — 127 files as this is written.
//!   3. **A real mistake is caught, naming the key.** A misspelled `[params]`
//!      key and an off-roster `[spectrum] layout`.
//!
//! ## One model, two renderings
//!
//! The validator below reads **no JSON**. It walks the same declarations
//! `json_schema` renders from — `export::table`, `KeyKind`, `Roster::accepts`,
//! `export::params_of` — and applies the rules that function writes into the
//! document. So the two cannot disagree about what a preset may contain without
//! one of them being edited, and no JSON parser enters the workspace (NFR
//! section 4).
//!
//! **What it therefore does not prove** is that Taplo reads draft-07 `if`/`then`
//! the way this does. Nothing in this repository can prove that; Plan 0169's
//! `human` phase is the evidence, and it is why that phase exists.
//!
//! GPU-free: everything here is a parse and a string comparison.

use std::path::{Path, PathBuf};

use rlx_core::preset::export::{self, ParamSurface};
use rlx_core::preset::{KeyKind, SystemKind};
use toml::Spanned;
use toml::de::{DeTable, DeValue};

/// The committed schema, resolved from the manifest rather than the cwd — a
/// test's working directory is not a contract.
fn schema_path() -> PathBuf {
    repo_root().join("presets/preset.schema.json")
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("core has a workspace-root parent")
        .to_path_buf()
}

/// **The committed file is what the engine renders.**
///
/// Regenerate with `RLX_UPDATE_PRESET_SCHEMA=1 cargo nextest run -p rlx-core
/// the_committed_schema_is_current`, which is the same env-var shape the
/// parameter reference in `presets/README.md` uses (ADR-0170).
#[test]
fn the_committed_schema_is_current() {
    let path = schema_path();
    let generated = export::json_schema();

    if std::env::var_os("RLX_UPDATE_PRESET_SCHEMA").is_some() {
        std::fs::write(&path, &generated)
            .unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
        return;
    }

    let committed =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    // Compared without the line endings, so a clone that checked the file out as
    // CRLF does not fail a test about the schema's content.
    assert_eq!(
        committed.replace("\r\n", "\n"),
        generated,
        "presets/preset.schema.json is not what the engine declares.\n\
         Re-run with RLX_UPDATE_PRESET_SCHEMA=1 to regenerate it. The file is \
         generated from the ParamSpec and TableDesc declarations; a hand edit \
         inside it is what this fails on."
    );
}

/// **The `.json` is not embedded.** `core/build.rs` globs `presets/` and filters
/// on the extension, so the schema file is held out by construction — but the
/// embedded set is what the foobar path renders with no preset directory, and a
/// stray entry there would ship a JSON document as a preset.
#[test]
fn the_schema_file_is_not_embedded_as_a_preset() {
    let names: Vec<&str> = rlx_core::preset::EMBEDDED
        .iter()
        .map(|(name, _)| *name)
        .collect();
    assert!(
        names.iter().all(|name| name.ends_with(".toml")),
        "the embedded set carries something that is not a preset: {:?}",
        names
            .iter()
            .filter(|name| !name.ends_with(".toml"))
            .collect::<Vec<_>>()
    );
    let on_disk = std::fs::read_dir(repo_root().join("presets"))
        .expect("read presets/")
        .filter_map(Result::ok)
        .filter(|entry| {
            entry.path().is_file() && entry.path().extension().is_some_and(|ext| ext == "toml")
        })
        .count();
    assert_eq!(
        names.len(),
        on_disk,
        "the embedded set and `presets/*.toml` disagree on how many presets there are"
    );
}

/// **No preset the repository tracks violates the schema.**
///
/// The failure this phase most has to avoid: an editor underlining a correct
/// shipped preset makes the schema worse than none at all, because an author
/// learns to ignore it.
#[test]
fn no_tracked_preset_violates_the_schema() {
    let mut findings: Vec<String> = Vec::new();
    let files = corpus();
    assert!(
        files.len() >= 100,
        "the corpus walk found only {} files, which means it stopped reading the \
         tree rather than that the library shrank",
        files.len()
    );
    for file in &files {
        let src = std::fs::read_to_string(file)
            .unwrap_or_else(|e| panic!("read {}: {e}", file.display()));
        let doc = DeTable::parse(&src)
            .unwrap_or_else(|e| panic!("{} is not valid TOML: {e}", file.display()));
        for violation in violations(doc.get_ref()) {
            findings.push(format!("{}: {violation}", file.display()));
        }
    }
    assert!(
        findings.is_empty(),
        "{} violation(s) against the engine's own declarations:\n{}",
        findings.len(),
        findings.join("\n")
    );
}

/// **A misspelled `[params]` key is a violation naming the key.** The class
/// ADR-0020 forgives at load: the binding is kept, nothing reads it, and the
/// editor underline is what makes it visible where it was typed.
#[test]
fn a_misspelled_parameter_is_a_violation_naming_the_key() {
    let found = violations_of("system = \"fragment_field\"\n\n[params]\nglwo = \"1.0\"\n");
    assert!(
        found.iter().any(|v| v.contains("glwo")),
        "the misspelled key is not named: {found:?}"
    );
    // The near-miss, and the half that matters: `glow` is what `glwo` was
    // reaching for, so a rule firing on both would be a rule about nothing.
    let clean = violations_of("system = \"fragment_field\"\n\n[params]\nglow = \"1.0\"\n");
    assert!(
        clean.is_empty(),
        "a correctly spelled parameter was convicted: {clean:?}"
    );
}

/// **An off-roster value is a violation naming the key.** `[spectrum] layout` is
/// a closed set the engine owns, so the schema's `enum` is that set and not a
/// copy of it.
#[test]
fn an_off_roster_layout_is_a_violation_naming_the_key() {
    let found = violations_of("system = \"spectrum\"\n\n[spectrum]\nlayout = \"helix\"\n");
    assert!(
        found.iter().any(|v| v.contains("layout")),
        "the off-roster key is not named: {found:?}"
    );
    assert!(
        found.iter().any(|v| v.contains("helix")),
        "the violation does not quote what was written: {found:?}"
    );
    let clean = violations_of("system = \"spectrum\"\n\n[spectrum]\nlayout = \"bars\"\n");
    assert!(
        clean.is_empty(),
        "an on-roster layout was convicted: {clean:?}"
    );
}

/// **A layer binds its own scene's parameters and never the compositing
/// stages.** That asymmetry is the loader's rule, and the schema keys the two
/// `[params]` surfaces separately to carry it — so this is the case that would
/// fail if both surfaces were rendered from one roster.
#[test]
fn the_two_param_surfaces_differ_on_a_compositing_parameter() {
    let stage_param = "bloom_amount";
    let at_root = violations_of(&format!(
        "system = \"swarm\"\n\n[params]\n{stage_param} = \"1\"\n"
    ));
    assert!(
        at_root.is_empty(),
        "a compositing parameter was refused at the top level, where the loader \
         reads it: {at_root:?}"
    );
    let in_layer = violations_of(&format!(
        "system = \"swarm\"\n\n[layer]\nsystem = \"swarm\"\n\n[layer.params]\n{stage_param} = \"1\"\n"
    ));
    assert!(
        in_layer.iter().any(|v| v.contains(stage_param)),
        "a compositing parameter was accepted inside a layer, where the loader \
         warns that nothing reads it: {in_layer:?}"
    );
}

// ---------------------------------------------------------------------------
// The validator: the schema's rules, applied to a parsed document
// ---------------------------------------------------------------------------

/// The violations in `src`, for the fixture cases above.
fn violations_of(src: &str) -> Vec<String> {
    let doc = DeTable::parse(src).expect("the fixture is valid TOML");
    violations(doc.get_ref())
}

/// Every way `doc` departs from what the engine declares.
///
/// The same rules [`export::json_schema`] writes, in the same order: the root's
/// key set is closed, each structural table's is closed, each value matches its
/// [`KeyKind`], and each `[params]` surface admits exactly the names that
/// surface's rosters declare.
fn violations(doc: &DeTable<'_>) -> Vec<String> {
    let mut out = Vec::new();
    let root = export::table("preset").expect("the root table is declared");
    let system = string_at(doc, "system").and_then(SystemKind::from_name);

    check_table(doc, root, "", &mut out);

    // The two parameter surfaces, each against its own roster. Skipped when
    // `system` names nothing: that is already a load error, and the schema's
    // `if` does not fire either.
    if let Some(kind) = system
        && let Some(params) = table_at(doc, "params")
    {
        check_params(params, kind, ParamSurface::Root, "params", &mut out);
    }
    if let Some(layer) = table_at(doc, "layer")
        && let Some(kind) = string_at(layer, "system").and_then(SystemKind::from_name)
        && let Some(params) = table_at(layer, "params")
    {
        check_params(params, kind, ParamSurface::Layer, "layer.params", &mut out);
    }
    out
}

/// One structural table: its key set is closed, and each value matches its kind.
fn check_table(table: &DeTable<'_>, desc: &export::TableDesc, at: &str, out: &mut Vec<String>) {
    for (key, value) in table.iter() {
        let name = key.get_ref().as_ref();
        let path = join(at, name);
        match desc.keys.iter().find(|k| k.name == name) {
            None => out.push(format!(
                "`{path}` is not a key of the `{}` table",
                desc.name
            )),
            // `params` is checked against the per-system roster instead, by the
            // caller: its declared kind is an open map of expressions, which is
            // what the schema says before an `if`/`then` narrows it.
            Some(_) if path == "params" || path == "layer.params" => {}
            Some(declared) => check_value(value, &declared.kind, &path, out),
        }
    }
}

/// One value against one [`KeyKind`].
fn check_value(value: &Spanned<DeValue<'_>>, kind: &KeyKind, at: &str, out: &mut Vec<String>) {
    let refuse = |out: &mut Vec<String>, wanted: &str| {
        out.push(format!(
            "`{at}` is {} where the engine declares {wanted}",
            value.get_ref().type_str()
        ));
    };
    match kind {
        KeyKind::Bool => {
            if !value.get_ref().is_bool() {
                refuse(out, "a boolean");
            }
        }
        KeyKind::Int => {
            if !value.get_ref().is_integer() {
                refuse(out, "a whole number");
            }
        }
        // TOML distinguishes `2` from `2.0` and an author writing a whole value
        // reaches for the integer, so both are a number here — which is what
        // JSON Schema's `"number"` accepts too.
        KeyKind::Float => {
            if !is_number(value.get_ref()) {
                refuse(out, "a number");
            }
        }
        KeyKind::Text | KeyKind::Expr => {
            if !value.get_ref().is_str() {
                refuse(out, "a string");
            }
        }
        KeyKind::Roster(roster) => match value.get_ref().as_str() {
            None => refuse(out, "one of a closed set of names"),
            Some(name) if !roster.accepts(name) => out.push(format!(
                "`{at}` is `{name}`, which is not one of: {}",
                roster.values().join(", ")
            )),
            Some(_) => {}
        },
        KeyKind::Easing => match value.get_ref() {
            v if is_number(v) => {}
            DeValue::Table(pair) => {
                for (key, inner) in pair.iter() {
                    let name = key.get_ref().as_ref();
                    if name != "attack" && name != "release" {
                        out.push(format!("`{}` is not `attack` or `release`", join(at, name)));
                    } else if !is_number(inner.get_ref()) {
                        out.push(format!("`{}` is not a number of seconds", join(at, name)));
                    }
                }
            }
            _ => refuse(out, "seconds, or an { attack, release } pair"),
        },
        KeyKind::Seed => match value.get_ref() {
            v if is_number(v) => {}
            DeValue::String(word) if word.as_ref() == "random" => {}
            _ => refuse(out, "a number, or the word `random`"),
        },
        // A named edge or a period, and the period may be written bare or quoted
        // — so any string is admitted here, exactly as the schema admits one.
        KeyKind::Hold => {
            if !is_number(value.get_ref()) && !value.get_ref().is_str() {
                refuse(out, "a named edge, or a period in seconds");
            }
        }
        KeyKind::Colour => match value.get_ref() {
            DeValue::String(_) => {}
            DeValue::Array(channels) => {
                for channel in channels.iter() {
                    if !is_number(channel.get_ref()) {
                        out.push(format!("`{at}` has a channel that is not a number"));
                    }
                }
            }
            _ => refuse(out, "a `#rrggbb` string, or an [r, g, b] array"),
        },
        // Author-chosen keys: the value's shape is the engine's business and the
        // key set is the preset's, which is why the schema sets no
        // `additionalProperties: false` here either.
        KeyKind::Map(of) => match value.get_ref().as_table() {
            None => refuse(out, "a table"),
            Some(inner) => {
                for (key, entry) in inner.iter() {
                    check_value(entry, of, &join(at, key.get_ref().as_ref()), out);
                }
            }
        },
        KeyKind::List(of) => match value.get_ref().as_array() {
            None => refuse(out, "an array"),
            Some(items) => {
                for (i, item) in items.iter().enumerate() {
                    check_value(item, of, &format!("{at}[{i}]"), out);
                }
            }
        },
        KeyKind::Table(name) => match value.get_ref().as_table() {
            None => refuse(out, "a table"),
            Some(inner) => match export::table(name) {
                Some(desc) => check_table(inner, desc, at, out),
                None => out.push(format!("`{at}` refers to an undeclared table `{name}`")),
            },
        },
    }
}

/// One `[params]` surface: every key is a parameter that surface accepts, and
/// every value is a string.
fn check_params(
    params: &DeTable<'_>,
    kind: SystemKind,
    surface: ParamSurface,
    at: &str,
    out: &mut Vec<String>,
) {
    let accepted = export::params_of(kind, surface);
    for (key, value) in params.iter() {
        let name = key.get_ref().as_ref();
        let path = join(at, name);
        if !accepted.iter().any(|spec| spec.name == name) {
            out.push(format!(
                "`{path}` is not a parameter `{}` accepts{}",
                kind.as_str(),
                match surface {
                    ParamSurface::Root => "",
                    ParamSurface::Layer => " inside a layer",
                }
            ));
        }
        // A binding is always an expression string; `glow = 1.0` is a load error
        // rather than a shorthand for one.
        if !value.get_ref().is_str() {
            out.push(format!(
                "`{path}` is {} where a binding is an expression string",
                value.get_ref().type_str()
            ));
        }
    }
}

/// Whether `value` is an integer or a float — the two spellings of a number.
fn is_number(value: &DeValue<'_>) -> bool {
    value.is_integer() || value.is_float()
}

/// The string at `key`, or `None` when it is absent or not a string.
fn string_at<'t>(table: &'t DeTable<'_>, key: &str) -> Option<&'t str> {
    table
        .iter()
        .find(|(name, _)| name.get_ref().as_ref() == key)
        .and_then(|(_, value)| value.get_ref().as_str())
}

/// The table at `key`, or `None` when it is absent or not a table.
fn table_at<'t, 'i>(table: &'t DeTable<'i>, key: &str) -> Option<&'t DeTable<'i>> {
    table
        .iter()
        .find(|(name, _)| name.get_ref().as_ref() == key)
        .and_then(|(_, value)| value.get_ref().as_table())
}

/// `a.b`, or `b` at the root.
fn join(at: &str, name: &str) -> String {
    if at.is_empty() {
        name.to_owned()
    } else {
        format!("{at}.{name}")
    }
}

/// Every tracked preset: `presets/` flat, `docs/examples/` recursively — the
/// same two trees the schema is associated with in `.taplo.toml`.
fn corpus() -> Vec<PathBuf> {
    let root = repo_root();
    let mut files = Vec::new();
    for dir in ["presets", "presets/proposed", "presets/pending"] {
        let path = root.join(dir);
        if let Ok(entries) = std::fs::read_dir(&path) {
            for entry in entries.filter_map(Result::ok) {
                let file = entry.path();
                if file.is_file() && file.extension().is_some_and(|ext| ext == "toml") {
                    files.push(file);
                }
            }
        }
    }
    collect(&root.join("docs/examples"), &mut files);
    files.sort();
    files
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = std::fs::read_dir(dir).unwrap_or_else(|e| panic!("read {}: {e}", dir.display()));
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "toml") {
            out.push(path);
        }
    }
}
