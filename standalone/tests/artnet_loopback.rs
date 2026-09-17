//! A configured rig lights up on the simulator, and goes dark when we leave
//! (ADR-0145, ADR-0174).
//!
//! The unit tests beside the encoder prove the packet against the
//! specification and the ones beside the simulator's decoder prove the decoder
//! against it independently. **This one puts the two on a socket**: a sink
//! built from a `config.toml` fixture map sends to a loopback port, the
//! simulator reassembles what arrived into the rig's raster, and the assertions
//! are made against the decoded frames rather than against either side's idea
//! of them.
//!
//! What it cannot see is every physical question — whether the nodes latch,
//! whether a chain's last stick goes stale, what refresh rate the hardware
//! prefers. ADR-0174 enumerates that list, and every item on it is a question
//! only the physical fixtures answer.
//!
//! GPU-free and window-free: the sink is a socket and a byte buffer.

// Draining a socket a sender on another thread is filling is a wall-clock wait:
// nothing here schedules the receiver, so the only bound available is a
// deadline. The clock-free rule is about the core's analysis, and none of this
// is in it.
#![allow(
    clippy::disallowed_methods,
    reason = "a delivery deadline on a socket this test does not schedule"
)]

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use rlx_artnet_sim::{Frame, RIG_PIXELS, RIG_UNIVERSES, Receiver, UdpCapture};
use standalone::artnet::ArtnetSink;
use standalone::config;

/// How long the capture thread waits for the whole run, and how long it keeps
/// draining after the sender says it is finished.
///
/// The grace window is what makes this a test of what was sent rather than of
/// how fast two threads are scheduled: the last datagram is already on the wire
/// when `run` returns, and a receiver that stopped at that instant would report
/// a truncated frame as a short one.
const RUN_DEADLINE: Duration = Duration::from_secs(10);
const GRACE: Duration = Duration::from_millis(400);

/// A fixture map aimed at `port`: the shipped geometry, a colour, and every
/// node on loopback.
///
/// The geometry is [`config::Artnet::default`]'s on purpose — the point of the
/// map being data is that the one thing a different receiver changes is the
/// address.
fn map_for(port: u16, color: [u8; 3]) -> config::Artnet {
    let mut map = config::Artnet {
        enabled: true,
        color,
        ..config::Artnet::default()
    };
    for node in &mut map.node {
        node.target = format!("127.0.0.1:{port}");
    }
    // Every frame, so a short run is not at the mercy of a 40 Hz cadence.
    map.rate_hz = 0;
    map
}

/// What one capture run observed.
struct Captured {
    /// The delimited frames, in order.
    frames: Vec<Frame>,
    /// One entry per datagram: the channel count its length field declared.
    channel_counts: Vec<usize>,
    /// Datagrams the decoder refused, which must be none.
    refused: usize,
}

/// Bind a loopback capture, build a sink aimed at it, run `drive`, and
/// reassemble everything that arrived.
fn capture(color: [u8; 3], drive: impl FnOnce(ArtnetSink)) -> Captured {
    let capture = UdpCapture::bind_ephemeral().expect("an ephemeral loopback port binds");
    capture
        .set_timeout(Some(Duration::from_millis(50)))
        .expect("the read timeout is set");
    let port = capture
        .local_addr()
        .expect("the bound address is readable")
        .port();

    let finished = Arc::new(AtomicBool::new(false));
    let reader = {
        let finished = Arc::clone(&finished);
        let mut capture = capture;
        std::thread::spawn(move || {
            let mut datagrams: Vec<Vec<u8>> = Vec::new();
            let deadline = Instant::now() + RUN_DEADLINE;
            let mut stop_at: Option<Instant> = None;
            loop {
                match capture.recv().expect("the receive succeeds") {
                    Some(datagram) => datagrams.push(datagram),
                    None => {
                        if Instant::now() >= deadline {
                            break;
                        }
                    }
                }
                if finished.load(Ordering::Acquire) {
                    let until = *stop_at.get_or_insert_with(|| Instant::now() + GRACE);
                    if Instant::now() >= until {
                        break;
                    }
                }
            }
            datagrams
        })
    };

    let sink = ArtnetSink::bind(&map_for(port, color)).expect("the fixture map binds");
    drive(sink);
    finished.store(true, Ordering::Release);
    let datagrams = reader.join().expect("the capture thread finished");

    let mut receiver = Receiver::rig();
    let mut frames = Vec::new();
    let mut channel_counts = Vec::new();
    let mut refused = 0usize;
    for datagram in &datagrams {
        match rlx_artnet_sim::decode(datagram) {
            Ok(packet) => channel_counts.push(packet.data.len()),
            Err(_) => refused += 1,
        }
        match receiver.accept(datagram) {
            Ok(Some(frame)) => frames.push(frame),
            Ok(None) => {}
            Err(_) => {}
        }
    }
    if let Some(frame) = receiver.flush() {
        frames.push(frame);
    }
    Captured {
        frames,
        channel_counts,
        refused,
    }
}

/// Every frame in `captured` that carried all 24 universes.
fn complete(captured: &Captured) -> Vec<&Frame> {
    captured.frames.iter().filter(|f| f.is_complete()).collect()
}

/// **A colour configured in `config.toml` reaches every pixel of every
/// universe.** The whole raster, which is the assertable form of "including the
/// last stick of each chain": the simulator sees the length, and the stale tail
/// it prevents is the rig's to confirm.
#[test]
fn the_configured_colour_fills_the_whole_raster() {
    const COLOR: [u8; 3] = [201, 47, 9];
    let captured = capture(COLOR, |mut sink| {
        let now = Instant::now();
        sink.send(now, COLOR);
        sink.send(now + Duration::from_millis(1), COLOR);
        // The sink's `Drop` appends a blackout, so the lit frames are the ones
        // before it — which is what `complete()[0]` picks.
        drop(sink);
    });

    assert_eq!(captured.refused, 0, "the simulator refused a datagram");
    let lit = complete(&captured);
    assert!(
        !lit.is_empty(),
        "no complete frame arrived; {} datagram(s) in {} frame(s)",
        captured.channel_counts.len(),
        captured.frames.len()
    );
    let frame = lit[0];
    assert_eq!(frame.raster.universes(), RIG_UNIVERSES);
    assert_eq!(frame.raster.pixels(), RIG_PIXELS);
    for universe in 0..RIG_UNIVERSES {
        for pixel in 0..RIG_PIXELS {
            assert_eq!(
                frame.raster.pixel(universe, pixel),
                COLOR,
                "universe {universe}, pixel {pixel}"
            );
        }
    }
}

/// The same frame, through the PNG the viewer writes — the form an author
/// actually looks at.
#[test]
fn the_configured_colour_appears_in_the_written_png() {
    const COLOR: [u8; 3] = [12, 200, 77];
    let captured = capture(COLOR, |mut sink| {
        let now = Instant::now();
        sink.send(now, COLOR);
        sink.send(now + Duration::from_millis(1), COLOR);
        drop(sink);
    });
    let lit = complete(&captured);
    assert!(!lit.is_empty(), "no complete frame arrived");

    let path = std::env::temp_dir().join(format!("rlx-artnet-loopback-{}.png", std::process::id()));
    lit[0]
        .raster
        .write_png(&path, 2)
        .expect("the simulator writes a PNG");
    let decoded = image::open(&path)
        .expect("the written file decodes")
        .to_rgb8();
    assert_eq!(decoded.width(), RIG_PIXELS as u32 * 2);
    assert_eq!(decoded.height(), RIG_UNIVERSES as u32 * 2);
    for (x, y, pixel) in decoded.enumerate_pixels() {
        assert_eq!(pixel.0, COLOR, "png pixel ({x}, {y})");
    }
    let _ = std::fs::remove_file(&path);
}

/// **Every datagram carries 510 channels**, over a whole captured run.
///
/// The short-frame trap as a test rather than a comment: a node sent fewer
/// leaves the later sticks of its chain holding their previous frame, and the
/// length is the half of that a receiver can see.
#[test]
fn every_datagram_carries_a_full_universe() {
    let captured = capture([3, 3, 3], |mut sink| {
        let now = Instant::now();
        for i in 0..3 {
            sink.send(now + Duration::from_millis(i), [3, 3, 3]);
        }
        drop(sink);
    });
    assert!(
        !captured.channel_counts.is_empty(),
        "nothing arrived, so nothing is asserted"
    );
    let short: Vec<usize> = captured
        .channel_counts
        .iter()
        .copied()
        .filter(|n| *n != RIG_PIXELS * 3)
        .collect();
    assert!(
        short.is_empty(),
        "{} of {} datagram(s) were not 510 channels: {short:?}",
        short.len(),
        captured.channel_counts.len()
    );
}

/// **Dropping the sink leaves an all-zero final frame on every universe** —
/// the normal-exit path.
///
/// Strictly stronger than watching a dark room, which cannot tell a blackout
/// from a node that merely stopped receiving: the assertion is that black was
/// *sent*, on all 24 universes, as the last thing on the wire.
#[test]
fn the_normal_exit_path_sends_black_on_every_universe() {
    let captured = capture([255, 128, 64], |mut sink| {
        let now = Instant::now();
        sink.send(now, [255, 128, 64]);
        // Dropped rather than forgotten: `Drop` is the normal-exit path.
        drop(sink);
    });
    assert_last_frame_is_black(&captured);
}

/// The same, through the explicit call the operator's quit makes.
#[test]
fn the_operator_quit_path_sends_black_on_every_universe() {
    let captured = capture([255, 128, 64], |mut sink| {
        let now = Instant::now();
        sink.send(now, [255, 128, 64]);
        sink.blackout();
        // `Drop` adds nothing after an explicit blackout, so what the stream
        // carries is the operator's quit and not the quit plus a tidy-up.
        drop(sink);
    });
    assert_last_frame_is_black(&captured);
}

/// A lit frame followed by a blackout, read off the captured stream.
fn assert_last_frame_is_black(captured: &Captured) {
    let complete = complete(captured);
    assert!(
        complete.len() >= 2,
        "expected a lit frame and a blackout; got {} complete frame(s) from {} datagram(s)",
        complete.len(),
        captured.channel_counts.len()
    );
    let first = complete[0];
    assert!(
        !first.raster.is_black(),
        "the first frame was already black, so a black last frame proves nothing"
    );
    let last = complete[complete.len() - 1];
    assert!(last.is_complete(), "the final frame missed a universe");
    assert!(
        last.raster.is_black(),
        "the last complete frame on the wire was not all-zero"
    );
}

/// **The encoder and the simulator's decoder round-trip**, which is evidence
/// only because the decoder was validated against the specification before this
/// encoder existed.
#[test]
fn the_encoder_and_the_decoder_agree_on_the_port_address() {
    let captured = capture([7, 8, 9], |mut sink| {
        sink.send(Instant::now(), [7, 8, 9]);
        drop(sink);
    });
    let lit = complete(&captured);
    assert!(!lit.is_empty(), "no complete frame arrived");
    // Every universe of the configured range contributed, and none outside it:
    // the decoder read each `SubUni`/`Net` pair back as the universe the map
    // named.
    assert_eq!(lit[0].seen.len(), RIG_UNIVERSES);
    assert_eq!(lit[0].seen_count(), RIG_UNIVERSES);
    assert_eq!(captured.refused, 0);
}
