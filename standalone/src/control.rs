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

use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use rlx_core::render::Renderer;

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

/// Receive buffer. One Ethernet MTU, which is far more than any message in the
/// vocabulary needs and enough that a legitimate datagram is never truncated
/// into a malformed one.
const RECV_BUF: usize = 1500;

/// How long the listener blocks before checking whether it has been asked to
/// stop. The socket is otherwise idle, so this is the whole cost of being able
/// to shut the thread down rather than leaking it.
const POLL: Duration = Duration::from_millis(200);

/// What the listener has accumulated since the render thread last drained.
///
/// Fixed capacity throughout — see the module docs.
pub struct Pending {
    params: Vec<(Name, f32)>,
    clears: Vec<Name>,
    clear_all: bool,
    preset: Option<Name>,
    transport: Vec<Transport>,
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
        self.pings.clear();
    }

    /// Whether anything at all was received.
    pub fn is_empty(&self) -> bool {
        self.params.is_empty()
            && self.clears.is_empty()
            && !self.clear_all
            && self.preset.is_none()
            && self.transport.is_empty()
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

/// What the two threads share: the queue, and the two running totals the
/// `health` event reports.
struct Shared {
    pending: Mutex<Pending>,
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

        let shared = Arc::new(Shared {
            pending: Mutex::new(Pending::default()),
            rejected: AtomicU64::new(0),
            dropped: AtomicU64::new(0),
            refused: AtomicU64::new(0),
        });
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
/// reserved before this thread existed.
fn listen(socket: &UdpSocket, shared: &Shared, stop: &AtomicBool) {
    let mut buf = [0u8; RECV_BUF];
    while !stop.load(Ordering::Relaxed) {
        let Ok((len, _from)) = socket.recv_from(&mut buf) else {
            // A timeout is the ordinary case and is how the stop flag gets
            // read; a real error on a bound loopback socket is not worth a line
            // per occurrence on a show machine, and the next loop re-reads.
            continue;
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

    /// A drain hands the caller what arrived and leaves the listener an empty
    /// buffer with its capacity intact.
    #[test]
    fn a_drain_swaps_the_buffers_rather_than_taking_one() {
        let shared = Arc::new(Shared {
            pending: Mutex::new(Pending::default()),
            rejected: AtomicU64::new(0),
            dropped: AtomicU64::new(0),
            refused: AtomicU64::new(0),
        });
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
