//! Art-Net output: the fixture map from `config.toml`, an `ArtDmx` emitter, and
//! blackout on the way out (ADR-0145).
//!
//! **Nothing here reaches `core`.** The shell owns the rig the way it already
//! owns the audio input policy and the OSC telemetry — a third sink beside the
//! first two, with no socket, no DMX type and no fixture concept crossing into
//! the source-agnostic core.
//!
//! ## The map is data
//!
//! [`ArtnetSink::bind`] resolves [`config::Artnet`] into a flat [`Universe`]
//! list and then never consults the config again. Every claim about the rig —
//! which universes exist, which node carries them, how long a chain is, which
//! spatial axis the universe index stands for — is a key in that section. A rig
//! patched differently is a file edit; nothing about a different patch is
//! expressible only in Rust.
//!
//! ## A frame fills the whole universe
//!
//! Each datagram carries `pixels * 3` channels, and the configured 170 pixels
//! is 510 of a universe's 512. Sending fewer does **not** leave the rest of the
//! chain dark: the later sticks keep whatever they were last sent, which
//! presents as half the rig being broken rather than as a short frame. The
//! length is the cause and the stale tail is the symptom, and only the cause is
//! visible from this side of the wire.
//!
//! ## Blackout is a correctness requirement
//!
//! The nodes latch — they hold their last frame indefinitely — so a process
//! that stops sending leaves the room exactly as lit as it was. [`blackout`]
//! sends an all-zero frame, `Drop` calls it, and the shell calls it explicitly
//! on the operator's quit. What that covers and what it does not is stated on
//! [`ArtnetSink::blackout`]; it is not a promise about every way a process can
//! end.
//!
//! ## Failure is a drop, and only transitions are announced
//!
//! The frame loop calls [`ArtnetSink::send`] and gets nothing back, exactly as
//! it calls the OSC sink: a send error counts a dropped datagram and prints one
//! line on the edge from working to failing and back. It never propagates with
//! `?`, because a lighting sink must not take the show's frame loop with it, and
//! at 40 Hz across 24 universes a line per failure is its own outage.
//!
//! [`blackout`]: ArtnetSink::blackout

use std::io;
use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::time::{Duration, Instant};

use crate::config;

#[cfg(test)]
mod tests;

/// The eight-byte identifier every Art-Net packet opens with, null included.
const ARTNET_ID: [u8; 8] = *b"Art-Net\0";

/// `OpDmx`, the opcode of a DMX data packet. **Little-endian on the wire**,
/// unlike every other multi-byte field in the header.
const OP_DMX: u16 = 0x5000;

/// The protocol revision, big-endian on the wire.
const PROTOCOL_VERSION: u16 = 14;

/// Bytes before the channel data.
const HEADER_LEN: usize = 18;

/// A DMX universe is 512 channels, so 170 is the most whole RGB pixels one
/// carries.
pub const MAX_PIXELS: u16 = 170;

/// How many times a blackout frame is repeated.
///
/// Art-Net is fire-and-forget UDP with no acknowledgement, and the nodes latch,
/// so a single lost datagram on the way out is a lamp left on. Repetition is the
/// only mitigation available from this side of the wire; whether it is enough
/// against real hardware is a rig question.
const BLACKOUT_REPEATS: usize = 2;

/// One universe as the fixture map resolved it.
///
/// A flat list rather than a node tree: the frame loop walks universes, and the
/// node a universe belongs to is nothing but the address its datagram goes to.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Universe {
    /// Where this universe's datagram is sent.
    pub target: SocketAddr,
    /// The 15-bit Art-Net port address, written as `SubUni` and `Net`.
    pub port_address: u16,
    /// Pixels in this universe's chain; the datagram carries three channels
    /// each.
    pub pixels: u16,
    /// Position along the axis `[artnet.space] universe_axis` names, normalized
    /// to `0..=1` across `universe_min..universe_max`.
    pub coord: f32,
}

/// The bound sink: a socket, the resolved fixture map, a send cadence, and the
/// transition latch that keeps a failing link from filling the log.
#[derive(Debug)]
pub struct ArtnetSink {
    socket: UdpSocket,
    universes: Vec<Universe>,
    /// Which axis the universe coordinate is on, kept for the startup line and
    /// for whatever evaluates a look over the map.
    axis: config::SpaceAxis,
    /// Minimum gap between frames. Zero when the configured rate is 0, which
    /// means "every frame" rather than "never".
    interval: Duration,
    /// When the next frame becomes due; `None` until the first one.
    next_send: Option<Instant>,
    /// Reused encode buffer, so a steady-state frame allocates nothing.
    buf: Vec<u8>,
    /// Art-Net's per-stream frame counter, cycling `1..=255`. 0 is the
    /// specification's "sequencing disabled", so it is never emitted.
    sequence: u8,
    /// Whether the last frame had a failing send, so the notice reports the edge
    /// rather than the state.
    failing: bool,
    /// Datagrams dropped since the sink was created.
    dropped: u64,
    /// Whether the last thing on the wire was a blackout, so `Drop` after an
    /// explicit one does not repeat it.
    dark: bool,
}

impl ArtnetSink {
    /// Resolve the fixture map and bind a sending socket.
    ///
    /// `Err` carries an operator-readable reason. Validation happens **here and
    /// only here** — the frame path downstream indexes a resolved list and
    /// assumes it well-formed, which is the boundary rule this file's hot half
    /// depends on.
    pub fn bind(config: &config::Artnet) -> Result<Self, String> {
        if config.node.is_empty() {
            return Err("artnet: the fixture map has no [[artnet.node]] entries".to_owned());
        }
        let mut universes: Vec<Universe> = Vec::new();
        for node in &config.node {
            let target = node
                .target
                .to_socket_addrs()
                .map_err(|err| format!("artnet node `{}`: {err}", node.target))?
                .next()
                .ok_or_else(|| format!("artnet node `{}`: resolved to no address", node.target))?;
            let [first, last] = node.universes;
            if first > last {
                return Err(format!(
                    "artnet node `{}`: universes [{first}, {last}] runs backwards",
                    node.target
                ));
            }
            if node.pixels == 0 || node.pixels > MAX_PIXELS {
                return Err(format!(
                    "artnet node `{}`: {} pixels is not in 1..={MAX_PIXELS} (a universe is 512 \
                     channels)",
                    node.target, node.pixels
                ));
            }
            for port_address in first..=last {
                if universes.iter().any(|u| u.port_address == port_address) {
                    return Err(format!(
                        "artnet: universe {port_address} is carried by more than one node"
                    ));
                }
                universes.push(Universe {
                    target,
                    port_address,
                    pixels: node.pixels,
                    coord: normalized(
                        port_address,
                        config.space.universe_min,
                        config.space.universe_max,
                    ),
                });
            }
        }

        // The bind family follows the first target's: a v4 and a v6 socket
        // cannot address each other, and a map mixing the two would silently
        // drop half the rig.
        let local: SocketAddr = if universes[0].target.is_ipv6() {
            ([0u16; 8], 0).into()
        } else {
            ([0u8; 4], 0).into()
        };
        let socket = UdpSocket::bind(local)
            .map_err(|err| format!("artnet: could not bind a socket: {err}"))?;
        socket
            .set_nonblocking(true)
            .map_err(|err| format!("artnet: could not set the socket non-blocking: {err}"))?;

        let widest = universes
            .iter()
            .map(|u| usize::from(u.pixels) * 3)
            .max()
            .unwrap_or(0);
        Ok(Self {
            socket,
            universes,
            axis: config.space.universe_axis,
            interval: if config.rate_hz == 0 {
                Duration::ZERO
            } else {
                Duration::from_secs_f64(1.0 / f64::from(config.rate_hz))
            },
            next_send: None,
            buf: Vec::with_capacity(HEADER_LEN + widest),
            sequence: 0,
            failing: false,
            dropped: 0,
            dark: false,
        })
    }

    /// The resolved map, in the order the frame loop walks it.
    pub fn universes(&self) -> &[Universe] {
        &self.universes
    }

    /// Which axis [`Universe::coord`] is on.
    pub fn axis(&self) -> config::SpaceAxis {
        self.axis
    }

    /// Datagrams dropped so far.
    pub fn dropped(&self) -> u64 {
        self.dropped
    }

    /// Paint every pixel of every universe one colour and emit a frame, if the
    /// cadence says one is due.
    ///
    /// `now` is the frame loop's own already-measured instant rather than a
    /// fresh clock read, the same way the telemetry sink rides it.
    pub fn send(&mut self, now: Instant, color: [u8; 3]) {
        if self.next_send.is_some_and(|due| now < due) {
            return;
        }
        // Scheduled from `now` rather than from the previous deadline: a missed
        // window must not leave a backlog that sends a burst of frames at a rig.
        self.next_send = Some(now + self.interval);
        self.emit(color);
        self.dark = color == [0, 0, 0];
    }

    /// Send an all-zero frame, whatever the cadence says.
    ///
    /// **What this covers:** the normal end of the process and the operator's
    /// quit, through `Drop` and through the shell's explicit call; and a panic
    /// that unwinds far enough to drop the sink.
    ///
    /// **What it does not:** an abort, a `SIGKILL` or its Windows equivalent, a
    /// power loss, and any panic that does not unwind to this value's owner.
    /// Those leave the nodes latched on their last frame, which is the hardware
    /// fact that makes this method exist rather than a shortcoming of it.
    pub fn blackout(&mut self) {
        if self.dark {
            return;
        }
        // Blocking for the length of the blackout: the non-blocking socket
        // exists so a stalled `sendto` cannot cost a rendered frame, and there
        // is no frame left to cost. A `WouldBlock` here would be a lit room.
        let restore = self.socket.set_nonblocking(false).is_ok();
        for _ in 0..BLACKOUT_REPEATS {
            self.emit([0, 0, 0]);
        }
        if restore {
            let _ = self.socket.set_nonblocking(true);
        }
        self.dark = true;
    }

    /// Build and send one frame of a single colour across the whole map.
    fn emit(&mut self, color: [u8; 3]) {
        // Cycles 1..=255: the specification reserves 0 for "sequencing
        // disabled", and a receiver watching this counter is how frame loss is
        // seen from the other end.
        self.sequence = self.sequence.checked_add(1).unwrap_or(1);
        let mut failed = None;
        for index in 0..self.universes.len() {
            let universe = self.universes[index];
            encode(
                &mut self.buf,
                self.sequence,
                universe.port_address,
                usize::from(universe.pixels),
                color,
            );
            if let Err(err) = self.socket.send_to(&self.buf, universe.target) {
                self.dropped += 1;
                failed = Some(err);
            }
        }
        self.note(failed);
    }

    /// Move the transition latch and announce only an edge.
    fn note(&mut self, failed: Option<io::Error>) {
        match (self.failing, failed) {
            (false, Some(err)) => {
                self.failing = true;
                eprintln!("artnet: send failed ({err}); dropping frames until it recovers");
            }
            (true, None) => {
                self.failing = false;
                eprintln!(
                    "artnet: sending recovered after {} dropped datagram(s)",
                    self.dropped
                );
            }
            // Steady state, working or broken: say nothing.
            _ => {}
        }
    }
}

impl Drop for ArtnetSink {
    fn drop(&mut self) {
        self.blackout();
    }
}

/// Write one `ArtDmx` datagram of `pixels` pixels, all `color`, into `buf`.
///
/// The buffer is cleared and refilled rather than reallocated, so a steady-state
/// frame allocates nothing after the first.
pub fn encode(buf: &mut Vec<u8>, sequence: u8, port_address: u16, pixels: usize, color: [u8; 3]) {
    let channels = pixels * 3;
    buf.clear();
    buf.extend_from_slice(&ARTNET_ID);
    // The one little-endian field in the header; everything after it is big.
    buf.extend_from_slice(&OP_DMX.to_le_bytes());
    buf.extend_from_slice(&PROTOCOL_VERSION.to_be_bytes());
    buf.push(sequence);
    buf.push(0); // physical: informational, and this sender has no input port
    // `SubUni` then `Net`: the 15-bit port address, low byte first.
    buf.push((port_address & 0xff) as u8);
    buf.push((port_address >> 8) as u8);
    buf.extend_from_slice(&(channels as u16).to_be_bytes());
    for _ in 0..pixels {
        buf.extend_from_slice(&color);
    }
}

/// `value` mapped to `0..=1` across `min..max`, with a reversed range flipping
/// the axis and a degenerate one reading 0.
///
/// Clamped rather than extrapolated: a universe outside the declared span is a
/// map the operator has not finished writing, and a coordinate past the ends of
/// the structure would place a look somewhere the structure is not.
fn normalized(value: u16, min: u16, max: u16) -> f32 {
    if min == max {
        return 0.0;
    }
    let (value, min, max) = (f32::from(value), f32::from(min), f32::from(max));
    ((value - min) / (max - min)).clamp(0.0, 1.0)
}
