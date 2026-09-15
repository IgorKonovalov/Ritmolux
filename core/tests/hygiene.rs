//! Convention guard tests (Plan 0002 Phase 2). std-only, no dependency, so
//! "lightweight is a feature" holds even for the guardrails.
//!
//! (a) Every hot-path module carries the panic-denial pragma, so a newly
//!     added hot module can't silently ship without it.
//! (b) Every direct dependency in a workspace member manifest is exact-pinned
//!     (`=x.y.z`), per CLAUDE.md ("pin direct dependencies to exact versions").
//! (c) No scene multiplies the shared clock by a settable field — every bindable
//!     rate integrates a phase instead (ADR-0132, ADR-0135).
//! (d) Every `SystemKind` has a gallery image in `scripts/docs-shots.mjs`, and
//!     every gallery entry names a real system (backlog 0133).
//! (e) The component size cap in `packaging/foobar/build-component.ps1` is the
//!     one `docs/nfr.md` §4 states (ADR-0159). Two copies of a number is the
//!     shape this repository keeps finding rot in.
//! (f) A frame delta is checked for finiteness in exactly one place in
//!     `core/src/`, `sanitize_frame_dt` (ADR-0191).
//! (g) Every integration test exempted from `clippy::disallowed_methods` is
//!     scheduled alone by `.config/nextest.toml`, or listed with a reason, and
//!     that override names nothing else (ADR-0193).
//! (h) Every spawn of the player or the `shot` example under `standalone/tests/`
//!     goes through `standalone/tests/common/mod.rs`, which gives the child a
//!     scratch per-user data root.

use std::path::{Path, PathBuf};

/// The panic-denial header every hot-path module must carry. Copy it verbatim
/// to the top of any new file under `core/src/dsp/`, `core/src/render/`,
/// `core/src/diag/`, `core/src/audio.rs`, `core/src/preset/expr.rs`, the
/// `core-cabi` crate's `src/` (the C ABI, moved out of `core/src/ffi.rs` by
/// ADR-0072), or the `rlx-ring` crate's `src/` (the extracted SPSC ring,
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
/// in. It reaches the sibling crates (`rlx-ring`, `standalone`) whose
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
        // The SPSC ring's `unsafe` now lives in the sibling rlx-ring crate
        // (Plan 0005); its whole `src/` is hot-path code.
        workspace_root().join("rlx-ring").join("src"),
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
        root.join("rlx-ring").join("Cargo.toml"),
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

/// (d) Every `SystemKind` has a gallery image in `scripts/docs-shots.mjs`, and
/// every gallery entry names a real system.
///
/// The check lives here rather than inside `scripts/docs-shots.mjs`, whose
/// manifest it reads, because there it can only run behind a GPU and the only
/// thing that executes that script is a human at a plan close. It is pure text -
/// the manifest and the system roster, no render - so it belongs where it fails on the
/// commit that ships a scene. Inside the script a system with no gallery entry
/// is a hard error that takes the whole sweep down, including the images that
/// have nothing to do with it: three systems accumulated over eleven days behind
/// exactly that, and nothing reported it (backlog 0133).
///
/// **It does NOT claim the images are current**, and must not grow into that.
/// A render is not byte-reproducible across machines - a different GPU, driver
/// or a WARP fallback moves pixels for reasons unrelated to whether the
/// documentation is true - so freshness stays a human duty at a named cadence
/// (ADR-0100). "Every system has a picture" is a different claim, it is
/// mechanical, and it is the one that failed. Backlog 0133.
#[test]
fn every_system_has_a_gallery_image() {
    let root = workspace_root();
    let manifest = std::fs::read_to_string(root.join("scripts").join("docs-shots.mjs"))
        .expect("scripts/docs-shots.mjs is readable");
    let schema = std::fs::read_to_string(
        root.join("core")
            .join("src")
            .join("preset")
            .join("schema")
            .join("system.rs"),
    )
    .expect("core/src/preset/schema/system.rs is readable");

    let systems = system_names(&schema);
    assert!(
        systems.len() >= 12,
        "read {} system names out of SystemKind::from_name; the parse has broken, \
         not the roster",
        systems.len()
    );

    let gallery = gallery_names(&manifest);
    assert!(
        !gallery.is_empty(),
        "read no gallery entries out of scripts/docs-shots.mjs; the parse has broken"
    );

    let missing: Vec<&String> = systems.iter().filter(|s| !gallery.contains(*s)).collect();
    let extra: Vec<&String> = gallery.iter().filter(|g| !systems.contains(*g)).collect();
    assert!(
        missing.is_empty() && extra.is_empty(),
        "the gallery manifest and SystemKind::from_name disagree.\n  \
         no docs/images/gallery/<name>.png entry for: {missing:?}\n  \
         not a system: {extra:?}\n\
         Add the entry to scripts/docs-shots.mjs and re-run it on a machine with a GPU."
    );

    // The committed PNG each entry names. Existence, not freshness: a manifest
    // entry whose file was never rendered is a broken picture in the docs, and
    // that is checkable without a GPU.
    for name in &gallery {
        let png = root
            .join("docs")
            .join("images")
            .join("gallery")
            .join(format!("{name}.png"));
        assert!(
            png.is_file(),
            "scripts/docs-shots.mjs lists {name} but docs/images/gallery/{name}.png \
             is not committed"
        );
    }
}

/// The canonical system names, read out of `SystemKind`'s own roster rather
/// than mirrored, so a system added later cannot be absent from both sides at
/// once.
///
/// Each row of the `const TABLE` block opens with a `SystemKind::` variant, and
/// the first string literal after it is that row's canonical name -- the second
/// is its family, which is not a system. So the body is split at each
/// `SystemKind::` and only the first quoted string of each piece is read. Reading
/// quotes rather than a per-row pattern is what makes this survive rustfmt
/// wrapping one row across several lines and leaving the next on one.
fn system_names(schema: &str) -> Vec<String> {
    let mut out = Vec::new();
    let Some(table) = schema.find("const TABLE:") else {
        return out;
    };
    // The body, past the type annotation, which names `SystemKind::VARIANT_COUNT`.
    let Some(open) = schema[table..].find("= {").map(|i| table + i) else {
        return out;
    };
    let end = schema[open..]
        .find("\n};")
        .map(|i| open + i)
        .unwrap_or(schema.len());
    for row in schema[open..end].split("SystemKind::").skip(1) {
        let Some((_, after)) = row.split_once('"') else {
            continue;
        };
        let Some((name, _)) = after.split_once('"') else {
            continue;
        };
        if !name.is_empty() {
            out.push(name.to_string());
        }
    }
    out
}

/// The per-SYSTEM gallery entries' file stems, read out of the manifest's
/// `out:` fields.
///
/// A stem containing `/` is skipped, and that is what keeps the two collections
/// apart: the per-preset cards live one level down at
/// `docs/images/gallery/presets/<preset>.png`, so they match the same prefix and
/// would otherwise read as 82 systems that do not exist. They are read instead
/// by [`card_presets`], out of the manifest's own `CARDS` list.
fn gallery_names(manifest: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in manifest.lines() {
        let Some((_, rest)) = line.split_once("docs/images/gallery/") else {
            continue;
        };
        let Some((stem, _)) = rest.split_once(".png") else {
            continue;
        };
        if stem.contains('/') {
            continue;
        }
        if !stem.is_empty() && !out.contains(&stem.to_string()) {
            out.push(stem.to_string());
        }
    }
    out
}

/// The preset names in the manifest's `CARDS` list.
///
/// Read as "every quoted string between `const CARDS = [` and the closing `];`",
/// the same shape as [`system_names`] reads `const TABLE`, so a group comment
/// gaining a name or rustfmt-equivalent reflowing cannot break the parse.
fn card_presets(manifest: &str) -> Vec<String> {
    let mut out = Vec::new();
    let Some(start) = manifest.find("const CARDS = [") else {
        return out;
    };
    let end = manifest[start..]
        .find("\n];")
        .map(|i| start + i)
        .unwrap_or(manifest.len());
    let mut rest = &manifest[start..end];
    while let Some(open) = rest.find('"') {
        let after = &rest[open + 1..];
        let Some(close) = after.find('"') else {
            break;
        };
        let name = &after[..close];
        if !name.is_empty() {
            out.push(name.to_string());
        }
        rest = &after[close + 1..];
    }
    out
}

/// The presets that actually ship: `presets/*.toml`, non-recursive.
///
/// This is `core/build.rs`'s own rule (ADR-0022), which is why `presets/pending/`
/// is invisible here without being named -- a `read_dir` that does not descend
/// skips a subdirectory by construction, and that is exactly how a pending
/// preset stays out of the shipped set.
fn shipped_presets(root: &Path) -> Vec<String> {
    let mut out: Vec<String> = std::fs::read_dir(root.join("presets"))
        .expect("presets/ is readable")
        .filter_map(|entry| {
            let path = entry.expect("presets/ entry is readable").path();
            if !path.is_file() || path.extension()? != "toml" {
                return None;
            }
            Some(path.file_stem()?.to_string_lossy().into_owned())
        })
        .collect();
    out.sort();
    out
}

/// (d2) Every shipped preset has a gallery card in `scripts/docs-shots.mjs`, and
/// every card names a preset that ships.
///
/// The second half of (d), over the second collection. Same reasoning, same
/// place, and for the same reason: it is pure text, so it fails on the commit
/// that adds a preset rather than whenever someone next runs the renderer behind
/// a GPU. The manifest's `CARDS` list is spelled out rather than globbed
/// precisely so this can fail -- a glob would define the missing case out of
/// existence.
///
/// **It does not claim the cards are current** either, for the reason (d) gives
/// above. Existence is mechanical; freshness is not.
#[test]
fn every_shipped_preset_has_a_gallery_card() {
    let root = workspace_root();
    let manifest = std::fs::read_to_string(root.join("scripts").join("docs-shots.mjs"))
        .expect("scripts/docs-shots.mjs is readable");

    let shipped = shipped_presets(&root);
    assert!(
        shipped.len() >= 12,
        "read {} presets out of presets/; the directory scan has broken, not the library",
        shipped.len()
    );

    let cards = card_presets(&manifest);
    assert!(
        !cards.is_empty(),
        "read no entries out of the CARDS list in scripts/docs-shots.mjs; the parse has broken"
    );

    let missing: Vec<&String> = shipped.iter().filter(|p| !cards.contains(p)).collect();
    let extra: Vec<&String> = cards.iter().filter(|c| !shipped.contains(c)).collect();
    assert!(
        missing.is_empty() && extra.is_empty(),
        "the CARDS list in scripts/docs-shots.mjs and the shipped preset set disagree.\n  \
         ships with no card: {missing:?}\n  \
         a card for a preset that does not ship: {extra:?}\n\
         Add the name to CARDS and re-run scripts/docs-shots.mjs on a machine with a GPU."
    );

    for preset in &cards {
        let png = root
            .join("docs")
            .join("images")
            .join("gallery")
            .join("presets")
            .join(format!("{preset}.png"));
        assert!(
            png.is_file(),
            "scripts/docs-shots.mjs lists {preset} in CARDS but \
             docs/images/gallery/presets/{preset}.png is not committed"
        );
    }
}

// ---------------------------------------------------------------------------
// (e) The component's size cap is written down once
// ---------------------------------------------------------------------------

/// ADR-0159's stated negative: the recipe now carries a constant that can drift
/// from the NFR it cites. This is what answers that, rather than a comment.
///
/// Both figures are asserted in **bytes with separators**, the form NFR §4 and
/// the size series in `docs/specs/0001-c-abi.md` both write, so a reader
/// comparing the two documents is comparing the same string. The warning
/// threshold is derived rather than transcribed: it is 90 % of the cap, and the
/// arithmetic is checked here so the two cannot drift apart independently.
#[test]
fn the_component_size_cap_agrees_between_the_recipe_and_the_nfr() {
    /// NFR §4's cap for `foo_ritmolux.dll`, as a number.
    const CAP: u64 = 12_582_912;
    /// 90 % of it, which is where the recipe warns.
    const WARN: u64 = 11_324_620;

    // Derived, not transcribed: a cap edited without its threshold is the
    // drift this test exists to catch, and it would otherwise pass.
    assert_eq!(
        WARN,
        CAP * 9 / 10,
        "the warning threshold must be 90 % of the cap"
    );

    let root = workspace_root();
    let recipe_path = root.join("packaging/foobar/build-component.ps1");
    let recipe = std::fs::read_to_string(&recipe_path)
        .unwrap_or_else(|e| panic!("read {}: {e}", recipe_path.display()));
    for (name, value) in [("$ComponentCapBytes", CAP), ("$ComponentWarnBytes", WARN)] {
        let assignment = format!("{name} = {value}");
        assert!(
            recipe.contains(&assignment),
            "packaging/foobar/build-component.ps1 does not set `{assignment}`; \
             the recipe and docs/nfr.md §4 have drifted apart"
        );
    }

    let nfr_path = root.join("docs/nfr.md");
    let nfr = std::fs::read_to_string(&nfr_path)
        .unwrap_or_else(|e| panic!("read {}: {e}", nfr_path.display()));
    // The unit is part of the claim: "~10 MB" is what could be read two ways,
    // and a figure written without `B` would reintroduce exactly that.
    for figure in ["12,582,912 B", "11,324,620 B"] {
        assert!(
            nfr.contains(figure),
            "docs/nfr.md does not state `{figure}`, which \
             packaging/foobar/build-component.ps1 reads it as saying"
        );
    }
    assert!(
        !nfr.contains("Soft cap ~10 MB"),
        "docs/nfr.md §4 still carries the unitless `~10 MB` cap ADR-0159 replaced"
    );
}

// ---------------------------------------------------------------------------
// (f) One stall policy (ADR-0191)
// ---------------------------------------------------------------------------

/// The one finiteness guard on a frame delta the engine keeps: the line in
/// `sanitize_frame_dt`, as `(path under core/src/, trimmed source line)`.
const DT_GUARD: (&str, &str) = ("render/mod.rs", "if dt.is_finite() && dt > 0.0 {");

/// Finiteness checks on a variable named `dt` that are **not** a frame delta
/// being guarded, each with the reason it is not. An entry is
/// `(path under core/src/, trimmed source line, reason)`. Every entry must still
/// match a line in the tree, so a stale one fails rather than silently widening
/// what passes.
const DT_GUARD_ALLOWED: &[(&str, &str, &str)] = &[(
    "render/tier.rs",
    "if dt.is_finite() && dt > threshold {",
    "`sustained_miss` counts measured frame times out of the governor's rolling \
     history; this `dt` is one sample of that series, never accumulated, and a \
     non-finite sample is simply not a miss",
)];

/// **A frame delta is checked in exactly one place** (ADR-0191).
///
/// Every `Renderer` entry that takes a caller's delta runs it through
/// `sanitize_frame_dt`, which substitutes one nominal step. A second guard below
/// that — keep the previous value, hold, freeze, run nothing — is unreachable on
/// the frame path and is also a second answer to the same question, which the
/// next reader cannot tell is deliberate. The population was enumerated by hand
/// three times and was short each time; this counts it instead.
///
/// It matches `dt.is_finite()` or `is_finite(dt)` on a variable or field named
/// exactly `dt`, which covers either operand order and a negated check, across
/// every non-test `.rs` under `core/src/`. Line comments are stripped first, so
/// prose describing the guard does not count.
///
/// # What it cannot see
///
/// A guard spelled another way (`dt.is_nan()`, a check on a renamed local) and,
/// above all, an entry point that forgets to call `sanitize_frame_dt`. The
/// function's doc and the `Renderer` doc are what stand there.
#[test]
fn a_frame_delta_is_checked_for_finiteness_in_exactly_one_place() {
    let src = core_src();
    let mut files = Vec::new();
    collect_rs_files(&src, &mut files);
    assert!(
        files.iter().any(|f| f.ends_with("tier.rs")) && files.iter().any(|f| f.ends_with("mod.rs")),
        "the scan no longer reaches the files it must, so it is guarding less than it reads"
    );

    let mut hits: Vec<(String, String)> = Vec::new();
    for file in &files {
        let text = std::fs::read_to_string(file)
            .unwrap_or_else(|e| panic!("read {}: {e}", file.display()));
        let rel = file
            .strip_prefix(&src)
            .expect("scanned file is under core/src")
            .to_string_lossy()
            .replace('\\', "/");
        for line in strip_line_comments(&text).lines() {
            if checks_dt_finiteness(line) {
                hits.push((rel.clone(), line.trim().to_string()));
            }
        }
    }

    for (path, line, reason) in DT_GUARD_ALLOWED {
        assert!(
            hits.iter().any(|(p, l)| p == path && l == line),
            "the allowlist entry `{path}: {line}` ({reason}) no longer matches the tree; \
             remove it rather than leaving it to excuse a future guard"
        );
    }
    let guards: Vec<&(String, String)> = hits
        .iter()
        .filter(|(p, l)| {
            !DT_GUARD_ALLOWED
                .iter()
                .any(|(path, line, _)| p == path && l == line)
        })
        .collect();

    let (guard_path, guard_line) = DT_GUARD;
    assert!(
        guards
            .iter()
            .any(|(p, l)| p == guard_path && l == guard_line),
        "found no `{guard_line}` in core/src/{guard_path}; either `sanitize_frame_dt` \
         changed shape and DT_GUARD must follow it, or the scan has broken"
    );
    assert_eq!(
        guards.len(),
        1,
        "a frame delta is checked for finiteness in more than one place, and only \
         `sanitize_frame_dt` may:\n  {}\n\
         Every renderer entry already replaces a degenerate delta with one nominal step \
         (ADR-0191), so a guard below it is a second policy. Delete it; if this `dt` is \
         not a frame delta, add it to DT_GUARD_ALLOWED with the reason.",
        guards
            .iter()
            .map(|(p, l)| format!("core/src/{p}: {l}"))
            .collect::<Vec<_>>()
            .join("\n  ")
    );
}

/// Whether `line` checks the finiteness of something named exactly `dt`.
fn checks_dt_finiteness(line: &str) -> bool {
    let is_ident = |c: char| c.is_alphanumeric() || c == '_';
    let method = "dt.is_finite()";
    let mut from = 0;
    while let Some(rel) = line[from..].find(method) {
        let at = from + rel;
        if !line[..at].chars().next_back().is_some_and(is_ident) {
            return true;
        }
        from = at + method.len();
    }
    line.contains("is_finite(dt)")
}

// ---------------------------------------------------------------------------
// (g) A test that reads the clock runs alone (ADR-0193)
// ---------------------------------------------------------------------------

/// The lint `clippy.toml` raises on every clock read; a test that reads one has
/// to carry an exemption from it, which is what makes the exemption a complete
/// marker of the class.
const CLOCK_LINT: &str = "clippy::disallowed_methods";

/// The setting that identifies the one `.config/nextest.toml` override whose
/// filter schedules tests alone.
const ALONE_SETTING: &str = "threads-required = \"num-test-threads\"";

/// Integration test binaries that carry [`CLOCK_LINT`] and are deliberately
/// **not** named by the run-alone override, as `(binary, reason)`. Every entry
/// must still carry the exemption and must stay out of the filter, so a stale
/// one fails rather than silently excusing a future test.
const CLOCK_ALONE_EXEMPT: &[(&str, &str)] = &[
    (
        "control_loopback",
        "backlog 0219: under load a loopback ctl/preset never reached the listener, and running          alone would hide it rather than remove load",
    ),
    (
        "stream_show",
        "backlog 0220: under load a drained ctl/preset never reached the screen, and running          alone would hide it rather than remove load",
    ),
];

/// One `+`-separated term of the run-alone override's filter.
#[derive(Debug, PartialEq)]
enum AloneTerm {
    /// `binary(name)` or `binary(/regex/)`: every test in the matching binaries.
    Binary(NamePattern),
    /// `(binary(name) & test(=name))`: one test in one binary, for a file whose
    /// exemption is on a single function rather than the whole file.
    Test { binary: String, test: String },
}

/// The binary-name matchers the guard can evaluate without a regex engine.
#[derive(Debug, PartialEq)]
enum NamePattern {
    /// `name`, or `/^name$/`.
    Exact(String),
    /// `/^name/`.
    Prefix(String),
    /// `/name$/`.
    Suffix(String),
    /// `/name/`.
    Contains(String),
}

impl NamePattern {
    fn matches(&self, binary: &str) -> bool {
        match self {
            NamePattern::Exact(name) => binary == name,
            NamePattern::Prefix(name) => binary.starts_with(name.as_str()),
            NamePattern::Suffix(name) => binary.ends_with(name.as_str()),
            NamePattern::Contains(name) => binary.contains(name.as_str()),
        }
    }
}

/// Whether `name` is a plain identifier: the only literal a pattern may hold.
fn is_plain_name(name: &str) -> bool {
    !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// The argument of `head(...)` when `term` is exactly that call.
fn call_argument<'a>(term: &'a str, head: &str) -> Option<&'a str> {
    term.strip_prefix(head)?
        .strip_prefix('(')?
        .strip_suffix(')')
}

/// A `binary(...)` argument as a [`NamePattern`].
fn parse_binary_argument(arg: &str) -> Result<NamePattern, String> {
    let Some(regex) = arg
        .strip_prefix('/')
        .and_then(|rest| rest.strip_suffix('/'))
    else {
        return if is_plain_name(arg) {
            Ok(NamePattern::Exact(arg.to_owned()))
        } else {
            Err(format!("`binary({arg})` is not a plain binary name"))
        };
    };
    let (anchored_start, rest) = match regex.strip_prefix('^') {
        Some(rest) => (true, rest),
        None => (false, regex),
    };
    let (anchored_end, name) = match rest.strip_suffix('$') {
        Some(name) => (true, name),
        None => (false, rest),
    };
    if !is_plain_name(name) {
        return Err(format!(
            "`binary(/{regex}/)` is a regex beyond an optionally anchored plain name"
        ));
    }
    let name = name.to_owned();
    Ok(match (anchored_start, anchored_end) {
        (true, true) => NamePattern::Exact(name),
        (true, false) => NamePattern::Prefix(name),
        (false, true) => NamePattern::Suffix(name),
        (false, false) => NamePattern::Contains(name),
    })
}

/// Parse the run-alone override's filter into its terms.
///
/// # The syntax it accepts
///
/// A union of terms joined by `+`, each of them one of:
///
/// - `binary(name)`, a plain identifier;
/// - `binary(/re/)`, where `re` is a plain identifier optionally anchored by a
///   leading `^` and/or a trailing `$`;
/// - `(binary(name) & test(=name))`, one test of one binary.
///
/// Anything else — `not`, `-`, `|`, a `test()` outside that parenthesised form,
/// a regex with metacharacters in it — is an `Err` naming the term. That is the
/// point: a filter rewritten into a form this cannot read must fail the guard,
/// never be read as naming nothing.
fn parse_alone_filter(filter: &str) -> Result<Vec<AloneTerm>, String> {
    let mut terms = Vec::new();
    let mut depth = 0i32;
    let mut start = 0;
    let mut pieces = Vec::new();
    for (at, c) in filter.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            '+' if depth == 0 => {
                pieces.push(&filter[start..at]);
                start = at + 1;
            }
            _ => {}
        }
        if depth < 0 {
            return Err(format!("unbalanced `)` in `{filter}`"));
        }
    }
    if depth != 0 {
        return Err(format!("unbalanced `(` in `{filter}`"));
    }
    pieces.push(&filter[start..]);

    for piece in pieces {
        let term = piece.trim();
        if let Some(arg) = call_argument(term, "binary") {
            terms.push(AloneTerm::Binary(parse_binary_argument(arg)?));
            continue;
        }
        let Some(inner) = term.strip_prefix('(').and_then(|t| t.strip_suffix(')')) else {
            return Err(format!("`{term}` is not a form the guard reads"));
        };
        let parts: Vec<&str> = inner.split('&').map(str::trim).collect();
        let [binary, test] = parts.as_slice() else {
            return Err(format!("`{term}` is not `(binary(name) & test(=name))`"));
        };
        let binary = call_argument(binary, "binary").filter(|name| is_plain_name(name));
        let test = call_argument(test, "test")
            .and_then(|arg| arg.strip_prefix('='))
            .filter(|name| is_plain_name(name));
        let (Some(binary), Some(test)) = (binary, test) else {
            return Err(format!("`{term}` is not `(binary(name) & test(=name))`"));
        };
        terms.push(AloneTerm::Test {
            binary: binary.to_owned(),
            test: test.to_owned(),
        });
    }
    Ok(terms)
}

/// The filter of the one `[[profile.default.overrides]]` block in `toml` that
/// carries [`ALONE_SETTING`]. Zero such blocks, two, or a filter that is not a
/// single-line literal string is an `Err`.
fn alone_override_filter(toml: &str) -> Result<String, String> {
    let mut blocks: Vec<Vec<&str>> = Vec::new();
    let mut in_override = false;
    for line in toml.lines().map(str::trim) {
        if line.starts_with('[') {
            in_override = line == "[[profile.default.overrides]]";
            if in_override {
                blocks.push(Vec::new());
            }
            continue;
        }
        if in_override && let Some(block) = blocks.last_mut() {
            block.push(line);
        }
    }
    let alone: Vec<&Vec<&str>> = blocks
        .iter()
        .filter(|block| block.contains(&ALONE_SETTING))
        .collect();
    let [block] = alone.as_slice() else {
        return Err(format!(
            "expected exactly one [[profile.default.overrides]] block setting \
             `{ALONE_SETTING}`, found {}",
            alone.len()
        ));
    };
    let filters: Vec<&str> = block
        .iter()
        .filter_map(|line| line.strip_prefix("filter"))
        .map(|rest| rest.trim_start())
        .filter_map(|rest| rest.strip_prefix('='))
        .map(str::trim)
        .collect();
    let [filter] = filters.as_slice() else {
        return Err(format!(
            "the `{ALONE_SETTING}` override carries {} `filter` lines, not one",
            filters.len()
        ));
    };
    filter
        .strip_prefix('\'')
        .and_then(|f| f.strip_suffix('\''))
        .filter(|f| !f.contains('\''))
        .map(str::to_owned)
        .ok_or_else(|| {
            format!(
                "the `{ALONE_SETTING}` override's filter is not a one-line literal string: {filter}"
            )
        })
}

/// What one integration test file says about the clock.
struct ClockExemption {
    /// `<member>/tests/<binary>.rs`, with forward slashes.
    rel: String,
    /// The file stem, which is the test binary's name.
    binary: String,
    /// Whether an inner `#![...]` attribute exempts the whole file.
    file_level: bool,
    /// The functions an outer `#[...]` attribute exempts.
    functions: Vec<String>,
}

/// The lint-level attributes an exemption can be written with.
const EXEMPTING_LEVELS: [&str; 2] = ["allow", "expect"];

/// A lint-level attribute starting at byte `at` of `text`: whether it is an
/// inner attribute, and the byte range of its parenthesised argument list.
///
/// Reads `#`, an optional `!`, `[`, one of [`EXEMPTING_LEVELS`], `(`, with any
/// whitespace between, then balances parentheses to the closing one. A `"`
/// string inside the list is skipped whole, escapes included, so a parenthesis
/// in a `reason` does not end it. `None` for anything else starting with `#`.
fn lint_attribute_at(text: &str, at: usize) -> Option<(bool, std::ops::Range<usize>)> {
    let rest = text.get(at..)?.strip_prefix('#')?;
    let (inner, rest) = match rest.trim_start().strip_prefix('!') {
        Some(after) => (true, after),
        None => (false, rest),
    };
    let rest = rest.trim_start().strip_prefix('[')?.trim_start();
    let level = leading_ident(rest);
    if !EXEMPTING_LEVELS.contains(&level.as_str()) {
        return None;
    }
    let rest = rest[level.len()..].trim_start().strip_prefix('(')?;
    let open = text.len() - rest.len();
    let mut depth = 1usize;
    let mut chars = rest.char_indices();
    while let Some((offset, c)) = chars.next() {
        match c {
            '"' => {
                while let Some((_, s)) = chars.next() {
                    match s {
                        '\\' => {
                            chars.next();
                        }
                        '"' => break,
                        _ => {}
                    }
                }
            }
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some((inner, open..open + offset));
                }
            }
            _ => {}
        }
    }
    None
}

/// Where `text` (comments already stripped) carries [`CLOCK_LINT`] in a
/// lint-level attribute: whole-file or per function, or `None` when it does not.
///
/// Only an attribute's argument list counts, so a string or an identifier that
/// merely spells the lint's name is not an exemption. An inner attribute makes
/// it file-level; an outer one attaches it to the next `fn` below it.
fn clock_exemption(rel: String, binary: String, text: &str) -> Option<ClockExemption> {
    let mut found = ClockExemption {
        rel,
        binary,
        file_level: false,
        functions: Vec::new(),
    };
    let mut any = false;
    for (at, _) in text.match_indices('#') {
        let Some((inner, args)) = lint_attribute_at(text, at) else {
            continue;
        };
        if !text[args.clone()].contains(CLOCK_LINT) {
            continue;
        }
        any = true;
        if inner {
            found.file_level = true;
        } else if let Some(name) = text[args.end..]
            .find("fn ")
            .map(|fn_at| leading_ident(&text[args.end + fn_at + 3..]))
            .filter(|name| !name.is_empty())
        {
            found.functions.push(name);
        }
    }
    any.then_some(found)
}

/// Every workspace member's direct `tests/*.rs` file, as `(rel, binary, text)`.
fn integration_test_files(root: &Path) -> Vec<(String, String, String)> {
    let manifest =
        std::fs::read_to_string(root.join("Cargo.toml")).expect("read the workspace manifest");
    let members = manifest
        .lines()
        .find_map(|line| line.trim().strip_prefix("members = ["))
        .and_then(|rest| rest.strip_suffix(']'))
        .expect("the workspace manifest lists `members = [...]` on one line");
    let mut files = Vec::new();
    for member in members
        .split(',')
        .map(|m| m.trim().trim_matches('"'))
        .filter(|m| !m.is_empty())
    {
        let dir = root.join(member).join("tests");
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut paths: Vec<PathBuf> = entries
            .filter_map(|entry| entry.ok().map(|e| e.path()))
            .filter(|p| p.is_file() && p.extension().is_some_and(|ext| ext == "rs"))
            .collect();
        paths.sort();
        for path in paths {
            let binary = path
                .file_stem()
                .expect("a file has a stem")
                .to_string_lossy()
                .into_owned();
            let text = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
            files.push((format!("{member}/tests/{binary}.rs"), binary, text));
        }
    }
    files
}

/// **Every clock-reading integration test is scheduled alone, and only those
/// are** (ADR-0193).
///
/// Both directions over every workspace member's `tests/*.rs`:
///
/// - a file carrying [`CLOCK_LINT`] is named by the run-alone override's filter —
///   by `binary(...)` when the exemption is file-level, or by that binary's
///   `test(=...)` when it sits on one function — or is listed in
///   [`CLOCK_ALONE_EXEMPT`] with a reason;
/// - every binary and test the filter names still carries the exemption, and
///   every pattern in it still matches a file.
///
/// # What it cannot see
///
/// A clock read in a `src/` test module: that runs in the library's own test
/// binary, which is not isolated (ADR-0193's Negative). And a clock read that
/// reaches `Instant::now` without tripping the lint, such as through a helper
/// crate the lint does not see into.
#[test]
fn every_clock_reading_test_is_scheduled_alone() {
    let root = workspace_root();
    let config_path = root.join(".config/nextest.toml");
    let config = std::fs::read_to_string(&config_path)
        .unwrap_or_else(|e| panic!("read {}: {e}", config_path.display()));
    let filter =
        alone_override_filter(&config).unwrap_or_else(|e| panic!(".config/nextest.toml: {e}"));
    let terms = parse_alone_filter(&filter).unwrap_or_else(|e| {
        panic!(
            ".config/nextest.toml: the run-alone override's filter cannot be read by this \
             guard, so it cannot be held to the clippy exemption: {e}"
        )
    });

    let files = integration_test_files(&root);
    let exemptions: Vec<ClockExemption> = files
        .iter()
        .filter_map(|(rel, binary, text)| {
            clock_exemption(rel.clone(), binary.clone(), &strip_line_comments(text))
        })
        .collect();
    assert!(
        !terms.is_empty() && !exemptions.is_empty(),
        "the guard found {} filter terms and {} exempted files, so it is checking nothing",
        terms.len(),
        exemptions.len()
    );

    let names_binary = |binary: &str| {
        terms
            .iter()
            .any(|term| matches!(term, AloneTerm::Binary(pattern) if pattern.matches(binary)))
    };
    let names_test = |binary: &str, test: &str| {
        terms.iter().any(|term| {
            matches!(term, AloneTerm::Test { binary: b, test: t } if b == binary && t == test)
        })
    };
    let override_name =
        "the `threads-required = \"num-test-threads\"` override in .config/nextest.toml";
    let mut failures: Vec<String> = Vec::new();

    for found in &exemptions {
        let exempt = CLOCK_ALONE_EXEMPT.iter().find(|(b, _)| *b == found.binary);
        let named = names_binary(&found.binary)
            || found.functions.iter().any(|f| names_test(&found.binary, f));
        if let Some((_, reason)) = exempt {
            if named {
                failures.push(format!(
                    "{} is in CLOCK_ALONE_EXEMPT ({reason}) and is also named by {override_name}; \
                     remove one of the two",
                    found.rel
                ));
            }
            continue;
        }
        if found.file_level && !names_binary(&found.binary) {
            failures.push(format!(
                "{} carries a file-level `{CLOCK_LINT}` exemption and {override_name} does not \
                 name its binary; add `binary({})` to that filter, or list it in \
                 CLOCK_ALONE_EXEMPT with a reason",
                found.rel, found.binary
            ));
        }
        for function in &found.functions {
            if !names_binary(&found.binary) && !names_test(&found.binary, function) {
                failures.push(format!(
                    "{} exempts `{function}` from `{CLOCK_LINT}` and {override_name} does not \
                     name it; add `(binary({}) & test(={function}))` to that filter, or list \
                     the binary in CLOCK_ALONE_EXEMPT with a reason",
                    found.rel, found.binary
                ));
            }
        }
    }

    for term in &terms {
        match term {
            AloneTerm::Binary(pattern) => {
                let matched: Vec<&(String, String, String)> = files
                    .iter()
                    .filter(|(_, binary, _)| pattern.matches(binary))
                    .collect();
                if matched.is_empty() {
                    failures.push(format!(
                        "{override_name} names `{pattern:?}`, which matches no integration test \
                         binary; remove it"
                    ));
                }
                for (rel, binary, _) in matched {
                    let carries = exemptions
                        .iter()
                        .any(|e| &e.binary == binary && e.file_level);
                    if !carries {
                        failures.push(format!(
                            "{override_name} schedules all of {rel} alone, and it carries no \
                             file-level `{CLOCK_LINT}` exemption; take it out of the filter"
                        ));
                    }
                }
            }
            AloneTerm::Test { binary, test } => {
                let carries = exemptions
                    .iter()
                    .any(|e| &e.binary == binary && e.functions.iter().any(|f| f == test));
                if !carries {
                    failures.push(format!(
                        "{override_name} names `{binary}` test `{test}`, and no integration test \
                         file `{binary}.rs` exempts a function of that name from `{CLOCK_LINT}`; \
                         take it out of the filter"
                    ));
                }
            }
        }
    }

    for (binary, reason) in CLOCK_ALONE_EXEMPT {
        if !exemptions.iter().any(|e| e.binary == *binary) {
            failures.push(format!(
                "CLOCK_ALONE_EXEMPT lists `{binary}` ({reason}), and no integration test file of \
                 that name carries `{CLOCK_LINT}`; remove the entry"
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "the run-alone schedule and the clock exemption disagree (ADR-0193):\n  {}",
        failures.join("\n  ")
    );
}

/// The filter parser reads the forms the override is written in and refuses
/// every other, so a rewrite it cannot follow fails the guard above instead of
/// reading as a filter that names nothing.
#[test]
fn the_run_alone_filter_parser_refuses_what_it_cannot_read() {
    assert_eq!(
        parse_alone_filter("binary(/_cost$/) + binary(help_cli) + (binary(dsp) & test(=one_hop))"),
        Ok(vec![
            AloneTerm::Binary(NamePattern::Suffix("_cost".to_owned())),
            AloneTerm::Binary(NamePattern::Exact("help_cli".to_owned())),
            AloneTerm::Test {
                binary: "dsp".to_owned(),
                test: "one_hop".to_owned(),
            },
        ])
    );
    for unreadable in [
        "binary(/a.*_cost$/)",
        "not binary(help_cli)",
        "binary(help_cli) - test(=x)",
        "binary(help_cli) | binary(stream_pipe)",
        "test(=one_hop)",
        "(binary(dsp) & test(one_hop))",
        "(binary(dsp) & test(=one_hop) & test(=two))",
        "binary(help_cli",
        "",
    ] {
        assert!(
            parse_alone_filter(unreadable).is_err(),
            "`{unreadable}` should be refused, not read"
        );
    }
}

// ---------------------------------------------------------------------------
// (h) A spawned player gets a scratch data root
// ---------------------------------------------------------------------------

/// What a spawn of a workspace binary has to name: the player's cargo variable,
/// or the `shot` example's locator. Only [`SPAWN_HELPER`] may name either.
const SPAWN_NAMES: [&str; 2] = ["CARGO_BIN_EXE_ritmolux", "shot_bin"];

/// The one file under `standalone/tests/` allowed to name [`SPAWN_NAMES`].
const SPAWN_HELPER: &str = "common/mod.rs";

/// Every `.rs` file under `dir`, recursively, sorted.
fn rs_files_under(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(next) = pending.pop() {
        let entries =
            std::fs::read_dir(&next).unwrap_or_else(|e| panic!("read_dir {}: {e}", next.display()));
        for entry in entries {
            let path = entry.expect("dir entry").path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

/// Every line of `text` that names one of [`SPAWN_NAMES`] outside a comment, as
/// `<rel>:<line>: <source>`. Line comments, doc comments included, are stripped
/// first, which keeps line numbers where they were.
fn spawn_sites(rel: &str, text: &str) -> Vec<String> {
    strip_line_comments(text)
        .lines()
        .enumerate()
        .filter(|(_, line)| SPAWN_NAMES.iter().any(|name| line.contains(name)))
        .map(|(at, line)| format!("{rel}:{}: {}", at + 1, line.trim()))
        .collect()
}

/// **No test under `standalone/tests/` spawns the player or `shot` except
/// through `common`.**
///
/// A spawned run that inherits the developer's `APPDATA` (or `HOME`, or
/// `XDG_DATA_HOME`) migrates their app directory, logs to it, binds the control
/// port their config names and reads their presets as input. `common::player`
/// and `common::shot` point all three at a scratch directory; this holds every
/// spawn to them, so the rule does not depend on someone remembering three
/// `.env()` calls.
///
/// # What it cannot see
///
/// A spawn that reaches the binary by a path it builds some other way, such as
/// joining `ritmolux.exe` onto a directory by hand.
#[test]
fn every_spawned_workspace_binary_gets_a_scratch_data_root() {
    let tests = workspace_root().join("standalone").join("tests");
    let files = rs_files_under(&tests);
    for must_scan in ["help_cli.rs", "stream_pipe.rs", "shot_cli.rs"] {
        assert!(
            files.iter().any(|f| f.ends_with(must_scan)),
            "the scan no longer reaches `{must_scan}`, so it is guarding less than it reads"
        );
    }

    let helper_path = tests.join(SPAWN_HELPER);
    let helper = std::fs::read_to_string(&helper_path)
        .unwrap_or_else(|e| panic!("read {}: {e}", helper_path.display()));
    let helper_sites = spawn_sites(SPAWN_HELPER, &helper);
    for name in SPAWN_NAMES {
        assert!(
            helper_sites.iter().any(|site| site.contains(name)),
            "standalone/tests/{SPAWN_HELPER} no longer names `{name}`, so this guard watches \
             for a name no spawn uses; follow the helper's rename here"
        );
    }

    let mut hits = Vec::new();
    for file in &files {
        let rel = file
            .strip_prefix(&tests)
            .expect("scanned file is under standalone/tests")
            .to_string_lossy()
            .replace('\\', "/");
        if rel == SPAWN_HELPER {
            continue;
        }
        let text = std::fs::read_to_string(file)
            .unwrap_or_else(|e| panic!("read {}: {e}", file.display()));
        hits.extend(spawn_sites(&format!("standalone/tests/{rel}"), &text));
    }
    assert!(
        hits.is_empty(),
        "a test spawns a workspace binary outside standalone/tests/{SPAWN_HELPER}, so the \
         child reads and writes the developer's real per-user directory:\n  {}\n\
         Build the command with `common::player()`, `common::player_with_data_root(..)` or \
         `common::shot()` instead.",
        hits.join("\n  ")
    );
}

/// The spawn guard names a bare spawn by file and line, and passes a comment
/// that merely mentions the variable.
#[test]
fn the_spawn_guard_names_a_bare_spawn_and_ignores_prose() {
    let bare = "fn run(args: &[&str], stall_after: Option<usize>) -> Option<Run> {\n    \
                let started = Instant::now();\n    \
                let mut child = Command::new(env!(\"CARGO_BIN_EXE_ritmolux\"))\n        \
                .args(args)\n";
    assert_eq!(
        spawn_sites("standalone/tests/stream_pipe.rs", bare),
        vec![
            "standalone/tests/stream_pipe.rs:3: \
             let mut child = Command::new(env!(\"CARGO_BIN_EXE_ritmolux\"))"
                .to_owned()
        ]
    );

    let prose = "//! `ritmolux` is a `[[bin]]`, so `CARGO_BIN_EXE_ritmolux` resolves it.\n\
                 /// The locator was `shot_bin`.\n\
                 let output = common::player().args(args);\n";
    assert!(
        spawn_sites("standalone/tests/help_cli.rs", prose).is_empty(),
        "a comment naming the variable is not a spawn"
    );
}
