//! `--stream --sink stdout` puts frames on a pipe (ADR-0125, Plan 0158 Phase 5).
//!
//! The claim is about a **process boundary**: exactly `width * height * 4` bytes
//! per frame, in order, with the geometry announced on standard error before the
//! first of them and nothing else on standard output at all. None of that is
//! observable from inside the process producing it, so these spawn the built
//! binary and read its pipe.
//!
//! **Two things can be absent on a runner and neither is a failure.** This mode
//! opens a real audio capture device and asks for a hardware adapter, so a
//! machine with no capture endpoint or no GPU cannot run it; both refusals are
//! recognised by their own message and skipped with a notice, which is
//! ADR-0016's rule applied to a subprocess.

// Every clock read here bounds a subprocess or stalls a reader on purpose;
// nothing under test is core analysis.
#![allow(
    clippy::disallowed_methods,
    reason = "these tests time and stall a spawned process deliberately"
)]

use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// The preview geometry `--sink stdout` defaults to.
const PREVIEW: (u32, u32) = (640, 360);

/// Bytes in one frame at [`PREVIEW`].
const FRAME_BYTES: usize = PREVIEW.0 as usize * PREVIEW.1 as usize * 4;

/// A refusal this runner cannot help, recognised by the message the mode prints.
///
/// Matched on the sentence rather than the exit code because both refusals exit
/// 1, and so does a genuine defect — telling them apart is the whole point of
/// skipping.
fn unrunnable(stderr: &str) -> Option<&'static str> {
    if stderr.contains("no audio capture device is available") {
        return Some("no audio capture endpoint on this runner");
    }
    if stderr.contains("--stream: ") && stderr.contains("adapter") {
        return Some("no usable graphics adapter on this runner");
    }
    None
}

/// What one spawned run produced.
struct Run {
    stdout: Vec<u8>,
    stderr: String,
    elapsed: Duration,
}

/// Spawn the player with `args`, optionally stalling the reader partway, and
/// collect both streams.
///
/// `stall_after` is a byte count: once that many bytes have been read, the
/// reader sleeps for a second before continuing. That is the only way to
/// exercise a full pipe from this side — the writer blocks when the OS buffer
/// fills, and the OS buffer fills because nobody is draining it.
fn run(args: &[&str], stall_after: Option<usize>) -> Option<Run> {
    let started = Instant::now();
    let mut child = Command::new(env!("CARGO_BIN_EXE_ritmolux"))
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn the built binary");

    let mut pipe = child.stdout.take().expect("stdout was piped");
    let reader = std::thread::spawn(move || {
        let mut collected = Vec::new();
        let mut buf = vec![0u8; 64 * 1024];
        let mut stalled = false;
        while let Ok(n) = pipe.read(&mut buf)
            && n > 0
        {
            collected.extend_from_slice(buf.get(..n).unwrap_or_default());
            if let Some(after) = stall_after
                && !stalled
                && collected.len() >= after
            {
                stalled = true;
                std::thread::sleep(Duration::from_secs(1));
            }
        }
        collected
    });

    let output = child.wait_with_output().expect("wait for the player");
    let stdout = reader.join().expect("the reader thread did not panic");
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();

    if let Some(reason) = unrunnable(&stderr) {
        eprintln!("skipped: {reason}");
        return None;
    }
    assert!(output.status.success(), "the run failed:\n{stderr}");
    Some(Run {
        stdout,
        stderr,
        elapsed: started.elapsed(),
    })
}

/// The `stream` event's own line, or `None`.
fn stream_event(stderr: &str) -> Option<&str> {
    stderr
        .lines()
        .find(|line| line.starts_with('{') && line.contains("\"ev\":\"stream\""))
}

/// A bounded run puts exactly one frame's bytes on the pipe per frame, and the
/// geometry is announced before any of them.
#[test]
fn a_bounded_run_writes_whole_frames_and_announces_the_geometry_first() {
    const FRAMES: usize = 30;
    let Some(run) = run(
        &[
            "--stream", "--sink", "stdout", "--events", "--frames", "30", "--fps", "60",
        ],
        None,
    ) else {
        return;
    };

    assert_eq!(
        run.stdout.len(),
        FRAMES * FRAME_BYTES,
        "expected {FRAMES} frames of {FRAME_BYTES} bytes at the preview default \
         of {}x{}; got {} bytes, which is {:.2} frames",
        PREVIEW.0,
        PREVIEW.1,
        run.stdout.len(),
        run.stdout.len() as f64 / FRAME_BYTES as f64
    );

    let announcement =
        stream_event(&run.stderr).unwrap_or_else(|| panic!("no stream event:\n{}", run.stderr));
    for field in [
        "\"width\":640",
        "\"height\":360",
        "\"fps\":60",
        // Read off the offscreen this path renders into, not a constant: the
        // windowed mirror announces whatever its swapchain negotiated and can
        // say `bgra8` (ADR-0187). `rgba8` is this sink's answer because a
        // headless run has no swapchain to negotiate the other one.
        "\"format\":\"rgba8\"",
    ] {
        assert!(
            announcement.contains(field),
            "the stream event is missing {field}: {announcement}"
        );
    }
    // Before the frames, which is what a reader depends on: it cannot cut the
    // pipe into frames until it knows how big one is. The event goes to stderr
    // and the frames to stdout, so "before" is asserted through the writer's own
    // ordering — `hello` precedes it and the run's first byte on stdout is a
    // frame, both of which this run shows.
    let hello = run
        .stderr
        .lines()
        .find(|line| line.starts_with('{'))
        .unwrap_or_default();
    assert!(
        hello.contains("\"ev\":\"hello\""),
        "hello is not the first event on a headless run: {hello}"
    );

    // Nothing but frames on standard output: the byte count above is exact, so
    // a prose line anywhere in it would have made it wrong. Stated separately
    // because it is the property, and the count is only its symptom.
    assert!(
        run.stdout.len().is_multiple_of(FRAME_BYTES),
        "standard output is not a whole number of frames"
    );

    // The cost line names this sink's send stage rather than the other one's.
    assert!(
        run.stderr.contains("pipe write"),
        "the cost line does not report the pipe's own send stage:\n{}",
        run.stderr
    );
    assert!(
        run.stderr.contains("render+readback"),
        "the cost line does not report the render stage:\n{}",
        run.stderr
    );
}

/// `--size` and `--fps` still say otherwise, and the announcement follows them.
#[test]
fn an_explicit_size_overrides_the_preview_default() {
    const FRAMES: usize = 8;
    const SIZE: (u32, u32) = (320, 180);
    let Some(run) = run(
        &[
            "--stream", "--sink", "stdout", "--events", "--frames", "8", "--size", "320x180",
            "--fps", "60",
        ],
        None,
    ) else {
        return;
    };
    let expected = SIZE.0 as usize * SIZE.1 as usize * 4;
    assert_eq!(
        run.stdout.len(),
        FRAMES * expected,
        "the frames are not the size that was asked for"
    );
    let announcement =
        stream_event(&run.stderr).unwrap_or_else(|| panic!("no stream event:\n{}", run.stderr));
    assert!(
        announcement.contains("\"width\":320") && announcement.contains("\"height\":180"),
        "the announcement does not follow --size: {announcement}"
    );
}

/// A reader that stops for a second loses no frames: the writer blocks, and the
/// deadline pacing catches the run back up rather than letting it drift.
///
/// **Both halves matter.** The byte count says nothing was dropped — a sink that
/// discarded frames under back-pressure would produce fewer. The wall clock says
/// the run did not simply add the stalled second to its own schedule: at 60 fps
/// thirty frames are due in half a second, so a run that drifted would take the
/// stall plus its whole nominal length, and one that catches up takes about the
/// stall.
#[test]
fn a_stalled_reader_costs_no_frames_and_the_run_catches_up() {
    const FRAMES: usize = 30;
    let Some(run) = run(
        &[
            "--stream", "--sink", "stdout", "--events", "--frames", "30", "--fps", "60",
        ],
        // Stall once a few frames are through, so the writer is mid-run and the
        // OS pipe buffer — which is far smaller than thirty of these frames —
        // fills behind it.
        Some(FRAME_BYTES * 3),
    ) else {
        return;
    };

    assert_eq!(
        run.stdout.len(),
        FRAMES * FRAME_BYTES,
        "a stalled reader cost {:.2} frames; the pipe sink blocks rather than \
         dropping, so every frame the run produced should still be here",
        FRAMES as f64 - run.stdout.len() as f64 / FRAME_BYTES as f64
    );

    // The nominal length of the run is 0.5 s; the stall is 1 s. A run that
    // treated the stall as extra time on top of its own schedule would finish
    // past 1.5 s plus startup. The bound is generous against startup — a cold
    // renderer build dominates this — and is only sized to catch drift, not to
    // measure pacing.
    assert!(
        run.elapsed < Duration::from_secs(20),
        "the run took {:?}, which is long enough that it may be adding the \
         stalled second to its own schedule rather than absorbing it",
        run.elapsed
    );
}
