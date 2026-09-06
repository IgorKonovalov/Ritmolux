//! `docs/configuration.md` is held to the surface it documents (Plan 0156 Phase 2).
//!
//! This is the third-copy pattern `core/tests/preset.rs` uses for the parameter
//! roster: the flag roster and the config schema are the authority, the document
//! is the copy, and a name that exists in one and not the other fails here.
//! Nothing else holds a prose reference to a binary's surface.
//!
//! Three properties, and they are deliberately different in strength:
//!
//!   1. Every flag `--help` prints is named in the document, in backticks. The
//!      flag roster is `pub(crate)` in the binary, so `--help` is how a test
//!      outside that crate reads it - and `--help` is the roster's authority
//!      anyway.
//!   2. Every section and key the config type serialises is named in the
//!      document, in backticks. Both `display_name` keys are `Option` and absent
//!      from a default serialisation, so the walked value sets them; otherwise
//!      the two keys a reader is most likely to need would be the two nothing
//!      checks.
//!   3. The complete example file ROUND-TRIPS: it parses as `Config`, and what
//!      it parses to is `Config::default()`. That is stronger than naming a
//!      default in a table, because a table is prose and this is the value.
//!
//! GPU-free: `--help` exits before a renderer exists (`help_cli.rs` is what
//! proves that from outside the process), and nothing else here leaves the
//! filesystem.

use std::path::{Path, PathBuf};
use std::process::Command;

use standalone::config::Config;

/// The document under test, resolved from the manifest rather than the cwd -
/// a test's working directory is not a contract.
fn doc_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../docs/configuration.md")
}

fn doc() -> String {
    let path = doc_path();
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

/// Whether `name` appears in the document inside backticks.
///
/// Backticks rather than a bare substring: `display` occurs in English prose all
/// over this document, and a bare search for it would pass on a page that never
/// documented the key at all.
fn documented(doc: &str, name: &str) -> bool {
    doc.contains(&format!("`{name}`"))
}

/// Every long flag `ritmolux --help` prints.
///
/// The roster lines are indented and start with the flag; the trailing prose
/// (`-h is a synonym…`) has no such line, so a prefix test over trimmed lines is
/// enough and needs no column arithmetic.
fn rostered_flags() -> Vec<String> {
    let out = Command::new(env!("CARGO_BIN_EXE_ritmolux"))
        .arg("--help")
        .output()
        .expect("failed to spawn the ritmolux binary");
    assert!(out.status.success(), "`ritmolux --help` did not exit 0");
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let flags: Vec<String> = text
        .lines()
        .filter_map(|line| {
            let line = line.trim_start();
            line.starts_with("--")
                .then(|| line.split_whitespace().next().unwrap_or("").to_owned())
        })
        .collect();
    assert!(
        flags.len() > 5,
        "`--help` yielded {} flags, which means this parse stopped matching the \
         roster's shape rather than that the roster shrank:\n{text}",
        flags.len(),
    );
    flags
}

#[test]
fn every_flag_the_help_roster_prints_is_documented() {
    let doc = doc();
    let missing: Vec<String> = rostered_flags()
        .into_iter()
        .filter(|flag| !documented(&doc, flag))
        .collect();
    assert!(
        missing.is_empty(),
        "docs/configuration.md does not name these flags in backticks: {missing:?}\n\
         `ritmolux --help` is the authority on the roster; the document is the copy, \
         and a flag that exists and is undocumented is a flag nobody can find."
    );
}

/// A config value with every `Option` set, so the walk below reaches every key.
fn every_key_populated() -> Config {
    let mut config = Config::default();
    config.output.display_name = Some("a monitor".to_owned());
    config.console.display_name = Some("another monitor".to_owned());
    config
}

/// The `[section]` names and bare keys of a serialised config, in file order.
///
/// A hand walk over the TOML text rather than a reflection pass: `toml` gives no
/// key iteration without a `Value` tree, and the serialised form is what a
/// reader actually types.
fn sections_and_keys(toml_text: &str) -> (Vec<String>, Vec<String>) {
    let mut sections = Vec::new();
    let mut keys = Vec::new();
    for line in toml_text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix('[')
            && let Some(name) = rest.strip_suffix(']')
        {
            sections.push(name.to_owned());
        } else if let Some((key, _)) = line.split_once('=') {
            keys.push(key.trim().to_owned());
        }
    }
    (sections, keys)
}

#[test]
fn every_config_section_and_key_is_documented() {
    let doc = doc();
    let serialised =
        toml::to_string(&every_key_populated()).expect("the config type must serialise");
    let (sections, keys) = sections_and_keys(&serialised);

    assert!(
        sections.len() >= 7 && keys.len() >= 15,
        "the walk found {} sections and {} keys, which means it stopped reading \
         the serialised shape rather than that the schema shrank:\n{serialised}",
        sections.len(),
        keys.len(),
    );

    let missing: Vec<String> = sections
        .iter()
        .map(|s| format!("[{s}]"))
        .chain(keys.iter().cloned())
        .filter(|name| !documented(&doc, name))
        .collect();
    assert!(
        missing.is_empty(),
        "docs/configuration.md does not name these in backticks: {missing:?}\n\
         The config type is the authority on what config.toml holds."
    );
}

#[test]
fn the_two_environment_variables_are_documented() {
    let doc = doc();
    for name in ["RLX_PRESET_DIR", "RLX_TIER"] {
        assert!(
            documented(&doc, name),
            "docs/configuration.md does not name `{name}` in backticks"
        );
    }
}

/// The fenced `toml` block the document marks as the complete file.
///
/// Anchored on the HTML comment above it rather than on "the last toml fence",
/// so a `toml` example added later cannot silently become the thing under test.
fn complete_example(doc: &str) -> &str {
    const MARKER: &str = "round-tripped through the config type";
    let after = doc
        .split_once(MARKER)
        .expect("docs/configuration.md must mark its complete example with the round-trip comment")
        .1;
    let fence = after
        .split_once("```toml\n")
        .expect("the round-trip marker must be followed by a ```toml block")
        .1;
    fence
        .split_once("\n```")
        .expect("the complete example's fence is unclosed")
        .0
}

#[test]
fn the_complete_example_parses_to_the_defaults() {
    let doc = doc();
    let example = complete_example(&doc);
    let parsed: Config = toml::from_str(example)
        .unwrap_or_else(|e| panic!("the complete config.toml example does not parse: {e}"));

    let from_doc = toml::to_string(&parsed).expect("the parsed config must serialise");
    let defaults = toml::to_string(&Config::default()).expect("the defaults must serialise");
    assert_eq!(
        from_doc, defaults,
        "the complete config.toml example does not parse to the built-in defaults.\n\
         Left is the document's file re-serialised; right is Config::default()."
    );
}

#[test]
fn the_complete_example_states_every_key_it_can() {
    // The two `display_name` keys have no spelling at their default - the value
    // is absent, and TOML has no null - so the example states every OTHER key.
    // Without this the example could quietly shrink to `[output]` alone and
    // still parse to the defaults, which is the one way the test above passes
    // while the document stops being complete.
    let doc = doc();
    let example = complete_example(&doc);
    let (sections, keys) = sections_and_keys(&toml::to_string(&Config::default()).unwrap());
    for section in &sections {
        assert!(
            example.contains(&format!("[{section}]")),
            "the complete config.toml example omits the `[{section}]` section"
        );
    }
    for key in &keys {
        assert!(
            example.contains(&format!("{key} =")),
            "the complete config.toml example omits `{key}`"
        );
    }
}
