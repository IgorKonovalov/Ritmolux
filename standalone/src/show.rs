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

use rlx_core::render::{PixelOrder, Renderer};
use standalone::config;
use standalone::control::Control;
use standalone::events::{Event, Events};
use standalone::marks::{Mark, Marks};
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

/// What a `preset_error` raised by a refused `ctl/preset` says.
///
/// Phrased as the request's outcome rather than as a cause, because the
/// renderer returns a bare `false` and has no reason to offer: the roster may
/// not hold the name, or may have been replaced between the click and the
/// datagram.
const UNRESOLVED_PRESET: &str =
    "ctl/preset: the selection did not take, so this preset is not on screen";

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

    /// What the last `preset` event said — name, system key and family — so the
    /// event is emitted on a **change** rather than every frame. `None` until
    /// the first frame reports one.
    ///
    /// **All three, not the name alone** (ADR-0194 point 4). A reload rewrites a
    /// preset in place, so a file whose `[curve] family` or whose `system` was
    /// edited comes back under the name it already had; keyed on the name that
    /// is not a change, and a parent keeps drawing a panel for the system it was
    /// last told about.
    reported_preset: Option<(String, &'static str, Option<&'static str>)>,

    /// The user's marks on the library, and the file they live in (ADR-0228).
    ///
    /// Held here rather than on the window's state because every path that runs
    /// a show holds one of these: the marks decide what rotation draws from, and
    /// a path that held them and rotated without them would be silently
    /// different rather than broken (ADR-0181). `path` is `None` when no
    /// per-user directory resolved, and then a mark applies live and is not
    /// persisted — the rule `config.toml` already follows.
    marks: Marks,

    marks_path: Option<PathBuf>,

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
        marks_path: Option<PathBuf>,
    ) -> Self {
        let now = Instant::now();
        let mut show = Self {
            dir: startup_preset_dir(),
            sig: None,
            last_poll: now,
            director: Director::from_config(rotate),
            events,
            control,
            reported_preset: None,
            marks: marks_path.as_deref().map(Marks::load).unwrap_or_default(),
            marks_path,
            // Due one interval from now, not immediately: the diagnostics window
            // is empty before the first frame, so a reading taken at startup is
            // a row of zeros — which a parent cannot tell from a player that has
            // stalled.
            next_health: now + HEALTH_INTERVAL,
        };
        show.reload(renderer);
        // What the user state carried across the restart. Silent when there is
        // none, so a fresh install says nothing; a count otherwise, because a
        // marks file that failed to load reports its own reason and one that
        // loaded empty would otherwise be indistinguishable from one that
        // loaded at all.
        let (favourite, hidden) = (show.marks.favourite.len(), show.marks.hidden.len());
        if favourite + hidden > 0 {
            eprintln!("preset marks: {favourite} favourite, {hidden} hidden");
        }
        show
    }

    /// The resolved preset directory, or an empty path when none did.
    pub(crate) fn preset_dir(&self) -> &Path {
        &self.dir
    }

    /// Put `name` in or out of `mark`'s set, persisting the change.
    ///
    /// **The one writer**, whatever asked — a hotkey, the browser, or a control
    /// message (ADR-0229). Returns whether anything moved, so a surface
    /// restating a mark it already set neither rewrites the file nor announces a
    /// change that did not happen.
    pub(crate) fn set_mark(&mut self, mark: Mark, name: &str, on: bool) -> bool {
        if !self.marks.apply(mark, name, on) {
            return false;
        }
        if let Some(path) = &self.marks_path {
            self.marks.save(path);
        }
        true
    }

    /// Flip `name`'s membership of `mark`'s set, returning the new state.
    pub(crate) fn toggle_mark(&mut self, mark: Mark, name: &str) -> bool {
        let on = !self.marks.is(mark, name);
        self.set_mark(mark, name, on);
        on
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
    /// arrived after them would leave the first frame unreadable. Every value
    /// differs per path — a requested size and rate headless, the mirror's size
    /// and the display's rate windowed, and a channel order that is the
    /// headless offscreen's on one path and whatever the swapchain negotiated on
    /// the other — so the caller supplies them, read from what the renderer is
    /// actually producing rather than from a constant (ADR-0187).
    pub(crate) fn emit_stream(&mut self, width: u32, height: u32, fps: u32, order: PixelOrder) {
        if let Some(events) = self.events.as_mut() {
            events.emit(&Event::Stream {
                width,
                height,
                fps,
                format: order.as_str(),
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
    ///
    /// **A reload that edits the on-screen preset's system or family re-reports
    /// it under the same name** (ADR-0194 point 4), so a parent receives a
    /// `preset` naming the preset it already shows. That is a refresh, and spec
    /// 0003 says so; a parent that resets transient state on every `preset` now
    /// resets on a family edit too. A reload that changes none of the three
    /// still emits nothing.
    pub(crate) fn report_active_preset(&mut self, renderer: &Renderer) {
        let Some(report) = self.preset_report(renderer) else {
            return;
        };
        let Some(events) = self.events.as_mut() else {
            return;
        };
        let (name, system, family) = &report;
        let index = renderer
            .preset_names()
            .position(|candidate| candidate == name)
            .unwrap_or(0);
        events.emit(&Event::Preset {
            name,
            index,
            system,
            file: renderer.active_preset_source(),
            family: *family,
        });
        self.reported_preset = Some(report);
    }

    /// What the on-screen preset is, or `None` because it is already what the
    /// last `preset` event said.
    ///
    /// **Separated from the emission so the decision can be asserted**:
    /// [`Events`] writes to standard error and offers nothing to read back, so a
    /// test at this seam can observe which of the two branches was taken only if
    /// the branch is a value.
    fn preset_report(
        &self,
        renderer: &Renderer,
    ) -> Option<(String, &'static str, Option<&'static str>)> {
        let name = renderer.preset_name();
        // The schema's key, never the scene's display name: the two coincide on
        // the four one-word systems and differ on every other, so a parent
        // resolving a parameter roster from the display name finds nothing for
        // most of them.
        let system = renderer.active_system_key();
        let family = renderer.active_family_key();
        let unchanged = self
            .reported_preset
            .as_ref()
            .is_some_and(|(seen, was, drew)| seen == name && *was == system && *drew == family);
        (!unchanged).then(|| (name.to_owned(), system, family))
    }

    /// Emit a `health` event once a second while frames are being drawn.
    ///
    /// Tied to the drawn frame rather than to a timer of its own, so a stalled
    /// or hidden player goes quiet: a parent watching this stream reads silence
    /// as "no frames", which is the fact it wants and which a timer that kept
    /// ticking would hide.
    /// `preview` is the preview pipe's `(sent, dropped)` totals, or `None` when
    /// no preview pipe is open. The caller supplies them because the pipe
    /// belongs to whichever path opened it, and only the producer can say how
    /// many frames it made: a reader downstream sees what arrived and cannot
    /// see what was never sent.
    pub(crate) fn report_health(
        &mut self,
        renderer: &Renderer,
        now: Instant,
        preview: Option<(u64, u64)>,
    ) {
        let Some(events) = self.events.as_mut() else {
            return;
        };
        if now < self.next_health {
            return;
        }
        self.next_health = now + HEALTH_INTERVAL;
        let metrics = renderer.metrics();
        // A run with no listener reports zeros and `false`: there is no thread,
        // so it is not listening, which is what `control` being `null` on
        // `hello` already told the parent.
        let control = self.control.as_ref();
        events.emit(&Event::Health {
            fps: metrics.fps,
            frame_ms_p50: renderer.frame_ms_p50(),
            frame_ms_p99: metrics.frame_ms_p99,
            ctl_rejected: control.map_or(0, Control::rejected),
            ctl_dropped: control.map_or(0, Control::dropped),
            ctl_refused: control.map_or(0, Control::refused),
            ctl_received: control.map_or(0, Control::received),
            ctl_recv_errors: control.map_or(0, Control::recv_errors),
            ctl_listening: control.is_some_and(Control::listening),
            preview_sent: preview.map(|(sent, _)| sent),
            preview_dropped: preview.map(|(_, dropped)| dropped),
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
    ///
    /// **A preset name the renderer declines is reported instead of counted**,
    /// because it is the opposite population: a preset arrives from a click on
    /// a roster the player itself published, so one refusal is one deliberate
    /// request that did not take, and a parent that is not told shows a click
    /// that did nothing (ADR-0221). It goes out as a `preset_error` naming the
    /// asked-for name, which is the event a parent already renders for "what
    /// you asked for is not on screen".
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
            if let Some(name) = applied.unresolved_preset.as_ref() {
                events.emit(&Event::PresetError {
                    // The asked-for name, which on this arm is not a path at
                    // all. Spec 0003 says so rather than leaving a parent to
                    // discover it.
                    file: Path::new(name.as_str()),
                    message: UNRESOLVED_PRESET,
                    line: None,
                    col: None,
                    param: None,
                });
            }
        }
        applied.switched
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use rlx_core::preset::Preset;
    use rlx_core::render::{HeadlessOptions, Renderer};

    /// The `[curve]` preset the reload cases rewrite: one name, one family, so
    /// what changes between two loads is exactly the family.
    const NAME: &str = "family report fixture";

    fn curve(family: &str) -> Preset {
        let text = format!(
            "system = \"parametric_curve\"\nname = \"{NAME}\"\n\n[curve]\nfamily = \"{family}\"\n"
        );
        Preset::from_toml_str(&text).expect("the fixture parses")
    }

    /// A `Show` with no directory, no watcher, no listener and no sink — every
    /// field this seam does not read, so the test is about the deduplication and
    /// nothing else. Built by literal rather than through `Show::start`, which
    /// resolves and seeds the real per-user preset directory.
    #[allow(
        clippy::disallowed_methods,
        reason = "`Instant::now` fills two deadline fields this seam never reads"
    )]
    fn show() -> Show {
        let now = Instant::now();
        Show {
            dir: PathBuf::new(),
            sig: None,
            last_poll: now,
            director: Director::from_config(&config::Rotate::default()),
            events: None,
            control: None,
            reported_preset: None,
            marks: Marks::default(),
            marks_path: None,
            next_health: now + HEALTH_INTERVAL,
        }
    }

    /// **A reload that rewrites the on-screen preset's family re-reports it**
    /// under the same name, and one that rewrites nothing reports nothing
    /// (ADR-0194 point 4).
    ///
    /// Under name-only deduplication the first of those was silent, and a parent
    /// went on drawing a slider whose ends belong to the family it was last told
    /// about. Asserted through `preset_report`, which is the decision
    /// `report_active_preset` acts on.
    #[test]
    fn a_reloaded_family_is_reported_again_and_an_unchanged_one_is_not() {
        let Ok(mut renderer) = Renderer::new_headless(HeadlessOptions {
            width: 64,
            height: 48,
            prefer_software: true,
        }) else {
            eprintln!("skipped: no GPU adapter on this runner (ADR-0016)");
            return;
        };
        let mut show = show();

        renderer.set_presets(vec![curve("lissajous")]);
        let first = show
            .preset_report(&renderer)
            .expect("the first frame reports the preset it drew");
        assert_eq!(
            first,
            (NAME.to_owned(), "parametric_curve", Some("lissajous")),
            "the first report names the preset, its system key and its family"
        );
        show.reported_preset = Some(first);

        // The reload a hot-reload watcher performs: same file, same name, a
        // rewritten `[curve] family`.
        renderer.set_presets(vec![curve("hypotrochoid")]);
        let second = show
            .preset_report(&renderer)
            .expect("a rewritten family is a change, whatever the name says");
        assert_eq!(
            second,
            (NAME.to_owned(), "parametric_curve", Some("hypotrochoid")),
            "the second report names the same preset and the new family"
        );
        show.reported_preset = Some(second);

        // And a reload that rewrote neither of the three.
        renderer.set_presets(vec![curve("hypotrochoid")]);
        assert_eq!(
            show.preset_report(&renderer),
            None,
            "a reload that changed neither the name, the system nor the family \
             must report nothing — this stream is a change feed"
        );
    }
}
