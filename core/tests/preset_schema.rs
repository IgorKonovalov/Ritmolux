//! The generated editor files are what the engine declares, and no tracked
//! preset violates the schema an editor applies to it (ADR-0190).
//!
//! The files are sixteen: `presets/preset.schema.json`, the generic schema that
//! validates every system and completes none; one self-contained schema per
//! system under `presets/schema/`, which is what completes and hovers; and
//! `.taplo.toml`, which hands a library file its family's schema by filename.
//!
//! Four properties, and the second is the one an editor user most feels:
//!
//!   1. **The committed files are current.** Each is rendered by
//!      [`export::editor_files`], so a hand edit, a stale checkout, or a schema
//!      in `presets/schema/` that no system renders fails here. `RLX_UPDATE_PRESET_SCHEMA=1`
//!      rewrites all of them — the same shape
//!      `the_parameter_reference_block_is_current` uses for `presets/README.md`.
//!   2. **Nothing in the corpus violates the schema it gets.** Every library file
//!      against its family's schema, and every tracked `*.toml` under `presets/`
//!      and `docs/examples/` against the generic one. An editor squiggle on a
//!      correct shipped preset would teach an author to ignore the editor.
//!   3. **A real mistake is caught, naming the key.** A misspelled `[params]`
//!      key, an off-roster `[spectrum] layout`, and — against one system's own
//!      file — another system's parameter and another system's name.
//!   4. **The build still embeds only presets.**
//!
//! ## One model, two renderings
//!
//! The validator below reads **no JSON**. It walks the same declarations the
//! schemas are rendered from — `export::table`, `KeyKind`, `Roster::accepts`,
//! `export::params_of` — and applies the rules those renderers write into the
//! documents. So the two cannot disagree about what a preset may contain without
//! one of them being edited, and no JSON parser enters the workspace (NFR
//! section 4).
//!
//! **What it therefore does not prove** is that Taplo reads the files the way
//! this does — which schema a glob selects, what it completes. Nothing in CI can
//! run the extension; Plan 0169's `human` phases are the evidence for that.
//!
//! GPU-free: everything here is a parse and a string comparison.

use std::path::{Path, PathBuf};

use rlx_core::preset::export::{self, ParamSurface};
use rlx_core::preset::{KeyKind, SystemKind};
use toml::Spanned;
use toml::de::{DeTable, DeValue};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("core has a workspace-root parent")
        .to_path_buf()
}

/// **Every committed editor file is what the engine renders**, and
/// `presets/schema/` holds nothing else.
///
/// Regenerate with `RLX_UPDATE_PRESET_SCHEMA=1 cargo nextest run -p rlx-core
/// the_generated_editor_files_are_current`, which is the same env-var shape the
/// parameter reference in `presets/README.md` uses (ADR-0170).
#[test]
fn the_generated_editor_files_are_current() {
    let root = repo_root();
    if std::env::var_os("RLX_UPDATE_PRESET_SCHEMA").is_some() {
        regenerate(&root);
    }
    let found = drift(&root);
    assert!(
        found.is_empty(),
        "the generated editor files are not what the engine declares:\n{}\n\
         Re-run with RLX_UPDATE_PRESET_SCHEMA=1 to regenerate them. They are \
         rendered from the ParamSpec, TableDesc and SystemKind declarations; a \
         hand edit inside one is what this fails on.",
        found.join("\n")
    );
}

/// Where the player's `--schema` document is committed, relative to the repo root.
///
/// Not under `presets/schema/`: that directory holds the editor's JSON Schemas and
/// its drift check refuses a file no system renders. This is the studio's panel
/// feed, read by its schema walks when no player is built.
const PLAYER_SCHEMA: &str = "docs/specs/player-schema.json";

/// **The committed player schema is what `ritmolux --schema` prints** — the
/// document [`export::document`] renders, byte for byte bar the line endings,
/// with the trailing newline `println!` adds.
///
/// Regenerates under the same `RLX_UPDATE_PRESET_SCHEMA=1` switch as the editor
/// files, so `RLX_UPDATE_PRESET_SCHEMA=1 cargo nextest run -p rlx-core --test
/// preset_schema` rewrites every file derived from the export.
#[test]
fn the_player_schema_snapshot_is_current() {
    let path = repo_root().join(PLAYER_SCHEMA);
    let generated = format!("{}\n", export::document());
    if std::env::var_os("RLX_UPDATE_PRESET_SCHEMA").is_some() {
        std::fs::write(&path, &generated)
            .unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
    }
    let committed = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{PLAYER_SCHEMA} cannot be read: {e}"));
    // Compared without the line endings, so a clone that checked the file out
    // as CRLF does not fail a test about its content.
    assert!(
        committed.replace("\r\n", "\n") == generated,
        "{PLAYER_SCHEMA} is not what `ritmolux --schema` prints. Regenerate it with\n\
         \n    RLX_UPDATE_PRESET_SCHEMA=1 cargo nextest run -p rlx-core --test preset_schema\n\n\
         It is rendered by rlx_core::preset::export::document(); a hand edit is what \
         this fails on."
    );
}

/// **The drift check names each of the sixteen files, and an extra schema, and
/// the regenerate command repairs every case.** Run against a scratch copy, so a
/// check that had silently stopped comparing one of the files cannot pass
/// `the_generated_editor_files_are_current` by agreeing with nothing.
#[test]
fn the_drift_check_names_every_stale_file_and_regeneration_repairs_it() {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("preset-schema-drift")
        .join(std::process::id().to_string());
    let _ = std::fs::remove_dir_all(&root);
    regenerate(&root);
    assert_eq!(drift(&root), Vec::<String>::new(), "a fresh render drifts");

    let files = export::editor_files();
    assert_eq!(
        files.len(),
        2 + SystemKind::VARIANT_COUNT,
        "the generic schema, one per system, and .taplo.toml"
    );
    for (path, _) in &files {
        let on_disk = root.join(path);
        let mut stale = std::fs::read_to_string(&on_disk).expect("read a rendered file");
        stale.push('\n');
        std::fs::write(&on_disk, stale).expect("perturb a rendered file");
        let found = drift(&root);
        assert!(
            found.len() == 1 && found[0].contains(path.as_str()),
            "a stale `{path}` is not the one thing reported: {found:?}"
        );
        regenerate(&root);
        assert!(
            drift(&root).is_empty(),
            "regenerating did not repair `{path}`"
        );

        std::fs::remove_file(&on_disk).expect("remove a rendered file");
        assert!(
            drift(&root).iter().any(|line| line.contains(path.as_str())),
            "a missing `{path}` is not reported"
        );
        regenerate(&root);
    }

    let extra = root
        .join(export::SYSTEM_SCHEMA_DIR)
        .join("retired_system.schema.json");
    std::fs::write(&extra, "{}\n").expect("write an extra schema");
    let found = drift(&root);
    assert!(
        found.len() == 1 && found[0].contains("retired_system.schema.json"),
        "a schema no system renders is not the one thing reported: {found:?}"
    );
    regenerate(&root);
    assert!(
        !extra.exists(),
        "regenerating did not remove the extra schema"
    );
    assert!(drift(&root).is_empty(), "regenerating left drift behind");
    let _ = std::fs::remove_dir_all(&root);
}

/// Every way the editor files under `root` depart from what the engine renders:
/// one line per stale or missing file, and per file in `presets/schema/` that no
/// system renders.
fn drift(root: &Path) -> Vec<String> {
    let files = export::editor_files();
    let mut out = Vec::new();
    for (path, generated) in &files {
        match std::fs::read_to_string(root.join(path)) {
            // Compared without the line endings, so a clone that checked the
            // file out as CRLF does not fail a test about its content.
            Ok(committed) if committed.replace("\r\n", "\n") == *generated => {}
            Ok(_) => out.push(format!("{path} is stale")),
            Err(e) => out.push(format!("{path} cannot be read: {e}")),
        }
    }
    for extra in unrendered_in_schema_dir(root, &files) {
        out.push(format!(
            "{} is in {} and no system renders it",
            extra.display(),
            export::SYSTEM_SCHEMA_DIR
        ));
    }
    out
}

/// Write every editor file under `root`, and remove each
/// `presets/schema/*.schema.json` no system renders — a schema left behind by a
/// removed or renamed system. Any other stray file there is left for a person to
/// look at, and `drift` keeps reporting it.
fn regenerate(root: &Path) {
    let files = export::editor_files();
    for (path, generated) in &files {
        let target = root.join(path);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)
                .unwrap_or_else(|e| panic!("create {}: {e}", parent.display()));
        }
        std::fs::write(&target, generated)
            .unwrap_or_else(|e| panic!("write {}: {e}", target.display()));
    }
    for extra in unrendered_in_schema_dir(root, &files) {
        if extra.to_string_lossy().ends_with(".schema.json") {
            std::fs::remove_file(&extra)
                .unwrap_or_else(|e| panic!("remove {}: {e}", extra.display()));
        }
    }
}

/// The files in `root`'s `presets/schema/` that `files` does not name.
fn unrendered_in_schema_dir(root: &Path, files: &[(String, String)]) -> Vec<PathBuf> {
    let dir = root.join(export::SYSTEM_SCHEMA_DIR);
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut out: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            let rendered = files.iter().any(|(name, _)| root.join(name) == *path);
            path.is_file() && !rendered
        })
        .collect();
    out.sort();
    out
}

/// **The `.json` files are not embedded.** `core/build.rs` reads `presets/`
/// non-recursively and filters on the extension, so the generic schema and the
/// whole of `presets/schema/` are held out by construction — but the embedded
/// set is what the foobar path renders with no preset directory, and a stray
/// entry there would ship a JSON document as a preset.
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
        for violation in violations(doc.get_ref(), Schema::Generic) {
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

/// **Every library preset validates against its family's own schema** — the
/// file `.taplo.toml` hands it by filename, and the one an author's completions
/// come from.
///
/// A library file named for no family is a finding here too: the editor would
/// give it the generic schema and no completion, and `ritmolux --check` warns on
/// the same file.
#[test]
fn every_library_preset_validates_against_its_familys_schema() {
    let mut findings: Vec<String> = Vec::new();
    let files = library();
    assert!(
        files.len() >= 100,
        "the library walk found only {} files, which means it stopped reading the \
         tree rather than that the library shrank",
        files.len()
    );
    for file in &files {
        let stem = file
            .file_stem()
            .and_then(|s| s.to_str())
            .expect("a library filename is UTF-8");
        let prefix = stem.split('_').next().unwrap_or(stem);
        let Some(kind) = SystemKind::ALL.into_iter().find(|k| k.family() == prefix) else {
            findings.push(format!(
                "{}: the prefix `{prefix}` is no system's family, so no schema but the \
                 generic one reaches this file",
                file.display()
            ));
            continue;
        };
        let src = std::fs::read_to_string(file)
            .unwrap_or_else(|e| panic!("read {}: {e}", file.display()));
        let doc = DeTable::parse(&src)
            .unwrap_or_else(|e| panic!("{} is not valid TOML: {e}", file.display()));
        for violation in violations(doc.get_ref(), Schema::System(kind)) {
            findings.push(format!(
                "{} against {}: {violation}",
                file.display(),
                export::system_schema_path(kind)
            ));
        }
    }
    assert!(
        findings.is_empty(),
        "{} violation(s) against the family schemas:\n{}",
        findings.len(),
        findings.join("\n")
    );
}

/// **Against one system's own file, another system's parameter and another
/// system's name are both violations naming what was written.** `force` is a
/// swarm parameter and nothing else declares it, which is what makes it the
/// case that fails if a per-system file carried every system's names.
#[test]
fn a_family_schema_refuses_another_systems_parameter_and_name() {
    let fragment = Schema::System(SystemKind::FragmentField);
    let found = violations_in(
        "system = \"fragment_field\"\n\n[params]\nforce = \"1\"\n",
        fragment,
    );
    assert!(
        found.iter().any(|v| v.contains("force")),
        "a swarm parameter was accepted by fragment_field's schema: {found:?}"
    );
    let found = violations_in("system = \"swarm\"\n\n[params]\nglow = \"1\"\n", fragment);
    assert!(
        found
            .iter()
            .any(|v| v.contains("system") && v.contains("swarm")),
        "another system's name was accepted by fragment_field's schema: {found:?}"
    );
    // The near-miss, and the documented shape of a layer: its own parameters
    // are not narrowed inside a per-system file, so a swarm layer's `force` is
    // not this file's to refuse.
    let clean = violations_in(
        "system = \"fragment_field\"\n\n[params]\nglow = \"1\"\nbloom_amount = \"0.5\"\n\n\
         [layer]\nsystem = \"swarm\"\n\n[layer.params]\nforce = \"1\"\n",
        fragment,
    );
    assert!(
        clean.is_empty(),
        "a correct fragment_field preset was convicted by its own schema: {clean:?}"
    );
    // The shape Taplo needs for completion: nothing conditional anywhere.
    for kind in SystemKind::ALL {
        assert!(
            !export::system_json_schema(kind).contains("\"if\""),
            "{} carries an `if`, and Taplo completes nothing through one",
            export::system_schema_path(kind)
        );
    }
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

/// Which rendered schema a document is validated against.
#[derive(Clone, Copy)]
enum Schema {
    /// `presets/preset.schema.json`: each `[params]` surface narrowed by the
    /// `system` it sits beside.
    Generic,
    /// `presets/schema/<system>.schema.json`: `system` pinned, the root's
    /// `[params]` narrowed to that system unconditionally, and a layer's
    /// `[params]` left as the table declares it.
    System(SystemKind),
}

/// The generic-schema violations in `src`, for the fixture cases above.
fn violations_of(src: &str) -> Vec<String> {
    violations_in(src, Schema::Generic)
}

/// The violations in `src` against `schema`.
fn violations_in(src: &str, schema: Schema) -> Vec<String> {
    let doc = DeTable::parse(src).expect("the fixture is valid TOML");
    violations(doc.get_ref(), schema)
}

/// Every way `doc` departs from what the engine declares, as `schema` renders it.
///
/// The same rules [`export::json_schema`] and [`export::system_json_schema`]
/// write: the root's key set is closed, each structural table's is closed, each
/// value matches its [`KeyKind`], and each `[params]` surface admits exactly the
/// names that surface's rosters declare.
fn violations(doc: &DeTable<'_>, schema: Schema) -> Vec<String> {
    let mut out = Vec::new();
    let root = export::table("preset").expect("the root table is declared");
    let declared = string_at(doc, "system");

    check_table(doc, root, "", &mut out);

    match schema {
        Schema::Generic => {
            // The two parameter surfaces, each against its own roster. Skipped
            // when `system` names nothing: that is already a load error, and the
            // schema's `if` does not fire either.
            if let Some(kind) = declared.and_then(SystemKind::from_name)
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
        }
        Schema::System(kind) => {
            // `const`: the pinned name, whatever the roster would also accept.
            if let Some(name) = declared
                && name != kind.as_str()
            {
                out.push(format!(
                    "`system` is `{name}`, where this schema is `{}`'s",
                    kind.as_str()
                ));
            }
            if let Some(params) = table_at(doc, "params") {
                check_params(params, kind, ParamSurface::Root, "params", &mut out);
            }
            // Not narrowed: the layer's `params` is checked as the layer table
            // declares it, which is an open map of expressions.
            if let Some(layer) = table_at(doc, "layer")
                && let Some(params) = value_at(layer, "params")
            {
                let declared = export::table("layer")
                    .and_then(|t| t.keys.iter().find(|k| k.name == "params"))
                    .expect("the layer table declares params");
                check_value(params, &declared.kind, "layer.params", &mut out);
            }
        }
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
    value_at(table, key).and_then(|value| value.get_ref().as_table())
}

/// The value at `key`, or `None` when it is absent.
fn value_at<'t, 'i>(table: &'t DeTable<'i>, key: &str) -> Option<&'t Spanned<DeValue<'i>>> {
    table
        .iter()
        .find(|(name, _)| name.get_ref().as_ref() == key)
        .map(|(_, value)| value)
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
    let mut files = library();
    collect(&repo_root().join("docs/examples"), &mut files);
    files.sort();
    files
}

/// Every library preset: the three library directories, each flat — the files
/// `.taplo.toml`'s family rules can reach.
fn library() -> Vec<PathBuf> {
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
