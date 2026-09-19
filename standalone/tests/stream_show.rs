//! The headless run manages a show, and says so (ADR-0181, Plan 0159 Phase 3).
//!
//! `stream_pipe.rs` asserts what `--stream` puts on standard *output*. This one
//! asserts what it does around that: it resolves and watches a preset directory,
//! reports the event roster for the same causes a windowed run reports it for,
//! and binds the control listener `--control` asks for.
//!
//! Every claim here is about a **process**, because the thing under test is
//! which code a run mode reaches — invisible from inside the one process that
//! reached it. So these spawn the built binary, point it at a preset directory
//! they own through `RLX_PRESET_DIR`, and read its standard error.
//!
//! **Nothing here waits on a clock for a step it can wait on a line for.** Each
//! mid-run action is triggered by the line that says the run has reached the
//! point the action belongs at; a sleep instead would be racing process start on
//! a loaded runner, which is what a runner inside a full suite always is.
//!
//! **Two things can be absent on a runner and neither is a failure**, exactly as
//! in `stream_pipe.rs`: this mode opens a real capture device and asks for a
//! hardware adapter, and a machine with neither is skipped with a notice, which
//! is ADR-0016's rule applied to a subprocess.

// Every clock read here bounds a subprocess or a channel; nothing under test is
// core analysis.
#![allow(
    clippy::disallowed_methods,
    reason = "these tests bound a spawned process deliberately"
)]

mod common;

use std::io::{BufRead, Read};
use std::net::{SocketAddr, UdpSocket};
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

use rlx_core::preset::SystemKind;
use standalone::osc::decode::{Action, Name, Transport};

/// A preset that compiles, so the directory the run is pointed at yields a
/// roster rather than falling back to the embedded set.
const GOOD: &str = "name = \"Show Probe\"\nsystem = \"spectrum\"\n";

/// The second preset, so `ctl/preset` has somewhere to move the show to.
const TARGET: &str = "name = \"Show Target\"\nsystem = \"spectrum\"\n";

/// A preset that does not compile, so a reload has a `preset_error` to report.
/// The unclosed string is a parse failure the loader can put a line number on.
const BROKEN: &str = "name = \"Show Probe Broken\nsystem = \"spectrum\"\n";

/// How long to wait for a wanted line of a running child's standard error,
/// counted from the moment the thing that should produce it happened — the
/// spawn, or the datagram that asked for it.
///
/// Generous rather than tight: it covers process start, the adapter request and
/// the config read on a runner already busy with the rest of the suite. The wait
/// is only ever paid in full by a genuine failure — the receive returns the
/// moment a line arrives.
///
/// The bound is on the **wanted** line, not on the next line of any kind: a
/// player writes a `health` event every second while it draws, so a per-line
/// timeout would never expire on a child that keeps running and never answers.
const LINE_DEADLINE: Duration = Duration::from_secs(60);

/// How many trailing `health` events a missed-wait report keeps. Each one
/// carries the frame rate and the control counters at that second, which is
/// what says whether the child was drawing and whether the listener refused
/// anything; the rest of them are the same line again.
const REPORT_HEALTH_LINES: usize = 3;

/// How many other lines a missed-wait report keeps, newest last.
const REPORT_OTHER_LINES: usize = 40;

/// The nonce sent on `ctl/ping`, distinctive enough that finding it in a line
/// cannot be a coincidence.
const PING_NONCE: i32 = 424_242;

/// A preset name no roster in this file holds, sent to make the player refuse a
/// selection.
const ABSENT_PRESET: &str = "Show Nothing By This Name";

/// A refusal this runner cannot help, recognised by the message the mode prints
/// rather than by the exit code — a genuine defect exits 1 too, and telling the
/// two apart is the whole point of skipping.
fn unrunnable(stderr: &str) -> Option<&'static str> {
    if stderr.contains("no audio capture device is available") {
        return Some("no audio capture endpoint on this runner");
    }
    if stderr.contains("--stream: ") && stderr.contains("adapter") {
        return Some("no usable graphics adapter on this runner");
    }
    None
}

/// A directory this test owns, under `CARGO_TARGET_TMPDIR`: the scratch directory
/// cargo sets inside the target directory it is actually building into, wherever
/// `CARGO_TARGET_DIR` or `build.target-dir` put that, so a runner cleans it up
/// with everything else.
fn scratch(tag: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("stream-show")
        .join(tag);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create the scratch preset directory");
    dir
}

/// The thread draining a child's standard output, and the running byte count it
/// keeps so a failed wait can say whether frames were still being written.
struct Drain {
    handle: std::thread::JoinHandle<()>,
    bytes: Arc<AtomicU64>,
}

impl Drain {
    fn bytes(&self) -> u64 {
        self.bytes.load(Ordering::Relaxed)
    }

    fn join(self) {
        self.handle.join().expect("the drain thread did not panic");
    }
}

/// Spawn the player headless, with standard output drained by a thread so the
/// writer never blocks on a full pipe.
///
/// `presets` becomes `RLX_PRESET_DIR`; an empty path leaves the run to resolve
/// its own, which with the data root cleared is the unresolved case. The data
/// root is cleared (`common::player_with_data_root` with an empty path) so the
/// child cannot reach the developer's real per-user directory on any platform —
/// the presets are redirected already, and this keeps the config, the
/// diagnostics log and the directory migration off it too.
fn spawn(presets: &Path, extra: &[&str]) -> (Child, Drain) {
    spawn_with_data_root(presets, Path::new(""), extra)
}

/// [`spawn`], with the per-user data root pointed at `root` rather than cleared,
/// so what the run writes under it — the diagnostics log — lands in a directory
/// this test owns instead of the developer's own.
fn spawn_with_data_root(presets: &Path, root: &Path, extra: &[&str]) -> (Child, Drain) {
    let mut args = vec![
        "--stream", "--sink", "stdout", "--events", "--fps", "30", "--size", "160x90",
    ];
    args.extend_from_slice(extra);
    let mut child = common::player_with_data_root(root)
        .args(&args)
        .env("RLX_PRESET_DIR", presets)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn the built binary");
    let mut pipe = child.stdout.take().expect("stdout was piped");
    let bytes = Arc::new(AtomicU64::new(0));
    let handle = std::thread::spawn({
        let bytes = Arc::clone(&bytes);
        move || {
            let mut buf = vec![0u8; 64 * 1024];
            while let Ok(n) = pipe.read(&mut buf)
                && n > 0
            {
                bytes.fetch_add(n as u64, Ordering::Relaxed);
            }
        }
    });
    (child, Drain { handle, bytes })
}

/// A running child's standard error as it arrives, and every line the most
/// recent wait read, so a wait that ends without its line can say what the
/// child said instead.
struct Lines {
    rx: Receiver<String>,
    /// When the watch began, which is the spawn for every caller: the bound for
    /// a line the run emits on its own, such as `hello`.
    opened: Instant,
    read: Vec<String>,
}

/// Why a wait ended without the line it wanted.
#[derive(Clone, Copy, Debug)]
enum Missed {
    /// [`LINE_DEADLINE`] passed since the wait's starting instant.
    Deadline,
    /// The child's standard error closed: it exited, or was killed.
    Closed,
}

impl Lines {
    /// Wait for the first line `want` accepts, until [`LINE_DEADLINE`] after
    /// `since`. Every line read, wanted or not, is kept until the next wait.
    fn wait_for<T>(
        &mut self,
        since: Instant,
        want: impl Fn(&str) -> Option<T>,
    ) -> Result<T, Missed> {
        self.read.clear();
        self.wait_more(since + LINE_DEADLINE, want)
    }

    /// Keep reading into the same record until `until`, for the report's late
    /// check: whether the line arrived after the deadline, or never.
    fn wait_more<T>(
        &mut self,
        until: Instant,
        want: impl Fn(&str) -> Option<T>,
    ) -> Result<T, Missed> {
        loop {
            let left = until.saturating_duration_since(Instant::now());
            match self.rx.recv_timeout(left) {
                Ok(line) => {
                    let found = want(&line);
                    self.read.push(line);
                    if let Some(found) = found {
                        return Ok(found);
                    }
                }
                Err(RecvTimeoutError::Timeout) => return Err(Missed::Deadline),
                Err(RecvTimeoutError::Disconnected) => return Err(Missed::Closed),
            }
        }
    }
}

/// One thing the test did that a line should answer, recorded as it was done.
struct Ask {
    /// What was asked for, as the report names it.
    what: String,
    /// When it was asked: the send, or the spawn for a line the run emits
    /// unprompted.
    since: Instant,
    /// The child's standard output byte count at that instant.
    stdout_at_ask: u64,
    /// The nonce of a `ctl/ping` sent straight after the ask, if one was.
    ///
    /// The listener keeps no count of `ctl/preset` datagrams and the player
    /// reports nothing for a name that selects nothing, so an unanswered preset
    /// ask alone cannot say whether it arrived. A `pong` for this nonce can: it
    /// is emitted by the same drain that applies the preset, so a pong with no
    /// `preset` event means the ask's frame was drained and the switch did not
    /// show, and no pong means the datagrams or the drain never got there.
    ping: Option<i32>,
}

impl Ask {
    fn now(what: impl Into<String>, drain: &Drain) -> Self {
        Self {
            what: what.into(),
            since: Instant::now(),
            stdout_at_ask: drain.bytes(),
            ping: None,
        }
    }
}

/// Everything a wait that ended without its line can say about the child, as
/// one readable block: which ask went unanswered and how long after it was sent,
/// whether the line turned up late, whether the child was alive, whether its
/// standard output was still moving, and what it wrote on standard error since
/// the ask — every non-`health` line (a `preset` or `roster` for something else,
/// a `preset_error`) and the last few `health` lines, whose `ctl_*` counters say
/// whether the listener rejected, dropped or refused anything.
///
/// On a [`Missed::Deadline`] it keeps reading for one more [`LINE_DEADLINE`]
/// before reporting, so the report tells a late answer from a missing one. The
/// test has already failed by then; the extra wait is paid only on that path.
fn missed_report<T>(
    ask: &Ask,
    missed: Missed,
    lines: &mut Lines,
    want: impl Fn(&str) -> Option<T>,
    child: &mut Child,
    drain: &Drain,
) -> String {
    let Ask {
        what,
        since,
        stdout_at_ask,
        ping,
    } = ask;
    let missed_after = since.elapsed();
    let late = match missed {
        Missed::Deadline => {
            let again = lines.wait_more(Instant::now() + LINE_DEADLINE, want);
            match again {
                Ok(_) => format!(
                    "it arrived LATE, {:.1} s after the ask",
                    since.elapsed().as_secs_f64()
                ),
                Err(Missed::Deadline) => format!(
                    "still absent {:.1} s after the ask",
                    since.elapsed().as_secs_f64()
                ),
                Err(Missed::Closed) => format!(
                    "standard error closed {:.1} s after the ask without it",
                    since.elapsed().as_secs_f64()
                ),
            }
        }
        Missed::Closed => "standard error had closed".to_owned(),
    };
    let alive = match child.try_wait() {
        Ok(None) => "still running".to_owned(),
        Ok(Some(status)) => format!("exited, {status}"),
        Err(err) => format!("unknown ({err})"),
    };
    let pong = match ping {
        None => "no ping was sent with this ask".to_owned(),
        Some(nonce) => {
            let answered = lines.read.iter().any(|line| {
                is_event(line, "pong") && line.contains(&format!("\"nonce\":{nonce}}}"))
            });
            if answered {
                format!("ctl/ping {nonce} sent with the ask WAS answered")
            } else {
                format!("ctl/ping {nonce} sent with the ask was NOT answered")
            }
        }
    };
    let (health, other): (Vec<&String>, Vec<&String>) =
        lines.read.iter().partition(|line| is_event(line, "health"));
    let mut report = format!(
        "{what}: nothing after {:.1} s ({missed:?}, bound {LINE_DEADLINE:?}); {late}\n\
         child: {alive}\n\
         {pong}\n\
         stdout: {stdout_at_ask} bytes at the ask, {} bytes now\n\
         stderr since the ask: {} lines, {} of them health; the last {} other lines and \
         the last {} health lines follow\n",
        missed_after.as_secs_f64(),
        drain.bytes(),
        lines.read.len(),
        health.len(),
        other.len().min(REPORT_OTHER_LINES),
        health.len().min(REPORT_HEALTH_LINES),
    );
    for line in other.iter().rev().take(REPORT_OTHER_LINES).rev() {
        report.push_str("  | ");
        report.push_str(line);
        report.push('\n');
    }
    for line in health.iter().rev().take(REPORT_HEALTH_LINES).rev() {
        report.push_str("  | ");
        report.push_str(line);
        report.push('\n');
    }
    report
}

/// Read the child's standard error line by line **while it runs**, forwarding
/// every line on a channel and accumulating the whole of it for the assertions.
fn watch(child: &mut Child) -> (std::thread::JoinHandle<String>, Lines) {
    let opened = Instant::now();
    let pipe = child.stderr.take().expect("stderr was piped");
    let (tx, rx) = std::sync::mpsc::channel::<String>();
    let handle = std::thread::spawn(move || {
        let mut text = String::new();
        for line in std::io::BufReader::new(pipe).lines().map_while(Result::ok) {
            // A dropped receiver means the test already has what it waited for;
            // keep accumulating so the assertions still see the whole run.
            let _ = tx.send(line.clone());
            text.push_str(&line);
            text.push('\n');
        }
        text
    });
    let lines = Lines {
        rx,
        opened,
        read: Vec::new(),
    };
    (handle, lines)
}

/// Wait for the child, join both threads, and hand back its standard error — or
/// `None` when the runner cannot run this mode at all.
fn finish(
    mut child: Child,
    drain: Drain,
    collector: std::thread::JoinHandle<String>,
) -> Option<String> {
    let status = child.wait().expect("wait for the player");
    drain.join();
    let stderr = collector.join().expect("the collector did not panic");
    if let Some(reason) = unrunnable(&stderr) {
        eprintln!("skipped: {reason}");
        return None;
    }
    assert!(status.success(), "the headless run failed:\n{stderr}");
    Some(stderr)
}

/// Whether a line is the event of the named kind.
fn is_event(line: &str, ev: &str) -> bool {
    line.starts_with('{') && line.contains(&format!("\"ev\":\"{ev}\""))
}

/// Every event line of the named kind.
fn events<'a>(stderr: &'a str, ev: &str) -> Vec<&'a str> {
    stderr.lines().filter(|line| is_event(line, ev)).collect()
}

/// The bound control address a `hello` line carries, or `None` for any other
/// line and for a `hello` that bound nothing.
///
/// Parsed by hand rather than with a JSON dependency: the field is one string in
/// a line this repository writes, and a test crate is not the place to take a
/// crate for it.
fn hello_control(line: &str) -> Option<SocketAddr> {
    if !is_event(line, "hello") {
        return None;
    }
    let rest = line.split("\"control\":").nth(1)?.trim_start();
    rest.strip_prefix('"')?.split('"').next()?.parse().ok()
}

/// A headless run reports the show it is running: the roster it loaded, the
/// preset on screen, and a file that would not parse — the same three causes a
/// windowed run reports them for, reached because it runs the same code.
///
/// The broken file is written **after the startup roster has been seen**, so
/// what the second roster proves is the watcher: a file that arrived before the
/// startup load would be reported by the load instead, and the reload would have
/// nothing to say.
#[test]
fn a_headless_run_emits_the_roster_the_preset_and_a_preset_error() {
    let dir = scratch("roster");
    std::fs::write(dir.join("probe.toml"), GOOD).expect("write the good preset");

    // Ten seconds of frames at 30 fps: the run has to outlive process start on a
    // loaded runner plus one 150 ms watcher poll after the edit below.
    let (mut child, drain) = spawn(&dir, &["--frames", "300"]);
    let (collector, mut lines) = watch(&mut child);

    let startup_roster = lines
        .wait_for(lines.opened, |line| {
            is_event(line, "roster").then(|| line.to_owned())
        })
        .ok();
    if startup_roster.is_some() {
        std::fs::write(dir.join("broken.toml"), BROKEN).expect("write the broken preset");
    }
    let Some(stderr) = finish(child, drain, collector) else {
        return;
    };
    let startup_roster = startup_roster.expect("the run produced no roster at all");

    assert!(
        startup_roster.contains("Show Probe"),
        "the startup roster should name the preset the directory holds: \
         {startup_roster}"
    );
    assert!(
        events(&stderr, "roster").len() >= 2,
        "the watcher should report a second roster after the edit in:\n{stderr}"
    );
    assert!(
        events(&stderr, "preset")
            .iter()
            .any(|line| line.contains("Show Probe")),
        "the preset on screen should be reported once it is drawn in:\n{stderr}"
    );
    let errors = events(&stderr, "preset_error");
    assert!(
        !errors.is_empty(),
        "a file that does not parse should reach the stream as a preset_error \
         in:\n{stderr}"
    );
    assert!(
        errors[0].contains("broken.toml") && errors[0].contains("\"line\":"),
        "a preset_error should name the file and the line an editor puts a \
         cursor on: {}",
        errors[0]
    );
}

/// `--control` on the headless path binds a real socket, says which one in
/// `hello`, and drains it between frames: a `ctl/preset` moves the show, a
/// `ctl/ping` is answered, and a `ctl/preset` naming nothing in the roster is
/// **reported** rather than silently doing nothing.
///
/// That last one is the studio's silent click (ADR-0221): before it, a name the
/// renderer declined produced no event, no counter and no line, so a click on a
/// preset the player would not select looked exactly like one it was about to
/// select.
#[test]
fn a_headless_run_binds_the_control_listener_and_drains_it() {
    let dir = scratch("control");
    std::fs::write(dir.join("one.toml"), GOOD).expect("write the first preset");
    std::fs::write(dir.join("two.toml"), TARGET).expect("write the second preset");

    let (mut child, drain) = spawn(&dir, &["--frames", "300", "--control", "127.0.0.1:0"]);
    let (collector, mut lines) = watch(&mut child);

    // The port is ephemeral: it exists only in `hello`, which is emitted before
    // the first frame, so the datagrams below cannot be sent until it arrives.
    let target = lines.wait_for(lines.opened, hello_control).ok();
    if let Some(target) = target {
        let socket = UdpSocket::bind("127.0.0.1:0").expect("bind the sender");
        let mut buf = Vec::new();
        Action::Preset {
            name: Name::new("Show Target").expect("a name inside the inline cap"),
        }
        .encode(&mut buf);
        socket.send_to(&buf, target).expect("send ctl/preset");
        Action::Ping(PING_NONCE).encode(&mut buf);
        socket.send_to(&buf, target).expect("send ctl/ping");
        // Awaited rather than sent straight after the good one: the queue keeps
        // the **last** preset asked for per frame, so two in one drain would
        // leave only this one and the switch above would never be applied.
        let landed = lines.wait_for(lines.opened, |line| {
            (is_event(line, "preset") && field(line, "name").as_deref() == Some("Show Target"))
                .then_some(())
        });
        if landed.is_ok() {
            Action::Preset {
                name: Name::new(ABSENT_PRESET).expect("a name inside the inline cap"),
            }
            .encode(&mut buf);
            socket
                .send_to(&buf, target)
                .expect("send the unselectable ctl/preset");
        }
    }
    let Some(stderr) = finish(child, drain, collector) else {
        return;
    };
    assert!(
        target.is_some(),
        "`--control` on the headless path should bind a socket and report it in \
         hello, not report null:\n{stderr}"
    );

    assert!(
        events(&stderr, "pong")
            .iter()
            .any(|line| line.contains(&PING_NONCE.to_string())),
        "a ctl/ping should be answered on the event stream in:\n{stderr}"
    );
    assert!(
        events(&stderr, "preset")
            .iter()
            .any(|line| line.contains("Show Target")),
        "a ctl/preset should move the show, and the move should be reported \
         in:\n{stderr}"
    );

    let errors = events(&stderr, "preset_error");
    let naming = errors
        .iter()
        .filter(|line| field(line, "file").as_deref() == Some(ABSENT_PRESET))
        .count();
    assert_eq!(
        naming, 1,
        "a ctl/preset naming a preset the roster does not hold should produce \
         exactly one preset_error naming it; {naming} of the {} preset_error \
         lines did:\n{stderr}",
        errors.len()
    );
    assert!(
        !errors
            .iter()
            .any(|line| field(line, "file").as_deref() == Some("Show Target")),
        "the ctl/preset that landed must raise no preset_error, or the studio \
         puts a red line on every preset click that worked:\n{stderr}"
    );
}

/// A run on a machine with no per-user data directory still runs, on the
/// embedded set, and says why in one line.
///
/// `PresetDir::Unresolved` is a degrade, never a failure (NFR 10): the headless
/// path gained a preset directory with the show, and a CI runner that has no
/// such directory must not thereby have gained a way to fail.
#[test]
fn a_headless_run_with_no_per_user_directory_keeps_the_embedded_set() {
    let (mut child, drain) = spawn(Path::new(""), &["--frames", "30"]);
    let (collector, _lines) = watch(&mut child);
    let Some(stderr) = finish(child, drain, collector) else {
        return;
    };

    assert!(
        stderr.contains("could not resolve a per-user data directory"),
        "an unresolved directory should say so in one line in:\n{stderr}"
    );
    let roster = events(&stderr, "roster");
    assert!(
        !roster.is_empty() && !roster[0].contains("\"names\":[]"),
        "the embedded set should still be the roster in:\n{stderr}"
    );
}

// ---------------------------------------------------------------------------
// What the run says about what it loaded (ADR-0184)
// ---------------------------------------------------------------------------

/// What a system needs beyond its key before it will load — the same two
/// exceptions `core/tests/suite/preset.rs` names, which is where the claim that these
/// are the only two is asserted.
fn extras(kind: SystemKind) -> &'static str {
    match kind {
        SystemKind::StarPattern => {
            "[generator]\ntiling = \"none\"\nrings = [ { motif = \"circle\", count = 6, radius = 0.4, scale = 0.2 } ]\n"
        }
        SystemKind::LSystem => {
            "[generator]\naxiom = \"F\"\nrules = { F = \"F[+F]F\" }\nangle_deg = 22\nmax_depth = 3\n"
        }
        _ => "",
    }
}

/// A string field's value, unescaped, or `None` when the key is absent or
/// holds `null`.
///
/// Parsed by hand for `hello_control`'s reason: these are single scalars in a
/// line this repository writes, and a test crate is not the place to take a JSON
/// dependency for them. The unescaping is not decoration — every path on a
/// Windows run arrives with each separator doubled, so a comparison against a
/// `Path` fails on nothing but the escaping without it.
fn field(line: &str, key: &str) -> Option<String> {
    let rest = line.split_once(&format!("\"{key}\":"))?.1;
    let mut out = String::new();
    let mut chars = rest.strip_prefix('"')?.chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => return Some(out),
            '\\' => out.push(chars.next()?),
            c => out.push(c),
        }
    }
    None
}

/// Whether the named key is present and holds `null`.
///
/// Distinct from `field(..).is_none()`, which is also true for a key that is not
/// there at all: `null` is a value this contract promises, and a field that
/// vanished instead would be a different break.
fn is_null(line: &str, key: &str) -> bool {
    line.split_once(&format!("\"{key}\":"))
        .is_some_and(|(_, rest)| rest.starts_with("null"))
}

/// The system keys the **built binary's own** `--schema` document labels its
/// parameter rosters with.
///
/// Taken from the same executable under test rather than from `rlx_core` in this
/// process, so what is compared is what a parent would actually receive.
fn schema_system_keys() -> Vec<String> {
    let out = common::player()
        .arg("--schema")
        .output()
        .expect("run the binary with --schema");
    assert!(out.status.success(), "--schema did not exit zero");
    let doc = String::from_utf8(out.stdout).expect("the schema document is UTF-8");
    let systems = doc
        .split_once("\"systems\":[")
        .expect("the document declares a systems array")
        .1
        .split_once("],\"stages\":[")
        .expect("the systems array ends where the stages begin")
        .0;
    systems
        .split("\"name\":\"")
        .skip(1)
        .filter_map(|rest| {
            let (label, after) = rest.split_once('"')?;
            after.starts_with(",\"params\":[").then(|| label.to_owned())
        })
        .collect()
}

/// Stop the child now and hand back its standard error, or `None` when the
/// runner cannot run this mode.
///
/// The counterpart to `finish` for a test that has seen everything it came for:
/// a walk over the whole system roster is done when the last preset is on
/// screen, and sizing `--frames` to cover it would make the test pay for the
/// slowest plausible runner on every run.
fn stop(
    mut child: Child,
    drain: Drain,
    collector: std::thread::JoinHandle<String>,
) -> Option<String> {
    let _ = child.kill();
    let _ = child.wait();
    drain.join();
    let stderr = collector.join().expect("the collector did not panic");
    if let Some(reason) = unrunnable(&stderr) {
        eprintln!("skipped: {reason}");
        return None;
    }
    Some(stderr)
}

/// The `preset` event names the system by the key the schema labels that
/// system's parameter roster with — for **every** system, not for one.
///
/// The whole roster rather than a sample, because the canonical key and the
/// scene's display name are the same string for the systems whose names are one
/// word and different for the rest: a check that named only `swarm` or
/// `attractor` would pass against either accessor, and the panel a parent builds
/// from the display name would find no parameters for two systems in three.
///
/// One process walks the roster over the control channel: rotation is held
/// first, so nothing but this test moves the show, and each step waits for the
/// `preset` event that says the dissolve finished before asking for the next.
#[test]
fn every_system_is_reported_by_the_key_the_schema_labels_its_roster_with() {
    let dir = scratch("system-keys");
    for kind in SystemKind::ALL {
        let key = kind.as_str();
        std::fs::write(
            dir.join(format!("{key}.toml")),
            format!("name = \"{key}\"\nsystem = \"{key}\"\n{}", extras(kind)),
        )
        .expect("write one preset per system");
    }

    let (mut child, drain) = spawn(&dir, &["--frames", "9000", "--control", "127.0.0.1:0"]);
    let (collector, mut lines) = watch(&mut child);

    let mut seen: Vec<(String, String)> = Vec::new();
    // The report of the one wait that ended without its line, taken while the
    // child is still in the state that wait left it in.
    let mut unanswered: Option<String> = None;
    let hello = Ask {
        what: "the `hello` line naming the bound control address".to_owned(),
        since: lines.opened,
        stdout_at_ask: 0,
        ping: None,
    };
    match lines.wait_for(hello.since, hello_control) {
        Err(missed) => {
            unanswered = Some(missed_report(
                &hello,
                missed,
                &mut lines,
                hello_control,
                &mut child,
                &drain,
            ));
        }
        Ok(target) => {
            let socket = UdpSocket::bind("127.0.0.1:0").expect("bind the sender");
            let mut buf = Vec::new();
            // Rotation off first: the director would otherwise move the show
            // underneath the walk and the events would not line up with the asks.
            Action::Transport(Transport::Hold).encode(&mut buf);
            socket
                .send_to(&buf, target)
                .expect("send ctl/transport hold");

            for (step, kind) in SystemKind::ALL.into_iter().enumerate() {
                let key = kind.as_str();
                Action::Preset {
                    name: Name::new(key).expect("a system key inside the inline cap"),
                }
                .encode(&mut buf);
                let mut ask = Ask::now(
                    format!(
                        "ctl/preset `{key}` (ask {} of {}), awaiting its `preset` event",
                        step + 1,
                        SystemKind::VARIANT_COUNT
                    ),
                    &drain,
                );
                socket.send_to(&buf, target).expect("send ctl/preset");
                // Evidence only, never asserted: see `Ask::ping`.
                let nonce = PING_NONCE + step as i32;
                Action::Ping(nonce).encode(&mut buf);
                socket.send_to(&buf, target).expect("send ctl/ping");
                ask.ping = Some(nonce);
                let want = |line: &str| {
                    (is_event(line, "preset") && field(line, "name").as_deref() == Some(key))
                        .then(|| line.to_owned())
                };
                match lines.wait_for(ask.since, want) {
                    Ok(line) => seen.push((
                        key.to_owned(),
                        field(&line, "system")
                            .unwrap_or_else(|| panic!("no system field on: {line}")),
                    )),
                    Err(missed) => {
                        unanswered = Some(missed_report(
                            &ask, missed, &mut lines, want, &mut child, &drain,
                        ));
                        break;
                    }
                }
            }
        }
    }
    let Some(stderr) = stop(child, drain, collector) else {
        return;
    };

    let keys = schema_system_keys();
    // The report rather than the whole of standard error: a walk that stalls
    // leaves a minute of `health` lines behind it, and the lines that say why
    // are the few the report keeps.
    let why = unanswered.unwrap_or_else(|| {
        let tail: Vec<&str> = stderr.lines().rev().take(REPORT_OTHER_LINES).collect();
        let tail: Vec<&str> = tail.into_iter().rev().collect();
        format!(
            "no wait missed; the last lines of stderr:\n{}",
            tail.join("\n")
        )
    });
    assert_eq!(
        seen.len(),
        SystemKind::VARIANT_COUNT,
        "the walk reported {} of {} systems; it stops at the first preset that \
         never reaches the screen:\n{why}",
        seen.len(),
        SystemKind::VARIANT_COUNT
    );
    for (asked, reported) in &seen {
        assert_eq!(
            reported, asked,
            "the show was dissolved to the preset whose only system is \
             `{asked}` and the event called it `{reported}` — the display name \
             for that system, not the key"
        );
        assert!(
            keys.iter().any(|key| key == reported),
            "`{reported}` labels no roster in the schema document this binary \
             prints, so a parent told it cannot find that system's parameters; \
             the document labels {keys:?}"
        );
    }
}

/// The `roster` event names the directory the run loaded and watches, and the
/// `preset` event names the file the preset on screen came from.
///
/// The directory is checked against the **human** line the same load prints, not
/// only against what the test asked for: the two come from one value, and a
/// structured field that agreed with the test's own input while disagreeing with
/// the run's own diagnostic would be reporting the request rather than the fact.
#[test]
fn the_roster_names_the_directory_and_the_preset_names_its_file() {
    let dir = scratch("directory");
    std::fs::write(dir.join("probe.toml"), GOOD).expect("write the good preset");

    let (mut child, drain) = spawn(&dir, &["--frames", "300"]);
    let (collector, mut lines) = watch(&mut child);
    let seen = lines
        .wait_for(lines.opened, |line| {
            (is_event(line, "preset") && field(line, "name").as_deref() == Some("Show Probe"))
                .then(|| line.to_owned())
        })
        .ok();
    let Some(stderr) = stop(child, drain, collector) else {
        return;
    };
    let preset = seen.expect("the run never reported a preset on screen");

    let roster = events(&stderr, "roster");
    let reported_dir = roster
        .first()
        .and_then(|line| field(line, "dir"))
        .unwrap_or_else(|| panic!("the roster carried no directory in:\n{stderr}"));

    let printed = stderr
        .lines()
        .find_map(|line| line.split_once("preset(s) from "))
        .map(|(_, dir)| dir.to_owned())
        .unwrap_or_else(|| panic!("the run printed no load line in:\n{stderr}"));
    assert_eq!(
        reported_dir, printed,
        "the roster's directory and the line the same load printed disagree, so \
         one of them is not the directory the watcher polls"
    );

    let expected = std::path::absolute(&dir).expect("absolute scratch path");
    assert_eq!(
        Path::new(&reported_dir),
        expected.as_path(),
        "the roster named a directory this test did not point the run at"
    );

    let file =
        field(&preset, "file").unwrap_or_else(|| panic!("the preset carried no file: {preset}"));
    assert_eq!(
        Path::new(&file),
        expected.join("probe.toml").as_path(),
        "the preset named a file it was not read from"
    );
    assert!(
        Path::new(&file).is_absolute() && Path::new(&reported_dir).is_absolute(),
        "a relative path is unusable to a parent process, which does not share \
         this run's working directory: {reported_dir} / {file}"
    );
}

/// A run on the embedded set reports a `null` file and a `null` directory.
///
/// The arm a parent offering to edit the preset on screen branches on: the
/// embedded set has no file on disk and there is nowhere the watcher is looking,
/// and both have to be sayable. `0`-like stand-ins — an empty string, an omitted
/// key — would each read as an editable path pointing at the wrong place.
#[test]
fn the_embedded_set_reports_no_file_and_no_directory() {
    let (mut child, drain) = spawn(Path::new(""), &["--frames", "60"]);
    let (collector, mut lines) = watch(&mut child);
    let seen = lines
        .wait_for(lines.opened, |line| {
            is_event(line, "preset").then(|| line.to_owned())
        })
        .ok();
    let Some(stderr) = stop(child, drain, collector) else {
        return;
    };

    let roster = events(&stderr, "roster");
    let roster = roster
        .first()
        .unwrap_or_else(|| panic!("the run produced no roster in:\n{stderr}"));
    assert!(
        is_null(roster, "dir"),
        "an unresolved directory should be reported as null, not omitted and \
         not as a path: {roster}"
    );
    assert!(
        !roster.contains("\"names\":[]"),
        "the embedded set should still be the roster: {roster}"
    );

    let preset = seen.expect("the run never reported a preset on screen");
    assert!(
        is_null(&preset, "file"),
        "an embedded preset has no file and should say so as null: {preset}"
    );
    assert!(
        field(&preset, "system").is_some_and(|system| !system.is_empty()),
        "the system is known whether or not the preset has a file: {preset}"
    );
}

/// A headless run reports `null` preview counters.
///
/// There is no preview pipe on this path — the headless sink writes the frames
/// itself and blocks rather than dropping — so there is no producer-side loss to
/// report. `null` says that; `0` would claim a preview that lost nothing, which
/// is the reading ADR-0184 exists to stop the studio from making.
#[test]
fn a_headless_run_reports_no_preview_counters() {
    let dir = scratch("preview-counters");
    std::fs::write(dir.join("probe.toml"), GOOD).expect("write the good preset");

    // `health` is emitted once a second while frames are drawn, so the run has
    // to outlive one interval: 90 frames at 30 fps is three.
    let (mut child, drain) = spawn(&dir, &["--frames", "90"]);
    let (collector, _lines) = watch(&mut child);
    let Some(stderr) = finish(child, drain, collector) else {
        return;
    };

    let health = events(&stderr, "health");
    assert!(
        !health.is_empty(),
        "the run drew frames for three health intervals and reported none:\n{stderr}"
    );
    for line in &health {
        assert!(
            is_null(line, "preview_sent") && is_null(line, "preview_dropped"),
            "a run with no preview pipe should report both counters as null: {line}"
        );
    }
}

/// Where a run whose data root env vars all name `root` writes its diagnostics
/// log — the per-OS arms of `standalone::preset_data_root`, plus the app dir.
fn diagnostics_log_under(root: &Path) -> PathBuf {
    #[cfg(target_os = "macos")]
    let root = root.join("Library").join("Application Support");
    #[cfg(not(target_os = "macos"))]
    let root = root.to_path_buf();
    root.join(standalone::APP_DIR_NAME).join("diagnostics.log")
}

/// A headless run writes the same `diagnostics.log` rows a windowed run writes,
/// and the rate in them is the rate it drew at rather than zero.
///
/// The data root is this test's own directory, so the log it reads is the one
/// this run wrote and the developer's real per-user directory is never touched.
#[test]
fn a_headless_run_writes_diagnostics_rows_with_a_live_rate() {
    let presets = scratch("diaglog-presets");
    std::fs::write(presets.join("probe.toml"), GOOD).expect("write the good preset");
    let root = scratch("diaglog-root");
    let log = diagnostics_log_under(&root);

    // Rows are written once a second: 90 frames at 30 fps is three.
    let (mut child, drain) = spawn_with_data_root(&presets, &root, &["--frames", "90"]);
    let (collector, _lines) = watch(&mut child);
    let Some(stderr) = finish(child, drain, collector) else {
        return;
    };

    let text = std::fs::read_to_string(&log).unwrap_or_else(|err| {
        panic!(
            "the run wrote no diagnostics log at {} ({err}):\n{stderr}",
            log.display()
        )
    });
    let mut lines = text.lines().filter(|line| !line.starts_with('#'));
    let header: Vec<&str> = lines
        .next()
        .expect("the log has a header")
        .split('\t')
        .collect();
    let column = |name: &str| {
        header
            .iter()
            .position(|c| *c == name)
            .unwrap_or_else(|| panic!("the header has no `{name}` column: {header:?}"))
    };
    let (fps, frames_total) = (column("fps"), column("frames_total"));

    let rows: Vec<Vec<&str>> = lines.map(|line| line.split('\t').collect()).collect();
    assert!(
        !rows.is_empty(),
        "a three-second run wrote a header and no rows:\n{text}"
    );
    for row in &rows {
        assert_eq!(
            row.len(),
            header.len(),
            "row width matches the header: {row:?}"
        );
    }
    let last = rows.last().expect("checked non-empty above");
    let rate: f32 = last[fps].parse().expect("the fps field is a number");
    let drawn: u64 = last[frames_total]
        .parse()
        .expect("the frames_total field is a number");
    assert!(
        rate > 0.0 && drawn > 0,
        "the last row reports {rate} fps over {drawn} frames for a run that drew \
         frames the whole time:\n{text}"
    );
}
