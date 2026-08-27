//! Convention guard tests (Plan 0002 Phase 2). std-only, no dependency, so
//! "lightweight is a feature" holds even for the guardrails.
//!
//! (a) Every hot-path module carries the panic-denial pragma, so a newly
//!     added hot module can't silently ship without it.
//! (b) Every direct dependency in a workspace member manifest is exact-pinned
//!     (`=x.y.z`), per CLAUDE.md ("pin direct dependencies to exact versions").

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
