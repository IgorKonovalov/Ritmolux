//! The `shot` CLI, run the way a user runs it (Plan 0032 Phase 2, ADR-0033 tier 4).
//!
//! `standalone/examples/shot.rs` is the headless-capture CLI the `preset-author`
//! lane self-verifies drafts through and the Plan 0013 visual-QA harness is built
//! on — and it had **no tests of any kind**, because `#[test]` does not run in an
//! `examples/` target. Moving its pure helpers into the `[lib]` (Plan 0031 Phase 1)
//! makes them assertable in-process but still leaves argument parsing, preset
//! resolution, exit codes, and file output unexercised. Those are only observable
//! from outside, so this suite spawns the built binary as a subprocess.
//!
//! `shot` is deliberately **not** promoted to a `[[bin]]` so `CARGO_BIN_EXE_shot`
//! would resolve it (ADR-0033 Alternative E): `image` is a dev-dependency precisely
//! to keep the PNG codec out of the shipped `lmv.exe`, and a `[[bin]]` does not get
//! dev-dependencies. It would also rename every documented invocation, including
//! ones in `.claude/skills/**` that cannot be edited. So the binary is located
//! under `target/<profile>/examples/` instead.
//!
//! `cargo nextest run` **does** build `examples/` targets — verified for this plan
//! by deleting `target/debug/examples/shot.exe` and watching `cargo nextest run -p
//! standalone` rebuild it — so neither CI nor the pre-push hook needs an explicit
//! build step. If the binary is missing anyway the tests fail with an actionable
//! message rather than passing silently.
//!
//! The GPU-free cases all exit before a renderer is constructed, so they run
//! everywhere. The rendering cases need a real adapter (`shot` asks for hardware,
//! not WARP) and **skip with a printed reason** where none exists — macOS has no
//! software Metal fallback (ADR-0016), and CI runners generally have no GPU. The
//! skip is keyed on the adapter error itself rather than on the OS, so an
//! adapterless Windows runner is handled too and any *other* failure still fails.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// Substring of the error `shot` prints when no GPU adapter can be acquired
/// (`RenderError::RequestAdapter` -> "no suitable GPU adapter: ..."). Matching the
/// adapter failure specifically means an unrelated non-zero exit is never mistaken
/// for a skip.
const NO_ADAPTER: &str = "no suitable GPU adapter";

/// A shipped preset used for the library-resolution cases. If it is ever renamed,
/// these tests fail with a message saying so rather than something cryptic.
const SHIPPED_PRESET_FILE: &str = "presets/fragment_aurora.toml";
const SHIPPED_PRESET_NAME: &str = "Aurora";

/// The repo root — `shot` is invoked from here so `presets` resolves and the
/// provenance label reads exactly `[--presets presets]`.
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("standalone/ has a parent")
        .to_path_buf()
}

/// Locate `target/<profile>/examples/shot[.exe]` by walking up from this test
/// binary. The test lives at `target/<profile>/deps/shot_cli-<hash>`, so the
/// `examples/` sibling is one or two levels up — searching the ancestors instead of
/// hardcoding the depth keeps this working under `CARGO_TARGET_DIR` and under a
/// `--target <triple>` layout, neither of which puts `target/debug` where you would
/// guess.
fn shot_bin() -> PathBuf {
    let exe = std::env::current_exe().expect("test binary has a path");
    let name = format!("shot{}", std::env::consts::EXE_SUFFIX);
    for dir in exe.ancestors().skip(1).take(4) {
        let candidate = dir.join("examples").join(&name);
        if candidate.is_file() {
            return candidate;
        }
    }
    panic!(
        "could not find the `shot` example next to {}; run \
         `cargo build -p standalone --example shot` first",
        exe.display()
    );
}

fn run(args: &[&str]) -> Output {
    Command::new(shot_bin())
        .args(args)
        .current_dir(repo_root())
        .output()
        .expect("spawning the shot binary")
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

/// A unique scratch directory for output files, under `target/` so it never
/// escapes the build tree. Keyed by process id so concurrent test binaries do not
/// collide.
fn scratch(name: &str) -> PathBuf {
    let dir = repo_root()
        .join("target")
        .join("shot-cli-tests")
        .join(format!("{}-{}", std::process::id(), name));
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    dir
}

/// `true` when the run failed only because the machine has no usable GPU adapter,
/// in which case the caller prints a skip. Any other failure is a real one.
fn skipped_for_no_adapter(out: &Output) -> bool {
    if out.status.success() {
        return false;
    }
    if stderr(out).contains(NO_ADAPTER) {
        eprintln!("skipped: no GPU adapter on this runner (ADR-0016)");
        return true;
    }
    false
}

/// Assert a run failed, and that its stderr names the thing that was wrong.
fn assert_failed_naming(out: &Output, needle: &str, case: &str) {
    assert!(
        !out.status.success(),
        "{case}: expected a non-zero exit, got success\nstdout: {}",
        stdout(out)
    );
    let err = stderr(out);
    assert!(
        err.contains(needle),
        "{case}: stderr does not name `{needle}`\nstderr: {err}"
    );
}

// ---------------------------------------------------------------------------
// GPU-free cases — these exit before a renderer is built, so they run everywhere
// ---------------------------------------------------------------------------

/// Every bad-input path exits non-zero *and* says which input was bad. An agent
/// driving this CLI (the `preset-author` lane does) needs the failure to be
/// actionable, not just detectable.
#[test]
fn bad_input_exits_non_zero_and_names_the_offending_input() {
    assert_failed_naming(
        &run(&["--definitely-not-a-flag"]),
        "--definitely-not-a-flag",
        "unknown flag",
    );

    let missing_dir = "no_such_preset_dir";
    assert_failed_naming(
        &run(&["--presets", missing_dir, "--report"]),
        missing_dir,
        "missing --presets directory",
    );

    let missing_file = "no_such_preset_file.toml";
    assert_failed_naming(
        &run(&["--preset-file", missing_file, "--out", "unused.png"]),
        missing_file,
        "missing --preset-file",
    );

    // A single shot with no destination: the offending input is the *absent*
    // --out, so that is what the message must name.
    assert_failed_naming(
        &run(&["--preset", SHIPPED_PRESET_NAME]),
        "--out",
        "--preset without --out",
    );
}

/// The explicit library flags must **error** rather than quietly falling back to
/// some other preset library — the exit-code contract Plan 0015 pinned. A silent
/// fallback would hand back a capture of the wrong presets, which is the confusing
/// failure this CLI exists to avoid.
#[test]
fn explicit_library_flags_do_not_silently_fall_back() {
    let out = run(&["--presets", "no_such_preset_dir", "--report"]);
    assert!(!out.status.success(), "a bad --presets must not exit 0");
    let combined = format!("{}{}", stdout(&out), stderr(&out));
    assert!(
        !combined.contains("embedded defaults"),
        "a bad --presets fell back to the embedded library instead of failing:\n{combined}"
    );
}

// ---------------------------------------------------------------------------
// Rendering cases — need a real adapter; skip with a reason where there is none
// ---------------------------------------------------------------------------

/// A single shot writes a real PNG at exactly the requested `--size`. This is the
/// whole point of the tool, and nothing checked that the file it wrote was a valid
/// image of the right dimensions.
#[test]
fn a_single_shot_writes_a_png_at_the_requested_size() {
    let out_dir = scratch("single");
    let png = out_dir.join("shot.png");
    let png_arg = png.to_string_lossy().into_owned();

    let out = run(&[
        "--preset-file",
        SHIPPED_PRESET_FILE,
        "--size",
        "64x48",
        "--frames",
        "2",
        "--out",
        &png_arg,
    ]);
    if skipped_for_no_adapter(&out) {
        return;
    }
    assert!(
        out.status.success(),
        "single shot failed\nstdout: {}\nstderr: {}",
        stdout(&out),
        stderr(&out)
    );
    assert!(png.is_file(), "no PNG at {}", png.display());

    let decoded = image::open(&png).expect("the written file decodes as an image");
    assert_eq!(
        (decoded.width(), decoded.height()),
        (64, 48),
        "the PNG is not the size --size asked for"
    );

    // A one-entry roster names itself, so --preset was not required (Plan 0015).
    assert!(
        stdout(&out).contains("--preset-file"),
        "the capture did not report its provenance:\n{}",
        stdout(&out)
    );
}

/// `--presets <dir>` wins the precedence chain and says so. The `[source]` label is
/// how a capture's provenance stops being a guess, and it is the direct evidence
/// that the app and the CLI share one resolver (ADR-0014).
#[test]
fn the_presets_flag_is_reported_as_the_source() {
    let out_dir = scratch("provenance");
    let png = out_dir.join("shot.png");
    let png_arg = png.to_string_lossy().into_owned();

    let out = run(&[
        "--presets",
        "presets",
        "--preset",
        SHIPPED_PRESET_NAME,
        "--size",
        "32x32",
        "--frames",
        "2",
        "--out",
        &png_arg,
    ]);
    if skipped_for_no_adapter(&out) {
        return;
    }
    assert!(
        out.status.success(),
        "capture from --presets failed (is `{SHIPPED_PRESET_NAME}` still a shipped \
         preset in {SHIPPED_PRESET_FILE}?)\nstdout: {}\nstderr: {}",
        stdout(&out),
        stderr(&out)
    );
    assert!(
        stdout(&out).contains("[--presets presets]"),
        "stdout does not carry the --presets provenance label:\n{}",
        stdout(&out)
    );
}

/// `--report --json` emits parseable JSON with the documented top-level shape. The
/// report is hand-rolled (no serde), so nothing but a consumer proves it is
/// well-formed — and the `preset-author` lane is that consumer.
#[test]
fn the_json_report_is_well_formed_and_carries_its_top_level_keys() {
    let out = run(&["--report", "--json", "--presets", "presets"]);
    if skipped_for_no_adapter(&out) {
        return;
    }
    assert!(
        out.status.success(),
        "--report --json failed\nstderr: {}",
        stderr(&out)
    );

    let json = stdout(&out);
    assert!(
        json_is_balanced(&json),
        "the report is not well-formed JSON (unbalanced braces/brackets/quotes):\n{json}"
    );
    for key in ["source", "families"] {
        assert!(
            top_level_keys(&json).iter().any(|k| k == key),
            "the report is missing top-level key `{key}`; keys were {:?}",
            top_level_keys(&json)
        );
    }
}

// ---------------------------------------------------------------------------
// Minimal JSON structural checks
//
// `serde_json` is not a dependency of this crate and adding one for a test would
// spend NFR section 4's dependency budget on a hundred lines of scanning. These
// two functions are string-aware and escape-aware, which is all the report's fixed
// numeric schema needs.
// ---------------------------------------------------------------------------

/// Braces, brackets and quotes all close, and no structural character inside a
/// string is counted.
fn json_is_balanced(s: &str) -> bool {
    let mut stack = Vec::new();
    let mut in_string = false;
    let mut escaped = false;
    for c in s.chars() {
        if in_string {
            match c {
                _ if escaped => escaped = false,
                '\\' => escaped = true,
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }
        match c {
            '"' => in_string = true,
            '{' => stack.push('}'),
            '[' => stack.push(']'),
            // Written as an inner `match` rather than a guard on the arm: the
            // guard form clippy suggests would make `stack.pop()` a side effect
            // inside a condition, which reads as though nothing was consumed when
            // the closer matches.
            '}' | ']' => match stack.pop() {
                Some(expected) if expected == c => {}
                _ => return false,
            },
            _ => {}
        }
    }
    !in_string && stack.is_empty()
}

/// The keys of the outermost object, in order of appearance: quoted strings that
/// sit at nesting depth 1 and are immediately followed by a colon.
fn top_level_keys(s: &str) -> Vec<String> {
    let mut keys = Vec::new();
    let mut depth = 0usize;
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' | '[' => depth += 1,
            '}' | ']' => depth = depth.saturating_sub(1),
            '"' => {
                let mut key = String::new();
                let mut escaped = false;
                for c in chars.by_ref() {
                    match c {
                        _ if escaped => escaped = false,
                        '\\' => escaped = true,
                        '"' => break,
                        _ => key.push(c),
                    }
                }
                // A key is a string at the outermost object depth followed by ':'.
                if depth == 1 && chars.peek() == Some(&':') {
                    keys.push(key);
                }
            }
            _ => {}
        }
    }
    keys
}

#[test]
fn the_json_helpers_reject_what_they_should() {
    assert!(json_is_balanced(r#"{"a":[1,2],"b":{"c":3}}"#));
    assert!(
        json_is_balanced(r#"{"a":"}]{["}"#),
        "structure inside a string"
    );
    assert!(json_is_balanced(r#"{"a":"esc\"aped"}"#));
    assert!(!json_is_balanced(r#"{"a":[1,2}"#), "mismatched closer");
    assert!(!json_is_balanced(r#"{"a":1"#), "unclosed object");
    assert!(
        !json_is_balanced(r#"{"a":"unterminated}"#),
        "unclosed string"
    );

    assert_eq!(top_level_keys(r#"{"a":1,"b":{"c":2}}"#), vec!["a", "b"]);
    assert_eq!(
        top_level_keys(r#"{"source":"x","families":{"swarm":{"presets":{}}}}"#),
        vec!["source", "families"],
        "nested keys must not be reported as top level"
    );
}
