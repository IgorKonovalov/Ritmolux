//! The show management has exactly one caller (ADR-0181, Plan 0159 Phase 3).
//!
//! `preset_dir.rs` resolves the preset directory, seeds it, and reloads what an
//! author edited into it. Reaching those from two run modes is what let one mode
//! hold a `Renderer` and a `Director` and report nothing about either: the
//! management lived beside the window, and the duplication needed to give the
//! headless path the same behaviour would have been the whole of it.
//!
//! So the rule is structural rather than behavioural, and this is what enforces
//! it: **every entry point below is reached from `show.rs` and from nowhere
//! else, and the reload is reached exactly once.** A second module calling one
//! of them fails here, which is the point — the failure a reviewer would
//! otherwise have to notice is a test instead.
//!
//! A source scan, deliberately, in the shape `cli.rs`'s
//! `every_scanner_flag_is_rostered` uses: the property is about *how many places
//! name a function*, and no amount of running the program can observe that.
//!
//! GPU-free and process-free — it reads files.

use std::path::{Path, PathBuf};

/// The module every call below must come from.
const OWNER: &str = "show.rs";

/// The entry points, where each is defined so its own `fn` line is not counted
/// as a call, and whether the show is held to a single call to it.
///
/// `reload_presets` is the one held to **one** call, because that call is the
/// behaviour: a startup load and a watcher reload performed by two calls are two
/// chances to disagree about what a reload does. The other two are queries -
/// `dir_signature` is compared against the last one and then re-baselined, which
/// is twice by construction - so for them the claim is only that no other module
/// reaches them.
const ENTRY_POINTS: &[(&str, &str, bool)] = &[
    ("reload_presets", "preset_dir.rs", true),
    ("startup_preset_dir", "preset_dir.rs", false),
    ("dir_signature", "preset_dir.rs", false),
];

fn src_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

/// Every `.rs` file under `standalone/src`, recursively.
fn sources() -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut stack = vec![src_dir()];
    while let Some(dir) = stack.pop() {
        let entries = std::fs::read_dir(&dir)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()));
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                found.push(path);
            }
        }
    }
    assert!(!found.is_empty(), "no sources found under {:?}", src_dir());
    found
}

/// Whether `line` calls `name` rather than defining, importing or naming it in
/// prose.
///
/// A call is `name(`; a definition is `fn name(`; an import is a `use` line; a
/// doc comment or a plain comment is neither. `use` is excluded rather than
/// counted because the extracted module must import what it calls, and an import
/// is not a second place the behaviour happens.
fn is_call(line: &str, name: &str) -> bool {
    let trimmed = line.trim_start();
    if trimmed.starts_with("//") || trimmed.starts_with("use ") {
        return false;
    }
    let Some(at) = line.find(&format!("{name}(")) else {
        return false;
    };
    // `fn reload_presets(` is the definition, and `self.reload_presets(` would
    // be a method of another type that happens to share the name.
    let before = line.get(..at).unwrap_or_default().trim_end();
    !before.ends_with("fn")
}

#[test]
fn every_show_entry_point_is_called_only_from_the_show() {
    let sources = sources();
    for (name, defined_in, single) in ENTRY_POINTS {
        let mut callers: Vec<(String, String)> = Vec::new();
        for path in &sources {
            let file = path
                .file_name()
                .and_then(|f| f.to_str())
                .unwrap_or_default()
                .to_owned();
            let text = std::fs::read_to_string(path)
                .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
            for line in text.lines() {
                if is_call(line, name) {
                    callers.push((file.clone(), line.trim().to_owned()));
                }
            }
        }
        // The definition's own file may call it internally; only the count
        // outside it is the claim, because that is what a second run mode would
        // add to.
        let outside: Vec<&(String, String)> = callers
            .iter()
            .filter(|(file, _)| file != defined_in)
            .collect();
        assert!(
            !outside.is_empty(),
            "`{name}` should be called from {OWNER}; nothing outside \
             {defined_in} calls it at all"
        );
        for (file, line) in &outside {
            assert_eq!(
                file, OWNER,
                "`{name}` should be reached only from {OWNER}, and {file} \
                 reaches it: {line}"
            );
        }
        if *single {
            assert_eq!(
                outside.len(),
                1,
                "`{name}` should be called exactly once outside \
                 {defined_in}; found {outside:#?}"
            );
        }
    }
}
