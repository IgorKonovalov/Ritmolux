//! Convention guard tests (Plan 0002 Phase 2). std-only, no dependency, so
//! "lightweight is a feature" holds even for the guardrails.
//!
//! (a) Every hot-path module carries the panic-denial pragma, so a newly
//!     added hot module can't silently ship without it.
//! (b) Every direct dependency in a workspace member manifest is exact-pinned
//!     (`=x.y.z`), per CLAUDE.md ("pin direct dependencies to exact versions").
//! (c) No scene multiplies the shared clock by a settable field — every bindable
//!     rate integrates a phase instead (ADR-0132, ADR-0135).

use std::path::{Path, PathBuf};

/// The panic-denial header every hot-path module must carry. Copy it verbatim
/// to the top of any new file under `core/src/dsp/`, `core/src/render/`,
/// `core/src/diag/`, `core/src/audio.rs`, `core/src/preset/expr.rs`, the
/// `core-cabi` crate's `src/` (the C ABI, moved out of `core/src/ffi.rs` by
/// ADR-0072), or the `lmv-ring` crate's `src/` (the extracted SPSC ring,
/// Plan 0005):
///
/// ```ignore
/// #![deny(
///     clippy::unwrap_used,
///     clippy::expect_used,
///     clippy::indexing_slicing,
///     clippy::panic,
///     clippy::unreachable
/// )]
/// ```
///
/// `indexing_slicing` is the grep-able sentinel proving the block is present.
const PRAGMA_SENTINEL: &str = "clippy::indexing_slicing";

fn core_src() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

/// The workspace root — the parent of the `core` crate this test lives
/// in. It reaches the sibling crates (`lmv-ring`, `standalone`) whose
/// manifests and hot-path source the guards below also cover.
fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("core crate has a workspace-root parent")
        .to_path_buf()
}

/// Is `path` an out-of-line **test** module — a file the parent declares as
/// `#[cfg(test)] mod <stem>;` (Plan 0061 Phase 2d)?
///
/// Such a file compiles only under `cfg(test)`, so it is not hot-path code and
/// the pragma does not apply to it. **Skipping it is what keeps this guard
/// honest, not a convenience**: a test module carries
/// `#![allow(clippy::expect_used, clippy::indexing_slicing, clippy::panic)]`,
/// and [`PRAGMA_SENTINEL`] greps for the literal `clippy::indexing_slicing`. So
/// a moved-out test file would satisfy the check with an **allow** exactly where
/// the guard means to demand a **deny** — passing vacuously, and turning a real
/// gate into a spelling coincidence.
///
/// This asks the parent rather than matching on the file name, because the moved
/// modules are not all called `tests`: `particles/projection_mirror.rs` is one
/// too, and a name-based rule would silently let it back in.
///
/// # It reads the declaration, not two adjacent lines
///
/// The first version of this matched `#[cfg(test)]` **immediately followed by**
/// `mod <stem>;`, and Plan 0110 Phase 1 wrote a declaration that satisfies
/// neither half:
///
/// ```ignore
/// #[cfg(test)]
/// #[path = "shader_tests.rs"]   // an attribute in between
/// mod tests;                     // and the module is named `tests`
/// ```
///
/// `shader_tests.rs` was therefore collected as hot-path source and passed only
/// because its `#![allow(...)]` block happens to contain the literal
/// [`PRAGMA_SENTINEL`] — the vacuous pass this function's own header warns
/// about, arriving by the exact route it warns about. So the matcher now steps
/// over the attribute run and resolves a `#[path]` to the file it names. That is
/// the general fix rather than the local one: moving the file to a name the old
/// matcher liked would have removed today's instance and left the blindness.
fn is_cfg_test_module(path: &Path) -> bool {
    let Some(dir) = path.parent() else {
        return false;
    };
    // `dir/name.rs` is a child of `dir/mod.rs` when that exists, otherwise of
    // the `dir.rs` sitting beside `dir` (the Rust 2018 layout) — **but a
    // `#[path]` declaration can come from any file in the module tree**, and the
    // one that motivated this came from `warp_mesh/shader.rs` while `mod.rs` sat
    // right there and was the only file being read. So every sibling is asked.
    // The directory is small and this is a test.
    let mut candidates = vec![dir.with_extension("rs")];
    if let Ok(entries) = std::fs::read_dir(dir) {
        let mut siblings: Vec<PathBuf> = entries
            .filter_map(|entry| entry.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|ext| ext == "rs") && p != path)
            .collect();
        siblings.sort();
        candidates.extend(siblings);
    }
    candidates
        .into_iter()
        .any(|parent| declares_cfg_test_module(&parent, path))
}

/// Does `parent` declare `path` as a `#[cfg(test)]` module?
///
/// Reads the declaration rather than two adjacent lines: it steps over the
/// attribute run between the gate and the `mod` line, and resolves a
/// `#[path = "…"]` to the file it names.
fn declares_cfg_test_module(parent: &Path, path: &Path) -> bool {
    let (Some(stem), Some(parent_dir)) = (path.file_stem(), parent.parent()) else {
        return false;
    };
    if !parent.is_file() {
        return false;
    }
    let Ok(text) = std::fs::read_to_string(parent) else {
        return false;
    };

    let mut lines = text.lines().map(str::trim).peekable();
    while let Some(line) = lines.next() {
        if line != "#[cfg(test)]" {
            continue;
        }
        // Everything between the gate and the declaration is attributes; one of
        // them may say which file the module actually lives in.
        let mut declared_path: Option<String> = None;
        while let Some(next) = lines.peek() {
            if !next.starts_with("#[") {
                break;
            }
            if let Some(value) = next
                .strip_prefix("#[path")
                .and_then(|rest| rest.split('"').nth(1))
            {
                declared_path = Some(value.to_string());
            }
            lines.next();
        }
        let Some(name) = lines
            .peek()
            .and_then(|l| l.strip_prefix("mod "))
            .and_then(|l| l.strip_suffix(';'))
        else {
            continue;
        };
        // A `#[path]` on a file-module declaration resolves against the
        // directory of the file that declares it, so `shader.rs` naming
        // `shader_tests.rs` means its own sibling.
        let matches = match &declared_path {
            Some(value) => parent_dir.join(value) == path,
            None => name.trim() == stem.to_string_lossy(),
        };
        if matches {
            return true;
        }
    }
    false
}

fn collect_rs_files(path: &Path, out: &mut Vec<PathBuf>) {
    if path.is_file() {
        if path.extension().is_some_and(|ext| ext == "rs") && !is_cfg_test_module(path) {
            out.push(path.to_path_buf());
        }
        return;
    }
    let mut entries: Vec<PathBuf> = std::fs::read_dir(path)
        .unwrap_or_else(|e| panic!("read_dir {}: {e}", path.display()))
        .map(|entry| entry.expect("dir entry").path())
        .collect();
    entries.sort();
    for entry in &entries {
        collect_rs_files(entry, out);
    }
}

/// **The guard sees a `#[path]`-declared test module** — Plan 0109 Phase 6.
///
/// A positive and a negative in the same file, because the failure this fixes
/// was one-sided: the matcher answered `false` for a test module and the file
/// was then scanned as hot-path source, where it passed on a spelling
/// coincidence. This does not replace the inversion probe the phase ran (delete
/// the sentinel, watch the guard flip) — nothing permanent can, since a guard
/// that has stopped guarding still passes. It pins the resolution.
#[test]
fn the_guard_resolves_a_path_declared_test_module() {
    let warp_mesh = core_src().join("render").join("scenes").join("warp_mesh");
    let declared = warp_mesh.join("shader_tests.rs");
    assert!(
        declared.is_file(),
        "the fixture for this test is a real file in the tree; if it moved,          point this at whatever `#[path]`-declared module replaced it"
    );
    assert!(
        is_cfg_test_module(&declared),
        "`shader_tests.rs` is declared `#[cfg(test)] #[path = \"shader_tests.rs\"]          mod tests;` in `shader.rs` — a test module by declaration, whatever it          is called and whichever file declares it"
    );
    assert!(
        !is_cfg_test_module(&warp_mesh.join("draw.rs")),
        "`draw.rs` builds geometry every displayed frame and is not a test          module; if the skip rule now covers it, the rule has widened"
    );
}

/// The hot-path set the pragma guards. Directories are scanned recursively;
/// a new hot-path directory added by a later plan must be listed here,
/// which is a Mode 4 review item.
#[test]
fn hot_path_modules_carry_the_panic_pragma() {
    let src = core_src();
    let targets = [
        src.join("dsp"),
        src.join("render"),
        src.join("diag"),
        src.join("audio.rs"),
        // The C ABI left this crate for `core-cabi` (ADR-0072). The guard
        // follows the file: if it did not, the pragma would silently stop being
        // enforced on the one seam in the project that must never panic, which
        // is exactly the failure this test exists to prevent.
        workspace_root().join("core-cabi").join("src"),
        // Per-frame preset evaluator (Plan 0003): a single hot-path file inside
        // an otherwise load-time module, so it is listed directly rather than
        // scanning all of `src/preset/`.
        src.join("preset").join("expr.rs"),
        // The SPSC ring's `unsafe` now lives in the sibling lmv-ring crate
        // (Plan 0005); its whole `src/` is hot-path code.
        workspace_root().join("lmv-ring").join("src"),
        // The EEL2 machine (Plan 0100 Phase 2, ADR-0113). Its whole directory,
        // not just `vm.rs`: this is the only code in the engine that executes
        // **untrusted program text** — a converted MilkDrop preset's — and it does
        // so once per mesh vertex per frame. The bytecode module beside the VM is
        // load-time, and is scanned anyway because the split between "decodes" and
        // "executes" is not one a future edit should have to remember.
        src.join("milk"),
    ];

    let mut files = Vec::new();
    for target in &targets {
        assert!(
            target.exists(),
            "hot-path target is missing: {}",
            target.display()
        );
        collect_rs_files(target, &mut files);
    }
    assert!(!files.is_empty(), "found no hot-path source files to check");

    for file in &files {
        let text = std::fs::read_to_string(file)
            .unwrap_or_else(|e| panic!("read {}: {e}", file.display()));
        assert!(
            text.contains(PRAGMA_SENTINEL),
            "hot-path module `{}` is missing the panic-denial pragma \
             (sentinel `{PRAGMA_SENTINEL}`). Copy the `#![deny(...)]` block \
             from tests/hygiene.rs to the top of the file.",
            file.display(),
        );
    }
}

#[test]
fn direct_dependencies_are_exact_pinned() {
    let root = workspace_root();
    let manifests = [
        root.join("core").join("Cargo.toml"),
        root.join("core-cabi").join("Cargo.toml"),
        root.join("lmv-ring").join("Cargo.toml"),
        root.join("standalone").join("Cargo.toml"),
    ];
    for manifest in &manifests {
        check_exact_pins(manifest);
    }
}

fn check_exact_pins(manifest: &Path) {
    let text = std::fs::read_to_string(manifest)
        .unwrap_or_else(|e| panic!("read {}: {e}", manifest.display()));

    let mut in_deps = false;
    let mut depth: i32 = 0;

    for raw in text.lines() {
        let line = raw.trim();

        // Section headers only register at the top level (depth 0).
        if depth == 0 && line.starts_with('[') {
            in_deps = is_dependency_header(line);
            continue;
        }

        // Parse entries only at the top level of a deps table; interior lines
        // of a multi-line inline table (depth > 0) are array/table members.
        if in_deps
            && depth == 0
            && let Some((name, value)) = dependency_entry(line)
            && let Some(version) = declared_version(value)
        {
            assert!(
                version.starts_with('='),
                "{}: dependency `{name}` is not exact-pinned (found `{version}`); \
                 use `=x.y.z` (CLAUDE.md).",
                manifest.display(),
            );
        }

        depth += bracket_delta(line);
        if depth < 0 {
            depth = 0;
        }
    }
}

/// Headers ending in `dependencies]` cover `[dependencies]`,
/// `[build-dependencies]`, and the per-target `[target.'...'.dependencies]`
/// tables where the standalone's real deps live.
fn is_dependency_header(line: &str) -> bool {
    line.starts_with('[') && line.ends_with("dependencies]")
}

/// Net change in `{`/`[` nesting on a line (parentheses ignored).
fn bracket_delta(line: &str) -> i32 {
    line.chars().fold(0, |acc, c| match c {
        '{' | '[' => acc + 1,
        '}' | ']' => acc - 1,
        _ => acc,
    })
}

/// A `name = value` dependency line, or `None` for blanks/comments/non-entry
/// lines. `name` must be a bare dependency identifier.
fn dependency_entry(line: &str) -> Option<(&str, &str)> {
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    let (name, value) = line.split_once('=')?;
    let name = name.trim();
    let is_ident = !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if is_ident {
        Some((name, value.trim()))
    } else {
        None
    }
}

/// The version requirement a dependency value declares, or `None` for
/// `path`/`workspace` deps that carry no version.
fn declared_version(value: &str) -> Option<String> {
    let value = value.trim();
    if value.starts_with('"') {
        return first_quoted(value).map(str::to_string);
    }
    if value.starts_with('{') {
        let key = value.find("version")?;
        let after = value.get(key + "version".len()..)?.trim_start();
        let after = after.strip_prefix('=')?.trim_start();
        return first_quoted(after).map(str::to_string);
    }
    None
}

fn first_quoted(s: &str) -> Option<&str> {
    let start = s.find('"')?;
    let rest = s.get(start + 1..)?;
    let end = rest.find('"')?;
    rest.get(..end)
}

// -----------------------------------------------------------------------
// (c) Every bindable rate integrates a phase (ADR-0132, ADR-0135)
// -----------------------------------------------------------------------

/// **No scene source multiplies the shared clock by one of its own fields.**
///
/// A rate parameter has to be *integrated* to be a rate: a phase computed as
/// `self.time * self.<rate>` lets a rate bound to audio retroactively rescale
/// all elapsed time, so at t = 100 s a small swing moves the picture by tens of
/// seconds in a single frame. `scenes::Phase` is the one way to accumulate one.
///
/// This exists because the rule was enumerated twice and was wrong both times:
/// ADR-0132 named two sites and shipped with three live counterexamples, all of
/// which were found by grepping rather than by reading the list. A list of sites
/// fails the same way whether it lives in a test or in a document, so this scans
/// **every** `.rs` under `scenes/` — a new scene directory is covered without
/// anyone remembering to add it.
///
/// # What it cannot see, and the evasion is one line away
///
/// It matches a *shape*, not a semantics. Binding the clock to a local first —
/// `let time = self.time;` and then `time * self.spin` — passes, and this is not
/// hypothetical: `swarm.rs` and `emitter.rs` both already bind that exact local
/// for unrelated reasons, so in two files the evasion is a single edit. The
/// guard raises the cost of the mistake; **it is not a proof that no scene makes
/// it.**
///
/// # Two deliberate exclusions
///
/// **Line comments are stripped before matching.** Documenting the rejected form
/// is exactly what the type's own doc comment in `scenes/mod.rs` does, and a
/// guard that fails the build on its own explanation would be paid for by
/// deleting the explanation.
///
/// **`warp_mesh/shader.rs` is scanned and passes**, and that is not an accident
/// of scoping: its roughly ten `time * <rate>` uses take the clock as a function
/// parameter and multiply it by locals holding the MilkDrop reference's own
/// fixed frequencies. None has a `self.` receiver because none is a field, and
/// none is settable from a preset — which is the whole of what ADR-0132 forbids.
#[test]
fn no_scene_multiplies_the_clock_by_a_field() {
    let scenes = core_src().join("render").join("scenes");
    let mut files = Vec::new();
    collect_rs_files(&scenes, &mut files);
    assert!(!files.is_empty(), "found no scene source files to check");

    // The scan reaches the file that motivated the exclusion note above, and the
    // file that carried the largest of the three defects. Named so that a change
    // to `collect_rs_files` cannot quietly narrow what this guard sees.
    for must_scan in ["shader.rs", "swarm.rs"] {
        assert!(
            files.iter().any(|f| f.ends_with(must_scan)),
            "the scan no longer reaches `{must_scan}`, so it is guarding less than it reads"
        );
    }

    for file in &files {
        let text = std::fs::read_to_string(file)
            .unwrap_or_else(|e| panic!("read {}: {e}", file.display()));
        if let Some(hit) = clock_multiplied_by_field(&text) {
            panic!(
                "scene `{}` computes `{hit}`. A bindable rate integrates a phase (ADR-0132): store `dt` in `advance`, keep a `scenes::Phase`, and `step(rate, dt)` it in `update`.",
                file.display(),
            );
        }
    }
}

/// The first `self.<field> * self.time` or `self.time * self.<field>` in `text`,
/// rendered back for the failure message, or `None` if it holds the rule.
///
/// Whitespace is collapsed first, so an expression rustfmt broke across lines is
/// matched as readily as one on a single line.
fn clock_multiplied_by_field(text: &str) -> Option<String> {
    let flat = collapse_whitespace(&strip_line_comments(text));

    let lead = "self.time * self.";
    let mut from = 0;
    while let Some(rel) = flat.get(from..)?.find(lead) {
        let at = from + rel;
        let field = leading_ident(flat.get(at + lead.len()..)?);
        if !field.is_empty() && field != "time" {
            return Some(format!("self.time * self.{field}"));
        }
        from = at + lead.len();
    }

    let tail = " * self.time";
    let mut from = 0;
    while let Some(rel) = flat.get(from..)?.find(tail) {
        let at = from + rel;
        let before = flat.get(..at)?;
        let head = before.trim_end_matches(|c: char| c.is_alphanumeric() || c == '_');
        let field = before.get(head.len()..)?;
        if !field.is_empty() && field != "time" && head.ends_with("self.") {
            return Some(format!("self.{field} * self.time"));
        }
        from = at + tail.len();
    }

    None
}

/// The identifier `s` starts with, or `""` if it does not start with one.
fn leading_ident(s: &str) -> String {
    s.chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect()
}

/// `text` with every `//`-to-end-of-line run removed.
///
/// Crude on purpose: a `//` inside a string literal truncates that line early,
/// which can only ever *shrink* what is matched, and no scene's string literals
/// hold the forbidden shape. The alternative is a Rust lexer in a test whose
/// whole premise is that it has no dependencies.
fn strip_line_comments(text: &str) -> String {
    text.lines()
        .map(|line| match line.find("//") {
            Some(at) => line.get(..at).unwrap_or(""),
            None => line,
        })
        .collect::<Vec<_>>()
        .join(
            "
",
        )
}

/// `text` with every run of whitespace collapsed to a single space.
fn collapse_whitespace(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut pending = false;
    for c in text.chars() {
        if c.is_whitespace() {
            pending = !out.is_empty();
        } else {
            if pending {
                out.push(' ');
            }
            pending = false;
            out.push(c);
        }
    }
    out
}
