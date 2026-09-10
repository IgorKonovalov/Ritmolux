//! The show a run manages around the renderer.
//!
//! One place performs preset-directory resolution and seeding, the hot-reload
//! watcher, scene rotation, the structured event emissions and the control-in
//! drain. The windowed path and the headless `--stream` path both call it; what
//! stays different between them is the window and which sink the frames go to.
//!
//! **The invariant this module exists to hold: a path that holds show state
//! reports it.** A `Renderer`, a `Director` and preset-selection-by-name are the
//! state; the directory, the watcher, the listener and the emissions are the
//! management around it. Splitting those two across paths lets one path hold the
//! state and say nothing about it — silent rather than broken, and so caught by
//! nothing (ADR-0181).

use std::path::{Path, PathBuf};
use std::time::Instant;

use rlx_core::render::Renderer;
use standalone::config;
use standalone::control::Control;
use standalone::events::{Event, Events};
use standalone::osc::decode::Transport;

use crate::director::Director;
use crate::preset_dir::{PRESET_POLL, dir_signature, reload_presets, startup_preset_dir};

/// How often a `health` event goes out while frames are being drawn.
///
/// One second, which is the cadence ADR-0176 names and the same one the
/// diagnostics log samples at — a parent plotting frame time and an operator
/// reading the file afterwards are looking at the same rate, so the two readings
/// are comparable rather than merely similar.
const HEALTH_INTERVAL: std::time::Duration = std::time::Duration::from_secs(1);

/// The largest number of pings one drained frame can carry, which is the
/// listener's own cap. Sized so the copy in [`Show::apply_control_rest`] is
/// bounded by construction rather than by what a sender happens to send.
const PING_SCRATCH: usize = 8;

/// Everything a run manages around the renderer, in one owner.
pub(crate) struct Show {
    /// Preset directory watched for hot-reload, with its last-seen signature and
    /// poll deadline. Empty when no per-user directory resolved, and then the
    /// watch is inert and the renderer keeps its embedded set (NFR 10).
    dir: PathBuf,

    sig: Option<(u128, usize)>,

    last_poll: Instant,

    /// Hands-off scene rotation policy, driven each drawn frame with the
    /// injected `dt`. Built from a `[rotate]` block the caller has already
    /// narrowed: the two paths disagree about the default — a window has an
    /// operator to press Space and a headless source does not — and that
    /// disagreement belongs to the caller rather than here.
    pub(crate) director: Director,

    /// The structured event stream (ADR-0176), present only when `--events`
    /// turned it on. Absent otherwise, so every emission below is a `None` test
    /// and standard error carries exactly what it always did.
    pub(crate) events: Option<Events>,

    /// The studio control-in listener (ADR-0176), present only when `--control`
    /// or `[control] enabled` turned it on. Absent otherwise, and then no socket
    /// is bound at all.
    control: Option<Control>,

    /// The preset name last reported through the event stream, so `preset` is
    /// emitted on a **change** rather than every frame. Empty until the first
    /// frame reports one.
    reported_preset: String,

    /// When the next `health` event is due. The stream's own cadence, not the
    /// diagnostics log's: that one writes a file an operator reads afterwards
    /// and this one feeds a parent watching now, so neither may silence the
    /// other by being configured off.
    next_health: Instant,
}

impl Show {
    /// Resolve the preset directory, seed the curated set into it on first run,
    /// load it over the renderer's embedded defaults, and record the signature
    /// later edits are compared against.
    ///
    /// `rotate` is taken already narrowed — see [`Show::director`].
    ///
    /// Any failure degrades to the embedded defaults rather than stopping the
    /// run: an unresolvable per-user directory is a notice and an inert watch,
    /// never an error (NFR 10).
    #[allow(
        clippy::disallowed_methods,
        reason = "the poll and health deadlines are shell frame pacing; core analysis stays clock-free"
    )]
    pub(crate) fn start(
        renderer: &mut Renderer,
        rotate: &config::Rotate,
        events: Option<Events>,
        control: Option<Control>,
    ) -> Self {
        let now = Instant::now();
        let mut show = Self {
            dir: startup_preset_dir(),
            sig: None,
            last_poll: now,
            director: Director::from_config(rotate),
            events,
            control,
            reported_preset: String::new(),
            // Due one interval from now, not immediately: the diagnostics window
            // is empty before the first frame, so a reading taken at startup is
            // a row of zeros — which a parent cannot tell from a player that has
            // stalled.
            next_health: now + HEALTH_INTERVAL,
        };
        show.reload(renderer);
        show
    }

    /// The resolved preset directory, or an empty path when none did.
    pub(crate) fn preset_dir(&self) -> &Path {
        &self.dir
    }

    /// **The single caller of [`reload_presets`]**, which is what holds the
    /// startup load and the watcher's reload to one behaviour rather than two
    /// that agree today. `sig` is re-baselined here so the next poll compares
    /// against what this load actually saw.
    fn reload(&mut self, renderer: &mut Renderer) {
        reload_presets(renderer, &self.dir, self.events.as_mut());
        self.sig = dir_signature(&self.dir);
    }

    /// Re-scan the preset directory if the poll interval has elapsed and its
    /// signature changed, hot-reloading on any edit.
    ///
    /// Returns whether a reload ran, so a caller with bookkeeping of its own — a
    /// browse overlay's highlight, a truncation baseline — does it only on the
    /// frames that changed anything. Keeps the current set if the reload yields
    /// nothing valid (degrade, never crash — NFR 10).
    #[allow(
        clippy::disallowed_methods,
        reason = "preset-poll pacing reads the wall clock; core analysis stays clock-free"
    )]
    pub(crate) fn poll_presets(&mut self, renderer: &mut Renderer) -> bool {
        if self.last_poll.elapsed() < PRESET_POLL {
            return false;
        }
        self.last_poll = Instant::now();
        let sig = dir_signature(&self.dir);
        if sig == self.sig {
            return false;
        }
        self.reload(renderer);
        true
    }

    /// Announce the geometry a parent cuts the frame pipe into pictures by.
    ///
    /// **Before the first frame**, which is the contract: a `stream` that
    /// arrived after them would leave the first frame unreadable. The numbers
    /// differ per path — a requested size and rate headless, the readback's size
    /// and the display's rate windowed — so the caller supplies them.
    pub(crate) fn emit_stream(&mut self, width: u32, height: u32, fps: u32) {
        if let Some(events) = self.events.as_mut() {
            events.emit(&Event::Stream {
                width,
                height,
                fps,
                format: crate::stream::STREAM_FORMAT,
            });
        }
    }

    /// Emit a `preset` event when the preset **on screen** has changed.
    ///
    /// Reported from what was actually drawn rather than from each of the sites
    /// that can change it — the director's rotation, a hotkey, the browse
    /// overlay, the console's strip, `--preset`, and `ctl/preset`. A switch
    /// dissolves, so a site announcing its own would name the incoming preset a
    /// frame before it was drawn, and every announcement would be one more
    /// chance to disagree about that.
    pub(crate) fn report_active_preset(&mut self, renderer: &Renderer) {
        let Some(events) = self.events.as_mut() else {
            return;
        };
        let name = renderer.preset_name();
        if name == self.reported_preset {
            return;
        }
        let index = renderer
            .preset_names()
            .position(|candidate| candidate == name)
            .unwrap_or(0);
        events.emit(&Event::Preset { name, index });
        self.reported_preset = name.to_owned();
    }

    /// Emit a `health` event once a second while frames are being drawn.
    ///
    /// Tied to the drawn frame rather than to a timer of its own, so a stalled
    /// or hidden player goes quiet: a parent watching this stream reads silence
    /// as "no frames", which is the fact it wants and which a timer that kept
    /// ticking would hide.
    pub(crate) fn report_health(&mut self, renderer: &Renderer, now: Instant) {
        let Some(events) = self.events.as_mut() else {
            return;
        };
        if now < self.next_health {
            return;
        }
        self.next_health = now + HEALTH_INTERVAL;
        let metrics = renderer.metrics();
        let (rejected, dropped, refused) = self
            .control
            .as_ref()
            .map_or((0, 0, 0), |c| (c.rejected(), c.dropped(), c.refused()));
        events.emit(&Event::Health {
            fps: metrics.fps,
            frame_ms_p50: renderer.frame_ms_p50(),
            frame_ms_p99: metrics.frame_ms_p99,
            ctl_rejected: rejected,
            ctl_dropped: dropped,
            ctl_refused: refused,
        });
    }

    /// Step one of the control drain: take the socket's traffic and copy out
    /// this frame's transport verbs.
    ///
    /// **The pair is load-bearing and must be called together.** The drained
    /// buffer stays the listener's until the next drain, so
    /// [`Show::apply_control_rest`] reads the same frame this call took; a
    /// caller that stops after this one has dropped a frame of parameter values
    /// on the floor. They are two calls rather than one because the fixed order
    /// a drained frame is applied in — transport, then preset, then clears, then
    /// values, then pings — puts a step in the middle that needs the caller's own
    /// state: a `ctl/transport` verb resolves the operator console's own action
    /// rather than a second copy of the rule (spec 0003).
    ///
    /// `out` is the caller's retained scratch, so a steady-state frame allocates
    /// nothing on the render thread. Returns whether anything arrived at all.
    pub(crate) fn take_control_transports(&mut self, out: &mut Vec<Transport>) -> bool {
        out.clear();
        let Some(control) = self.control.as_mut() else {
            return false;
        };
        let drained = control.drain();
        if drained.is_empty() {
            return false;
        }
        out.extend_from_slice(drained.transport());
        true
    }

    /// Step two of the control drain: apply everything after the transport
    /// verbs — the renderer-only half through the same function the loopback
    /// test drives, then the pings, then the refusal count.
    ///
    /// Returns whether a preset switch happened, so the caller runs whatever
    /// post-switch bookkeeping it has. A parameter name nothing claims is
    /// refused by the core and **counted**, not printed: OSC has no reply
    /// channel, and a sender scrubbing a mistyped name at slider rate would
    /// otherwise produce a line per frame.
    pub(crate) fn apply_control_rest(&mut self, renderer: &mut Renderer) -> bool {
        // Moved out of `self` for the duration rather than borrowed from it: the
        // drained buffer borrows the listener, and the pongs below need `self`.
        let Some(control) = self.control.take() else {
            return false;
        };
        let drained = control.last_drained();
        let applied = standalone::control::apply_to_renderer(drained, renderer);
        // A ping is rare and deliberate and the cap is eight, so copying the
        // nonces out is bounded by construction and off the steady-state path.
        let nonces = drained.pings();
        let mut scratch = [0_i32; PING_SCRATCH];
        let count = nonces.len().min(PING_SCRATCH);
        scratch[..count].copy_from_slice(&nonces[..count]);
        control.note_refused(applied.refused);
        self.control = Some(control);
        // The one message answered individually (ADR-0176): OSC carries no
        // acknowledgement, so `ping` exists precisely so a studio can tell a dead
        // player from a quiet one, and the answer goes back on the stream that
        // cannot drop.
        if let Some(events) = self.events.as_mut() {
            for nonce in &scratch[..count] {
                events.emit(&Event::Pong { nonce: *nonce });
            }
        }
        applied.switched
    }
}
