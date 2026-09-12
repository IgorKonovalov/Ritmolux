//! `ritmolux --check`, run the way an author and the gate run it (ADR-0190).
//!
//! Two things are under test here and they need different machinery:
//!
//!   1. **The flag answers and exits, with the right code and the right
//!      position.** That is only observable from outside the process — the exit
//!      code is the contract, and the absence of a window is the property
//!      `help_cli.rs` exists for on the other early-exit flags. So these cases
//!      spawn the built binary.
//!   2. **The library agrees with the engine over the whole corpus.** That is
//!      the gate, and it calls `standalone::preset_check` directly: one process
//!      per file would spend minutes to learn what one pass over the same
//!      loader learns in seconds.
//!
//! GPU-free by construction: `--check` exits before a renderer exists, and the
//! library path never had one. Both run on an adapterless runner.
//!
//! The failing fixtures are **written to a scratch directory**, never committed
//! under `presets/`: the gate reads those directories as they are on disk, so a
//! deliberately broken file parked in one would turn the gate red for everyone.

use std::path::{Path, PathBuf};
use std::process::Command;

use standalone::preset_check::{self, Severity};

/// Run `ritmolux` with `args`; exit code, stdout, stderr.
fn run(args: &[&str]) -> (Option<i32>, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_ritmolux"))
        .args(args)
        .output()
        .expect("failed to spawn the ritmolux binary");
    (
        output.status.code(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// A per-process scratch directory under `CARGO_TARGET_TMPDIR`, which cargo
/// points at the target directory it is actually writing to — unlike
/// `CARGO_MANIFEST_DIR`, which names the source tree and would put fixtures in
/// the worktree the gate below reads (ADR-0147).
fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("preset-check-tests")
        .join(format!("{}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    dir
}

/// Write `body` as `<scratch>/<name>.toml` and return its path.
fn fixture(name: &str, body: &str) -> PathBuf {
    let path = scratch(name).join(format!("{name}.toml"));
    std::fs::write(&path, body).expect("write fixture");
    path
}

/// The repository root, from this crate's manifest rather than the cwd — a
/// test's working directory is not a contract.
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("standalone has a workspace-root parent")
        .to_path_buf()
}

// ---------------------------------------------------------------------------
// The flag, from outside the process
// ---------------------------------------------------------------------------

/// **The shipped library passes its own checker.** `core/tests/preset.rs` holds
/// the embedded set to zero warnings through the loader, so any line printed
/// here is this checker misreporting rather than a preset being wrong — which is
/// what makes a bare exit-0 the right assertion and a printed line the failure.
#[test]
fn the_shipped_presets_check_clean_with_nothing_to_say() {
    let presets = repo_root().join("presets");
    let (code, stdout, stderr) = run(&["--check", &presets.to_string_lossy()]);
    assert_eq!(code, Some(0), "`--check presets` did not exit 0: {stderr}");
    assert!(
        stdout.is_empty(),
        "`--check presets` printed diagnostics on a tree the core suite holds \
         clean, so the checker is misreporting:\n{stdout}"
    );
    // The summary is the only thing on stderr, and it says what was read: a run
    // that resolved zero files would otherwise be indistinguishable from a
    // clean one.
    assert!(
        stderr.contains("checked 112 files") || stderr.contains(" files: 0 errors, 0 warnings"),
        "the summary does not report the files it read: {stderr}"
    );
}

/// **A non-compiling binding is an error on the line of its key.** The author's
/// whole reason for the flag: the loader names the parameter and nothing else,
/// and the position is recovered by looking that name up in a spanned re-parse.
#[test]
fn a_binding_that_does_not_compile_is_an_error_at_its_key() {
    let path = fixture(
        "bad-binding",
        "# A fixture whose binding does not compile.\n\
         system = \"fragment_field\"\n\
         \n\
         [params]\n\
         zoom = \"1.0\"\n\
         glow = \"bass *\"\n",
    );
    let (code, stdout, stderr) = run(&["--check", &path.to_string_lossy()]);
    assert_eq!(code, Some(1), "a load error must exit 1: {stderr}");
    let line = stdout.lines().next().unwrap_or_default();
    // Line 6 is `glow`, line 5 is the well-formed `zoom` above it: the position
    // has to single out the failing binding rather than the table.
    assert!(
        positioned_at(line, &path, "6:1", "error"),
        "the diagnostic is not on the failing binding's key: {line}"
    );
    assert!(
        line.contains("error[engine]") && line.contains("glow"),
        "the diagnostic does not name the severity, the rule and the parameter: {line}"
    );
    assert_eq!(
        stdout.lines().count(),
        1,
        "one diagnostic must be one line, so a `file:line:col` matcher can read \
         it:\n{stdout}"
    );
}

/// **A bare number is an error on the value, from the TOML span.** The other
/// half of the positioning story: here the loader *does* carry a range, and the
/// checker must use it rather than fall back to the file level.
#[test]
fn a_bare_number_is_an_error_at_the_value() {
    let path = fixture(
        "bare-number",
        "# A fixture whose value is a bare number.\n\
         system = \"fragment_field\"\n\
         \n\
         [params]\n\
         zoom = \"1.0\"\n\
         glow = 1.0\n",
    );
    let (code, stdout, stderr) = run(&["--check", &path.to_string_lossy()]);
    assert_eq!(code, Some(1), "a load error must exit 1: {stderr}");
    let line = stdout.lines().next().unwrap_or_default();
    // Column 8 is the `1.0`, not the `glow`: a TOML type error is about the
    // value, and the span the parser gave is what says so.
    assert!(
        positioned_at(line, &path, "6:8", "error"),
        "the diagnostic is not on the offending value: {line}"
    );
    assert!(
        line.contains("error[engine]"),
        "the diagnostic is not an engine error: {line}"
    );
    assert_eq!(
        stdout.lines().count(),
        1,
        "the TOML error's multi-line snippet has leaked into the output:\n{stdout}"
    );
}

/// **A misspelled parameter warns, and only `--strict` makes it cost an exit
/// code.** This is the class ADR-0020 deliberately forgives, so the checker must
/// report it without changing the loader's verdict: the preset still loads.
#[test]
fn a_misspelled_parameter_warns_and_only_strict_fails() {
    let path = fixture(
        "typo",
        "# A fixture with a misspelled parameter.\n\
         system = \"fragment_field\"\n\
         \n\
         [params]\n\
         glwo = \"1.0\"\n",
    );
    let arg = path.to_string_lossy().into_owned();

    let (code, stdout, stderr) = run(&["--check", &arg]);
    assert_eq!(
        code,
        Some(0),
        "a warning alone must not fail an unstrict run: {stderr}"
    );
    let line = stdout.lines().next().unwrap_or_default();
    assert!(
        line.contains("warning[engine]") && line.contains("glwo"),
        "the diagnostic does not name the severity, the rule and the typo: {line}"
    );
    // File level, because `Preset::warnings` is a `Vec<String>` with nothing
    // positional in it. The editor schema is what puts a real position on this
    // class (ADR-0190); when the loader's warnings gain structure this moves.
    assert!(
        positioned_at(line, &path, "1:1", "warning"),
        "a warning is reported at the file level until the loader's warnings \
         carry a position: {line}"
    );

    let (strict_code, strict_stdout, _) = run(&["--check", &arg, "--strict"]);
    assert_eq!(
        strict_code,
        Some(1),
        "`--strict` must make the same warning cost an exit code"
    );
    assert_eq!(
        strict_stdout, stdout,
        "`--strict` changes the exit code and nothing about what is reported"
    );
}

/// **A path naming nothing is exit 2, the code this binary uses for an argument
/// list wrong in shape.** A missing file is the operator's spelling rather than
/// a preset that failed, and conflating the two would tell a gate that a typo in
/// its own path was a red preset.
#[test]
fn a_missing_path_is_a_usage_failure() {
    let missing = scratch("missing").join("no-such-file.toml");
    let (code, stdout, stderr) = run(&["--check", &missing.to_string_lossy()]);
    assert_eq!(code, Some(2), "a missing path must exit 2: {stderr}");
    assert!(
        stdout.is_empty(),
        "a usage failure must not print diagnostics: {stdout}"
    );
    assert!(
        stderr.contains("no-such-file.toml"),
        "the refusal does not name the path that was given: {stderr}"
    );
}

/// **`--strict` alone is refused, before anything is read.** It is rostered with
/// `requires: Some("--check")`, so the companion gate (ADR-0155) is what catches
/// it — and this is the case that proves the roster entry is wired rather than
/// merely written.
#[test]
fn strict_without_check_is_refused() {
    let (code, _, stderr) = run(&["--strict"]);
    assert_eq!(code, Some(2), "`--strict` alone must be refused: {stderr}");
    assert!(
        stderr.contains("--check"),
        "the refusal does not name the missing companion: {stderr}"
    );
}

/// **A directory is read non-recursively.** The convention `core/build.rs`
/// already follows for the embedded set (ADR-0022): a subdirectory is held back
/// by construction, so `presets/pending/` is not swept into a check of
/// `presets/`. The gate below is what covers a nested tree, by walking it
/// itself.
#[test]
fn a_directory_is_not_walked_recursively() {
    let dir = scratch("nested");
    let nested = dir.join("inner");
    std::fs::create_dir_all(&nested).expect("create nested dir");
    // Clean at the top, broken one level down. A recursive read would report
    // the error and exit 1.
    std::fs::write(
        dir.join("top.toml"),
        "# Clean.\nsystem = \"fragment_field\"\n",
    )
    .expect("write top fixture");
    std::fs::write(nested.join("deep.toml"), "# Broken.\nsystem = \"nope\"\n")
        .expect("write nested fixture");

    let (code, stdout, stderr) = run(&["--check", &dir.to_string_lossy()]);
    assert_eq!(code, Some(0), "a subdirectory was read: {stdout}{stderr}");
    assert!(stdout.is_empty(), "a subdirectory was read: {stdout}");
    assert!(
        stderr.contains("checked 1 file"),
        "the summary does not report exactly the top-level file: {stderr}"
    );
}

/// **An empty directory is not a failure.** `presets/pending/` holds nothing
/// whenever nothing is waiting on an engine gap, and the gate reads it every
/// run — so a refusal here would be red on a correct tree. The summary reporting
/// `0 files` is what keeps that from being a silent pass.
#[test]
fn an_empty_directory_reports_zero_files_and_exits_zero() {
    let dir = scratch("empty-dir");
    let (code, stdout, stderr) = run(&["--check", &dir.to_string_lossy()]);
    assert_eq!(code, Some(0), "an empty directory must not fail: {stderr}");
    assert!(stdout.is_empty());
    assert!(
        stderr.contains("checked 0 files"),
        "an empty run must say it read nothing: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// The library, over the corpus
// ---------------------------------------------------------------------------

/// **Every preset the repository tracks passes the checker.** The whole reason
/// the checker is a library module: this reads `presets/`, `presets/proposed/`,
/// `presets/pending/` and `docs/examples/` through one loader pass rather than
/// 127 process starts.
#[test]
fn the_whole_corpus_is_free_of_errors() {
    let mut failed: Vec<String> = Vec::new();
    let mut checked = 0usize;
    for file in corpus() {
        let src = std::fs::read_to_string(&file)
            .unwrap_or_else(|e| panic!("read {}: {e}", file.display()));
        checked += 1;
        for diagnostic in preset_check::check(&file, &src) {
            if diagnostic.severity == Severity::Error {
                failed.push(diagnostic.render(&file, &src));
            }
        }
    }
    assert!(
        checked >= 100,
        "the corpus walk found only {checked} files, which means it stopped \
         reading the tree rather than that the library shrank"
    );
    assert!(
        failed.is_empty(),
        "these presets do not load:\n{}",
        failed.join("\n")
    );
}

/// Every tracked preset, across the four directories.
///
/// `docs/examples/` is walked **recursively** while the `--check` CLI reads a
/// directory flat: fourteen of the fifteen examples live in a subdirectory named
/// for what it teaches (`tuning/`, `curves/`), and a gate that read only the top
/// level would cover one of them. The preset directories are flat and are read
/// flat, which is what keeps `presets/pending/` out of a walk of `presets/`.
fn corpus() -> Vec<PathBuf> {
    let root = repo_root();
    let mut files = Vec::new();
    for dir in ["presets", "presets/proposed", "presets/pending"] {
        files.extend(flat_tomls(&root.join(dir)));
    }
    collect_tomls(&root.join("docs/examples"), &mut files);
    files.sort();
    files
}

/// The `*.toml` directly in `dir`, through the same resolver the CLI uses — so
/// the gate cannot disagree with `--check <dir>` about which files those are.
fn flat_tomls(dir: &Path) -> Vec<PathBuf> {
    preset_check::targets(dir).unwrap_or_else(|e| panic!("resolve {}: {e}", dir.display()))
}

/// Every `*.toml` at or below `dir`.
fn collect_tomls(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = std::fs::read_dir(dir).unwrap_or_else(|e| panic!("read {}: {e}", dir.display()));
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if path.is_dir() {
            collect_tomls(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "toml") {
            out.push(path);
        }
    }
}

/// Whether `line` opens with exactly `<path>:<line>:<col>: <severity>[`.
///
/// An exact prefix rather than a `contains`: the rendered path carries colons of
/// its own on Windows (`C:\...`), and a substring test for `:6:1:` would also be
/// satisfied by `:16:1:` — so the assertion that matters most here would pass on
/// a diagnostic ten lines away from the fixture's mistake.
fn positioned_at(line: &str, path: &Path, position: &str, severity: &str) -> bool {
    line.starts_with(&format!("{}:{position}: {severity}[", path.display()))
}
