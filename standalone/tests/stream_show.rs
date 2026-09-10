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

use std::io::{BufRead, Read};
use std::net::{SocketAddr, UdpSocket};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::Receiver;
use std::time::Duration;

use standalone::osc::decode::{Action, Name};

/// A preset that compiles, so the directory the run is pointed at yields a
/// roster rather than falling back to the embedded set.
const GOOD: &str = "name = \"Show Probe\"\nsystem = \"spectrum\"\n";

/// The second preset, so `ctl/preset` has somewhere to move the show to.
const TARGET: &str = "name = \"Show Target\"\nsystem = \"spectrum\"\n";

/// A preset that does not compile, so a reload has a `preset_error` to report.
/// The unclosed string is a parse failure the loader can put a line number on.
const BROKEN: &str = "name = \"Show Probe Broken\nsystem = \"spectrum\"\n";

/// How long to wait for the next line of a running child's standard error.
///
/// Generous rather than tight: it covers process start, the adapter request and
/// the config read on a runner already busy with the rest of the suite. The wait
/// is only ever paid in full by a genuine failure — the receive returns the
/// moment a line arrives.
const LINE_DEADLINE: Duration = Duration::from_secs(60);

/// The nonce sent on `ctl/ping`, distinctive enough that finding it in a line
/// cannot be a coincidence.
const PING_NONCE: i32 = 424_242;

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

/// A directory this test owns, under the workspace's own target dir so a runner
/// cleans it up with everything else.
fn scratch(tag: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../target/tests/stream-show")
        .join(tag);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create the scratch preset directory");
    dir
}

/// Spawn the player headless, with standard output drained by a thread so the
/// writer never blocks on a full pipe.
///
/// `presets` becomes `RLX_PRESET_DIR`; an empty path leaves the run to resolve
/// its own, which with `APPDATA` cleared is the unresolved case. `APPDATA`,
/// `HOME` and `XDG_DATA_HOME` are all cleared so the child cannot reach the
/// developer's real per-user directory on any platform — the presets are
/// redirected already, and this keeps the config, the diagnostics log and the
/// directory migration off it too.
fn spawn(presets: &Path, extra: &[&str]) -> (Child, std::thread::JoinHandle<()>) {
    let mut args = vec![
        "--stream", "--sink", "stdout", "--events", "--fps", "30", "--size", "160x90",
    ];
    args.extend_from_slice(extra);
    let mut child = Command::new(env!("CARGO_BIN_EXE_ritmolux"))
        .args(&args)
        .env("RLX_PRESET_DIR", presets)
        .env("APPDATA", "")
        .env("HOME", "")
        .env("XDG_DATA_HOME", "")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn the built binary");
    let mut pipe = child.stdout.take().expect("stdout was piped");
    let drain = std::thread::spawn(move || {
        let mut buf = vec![0u8; 64 * 1024];
        while let Ok(n) = pipe.read(&mut buf)
            && n > 0
        {}
    });
    (child, drain)
}

/// Read the child's standard error line by line **while it runs**, forwarding
/// every line on a channel and accumulating the whole of it for the assertions.
fn watch(child: &mut Child) -> (std::thread::JoinHandle<String>, Receiver<String>) {
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
    (handle, rx)
}

/// Wait for the first line `want` accepts, or `None` if the run ends first.
fn wait_for<T>(rx: &Receiver<String>, want: impl Fn(&str) -> Option<T>) -> Option<T> {
    while let Ok(line) = rx.recv_timeout(LINE_DEADLINE) {
        if let Some(found) = want(&line) {
            return Some(found);
        }
    }
    None
}

/// Wait for the child, join both threads, and hand back its standard error — or
/// `None` when the runner cannot run this mode at all.
fn finish(
    mut child: Child,
    drain: std::thread::JoinHandle<()>,
    collector: std::thread::JoinHandle<String>,
) -> Option<String> {
    let status = child.wait().expect("wait for the player");
    drain.join().expect("the drain thread did not panic");
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
    let (collector, rx) = watch(&mut child);

    let startup_roster = wait_for(&rx, |line| {
        is_event(line, "roster").then(|| line.to_owned())
    });
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
/// `hello`, and drains it between frames: a `ctl/preset` moves the show and a
/// `ctl/ping` is answered.
#[test]
fn a_headless_run_binds_the_control_listener_and_drains_it() {
    let dir = scratch("control");
    std::fs::write(dir.join("one.toml"), GOOD).expect("write the first preset");
    std::fs::write(dir.join("two.toml"), TARGET).expect("write the second preset");

    let (mut child, drain) = spawn(&dir, &["--frames", "300", "--control", "127.0.0.1:0"]);
    let (collector, rx) = watch(&mut child);

    // The port is ephemeral: it exists only in `hello`, which is emitted before
    // the first frame, so the datagrams below cannot be sent until it arrives.
    let target = wait_for(&rx, hello_control);
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
    let (collector, _rx) = watch(&mut child);
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
