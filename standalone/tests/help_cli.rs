//! `lmv --help`, run the way a guard runs it (Plan 0135 Phase 2, ADR-0148).
//!
//! The property under test is not the text — `main.rs`'s own unit tests assert
//! that the roster is printed in full. It is that the process **answers and
//! exits**: a guard written for the lighting runner shelled out to `--help` to
//! discover the flag surface and hung, because every argument fell through the
//! scanners unclaimed and started the visualizer. A window, a wgpu device or a
//! capture client on this path is the failure, and none of them is observable
//! from inside the process that would be creating them.
//!
//! So this spawns the built binary. `lmv` is a `[[bin]]`, so `CARGO_BIN_EXE_lmv`
//! resolves it and cargo rebuilds it before the test runs — unlike the `shot`
//! CLI beside it, which is an example and has to be located by path.
//!
//! GPU-free by construction: every case here exits before a renderer exists, so
//! they run on any machine including an adapterless CI runner.

// The bound below is on a *subprocess*, and nothing under test reads a clock:
// the path being timed formats a string and returns before an event loop,
// a renderer or a capture client exists.
#![allow(
    clippy::disallowed_methods,
    reason = "the exit bound deliberately times a spawned process; the code under test is clock-free"
)]

use std::process::Command;
use std::time::{Duration, Instant};

/// The bar for "answers and exits" rather than "starts the app". Generous by
/// three orders of magnitude against the work the path actually does (format a
/// string, write it, return), because it is sized to catch a window opening on a
/// cold cache, not to measure printing.
const RESPONDS_WITHIN: Duration = Duration::from_secs(1);

/// Run `lmv` with `args` and return its exit code, stdout and how long it took.
fn run(args: &[&str]) -> (Option<i32>, String, Duration) {
    let started = Instant::now();
    let output = Command::new(env!("CARGO_BIN_EXE_lmv"))
        .args(args)
        .output()
        .expect("failed to spawn the lmv binary");
    let elapsed = started.elapsed();
    (
        output.status.code(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        elapsed,
    )
}

/// **The failure this exists for.** `--help` must terminate on its own. A
/// process still alive when this returns is one that opened a window, and the
/// `output()` call would not have returned at all.
#[test]
fn help_prints_the_roster_and_exits_zero() {
    for flag in ["--help", "-h"] {
        let (code, stdout, elapsed) = run(&[flag]);
        assert_eq!(code, Some(0), "`lmv {flag}` did not exit 0");
        assert!(
            elapsed < RESPONDS_WITHIN,
            "`lmv {flag}` took {elapsed:?}, which is long enough to have built something"
        );
        assert!(
            stdout.contains("usage: lmv"),
            "`lmv {flag}` printed no usage to stdout: {stdout:?}"
        );
        // One flag from each of the two scanner families the roster spans, so a
        // roster that printed only what `main.rs` parses fails here.
        assert!(stdout.contains("--osc"), "the roster omitted --osc");
        assert!(stdout.contains("--sender"), "the roster omitted --sender");
    }
}

/// **Help wins over a typo sharing the command line with it.** Someone asking
/// what the flags are is the one caller to answer rather than refuse.
#[test]
fn help_is_answered_even_beside_an_unrecognized_argument() {
    let (code, stdout, _) = run(&["--ocs", "127.0.0.1:9000", "--help"]);
    assert_eq!(code, Some(0));
    assert!(stdout.contains("usage: lmv"));
}

/// **The roster gate, asserted on the process rather than on the function.**
/// Both commands are design-backlog 0159's own reduction, which measured them
/// starting the app and drawing.
#[test]
fn an_unrecognized_argument_exits_non_zero_and_names_it() {
    let output = Command::new(env!("CARGO_BIN_EXE_lmv"))
        .arg("--ocs")
        .arg("127.0.0.1:9000")
        .output()
        .expect("failed to spawn the lmv binary");
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--ocs"),
        "the refusal did not name it: {stderr}"
    );
    assert!(
        stderr.contains("--osc"),
        "the refusal did not name the nearest flag: {stderr}"
    );

    let output = Command::new(env!("CARGO_BIN_EXE_lmv"))
        .arg("--definitely-not-a-flag")
        .output()
        .expect("failed to spawn the lmv binary");
    assert_eq!(output.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("--definitely-not-a-flag"),
        "the refusal did not name the argument"
    );
}
