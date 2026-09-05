//! The live show: everything the shell holds once a window exists.
//!
//! [`AppState`]'s fields are grouped by what owns them — [`Capture`],
//! [`Presets`], [`Hud`] and [`Diagnostics`] — so the struct names four
//! collaborators rather than a flat roster, and a field added to one of them
//! does not widen the top-level shape.
//!
//! The methods are split across three files by what they do: routing lives in
//! [`crate::input`], this frame's text in [`crate::hud`], and the rest — the
//! frame loop, the window, the capture swap and the settings — here.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use rlx_core::audio::{AudioFormat, SampleConsumer};
use rlx_core::dsp::Analyzer;
use rlx_core::render::{AdapterChoice, CapOverflow, Renderer, RendererOptions, Tier};
use standalone::osc::{OscSink, Telemetry, rms_of};
use standalone::rss;
use winit::event_loop::ActiveEventLoop;
use winit::monitor::MonitorHandle;
use winit::window::{Fullscreen, Window};

#[cfg(windows)]
use crate::capture_start::capture_mode;
use crate::capture_start::{
    CAPTURE_BACKEND, DEFAULT_ENDPOINT, INPUT_RECOVERY_ATTEMPTS, Persist, Recovery, RecoveryPolicy,
    capture_handle, capture_lost, device_row_index, start_capture,
};
use crate::capture_verdict::CaptureVerdict;
#[cfg(windows)]
use crate::capture_win;
use crate::cli::resolve_log_path;
use crate::config::{self, Config};
use crate::console;
use crate::diaglog::DiagLog;
use crate::director::Director;
use crate::downbeatlog::DownbeatLog;
#[cfg(windows)]
use crate::nowplaying_win;
use crate::overlay::{OverlayKey, OverlayState};
use crate::preset_dir::{PRESET_POLL, dir_signature, reload_presets, startup_preset_dir};
use crate::run::{App, resolve_display};
use crate::settings::{SettingsAction, SettingsState, SettingsView, TierState};
use crate::soak::SoakLog;

/// How often the render loop wakes to keep DSP fed while hidden (NFR 1:
/// near-zero GPU in the background, analysis stays warm).
pub(crate) const HIDDEN_TICK: Duration = Duration::from_millis(100);

/// Window-title prefix: app name plus the application version. `CARGO_PKG_VERSION`
/// resolves at compile time to the single [workspace.package].version (ADR-0005).
pub(crate) const APP_TITLE: &str = concat!("Ritmolux ", env!("CARGO_PKG_VERSION"));
/// Clamp the per-frame `dt` fed to the scene director. A long hidden/paused gap
/// would otherwise dump a huge step into the dwell timer and rotate on return.
pub(crate) const MAX_DT: f32 = 0.25;
/// Refresh the window title (fps + p99) every this many rendered frames — a
/// frame-count cadence keeps the shell clock-free for the title; the numbers
/// themselves come from the core's diagnostics.
pub(crate) const TITLE_UPDATE_FRAMES: u32 = 30;
/// The operator console's window title, so it is tellable from the show's in a
/// taskbar and by a window manager.
pub(crate) const CONSOLE_TITLE: &str = concat!("Ritmolux console ", env!("CARGO_PKG_VERSION"));
/// The console's default size. Wide enough for the browser's multi-column list
/// at its current column width, short enough to sit beside other desk windows.
pub(crate) const CONSOLE_WIDTH: u32 = 900;
pub(crate) const CONSOLE_HEIGHT: u32 = 640;
/// Inset from the chosen monitor's top-left, so the console does not open flush
/// into a corner under a taskbar.
pub(crate) const CONSOLE_MARGIN: i32 = 64;

/// The capture stream and the analysis it feeds.
///
/// Everything the shell knows about what it is listening to: the running
/// stream, the format the analyzer was built on, the verdict token both durable
/// surfaces borrow, and the recovery policy that decides whether a dead stream
/// is reopened.
pub(crate) struct Capture {
    pub(crate) analyzer: Analyzer,

    pub(crate) consumer: Option<SampleConsumer>,

    // Held for its Drop: stops the capture thread with the app.
    pub(crate) _capture: Option<capture_handle::Handle>,

    /// The capture verdict of the stream running *now*, already rendered to its
    /// one-line token. Rendered in one place and stored, so the two durable
    /// surfaces — the `diagnostics.log` `capture` column and the F3 overlay —
    /// borrow the same string and cannot disagree; re-rendered on every swap, so
    /// the log answers what capture is listening to rather than what it started
    /// on (ADR-0142).
    pub(crate) capture_token: String,

    /// The format the live [`Capture::analyzer`] was built on. Kept beside the
    /// analyzer because a swap rebuilds it only when the negotiated format
    /// actually moved — an unchanged format keeps the AGC's running peak and the
    /// tempo history, which is most swaps between two 48 kHz endpoints.
    pub(crate) capture_format: AudioFormat,

    /// Friendly name of the endpoint the running stream actually opened, as the
    /// capture layer resolved it — `None` when nothing is running, or on a
    /// platform whose capture path selects no endpoint.
    ///
    /// This is what positions the `Input device` row. Deriving that position by
    /// matching the *configured* name against the roster instead needs a second
    /// implementation of `pick_device`'s rule, and a second one spelled
    /// differently disagrees whenever one endpoint's name is a substring of
    /// another's.
    pub(crate) capture_endpoint: Option<String>,

    pub(crate) scratch: Vec<f32>,

    /// The capture selection currently running — resolved at launch across
    /// `--input` / `--device` / `[input]`, and moved by the settings rows.
    /// Held apart from `config.input` because a launch flag pins the run
    /// without writing itself into the file the operator keeps.
    pub(crate) input: config::Input,

    /// Whether the running capture stream has reported itself dead and has not
    /// been replaced by a live one since.
    ///
    /// Sticky across a failed reopen on purpose: the capture thread's flag goes
    /// away with the handle, so a recovery attempt that fails would otherwise
    /// read as "nothing wrong" on the next frame and spend the whole retry
    /// budget on its first attempt.
    pub(crate) input_lost: bool,

    /// How many reopens the lost input has already cost, and whether the bound
    /// has been announced.
    pub(crate) input_recovery: RecoveryPolicy,

    /// The active endpoints of [`Self::input`]'s mode, `default` first, as the
    /// `Input device` row cycles them.
    ///
    /// **Cached, and refreshed only when the settings modal opens and when the
    /// mode changes.** Enumeration is COM — it allocates and blocks — and
    /// [`AppState::settings_view`] runs every frame the modal is up, so
    /// enumerating there would put a blocking COM call on the render thread once
    /// a frame. The cost of caching is that a device appearing or disappearing
    /// while the menu is open is not seen until it is reopened.
    pub(crate) input_roster: Vec<String>,
}

/// The preset directory and where the roster currently stands.
///
/// `sig` and `last_poll` are the hot-reload watch; `pending_switch_settle` is
/// the one-frame delay a dissolve costs anything that describes the *active*
/// preset (ADR-0007).
pub(crate) struct Presets {
    /// Preset directory watched for hot-reload, with its last-seen signature
    /// and poll deadline.
    pub(crate) preset_dir: PathBuf,

    pub(crate) preset_sig: Option<(u128, usize)>,

    pub(crate) last_preset_poll: Instant,

    /// Set at a preset switch, consumed after the next rendered frame: a switch
    /// now **dissolves** (Plan 0023), so the roster does not reach the incoming
    /// preset until that frame's capture step has run. Everything that describes
    /// the active preset therefore still answers with the *outgoing* one at the
    /// switch site — the window title, and (the one that does not self-correct)
    /// its segment-cap truncation, which does not even exist until the incoming
    /// preset's structural config is applied at the flip. ADR-0007 says the cap is
    /// never a silent cut, so the check waits for the frame that makes it real.
    pub(crate) pending_switch_settle: bool,
}

/// The two modals, the operator console's window, and the retained scratch
/// this frame's text is composed into.
///
/// Every buffer here is kept across frames for its capacity: the text queue is
/// cleared and re-queued every frame, so reuse — not an early return — is what
/// keeps a steady-state frame from allocating.
pub(crate) struct Hud {
    /// The now-playing metadata source, watching SMTC on its own thread
    /// (Plan 0097 Phase 2). Windows-only; absent everywhere else, where the
    /// banner simply never fires.
    #[cfg(windows)]
    pub(crate) now_playing: nowplaying_win::NowPlayingSource,

    /// The preset browse overlay's modal state (Tab toggles; Plan 0008).
    pub(crate) browse: OverlayState,

    /// The settings modal's state (`S` toggles; Plan 0050 Phase 4). A second,
    /// independent pure state machine — see [`crate::settings`] for why it is not the
    /// same one.
    pub(crate) settings: SettingsState,

    /// The operator console's window, `None` while it is closed (ADR-0143).
    ///
    /// The window is the whole of the console's shell-side state: there is no
    /// second `Renderer`, no second scene clock and no second modal state
    /// machine. What the console *shows* is decided every frame by
    /// `console::route`, from the same lines the output would have drawn.
    pub(crate) console_window: Option<Arc<Window>>,

    /// Last cursor position seen on the console surface, in its device pixels.
    ///
    /// Tracked rather than read at the press because winit's `MouseInput`
    /// carries no coordinates. Parked off-surface when the pointer leaves, so a
    /// press that arrives afterwards cannot land on a stale rectangle.
    pub(crate) console_cursor: (f32, f32),

    /// A console open/close asked for by something that had no
    /// `ActiveEventLoop` to create a window with — the settings row, or a
    /// launch flag. Serviced once per event, where one is in scope.
    ///
    /// The bool is **whether the result is written to `config.toml`**. An
    /// operator's own toggle persists, because where they left the console is a
    /// staging choice that should outlive the restart; a launch flag does not,
    /// because `--console` turns the console on for one run and must not edit
    /// the file — the shape `--input` / `--device` / `--osc` already follow
    /// (ADR-0142).
    pub(crate) console_request: Option<bool>,

    /// State for the console's `random` control.
    ///
    /// A counter mixed on each press rather than a dependency or a clock read:
    /// the only property the operator wants is *a different scene*, and
    /// `console::random_index` guarantees "not the current one" structurally.
    /// Seeded from the roster length so two machines with different libraries do
    /// not walk the same order, and advanced per press so a held button does not
    /// repeat one preset.
    pub(crate) random_state: u32,

    /// This frame's text, split by destination and retained across frames so the
    /// split reuses its buffers rather than allocating two vectors per frame.
    pub(crate) frame_text: console::FrameText,

    /// Scratch for the frame's modal rows, before routing moves them into
    /// [`Hud::frame_text`]. Retained for its capacity.
    pub(crate) modal_scratch: Vec<console::Line>,

    /// Retained scratch for [`AppState::queue_frame_text`], cleared at entry
    /// rather than reallocated (Plan 0061 Phase 5).
    ///
    /// `text_layer.end_frame()` clears the text queue every frame, so the runs
    /// have to be re-queued every frame — an early return is **not** the fix
    /// here, reuse is. Holding the two vectors on the state means a steady-state
    /// frame allocates only when the content grows past the retained capacity.
    /// The show's own furniture for this frame — corner preset name, capture
    /// verdict — before routing moves it onto the output.
    pub(crate) chrome_scratch: Vec<console::Line>,
}

/// What the run reports about itself: the F3 overlay, the ~1 Hz log, the
/// optional samplers, and the two announce-once latches.
///
/// `reported_overflow` and `reported_demotion` both report a **transition**
/// rather than a state, which is why each is held rather than re-derived.
pub(crate) struct Diagnostics {
    /// Frames since the last title refresh (title shows core-sourced fps + p99).
    pub(crate) title_tick: u32,

    /// Whether the diagnostics debug overlay is currently painted (toggled by F3).
    pub(crate) overlay_on: bool,

    /// ~1 Hz structured diagnostics logger (render thread only).
    pub(crate) diag_log: DiagLog,

    /// Long-run soak sampler, present only with `--soak` (else the render loop
    /// is byte-unchanged).
    pub(crate) soak: Option<SoakLog>,

    /// Per-beat downbeat decomposition log, present only with `--downbeat-log`
    /// (Plan 0086 Phase 1). Absent otherwise, so the frame path is unchanged.
    pub(crate) downbeat_log: Option<DownbeatLog>,

    /// Lighting telemetry sink, present only when `--osc` or `[osc] enabled`
    /// turned it on (ADR-0144). Absent otherwise, so the frame path is a `None`
    /// test and no socket is bound.
    pub(crate) osc: Option<OscSink>,

    /// The segment-cap truncation already announced on stderr, so
    /// [`poll_cap_overflow`](AppState::poll_cap_overflow) reports the **transition**
    /// rather than the state (Plan 0031 Phase 6).
    ///
    /// The load-time half of the cap (an oversized L-system depth) is announced by
    /// [`warn_cap_overflow`] when the preset changes. The **per-frame** half — a
    /// geometry mirror an audio expression drives over the cap — was tracked by the
    /// core and never reported, because the only reader ran on a preset change.
    /// This field is what lets the frame loop report it without shouting.
    pub(crate) reported_overflow: Option<CapOverflow>,

    /// Whether the quality governor's demotion has already been announced, so it
    /// is reported once as a **transition** rather than every frame after it
    /// (Plan 0044 Phase 2, the same shape as `reported_overflow` above). The
    /// demotion is one-way, so this only ever goes false -> true.
    pub(crate) reported_demotion: bool,
}

pub(crate) struct AppState {
    pub(crate) window: Arc<Window>,

    pub(crate) renderer: Renderer,

    pub(crate) occluded: bool,

    /// Whether the tier is pinned rather than engine-resolved — seeded from the
    /// launch pin (`--tier` / `RLX_TIER` / `[quality] tier`) and set by any
    /// explicit change. Tracked here rather than read off the renderer because
    /// the core exposes the *demotion* latch but not the pin, and widening the
    /// core's surface to render one menu suffix is the wrong trade.
    pub(crate) tier_pinned: bool,

    /// Operator config (display/fullscreen; grows in later phases) and where to
    /// persist it. `config_path` is `None` when the per-user dir can't be
    /// resolved — hotkey changes then apply live but don't persist.
    pub(crate) config: Config,

    pub(crate) config_path: Option<PathBuf>,

    /// Index (into the live monitor list) of the display the operator has
    /// selected — advanced by the `D` hotkey, used when going fullscreen.
    pub(crate) display_index: usize,

    /// Hands-off scene rotation policy (auto-rotate + drop bias); driven each
    /// visible frame with the injected `dt`.
    pub(crate) director: Director,

    /// Wall-clock time of the previous rendered frame, for measuring the `dt`
    /// fed to the director. Shell frame pacing only — core stays clock-free.
    pub(crate) last_frame: Instant,

    /// Wall-clock time of the previous left-button press, for detecting a
    /// double-click (fullscreen toggle). `None` until the first click.
    pub(crate) last_click: Option<Instant>,

    pub(crate) capture: Capture,

    pub(crate) presets: Presets,

    pub(crate) hud: Hud,

    pub(crate) diagnostics: Diagnostics,
}

/// The rotation config the director is built from, given the operator's
/// `[rotate]` block and whatever `--preset` held.
///
/// A held preset opts out of the dwell timer and changes nothing else: the
/// bounds stay the operator's, and `auto` is only ever narrowed — a config with
/// `auto = false` and no flag is already the answer (ADR-0155).
pub(crate) fn rotate_for(config: &config::Rotate, held_preset: Option<&str>) -> config::Rotate {
    config::Rotate {
        auto: config.auto && held_preset.is_none(),
        ..config.clone()
    }
}

impl AppState {
    /// Build the running show from the launch state [`App`] already resolved.
    ///
    /// Takes `&mut App` rather than the eleven values it holds: every one of
    /// them is read exactly once on the way into a field, so the options are
    /// **moved out** of the launch state here and `App` keeps only what the
    /// event loop still needs.
    pub(crate) fn new(window: Arc<Window>, app: &mut App, display_index: usize) -> Self {
        let config = std::mem::take(&mut app.config);
        let config_path = app.config_path.take();
        let soak_path = app.soak_path.take();
        let downbeat_log_path = app.downbeat_log_path.take();
        let tier = app.tier;
        let adapter = std::mem::take(&mut app.adapter);
        let held_preset = app.held_preset.take();
        let input = std::mem::take(&mut app.input);
        let osc = app.osc.take();
        let size = window.inner_size();
        let pinned = adapter != AdapterChoice::Default;
        let mut renderer = Renderer::new(
            Arc::clone(&window),
            size.width,
            size.height,
            // The window is the live path by definition, so the live sample
            // ceiling (ADR-0140) - spelled out rather than left to `..default()`
            // because this is the call site the choice is *about*.
            RendererOptions {
                tier,
                adapter,
                budget: rlx_core::render::SampleBudget::Live,
            },
        )
        .unwrap_or_else(|err| {
            eprintln!("renderer init failed: {err}");
            // An operator who named an adapter and did not get it must not be
            // left with a window on a different GPU, so the refusal is fatal
            // rather than a degrade (ADR-0155). Exit 1: the flag was recognized
            // and its effect failed, against 2 for an argument list wrong in
            // shape.
            std::process::exit(1);
        });
        // Say which tier the show is running at. The same preset looks different
        // on different machines now (ADR-0045), so this is the first line an odd
        // look should be checked against — and F3's overlay repeats it live.
        eprintln!("quality tier: {}", renderer.tier().as_str());

        // The governor measures against the *display's* frame budget, and a
        // refresh rate is a shell concern — the core never reads one itself. An
        // unreported rate leaves the core's 60 Hz default in place.
        if let Some(hz) = display_hz(&window) {
            renderer.set_display_hz(hz);
        }

        // Resolve the preset directory, seed the curated set into it on first
        // run (write-if-absent — but never into an RLX_PRESET_DIR override,
        // which is the user's own folder), then load it over the renderer's
        // embedded defaults and record the signature so later edits hot-reload.
        // Any failure degrades to the embedded defaults (NFR 10).
        let preset_dir = startup_preset_dir();
        reload_presets(&mut renderer, &preset_dir);
        let preset_sig = dir_signature(&preset_dir);

        // `--preset` holds one scene for the run. The name was checked against
        // this same roster before the window was created, so a miss here means
        // the directory changed underneath the launch; say so rather than
        // starting on an arbitrary scene (ADR-0155).
        if let Some(name) = held_preset.as_deref() {
            if renderer.select_preset_by_name(name) {
                eprintln!("preset: '{name}', held for the run - rotation is off");
            } else {
                // `rotate_for` has already turned the dwell timer off on the
                // strength of the flag, so the fallback is the startup scene
                // held for the run, not rotation. Hotkeys still move off it.
                eprintln!(
                    "--preset `{name}`: gone from the preset directory since startup; \
                     holding the startup scene instead - rotation is still off"
                );
            }
        }

        // Collect rolling frame-time stats from the first frame so the title
        // shows live fps/p99 (the overlay itself stays off until F3 — Plan 0011).
        renderer.enable_diagnostics(true);

        let capture = start_capture(&input);
        let capture_format = capture.format;
        let capture_endpoint = capture.endpoint.clone();
        // Rendered once, here, and only borrowed afterwards: the log's row builder
        // runs every frame and the overlay every frame it is up (Plan 0083).
        let capture_token = capture.verdict.token();
        let analyzer = Analyzer::new(capture.format)
            .expect("capture layer already validated this format at the boundary");

        // Frame pacing is a shell concern; the core stays clock-free (determinism).
        #[allow(
            clippy::disallowed_methods,
            reason = "preset-poll start; wall-clock pacing lives in the shell, not core analysis"
        )]
        let start = Instant::now();
        let renderer_overflow = renderer.cap_overflow().copied();
        let mut state = Self {
            window,
            renderer,
            occluded: false,
            tier_pinned: tier.is_some(),
            director: Director::from_config(&rotate_for(&config.rotate, held_preset.as_deref())),
            last_frame: start,
            last_click: None,
            config,
            config_path,
            display_index,
            capture: Capture {
                analyzer,
                consumer: capture.consumer,
                _capture: capture.handle,
                capture_token,
                capture_format,
                capture_endpoint,
                scratch: vec![0.0; 32_768],
                input,
                input_lost: false,
                input_recovery: RecoveryPolicy::default(),
                // Left empty until the settings modal is opened: a roster nothing is
                // reading is a COM enumeration paid for nothing, and every reader of
                // it is behind a keypress.
                input_roster: Vec::new(),
            },
            presets: Presets {
                preset_dir,
                preset_sig,
                last_preset_poll: start,
                pending_switch_settle: false,
            },
            hud: Hud {
                #[cfg(windows)]
                now_playing: nowplaying_win::NowPlayingSource::start(),
                browse: OverlayState::new(),
                settings: SettingsState::new(),
                console_window: None,
                console_cursor: (-1.0, -1.0),
                console_request: None,
                random_state: 0x9E37_79B9,
                frame_text: console::FrameText::default(),
                modal_scratch: Vec::new(),
                chrome_scratch: Vec::new(),
            },
            diagnostics: Diagnostics {
                title_tick: 0,
                overlay_on: false,
                diag_log: DiagLog::new(resolve_log_path()),
                soak: soak_path.map(SoakLog::new),
                downbeat_log: downbeat_log_path.map(DownbeatLog::new),
                osc,
                // Seeded from what `reload_presets` already printed above, so the
                // frame loop does not re-announce the startup preset's truncation.
                reported_overflow: renderer_overflow,
                reported_demotion: false,
            },
        };
        // **Which GPU is rendering the show**, once, at startup. Unflagged, the
        // window takes whatever wgpu returns for the surface, which on a hybrid
        // machine is not necessarily the discrete GPU; every frame-time figure
        // taken from this run is a property of that choice (ADR-0071). The line
        // names whether an operator pinned it, so a measurement can say which of
        // the two it is rather than leaving the reader to guess.
        state.diagnostics.diag_log.note(&format!(
            "renderer adapter: {}{}",
            state.renderer.adapter_description(),
            if pinned { " (pinned by --gpu)" } else { "" }
        ));
        state
    }

    /// Persist the current config to disk if a per-user path was resolved. A
    /// best-effort write — a failure is logged inside `Config::save`, never
    /// fatal to the running show.
    pub(crate) fn save_config(&self) {
        if let Some(path) = &self.config_path {
            self.config.save(path);
        }
    }

    /// Open or close the operator console (the `C` hotkey).
    pub(crate) fn toggle_console(&mut self, event_loop: &ActiveEventLoop, persist: bool) {
        if self.hud.console_window.is_some() {
            self.close_console();
        } else {
            self.open_console(event_loop);
        }
        if persist {
            // Persisted like fullscreen and the two `[hud]` switches: where the
            // operator left the console is a staging choice, not a debugging
            // state, so it survives the restart. Written from the **window**
            // rather than from the intent, so a console that refused to open
            // does not persist as on.
            self.config.console.enabled = self.hud.console_window.is_some();
            self.save_config();
        }
    }

    /// Service a console toggle asked for while no `ActiveEventLoop` was in
    /// scope — the settings row and the launch flag both take this path.
    pub(crate) fn service_console_request(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(persist) = self.hud.console_request.take() {
            self.toggle_console(event_loop, persist);
        }
    }

    /// Open the console on a display that is **not** the show's, when there is
    /// one; with a single monitor it opens as an ordinary window on that monitor,
    /// which is a supported mode rather than an error — a one-screen machine is
    /// where this gets developed and demonstrated.
    ///
    /// A surface the renderer's adapter cannot drive is not fatal: the window
    /// stays shut, the reason is logged once, and the show is untouched. That is
    /// the dual-GPU path, and it is built blind here — no CI runner and no
    /// single-GPU dev box can exercise it.
    pub(crate) fn open_console(&mut self, event_loop: &ActiveEventLoop) {
        let monitors: Vec<MonitorHandle> = self.window.available_monitors().collect();
        // The console's own `[console]` display, through the **same**
        // name-over-index rule the show's display uses. Where that resolves to
        // the screen the show is on, and there is another, take the other: a
        // console stacked on the output is the one configuration it exists to
        // avoid, and a default index cannot know which screen the show ended up
        // on. An explicit `display_name` is honoured either way — an operator
        // who named a screen meant it.
        let named = self.config.console.display_name.is_some();
        let resolved = resolve_display(
            &monitors,
            self.config.console.display_name.as_deref(),
            self.config.console.display,
        );
        let target = match resolved {
            Some((index, monitor)) if named || index != self.display_index => Some(monitor),
            Some(_) => monitors
                .iter()
                .enumerate()
                .find(|(i, _)| *i != self.display_index)
                .map(|(_, m)| m.clone()),
            None => None,
        };

        let mut attrs = Window::default_attributes()
            .with_title(CONSOLE_TITLE)
            .with_inner_size(winit::dpi::PhysicalSize::new(CONSOLE_WIDTH, CONSOLE_HEIGHT));
        if let Some(monitor) = target {
            // Position rather than fullscreen: the console is a desk-side
            // window an operator drags and resizes, not a second show surface.
            let origin = monitor.position();
            attrs = attrs.with_position(winit::dpi::PhysicalPosition::new(
                origin.x + CONSOLE_MARGIN,
                origin.y + CONSOLE_MARGIN,
            ));
        }

        let window = match event_loop.create_window(attrs) {
            Ok(window) => Arc::new(window),
            Err(err) => {
                eprintln!("could not open the operator console: {err}");
                return;
            }
        };
        let size = window.inner_size();
        match self
            .renderer
            .attach_aux(Arc::clone(&window), size.width, size.height)
        {
            Ok(mode) => {
                // The program preview, opened with the window that consumes it.
                // A refusal here is not fatal: the swapchain does not accept the
                // exact copy the preview rests on, so the console runs without a
                // picture rather than the show running through an inexact path.
                let preview = match self.renderer.open_preview() {
                    Ok(()) => "with preview",
                    Err(err) => {
                        self.diagnostics
                            .diag_log
                            .note(&format!("console preview unavailable, text only: {err}"));
                        "text only"
                    }
                };
                self.diagnostics.diag_log.note(&format!(
                    "console opened: {}x{}, present mode {}, {preview}",
                    size.width,
                    size.height,
                    mode.as_str()
                ));
                window.request_redraw();
                self.hud.console_window = Some(window);
            }
            Err(err) => {
                // The window is dropped here, so nothing is left on screen.
                self.diagnostics.diag_log.note(&format!(
                    "console surface unavailable on this adapter, staying closed: {err}"
                ));
            }
        }
    }

    /// Close the console and release its swapchain. Idempotent.
    pub(crate) fn close_console(&mut self) {
        if self.hud.console_window.take().is_some() {
            self.renderer.detach_aux();
            // Released with the window: while this is open the show is drawn
            // into an intermediate and copied out, so leaving it behind would
            // hold both the allocation and the extra copy for a console nobody
            // is looking at.
            self.renderer.close_preview();
            self.diagnostics.diag_log.note("console closed");
        }
    }

    /// Toggle borderless-fullscreen (the `F` hotkey). Going fullscreen targets
    /// the operator-selected display (falling back to the current/primary one);
    /// the new state and chosen monitor name are persisted so a restart matches.
    pub(crate) fn toggle_fullscreen(&mut self) {
        if self.window.fullscreen().is_some() {
            self.window.set_fullscreen(None);
            self.config.output.fullscreen = false;
        } else {
            let monitors: Vec<MonitorHandle> = self.window.available_monitors().collect();
            let monitor = monitors
                .get(self.display_index)
                .cloned()
                .or_else(|| self.window.current_monitor())
                .or_else(|| self.window.primary_monitor());
            if let Some(name) = monitor.as_ref().and_then(MonitorHandle::name) {
                self.config.output.display_name = Some(name);
            }
            // `Borderless(None)` means "the current monitor" — a safe fallback
            // when we couldn't resolve a specific handle.
            self.window
                .set_fullscreen(Some(Fullscreen::Borderless(monitor)));
            self.config.output.fullscreen = true;
        }
        self.refresh_display_hz();
        self.save_config();
        self.window.request_redraw();
    }

    /// Advance to the next display (the `D` hotkey): record it as the selected
    /// output, and — if currently fullscreen — move the fullscreen surface onto
    /// it immediately. Persists the new index + monitor name.
    pub(crate) fn cycle_display(&mut self) {
        let monitors: Vec<MonitorHandle> = self.window.available_monitors().collect();
        if monitors.is_empty() {
            return;
        }
        self.display_index = (self.display_index + 1) % monitors.len();
        let monitor = monitors.get(self.display_index).cloned();
        self.config.output.display = self.display_index;
        self.config.output.display_name = monitor.as_ref().and_then(MonitorHandle::name);
        if self.window.fullscreen().is_some() {
            self.window
                .set_fullscreen(Some(Fullscreen::Borderless(monitor)));
        }
        self.refresh_display_hz();
        self.save_config();
        self.window.request_redraw();
    }

    /// Re-read the display's refresh rate into the renderer's governor budget.
    ///
    /// Called wherever the window may have changed monitor. A stale budget in the
    /// *lenient* direction (60 Hz assumed on a 144 Hz panel) is harmless, but the
    /// strict direction — assuming 60 on a 30 Hz output — would demote a machine
    /// that was holding its actual rate perfectly well.
    pub(crate) fn refresh_display_hz(&mut self) {
        if let Some(hz) = display_hz(&self.window) {
            self.renderer.set_display_hz(hz);
        }
    }

    /// Report a **live** segment-cap truncation on entry, and its clearing once —
    /// never per frame.
    ///
    /// `Renderer::cap_overflow` covers both producers: the load-time L-system
    /// depth, which [`warn_cap_overflow`] already announces on a preset change,
    /// and the per-frame geometry mirror, which an audio-driven `mirror_order` can
    /// push over the cap at any moment. That second half was tracked by the core
    /// and never surfaced, because nothing read it between preset changes — the
    /// silent cut ADR-0007 forbids.
    ///
    /// **Edge-triggered on purpose.** `eprintln!` is file I/O on the render
    /// thread; doing it every frame for as long as a bound expression sits over
    /// the cap would be a worse bug than the gap it closes. The comparison is on
    /// *presence*, not on the dropped count, so a mirror order sweeping over the
    /// cap prints once rather than on every change of magnitude.
    pub(crate) fn poll_cap_overflow(&mut self) {
        let current = self.renderer.cap_overflow().copied();
        match (&self.diagnostics.reported_overflow, &current) {
            (None, Some(overflow)) => {
                eprintln!("preset '{}': {overflow}", self.renderer.preset_name());
            }
            (Some(_), None) => {
                eprintln!(
                    "preset '{}': geometry is back within the segment cap",
                    self.renderer.preset_name()
                );
            }
            // Still over (whatever the count now is), or still fine: say nothing.
            _ => {}
        }
        self.diagnostics.reported_overflow = current;
    }

    /// Announce a quality-tier demotion the first frame after it happens
    /// (ADR-0045: never silent). One-way, so this fires at most once a session
    /// and costs a bool read on every other frame.
    pub(crate) fn poll_tier_demotion(&mut self) {
        if self.renderer.tier_demoted() && !self.diagnostics.reported_demotion {
            self.diagnostics.reported_demotion = true;
            eprintln!(
                "quality tier demoted to {} -- the rich tier did not hold this \
                 display's frame budget. Pin it with --tier rich to override.",
                self.renderer.tier().as_str()
            );
            self.update_title();
        }
    }

    /// Re-scan the preset directory if the poll interval has elapsed and its
    /// signature changed, hot-reloading on any edit. Keeps the current set if
    /// the reload yields nothing valid (degrade, never crash — NFR 10).
    #[allow(
        clippy::disallowed_methods,
        reason = "preset-poll pacing reads the wall clock; core analysis stays clock-free"
    )]
    pub(crate) fn poll_presets(&mut self) {
        if self.presets.last_preset_poll.elapsed() < PRESET_POLL {
            return;
        }
        self.presets.last_preset_poll = Instant::now();
        let sig = dir_signature(&self.presets.preset_dir);
        if sig == self.presets.preset_sig {
            return;
        }
        self.presets.preset_sig = sig;
        reload_presets(&mut self.renderer, &self.presets.preset_dir);
        // `reload_presets` announced any truncation itself; re-baseline so the
        // frame loop reports only what changes from here.
        self.diagnostics.reported_overflow = self.renderer.cap_overflow().copied();
        // Keep the browse overlay's highlight valid if the roster just changed
        // shape under it (re-clamp; the open state and filter are preserved).
        let names = self.roster_names();
        let refs: Vec<&str> = names.iter().map(String::as_str).collect();
        self.hud.browse.on_roster_changed(&refs);
    }

    /// Drain whatever audio arrived since last frame into the analyzer.
    /// Runs even while hidden so visuals resume in sync.
    pub(crate) fn pump_audio(&mut self) {
        if let Some(consumer) = self.capture.consumer.as_mut() {
            loop {
                let n = consumer.pop_samples(&mut self.capture.scratch);
                if n == 0 {
                    break;
                }
                self.capture
                    .analyzer
                    .push_interleaved(&self.capture.scratch[..n]);
            }
        }
    }

    pub(crate) fn hidden(&self) -> bool {
        let size = self.window.inner_size();
        self.occluded || size.width == 0 || size.height == 0
    }

    pub(crate) fn redraw(&mut self) {
        // Measure wall-clock dt for the scene director and the recovery policy's
        // settle window (shell frame pacing; core analysis stays clock-free).
        // Update the marker even while hidden so the first visible frame after a
        // gap gets a small, clamped dt. It is the first thing the frame does
        // because the recovery poll below measures a duration, not a frame count.
        #[allow(
            clippy::disallowed_methods,
            reason = "shell frame pacing: measures dt for the scene director; core analysis stays clock-free"
        )]
        let now = Instant::now();
        let dt = now
            .duration_since(self.last_frame)
            .as_secs_f32()
            .min(MAX_DT);
        self.last_frame = now;

        self.poll_input_lost(dt);
        self.pump_audio();

        if self.hidden() {
            return;
        }
        self.poll_presets();
        let frame = self.capture.analyzer.take_frame();

        // Per-beat downbeat decomposition (opt-in, Plan 0086 Phase 1). Absent
        // unless `--downbeat-log` was passed; when present it returns on the
        // frames that carry no beat, so the per-frame cost is a bool test and the
        // terms are only recomputed on a beat. Reading them cannot change what
        // they say — `downbeat_terms` is `&self`, alloc-free and clock-free.
        if let Some(log) = self.diagnostics.downbeat_log.as_mut() {
            let analyzer = &self.capture.analyzer;
            log.maybe_log(&frame, || analyzer.downbeat_terms());
        }

        // Hands-off scene rotation: the director decides from dt + this frame's
        // energy whether to advance the preset (manual Space/A override it).
        if self.director.advance(dt, &frame).is_some() {
            self.rotate_to_next();
        }

        // Hand the core any track change the metadata source picked up since the
        // last frame (Plan 0097). A slot check, not a query: the WinRT event
        // threads do the work, so a frame with no change costs one uncontended
        // lock. The core ignores a string it already has, so this cannot
        // re-trigger the banner on its own.
        self.poll_now_playing();

        // Lighting telemetry (ADR-0144), opt-in. Sent **before** the render so
        // the datagrams leave ahead of the present's vsync wait rather than
        // behind it, which is the largest single latency this path can avoid.
        //
        // Two consequences of riding the rendered frame, both deliberate. A
        // preset switch dissolves, so `preset_name` still answers with the
        // outgoing preset for one frame after a switch (see
        // `pending_switch_settle`) - a 16 ms lag on a string that moves every
        // tens of seconds. And a hidden window returns above this point, so
        // telemetry stops with the picture; the sink follows what is drawn.
        if let Some(osc) = self.diagnostics.osc.as_mut() {
            let preset = self.renderer.preset_name();
            osc.send(
                now,
                &Telemetry {
                    bass: frame.bass,
                    mid: frame.mid,
                    treb: frame.treb,
                    onset: frame.onset,
                    rms: rms_of(&frame.waveform),
                    bass_raw: frame.bass_raw,
                    mid_raw: frame.mid_raw,
                    treb_raw: frame.treb_raw,
                    onset_raw: frame.onset_raw,
                    beat: frame.beat,
                    beat_index: frame.beat_index,
                    // `bar` is beat phase under a documented misnomer
                    // (ADR-0050); the wire uses the true name.
                    beat_phase: frame.bar,
                    tempo: frame.bpm,
                    preset,
                },
            );
        }

        // Queue the on-canvas text for this frame (active name + browse list).
        self.queue_frame_text();

        if let Err(err) = self.renderer.render(&frame, dt) {
            eprintln!("render error: {err}");
        }
        // After the show's present, never before it and never inside it: the
        // console is a monitor and must not delay the frame it reports on.
        self.present_console();
        // A dissolve's capture frame has now flipped the roster to the incoming
        // preset and applied its structural config, so this is the first moment the
        // renderer describes it rather than the one it is leaving (see
        // `pending_switch_settle`).
        if std::mem::take(&mut self.presets.pending_switch_settle) {
            warn_cap_overflow(&self.renderer);
            self.diagnostics.reported_overflow = self.renderer.cap_overflow().copied();
            self.update_title();
        }
        // The *live* half: a per-frame geometry-mirror overflow, which no
        // preset-change hook can see (ADR-0007 -- never a silent cut).
        self.poll_cap_overflow();
        self.poll_tier_demotion();
        self.diagnostics.title_tick += 1;
        if self.diagnostics.title_tick >= TITLE_UPDATE_FRAMES {
            self.diagnostics.title_tick = 0;
            self.update_title();
        }
        // Structured 1 Hz log (render thread). The analysis snapshot and RSS are
        // both read lazily, only on the seconds a sample is actually due — this
        // runs every frame.
        let metrics = self.renderer.metrics();
        let renderer = &self.renderer;
        self.diagnostics.diag_log.maybe_log(
            &metrics,
            || renderer.analysis_metrics(),
            rss::current_rss_bytes,
            &self.capture.capture_token,
        );
        // Long-run soak trace (opt-in). Absent unless `--soak` was passed, so the
        // normal loop is unaffected; when present it samples only every few
        // seconds, off the per-frame path.
        if let Some(soak) = self.diagnostics.soak.as_mut() {
            soak.maybe_sample(&metrics, rss::current_rss_bytes);
        }
        self.window.request_redraw();
    }

    /// Bookkeeping common to every preset switch — Space, a browse-overlay pick,
    /// and the director's auto-rotate.
    ///
    /// It defers rather than reports: a switch **dissolves** now (Plan 0023), and
    /// the roster does not reach the incoming preset until the dissolve's capture
    /// frame has rendered. Reading the title or the cap overflow here would describe
    /// the preset being left, so both wait one frame — see
    /// [`pending_switch_settle`](Presets::pending_switch_settle).
    pub(crate) fn on_preset_switched(&mut self) {
        self.presets.pending_switch_settle = true;
        self.note_soak_switch();
        self.window.request_redraw();
    }

    /// Advance to the next preset and record the switch.
    ///
    /// **The single path a rotation takes**, whether the director asked for it
    /// or the operator did. It exists because the two were open-coded and drifted:
    /// `on_preset_switched` is bookkeeping *about* a switch and performs none, so
    /// a caller that reached for it alone marked a rotation that never happened
    /// and the scene never changed. Pairing the two here is what makes that
    /// unrepresentable.
    pub(crate) fn rotate_to_next(&mut self) {
        self.renderer.cycle_preset();
        self.on_preset_switched();
    }

    /// Mark a GPU-resource rebuild in the soak log, if one is running
    /// (Plan 0085 Phase 3).
    ///
    /// Off the per-frame path by construction — the two callers are a preset
    /// switch and a surface reconfigure, both event-driven and both rare. The
    /// frame count comes from the core's own counter so the exclusion window is
    /// measured in the same units the frame-time ring is.
    pub(crate) fn note_soak_switch(&mut self) {
        if self.diagnostics.soak.is_none() {
            return;
        }
        let frames_total = self.renderer.metrics().frames_total;
        if let Some(soak) = self.diagnostics.soak.as_mut() {
            soak.note_switch(frames_total);
        }
    }

    /// Refresh the window title with the preset, system, and the core's
    /// diagnostics (fps + p99). No wall-clock read — the numbers come from the
    /// core's gated clock, the cadence from a frame counter.
    pub(crate) fn update_title(&mut self) {
        let m = self.renderer.metrics();
        let preset = self.renderer.preset_name();
        let system = self.renderer.active_system_name();
        let rotate = if self.director.auto_enabled() {
            "auto"
        } else {
            "manual"
        };
        // The tier is in the title because `[` / `]` move it live now (ADR-0054),
        // so it has to be visible without opening F3 to read the confirmation.
        let tier = self.renderer.tier().as_str();
        self.window.set_title(&format!(
            "{APP_TITLE} — {preset} [{system}] {rotate} {tier} — {:.0} fps  p99 {:.1} ms",
            m.fps, m.frame_ms_p99
        ));
    }

    /// Swap the quality tier on the running renderer (`[` / `]` and the settings
    /// menu's Quality row, ADR-0054).
    ///
    /// Asking for the tier it is already on still **pins** it and still persists
    /// the choice — that is the operator stating an intent — but skips the
    /// rebuild, because the core's entry point rebuilds unconditionally and
    /// restarting the trails for no change is a worse answer than doing nothing.
    ///
    /// The core clears its demotion latch on an explicit change, so this clears
    /// the shell's "already announced" latch alongside — otherwise a later real
    /// demotion would be silent, which ADR-0045 rules out.
    ///
    /// `--tier` and `RLX_TIER` still win at the next launch; this writes the
    /// `[quality] tier` they override, so the documented precedence is unchanged.
    pub(crate) fn swap_tier(&mut self, tier: Tier) {
        self.tier_pinned = true;
        self.config.quality.tier = match tier {
            Tier::Floor => config::TierChoice::Floor,
            Tier::Rich => config::TierChoice::Rich,
        };
        self.save_config();
        if self.renderer.tier() != tier {
            self.renderer.set_tier(tier);
            self.diagnostics.reported_demotion = false;
            eprintln!("quality tier: {} (pinned)", self.renderer.tier().as_str());
        }
        self.update_title();
        self.window.request_redraw();
    }

    /// The mode's active endpoints, re-enumerated into the cache.
    ///
    /// COM, so this is a keypress-driven call and never a per-frame one. A
    /// failed enumeration empties the roster rather than keeping a stale one:
    /// the `Input device` row then reports what is running and goes inert, which
    /// is honest, where stale names would offer endpoints that may be gone.
    #[cfg(windows)]
    pub(crate) fn refresh_input_roster(&mut self) {
        let mode = capture_mode(self.capture.input.mode);
        self.capture.input_roster.clear();
        match capture_win::endpoints(mode) {
            Ok(names) => {
                // `default` is a real, always-reachable choice — it is what an
                // unnamed selection means in `config.toml`, and it is where a
                // lost device recovers to — so it leads the roster rather than
                // being only spellable by editing the file.
                self.capture.input_roster.push(DEFAULT_ENDPOINT.to_owned());
                self.capture.input_roster.extend(names);
            }
            Err(err) => eprintln!(
                "could not enumerate {} endpoints: {err}",
                self.capture.input.mode.as_str()
            ),
        }
    }

    /// No endpoint enumeration on platforms whose capture path takes no
    /// selection; the two input rows are read-only there.
    #[cfg(not(windows))]
    pub(crate) fn refresh_input_roster(&mut self) {}

    /// Where the running endpoint sits in the cached roster.
    ///
    /// Positioned by the name the capture layer reports it **actually opened**,
    /// compared exactly: both strings come out of the same `endpoints()`
    /// enumeration, so exact is total here. Matching the *configured* name
    /// instead would need a second copy of `pick_device`'s rule — exact across
    /// every endpoint, and only then substring across every endpoint — and a
    /// copy spelled as a single per-element pass disagrees with it whenever one
    /// endpoint's name is a substring of another's, highlighting a row the
    /// stream is not on and cycling from the wrong place.
    ///
    /// An explicit `default` selection is the roster's leading slot whatever it
    /// resolved to, tested with the same rule `start_capture` uses to decide
    /// there is no name to match. A selection that named an absent endpoint has
    /// no such slot: it degraded to the default endpoint, and the row names
    /// that endpoint rather than the word.
    pub(crate) fn input_device_index(&self) -> usize {
        device_row_index(
            &self.capture.input_roster,
            &self.capture.input.device,
            self.capture.capture_endpoint.as_deref(),
        )
    }

    /// Stop the running capture stream and start one on `input`, in place.
    ///
    /// Synchronous on the render/UI thread, which is where every policy
    /// decision about capture lives (ADR-0142): dropping the handle joins the
    /// polling thread, and `start_capture` blocks until the new stream is live.
    /// That is a hitch of tens of milliseconds, paid on a keypress the operator
    /// just made.
    ///
    /// The [`Analyzer`] is rebuilt **only when the negotiated format moved**. A
    /// rebuild discards the AGC's running peak and the tempo history, so a swap
    /// between two endpoints that both negotiate 48 kHz keeps the picture
    /// adapted; a 48 -> 44.1 kHz swap cannot, and re-adapts over a second or two.
    ///
    /// A failed start is not an error path out of here: the handle and consumer
    /// are simply absent, the analyzer runs on `FALLBACK_FORMAT`, and the reason
    /// goes into the verdict — the same degradation a failed startup capture
    /// takes. A swap that fails must not be able to end the show.
    ///
    /// `persist` decides whether `config.toml` follows. See [`Persist`]: an
    /// operator's swap is a choice the file records, a recovery is not.
    pub(crate) fn restart_capture(&mut self, input: &config::Input, persist: Persist) {
        // Stop first. The old thread has to be joined before the new one exists
        // or the ring's single-producer invariant would briefly have two
        // claimants, and an endpoint is not reliably re-openable while a stream
        // still holds it.
        self.capture._capture = None;
        self.capture.consumer = None;

        let started = start_capture(input);
        self.capture.capture_token = started.verdict.token();
        if started.format != self.capture.capture_format {
            self.capture.analyzer = Analyzer::new(started.format)
                .expect("capture layer already validated this format at the boundary");
            self.capture.capture_format = started.format;
        }
        self.capture.consumer = started.consumer;
        self.capture._capture = started.handle;
        self.capture.capture_endpoint = started.endpoint;
        // A stream that is running is not a lost one, whoever asked for it. A
        // start that *failed* leaves the flag as it was, so a recovery keeps its
        // budget and a manual swap does not acquire one.
        if self.capture._capture.is_some() {
            self.capture.input_lost = false;
        }

        self.capture.input_recovery.on_restart(persist);

        // The running selection follows even when the start failed: it is what
        // was asked for, and the verdict is what says it is not delivering. The
        // two settings rows then disagree about it. `Input mode` reads the pick,
        // but a failed start leaves `capture_endpoint` at `None`, so
        // `device_row_index` falls to the roster's leading slot and
        // `Input device` reads `default` while `self.capture.input.device` still holds
        // the endpoint the operator named.
        self.capture.input = input.clone();
        if persist == Persist::Yes {
            self.config.input = input.clone();
            self.save_config();
        }
        eprintln!("audio input: {}", self.capture.capture_token);
        self.window.request_redraw();
    }

    /// Observe the capture thread's lost flag and act on the recovery policy.
    ///
    /// One relaxed atomic load per frame in the common case, which is what buys
    /// the whole mechanism. Recovery reopens the mode's **default** endpoint
    /// rather than the named one: the named one is the device that just went
    /// away, and asking for it again is the one request guaranteed to fail.
    ///
    /// The reopen is [`Persist::No`]. The operator did not choose the default
    /// endpoint, and since a re-plug deliberately does not restore the device
    /// (ADR-0142 Alternative D), `[input] device` in the file is the only record
    /// of which endpoint the rig wants — a recovery that wrote over it would
    /// turn a pulled cable into a permanently changed configuration.
    ///
    /// Called from `redraw`, so the flag is observed at frame cadence: while the
    /// window is occluded or minimized nothing redraws and a loss is not seen
    /// until it comes back. Accepted — there is no show to interrupt behind a
    /// hidden window.
    pub(crate) fn poll_input_lost(&mut self, dt: f32) {
        if capture_lost(self.capture._capture.as_ref()) {
            self.capture.input_lost = true;
        }
        match self
            .capture
            .input_recovery
            .poll(self.capture.input_lost, dt)
        {
            Recovery::Hold => {}
            Recovery::Reopen(attempt) => {
                eprintln!(
                    "audio input lost; reopening the default {} endpoint \
                     (attempt {attempt} of {INPUT_RECOVERY_ATTEMPTS})",
                    self.capture.input.mode.as_str()
                );
                self.restart_capture(
                    &config::Input {
                        mode: self.capture.input.mode,
                        device: DEFAULT_ENDPOINT.to_owned(),
                    },
                    Persist::No,
                );
            }
            Recovery::GiveUp => {
                self.capture.capture_token = CaptureVerdict::Lost {
                    backend: CAPTURE_BACKEND,
                    attempts: INPUT_RECOVERY_ATTEMPTS,
                }
                .token();
                eprintln!(
                    "audio input not recovered after {INPUT_RECOVERY_ATTEMPTS} attempts; \
                     rendering without audio until the input is set again"
                );
            }
        }
    }

    /// Switch the capture path (the `Input mode` row).
    ///
    /// The device name goes back to `default`, because a friendly name belongs
    /// to one dataflow: a render endpoint's name matches nothing among capture
    /// endpoints, so carrying it across would resolve to the default anyway and
    /// leave the row naming an endpoint that is not running.
    pub(crate) fn set_input_mode(&mut self, mode: config::InputMode) {
        if self.capture.input.mode == mode {
            // Idempotent, so holding the key does not restart capture per repeat.
            return;
        }
        self.restart_capture(
            &config::Input {
                mode,
                device: DEFAULT_ENDPOINT.to_owned(),
            },
            Persist::Yes,
        );
        // The other dataflow has its own endpoints; the row would otherwise
        // cycle the previous mode's list.
        self.refresh_input_roster();
    }

    /// Advance to the next endpoint in the cached roster, **wrapping** — the
    /// `Input device` row, and the same end-of-list rule `cycle_display` uses.
    pub(crate) fn cycle_input_device(&mut self) {
        if self.capture.input_roster.is_empty() {
            return;
        }
        let next = (self.input_device_index() + 1) % self.capture.input_roster.len();
        self.restart_capture(
            &config::Input {
                mode: self.capture.input.mode,
                device: self.capture.input_roster[next].clone(),
            },
            Persist::Yes,
        );
    }

    /// Toggle auto-rotate and **persist it** — the one path for both the `A`
    /// hotkey and the settings row.
    ///
    /// A hotkey that changes the director and prints to stderr without
    /// writing `[rotate] auto` — unlike `F` and `D`, which both persist
    /// — leaves the two controls able to disagree: set it with `A`,
    /// restart, and the config's value comes back. One path is what
    /// makes that impossible rather than merely fixed.
    pub(crate) fn toggle_auto_rotate(&mut self) {
        let on = self.director.toggle_auto();
        self.config.rotate.auto = on;
        self.save_config();
        eprintln!("auto-rotate {}", if on { "on" } else { "off" });
        self.update_title();
        self.window.request_redraw();
    }

    /// Toggle the diagnostics overlay (`F3` and the settings row).
    ///
    /// **Deliberately not persisted.** It is a debugging state, and a live show
    /// that comes up with the overlay painted because someone pressed `F3` last
    /// week is a worse default than pressing `F3` again.
    pub(crate) fn toggle_diagnostics(&mut self) {
        self.diagnostics.overlay_on = !self.diagnostics.overlay_on;
        self.renderer.set_overlay(self.diagnostics.overlay_on);
        self.window.request_redraw();
    }

    /// The live values the settings rows show, gathered fresh each time they are
    /// drawn or edited — which is what lets [`SettingsState`] hold none of them.
    pub(crate) fn settings_view(&self) -> SettingsView {
        let monitors: Vec<MonitorHandle> = self.window.available_monitors().collect();
        let display_name = monitors
            .get(self.display_index)
            .and_then(MonitorHandle::name)
            .or_else(|| self.config.output.display_name.clone())
            .unwrap_or_else(|| "unknown".to_owned());
        SettingsView {
            console: self.hud.console_window.is_some(),
            tier: self.renderer.tier(),
            // Demotion wins over the pin: the governor only demotes an *unpinned*
            // session, so the two cannot both be true, and reporting a demotion
            // is the case ADR-0045 will not let go silent.
            tier_state: if self.renderer.tier_demoted() {
                TierState::Demoted
            } else if self.tier_pinned {
                TierState::Pinned
            } else {
                TierState::Auto
            },
            auto_rotate: self.director.auto_enabled(),
            min_dwell_secs: self.config.rotate.min_dwell_secs,
            max_dwell_secs: self.config.rotate.max_dwell_secs,
            fullscreen: self.window.fullscreen().is_some(),
            display_index: self.display_index,
            display_count: monitors.len(),
            display_name,
            diagnostics: self.diagnostics.overlay_on,
            input_mode: self.capture.input.mode,
            // Read off the cache, never enumerated here: this runs every frame
            // the modal is up.
            input_device_index: self.input_device_index(),
            input_device_count: self.capture.input_roster.len(),
            input_device_name: self
                .capture
                .input_roster
                .get(self.input_device_index())
                .cloned()
                .unwrap_or_else(|| self.capture.input.device.clone()),
            // Windows is the only platform whose capture path takes a selection;
            // elsewhere the rows render and do not move.
            input_editable: cfg!(windows),
            preset_name: self.config.hud.preset_name,
            now_playing: self.config.hud.now_playing,
            preset_dir: self.presets.preset_dir.display().to_string(),
        }
    }

    /// Carry out what the settings modal asked for. The modal decides *what*
    /// changes; every effect — the renderer, the window, the director, the config
    /// file — happens here.
    pub(crate) fn apply_settings_action(&mut self, action: SettingsAction) {
        match action {
            SettingsAction::None => return,
            SettingsAction::Redraw | SettingsAction::Close => {}
            SettingsAction::OpenBrowse => {
                self.hud.settings.close();
                self.open_browse();
            }
            SettingsAction::SetTier(tier) => self.swap_tier(tier),
            SettingsAction::ToggleAuto => self.toggle_auto_rotate(),
            SettingsAction::SetDwell { min_secs, max_secs } => {
                self.config.rotate.min_dwell_secs = min_secs;
                self.config.rotate.max_dwell_secs = max_secs;
                // The live director, not a rebuilt one: a rebuild would reset the
                // dwell clock under the operator's hand.
                self.director.set_dwell_bounds(min_secs, max_secs);
                self.save_config();
            }
            // Deferred, not done here: creating a window needs an
            // `ActiveEventLoop`, which this applier is not given. The flag is
            // serviced in `window_event`, where one is in scope.
            //
            // **Closing the console must not close the menu that asked.** Only
            // the window goes; `settings` is untouched, so the next frame's
            // routing — which sends modal lines to the output whenever no
            // console is open — draws the same menu on the show. That is the
            // one interaction where the Phase 3 move has to reverse
            // mid-keystroke, and it reverses by construction rather than by a
            // special case.
            SettingsAction::ToggleConsole => self.hud.console_request = Some(true),
            SettingsAction::ToggleFullscreen => self.toggle_fullscreen(),
            SettingsAction::CycleDisplay => self.cycle_display(),
            SettingsAction::ToggleDiagnostics => self.toggle_diagnostics(),
            SettingsAction::SetInputMode(mode) => self.set_input_mode(mode),
            SettingsAction::CycleInputDevice => self.cycle_input_device(),
            // Persisted, unlike diagnostics: a clean canvas is a staging choice,
            // not a debugging state, so it should survive the restart.
            SettingsAction::TogglePresetName => {
                self.config.hud.preset_name = !self.config.hud.preset_name;
                self.save_config();
            }
            // Persisted for the same reason, but with one extra effect: the
            // banner is core-owned and may be mid-envelope right now, so turning
            // it off clears the string rather than waiting out the fade. The
            // operator's "off" has to take the canvas back immediately.
            SettingsAction::ToggleNowPlaying => {
                self.config.hud.now_playing = !self.config.hud.now_playing;
                if !self.config.hud.now_playing {
                    self.renderer.set_now_playing("");
                }
                self.save_config();
            }
        }
        self.window.request_redraw();
    }

    /// Open the browse overlay on the active preset, as `Tab` does.
    pub(crate) fn open_browse(&mut self) {
        let names = self.roster_names();
        let refs: Vec<&str> = names.iter().map(String::as_str).collect();
        let active = self.renderer.active_index();
        let layout = self.list_layout(refs.len());
        self.hud
            .browse
            .handle_key(OverlayKey::Toggle, &refs, active, &layout);
    }

    /// Push any track change the metadata source picked up into the core's
    /// banner (Plan 0097). The source runs on its own thread and leaves the
    /// newest string in a slot; this takes it. The **only** place a WinRT-sourced
    /// string reaches the renderer, so nothing calls into the core from a
    /// callback thread.
    #[cfg(windows)]
    pub(crate) fn poll_now_playing(&mut self) {
        // Drained even when the operator has it off, rather than skipped: a
        // string left in the slot would announce a track that changed minutes
        // ago the moment the row is switched back on. Off means the *next*
        // change is the first one drawn.
        let Some(track) = self.hud.now_playing.take_change() else {
            return;
        };
        if self.config.hud.now_playing {
            self.renderer.set_now_playing(&track);
        }
    }

    /// No metadata source outside Windows — `MediaRemote` is private and
    /// restricted (ADR-0110), so the Mac path's answer is the foobar plugin.
    #[cfg(not(windows))]
    pub(crate) fn poll_now_playing(&mut self) {}

    /// The current roster names, owned — so a caller can borrow `&mut` the
    /// renderer afterward without holding a live borrow of the preset list.
    pub(crate) fn roster_names(&self) -> Vec<String> {
        self.renderer.preset_names().map(str::to_owned).collect()
    }
}

/// The refresh rate of the monitor this window is on, in Hz, or `None` when winit
/// reports none — which is common on a virtual or remote display. The governor's
/// budget falls back to 60 Hz in that case.
pub(crate) fn display_hz(window: &Window) -> Option<f32> {
    let millihertz = window
        .current_monitor()
        .or_else(|| window.primary_monitor())
        .and_then(|m| m.refresh_rate_millihertz())?;
    Some(millihertz as f32 / 1000.0)
}

/// Surface a line scene's segment-cap truncation to stderr (ADR-0007: the cap
/// is never a silent cut). A no-op in the common case where the active preset's
/// geometry fit within the cap. Called after every active-preset change.
pub(crate) fn warn_cap_overflow(renderer: &Renderer) {
    if let Some(overflow) = renderer.cap_overflow() {
        eprintln!("preset '{}': {overflow}", renderer.preset_name());
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::rotate_for;
    use crate::config;

    /// **A held preset turns the dwell timer off, and only a held one does.**
    /// Rotation is the operator's config in every other case, including a
    /// config that already has `auto = false` — `--preset` narrows the setting
    /// and never widens it — and the dwell bounds stay theirs either way.
    #[test]
    fn a_held_preset_is_the_only_thing_that_disables_rotation() {
        let on = config::Rotate {
            auto: true,
            ..config::Rotate::default()
        };
        let off = config::Rotate {
            auto: false,
            ..config::Rotate::default()
        };

        assert!(
            rotate_for(&on, None).auto,
            "no flag leaves the config alone"
        );
        assert!(
            !rotate_for(&on, Some("attractor_clifford")).auto,
            "a held preset turns rotation off"
        );
        assert!(
            !rotate_for(&off, None).auto,
            "a config with rotation already off stays off"
        );
        assert_eq!(
            rotate_for(&on, Some("attractor_clifford")).min_dwell_secs,
            on.min_dwell_secs,
            "the dwell bounds stay the operator's even when rotation is off"
        );
    }
}
