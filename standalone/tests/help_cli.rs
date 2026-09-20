//! `ritmolux --help`, run the way a guard runs it (Plan 0135 Phase 2, ADR-0148).
//!
//! The property under test is not the text — `main.rs`'s own unit tests assert
//! that the roster is printed in full. It is that the process **answers and
//! exits**: a guard written for the lighting runner shelled out to `--help` to
//! discover the flag surface and hung, because every argument fell through the
//! scanners unclaimed and started the visualizer. A window, a wgpu device or a
//! capture client on this path is the failure, and none of them is observable
//! from inside the process that would be creating them.
//!
//! So this spawns the built binary, through `common::player`. `ritmolux` is a
//! `[[bin]]`, so cargo rebuilds it before the test runs — unlike the `shot` CLI
//! beside it, which is an example and has to be located by path.
//!
//! GPU-free by construction: every case here exits before a renderer exists, so
//! they run on any machine including an adapterless CI runner.
//!
//! **Each `#[test]` here spawns several command lines rather than one.** This
//! binary is in the run-alone class (ADR-0193), where nextest drains every slot
//! before a testcase starts and admits nothing beside it, so the unit that costs
//! is the *testcase*, not the spawn: a case that checks six command lines pays
//! one testcase's share of that serialization and six cheap process launches.
//! A case groups the command lines that hold one property, and the property is
//! named in the case's own doc comment. Do not split one back out to make a
//! failure easier to read — the assertion messages name the command line.

// The bound below is on a *subprocess*, and nothing under test reads a clock:
// the path being timed formats a string and returns before an event loop,
// a renderer or a capture client exists.
#![allow(
    clippy::disallowed_methods,
    reason = "the exit bound deliberately times a spawned process; the code under test is clock-free"
)]

mod common;

use std::time::{Duration, Instant};

/// The bar for "answers and exits" rather than "starts the app". Generous by
/// three orders of magnitude against the work the path actually does (format a
/// string, write it, return), because it is sized to catch a window opening on a
/// cold cache, not to measure printing.
const RESPONDS_WITHIN: Duration = Duration::from_secs(1);

/// Run `ritmolux` with `args` and return its exit code, stdout and how long it took.
fn run(args: &[&str]) -> (Option<i32>, String, Duration) {
    let (code, stdout, _, elapsed) = run_both(args);
    (code, stdout, elapsed)
}

/// As [`run`], with stderr as well: a refusal writes there, and the point of
/// these cases is what the operator is told before the process ends.
fn run_both(args: &[&str]) -> (Option<i32>, String, String, Duration) {
    let started = Instant::now();
    let output = common::player()
        .args(args)
        .output()
        .expect("failed to spawn the ritmolux binary");
    let elapsed = started.elapsed();
    (
        output.status.code(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
        elapsed,
    )
}

/// **The failure this file exists for: a query answers, within the bound, and
/// the process is gone.** Every command line here is one a program shells out to
/// before it drives the player — `--help` to discover the flag surface, a studio
/// running `--schema` once at startup — so one that opened a window or waited for
/// a GPU would hang its parent. A process still alive when [`run_both`] returns
/// is one that opened a window, and `output()` would not have returned at all;
/// the elapsed bound catches the weaker version, where something was built and
/// then torn down.
#[test]
fn every_query_answers_within_the_bound_and_exits_zero() {
    // `--help` and its short form. The banner, the usage line, and one flag from
    // each of the two scanner families the roster spans, so a roster that
    // printed only what `main.rs` parses fails here.
    for flag in ["--help", "-h"] {
        let (code, stdout, elapsed) = run(&[flag]);
        assert_eq!(code, Some(0), "`ritmolux {flag}` did not exit 0");
        assert!(
            elapsed < RESPONDS_WITHIN,
            "`ritmolux {flag}` took {elapsed:?}, which is long enough to have built something"
        );
        assert!(
            stdout.contains("usage: ritmolux"),
            "`ritmolux {flag}` printed no usage to stdout: {stdout:?}"
        );
        // The product is `Ritmolux` (ADR-0162) and the binary is `ritmolux`, so
        // the banner and the usage line disagree on case on purpose.
        assert!(
            stdout.starts_with("Ritmolux — "),
            "the banner does not open with the product's name: {stdout:?}"
        );
        assert!(stdout.contains("--osc"), "the roster omitted --osc");
        assert!(stdout.contains("--sender"), "the roster omitted --sender");
    }

    // **Help wins over a typo sharing the command line with it.** Someone asking
    // what the flags are is the one caller to answer rather than refuse.
    let (code, stdout, _) = run(&["--ocs", "127.0.0.1:9000", "--help"]);
    assert_eq!(code, Some(0), "`--help` beside a typo did not exit 0");
    assert!(
        stdout.contains("usage: ritmolux"),
        "`--help` beside a typo printed no usage: {stdout:?}"
    );

    // `--schema` answers on stdout with nothing else on it. The content is
    // asserted in `core` (against the published reference and against its own
    // hash); what only a subprocess can show is that the document is **alone**
    // on standard output, which is what lets a parent pipe it into a parser.
    let (code, stdout, stderr, elapsed) = run_both(&["--schema"]);
    assert_eq!(code, Some(0), "--schema exits cleanly");
    assert!(
        elapsed < RESPONDS_WITHIN,
        "--schema took {elapsed:?}, which is long enough that it may have \
         started something"
    );
    assert!(
        stderr.is_empty(),
        "--schema wrote to stderr, so a parent reading both streams sees noise \
         beside the document: {stderr}"
    );
    let trimmed = stdout.trim_end_matches(['\r', '\n']);
    assert!(
        trimmed.starts_with('{') && trimmed.ends_with('}'),
        "standard output is not one JSON object; it begins `{}` and ends `{}`",
        trimmed.chars().take(20).collect::<String>(),
        trimmed.chars().rev().take(20).collect::<String>(),
    );
    assert!(
        !trimmed.contains('\n'),
        "the document spans more than one line, so a parent reading a line at a \
         time cannot take it in one"
    );
    assert!(
        trimmed.contains("\"hash\":\"") && trimmed.contains("\"systems\":["),
        "the document is missing the hash or the systems roster"
    );
}

/// **A command line this launch cannot honour is refused before anything is
/// built, and the refusal names what was wrong.** The silence each of these
/// replaces was a running visualizer doing less than it was asked; the refusal
/// has to arrive the way `--help` does, from a process that never opened a
/// window (ADR-0155).
///
/// **Only the last case carries an elapsed bound**, and the difference is the
/// number of spawns. `RESPONDS_WITHIN` is sized against one cold process; the
/// loops here spawn several, and on a saturated runner the first can exceed a
/// second while behaving perfectly. That would be a reading about the load on
/// the box rather than about the code (ADR-0071). The property is carried
/// without it: `output()` waits for exit, so a process that opened a window
/// never returns here at all, and the exit code and the named cause are what is
/// actually under test. The unknown-preset paths have a second reason — they
/// read and parse the whole preset directory to build the roster they refuse
/// against, which is real work whose duration is a property of that directory's
/// size.
#[test]
fn an_unhonourable_command_line_is_refused_before_anything_is_built() {
    // **The roster gate, asserted on the process rather than on the function.**
    // Both command lines are design-backlog 0159's own reduction, which measured
    // them starting the app and drawing.
    let (code, _, stderr, _) = run_both(&["--ocs", "127.0.0.1:9000"]);
    assert_eq!(
        code,
        Some(2),
        "a misspelt flag is a usage error: {stderr:?}"
    );
    assert!(
        stderr.contains("--ocs"),
        "the refusal did not name it: {stderr}"
    );
    assert!(
        stderr.contains("--osc"),
        "the refusal did not name the nearest flag: {stderr}"
    );

    let (code, _, stderr, _) = run_both(&["--definitely-not-a-flag"]);
    assert_eq!(
        code,
        Some(2),
        "an unknown flag is a usage error: {stderr:?}"
    );
    assert!(
        stderr.contains("--definitely-not-a-flag"),
        "the refusal did not name the argument: {stderr}"
    );

    // A flag whose companion is absent is refused rather than ignored.
    for args in [
        ["--fps", "30"].as_slice(),
        ["--size", "1280x720"].as_slice(),
        ["--sender=rig"].as_slice(),
        ["--frames", "100"].as_slice(),
    ] {
        let (code, _, stderr, _) = run_both(args);
        assert_eq!(
            code,
            Some(2),
            "`ritmolux {args:?}` did not exit 2: {stderr:?}"
        );
        assert!(
            stderr.contains("--stream"),
            "`ritmolux {args:?}` did not name the missing companion: {stderr:?}"
        );
    }

    // **`--preset` is NOT refused for a missing `--stream`**, because it reaches
    // the window. This covers `--preset` alone: `--gpu` gets past its own scanner
    // only by opening a wgpu device, so its freedom from `--stream` is pinned
    // against the roster by `the_two_windowed_flags_carry_no_dependency` in
    // `main.rs` rather than by a process here.
    let (code, _, stderr, _) = run_both(&["--preset", "a-name-no-preset-has"]);
    assert_eq!(
        code,
        Some(2),
        "an unknown preset name is still a usage error: {stderr:?}"
    );
    assert!(
        !stderr.contains("--stream"),
        "`--preset` was refused for a missing `--stream`: {stderr:?}"
    );
    assert!(
        stderr.contains("a-name-no-preset-has"),
        "the refusal did not name what was typed: {stderr:?}"
    );

    // **An unknown `--preset` costs no window.** The name is judged against the
    // roster this launch would load, before the event loop exists, so the
    // failure is a message rather than a window that opens on an arbitrary
    // scene, and the roster is listed so the operator can see what they could
    // have meant.
    let (code, _, stderr, _) = run_both(&["--preset", "definitely-not-a-preset"]);
    assert_eq!(code, Some(2), "expected a usage error: {stderr:?}");
    assert!(
        stderr.contains("this launch holds"),
        "the refusal did not list the roster: {stderr:?}"
    );

    // `--preview` with a sink this build does not have exits without a window,
    // naming the one it does. The same rule every windowed flag follows: a value
    // typed for this run that cannot be honoured is a usage error, not a window
    // that opens and then reports one. The default — the flag absent — is
    // covered by every other case in this file, none of which mirrors anything.
    let (code, _, stderr, elapsed) = run_both(&["--preview", "syphon"]);
    assert_eq!(code, Some(2), "a bad flag value is a usage error");
    assert!(
        elapsed < RESPONDS_WITHIN,
        "--preview syphon took {elapsed:?}, which is long enough that it may \
         have opened a window before refusing"
    );
    assert!(
        stderr.contains("syphon") && stderr.contains("stdout"),
        "the refusal does not name the value and the sink that exists: {stderr}"
    );
}

/// **`--list-presets` answers from the directory this launch would read, and
/// exits.** The property is the same one `--help` and `--schema` are spawned
/// for, plus the one only a subprocess can show here: the statuses are computed
/// against a directory handed in from outside, so the row for a file nobody
/// shipped and the marker on a name two files claim are visible rather than
/// inferred. It stands alone because it is the only case here that has to build
/// a directory first.
///
/// The preset directory is scratch **and so is the data root**: left inherited,
/// the run would resolve the developer's own `%APPDATA%` for `config.toml` and
/// the diagnostics log, which is the class design-backlog 0181 records.
/// `common::player` sets the root; `RLX_PRESET_DIR` is set here on top of it.
#[test]
fn list_presets_names_each_files_status_and_exits_zero() {
    let dir = common::scratch_data_root().join("presets");
    std::fs::create_dir_all(&dir).expect("create the scratch preset directory");

    // A byte-identical copy of a shipped file, so one row reads `shipped`, and
    // a second file carrying the same display name under a filename that sorts
    // after it, so the first is the one a lookup by name reaches.
    let (shipped_file, shipped_source) = rlx_core::preset::EMBEDDED[0];
    let duplicated = rlx_core::preset::Preset::from_toml_str(shipped_source)
        .expect("a shipped preset compiles")
        .name;
    std::fs::write(dir.join(shipped_file), shipped_source).expect("write the shipped copy");
    std::fs::write(dir.join("zz_duplicate.toml"), shipped_source).expect("write the duplicate");

    // A preset this build's set does not have, under a name no build ships:
    // another shipped source with its `name =` line rewritten, so it is known to
    // compile and its display name is known not to collide.
    let (_, borrowed) = rlx_core::preset::EMBEDDED[1];
    // The **first** `name =` line only: it precedes every `[table]` header, so
    // it is the preset's own, and a later one would be a palette's.
    let mut rewritten = false;
    let mine: String = borrowed
        .lines()
        .map(|line| {
            if !rewritten && line.starts_with("name") && line.contains('=') {
                rewritten = true;
                "name = \"A Name No Build Ships\"".to_owned()
            } else {
                line.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        mine.contains("A Name No Build Ships"),
        "the fixture's `name` line was not found in the borrowed source"
    );
    std::fs::write(dir.join("zz_mine.toml"), &mine).expect("write the unshipped preset");

    let output = common::player()
        .arg("--list-presets")
        .env(standalone::PRESET_DIR_ENV, &dir)
        .output()
        .expect("failed to spawn the ritmolux binary");
    assert_eq!(
        output.status.code(),
        Some(0),
        "--list-presets is a query and exits cleanly"
    );
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();

    let row = |file: &str| -> String {
        stderr
            .lines()
            .find(|line| line.contains(file))
            .unwrap_or_else(|| panic!("no row for {file} in:\n{stderr}"))
            .to_owned()
    };

    let shipped_row = row(shipped_file);
    assert!(
        !shipped_row.contains("not shipped") && shipped_row.contains("shipped"),
        "the untouched copy is not reported as shipped: {shipped_row}"
    );
    let mine_row = row("zz_mine.toml");
    assert!(
        mine_row.contains("not shipped") && mine_row.contains("A Name No Build Ships"),
        "the unshipped file's row does not say so: {mine_row}"
    );
    let duplicate_row = row("zz_duplicate.toml");
    assert!(
        duplicate_row.contains("not shipped") && duplicate_row.contains(shipped_file),
        "the second claimant's row does not name the file that wins the name: {duplicate_row}"
    );
    assert!(
        stderr.matches(duplicated.as_str()).count() >= 2,
        "the contested display name appears on only one row: {stderr}"
    );
    assert!(
        stderr.contains(&dir.display().to_string()),
        "the listing does not name the directory it read: {stderr}"
    );
}
