//! The control-in listener: one UDP socket, a decoder, and a bounded queue the
//! render thread drains between frames (ADR-0176).
//!
//! **Off unless asked for.** Nothing here is constructed without `--control` or
//! `[control] enabled`, so a windowed run that was not asked to listen opens no
//! socket at all — not a socket that ignores everything, none.
//!
//! ## Three threads and what each may do
//!
//! The **audio callback** is not involved and must never become involved: it is
//! the one thread in this process that may not block, and nothing here is
//! reachable from it. The **listener** thread blocks on `recv_from`, decodes
//! into an allocation-free [`Action`], and takes one uncontended lock to record
//! it. The **render** thread takes that lock once per frame and swaps the buffer
//! out. A mutex is right here for the reason it would be wrong in the callback:
//! both sides are already allowed to wait, and the critical section is a memcpy
//! of a few dozen bytes.
//!
//! ## The queue keeps the last value per name
//!
//! A slider drag is *nudges at slider rate* — a dropped nudge costs nothing
//! because the next one replaces it, and ordering matters only within one
//! parameter. So [`Pending`] holds **one slot per parameter name**: ten thousand
//! datagrams for `warp` occupy one slot and the frame sees the last of them.
//! Transport verbs and pings are events rather than levels and are kept in
//! order, up to a cap.
//!
//! Every buffer is allocated **once, at construction**, and only ever pushed
//! into below its capacity, so no packet reaches the allocator. A message with
//! no room left is dropped and counted; it is not a reason to grow.
//!
//! ## The application order is fixed
//!
//! A frame's drained actions are applied transport, then preset, then
//! `params/clear`, then per-name clears, then per-name values, then pings. That
//! order is what makes a studio's "switch to this preset and hold these three
//! parameters on it" land as written: a switch drops every override, so the
//! overrides have to follow it, and a wholesale clear has to precede the values
//! that survive it. [`Drained`] is the drained buffer and the shell walks it in
//! this order.

use std::io;
use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use rlx_core::render::Renderer;

use crate::marks::Mark;
use crate::osc::decode::{self, Action, Name, Transport};

/// Distinct parameter names one frame may carry values for.
///
/// A drag moves one parameter and a panel of sliders moves a handful; sixty-four
/// is far past either. A distinct sixty-fifth name in one frame is dropped and
/// counted, which is the bounded behaviour a flood has to have.
const PARAM_SLOTS: usize = 64;

/// Distinct parameter names one frame may carry clears for.
const CLEAR_SLOTS: usize = 64;

/// Transport verbs one frame may carry. Events, not levels: `next` twice is two
/// switches, so these are not deduplicated.
const TRANSPORT_SLOTS: usize = 8;

/// Pings one frame may answer.
const PING_SLOTS: usize = 8;

/// Distinct `(preset, mark)` pairs one frame may carry marks for.
///
/// A mark is a deliberate click, so one frame carrying more than a handful is
/// already a sender doing something odd. Deduplicated per pair like a parameter
/// value, because a mark is a **state**: the last one sent for a pair is what it
/// should be.
const MARK_SLOTS: usize = 16;

/// Receive buffer. One Ethernet MTU, which is far more than any message in the
/// vocabulary needs and enough that a legitimate datagram is never truncated
/// into a malformed one.
const RECV_BUF: usize = 1500;

/// How long the listener blocks before checking whether it has been asked to
/// stop. The socket is otherwise idle, so this is the whole cost of being able
/// to shut the thread down rather than leaking it.
const POLL: Duration = Duration::from_millis(200);

/// How many receive failures in a row end the listener.
///
/// A transient refusal — a stack handing back a queued ICMP unreachable — is one
/// error with a datagram or a [`POLL`] timeout on either side of it, so the run
/// resets to zero and the loop carries on. A socket that has gone is an error
/// every iteration with nothing between them, which reaches this budget in
/// microseconds; the thread then ends and [`Control::listening`] says so, which
/// is the fact a reader wants and is also what stops the loop spinning a core
/// for the life of a show.
const RECV_ERROR_BUDGET: u32 = 64;

/// What the listener has accumulated since the render thread last drained.
///
/// Fixed capacity throughout — see the module docs.
pub struct Pending {
    params: Vec<(Name, f32)>,
    clears: Vec<Name>,
    clear_all: bool,
    preset: Option<Name>,
    transport: Vec<Transport>,
    marks: Vec<(Name, Mark, bool)>,
    pings: Vec<i32>,
}

impl Default for Pending {
    fn default() -> Self {
        Self {
            params: Vec::with_capacity(PARAM_SLOTS),
            clears: Vec::with_capacity(CLEAR_SLOTS),
            clear_all: false,
            preset: None,
            transport: Vec::with_capacity(TRANSPORT_SLOTS),
            marks: Vec::with_capacity(MARK_SLOTS),
            pings: Vec::with_capacity(PING_SLOTS),
        }
    }
}

impl Pending {
    /// Record `action`, returning `false` when there was no room for it.
    ///
    /// The one place a bound is enforced. A parameter that is already held takes
    /// its own slot rather than a new one, which is why a flood of one name
    /// never fills anything.
    fn record(&mut self, action: Action) -> bool {
        match action {
            Action::Param { name, value } => {
                if let Some(slot) = self.params.iter_mut().find(|(held, _)| *held == name) {
                    slot.1 = value;
                    return true;
                }
                if self.params.len() == self.params.capacity() {
                    return false;
                }
                self.params.push((name, value));
            }
            Action::ClearParam { name } => {
                if self.clears.contains(&name) {
                    return true;
                }
                if self.clears.len() == self.clears.capacity() {
                    return false;
                }
                self.clears.push(name);
            }
            // Two of these in one frame mean what one means.
            Action::ClearParams => self.clear_all = true,
            // Likewise: the last preset asked for is the one to land on.
            Action::Preset { name } => self.preset = Some(name),
            Action::Transport(verb) => {
                if self.transport.len() == self.transport.capacity() {
                    return false;
                }
                self.transport.push(verb);
            }
            // A state per `(preset, mark)` pair, like a parameter value: the
            // last one sent is what the mark should be.
            Action::Mark {
                name,
                mark,
                on: state,
            } => {
                if let Some(slot) = self
                    .marks
                    .iter_mut()
                    .find(|(held, kind, _)| *held == name && *kind == mark)
                {
                    slot.2 = state;
                    return true;
                }
                if self.marks.len() == self.marks.capacity() {
                    return false;
                }
                self.marks.push((name, mark, state));
            }
            Action::Ping(nonce) => {
                if self.pings.len() == self.pings.capacity() {
                    return false;
                }
                self.pings.push(nonce);
            }
        }
        true
    }

    /// Empty it, keeping every allocation.
    ///
    /// `Vec::clear` rather than `mem::take`: taking would hand the listener a
    /// zero-capacity buffer and put an allocation on the next packet, which is
    /// the one thing this type exists to avoid.
    fn clear(&mut self) {
        self.params.clear();
        self.clears.clear();
        self.clear_all = false;
        self.preset = None;
        self.transport.clear();
        self.marks.clear();
        self.pings.clear();
    }

    /// Whether anything at all was received.
    pub fn is_empty(&self) -> bool {
        self.params.is_empty()
            && self.clears.is_empty()
            && !self.clear_all
            && self.preset.is_none()
            && self.transport.is_empty()
            && self.marks.is_empty()
            && self.pings.is_empty()
    }

    /// The transport verbs, in arrival order.
    pub fn transport(&self) -> &[Transport] {
        &self.transport
    }

    /// The preset to dissolve to, if one was asked for.
    pub fn preset(&self) -> Option<&Name> {
        self.preset.as_ref()
    }

    /// Whether every override was asked to be dropped.
    pub fn clear_all(&self) -> bool {
        self.clear_all
    }

    /// The parameter names whose overrides were asked to be dropped.
    pub fn clears(&self) -> &[Name] {
        &self.clears
    }

    /// The held values, one per name.
    pub fn params(&self) -> &[(Name, f32)] {
        &self.params
    }

    /// The marks to set, one per `(preset, mark)` pair.
    pub fn marks(&self) -> &[(Name, Mark, bool)] {
        &self.marks
    }

    /// The ping nonces to answer.
    pub fn pings(&self) -> &[i32] {
        &self.pings
    }
}

/// One frame's worth of drained control input.
pub type Drained = Pending;

/// What applying one frame's parameter and preset traffic did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Applied {
    /// Whether a `ctl/preset` actually landed on a preset the roster holds. The
    /// shell has bookkeeping that follows a switch, and an unknown name must not
    /// trigger it.
    pub switched: bool,
    /// The name a `ctl/preset` asked for that the renderer declined to select.
    ///
    /// `None` when no preset was asked for and when the one asked for landed,
    /// so the two silent cases stay silent and only the refusal is reportable.
    /// Carried out rather than reported here because this function has the
    /// renderer and not the event stream, and the refusal is a fact about the
    /// request rather than about the picture.
    pub unresolved_preset: Option<Name>,
    /// Parameter overrides the engine refused because nothing on the active
    /// preset's system claims the name.
    pub refused: u64,
}

/// Apply the preset selection and the parameter overrides a drained frame
/// carries.
///
/// **Split out because this half needs only the renderer.** The transport verbs
/// reach the director and the console's own settings, which live above this
/// module; these four calls do not. That split is what lets the whole
/// socket-to-picture path be driven in a test without a window.
///
/// The order inside is the module's: a switch drops every override, so it has to
/// precede the values, and a wholesale clear has to precede the values that
/// survive it.
pub fn apply_to_renderer(drained: &Drained, renderer: &mut Renderer) -> Applied {
    let mut applied = Applied::default();
    if let Some(name) = drained.preset() {
        applied.switched = renderer.select_preset_by_name(name.as_str());
        if !applied.switched {
            applied.unresolved_preset = Some(*name);
        }
    }
    if drained.clear_all() {
        renderer.clear_param_overrides();
    }
    for name in drained.clears() {
        renderer.clear_param_override(name.as_str());
    }
    for (name, value) in drained.params() {
        if renderer.set_param_override(name.as_str(), *value).is_err() {
            applied.refused += 1;
        }
    }
    applied
}

/// What the two threads share: the queue, and the running totals the `health`
/// event reports.
struct Shared {
    pending: Mutex<Pending>,
    /// Datagrams `recv_from` handed over, whatever became of them afterwards.
    ///
    /// Counted before the decoder, so it is the one reading that separates "no
    /// datagram arrived" from "one arrived and was discarded": every other
    /// total below is a subset of this one.
    received: AtomicU64,
    /// Receive failures that were not the ordinary read timeout.
    ///
    /// The timeout is how [`POLL`] gets the stop flag read and says nothing
    /// about the socket, so counting it would bury the failures underneath a
    /// number that climbs five times a second on a healthy idle listener.
    recv_errors: AtomicU64,
    /// Whether the listener thread is still in its receive loop.
    ///
    /// Set true before the loop and cleared when it returns by any route —
    /// stop flag, error budget or unwind — so a thread that has gone is
    /// distinguishable from a socket that is merely quiet.
    listening: AtomicBool,
    /// Datagrams that did not decode into an action the player knows.
    rejected: AtomicU64,
    /// Actions that decoded but found the queue full for that frame.
    dropped: AtomicU64,
    /// Parameter overrides the core refused because nothing on the active
    /// preset's system claims the name.
    ///
    /// Counted on the render thread rather than the listener's, because that is
    /// where the answer exists: the decoder knows a name is well-formed, and
    /// only the engine knows whether anything answers to it.
    refused: AtomicU64,
}

impl Shared {
    /// An empty queue and zeroed totals, not yet listening.
    fn new() -> Self {
        Self {
            pending: Mutex::new(Pending::default()),
            received: AtomicU64::new(0),
            recv_errors: AtomicU64::new(0),
            listening: AtomicBool::new(false),
            rejected: AtomicU64::new(0),
            dropped: AtomicU64::new(0),
            refused: AtomicU64::new(0),
        }
    }
}

/// Holds [`Shared::listening`] true for as long as the receive loop runs.
///
/// A guard rather than a pair of stores: the loop leaves by the stop flag, by
/// the error budget and — were a decoder ever to panic — by an unwind, and a
/// flag that stayed true on the third would report a thread that is not there.
struct Listening<'a>(&'a Shared);

impl<'a> Listening<'a> {
    fn start(shared: &'a Shared) -> Self {
        shared.listening.store(true, Ordering::Relaxed);
        Self(shared)
    }
}

impl Drop for Listening<'_> {
    fn drop(&mut self) {
        self.0.listening.store(false, Ordering::Relaxed);
    }
}

/// What the receive loop reads from.
///
/// One method, and [`UdpSocket`] is the only implementation that ships. It
/// exists because the loop's failure policy cannot otherwise be exercised: the
/// socket is moved into the listener thread and nothing outside that thread
/// holds a handle to it, so no test can break the socket under a running
/// listener. A fake receiver can.
trait Receive {
    /// Read one datagram into `buf`, returning its length.
    fn receive(&self, buf: &mut [u8]) -> io::Result<usize>;
}

impl Receive for UdpSocket {
    fn receive(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.recv_from(buf).map(|(len, _from)| len)
    }
}

/// Whether `err` is the read timeout [`POLL`] arms rather than a failure.
///
/// Both kinds, because the platforms disagree: a socket with `SO_RCVTIMEO` set
/// reports `WouldBlock` on Unix and `TimedOut` on Windows, and `std` documents
/// either as possible.
fn is_read_timeout(err: &io::Error) -> bool {
    matches!(
        err.kind(),
        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
    )
}

/// The listener: a bound socket, the thread reading it, and the render thread's
/// own drain buffer.
pub struct Control {
    local: SocketAddr,
    shared: Arc<Shared>,
    /// The render thread's buffer, swapped with the shared one on every drain so
    /// neither side ever allocates.
    scratch: Pending,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Control {
    /// Bind `addr` (`host:port`) and start listening.
    ///
    /// `Err` carries an operator-readable reason. The caller decides whether that
    /// is fatal — an address typed on the command line for this run is a usage
    /// error, where a stale one in `config.toml` is not worth refusing to start
    /// over, which is the rule `--osc` already follows.
    pub fn bind(addr: &str) -> Result<Self, String> {
        let resolved = addr
            .to_socket_addrs()
            .map_err(|err| format!("--control `{addr}`: {err}"))?
            .next()
            .ok_or_else(|| format!("--control `{addr}`: resolved to no address"))?;
        let socket = UdpSocket::bind(resolved)
            .map_err(|err| format!("--control `{addr}`: could not bind: {err}"))?;
        // The bound address rather than the requested one: a port of 0 asks the
        // stack to choose, and `hello` reports what was actually taken.
        let local = socket.local_addr().unwrap_or(resolved);
        socket
            .set_read_timeout(Some(POLL))
            .map_err(|err| format!("--control `{addr}`: could not set a read timeout: {err}"))?;

        let shared = Arc::new(Shared::new());
        // True before the thread exists rather than at the top of `listen`: a
        // caller that reads `listening` between the spawn and the thread's
        // first instruction would otherwise be told the listener is gone.
        shared.listening.store(true, Ordering::Relaxed);
        let stop = Arc::new(AtomicBool::new(false));
        let thread = std::thread::Builder::new()
            .name("rlx-control".to_owned())
            .spawn({
                let shared = Arc::clone(&shared);
                let stop = Arc::clone(&stop);
                move || listen(&socket, &shared, &stop)
            })
            .map_err(|err| format!("--control `{addr}`: could not start the listener: {err}"))?;

        Ok(Self {
            local,
            shared,
            scratch: Pending::default(),
            stop,
            thread: Some(thread),
        })
    }

    /// The address actually bound — what the `hello` event reports.
    pub fn local_addr(&self) -> SocketAddr {
        self.local
    }

    /// Datagrams the socket handed the listener, before decoding.
    pub fn received(&self) -> u64 {
        self.shared.received.load(Ordering::Relaxed)
    }

    /// Receive failures that were not the ordinary read timeout.
    pub fn recv_errors(&self) -> u64 {
        self.shared.recv_errors.load(Ordering::Relaxed)
    }

    /// Whether the listener thread is still reading the socket.
    pub fn listening(&self) -> bool {
        self.shared.listening.load(Ordering::Relaxed)
    }

    /// Datagrams the decoder refused.
    pub fn rejected(&self) -> u64 {
        self.shared.rejected.load(Ordering::Relaxed)
    }

    /// Actions dropped because a frame's queue was full.
    pub fn dropped(&self) -> u64 {
        self.shared.dropped.load(Ordering::Relaxed)
    }

    /// Parameter overrides the engine refused.
    pub fn refused(&self) -> u64 {
        self.shared.refused.load(Ordering::Relaxed)
    }

    /// Add `count` engine refusals to the running total.
    pub fn note_refused(&self, count: u64) {
        if count > 0 {
            self.shared.refused.fetch_add(count, Ordering::Relaxed);
        }
    }

    /// Whether anything is waiting to be drained.
    ///
    /// Peeks at the shared queue without taking it. The frame loop never calls
    /// this — it drains unconditionally, and an empty drain is a swap of two
    /// empty buffers. It exists for a caller that has to **wait** for a datagram
    /// whose listener thread it does not schedule, which is what a loopback test
    /// is; polling `drain` for that would consume the very delivery it was
    /// waiting for.
    pub fn has_pending(&self) -> bool {
        self.shared
            .pending
            .lock()
            .map(|pending| !pending.is_empty())
            .unwrap_or(false)
    }

    /// Take everything received since the last call.
    ///
    /// Bounded by construction, so the render thread's per-frame work is bounded
    /// whatever a sender does. The returned buffer is the caller's until the next
    /// drain.
    pub fn drain(&mut self) -> &Drained {
        self.scratch.clear();
        if let Ok(mut pending) = self.shared.pending.lock() {
            std::mem::swap(&mut *pending, &mut self.scratch);
        }
        &self.scratch
    }

    /// The buffer the last [`drain`](Self::drain) took, without taking another.
    ///
    /// Empty before the first drain, and it holds whatever that drain returned
    /// until the next one. It exists so a caller applying a drained frame in the
    /// fixed order - transport, then preset, then clears, then values, then
    /// pings - can hand the earlier steps to code that borrows something else
    /// and come back here for the rest. Calling `drain` a second time for that
    /// would swap in an empty buffer and discard the frame.
    pub fn last_drained(&self) -> &Drained {
        &self.scratch
    }
}

impl Drop for Control {
    /// Ask the listener to stop and wait for it.
    ///
    /// Joined rather than detached so a test can assert the socket is released;
    /// the wait is bounded by `POLL`, which is what the read timeout is for.
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// The listener thread's whole body.
///
/// Allocates nothing after entry: the receive buffer is on the stack, the
/// decoder is allocation-free by construction, and the queue's capacity was
/// reserved before this thread existed. Every total it moves is a relaxed
/// atomic — the readings are running counts a reader samples once a second, not
/// a happens-before edge anything downstream depends on.
fn listen<R: Receive>(socket: &R, shared: &Shared, stop: &AtomicBool) {
    let _listening = Listening::start(shared);
    let mut buf = [0u8; RECV_BUF];
    let mut failures = 0u32;
    while !stop.load(Ordering::Relaxed) {
        let len = match socket.receive(&mut buf) {
            Ok(len) => {
                failures = 0;
                shared.received.fetch_add(1, Ordering::Relaxed);
                len
            }
            // The ordinary case, and how the stop flag gets read. It says the
            // socket is healthy and idle, so it also clears the failure run.
            Err(ref err) if is_read_timeout(err) => {
                failures = 0;
                continue;
            }
            // Counted rather than logged: a socket that fails keeps failing,
            // and a line per occurrence floods a show machine exactly when
            // something is already wrong (ADR-0221).
            Err(_) => {
                shared.recv_errors.fetch_add(1, Ordering::Relaxed);
                failures += 1;
                if failures >= RECV_ERROR_BUDGET {
                    return;
                }
                continue;
            }
        };
        let Some(datagram) = buf.get(..len) else {
            continue;
        };
        match decode::decode(datagram) {
            Ok(action) => {
                let recorded = shared
                    .pending
                    .lock()
                    .map(|mut pending| pending.record(action))
                    .unwrap_or(false);
                if !recorded {
                    shared.dropped.fetch_add(1, Ordering::Relaxed);
                }
            }
            Err(_) => {
                shared.rejected.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name(text: &str) -> Name {
        Name::new(text).expect("a short test name fits inline")
    }

    /// A scripted receiver: one outcome per call, then the last one forever.
    ///
    /// The socket a real listener reads is owned by its own thread and cannot
    /// be broken from outside it, so this is how the receive loop's failure
    /// policy is exercised at all.
    struct Scripted {
        script: Vec<io::ErrorKind>,
        datagrams: Vec<Vec<u8>>,
        calls: std::sync::atomic::AtomicUsize,
    }

    impl Scripted {
        /// A receiver that fails with `kind` on every call.
        fn failing(kind: io::ErrorKind) -> Self {
            Self {
                script: vec![kind],
                datagrams: Vec::new(),
                calls: std::sync::atomic::AtomicUsize::new(0),
            }
        }

        /// A receiver that hands over each datagram in turn and then times out
        /// forever.
        fn delivering(datagrams: Vec<Vec<u8>>) -> Self {
            Self {
                script: vec![io::ErrorKind::WouldBlock],
                datagrams,
                calls: std::sync::atomic::AtomicUsize::new(0),
            }
        }

        fn calls(&self) -> usize {
            self.calls.load(Ordering::Relaxed)
        }
    }

    impl Receive for Scripted {
        fn receive(&self, buf: &mut [u8]) -> io::Result<usize> {
            let call = self.calls.fetch_add(1, Ordering::Relaxed);
            if let Some(datagram) = self.datagrams.get(call) {
                buf[..datagram.len()].copy_from_slice(datagram);
                return Ok(datagram.len());
            }
            let index = call.saturating_sub(self.datagrams.len());
            let kind = self
                .script
                .get(index)
                .copied()
                .or_else(|| self.script.last().copied())
                .unwrap_or(io::ErrorKind::WouldBlock);
            Err(io::Error::from(kind))
        }
    }

    /// The bytes a sender puts on the wire for `ctl/param warp 0.5`.
    fn param_datagram() -> Vec<u8> {
        let mut buf = Vec::new();
        Action::Param {
            name: name("warp"),
            value: 0.5,
        }
        .encode(&mut buf);
        buf
    }

    /// A real bound listener that has been sent one datagram reports one
    /// received, and reports itself listening the whole time.
    #[allow(
        clippy::disallowed_methods,
        reason = "a delivery deadline on a socket this test does not schedule"
    )]
    #[test]
    fn a_delivered_datagram_is_counted_as_received() {
        let Ok(mut control) = Control::bind("127.0.0.1:0") else {
            eprintln!("skipped: could not bind a loopback control socket");
            return;
        };
        assert_eq!(control.received(), 0, "nothing has been sent yet");
        assert!(control.listening(), "a freshly bound listener is listening");

        let socket = UdpSocket::bind("127.0.0.1:0").expect("bind an ephemeral sender");
        socket
            .send_to(&param_datagram(), control.local_addr())
            .expect("send to the loopback listener");

        // The listener runs on a thread this test does not schedule, so the
        // only bound available is a deadline. Waited on the queue rather than
        // on the count: `received` rises the instant `recv_from` returns and
        // the record follows it, so a wait on the count would race the record.
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while !control.has_pending() && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(2));
        }
        assert_eq!(
            control.received(),
            1,
            "one datagram was sent, so exactly one should be counted as received"
        );
        assert_eq!(
            control.recv_errors(),
            0,
            "a healthy socket reports no receive errors"
        );
        assert!(
            control.listening(),
            "the listener is still reading the socket"
        );
        assert_eq!(
            control.drain().params().len(),
            1,
            "and the datagram reached the queue"
        );
    }

    /// A receive that keeps failing is counted, ends the listener, and leaves
    /// the listener reporting itself as not listening.
    #[test]
    fn a_failing_receive_is_counted_and_ends_the_listener() {
        let shared = Shared::new();
        let stop = AtomicBool::new(false);
        let socket = Scripted::failing(io::ErrorKind::ConnectionReset);

        listen(&socket, &shared, &stop);

        assert_eq!(
            shared.recv_errors.load(Ordering::Relaxed),
            u64::from(RECV_ERROR_BUDGET),
            "every failure up to the budget is counted"
        );
        assert_eq!(
            shared.received.load(Ordering::Relaxed),
            0,
            "a failed receive delivered no datagram"
        );
        assert!(
            !shared.listening.load(Ordering::Relaxed),
            "a listener that has left its receive loop must not report itself \
             as still listening"
        );
        assert!(
            !stop.load(Ordering::Relaxed),
            "it stopped on the socket, not because it was asked to"
        );
    }

    /// The ordinary read timeout moves neither counter, at any rate, and leaves
    /// the listener listening until it is asked to stop.
    #[test]
    fn the_timeout_path_counts_nothing_and_stays_listening() {
        for kind in [io::ErrorKind::WouldBlock, io::ErrorKind::TimedOut] {
            let shared = Shared::new();
            let stop = AtomicBool::new(false);
            let socket = Scripted::failing(kind);

            // Ten thousand timeouts, far past the failure budget: a timeout
            // says the socket is idle, so no number of them is a failure.
            std::thread::scope(|scope| {
                scope.spawn(|| {
                    while socket.calls() < 10_000 {
                        std::hint::spin_loop();
                    }
                    assert!(
                        shared.listening.load(Ordering::Relaxed),
                        "a listener that has only timed out is still listening, \
                         however many {kind:?} timeouts it has seen"
                    );
                    stop.store(true, Ordering::Relaxed);
                });
                listen(&socket, &shared, &stop);
            });

            assert!(
                socket.calls() >= 10_000,
                "the loop kept receiving through {kind:?} timeouts"
            );
            assert_eq!(
                shared.recv_errors.load(Ordering::Relaxed),
                0,
                "a {kind:?} read timeout is not a receive error"
            );
            assert_eq!(
                shared.received.load(Ordering::Relaxed),
                0,
                "a timeout delivered no datagram"
            );
        }
    }

    /// A timeout between two failures clears the run, so a transient refusal
    /// never reaches the budget.
    #[test]
    fn a_transient_failure_does_not_end_the_listener() {
        let shared = Shared::new();
        let stop = AtomicBool::new(false);
        let socket = Scripted {
            // One failure, one timeout, repeated: twice the budget's worth of
            // failures, none of them consecutive.
            script: (0..RECV_ERROR_BUDGET * 2)
                .flat_map(|_| [io::ErrorKind::ConnectionReset, io::ErrorKind::WouldBlock])
                .collect(),
            datagrams: Vec::new(),
            calls: std::sync::atomic::AtomicUsize::new(0),
        };
        let wanted = RECV_ERROR_BUDGET as usize * 4;

        std::thread::scope(|scope| {
            scope.spawn(|| {
                while socket.calls() < wanted {
                    std::hint::spin_loop();
                }
                stop.store(true, Ordering::Relaxed);
            });
            listen(&socket, &shared, &stop);
        });

        assert!(
            shared.recv_errors.load(Ordering::Relaxed) > u64::from(RECV_ERROR_BUDGET),
            "more failures than the budget were counted, and the listener ran \
             on through them because none of them were consecutive"
        );
    }

    /// A datagram received and then refused by the decoder is counted in both
    /// places: `received` is what separates "nothing arrived" from "something
    /// arrived and was discarded".
    #[test]
    fn a_rejected_datagram_is_counted_as_received_too() {
        let shared = Shared::new();
        let stop = AtomicBool::new(false);
        let socket = Scripted::delivering(vec![b"not osc at all\0\0".to_vec()]);

        std::thread::scope(|scope| {
            scope.spawn(|| {
                while socket.calls() < 4 {
                    std::hint::spin_loop();
                }
                stop.store(true, Ordering::Relaxed);
            });
            listen(&socket, &shared, &stop);
        });

        assert_eq!(
            shared.received.load(Ordering::Relaxed),
            1,
            "the datagram arrived, whatever the decoder made of it"
        );
        assert_eq!(
            shared.rejected.load(Ordering::Relaxed),
            1,
            "and the decoder refused it"
        );
        assert_eq!(
            shared.recv_errors.load(Ordering::Relaxed),
            0,
            "a datagram the decoder refuses is not a receive failure"
        );
    }

    /// A `ctl/preset` the renderer declines carries the asked-for name back out,
    /// and one it accepts carries nothing.
    ///
    /// The decision the `preset_error` emission acts on, asserted here because
    /// the emission itself writes to standard error and offers nothing to read
    /// back. Before this, a `false` from `select_preset_by_name` produced no
    /// event, no counter and no line, so a studio click on a preset the player
    /// will not select looked exactly like one it was about to select
    /// (ADR-0221).
    #[test]
    fn a_refused_preset_selection_carries_the_name_back_out() {
        let mut renderer = match Renderer::new_headless(rlx_core::render::HeadlessOptions {
            width: 64,
            height: 48,
            prefer_software: true,
        }) {
            Ok(renderer) => renderer,
            Err(_) => {
                eprintln!("skipped: no GPU adapter on this runner (ADR-0016)");
                return;
            }
        };
        let preset = rlx_core::preset::Preset::from_toml_str(
            "system = \"swarm\"\nname = \"held\"\n[params]\nbg_bright = \"0.2\"\n",
        )
        .expect("hand-written probe preset is valid");
        renderer.set_presets(vec![preset]);

        let mut drained = Pending::default();
        drained.record(Action::Preset {
            name: name("no_such_preset"),
        });
        let applied = apply_to_renderer(&drained, &mut renderer);
        assert!(
            !applied.switched,
            "a name the roster does not hold cannot switch"
        );
        assert_eq!(
            applied
                .unresolved_preset
                .map(|held| held.as_str().to_owned()),
            Some("no_such_preset".to_owned()),
            "a refused selection must name what was asked for, or the parent \
             that asked is told nothing at all"
        );

        let mut drained = Pending::default();
        drained.record(Action::Preset { name: name("held") });
        let applied = apply_to_renderer(&drained, &mut renderer);
        assert!(applied.switched, "a name the roster holds switches");
        assert_eq!(
            applied.unresolved_preset, None,
            "a selection that took is not an error, and reporting one would put \
             a red line on the studio for every preset click that worked"
        );

        // And a frame that asked for no preset at all: the third case, which
        // is every ordinary parameter frame and must stay silent.
        let mut drained = Pending::default();
        drained.record(Action::Param {
            name: name("bg_bright"),
            value: 0.5,
        });
        let applied = apply_to_renderer(&drained, &mut renderer);
        assert_eq!(
            applied.unresolved_preset, None,
            "a frame carrying no ctl/preset asked for nothing to select"
        );
    }

    /// A flood of one name occupies one slot, and the frame sees the last value.
    #[test]
    fn a_flood_of_one_name_leaves_one_slot_holding_the_last_value() {
        let mut pending = Pending::default();
        for i in 0..10_000 {
            assert!(
                pending.record(Action::Param {
                    name: name("warp"),
                    value: i as f32,
                }),
                "a repeated name never fills the queue"
            );
        }
        assert_eq!(
            pending.params().len(),
            1,
            "ten thousand datagrams for one name occupy one slot"
        );
        assert_eq!(
            pending.params().first().map(|(_, v)| *v),
            Some(9_999.0),
            "the frame sees the last value sent"
        );
    }

    /// Distinct names past the cap are dropped, not grown into.
    #[test]
    fn distinct_names_past_the_cap_are_dropped_rather_than_allocated() {
        let mut pending = Pending::default();
        let capacity = pending.params.capacity();
        for i in 0..capacity {
            assert!(
                pending.record(Action::Param {
                    name: name(&format!("p{i}")),
                    value: 1.0,
                }),
                "the first {capacity} distinct names fit"
            );
        }
        assert!(
            !pending.record(Action::Param {
                name: name("one_too_many"),
                value: 1.0,
            }),
            "the name past the cap is refused"
        );
        assert_eq!(
            pending.params.capacity(),
            capacity,
            "the buffer did not grow, so the listener never reached the allocator"
        );
    }

    /// Transport verbs are events and keep their order; a preset is a
    /// destination and only the last one matters.
    #[test]
    fn transport_verbs_are_kept_in_order_and_a_preset_is_a_destination() {
        let mut pending = Pending::default();
        pending.record(Action::Transport(Transport::Next));
        pending.record(Action::Transport(Transport::Next));
        pending.record(Action::Transport(Transport::Auto));
        pending.record(Action::Preset { name: name("a") });
        pending.record(Action::Preset { name: name("b") });

        assert_eq!(
            pending.transport(),
            [Transport::Next, Transport::Next, Transport::Auto],
            "two `next` verbs are two switches, in the order they arrived"
        );
        assert_eq!(
            pending.preset().map(Name::as_str),
            Some("b"),
            "the last preset asked for is the one to land on"
        );
    }

    /// **A mark is a state, so the last one per `(preset, mark)` pair wins** and
    /// a surface restating what it already set fills nothing. The two marks on
    /// one preset are separate pairs, because they are separate questions.
    #[test]
    fn a_mark_keeps_the_last_state_per_preset_and_kind() {
        let mut pending = Pending::default();
        let capacity = pending.marks.capacity();

        for on in [true, false, true] {
            assert!(pending.record(Action::Mark {
                name: name("Gyre"),
                mark: Mark::Favourite,
                on,
            }));
        }
        assert_eq!(
            pending.marks().len(),
            1,
            "three states for one pair occupy one slot"
        );
        assert!(pending.marks()[0].2, "the frame sees the last state sent");

        pending.record(Action::Mark {
            name: name("Gyre"),
            mark: Mark::Hidden,
            on: true,
        });
        assert_eq!(
            pending.marks().len(),
            2,
            "the two marks on one preset are separate questions"
        );

        // Distinct pairs past the cap are dropped, not grown into.
        for i in 0..capacity {
            pending.record(Action::Mark {
                name: name(&format!("p{i}")),
                mark: Mark::Favourite,
                on: true,
            });
        }
        assert_eq!(
            pending.marks.capacity(),
            capacity,
            "the buffer grew, so the listener reached the allocator"
        );
        assert!(
            pending.marks.len() <= capacity,
            "the cap did not hold: {} entries",
            pending.marks.len()
        );

        // And a frame carrying only marks is not empty, or the drain would
        // never take it.
        let mut only = Pending::default();
        only.record(Action::Mark {
            name: name("Gyre"),
            mark: Mark::Favourite,
            on: true,
        });
        assert!(!only.is_empty());
        only.clear();
        assert!(only.is_empty(), "clear left a mark behind");
    }

    /// A drain hands the caller what arrived and leaves the listener an empty
    /// buffer with its capacity intact.
    #[test]
    fn a_drain_swaps_the_buffers_rather_than_taking_one() {
        let shared = Arc::new(Shared::new());
        let mut control = Control {
            local: "127.0.0.1:0".parse().expect("a literal socket address"),
            shared: Arc::clone(&shared),
            scratch: Pending::default(),
            stop: Arc::new(AtomicBool::new(false)),
            thread: None,
        };
        let capacity = {
            let mut pending = shared.pending.lock().expect("uncontended in a test");
            pending.record(Action::Param {
                name: name("warp"),
                value: 0.5,
            });
            pending.params.capacity()
        };

        let drained = control.drain();
        assert_eq!(drained.params().len(), 1, "the drain carries what arrived");

        let left = shared.pending.lock().expect("uncontended in a test");
        assert!(left.is_empty(), "the listener's buffer is empty again");
        assert_eq!(
            left.params.capacity(),
            capacity,
            "and keeps its capacity, so the next packet allocates nothing"
        );
    }
}
