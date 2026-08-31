//! OSC telemetry sink: the analyzer's fixed signal set, published over UDP to a
//! lighting console (ADR-0144).
//!
//! **Nothing here reaches `core`.** The shell already holds the `AnalysisFrame`
//! it hands to the renderer and the director, so this reads a value that is in
//! hand and needs no new accessor — which is what keeps the source-agnostic core
//! free of a socket type.
//!
//! ## The encoder
//!
//! OSC 1.0 over UDP, hand-rolled. The wire format is an address string, a type-
//! tag string and the arguments, each element padded to a 4-byte boundary; the
//! `f`, `i` and `s` types below are the whole of what this sink emits. NFR
//! section 4's dependency gate asks for a justification for a crate pulling a
//! transitive graph, and the encoder is smaller than the justification would be.
//!
//! The padding rule is the trap: an OSC-string is **always** NUL-terminated and
//! then padded up, so a 4-byte address takes 8 bytes on the wire, not 4. A
//! string whose length is already a multiple of 4 gains a full 4 bytes of
//! padding rather than none. `pad_to_4` is that rule, and every element goes
//! through it.
//!
//! ## The address space is versioned in the addresses
//!
//! Every address begins [`ADDRESS_PREFIX`]. An operator's console mapping is
//! bound against those strings, so a later signal is **additive** under the same
//! prefix and an incompatible change takes a new one — the mapping keeps working
//! either way. [`Telemetry::messages`] is the whole table; it is a fixed-size
//! array so the roster cannot drift from what the sink sends.
//!
//! ## Failure is a drop, and only transitions are announced
//!
//! The frame loop calls [`OscSink::send`] and gets nothing back. A send error
//! increments a counter and, on the **edge** from working to failing (and back),
//! prints one line — the same report-the-transition shape the cap-overflow and
//! tier-demotion notices use. It never propagates with `?`, because a lighting
//! sink must not take the show's frame loop with it, and it never logs per
//! frame, because at 60 Hz a per-frame line is its own outage.
//!
//! The socket is **non-blocking** for the same reason: a blocking `sendto` to a
//! LAN address can stall on ARP resolution, and a stalled `sendto` on the render
//! thread is a dropped frame. `WouldBlock` counts as a drop like any other.

use std::io;
use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::time::{Duration, Instant};

#[cfg(test)]
mod tests;

/// Prefix every published address carries, version included.
pub const ADDRESS_PREFIX: &str = "/lmv/v1";

/// How many addresses the fixed set publishes — the length of
/// [`Telemetry::messages`], exposed so a caller can size a buffer or assert the
/// roster without re-counting it.
pub const ADDRESS_COUNT: usize = 14;

/// One OSC argument. The three types this sink emits; `s` borrows so the preset
/// name does not have to be cloned every frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Arg<'a> {
    /// OSC `f` — 32-bit IEEE-754 float, big-endian.
    F(f32),
    /// OSC `i` — 32-bit two's-complement integer, big-endian.
    I(i32),
    /// OSC `s` — NUL-terminated, padded to a 4-byte boundary.
    S(&'a str),
}

impl Arg<'_> {
    /// The type-tag character this argument contributes to the tag string.
    fn tag(self) -> u8 {
        match self {
            Arg::F(_) => b'f',
            Arg::I(_) => b'i',
            Arg::S(_) => b's',
        }
    }
}

/// Bytes of NUL padding that take `len` up to the next multiple of 4, **at least
/// one** — an OSC-string is NUL-terminated before it is padded, so a length that
/// is already a multiple of 4 still takes a full 4 bytes.
fn pad_to_4(len: usize) -> usize {
    4 - (len % 4)
}

/// Append `text` as an OSC-string: the bytes, a NUL, then padding to the next
/// 4-byte boundary.
///
/// An interior NUL would make the receiver read a shorter string than was
/// written and then mis-parse everything after it, so it is replaced rather than
/// passed through. The only string this sink sends is a preset name, which comes
/// from a file stem and never contains one; the substitution is a guard against
/// a future caller, not a live case.
fn push_string(buf: &mut Vec<u8>, text: &str) {
    let start = buf.len();
    buf.extend(text.bytes().map(|b| if b == 0 { b'?' } else { b }));
    let written = buf.len() - start;
    buf.extend(std::iter::repeat_n(0u8, pad_to_4(written)));
}

/// Encode one OSC message into `buf`, **replacing** whatever it held.
///
/// The buffer is passed in rather than returned so a per-frame send reuses one
/// allocation. The result is always a multiple of 4 bytes long, which is the
/// property the whole format rests on.
pub fn encode(buf: &mut Vec<u8>, address: &str, args: &[Arg<'_>]) {
    buf.clear();
    push_string(buf, address);

    // The type-tag string is itself an OSC-string, leading comma included, so it
    // takes the same NUL-terminate-then-pad treatment as the address.
    let start = buf.len();
    buf.push(b',');
    buf.extend(args.iter().map(|a| a.tag()));
    let written = buf.len() - start;
    buf.extend(std::iter::repeat_n(0u8, pad_to_4(written)));

    for arg in args {
        match *arg {
            Arg::F(v) => buf.extend_from_slice(&v.to_bits().to_be_bytes()),
            Arg::I(v) => buf.extend_from_slice(&v.to_be_bytes()),
            Arg::S(v) => push_string(buf, v),
        }
    }
}

/// One frame's worth of the fixed telemetry set, already read off the
/// `AnalysisFrame` and the renderer by the shell.
///
/// A plain snapshot rather than a borrow of the frame: this module is the sink,
/// and giving it the analysis type would put the core's DSP vocabulary in the
/// transport layer for no gain.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Telemetry<'a> {
    /// Bass level, peak-normalized (ADR-0049).
    pub bass: f32,
    /// Mid level, peak-normalized.
    pub mid: f32,
    /// Treble level, peak-normalized.
    pub treb: f32,
    /// Spectral-flux onset envelope, peak-normalized.
    pub onset: f32,
    /// Broadband RMS of the waveform trace.
    ///
    /// **Un-normalized, unlike the four levels above it.** The waveform it is
    /// computed from is deliberately raw amplitude (ADR-0049 normalizes the
    /// headline levels and leaves the trace alone, because a scope that reads
    /// the same when quiet as when loud is not a scope), and an RMS of a raw
    /// signal is a raw level. An operator mapping this needs a gain in the
    /// console where the four above need none. [`rms_of`] computes it.
    pub rms: f32,
    /// Raw mean magnitude in the bass band — the absolute twin of [`Self::bass`].
    pub bass_raw: f32,
    /// Raw mean magnitude in the mid band.
    pub mid_raw: f32,
    /// Raw mean magnitude in the treble band.
    pub treb_raw: f32,
    /// Raw spectral-flux envelope.
    pub onset_raw: f32,
    /// Whether an onset fired on this frame — the discrete event, as against
    /// [`Self::onset`]'s continuous envelope.
    pub beat: bool,
    /// Monotone count of onset detections since the stream started.
    ///
    /// **Not a musical beat count** (ADR-0109): the detector fires 1.2x-2.3x per
    /// musical beat depending on material, so no fixed multiplier converts this
    /// to bars. It is useful as a "something happened" ratchet, not as a meter.
    pub beat_index: u32,
    /// Beat phase in `[0, 1)`: 0 on each beat, ramping to the next.
    ///
    /// This is `AnalysisFrame::bar`, published under its true name. That field
    /// carries a documented misnomer too widely bound inside the engine to
    /// rename (ADR-0050); nothing outside this repo inherits the naming debt.
    pub beat_phase: f32,
    /// Tempo estimate in BPM, 0 until the tracker warms.
    pub tempo: f32,
    /// The active preset's name.
    pub preset: &'a str,
}

impl<'a> Telemetry<'a> {
    /// The whole published set, address by address — **this array is the address
    /// table.** Anything documented for an operator is checked against it rather
    /// than against a prose list, which is what stops the two drifting.
    ///
    /// One argument per address on purpose: a console binds a parameter to an
    /// address, and a multi-argument message would make it bind to a position
    /// inside one.
    pub fn messages(&self) -> [(&'static str, Arg<'a>); ADDRESS_COUNT] {
        [
            ("/lmv/v1/level/bass", Arg::F(self.bass)),
            ("/lmv/v1/level/mid", Arg::F(self.mid)),
            ("/lmv/v1/level/treb", Arg::F(self.treb)),
            ("/lmv/v1/level/onset", Arg::F(self.onset)),
            ("/lmv/v1/level/rms", Arg::F(self.rms)),
            ("/lmv/v1/raw/bass", Arg::F(self.bass_raw)),
            ("/lmv/v1/raw/mid", Arg::F(self.mid_raw)),
            ("/lmv/v1/raw/treb", Arg::F(self.treb_raw)),
            ("/lmv/v1/raw/onset", Arg::F(self.onset_raw)),
            ("/lmv/v1/beat/trigger", Arg::I(i32::from(self.beat))),
            // Saturating rather than wrapping: at a few onsets a second the cap
            // is some seventeen years of continuous playback, and a console
            // watching a ratchet is better served by a stuck maximum than by a
            // sudden negative.
            (
                "/lmv/v1/beat/index",
                Arg::I(i32::try_from(self.beat_index).unwrap_or(i32::MAX)),
            ),
            ("/lmv/v1/beat/phase", Arg::F(self.beat_phase)),
            ("/lmv/v1/tempo", Arg::F(self.tempo)),
            ("/lmv/v1/preset", Arg::S(self.preset)),
        ]
    }
}

/// Root-mean-square of a waveform trace, or 0 for an empty one.
///
/// Accumulated in `f64` because the trace is hundreds of samples and an `f32`
/// sum of squares loses low-order bits well before the end of it.
pub fn rms_of(waveform: &[f32]) -> f32 {
    if waveform.is_empty() {
        return 0.0;
    }
    let sum: f64 = waveform.iter().map(|s| f64::from(*s) * f64::from(*s)).sum();
    (sum / waveform.len() as f64).sqrt() as f32
}

/// The UDP sink: a bound socket, a target, a send cadence, and the transition
/// latch that keeps a failing link from filling the log.
pub struct OscSink {
    socket: UdpSocket,
    target: SocketAddr,
    /// Minimum gap between sends. Zero when the configured rate is 0, which
    /// means "every frame" rather than "never".
    interval: Duration,
    /// When the next send becomes due; `None` until the first send.
    next_send: Option<Instant>,
    /// Reused encode buffer, so a steady-state frame allocates nothing.
    buf: Vec<u8>,
    /// Whether the last send attempt failed, so the notice reports the edge
    /// rather than the state.
    failing: bool,
    /// Datagrams dropped since the sink was created, for the closing summary.
    dropped: u64,
}

impl OscSink {
    /// Bind a sending socket and resolve `target` (`host:port`).
    ///
    /// `Err` carries an operator-readable reason. The caller decides whether
    /// that is fatal — a target typed on the command line for this run is a
    /// usage error, where a stale one in `config.toml` is not worth refusing to
    /// start over.
    pub fn bind(target: &str, rate_hz: u32) -> Result<Self, String> {
        let target = target
            .to_socket_addrs()
            .map_err(|err| format!("--osc `{target}`: {err}"))?
            .next()
            .ok_or_else(|| format!("--osc `{target}`: resolved to no address"))?;
        // An unspecified local address and an ephemeral port: nothing listens
        // here, and binding to the interface that routes to `target` is the
        // stack's job, not ours. v4 and v6 sockets cannot address each other, so
        // the bind family follows the resolved target's.
        let local: SocketAddr = if target.is_ipv6() {
            ([0u16; 8], 0).into()
        } else {
            ([0u8; 4], 0).into()
        };
        let socket =
            UdpSocket::bind(local).map_err(|err| format!("osc: could not bind a socket: {err}"))?;
        socket
            .set_nonblocking(true)
            .map_err(|err| format!("osc: could not set the socket non-blocking: {err}"))?;
        Ok(Self {
            socket,
            target,
            interval: if rate_hz == 0 {
                Duration::ZERO
            } else {
                Duration::from_secs_f64(1.0 / f64::from(rate_hz))
            },
            next_send: None,
            buf: Vec::with_capacity(64),
            failing: false,
            dropped: 0,
        })
    }

    /// The resolved target, for the startup line.
    pub fn target(&self) -> SocketAddr {
        self.target
    }

    /// Datagrams dropped so far.
    pub fn dropped(&self) -> u64 {
        self.dropped
    }

    /// Publish the set, if the cadence says one is due.
    ///
    /// `now` is the frame loop's own already-measured instant rather than a
    /// fresh clock read: the shell reads the wall clock once a frame for the
    /// director's `dt`, and this rides on it.
    pub fn send(&mut self, now: Instant, telemetry: &Telemetry<'_>) {
        if self.next_send.is_some_and(|due| now < due) {
            return;
        }
        // Schedule from `now` rather than from the previous deadline: a missed
        // window (a hidden gap, a long frame) must not leave a backlog that
        // sends a burst on the next visible frame.
        self.next_send = Some(now + self.interval);

        let mut failed = None;
        for (address, arg) in telemetry.messages() {
            encode(&mut self.buf, address, &[arg]);
            if let Err(err) = self.socket.send_to(&self.buf, self.target) {
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
                eprintln!(
                    "osc: send to {} failed ({err}); dropping telemetry until it recovers",
                    self.target
                );
            }
            (true, None) => {
                self.failing = false;
                eprintln!(
                    "osc: send to {} recovered after {} dropped datagram(s)",
                    self.target, self.dropped
                );
            }
            // Steady state, working or broken: say nothing.
            _ => {}
        }
    }
}
