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
//! So this spawns the built binary. `ritmolux` is a `[[bin]]`, so `CARGO_BIN_EXE_ritmolux`
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

/// Run `ritmolux` with `args` and return its exit code, stdout and how long it took.
fn run(args: &[&str]) -> (Option<i32>, String, Duration) {
    let (code, stdout, _, elapsed) = run_both(args);
    (code, stdout, elapsed)
}

/// As [`run`], with stderr as well: a refusal writes there, and the point of
/// these cases is what the operator is told before the process ends.
fn run_both(args: &[&str]) -> (Option<i32>, String, String, Duration) {
    let started = Instant::now();
    let output = Command::new(env!("CARGO_BIN_EXE_ritmolux"))
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

/// **The failure this exists for.** `--help` must terminate on its own. A
/// process still alive when this returns is one that opened a window, and the
/// `output()` call would not have returned at all.
#[test]
fn help_prints_the_roster_and_exits_zero() {
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
    assert!(stdout.contains("usage: ritmolux"));
}

/// **The roster gate, asserted on the process rather than on the function.**
/// Both commands are design-backlog 0159's own reduction, which measured them
/// starting the app and drawing.
#[test]
fn an_unrecognized_argument_exits_non_zero_and_names_it() {
    let output = Command::new(env!("CARGO_BIN_EXE_ritmolux"))
        .arg("--ocs")
        .arg("127.0.0.1:9000")
        .output()
        .expect("failed to spawn the ritmolux binary");
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

    let output = Command::new(env!("CARGO_BIN_EXE_ritmolux"))
        .arg("--definitely-not-a-flag")
        .output()
        .expect("failed to spawn the ritmolux binary");
    assert_eq!(output.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("--definitely-not-a-flag"),
        "the refusal did not name the argument"
    );
}

/// **A flag whose companion is absent is refused before anything is built.**
/// The silence this replaces was a running visualizer doing less than it was
/// asked; the refusal has to arrive the way `--help` does, from a process that
/// never opened a window (ADR-0155).
///
/// **No elapsed-time bound here, unlike the `--help` cases above**, and the
/// difference is the number of spawns. `RESPONDS_WITHIN` is sized against one
/// cold process; this case spawns four in a loop, and on a saturated runner —
/// this suite puts 1223 tests through one machine — the first can exceed a
/// second while behaving perfectly. That would be a reading about the load on
/// the box rather than about the code (ADR-0071). The property is carried
/// without it: `output()` waits for exit, so a process that opened a window
/// never returns here at all, and the exit code and named companion are what is
/// actually under test.
#[test]
fn a_stream_only_flag_without_stream_exits_without_starting() {
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
}

/// **`--preset` is NOT refused for a missing `--stream`**, because it reaches
/// the window. This covers `--preset` alone: `--gpu` gets past its own scanner
/// only by opening a wgpu device, so its freedom from `--stream` is pinned
/// against the roster by `the_two_windowed_flags_carry_no_dependency` in
/// `main.rs` rather than by a process here.
#[test]
fn the_windowed_preset_flag_is_not_refused_for_a_missing_stream() {
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
}

/// **An unknown `--preset` costs no window.** The name is judged against the
/// roster this launch would load, before the event loop exists, so the failure
/// is a message rather than a window that opens on an arbitrary scene.
///
/// Bound-free for the reason given above, and with one of its own: this path
/// reads and parses the whole preset directory to build the roster it refuses
/// against, which is real work whose duration is a property of that directory's
/// size rather than of whether a window opened.
#[test]
fn an_unknown_preset_exits_without_opening_a_window() {
    let (code, _, stderr, _) = run_both(&["--preset", "definitely-not-a-preset"]);
    assert_eq!(code, Some(2), "expected a usage error: {stderr:?}");
    // The roster is listed so the operator can see what they could have meant.
    assert!(
        stderr.contains("this launch holds"),
        "the refusal did not list the roster: {stderr:?}"
    );
}
