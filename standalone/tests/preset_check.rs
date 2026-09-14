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
    // Line 5 is `glwo`: the warning names its binding (ADR-0192), and the
    // binding is placed the way an error naming one is.
    assert!(
        positioned_at(line, &path, "5:1", "warning"),
        "the warning is not on the misspelled binding's key: {line}"
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

/// **Every warning that names a binding lands on that binding's key, and one
/// that names none stays at the file level.** Walked through the library over
/// one fixture per labelled class, so a label the loader spells in a way
/// `--check` cannot peel back into a key path fails here rather than quietly
/// falling back to `1:1`.
#[test]
fn a_warning_lands_on_the_binding_it_names_and_an_unanchored_one_at_the_file() {
    // (what, source, the key the warning belongs on, `None` for file level)
    let cases: [(&str, &str, Option<&str>); 7] = [
        (
            "an undeclared binding",
            "# x\nsystem = \"fragment_field\"\n[params]\nzoom = \"1\"\nglwo = \"1\"\n",
            Some("glwo"),
        ),
        (
            "an undeclared layer binding",
            "# x\nsystem = \"swarm\"\n[params]\nforce = \"1\"\n\
             [layer]\nsystem = \"fragment_field\"\n[layer.params]\nglwo = \"1\"\n",
            Some("glwo"),
        ),
        (
            "a resting dead-zone value",
            "# x\nsystem = \"parametric_curve\"\n[params]\nn = \"3\"\nthickness = \"0.016\"\n",
            Some("thickness"),
        ),
        (
            "an unknown per-vertex binding",
            "# x\nsystem = \"warp_mesh\"\n[per_vertex]\nrot = \"x\"\nwobble = \"x\"\n",
            Some("wobble"),
        ),
        (
            "a layer vertex-variable reach",
            "# x\nsystem = \"fragment_field\"\n[params]\nzoom = \"0.6\"\n\
             [layer]\nsystem = \"warp_mesh\"\n[layer.params]\nwarp = \"1\"\nzoom = \"rad\"\n",
            Some("zoom"),
        ),
        (
            "an inert hold entry",
            "# x\nsystem = \"parametric_curve\"\n[params]\nn = \"3\"\n[hold]\nd = \"bar\"\n",
            None,
        ),
        (
            "a blend on an under join",
            "# x\nsystem = \"swarm\"\n[params]\nforce = \"1\"\n\
             [layer]\nsystem = \"swarm\"\nblend = \"screen\"\n",
            None,
        ),
    ];
    for (what, src, key) in cases {
        let path = Path::new("fixture.toml");
        let warnings: Vec<_> = preset_check::check(path, src)
            .into_iter()
            .filter(|d| d.rule == preset_check::RULE_ENGINE && d.severity == Severity::Warning)
            .collect();
        assert_eq!(
            warnings.len(),
            1,
            "{what}: expected one engine warning, got {warnings:?}"
        );
        let span = warnings[0].span.clone();
        match key {
            // The LAST occurrence of `<key> =`, which each fixture arranges to be
            // the binding the warning is about: a same-named key earlier in the
            // file is the case a wrong key path would land on instead.
            Some(key) => {
                let at = src
                    .rfind(&format!("\n{key} ="))
                    .map(|i| i + 1)
                    .expect("the fixture has the key");
                assert_eq!(
                    span,
                    Some(at..at + key.len()),
                    "{what}: the warning is not on the `{key}` binding: {}",
                    warnings[0].render(path, src)
                );
            }
            None => assert_eq!(
                span,
                None,
                "{what}: a warning about no single binding was placed: {}",
                warnings[0].render(path, src)
            ),
        }
    }
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
// The gate, over the corpus
// ---------------------------------------------------------------------------

/// Where a directory's warnings are binding, and where they are only reported.
///
/// **`presets/pending/` is the one exception, and it is not a loophole.** It
/// holds content that is authored and approved and held back by a known engine
/// or harness gap — which is exactly the state in which a preset may legitimately
/// trip a loader warning, since the parameter it binds may be the thing that does
/// not exist yet. Every other directory holds finished content, so a warning
/// there is a defect. Errors are binding everywhere, `pending/` included: nothing
/// is approved in a state where it does not load.
const DIRECTORIES: [(&str, Warnings); 4] = [
    ("presets", Warnings::Fail),
    ("presets/proposed", Warnings::Fail),
    ("presets/pending", Warnings::Report),
    ("docs/examples", Warnings::Fail),
];

/// What a warning costs in one directory.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Warnings {
    Fail,
    Report,
}

/// **The gate.** Every tracked preset in the four directories loads, and every
/// one outside `presets/pending/` has nothing said about it.
///
/// The whole reason the checker is a library module: this reads all four
/// directories through one loader pass rather than 127 process starts, so it is
/// cheap enough to be an ordinary member of the test run — which is what makes
/// `.githooks/pre-push` and CI the gate without either of them naming it.
#[test]
fn the_corpus_holds_to_the_engine_and_to_the_house_style() {
    let root = repo_root();
    let mut errors: Vec<String> = Vec::new();
    let mut binding_warnings: Vec<String> = Vec::new();
    let mut tolerated: Vec<String> = Vec::new();
    let mut checked = 0usize;

    for (dir, policy) in DIRECTORIES {
        for file in files_in(&root.join(dir)) {
            let src = std::fs::read_to_string(&file)
                .unwrap_or_else(|e| panic!("read {}: {e}", file.display()));
            checked += 1;
            for diagnostic in preset_check::check(&file, &src) {
                let line = diagnostic.render(&file, &src);
                match (diagnostic.severity, policy) {
                    (Severity::Error, _) => errors.push(line),
                    (Severity::Warning, Warnings::Fail) => binding_warnings.push(line),
                    (Severity::Warning, Warnings::Report) => tolerated.push(line),
                }
            }
        }
    }

    // A walk that stopped reading the tree would otherwise pass by finding
    // nothing, which is the one way a corpus gate fails quietly.
    assert!(
        checked >= 100,
        "the corpus walk found only {checked} files, which means it stopped \
         reading the tree rather than that the library shrank"
    );

    // Printed, never asserted: `pending/` is where a warning is allowed to
    // stand, and a reader of a green run should still see what is standing.
    for line in &tolerated {
        eprintln!("tolerated in presets/pending/: {line}");
    }

    assert!(
        errors.is_empty(),
        "these presets do not load:\n{}",
        errors.join("\n")
    );
    assert!(
        binding_warnings.is_empty(),
        "{} warning(s) in directories that hold finished content:\n{}\n\
         The content belongs to the `preset-author` lane and the policy to \
         `architect`: fix the preset, or route the rule — do not widen the \
         tolerated set.",
        binding_warnings.len(),
        binding_warnings.join("\n")
    );
}

/// Every `*.toml` the gate reads under `dir`.
///
/// `docs/examples/` is walked **recursively** while the `--check` CLI reads a
/// directory flat: fourteen of the fifteen examples live in a subdirectory named
/// for what it teaches (`tuning/`, `curves/`), and a gate that read only the top
/// level would cover one of them. The three preset directories are flat, and are
/// read through the CLI's own resolver so the gate cannot disagree with
/// `--check <dir>` about which files those are — which is also what keeps
/// `presets/pending/` out of a walk of `presets/`.
fn files_in(dir: &Path) -> Vec<PathBuf> {
    if dir.ends_with("examples") {
        let mut out = Vec::new();
        collect_tomls(dir, &mut out);
        out.sort();
        return out;
    }
    flat_tomls(dir)
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

// ---------------------------------------------------------------------------
// The house-style rules, one fixture and one near-miss each
// ---------------------------------------------------------------------------

/// Write `body` as `<scratch>/<dir>/<name>` and return its path.
///
/// `dir` matters: the two naming rules apply only to a **library** file, which is
/// one whose parent directory is `presets/`, `presets/proposed/` or
/// `presets/pending/`. So a fixture for those rules has to sit in a directory
/// spelled that way, and a fixture for the exemption has to sit somewhere else.
fn file_in(dir: &str, name: &str, body: &str) -> PathBuf {
    let parent = scratch("rules").join(dir);
    std::fs::create_dir_all(&parent).expect("create rule fixture dir");
    let path = parent.join(name);
    std::fs::write(&path, body).expect("write rule fixture");
    path
}

/// The rules that fired on a fixture, by id.
fn rules_on(path: &Path) -> Vec<&'static str> {
    let src = std::fs::read_to_string(path).expect("read rule fixture");
    preset_check::check(path, &src)
        .into_iter()
        .map(|diagnostic| diagnostic.rule)
        .collect()
}

/// Assert `rule` fired on `tripped` and did not fire on `clean`.
///
/// The near-miss is the half that matters: a rule with only a positive case
/// passes just as well when it fires on everything, which would turn the gate
/// into noise the first time a correct preset tripped it.
fn rule_separates(rule: &'static str, tripped: &Path, clean: &Path) {
    assert!(
        rules_on(tripped).contains(&rule),
        "`{rule}` did not fire on {}",
        tripped.display()
    );
    assert!(
        !rules_on(clean).contains(&rule),
        "`{rule}` fired on {}, which does not trip it",
        clean.display()
    );
}

/// **`file-name`**: the prefix up to the first `_` is the system's family.
///
/// The family, not any segment of the system name: `shape_x` declaring
/// `shape_collage` shares the segment `shape` and would still be handed
/// `shape_field`'s editor schema, so it fails; `collage_x` passes. A prefix naming
/// an unrelated system (`swarm_x` declaring `attractor`) fails too, and an
/// unknown `system` is left to the loader's error.
#[test]
fn the_file_name_rule_reads_the_prefix_against_the_systems_family() {
    let clean = file_in(
        "presets",
        "collage_x.toml",
        "# A collage.\nsystem = \"shape_collage\"\n",
    );
    let shares_a_segment = file_in(
        "presets",
        "shape_x.toml",
        "# A collage named for the shape field.\nsystem = \"shape_collage\"\n",
    );
    let unrelated = file_in(
        "presets",
        "swarm_x.toml",
        "# A swarm that is not one.\nsystem = \"attractor\"\n",
    );
    rule_separates("file-name", &shares_a_segment, &clean);
    rule_separates("file-name", &unrelated, &clean);

    let src = std::fs::read_to_string(&shares_a_segment).expect("read rule fixture");
    let message = preset_check::check(&shares_a_segment, &src)
        .into_iter()
        .find(|d| d.rule == "file-name")
        .map(|d| d.message)
        .unwrap_or_default();
    assert!(
        message.contains("`collage`"),
        "the diagnostic does not name the family the file should carry: {message}"
    );

    let unknown = file_in(
        "presets",
        "shape_unknown.toml",
        "# No such system.\nsystem = \"no_such_system\"\n",
    );
    assert!(
        !rules_on(&unknown).contains(&"file-name"),
        "`file-name` fired on an unknown system, which is the loader's error to report"
    );
}

/// **`file-name` and `header-comment` apply to library files only.**
/// `docs/examples/` is exempt: its files are named for what they teach
/// (`step-1-constants.toml`) and `minimal.toml` is bare of a header on purpose,
/// so applying either rule there would convict the teaching material for
/// teaching.
#[test]
fn the_two_naming_rules_do_not_reach_outside_the_library() {
    // The same content that trips both rules inside `presets/`.
    let body = "system = \"attractor\"\n";
    let in_library = file_in("presets", "swarm_exempt.toml", body);
    let in_examples = file_in("examples", "swarm_exempt.toml", body);

    let library_rules = rules_on(&in_library);
    assert!(
        library_rules.contains(&"file-name") && library_rules.contains(&"header-comment"),
        "the fixture does not trip both naming rules inside presets/: {library_rules:?}"
    );
    let example_rules = rules_on(&in_examples);
    assert!(
        !example_rules.contains(&"file-name") && !example_rules.contains(&"header-comment"),
        "a naming rule reached outside the library: {example_rules:?}"
    );
}

/// **`header-comment`**: the file opens with a `#` comment before its first key.
#[test]
fn the_header_comment_rule_wants_a_comment_before_the_first_key() {
    let tripped = file_in(
        "presets",
        "collage_noheader.toml",
        "system = \"shape_collage\"\n",
    );
    let clean = file_in(
        "presets",
        "collage_header.toml",
        "# What this preset is for.\nsystem = \"shape_collage\"\n",
    );
    rule_separates("header-comment", &tripped, &clean);
}

/// **`hex-case`**: a `#rrggbb` colour is lowercase.
///
/// The near-miss here is doing double duty. `#ddeeff` in the same shape must not
/// fire — and `presets/collage_suprematist.toml` names `#E2E0DA` inside a prose
/// comment, which is why the rule walks the parsed document rather than the
/// lines. A line matcher would convict that file for discussing a colour it does
/// not set, so the third fixture is a comment.
#[test]
fn the_hex_case_rule_reads_colour_values_and_not_comments() {
    let stops = |colour: &str| {
        format!(
            "# Colours.\nsystem = \"shape_collage\"\n\n\
             [[palette.stops]]\nat = 0.0\ncolor = \"{colour}\"\n\n\
             [[palette.stops]]\nat = 1.0\ncolor = \"#112233\"\n"
        )
    };
    let tripped = file_in("presets", "collage_upper.toml", &stops("#AABBCC"));
    let clean = file_in("presets", "collage_lower.toml", &stops("#ddeeff"));
    rule_separates("hex-case", &tripped, &clean);

    let in_a_comment = file_in(
        "presets",
        "collage_comment.toml",
        "# The plateau renders `#E2E0DA`, which is the exception.\n\
         system = \"shape_collage\"\n",
    );
    assert!(
        !rules_on(&in_a_comment).contains(&"hex-case"),
        "an uppercase colour named in a comment was convicted; the rule has \
         stopped reading the document and started reading lines"
    );
}

/// **`trailing-whitespace`**: no line ends in a space or a tab.
#[test]
fn the_trailing_whitespace_rule_reads_the_end_of_each_line() {
    let tripped = file_in(
        "presets",
        "collage_trail.toml",
        "# Trailing.\nsystem = \"shape_collage\"   \n",
    );
    let clean = file_in(
        "presets",
        "collage_notrail.toml",
        "# Clean.\nsystem = \"shape_collage\"\n",
    );
    rule_separates("trailing-whitespace", &tripped, &clean);
}

/// **`final-newline`**: the file ends in exactly one newline.
///
/// Two opposite mistakes, and both are the rule's: none at all, and more than
/// one. A CRLF ending is **not** either of them — `.gitattributes` checks
/// `*.toml` out as LF, and a clone that produced CRLF anyway must not fail every
/// file in the tree.
#[test]
fn the_final_newline_rule_wants_exactly_one() {
    let clean = file_in(
        "presets",
        "collage_onenl.toml",
        "# One.\nsystem = \"shape_collage\"\n",
    );
    let none = file_in(
        "presets",
        "collage_nonl.toml",
        "# None.\nsystem = \"shape_collage\"",
    );
    let two = file_in(
        "presets",
        "collage_twonl.toml",
        "# Two.\nsystem = \"shape_collage\"\n\n",
    );
    rule_separates("final-newline", &none, &clean);
    rule_separates("final-newline", &two, &clean);

    let crlf = file_in(
        "presets",
        "collage_crlf.toml",
        "# CRLF.\r\nsystem = \"shape_collage\"\r\n",
    );
    let rules = rules_on(&crlf);
    assert!(
        !rules.contains(&"final-newline") && !rules.contains(&"trailing-whitespace"),
        "a CRLF checkout is convicted by the whitespace rules: {rules:?}"
    );
}

/// **`tab`**: no tab character anywhere.
#[test]
fn the_tab_rule_reads_the_whole_line() {
    let tripped = file_in(
        "presets",
        "collage_tab.toml",
        "# Tabbed.\nsystem = \"shape_collage\"\n\n[params]\n\tcount = \"8\"\n",
    );
    let clean = file_in(
        "presets",
        "collage_spaces.toml",
        "# Spaced.\nsystem = \"shape_collage\"\n\n[params]\ncount = \"8\"\n",
    );
    rule_separates("tab", &tripped, &clean);
}

/// **Every rule the module names has a case above.** A rule added to
/// `HOUSE_RULES` and to nothing else is a rule nobody has shown to fire, and the
/// gate would carry it over the whole corpus on trust.
#[test]
fn every_house_rule_is_covered_by_a_fixture() {
    // The rule ids the cases above assert on, each through `rule_separates` or
    // a direct `contains`.
    const COVERED: [&str; 6] = [
        "file-name",
        "header-comment",
        "hex-case",
        "trailing-whitespace",
        "final-newline",
        "tab",
    ];
    let missing: Vec<&str> = preset_check::HOUSE_RULES
        .into_iter()
        .filter(|rule| !COVERED.contains(rule))
        .collect();
    assert!(
        missing.is_empty(),
        "these house-style rules have no fixture in this file: {missing:?}"
    );
    let stale: Vec<&str> = COVERED
        .into_iter()
        .filter(|rule| !preset_check::HOUSE_RULES.contains(rule))
        .collect();
    assert!(
        stale.is_empty(),
        "these rule ids are asserted here and no longer exist: {stale:?}"
    );
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
