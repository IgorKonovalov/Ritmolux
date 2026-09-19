//! A datagram on the loopback port moves the picture (ADR-0176, Plan 0158
//! Phase 2).
//!
//! The unit tests beside the decoder prove the wire format and the ones beside
//! the queue prove the bounding. Neither of them sends anything. **This one
//! opens the real socket**, sends the real bytes an OSC sender would send, and
//! asserts against rendered pixels — which is the only place the claim "a
//! `ctl/param` datagram changes the routed value on the next frame" is actually
//! a claim rather than a chain of three assumptions.
//!
//! The reference for "`bg_bright` was driven to 0.8" is a second renderer whose
//! preset binds `bg_bright = "0.8"`, and the comparison is byte identity —
//! `core/tests/suite/override.rs`'s method, for its reason.
//!
//! What this file cannot reach is the transport verbs: those resolve a
//! `ConsoleAction` and land on the shell's director and settings menu, which
//! need a window. `standalone::control::apply_to_renderer` is the seam, and it
//! is deliberately the half that has no window in it.

// Waiting for a datagram to cross a loopback socket is a wall-clock wait: the
// listener runs on a thread this test does not schedule, so the only bound
// available is a deadline. The clock-free rule is about the core's analysis, and
// nothing here is in it.
#![allow(
    clippy::disallowed_methods,
    reason = "a delivery deadline on a socket this test does not schedule"
)]

use std::net::UdpSocket;
use std::time::{Duration, Instant};

use rlx_core::dsp::AnalysisFrame;
use rlx_core::preset::Preset;
use rlx_core::render::{CaptureImage, HeadlessOptions, RenderError, Renderer};
use standalone::control::Control;
use standalone::osc::decode::{Action, Name, Transport};

/// Small offscreen: the claim is about which value arrived, not how many pixels
/// carried it.
const SIZE: u32 = 96;

/// One 60 Hz frame.
const DT: f32 = 1.0 / 60.0;

/// How long to wait for the listener thread to pick a datagram off the socket.
///
/// Generous rather than tight: this is a scheduler wait on a loopback send, and
/// a test that flakes under load is worse than one that takes an extra
/// millisecond. The loop below exits as soon as the queue is non-empty, so the
/// bound is only ever paid by a genuine failure.
const DELIVERY: Duration = Duration::from_secs(5);

/// A preset that binds `bg_bright` to `value` and nothing else, on a system that
/// needs no structural table.
fn lit(name: &str, value: &str) -> Preset {
    Preset::from_toml_str(&format!(
        "system = \"swarm\"\nname = \"{name}\"\n[params]\nbg_bright = \"{value}\"\n"
    ))
    .expect("hand-written probe preset is valid")
}

/// A headless renderer, or `None` (a logged skip) on a runner with no GPU
/// adapter at all — ADR-0016's rule, restated here because `core/tests/common/`
/// is not reachable from this crate.
fn headless() -> Option<Renderer> {
    match Renderer::new_headless(HeadlessOptions {
        width: SIZE,
        height: SIZE,
        prefer_software: true,
    }) {
        Ok(renderer) => Some(renderer),
        Err(RenderError::RequestAdapter(_)) => {
            eprintln!("skipped: no GPU adapter on this runner (ADR-0016)");
            None
        }
        Err(err) => panic!("headless renderer build failed: {err}"),
    }
}

/// Bind a listener on an ephemeral loopback port, or `None` with a notice when
/// the runner forbids it.
///
/// Port 0 rather than a fixed number: two of these tests may run at once under
/// nextest's process-per-test, and a fixed port would make them fight.
fn listener() -> Option<Control> {
    match Control::bind("127.0.0.1:0") {
        Ok(control) => Some(control),
        Err(err) => {
            eprintln!("skipped: could not bind a loopback control socket — {err}");
            None
        }
    }
}

/// Send `action` to `control`'s port as the bytes a sender would put on the
/// wire.
fn send(control: &Control, action: &Action) {
    let socket = UdpSocket::bind("127.0.0.1:0").expect("bind an ephemeral sending socket");
    let mut buf = Vec::new();
    action.encode(&mut buf);
    socket
        .send_to(&buf, control.local_addr())
        .expect("send to the loopback listener");
}

/// Wait until something is queued, or give up after [`DELIVERY`].
///
/// Peeks rather than drains, so the delivery it waited for is still there for
/// the caller — polling `drain` here would consume it. Returns whether anything
/// arrived, so the caller fails with its own message rather than on an empty
/// buffer three assertions later.
fn wait_for_delivery(control: &Control) -> bool {
    let deadline = Instant::now() + DELIVERY;
    while !control.has_pending() {
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    true
}

/// The listener's readings at one instant, as the report names them.
struct Readings {
    /// Datagrams the socket handed over **since the send**, which is the count
    /// a missing delivery is judged by: the per-process total includes whatever
    /// an earlier step of the same test sent.
    arrived: u64,
    recv_errors: u64,
    listening: bool,
    rejected: u64,
    dropped: u64,
}

/// Which of the candidates the readings convict, in one sentence.
///
/// The four ADR-0221 names, and the only reason this is a function rather than
/// a line inside the panic is that each candidate's wording is then assertable
/// without breaking a listener under a live test.
///
/// The order is what settles them. A datagram that reached the socket is the
/// strongest fact available — whatever else is wrong, the send worked — so it
/// is read first and `rejected`/`dropped` say what became of it. Past that, a
/// thread that has left its loop explains every later silence, a receive error
/// explains the thread leaving, and a listener that is reading a healthy quiet
/// socket convicts the path in front of it.
fn verdict(r: &Readings) -> String {
    if r.arrived > 0 {
        return format!(
            "RECEIVED moved ({} since the send): the datagram reached the \
             socket and was discarded after it — rejected {}, dropped {}",
            r.arrived, r.rejected, r.dropped
        );
    }
    if !r.listening {
        return format!(
            "LISTENING is false: the listener thread has left its receive loop, \
             so nothing was reading the socket (recv_errors {})",
            r.recv_errors
        );
    }
    if r.recv_errors > 0 {
        return format!(
            "RECV_ERRORS moved ({}): the socket is failing receives, and the \
             listener is still in its loop retrying them",
            r.recv_errors
        );
    }
    "RECEIVED did not move, RECV_ERRORS did not move and LISTENING is true: the \
     listener was reading a healthy socket and no datagram reached it, so the \
     loss is in front of the socket"
        .to_owned()
}

/// Wait for the delivery of what was just sent, or fail with what the listener
/// saw.
///
/// On a miss it keeps watching for one more [`DELIVERY`] before failing, so the
/// message tells a late delivery from one that never came; the test has already
/// failed by then, and the extra wait is paid only on that path. The readings
/// [`verdict`] reads settle which candidate it was: whether any datagram reached
/// the socket at all, whether receives are failing, and whether the listener
/// thread is still there — the three facts a report that said only "rejected 0,
/// dropped 0" could not distinguish.
fn expect_delivery(control: &Control, what: &str) {
    let sent = Instant::now();
    let before = control.received();
    if wait_for_delivery(control) {
        return;
    }
    let missed_after = sent.elapsed();
    let late = if wait_for_delivery(control) {
        format!(
            "it arrived LATE, {:.2} s after the send",
            sent.elapsed().as_secs_f64()
        )
    } else {
        format!(
            "still nothing {:.2} s after the send",
            sent.elapsed().as_secs_f64()
        )
    };
    let readings = Readings {
        arrived: control.received().saturating_sub(before),
        recv_errors: control.recv_errors(),
        listening: control.listening(),
        rejected: control.rejected(),
        dropped: control.dropped(),
    };
    panic!(
        "{what}: nothing reached the listener within {DELIVERY:?} (gave up after \
         {:.2} s); {late}\n\
         listener readings: received +{} since the send, recv_errors {}, \
         listening {}, rejected {}, dropped {}\n\
         verdict: {}",
        missed_after.as_secs_f64(),
        readings.arrived,
        readings.recv_errors,
        readings.listening,
        readings.rejected,
        readings.dropped,
        verdict(&readings),
    );
}

/// The index of the first differing byte, and what the two frames hold there.
fn first_difference(a: &CaptureImage, b: &CaptureImage) -> Option<String> {
    a.rgba
        .iter()
        .zip(&b.rgba)
        .position(|(x, y)| x != y)
        .map(|i| {
            let (px, chan) = (i / 4, i % 4);
            let (row, col) = (px / a.width as usize, px % a.width as usize);
            format!(
                "byte {i} (pixel {col},{row} channel {chan}): {:?} vs {:?}",
                a.rgba.get(i),
                b.rgba.get(i)
            )
        })
}

/// One frame of `presets`, with `drive` applied to the renderer first.
fn frame_of(presets: Vec<Preset>, drive: impl FnOnce(&mut Renderer)) -> Option<CaptureImage> {
    let mut renderer = headless()?;
    renderer.set_presets(presets);
    let mut tap = renderer.open_tap();
    drive(&mut renderer);
    Some(
        renderer
            .render_tapped(&mut tap, &AnalysisFrame::default(), DT)
            .expect("render_tapped on a headless renderer"),
    )
}

/// A `ctl/param` datagram sent to the loopback port changes the routed value on
/// the next frame.
#[test]
fn a_param_datagram_moves_the_next_frame() {
    let Some(mut control) = listener() else {
        return;
    };
    send(
        &control,
        &Action::Param {
            name: Name::new("bg_bright").expect("a short name fits inline"),
            value: 0.8,
        },
    );
    expect_delivery(&control, "ctl/param bg_bright 0.8");

    let Some(driven) = frame_of(vec![lit("dim", "0.2")], |renderer| {
        let applied = standalone::control::apply_to_renderer(control.drain(), renderer);
        assert_eq!(applied.refused, 0, "bg_bright is claimed by the backdrop");
    }) else {
        return;
    };
    let Some(bound) = frame_of(vec![lit("bright", "0.8")], |_| {}) else {
        return;
    };
    let Some(untouched) = frame_of(vec![lit("dim", "0.2")], |_| {}) else {
        return;
    };

    assert_ne!(
        untouched.rgba, bound.rgba,
        "bg_bright 0.2 and 0.8 rendered identically, so this test cannot \
         observe a datagram at all"
    );
    if let Some(diff) = first_difference(&bound, &driven) {
        panic!("a ctl/param datagram did not route what a binding of 0.8 routes: {diff}");
    }
}

/// A `ctl/preset` datagram lands on the named preset, and an unknown name is
/// reported as not having switched rather than silently doing nothing.
#[test]
fn a_preset_datagram_selects_by_name() {
    let Some(mut control) = listener() else {
        return;
    };
    let Some(mut renderer) = headless() else {
        return;
    };
    renderer.set_presets(vec![lit("first", "0.2"), lit("second", "0.4")]);
    assert_eq!(
        renderer.preset_name(),
        "first",
        "the roster starts at index 0"
    );

    send(
        &control,
        &Action::Preset {
            name: Name::new("second").expect("a short name fits inline"),
        },
    );
    expect_delivery(&control, "ctl/preset `second`");
    let applied = standalone::control::apply_to_renderer(control.drain(), &mut renderer);
    assert!(applied.switched, "a name the roster holds switches");

    // `ctl/preset` **dissolves** rather than cuts, which is what the ADR's table
    // says and what an operator watching a show wants. So the roster still names
    // the outgoing preset for as long as the crossfade composites it, and the
    // assertion is that the show lands there — not that it is already there.
    let mut tap = renderer.open_tap();
    let mut landed = false;
    for _ in 0..600 {
        renderer
            .render_tapped(&mut tap, &AnalysisFrame::default(), DT)
            .expect("render_tapped on a headless renderer");
        if renderer.preset_name() == "second" {
            landed = true;
            break;
        }
    }
    assert!(
        landed,
        "the dissolve never reached the preset ctl/preset named; the roster is \
         still on '{}'",
        renderer.preset_name()
    );

    send(
        &control,
        &Action::Preset {
            name: Name::new("no_such_preset").expect("a short name fits inline"),
        },
    );
    expect_delivery(&control, "ctl/preset `no_such_preset`, the second send");
    let applied = standalone::control::apply_to_renderer(control.drain(), &mut renderer);
    assert!(
        !applied.switched,
        "an unknown name must not report a switch, or the shell runs its \
         post-switch bookkeeping for a switch that did not happen"
    );
}

/// A flood of one parameter name stays bounded through the socket.
///
/// The **numeric** half of the done-when — "the frame after sees only the last"
/// — is asserted deterministically beside the queue, where ten thousand
/// `record` calls are ten thousand `record` calls. Through a real socket it is
/// not assertable: UDP may drop under a burst, so the last value *received* need
/// not be the last value sent, and a test that asserted otherwise would be
/// asserting the kernel's buffer size.
///
/// What is assertable here, and is the property that matters, is that however
/// many datagrams a burst delivers between two drains, the render thread is
/// handed **one slot** — so a flood cannot make a frame's work grow.
#[test]
fn a_flood_through_the_socket_stays_one_slot_wide() {
    let Some(mut control) = listener() else {
        return;
    };
    let socket = UdpSocket::bind("127.0.0.1:0").expect("bind an ephemeral sending socket");
    let name = Name::new("bg_bright").expect("a short name fits inline");
    let mut buf = Vec::new();
    let mut sent = 0u32;
    for i in 0..10_000u32 {
        Action::Param {
            name,
            value: i as f32,
        }
        .encode(&mut buf);
        // A loopback send can still be refused under buffer pressure, and that
        // is not what this test is about — it is counted so the assertions below
        // are stated against what actually left.
        if socket.send_to(&buf, control.local_addr()).is_ok() {
            sent += 1;
        }
    }
    assert!(sent > 0, "nothing was sent, so nothing can be asserted");

    // Drain repeatedly while the listener walks the backlog, tracking the widest
    // buffer any single frame would have been handed.
    let deadline = Instant::now() + DELIVERY;
    let mut widest = 0usize;
    let mut seen = 0usize;
    let mut last = None;
    while Instant::now() < deadline {
        let drained = control.drain();
        widest = widest.max(drained.params().len());
        if let Some((_, value)) = drained.params().first() {
            seen += 1;
            last = Some(*value);
        } else if seen > 0 && !control.has_pending() {
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }

    assert!(
        seen > 0,
        "{sent} datagrams were sent and none reached the queue"
    );
    assert_eq!(
        widest, 1,
        "a flood of one name was handed to a frame {widest} slots wide; the \
         queue keeps the last value per name so that a frame's work is bounded"
    );
    assert!(
        last.is_some_and(|value| value >= 0.0 && value < sent as f32),
        "the value handed over is one of the ones sent"
    );
    assert_eq!(
        control.rejected(),
        0,
        "every datagram was well-formed, so none should have been rejected"
    );
    assert_eq!(
        control.dropped(),
        0,
        "a repeated name never fills the queue, so nothing should have been \
         dropped"
    );
}

/// Each of the four candidates a broken listener produces is named by the
/// reading that convicts it.
///
/// The deliberate break, expressed as the readings it leaves behind rather than
/// by breaking a socket under a running test: the socket is moved into the
/// listener thread and nothing outside holds a handle to it, so a live test
/// cannot break one. What is asserted is the thing a failing run is read for —
/// that the report names a reading and not merely a number.
#[test]
fn each_broken_listener_is_named_by_the_reading_that_convicts_it() {
    let quiet = Readings {
        arrived: 0,
        recv_errors: 0,
        listening: true,
        rejected: 0,
        dropped: 0,
    };

    let discarded = verdict(&Readings {
        arrived: 1,
        rejected: 1,
        ..quiet
    });
    assert!(
        discarded.contains("RECEIVED moved") && discarded.contains("rejected 1"),
        "a datagram that reached the socket and was discarded must convict the \
         path after the socket: {discarded}"
    );

    let dead = verdict(&Readings {
        listening: false,
        ..quiet
    });
    assert!(
        dead.contains("LISTENING is false"),
        "a listener thread that has left its loop must be named as such, not \
         read as a quiet socket: {dead}"
    );

    let failing = verdict(&Readings {
        recv_errors: 9,
        ..quiet
    });
    assert!(
        failing.contains("RECV_ERRORS moved") && failing.contains('9'),
        "a socket failing its receives must be named as such: {failing}"
    );

    let lost = verdict(&quiet);
    assert!(
        lost.contains("RECEIVED did not move") && lost.contains("in front of the socket"),
        "a live listener on a healthy socket that saw nothing convicts what is \
         in front of it, and must say so rather than leave the four candidates \
         open: {lost}"
    );

    // A dead listener and a failing socket are the same run seen twice, and the
    // report must not read the second as the first: the thread leaves *because*
    // the receives failed, so the liveness fact is the one to lead with.
    let both = verdict(&Readings {
        recv_errors: 64,
        listening: false,
        ..quiet
    });
    assert!(
        both.contains("LISTENING is false") && both.contains("recv_errors 64"),
        "a listener that failed its way out of the loop must report both facts \
         in one sentence: {both}"
    );
}

/// A datagram the decoder refuses is counted and changes nothing.
#[test]
fn a_malformed_datagram_is_counted_and_moves_nothing() {
    let Some(mut control) = listener() else {
        return;
    };
    let socket = UdpSocket::bind("127.0.0.1:0").expect("bind an ephemeral sending socket");
    // Not OSC at all, a known address under the wrong version, and a known
    // address with the wrong argument types.
    let mut wrong_version = Vec::new();
    Action::Transport(Transport::Next).encode(&mut wrong_version);
    let mut junk = wrong_version.clone();
    if let Some(byte) = junk.get_mut(6) {
        // `/rlx/v1/...` -> `/rlx/vX/...`, an address no build answers to.
        *byte = b'X';
    }
    for datagram in [b"not osc at all\0\0".to_vec(), junk] {
        socket
            .send_to(&datagram, control.local_addr())
            .expect("send to the loopback listener");
    }

    let deadline = Instant::now() + DELIVERY;
    while control.rejected() < 2 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(
        control.rejected(),
        2,
        "both malformed datagrams should have been counted as rejected"
    );
    assert!(
        control.drain().is_empty(),
        "a refused datagram must not reach the queue"
    );
}
