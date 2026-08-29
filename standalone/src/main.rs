#[cfg(target_os = "macos")]
mod capture_mac;
mod capture_verdict;
#[cfg(windows)]
mod capture_win;
mod config;
mod console;
mod diaglog;
mod director;
mod downbeatlog;
// Windows-only: the standalone's now-playing source (Plan 0097 / ADR-0110).
// macOS has no supported equivalent, so the banner exists there and is simply
// never fed — the same asymmetry loopback capture already has.
#[cfg(windows)]
mod nowplaying_win;
mod overlay;
mod settings;
mod soak;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use capture_verdict::CaptureVerdict;
use config::Config;
use diaglog::DiagLog;
use director::Director;
use downbeatlog::DownbeatLog;
use lmv_core::audio::{AudioFormat, SampleConsumer};
use lmv_core::dsp::Analyzer;
use lmv_core::render::{CapOverflow, Renderer, RendererOptions, Tier};
use overlay::{LIST_INSET, LIST_TOP, OverlayAction, OverlayKey, OverlayState, ROW_H, ROW_SIZE};
use settings::{SettingsAction, SettingsKey, SettingsState, SettingsView, TierState};
use soak::SoakLog;
use standalone::osc::{OscSink, Telemetry, rms_of};
use standalone::{
    APP_DIR_NAME, PRESET_DIR_ENV, PresetDir, preset_data_root, resolve_preset_dir, resolve_tier,
    rss, tier_env,
};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, KeyEvent, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::monitor::MonitorHandle;
use winit::window::{Fullscreen, Window, WindowId};

/// How often the render loop wakes to keep DSP fed while hidden (NFR 1:
/// near-zero GPU in the background, analysis stays warm).
const HIDDEN_TICK: Duration = Duration::from_millis(100);

/// Window-title prefix: app name plus the application version. `CARGO_PKG_VERSION`
/// resolves at compile time to the single [workspace.package].version (ADR-0005).
const APP_TITLE: &str = concat!("light-music-visualizer ", env!("CARGO_PKG_VERSION"));

/// How often to re-scan the preset directory for edits. Tight enough that an
/// edit to a `.toml` reads as immediate while authoring (ADR-0014); the scan
/// itself is a `read_dir` + mtime pass, negligible beside a rendered frame.
const PRESET_POLL: Duration = Duration::from_millis(150);
/// Clamp the per-frame `dt` fed to the scene director. A long hidden/paused gap
/// would otherwise dump a huge step into the dwell timer and rotate on return.
const MAX_DT: f32 = 0.25;
/// Refresh the window title (fps + p99) every this many rendered frames — a
/// frame-count cadence keeps the shell clock-free for the title; the numbers
/// themselves come from the core's diagnostics.
const TITLE_UPDATE_FRAMES: u32 = 30;
/// Max gap between two left-button presses for them to count as a double-click
/// (the common OS default). winit has no native double-click event, so the
/// shell times consecutive presses itself.
const DOUBLE_CLICK: Duration = Duration::from_millis(400);

/// On-canvas active-preset-name label: top-left inset (device px), font size,
/// and a light near-white color legible over most scenes.
/// The operator console's window title, so it is tellable from the show's in a
/// taskbar and by a window manager.
const CONSOLE_TITLE: &str = concat!("lmv console ", env!("CARGO_PKG_VERSION"));
/// The console's default size. Wide enough for the browser's multi-column list
/// at its current column width, short enough to sit beside other desk windows.
const CONSOLE_WIDTH: u32 = 900;
const CONSOLE_HEIGHT: u32 = 640;
/// Inset from the chosen monitor's top-left, so the console does not open flush
/// into a corner under a taskbar.
const CONSOLE_MARGIN: i32 = 64;

const NAME_INSET: f32 = 16.0;
const NAME_SIZE: f32 = 28.0;
const NAME_COLOR: [f32; 4] = [0.9, 0.95, 1.0, 1.0];

/// Browse-overlay row colors. The **geometry** (insets, pitch, font size, column
/// width) lives in [`overlay`] beside the pure layout function that reasons about
/// it, so the pixels drawn here and the arithmetic tested there cannot drift.
const ROW_COLOR: [f32; 4] = [0.72, 0.78, 0.88, 0.95];
const ROW_HL_COLOR: [f32; 4] = [1.0, 0.88, 0.35, 1.0];
/// The filter-echo header sits above the list; dimmer than the rows.
const HEADER_COLOR: [f32; 4] = [0.6, 0.66, 0.76, 0.9];

struct AppState {
    window: Arc<Window>,
    renderer: Renderer,
    analyzer: Analyzer,
    consumer: Option<SampleConsumer>,
    // Held for its Drop: stops the capture thread with the app.
    _capture: Option<capture_handle::Handle>,
    /// The capture verdict of the stream running *now*, already rendered to its
    /// one-line token. Rendered in one place and stored, so the two durable
    /// surfaces — the `diagnostics.log` `capture` column and the F3 overlay —
    /// borrow the same string and cannot disagree; re-rendered on every swap, so
    /// the log answers what capture is listening to rather than what it started
    /// on (ADR-0142).
    capture_token: String,
    /// The format the live [`AppState::analyzer`] was built on. Kept beside the
    /// analyzer because a swap rebuilds it only when the negotiated format
    /// actually moved — an unchanged format keeps the AGC's running peak and the
    /// tempo history, which is most swaps between two 48 kHz endpoints.
    capture_format: AudioFormat,
    /// Friendly name of the endpoint the running stream actually opened, as the
    /// capture layer resolved it — `None` when nothing is running, or on a
    /// platform whose capture path selects no endpoint.
    ///
    /// This is what positions the `Input device` row. Deriving that position by
    /// matching the *configured* name against the roster instead needs a second
    /// implementation of `pick_device`'s rule, and a second one spelled
    /// differently disagrees whenever one endpoint's name is a substring of
    /// another's.
    capture_endpoint: Option<String>,
    scratch: Vec<f32>,
    occluded: bool,
    /// Frames since the last title refresh (title shows core-sourced fps + p99).
    title_tick: u32,
    /// Whether the diagnostics debug overlay is currently painted (toggled by F3).
    overlay_on: bool,
    /// The now-playing metadata source, watching SMTC on its own thread
    /// (Plan 0097 Phase 2). Windows-only; absent everywhere else, where the
    /// banner simply never fires.
    #[cfg(windows)]
    now_playing: nowplaying_win::NowPlayingSource,
    /// The preset browse overlay's modal state (Tab toggles; Plan 0008).
    browse: OverlayState,
    /// The settings modal's state (`S` toggles; Plan 0050 Phase 4). A second,
    /// independent pure state machine — see [`settings`] for why it is not the
    /// same one.
    settings: SettingsState,
    /// The operator console's window, `None` while it is closed (ADR-0143).
    ///
    /// The window is the whole of the console's shell-side state: there is no
    /// second `Renderer`, no second scene clock and no second modal state
    /// machine. What the console *shows* is decided every frame by
    /// [`console::route`], from the same lines the output would have drawn.
    console_window: Option<Arc<Window>>,
    /// This frame's text, split by destination and retained across frames so the
    /// split reuses its buffers rather than allocating two vectors per frame.
    frame_text: console::FrameText,
    /// Scratch for the frame's modal rows, before routing moves them into
    /// [`AppState::frame_text`]. Retained for its capacity.
    modal_scratch: Vec<console::Line>,
    /// Whether the tier is pinned rather than engine-resolved — seeded from the
    /// launch pin (`--tier` / `LMV_TIER` / `[quality] tier`) and set by any
    /// explicit change. Tracked here rather than read off the renderer because
    /// the core exposes the *demotion* latch but not the pin, and widening the
    /// core's surface to render one menu suffix is the wrong trade.
    tier_pinned: bool,
    /// ~1 Hz structured diagnostics logger (render thread only).
    diag_log: DiagLog,
    /// Preset directory watched for hot-reload, with its last-seen signature
    /// and poll deadline.
    preset_dir: PathBuf,
    preset_sig: Option<(u128, usize)>,
    last_preset_poll: Instant,
    /// Operator config (display/fullscreen; grows in later phases) and where to
    /// persist it. `config_path` is `None` when the per-user dir can't be
    /// resolved — hotkey changes then apply live but don't persist.
    config: Config,
    config_path: Option<PathBuf>,
    /// The capture selection currently running — resolved at launch across
    /// `--input` / `--device` / `[input]`, and moved by the settings rows.
    /// Held apart from `config.input` because a launch flag pins the run
    /// without writing itself into the file the operator keeps.
    input: config::Input,
    /// Whether the running capture stream has reported itself dead and has not
    /// been replaced by a live one since.
    ///
    /// Sticky across a failed reopen on purpose: the capture thread's flag goes
    /// away with the handle, so a recovery attempt that fails would otherwise
    /// read as "nothing wrong" on the next frame and spend the whole retry
    /// budget on its first attempt.
    input_lost: bool,
    /// How many reopens the lost input has already cost, and whether the bound
    /// has been announced.
    input_recovery: RecoveryPolicy,
    /// The active endpoints of [`Self::input`]'s mode, `default` first, as the
    /// `Input device` row cycles them.
    ///
    /// **Cached, and refreshed only when the settings modal opens and when the
    /// mode changes.** Enumeration is COM — it allocates and blocks — and
    /// [`AppState::settings_view`] runs every frame the modal is up, so
    /// enumerating there would put a blocking COM call on the render thread once
    /// a frame. The cost of caching is that a device appearing or disappearing
    /// while the menu is open is not seen until it is reopened.
    input_roster: Vec<String>,
    /// Index (into the live monitor list) of the display the operator has
    /// selected — advanced by the `D` hotkey, used when going fullscreen.
    display_index: usize,
    /// Hands-off scene rotation policy (auto-rotate + drop bias); driven each
    /// visible frame with the injected `dt`.
    director: Director,
    /// Wall-clock time of the previous rendered frame, for measuring the `dt`
    /// fed to the director. Shell frame pacing only — core stays clock-free.
    last_frame: Instant,
    /// Long-run soak sampler, present only with `--soak` (else the render loop
    /// is byte-unchanged).
    soak: Option<SoakLog>,
    /// Per-beat downbeat decomposition log, present only with `--downbeat-log`
    /// (Plan 0086 Phase 1). Absent otherwise, so the frame path is unchanged.
    downbeat_log: Option<DownbeatLog>,
    /// Lighting telemetry sink, present only when `--osc` or `[osc] enabled`
    /// turned it on (ADR-0144). Absent otherwise, so the frame path is a `None`
    /// test and no socket is bound.
    osc: Option<OscSink>,
    /// Wall-clock time of the previous left-button press, for detecting a
    /// double-click (fullscreen toggle). `None` until the first click.
    last_click: Option<Instant>,
    /// Set at a preset switch, consumed after the next rendered frame: a switch
    /// now **dissolves** (Plan 0023), so the roster does not reach the incoming
    /// preset until that frame's capture step has run. Everything that describes
    /// the active preset therefore still answers with the *outgoing* one at the
    /// switch site — the window title, and (the one that does not self-correct)
    /// its segment-cap truncation, which does not even exist until the incoming
    /// preset's structural config is applied at the flip. ADR-0007 says the cap is
    /// never a silent cut, so the check waits for the frame that makes it real.
    pending_switch_settle: bool,
    /// The segment-cap truncation already announced on stderr, so
    /// [`poll_cap_overflow`](AppState::poll_cap_overflow) reports the **transition**
    /// rather than the state (Plan 0031 Phase 6).
    ///
    /// The load-time half of the cap (an oversized L-system depth) is announced by
    /// [`warn_cap_overflow`] when the preset changes. The **per-frame** half — a
    /// geometry mirror an audio expression drives over the cap — was tracked by the
    /// core and never reported, because the only reader ran on a preset change.
    /// This field is what lets the frame loop report it without shouting.
    reported_overflow: Option<CapOverflow>,
    /// Whether the quality governor's demotion has already been announced, so it
    /// is reported once as a **transition** rather than every frame after it
    /// (Plan 0044 Phase 2, the same shape as `reported_overflow` above). The
    /// demotion is one-way, so this only ever goes false -> true.
    reported_demotion: bool,
    /// Retained scratch for [`AppState::queue_frame_text`], cleared at entry
    /// rather than reallocated (Plan 0061 Phase 5).
    ///
    /// `text_layer.end_frame()` clears the text queue every frame, so the runs
    /// have to be re-queued every frame — an early return is **not** the fix
    /// here, reuse is. Holding the two vectors on the state means a steady-state
    /// frame allocates only when the content grows past the retained capacity.
    /// The show's own furniture for this frame — corner preset name, capture
    /// verdict — before routing moves it onto the output.
    chrome_scratch: Vec<console::Line>,
}

/// Narrow alias so the non-Windows build (no capture until Phase 9) compiles
/// the same struct shape.
mod capture_handle {
    #[cfg(target_os = "macos")]
    pub type Handle = crate::capture_mac::CaptureHandle;
    #[cfg(windows)]
    pub type Handle = crate::capture_win::CaptureHandle;
    #[cfg(not(any(windows, target_os = "macos")))]
    pub type Handle = ();
}

impl AppState {
    #[allow(
        clippy::too_many_arguments,
        reason = "the shell's startup inputs, each read once on the way into one of the state's fields"
    )]
    fn new(
        window: Arc<Window>,
        config: Config,
        config_path: Option<PathBuf>,
        display_index: usize,
        soak_path: Option<PathBuf>,
        downbeat_log_path: Option<PathBuf>,
        tier: Option<Tier>,
        input: config::Input,
        osc: Option<OscSink>,
    ) -> Self {
        let size = window.inner_size();
        let mut renderer = Renderer::new(
            Arc::clone(&window),
            size.width,
            size.height,
            RendererOptions { tier },
        )
        .unwrap_or_else(|err| {
            eprintln!("renderer init failed: {err}");
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
        // run (write-if-absent — but never into an LMV_PRESET_DIR override,
        // which is the user's own folder), then load it over the renderer's
        // embedded defaults and record the signature so later edits hot-reload.
        // Any failure degrades to the embedded defaults (NFR 10).
        let preset_dir = startup_preset_dir();
        reload_presets(&mut renderer, &preset_dir);
        let preset_sig = dir_signature(&preset_dir);

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
        Self {
            window,
            renderer,
            analyzer,
            consumer: capture.consumer,
            _capture: capture.handle,
            capture_token,
            capture_format,
            capture_endpoint,
            scratch: vec![0.0; 32_768],
            occluded: false,
            title_tick: 0,
            overlay_on: false,
            #[cfg(windows)]
            now_playing: nowplaying_win::NowPlayingSource::start(),
            browse: OverlayState::new(),
            settings: SettingsState::new(),
            tier_pinned: tier.is_some(),
            diag_log: DiagLog::new(resolve_log_path()),
            preset_dir,
            preset_sig,
            last_preset_poll: start,
            director: Director::from_config(&config.rotate),
            last_frame: start,
            soak: soak_path.map(SoakLog::new),
            downbeat_log: downbeat_log_path.map(DownbeatLog::new),
            osc,
            last_click: None,
            pending_switch_settle: false,
            // Seeded from what `reload_presets` already printed above, so the
            // frame loop does not re-announce the startup preset's truncation.
            reported_overflow: renderer_overflow,
            reported_demotion: false,
            console_window: None,
            frame_text: console::FrameText::default(),
            modal_scratch: Vec::new(),
            chrome_scratch: Vec::new(),
            config,
            config_path,
            input,
            input_lost: false,
            input_recovery: RecoveryPolicy::default(),
            // Left empty until the settings modal is opened: a roster nothing is
            // reading is a COM enumeration paid for nothing, and every reader of
            // it is behind a keypress.
            input_roster: Vec::new(),
            display_index,
        }
    }

    /// Persist the current config to disk if a per-user path was resolved. A
    /// best-effort write — a failure is logged inside `Config::save`, never
    /// fatal to the running show.
    fn save_config(&self) {
        if let Some(path) = &self.config_path {
            self.config.save(path);
        }
    }

    /// Open or close the operator console (the `C` hotkey).
    fn toggle_console(&mut self, event_loop: &ActiveEventLoop) {
        if self.console_window.is_some() {
            self.close_console();
        } else {
            self.open_console(event_loop);
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
    fn open_console(&mut self, event_loop: &ActiveEventLoop) {
        let monitors: Vec<MonitorHandle> = self.window.available_monitors().collect();
        // Not the show's monitor, when the machine has another. `display_index`
        // is the output's own resolved position, so this asks the same question
        // `resolve_monitor` answered rather than a second one spelled
        // differently.
        let target = monitors
            .iter()
            .enumerate()
            .find(|(i, _)| *i != self.display_index)
            .map(|(_, m)| m.clone());

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
                self.diag_log.note(&format!(
                    "console opened: {}x{}, present mode {}",
                    size.width,
                    size.height,
                    mode.as_str()
                ));
                window.request_redraw();
                self.console_window = Some(window);
            }
            Err(err) => {
                // The window is dropped here, so nothing is left on screen.
                self.diag_log.note(&format!(
                    "console surface unavailable on this adapter, staying closed: {err}"
                ));
            }
        }
    }

    /// Close the console and release its swapchain. Idempotent.
    fn close_console(&mut self) {
        if self.console_window.take().is_some() {
            self.renderer.detach_aux();
            self.diag_log.note("console closed");
        }
    }

    /// Toggle borderless-fullscreen (the `F` hotkey). Going fullscreen targets
    /// the operator-selected display (falling back to the current/primary one);
    /// the new state and chosen monitor name are persisted so a restart matches.
    fn toggle_fullscreen(&mut self) {
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
    fn cycle_display(&mut self) {
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
    fn refresh_display_hz(&mut self) {
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
    fn poll_cap_overflow(&mut self) {
        let current = self.renderer.cap_overflow().copied();
        match (&self.reported_overflow, &current) {
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
        self.reported_overflow = current;
    }

    /// Announce a quality-tier demotion the first frame after it happens
    /// (ADR-0045: never silent). One-way, so this fires at most once a session
    /// and costs a bool read on every other frame.
    fn poll_tier_demotion(&mut self) {
        if self.renderer.tier_demoted() && !self.reported_demotion {
            self.reported_demotion = true;
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
    fn poll_presets(&mut self) {
        if self.last_preset_poll.elapsed() < PRESET_POLL {
            return;
        }
        self.last_preset_poll = Instant::now();
        let sig = dir_signature(&self.preset_dir);
        if sig == self.preset_sig {
            return;
        }
        self.preset_sig = sig;
        reload_presets(&mut self.renderer, &self.preset_dir);
        // `reload_presets` announced any truncation itself; re-baseline so the
        // frame loop reports only what changes from here.
        self.reported_overflow = self.renderer.cap_overflow().copied();
        // Keep the browse overlay's highlight valid if the roster just changed
        // shape under it (re-clamp; the open state and filter are preserved).
        let names = self.roster_names();
        let refs: Vec<&str> = names.iter().map(String::as_str).collect();
        self.browse.on_roster_changed(&refs);
    }

    /// Drain whatever audio arrived since last frame into the analyzer.
    /// Runs even while hidden so visuals resume in sync.
    fn pump_audio(&mut self) {
        if let Some(consumer) = self.consumer.as_mut() {
            loop {
                let n = consumer.pop_samples(&mut self.scratch);
                if n == 0 {
                    break;
                }
                self.analyzer.push_interleaved(&self.scratch[..n]);
            }
        }
    }

    fn hidden(&self) -> bool {
        let size = self.window.inner_size();
        self.occluded || size.width == 0 || size.height == 0
    }

    fn redraw(&mut self) {
        self.poll_input_lost();
        self.pump_audio();

        // Measure wall-clock dt for the scene director (shell frame pacing;
        // core analysis stays clock-free). Update the marker even while hidden
        // so the first visible frame after a gap gets a small, clamped dt.
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

        if self.hidden() {
            return;
        }
        self.poll_presets();
        let frame = self.analyzer.take_frame();

        // Per-beat downbeat decomposition (opt-in, Plan 0086 Phase 1). Absent
        // unless `--downbeat-log` was passed; when present it returns on the
        // frames that carry no beat, so the per-frame cost is a bool test and the
        // terms are only recomputed on a beat. Reading them cannot change what
        // they say — `downbeat_terms` is `&self`, alloc-free and clock-free.
        if let Some(log) = self.downbeat_log.as_mut() {
            let analyzer = &self.analyzer;
            log.maybe_log(&frame, || analyzer.downbeat_terms());
        }

        // Hands-off scene rotation: the director decides from dt + this frame's
        // energy whether to advance the preset (manual Space/A override it).
        if self.director.advance(dt, &frame).is_some() {
            self.on_preset_switched();
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
        if let Some(osc) = self.osc.as_mut() {
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
        if std::mem::take(&mut self.pending_switch_settle) {
            warn_cap_overflow(&self.renderer);
            self.reported_overflow = self.renderer.cap_overflow().copied();
            self.update_title();
        }
        // The *live* half: a per-frame geometry-mirror overflow, which no
        // preset-change hook can see (ADR-0007 -- never a silent cut).
        self.poll_cap_overflow();
        self.poll_tier_demotion();
        self.title_tick += 1;
        if self.title_tick >= TITLE_UPDATE_FRAMES {
            self.title_tick = 0;
            self.update_title();
        }
        // Structured 1 Hz log (render thread). The analysis snapshot and RSS are
        // both read lazily, only on the seconds a sample is actually due — this
        // runs every frame.
        let metrics = self.renderer.metrics();
        let renderer = &self.renderer;
        self.diag_log.maybe_log(
            &metrics,
            || renderer.analysis_metrics(),
            rss::current_rss_bytes,
            &self.capture_token,
        );
        // Long-run soak trace (opt-in). Absent unless `--soak` was passed, so the
        // normal loop is unaffected; when present it samples only every few
        // seconds, off the per-frame path.
        if let Some(soak) = self.soak.as_mut() {
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
    /// [`pending_switch_settle`](AppState::pending_switch_settle).
    fn on_preset_switched(&mut self) {
        self.pending_switch_settle = true;
        self.note_soak_switch();
        self.window.request_redraw();
    }

    /// Mark a GPU-resource rebuild in the soak log, if one is running
    /// (Plan 0085 Phase 3).
    ///
    /// Off the per-frame path by construction — the two callers are a preset
    /// switch and a surface reconfigure, both event-driven and both rare. The
    /// frame count comes from the core's own counter so the exclusion window is
    /// measured in the same units the frame-time ring is.
    fn note_soak_switch(&mut self) {
        if self.soak.is_none() {
            return;
        }
        let frames_total = self.renderer.metrics().frames_total;
        if let Some(soak) = self.soak.as_mut() {
            soak.note_switch(frames_total);
        }
    }

    /// Refresh the window title with the preset, system, and the core's
    /// diagnostics (fps + p99). No wall-clock read — the numbers come from the
    /// core's gated clock, the cadence from a frame counter.
    fn update_title(&mut self) {
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
    /// `--tier` and `LMV_TIER` still win at the next launch; this writes the
    /// `[quality] tier` they override, so the documented precedence is unchanged.
    fn swap_tier(&mut self, tier: Tier) {
        self.tier_pinned = true;
        self.config.quality.tier = match tier {
            Tier::Floor => config::TierChoice::Floor,
            Tier::Rich => config::TierChoice::Rich,
        };
        self.save_config();
        if self.renderer.tier() != tier {
            self.renderer.set_tier(tier);
            self.reported_demotion = false;
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
    fn refresh_input_roster(&mut self) {
        let mode = capture_mode(self.input.mode);
        self.input_roster.clear();
        match capture_win::endpoints(mode) {
            Ok(names) => {
                // `default` is a real, always-reachable choice — it is what an
                // unnamed selection means in `config.toml`, and it is where a
                // lost device recovers to — so it leads the roster rather than
                // being only spellable by editing the file.
                self.input_roster.push(DEFAULT_ENDPOINT.to_owned());
                self.input_roster.extend(names);
            }
            Err(err) => eprintln!(
                "could not enumerate {} endpoints: {err}",
                self.input.mode.as_str()
            ),
        }
    }

    /// No endpoint enumeration on platforms whose capture path takes no
    /// selection; the two input rows are read-only there.
    #[cfg(not(windows))]
    fn refresh_input_roster(&mut self) {}

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
    fn input_device_index(&self) -> usize {
        device_row_index(
            &self.input_roster,
            &self.input.device,
            self.capture_endpoint.as_deref(),
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
    fn restart_capture(&mut self, input: &config::Input, persist: Persist) {
        // Stop first. The old thread has to be joined before the new one exists
        // or the ring's single-producer invariant would briefly have two
        // claimants, and an endpoint is not reliably re-openable while a stream
        // still holds it.
        self._capture = None;
        self.consumer = None;

        let started = start_capture(input);
        self.capture_token = started.verdict.token();
        if started.format != self.capture_format {
            self.analyzer = Analyzer::new(started.format)
                .expect("capture layer already validated this format at the boundary");
            self.capture_format = started.format;
        }
        self.consumer = started.consumer;
        self._capture = started.handle;
        self.capture_endpoint = started.endpoint;
        // A stream that is running is not a lost one, whoever asked for it. A
        // start that *failed* leaves the flag as it was, so a recovery keeps its
        // budget and a manual swap does not acquire one.
        if self._capture.is_some() {
            self.input_lost = false;
        }

        // The running selection follows even when the start failed: it is what
        // was asked for, so the row shows it while the verdict says it is not
        // delivering.
        self.input = input.clone();
        if persist == Persist::Yes {
            self.config.input = input.clone();
            self.save_config();
        }
        eprintln!("audio input: {}", self.capture_token);
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
    fn poll_input_lost(&mut self) {
        if capture_lost(self._capture.as_ref()) {
            self.input_lost = true;
        }
        match self.input_recovery.poll(self.input_lost) {
            Recovery::Hold => {}
            Recovery::Reopen(attempt) => {
                eprintln!(
                    "audio input lost; reopening the default {} endpoint \
                     (attempt {attempt} of {INPUT_RECOVERY_ATTEMPTS})",
                    self.input.mode.as_str()
                );
                self.restart_capture(
                    &config::Input {
                        mode: self.input.mode,
                        device: DEFAULT_ENDPOINT.to_owned(),
                    },
                    Persist::No,
                );
            }
            Recovery::GiveUp => {
                self.capture_token = CaptureVerdict::Lost {
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
    fn set_input_mode(&mut self, mode: config::InputMode) {
        if self.input.mode == mode {
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
    fn cycle_input_device(&mut self) {
        if self.input_roster.is_empty() {
            return;
        }
        let next = (self.input_device_index() + 1) % self.input_roster.len();
        self.restart_capture(
            &config::Input {
                mode: self.input.mode,
                device: self.input_roster[next].clone(),
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
    fn toggle_auto_rotate(&mut self) {
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
    fn toggle_diagnostics(&mut self) {
        self.overlay_on = !self.overlay_on;
        self.renderer.set_overlay(self.overlay_on);
        self.window.request_redraw();
    }

    /// The live values the settings rows show, gathered fresh each time they are
    /// drawn or edited — which is what lets [`SettingsState`] hold none of them.
    fn settings_view(&self) -> SettingsView {
        let monitors: Vec<MonitorHandle> = self.window.available_monitors().collect();
        let display_name = monitors
            .get(self.display_index)
            .and_then(MonitorHandle::name)
            .or_else(|| self.config.output.display_name.clone())
            .unwrap_or_else(|| "unknown".to_owned());
        SettingsView {
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
            diagnostics: self.overlay_on,
            input_mode: self.input.mode,
            // Read off the cache, never enumerated here: this runs every frame
            // the modal is up.
            input_device_index: self.input_device_index(),
            input_device_count: self.input_roster.len(),
            input_device_name: self
                .input_roster
                .get(self.input_device_index())
                .cloned()
                .unwrap_or_else(|| self.input.device.clone()),
            // Windows is the only platform whose capture path takes a selection;
            // elsewhere the rows render and do not move.
            input_editable: cfg!(windows),
            preset_name: self.config.hud.preset_name,
            now_playing: self.config.hud.now_playing,
            preset_dir: self.preset_dir.display().to_string(),
        }
    }

    /// Carry out what the settings modal asked for. The modal decides *what*
    /// changes; every effect — the renderer, the window, the director, the config
    /// file — happens here.
    fn apply_settings_action(&mut self, action: SettingsAction) {
        match action {
            SettingsAction::None => return,
            SettingsAction::Redraw | SettingsAction::Close => {}
            SettingsAction::OpenBrowse => {
                self.settings.close();
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
    fn open_browse(&mut self) {
        let names = self.roster_names();
        let refs: Vec<&str> = names.iter().map(String::as_str).collect();
        let active = self.renderer.active_index();
        let layout = self.list_layout(refs.len());
        self.browse
            .handle_key(OverlayKey::Toggle, &refs, active, &layout);
    }

    /// Which modal owns the keyboard and the canvas, if either.
    ///
    /// **One place, consulted by both routing and drawing.** Two `is_open()`
    /// calls kept in agreement by hand is how a key gets routed to the modal that
    /// is not on screen and silently swallowed.
    fn modal(&self) -> Option<Modal> {
        if self.settings.is_open() {
            Some(Modal::Settings)
        } else if self.browse.is_open() {
            Some(Modal::Browse)
        } else {
            None
        }
    }

    /// The browse list's layout for `visible_len` rows at the window's current
    /// size — the one place the shell turns a window into
    /// [`overlay::ListLayout`], so the drawing and the `Left`/`Right` keys can
    /// never disagree about where a row is.
    fn list_layout(&self, visible_len: usize) -> overlay::ListLayout {
        let size = self.window.inner_size();
        overlay::layout(
            visible_len,
            self.browse.highlight(),
            size.width as f32,
            size.height as f32,
        )
    }

    /// Push any track change the metadata source picked up into the core's
    /// banner (Plan 0097). The source runs on its own thread and leaves the
    /// newest string in a slot; this takes it. The **only** place a WinRT-sourced
    /// string reaches the renderer, so nothing calls into the core from a
    /// callback thread.
    #[cfg(windows)]
    fn poll_now_playing(&mut self) {
        // Drained even when the operator has it off, rather than skipped: a
        // string left in the slot would announce a track that changed minutes
        // ago the moment the row is switched back on. Off means the *next*
        // change is the first one drawn.
        let Some(track) = self.now_playing.take_change() else {
            return;
        };
        if self.config.hud.now_playing {
            self.renderer.set_now_playing(&track);
        }
    }

    /// No metadata source outside Windows — `MediaRemote` is private and
    /// restricted (ADR-0110), so the Mac path's answer is the foobar plugin.
    #[cfg(not(windows))]
    fn poll_now_playing(&mut self) {}

    /// Build this frame's on-canvas text and hand it to the renderer: the active
    /// preset name in the corner when [`preset_name_visible`] allows it, plus —
    /// while a modal is open — that modal's own rows. Strings are owned locally
    /// so the renderer's `queue_text` (which copies them) needs no live borrow of
    /// the roster.
    fn queue_frame_text(&mut self) {
        // Taken out and put back rather than borrowed in place: the body below
        // calls `&self` methods (`modal`, `settings_view`, `roster_names`,
        // `list_layout`) while filling them, which a live `&mut self.field`
        // borrow would forbid. `take` leaves an empty Vec behind for the
        // duration and the originals - with their retained capacity - go back at
        // the end, so a steady-state frame does no allocation here.
        //
        // Two buffers, not one: `chrome` is the picture's own furniture and
        // always lands on the output, while `modal` follows the operator to
        // whichever surface is driving. `console::route_into` is what decides,
        // and it is the only thing that decides — a branch here that skipped
        // building the modal rows when the console is open would work today and
        // silently disagree with the routing the first time the rule changes.
        let mut chrome = std::mem::take(&mut self.chrome_scratch);
        let mut modal = std::mem::take(&mut self.modal_scratch);
        chrome.clear();
        modal.clear();

        // `output_modal`, not `modal`: with the console open the rows are not on
        // the show, so nothing covers the corner name and it must stay.
        let console_open = self.console_state();
        if preset_name_visible(
            output_modal(self.modal(), console_open),
            self.overlay_on,
            self.config.hud.preset_name,
        ) {
            chrome.push(console::Line::new(
                self.renderer.preset_name().to_owned(),
                NAME_INSET,
                NAME_INSET,
                NAME_SIZE,
                NAME_COLOR,
            ));
        }

        // The capture verdict, under the core's diagnostics panel and only while
        // it is up (Plan 0083). Built from the stored token rather than from the
        // capture state, so this line and the log's `capture` column are the same
        // sentence about the same run.
        if self.overlay_on {
            chrome.push(console::Line::new(
                overlay::capture_line(&self.capture_token),
                NAME_INSET,
                overlay::CAPTURE_TOP,
                overlay::CAPTURE_SIZE,
                overlay::CAPTURE_COLOR,
            ));
        }

        if self.modal() == Some(Modal::Settings) {
            let view = self.settings_view();
            modal.push(console::Line::new(
                "settings  -  up/down  left/right  esc".to_owned(),
                LIST_INSET,
                LIST_TOP,
                ROW_SIZE,
                HEADER_COLOR,
            ));

            // One column, always: ten rows fit any window this app opens in —
            // they start at `ROWS_TOP` (94 px) with a 30 px pitch, so the last
            // ends at 394 px — and a settings menu that reflowed would move a row
            // out from under the operator's hand mid-edit.
            for (row, (label, value)) in self.settings.lines(&view).into_iter().enumerate() {
                let y = overlay::ROWS_TOP + row as f32 * ROW_H;
                let (marker, color) = if row == self.settings.row() {
                    ("> ", ROW_HL_COLOR)
                } else {
                    ("  ", ROW_COLOR)
                };
                modal.push(console::Line::new(
                    format!("{marker}{label:<14}{value}"),
                    LIST_INSET,
                    y,
                    ROW_SIZE,
                    color,
                ));
            }
        } else if self.modal() == Some(Modal::Browse) {
            let names = self.roster_names();
            let name_refs: Vec<&str> = names.iter().map(String::as_str).collect();
            let visible = self.browse.visible(&name_refs);
            let highlight = self.browse.highlight();

            // Header echoes the filter query (or a hint) above the list, so the
            // user sees what they've typed as it narrows the roster.
            let header = if self.browse.filter().is_empty() {
                "type to filter  -  arrows  enter  esc".to_owned()
            } else {
                format!("filter: {}", self.browse.filter())
            };
            modal.push(console::Line::new(
                header,
                LIST_INSET,
                LIST_TOP,
                ROW_SIZE,
                HEADER_COLOR,
            ));

            // Column-major flow (Plan 0050 Phase 3): every placement decision is
            // the pure `layout`, so this loop only turns `(column, row)` into
            // pixels. Rows the layout scrolls off answer `None` and are skipped.
            let layout = self.list_layout(visible.len());
            for (row, &(_abs, name)) in visible.iter().enumerate() {
                let Some((col, r)) = layout.place(row) else {
                    continue;
                };
                let x = LIST_INSET + col as f32 * overlay::COL_W;
                let y = overlay::ROWS_TOP + r as f32 * ROW_H;
                let (marker, color) = if row == highlight {
                    ("> ", ROW_HL_COLOR)
                } else {
                    ("  ", ROW_COLOR)
                };
                modal.push(console::Line::new(
                    format!("{marker}{}", overlay::fit(name)),
                    x,
                    y,
                    ROW_SIZE,
                    color,
                ));
            }
        }

        // The console's standing header, so an idle console still reads as live.
        // Queued after the routing has cleared last frame's lines and before the
        // modal rows land under it.
        console::route_into(&mut self.frame_text, &mut chrome, &mut modal, console_open);
        if console_open.is_open() {
            self.frame_text
                .console
                .insert(0, console::header(self.renderer.preset_name()));
        }

        let runs = self.frame_text.output_runs();
        self.renderer.queue_text(&runs);

        // `runs` borrows `self.frame_text`, so the scratch buffers can only go
        // home once its last use is behind us.
        drop(runs);
        self.chrome_scratch = chrome;
        self.modal_scratch = modal;
    }

    /// Whether the operator console is open this frame.
    fn console_state(&self) -> console::Console {
        if self.console_window.is_some() {
            console::Console::Open
        } else {
            console::Console::Closed
        }
    }

    /// Present the console's half of this frame, if one is attached.
    ///
    /// Separate from the output's `render` and after it: the console is a
    /// monitor, so a frame it drops or a present that stalls must cost the show
    /// nothing. A failure here closes the console rather than killing the app.
    fn present_console(&mut self) {
        if self.console_window.is_none() {
            return;
        }
        let runs = self.frame_text.console_runs();
        let result = self.renderer.present_aux(&runs);
        drop(runs);
        if let Err(err) = result {
            eprintln!("console present failed, closing it: {err}");
            self.close_console();
        }
    }

    /// Route a pressed key. Overlay control keys (toggle / nav / enter / esc /
    /// backspace) go through its state machine first; while the overlay is open,
    /// printable characters narrow the type-to-filter query and every other key
    /// is swallowed. When it is closed, non-overlay keys fall through to the
    /// shell's own bindings — Space-cycle and the F3 diagnostics toggle.
    fn handle_key(&mut self, event_loop: &ActiveEventLoop, event: &KeyEvent) {
        let PhysicalKey::Code(code) = event.physical_key else {
            return;
        };

        // --- the settings modal owns the keyboard while it is open ---
        if self.modal() == Some(Modal::Settings) {
            // `Tab` hands over rather than stacking: one modal at a time.
            if code == KeyCode::Tab && !event.repeat {
                self.apply_settings_action(SettingsAction::OpenBrowse);
                return;
            }
            // Anything the modal does not own is swallowed, so `Space` cannot
            // cycle a preset out from under an open menu.
            let Some(key) = decode_settings_key(code) else {
                return;
            };
            if event.repeat && !key.is_nav() {
                return;
            }
            let view = self.settings_view();
            let action = self.settings.handle_key(key, &view);
            self.apply_settings_action(action);
            return;
        }

        // **OS key repeat is honoured for modal navigation keys only** (Plan 0050
        // Phase 2). An event loop that drops every repeat before it gets here
        // makes holding an arrow in the browser do nothing. Widening this to "all
        // keys" is what must not happen: a held `Space` would machine-gun preset
        // switches through a ~1 s dissolve each, and a held `F` would thrash
        // fullscreen. So the gate is here, where the key's role is known, rather
        // than at the event site, where it is not.
        let overlay_key = decode_overlay_key(code);
        if event.repeat && !(self.browse.is_open() && overlay_key.is_some_and(OverlayKey::is_nav)) {
            return;
        }

        // `Escape` leaves fullscreen (Plan 0096 Phase 2) — checked here, *after*
        // the modal branches, because a menu on screen owns the key first.
        //
        // It has to be intercepted before the overlay dispatch below:
        // `decode_overlay_key` maps `Escape` unconditionally, so with the browser
        // **closed** it lands on `OverlayAction::None => return` and never reaches
        // the shell's own match. Widening that `None` arm to fall through would
        // route `Enter`, `Backspace` and the arrows out here too — a much broader
        // change than one binding.
        //
        // Windowed it does nothing, and it **never quits**: one stray keypress
        // ending a running show is the failure mode this binding is worth
        // avoiding. Fullscreen goes through the existing toggle so the
        // `[output] fullscreen` write stays on one path with `F`.
        if code == KeyCode::Escape && self.modal().is_none() {
            if self.window.fullscreen().is_some() {
                self.toggle_fullscreen();
            }
            return;
        }

        if let Some(key) = overlay_key {
            let name_refs = self.roster_names();
            let refs: Vec<&str> = name_refs.iter().map(String::as_str).collect();
            let active = self.renderer.active_index();
            let layout = self.list_layout(self.browse.visible(&refs).len());
            match self.browse.handle_key(key, &refs, active, &layout) {
                OverlayAction::None => return, // closed + non-toggle: let it fall away
                OverlayAction::Redraw | OverlayAction::Close => {}
                OverlayAction::Select(index) => {
                    self.renderer.select_preset(index);
                    self.on_preset_switched();
                }
            }
            self.window.request_redraw();
            return;
        }

        // While open, printable characters filter the list; anything else is
        // consumed so it can't reach Space-cycle / F3.
        if self.browse.is_open() {
            if let Some(text) = &event.text {
                let name_refs = self.roster_names();
                let refs: Vec<&str> = name_refs.iter().map(String::as_str).collect();
                let active = self.renderer.active_index();
                let layout = self.list_layout(self.browse.visible(&refs).len());
                let mut changed = false;
                for c in text
                    .chars()
                    .filter(|c| !c.is_control() && !c.is_whitespace())
                {
                    self.browse
                        .handle_key(OverlayKey::Char(c), &refs, active, &layout);
                    changed = true;
                }
                if changed {
                    self.window.request_redraw();
                }
            }
            return;
        }

        match code {
            KeyCode::Space => {
                // Manual next scene: reset the director's dwell so the auto
                // timer restarts from this moment.
                self.director.force_next();
                self.renderer.cycle_preset();
                self.on_preset_switched();
            }
            KeyCode::KeyA => self.toggle_auto_rotate(),
            KeyCode::F3 => self.toggle_diagnostics(),
            // `S` opens settings only out here — while the browser is open it is
            // a filter character, and the branch above returns before reaching
            // this match.
            KeyCode::KeyS => {
                // One of the two refresh points (the other is a mode change).
                // Enumeration is COM, so it happens on the keypress that makes
                // the roster visible, not on the frames that draw it.
                if !self.settings.is_open() {
                    self.refresh_input_roster();
                }
                let view = self.settings_view();
                let action = self.settings.handle_key(SettingsKey::Toggle, &view);
                self.apply_settings_action(action);
            }
            KeyCode::KeyF => self.toggle_fullscreen(),
            KeyCode::KeyD => self.cycle_display(),
            // The operator console. Out here only, like `S`: while the browser
            // is open `C` is a filter character and the branch above has already
            // returned.
            KeyCode::KeyC => self.toggle_console(event_loop),
            // Quality, live (ADR-0054). `[` down a tier, `]` up — the bracket
            // pair reads as a range with the floor on the left.
            KeyCode::BracketLeft => self.swap_tier(Tier::Floor),
            KeyCode::BracketRight => self.swap_tier(Tier::Rich),
            _ => {}
        }
    }

    /// A left-button press: toggle fullscreen when it lands within
    /// `DOUBLE_CLICK` of the previous one (same binding as the `F` hotkey).
    /// Suppressed while the browse overlay is open so it doesn't fight modal
    /// interaction. Wall-clock timing is a shell concern; core stays clock-free.
    #[allow(
        clippy::disallowed_methods,
        reason = "double-click timing is shell input handling; core analysis stays clock-free"
    )]
    fn handle_left_press(&mut self) {
        // Suppressed under **either** modal, through the one accessor — the
        // second one was the easy thing to forget.
        if self.modal().is_some() {
            return;
        }
        let now = Instant::now();
        if self
            .last_click
            .is_some_and(|prev| now.duration_since(prev) <= DOUBLE_CLICK)
        {
            self.last_click = None;
            self.toggle_fullscreen();
        } else {
            self.last_click = Some(now);
        }
    }

    /// The current roster names, owned — so a caller can borrow `&mut` the
    /// renderer afterward without holding a live borrow of the preset list.
    fn roster_names(&self) -> Vec<String> {
        self.renderer.preset_names().map(str::to_owned).collect()
    }
}

/// Which modal, if any, currently owns the keyboard and the canvas.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Modal {
    Browse,
    Settings,
}

/// Whether the corner preset name is drawn this frame (Plan 0096 Phase 1).
///
/// **Presence-based, not timed**: the name yields to anything drawn over it and
/// returns the instant that thing closes. Two things cover it — either modal
/// (whose header starts at [`LIST_TOP`] and crowds it from below) and the core's
/// F3 diagnostics panel, which composites *after* the text layer and so paints
/// straight over it. `enabled` is the operator's own switch (`[hud] preset_name`).
///
/// A free function, not a method, so the rule is assertable as a value with no
/// window and no GPU — the same discipline [`overlay`] and [`settings`] keep.
///
/// This governs the **show furniture only**. The F3 capture line is deliberately
/// not gated on it: that line exists *because* the panel is up (Plan 0083), so
/// the flag that hides the name must not take it with it.
fn preset_name_visible(modal: Option<Modal>, diagnostics: bool, enabled: bool) -> bool {
    enabled && modal.is_none() && !diagnostics
}

/// The modal **as the output sees it**: `None` while the operator console is
/// open, because the rows are drawn there instead.
///
/// [`preset_name_visible`] yields the corner name to whatever is drawn over it.
/// Once the console exists, "a modal is open" and "a modal covers the show" stop
/// being the same fact, and feeding the raw one to that rule would blank the
/// name on the projector every time the operator opened a menu on their own
/// screen — a visible change to the show, caused by a surface the audience
/// cannot see.
///
/// A free function beside the rule it feeds, so both are assertable as values.
fn output_modal(modal: Option<Modal>, console: console::Console) -> Option<Modal> {
    if console.is_open() { None } else { modal }
}

/// Map a physical key to the settings modal's abstract key, or `None` for keys
/// the modal does not own (which are then swallowed while it is open).
fn decode_settings_key(code: KeyCode) -> Option<SettingsKey> {
    Some(match code {
        KeyCode::KeyS => SettingsKey::Toggle,
        KeyCode::ArrowUp => SettingsKey::Up,
        KeyCode::ArrowDown => SettingsKey::Down,
        KeyCode::ArrowLeft => SettingsKey::Left,
        KeyCode::ArrowRight => SettingsKey::Right,
        KeyCode::Escape => SettingsKey::Escape,
        _ => return None,
    })
}

/// Map a physical key to the overlay's abstract key, or `None` for keys the
/// overlay does not own (which then reach the shell's own bindings).
fn decode_overlay_key(code: KeyCode) -> Option<OverlayKey> {
    Some(match code {
        KeyCode::Tab => OverlayKey::Toggle,
        KeyCode::ArrowUp => OverlayKey::Up,
        KeyCode::ArrowDown => OverlayKey::Down,
        KeyCode::ArrowLeft => OverlayKey::Left,
        KeyCode::ArrowRight => OverlayKey::Right,
        KeyCode::Enter | KeyCode::NumpadEnter => OverlayKey::Enter,
        KeyCode::Escape => OverlayKey::Escape,
        KeyCode::Backspace => OverlayKey::Backspace,
        _ => return None,
    })
}

/// What one call to `start_capture` produced: the handle and consumer when it
/// worked, the format the analyzer is built on either way, and — the point of
/// Plan 0083 — the [`CaptureVerdict`] that says which of those two happened.
struct CaptureStart {
    handle: Option<capture_handle::Handle>,
    consumer: Option<SampleConsumer>,
    format: AudioFormat,
    verdict: CaptureVerdict,
    /// The endpoint the stream actually opened, by friendly name — the resolved
    /// one, so a selection that degraded names what is running. `None` when the
    /// start failed, and on a platform whose capture path picks no endpoint.
    endpoint: Option<String>,
}

/// Whether a capture swap writes its selection back to `config.toml`.
///
/// The distinction is who asked. A settings row is an operator choosing an
/// input, and the file records it like every other row. A recovery is the shell
/// keeping the show alive on whatever endpoint is left, which is not a choice
/// and must not overwrite one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Persist {
    Yes,
    No,
}

/// How many times a lost input is reopened before the shell stops trying.
///
/// Each attempt is a synchronous WASAPI activation on the render thread — a
/// visible hitch — and what is being guarded against is one of those per frame
/// against an audio subsystem that is not coming back. Three covers the case the
/// recovery exists for, a device that returns within a moment of a driver reset,
/// without letting a permanently removed interface stutter the show for longer
/// than the silence would have.
const INPUT_RECOVERY_ATTEMPTS: u32 = 3;

/// Consecutive live frames before a recovered input is judged settled and its
/// retry budget restored.
///
/// Without it a single live frame restores the budget, so a stream that opens
/// and dies immediately — a flapping USB interface, a driver resetting in a loop
/// — gets a fresh three attempts every cycle and reopens for the rest of the
/// show. That is the blocking device activation per frame [`INPUT_RECOVERY_ATTEMPTS`]
/// exists to bound, reached by a different road. A stream that survives this many
/// frames is delivering rather than merely constructed: an invalidated endpoint
/// reports itself on the first packet call after start, well inside it. The cost
/// is that a genuine second unplug within the window inherits the first
/// incident's remaining budget instead of a full one.
const INPUT_RECOVERY_SETTLE_FRAMES: u32 = 60;

/// What the shell should do about the capture stream this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Recovery {
    /// Nothing: the stream is alive, or the bound is already spent and said so.
    Hold,
    /// Reopen on the mode's default endpoint; the payload is the 1-based attempt.
    Reopen(u32),
    /// The bound is spent. Emitted **once**, on the frame it runs out, so the
    /// verdict is rewritten and the operator told exactly one time.
    GiveUp,
}

/// The bounded-retry rule, as a value.
///
/// Pure and WASAPI-free: the shell hands it whether the input is lost and does
/// what it says, which is what makes the bound assertable without an audio
/// device to unplug.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct RecoveryPolicy {
    attempts: u32,
    announced: bool,
    /// Consecutive live frames since the last loss, counted only while a budget
    /// is actually spent — an input that has never been lost has nothing to
    /// restore and never enters the window.
    settled: u32,
}

impl RecoveryPolicy {
    /// Advance one frame.
    fn poll(&mut self, lost: bool) -> Recovery {
        if !lost {
            if self.attempts == 0 {
                return Recovery::Hold;
            }
            // A device lost twice in one show should get a real second chance
            // rather than the remainder of the first — but only once the stream
            // has proved it is delivering, or a flap would restore the budget
            // faster than the bound can spend it.
            self.settled += 1;
            if self.settled >= INPUT_RECOVERY_SETTLE_FRAMES {
                *self = Self::default();
            }
            return Recovery::Hold;
        }
        self.settled = 0;
        if self.attempts < INPUT_RECOVERY_ATTEMPTS {
            self.attempts += 1;
            return Recovery::Reopen(self.attempts);
        }
        if self.announced {
            return Recovery::Hold;
        }
        self.announced = true;
        Recovery::GiveUp
    }
}

/// Whether the running capture stream has reported itself dead.
///
/// Only the Windows path reports it; elsewhere a stream is either running or was
/// never started, and there is nothing to observe.
#[cfg(windows)]
fn capture_lost(handle: Option<&capture_handle::Handle>) -> bool {
    handle.is_some_and(capture_win::CaptureHandle::lost)
}

#[cfg(not(windows))]
fn capture_lost(_handle: Option<&capture_handle::Handle>) -> bool {
    false
}

/// The short name of this platform's capture path, as the verdict token carries
/// it. One constant so the live, failed and lost tokens of a run cannot name
/// three different backends.
#[cfg(windows)]
const CAPTURE_BACKEND: &str = "WASAPI";
#[cfg(target_os = "macos")]
const CAPTURE_BACKEND: &str = "SCK";
#[cfg(not(any(windows, target_os = "macos")))]
const CAPTURE_BACKEND: &str = "none";

/// The device name that means "the mode's default endpoint" — the value
/// `config.toml` ships with, the word `pick_device` treats as "no name to
/// match", and the leading entry of the settings menu's endpoint roster.
const DEFAULT_ENDPOINT: &str = "default";

/// The format both platform arms fall back to when capture fails, so the analyzer
/// has something valid to start on. **The verdict never reports it** — a log
/// stating a format nothing is delivering is worse than no log.
const FALLBACK_FORMAT: AudioFormat = AudioFormat {
    sample_rate: 48_000,
    channels: 2,
};

/// The roster position of the endpoint a stream actually opened.
///
/// Pure, so the one property that matters is assertable without a window: the
/// row is positioned by what capture **reports running** (`running`), never by
/// re-matching the configured name against the roster. The configured name is
/// consulted for exactly one thing — whether it is the `default` word, which is
/// the roster's leading slot whatever endpoint it resolved to — and that test is
/// the same one `start_capture` uses to decide there is no name to match.
///
/// A name that matched no endpoint degraded to the default endpoint inside the
/// capture layer, so `running` names that endpoint and the row does too, rather
/// than echoing a request nothing honoured.
fn device_row_index(roster: &[String], configured: &str, running: Option<&str>) -> usize {
    if configured.trim().is_empty() || configured.eq_ignore_ascii_case(DEFAULT_ENDPOINT) {
        return 0;
    }
    let running = running.unwrap_or_default();
    roster.iter().position(|name| name == running).unwrap_or(0)
}

/// The capture layer's mode for a config mode — the one place the two enums
/// meet, so `core`'s source-agnostic split does not turn into a match repeated
/// at every call site.
#[cfg(windows)]
fn capture_mode(mode: config::InputMode) -> capture_win::CaptureMode {
    match mode {
        config::InputMode::Loopback => capture_win::CaptureMode::Loopback,
        config::InputMode::LineIn => capture_win::CaptureMode::LineIn,
    }
}

#[cfg(windows)]
fn start_capture(input: &config::Input) -> CaptureStart {
    let selector = capture_win::CaptureSelector {
        mode: capture_mode(input.mode),
        // "default" (or empty) means the mode's default endpoint — no name match.
        device: (!input.device.trim().is_empty()
            && !input.device.eq_ignore_ascii_case(DEFAULT_ENDPOINT))
        .then(|| input.device.clone()),
    };
    match capture_win::start(&selector) {
        Ok((handle, consumer)) => {
            let format = handle.format();
            let endpoint = handle.device().to_owned();
            let verdict = CaptureVerdict::live(CAPTURE_BACKEND, format, &endpoint);
            CaptureStart {
                handle: Some(handle),
                consumer: Some(consumer),
                format,
                verdict,
                endpoint: Some(endpoint),
            }
        }
        Err(err) => {
            // Stays: it costs nothing and is still the fastest read for anyone
            // already at a terminal. The verdict is for everyone who is not.
            eprintln!("audio capture unavailable ({err}); rendering without audio");
            CaptureStart {
                handle: None,
                consumer: None,
                format: FALLBACK_FORMAT,
                verdict: CaptureVerdict::failed(CAPTURE_BACKEND, err),
                endpoint: None,
            }
        }
    }
}

#[cfg(target_os = "macos")]
fn start_capture(_input: &config::Input) -> CaptureStart {
    match capture_mac::start() {
        Ok((handle, consumer)) => {
            let format = handle.format();
            // ScreenCaptureKit taps the system mix rather than an endpoint the
            // caller picks, so there is no device name to report.
            let verdict = CaptureVerdict::live(CAPTURE_BACKEND, format, "system audio");
            CaptureStart {
                handle: Some(handle),
                consumer: Some(consumer),
                format,
                verdict,
                // Not an endpoint anything can select, so it positions no row.
                endpoint: None,
            }
        }
        Err(err) => {
            eprintln!("ScreenCaptureKit capture unavailable ({err}); rendering without audio");
            CaptureStart {
                handle: None,
                consumer: None,
                format: FALLBACK_FORMAT,
                verdict: CaptureVerdict::failed(CAPTURE_BACKEND, err),
                endpoint: None,
            }
        }
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
fn start_capture(_input: &config::Input) -> CaptureStart {
    // No capture path on this platform; render silence-driven visuals.
    CaptureStart {
        handle: None,
        consumer: None,
        format: FALLBACK_FORMAT,
        verdict: CaptureVerdict::Unsupported,
        endpoint: None,
    }
}

struct App {
    /// Loaded once at startup; the window is created from it on `resumed` and
    /// it is then handed to the `AppState` for live edits + persistence.
    config: Config,
    config_path: Option<PathBuf>,
    /// Soak-log path from `--soak`, or `None` when the mode is off.
    soak_path: Option<PathBuf>,
    /// Per-beat downbeat-log path from `--downbeat-log`, or `None` when the mode
    /// is off (Plan 0086 Phase 1).
    downbeat_log_path: Option<PathBuf>,
    /// The quality-tier pin, already resolved across `--tier` / `LMV_TIER` /
    /// config (Plan 0044). `None` is auto — rich, governed.
    tier: Option<Tier>,
    /// The capture selection, already resolved across `--input` / `--device` /
    /// `[input]` (Plan 0130). Held beside `config` rather than written into it,
    /// so a flag pins this launch without persisting itself on the next save.
    input: config::Input,
    /// The telemetry sink, already bound in `main` — so a bad target is a
    /// startup error rather than a window that opens and then reports one.
    /// `None` when the sink is off.
    osc: Option<OscSink>,
    state: Option<AppState>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_none() {
            // Resolve the configured output display against the live monitor
            // list, then open borderless-fullscreen on it (walking skeleton) or
            // fall back to the windowed 1080p default — the size the NFR 1
            // performance floor is quoted at — when unset/unmatched.
            let monitors: Vec<MonitorHandle> = event_loop.available_monitors().collect();
            let target = resolve_monitor(&monitors, &self.config.output);
            let display_index = target
                .as_ref()
                .map_or(self.config.output.display, |(i, _)| *i);

            let mut attrs = Window::default_attributes().with_title(APP_TITLE);
            attrs = match (self.config.output.fullscreen, target) {
                (true, Some((_, monitor))) => {
                    attrs.with_fullscreen(Some(Fullscreen::Borderless(Some(monitor))))
                }
                // Fullscreen requested but no display resolved -> current monitor.
                (true, None) => attrs.with_fullscreen(Some(Fullscreen::Borderless(None))),
                (false, _) => {
                    attrs.with_inner_size(winit::dpi::PhysicalSize::new(1920u32, 1080u32))
                }
            };

            match event_loop.create_window(attrs) {
                Ok(window) => {
                    let state = AppState::new(
                        Arc::new(window),
                        std::mem::take(&mut self.config),
                        self.config_path.take(),
                        display_index,
                        self.soak_path.take(),
                        self.downbeat_log_path.take(),
                        self.tier,
                        std::mem::take(&mut self.input),
                        self.osc.take(),
                    );
                    state.window.request_redraw();
                    self.state = Some(state);
                }
                Err(err) => {
                    eprintln!("failed to create window: {err}");
                    event_loop.exit();
                }
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        let Some(state) = self.state.as_mut() else {
            if let WindowEvent::CloseRequested = event {
                event_loop.exit();
            }
            return;
        };

        // Which of our two windows this came from. The console's own close
        // button and resize must not be handled as the show's — closing the
        // console leaves the app running, closing the output exits.
        let target = console::dispatch(
            &id,
            &state.window.id(),
            state.console_window.as_ref().map(|w| w.id()).as_ref(),
        );
        if target == console::Target::Unknown {
            return; // a stale event from a window already gone
        }

        if target == console::Target::Console {
            match event {
                WindowEvent::CloseRequested => state.close_console(),
                WindowEvent::Resized(size) => {
                    state.renderer.resize_aux(size.width, size.height);
                }
                // The console redraws with the show, from the output's frame
                // loop — it carries no clock of its own.
                WindowEvent::RedrawRequested => state.window.request_redraw(),
                // Keys reach the **same** state machines from either window: the
                // console is a second keyboard onto one app, not a second app.
                WindowEvent::KeyboardInput { event, .. }
                    if event.state == ElementState::Pressed =>
                {
                    state.handle_key(event_loop, &event);
                }
                _ => {}
            }
            return;
        }

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                state.renderer.resize(size.width, size.height);
                // A reconfigure rebuilds GPU resources exactly as a preset
                // switch does, so the soak log counts it as the same event
                // (Plan 0085 Phase 3) — the measured `frame_ms_p99` spikes
                // included a fullscreen toggle, which arrives here.
                state.note_soak_switch();
                state.window.request_redraw();
            }
            WindowEvent::Occluded(occluded) => {
                state.occluded = occluded;
                if !occluded {
                    state.window.request_redraw();
                }
            }
            WindowEvent::RedrawRequested => state.redraw(),
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => state.handle_left_press(),
            // Repeats are **passed through** now and filtered inside `handle_key`,
            // which is the only place that knows whether the key is a modal
            // navigation key (Plan 0050 Phase 2). Dropping them here is what made
            // holding an arrow in the browser do nothing.
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                state.handle_key(event_loop, &event);
            }
            _ => {}
        }
    }

    #[allow(
        clippy::disallowed_methods,
        reason = "hidden-window wake deadline; shell frame pacing, not core analysis"
    )]
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let Some(state) = self.state.as_mut() else {
            return;
        };
        if state.hidden() {
            // Hidden: no redraws (near-zero GPU), but wake periodically to
            // keep draining audio so the picture is current on return.
            state.pump_audio();
            event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now() + HIDDEN_TICK));
        } else {
            event_loop.set_control_flow(ControlFlow::Wait);
        }
    }
}

/// The preset directory to load and poll, seeding the curated set only when it
/// is the per-user default — an `LMV_PRESET_DIR` override names a directory the
/// user owns (typically the repo's version-controlled `presets/`), so writing
/// our copies into it would be a surprise (ADR-0014). Returns an empty path if
/// nothing resolves, so the caller keeps the renderer's embedded defaults
/// (degrade, never crash — NFR 10).
fn startup_preset_dir() -> PathBuf {
    match resolve_preset_dir() {
        PresetDir::Override(dir) => {
            eprintln!(
                "{PRESET_DIR_ENV} set: reading presets from {}",
                dir.display()
            );
            dir
        }
        PresetDir::Default(dir) => {
            seed_preset_dir(&dir);
            dir
        }
        PresetDir::Unresolved => {
            eprintln!("could not resolve a per-user data directory; keeping embedded presets");
            PathBuf::new()
        }
    }
}

/// Resolve `diagnostics.log` under the per-user app dir (alongside the shared
/// `presets` dir). `None` if the OS data root can't be resolved — the logger
/// then silently no-ops (degrade, never crash — NFR 10).
fn resolve_log_path() -> Option<PathBuf> {
    preset_data_root().map(|root| root.join(APP_DIR_NAME).join("diagnostics.log"))
}

/// Resolve `config.toml` under the per-user app dir (same base as the presets
/// and diagnostics log). `None` if the OS data root can't be resolved — the
/// config then loads defaults and hotkey changes apply live but don't persist.
fn resolve_config_path() -> Option<PathBuf> {
    preset_data_root().map(|root| root.join(APP_DIR_NAME).join("config.toml"))
}

/// The soak-log path if `--soak` was passed (`--soak <path>` / `--soak=<path>`,
/// or a bare `--soak` for the default under the per-user dir), else `None` so
/// the soak sampler is never created and the render loop is unchanged.
fn parse_soak_arg() -> Option<PathBuf> {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if let Some(path) = arg.strip_prefix("--soak=") {
            return Some(PathBuf::from(path));
        }
        if arg == "--soak" {
            // An explicit path may follow; otherwise use the default location.
            return match args.next() {
                Some(next) if !next.starts_with("--") => Some(PathBuf::from(next)),
                _ => Some(default_soak_path()),
            };
        }
    }
    None
}

/// The downbeat-log path if `--downbeat-log` was passed (`--downbeat-log <path>`
/// / `--downbeat-log=<path>`, or a bare `--downbeat-log` for the default under the
/// per-user dir), else `None` so no logger is created and the frame path is
/// unchanged (Plan 0086 Phase 1).
///
/// Deliberately the same three shapes `--soak` accepts, rather than a tidier
/// single one: both flags are typed by hand at a capture session, and a mode that
/// took its path differently from the one beside it would be a footgun for the
/// person running both.
fn parse_downbeat_log_arg() -> Option<PathBuf> {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if let Some(path) = arg.strip_prefix("--downbeat-log=") {
            return Some(PathBuf::from(path));
        }
        if arg == "--downbeat-log" {
            // An explicit path may follow; otherwise use the default location.
            return match args.next() {
                Some(next) if !next.starts_with("--") => Some(PathBuf::from(next)),
                _ => Some(default_downbeat_log_path()),
            };
        }
    }
    None
}

/// The tier `--tier <name>` / `--tier=<name>` pins, or `None` when the flag is
/// absent (Plan 0044).
///
/// `Err` on a missing or unparseable value. Unlike `LMV_TIER`, a bad `--tier` is
/// a **usage error** rather than something to degrade past: it was typed for this
/// run, so silently starting on another tier would answer the wrong question.
fn parse_tier_arg() -> Result<Option<Tier>, String> {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let value = if let Some(inline) = arg.strip_prefix("--tier=") {
            Some(inline.to_owned())
        } else if arg == "--tier" {
            Some(args.next().unwrap_or_default())
        } else {
            None
        };
        if let Some(value) = value {
            return Tier::from_name(&value)
                .map(Some)
                .ok_or_else(|| format!("--tier `{value}`: expected `floor` or `rich`"));
        }
    }
    Ok(None)
}

/// The value of `--name value` / `--name=value` when `arg` is that flag, else
/// `None`. The spaced spelling consumes the next argument, which is why the
/// iterator is threaded through rather than re-scanned.
///
/// The inner `Err` is the flag with nothing after it at all. It is a usage error
/// rather than an empty value, because an empty value is not inert: `--device`
/// reads `""` as the mode's default endpoint, which is the opposite of asking
/// for a device by name.
fn flag_value(
    arg: &str,
    name: &str,
    args: &mut impl Iterator<Item = String>,
) -> Option<Result<String, String>> {
    if let Some(inline) = arg
        .strip_prefix(name)
        .and_then(|rest| rest.strip_prefix('='))
    {
        return Some(Ok(inline.to_owned()));
    }
    if arg != name {
        return None;
    }
    Some(
        args.next()
            .ok_or_else(|| format!("{name}: expected a value")),
    )
}

/// The `--input <mode>` / `--device <name>` overrides, in both the spaced and
/// the `=` spelling `--soak` and `--tier` already accept.
///
/// `Err` on an `--input` value that names no mode. Like `--tier` and unlike
/// `LMV_TIER`, a bad flag is a **usage error** rather than something to degrade
/// past: it was typed for this run, so starting on another input would answer
/// the wrong question. A `--device` naming an absent endpoint is *not* an error
/// — the capture layer degrades to the mode's default endpoint and says so, and
/// a flag must not be stricter about the world than about its own spelling.
fn parse_input_args() -> Result<(Option<config::InputMode>, Option<String>), String> {
    parse_input_args_from(std::env::args().skip(1))
}

/// [`parse_input_args`]'s rule as a pure function of the argument list, so both
/// spellings are testable without a process.
fn parse_input_args_from(
    args: impl Iterator<Item = String>,
) -> Result<(Option<config::InputMode>, Option<String>), String> {
    let mut args = args.peekable();
    let mut mode = None;
    let mut device = None;
    while let Some(arg) = args.next() {
        if let Some(value) = flag_value(&arg, "--input", &mut args) {
            let value = value?;
            mode =
                Some(config::InputMode::from_name(&value).ok_or_else(|| {
                    format!("--input `{value}`: expected `loopback` or `line-in`")
                })?);
        } else if let Some(value) = flag_value(&arg, "--device", &mut args) {
            let value = value?;
            // Both spellings, so `--device=` is refused for the same reason a
            // trailing `--device` is: it selects the default endpoint while
            // reading as a request for a named one.
            if value.trim().is_empty() {
                return Err("--device: expected an endpoint name (see --list-devices)".to_owned());
            }
            device = Some(value);
        }
    }
    Ok((mode, device))
}

/// Which source decided the capture selection, so a surprising input is
/// traceable to what set it — the tier-source shape ADR-0045 established, minus
/// the environment level (Plan 0130 says why there is no `LMV_INPUT`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InputSource {
    /// `--input` / `--device` on the command line.
    Flag,
    /// `config.toml`'s `[input]` section moved it off the built-in.
    Config,
    /// Nothing chose it: loopback of the default render endpoint.
    Default,
}

impl InputSource {
    /// How to name this source in a log line.
    fn as_str(self) -> &'static str {
        match self {
            InputSource::Flag => "--input/--device",
            InputSource::Config => "config.toml",
            InputSource::Default => "default",
        }
    }
}

/// Resolve the capture selection: `--input` / `--device` over `[input]`.
///
/// **Each flag overrides its own field**, so `--device` alone keeps the
/// configured mode and `--input` alone keeps the configured device name. A name
/// that belongs to the other dataflow then matches nothing and degrades to that
/// mode's default endpoint with a stderr note — announced and self-correcting,
/// which is a better answer than silently discarding what the operator
/// configured.
///
/// Pure: every source arrives already parsed, so the precedence rule is testable
/// without touching the process environment.
fn resolve_input(
    mode: Option<config::InputMode>,
    device: Option<String>,
    config: &config::Input,
) -> (config::Input, InputSource) {
    let from_flag = mode.is_some() || device.is_some();
    let resolved = config::Input {
        mode: mode.unwrap_or(config.mode),
        device: device.unwrap_or_else(|| config.device.clone()),
    };
    let built_in = config::Input::default();
    let source = if from_flag {
        InputSource::Flag
    } else if resolved.mode == built_in.mode && resolved.device == built_in.device {
        InputSource::Default
    } else {
        InputSource::Config
    };
    (resolved, source)
}

/// The `--osc <host:port>` override, in both the spaced and the `=` spelling.
///
/// `Err` on the flag with nothing after it. An empty value is refused for the
/// same reason `--device=` is: it reads as a request and means nothing.
fn parse_osc_arg() -> Result<Option<String>, String> {
    parse_osc_arg_from(std::env::args().skip(1))
}

/// [`parse_osc_arg`]'s rule as a pure function of the argument list.
fn parse_osc_arg_from(args: impl Iterator<Item = String>) -> Result<Option<String>, String> {
    let mut args = args.peekable();
    let mut target = None;
    while let Some(arg) = args.next() {
        if let Some(value) = flag_value(&arg, "--osc", &mut args) {
            let value = value?;
            if value.trim().is_empty() {
                return Err("--osc: expected a target as host:port".to_owned());
            }
            target = Some(value);
        }
    }
    Ok(target)
}

/// Resolve the telemetry target: `--osc` over `[osc]`. `None` means the sink
/// stays off and no socket is bound.
///
/// **The flag both aims the sink and turns it on**, which is what makes
/// `--osc 10.0.0.4:7700` a complete instruction rather than one that also needs
/// `enabled = true` in a file. `enabled = false` in the config cannot veto a
/// flag typed for this run — the flag is the more specific statement, the same
/// precedence `--input` and `--tier` already have.
fn resolve_osc(flag: Option<String>, config: &config::Osc) -> Option<(String, u32)> {
    match flag {
        Some(target) => Some((target, config.rate_hz)),
        None if config.enabled => Some((config.target.clone(), config.rate_hz)),
        None => None,
    }
}

/// Default soak-log location: under the per-user app dir, or `soak.log` in the
/// current directory if that can't be resolved — so `--soak` always logs
/// somewhere.
fn default_soak_path() -> PathBuf {
    preset_data_root()
        .map(|root| root.join(APP_DIR_NAME).join("soak.log"))
        .unwrap_or_else(|| PathBuf::from("soak.log"))
}

/// Default per-beat downbeat-log location: under the per-user app dir, or
/// `downbeat.log` in the current directory if that can't be resolved — so a bare
/// `--downbeat-log` always logs somewhere.
fn default_downbeat_log_path() -> PathBuf {
    preset_data_root()
        .map(|root| root.join(APP_DIR_NAME).join("downbeat.log"))
        .unwrap_or_else(|| PathBuf::from("downbeat.log"))
}

/// Pick the monitor for the configured output, returning its index in
/// `monitors` and a handle. A stored *name* wins over the raw index (winit's
/// monitor ordering isn't stable across boot/hotplug — plan Risks); an
/// out-of-range index falls back to the first monitor. `None` only when no
/// monitors are enumerated at all.
fn resolve_monitor(
    monitors: &[MonitorHandle],
    output: &config::Output,
) -> Option<(usize, MonitorHandle)> {
    if monitors.is_empty() {
        return None;
    }
    if let Some(name) = &output.display_name
        && let Some((index, monitor)) = monitors
            .iter()
            .enumerate()
            .find(|(_, m)| m.name().as_deref() == Some(name.as_str()))
    {
        return Some((index, monitor.clone()));
    }
    if let Some(monitor) = monitors.get(output.display) {
        return Some((output.display, monitor.clone()));
    }
    monitors.first().map(|monitor| (0, monitor.clone()))
}

/// Seed the embedded curated set into `dir` on first run. An unresolved
/// (empty) path or a seeding error is logged and otherwise ignored — the
/// renderer's embedded defaults remain (degrade, never crash — NFR 10).
fn seed_preset_dir(dir: &Path) {
    if dir.as_os_str().is_empty() {
        return;
    }
    match lmv_core::preset::seed_dir(dir) {
        Ok(0) => {}
        Ok(n) => eprintln!("seeded {n} curated preset(s) into {}", dir.display()),
        Err(err) => eprintln!("could not seed presets into {}: {err}", dir.display()),
    }
}

/// A cheap change signature for the preset directory: the newest `.toml` mtime
/// (nanoseconds) and the file count. Any edit bumps an mtime; add/remove
/// changes the count. `None` if the directory can't be read.
fn dir_signature(dir: &Path) -> Option<(u128, usize)> {
    let mut latest = 0u128;
    let mut count = 0usize;
    for entry in std::fs::read_dir(dir).ok()?.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "toml") {
            count += 1;
            if let Ok(modified) = entry.metadata().and_then(|m| m.modified())
                && let Ok(since) = modified.duration_since(std::time::UNIX_EPOCH)
            {
                latest = latest.max(since.as_nanos());
            }
        }
    }
    Some((latest, count))
}

/// Load presets from `dir` and, if any compiled, install them on the renderer.
/// Malformed files are reported to stderr; a directory with no valid presets
/// leaves the renderer's current set (embedded defaults or last good) in place.
/// Non-fatal warnings (an unknown parameter name — usually a typo) are printed
/// too: the preset still loads and renders, and the mistake is not silent
/// (ADR-0020).
fn reload_presets(renderer: &mut Renderer, dir: &Path) {
    let report = lmv_core::preset::load_dir(dir);
    for (path, err) in &report.errors {
        eprintln!("preset {}: {err}", path.display());
    }
    for (path, warning) in &report.warnings {
        eprintln!("preset {}: warning: {warning}", path.display());
    }
    if report.presets.is_empty() {
        if !report.errors.is_empty() {
            eprintln!("no valid presets in {}; keeping current set", dir.display());
        }
    } else {
        eprintln!(
            "loaded {} preset(s) from {}",
            report.presets.len(),
            dir.display()
        );
        renderer.set_presets(report.presets);
        warn_cap_overflow(renderer);
    }
}

/// Print the enumerable audio devices (the `--list-devices` aid). Windows-first
/// per ADR-0001; other platforms note that device selection isn't wired there.
#[cfg(windows)]
fn list_devices_and_exit() {
    if let Err(err) = capture_win::list_devices() {
        eprintln!("could not list audio devices: {err}");
    }
}

#[cfg(not(windows))]
fn list_devices_and_exit() {
    eprintln!("--list-devices is Windows-only (Plan 0009 Phase 2)");
}

/// The refresh rate of the monitor this window is on, in Hz, or `None` when winit
/// reports none — which is common on a virtual or remote display. The governor's
/// budget falls back to 60 Hz in that case.
fn display_hz(window: &Window) -> Option<f32> {
    let millihertz = window
        .current_monitor()
        .or_else(|| window.primary_monitor())
        .and_then(|m| m.refresh_rate_millihertz())?;
    Some(millihertz as f32 / 1000.0)
}

/// Surface a line scene's segment-cap truncation to stderr (ADR-0007: the cap
/// is never a silent cut). A no-op in the common case where the active preset's
/// geometry fit within the cap. Called after every active-preset change.
fn warn_cap_overflow(renderer: &Renderer) {
    if let Some(overflow) = renderer.cap_overflow() {
        eprintln!("preset '{}': {overflow}", renderer.preset_name());
    }
}

fn main() {
    // Startup aid: print the enumerable audio endpoints and exit, so the
    // operator can copy a friendly name into `input.device` (Plan 0009 Phase 2).
    if std::env::args().skip(1).any(|arg| arg == "--list-devices") {
        list_devices_and_exit();
        return;
    }

    // expect: init-time invariant — without an event loop there is no app.
    let event_loop = EventLoop::new().expect("failed to create event loop");

    // Load the operator config before the window exists so the first frame can
    // open on the right display; a missing/garbled file degrades to windowed
    // defaults (NFR 10).
    let config_path = resolve_config_path();
    let config = config_path.as_deref().map(Config::load).unwrap_or_default();

    let soak_path = parse_soak_arg();
    let downbeat_log_path = parse_downbeat_log_arg();

    // Quality tier, highest precedence first: `--tier`, `LMV_TIER`, config
    // (Plan 0044 / ADR-0045). A bad flag is a usage error; a bad env var is
    // reported and stepped past, so a stale export cannot stop the show (NFR 10).
    let tier_flag = match parse_tier_arg() {
        Ok(tier) => tier,
        Err(msg) => {
            eprintln!("{msg}");
            std::process::exit(1);
        }
    };
    let tier_from_env = tier_env().unwrap_or_else(|msg| {
        eprintln!("{msg}; ignoring");
        None
    });
    let (tier, tier_source) = resolve_tier(tier_flag, tier_from_env, config.quality.tier.tier());
    if let Some(tier) = tier {
        eprintln!(
            "quality tier pinned {} by {}",
            tier.as_str(),
            tier_source.as_str()
        );
    }

    // Audio input, flags over config (Plan 0130 / ADR-0142). A bad `--input`
    // exits for the same reason a bad `--tier` does — it was typed for this run.
    let (input_mode_flag, input_device_flag) = match parse_input_args() {
        Ok(flags) => flags,
        Err(msg) => {
            eprintln!("{msg}");
            std::process::exit(1);
        }
    };
    let (input, input_source) = resolve_input(input_mode_flag, input_device_flag, &config.input);
    if input_source != InputSource::Default {
        // Only Windows' capture path takes a selection; the macOS arm taps the
        // system mix and Linux has no capture at all. A line that named a source
        // for a selection the platform never applied would explain a surprising
        // input by pointing at something that had no effect on it.
        let applied = if cfg!(windows) {
            ""
        } else {
            " (ignored: this platform's capture path selects no endpoint)"
        };
        eprintln!(
            "audio input {} on '{}' by {}{}",
            input.mode.as_str(),
            input.device,
            input_source.as_str(),
            applied
        );
    }

    // Lighting telemetry, flag over config (ADR-0144). Bound here rather than
    // in the window's `resumed`, so a target that cannot be resolved is a usage
    // error at startup - a flag typed for this run, refused for the same reason
    // a bad `--tier` is. A target that comes from `config.toml` is **not**
    // fatal: a stale file must not stop the show, so it degrades to no sink and
    // says so (NFR 10).
    let osc_flag = match parse_osc_arg() {
        Ok(target) => target,
        Err(msg) => {
            eprintln!("{msg}");
            std::process::exit(1);
        }
    };
    let from_flag = osc_flag.is_some();
    let osc = match resolve_osc(osc_flag, &config.osc) {
        Some((target, rate_hz)) => match OscSink::bind(&target, rate_hz) {
            Ok(sink) => {
                eprintln!("osc telemetry to {} at {rate_hz} Hz", sink.target());
                Some(sink)
            }
            Err(msg) if from_flag => {
                eprintln!("{msg}");
                std::process::exit(1);
            }
            Err(msg) => {
                eprintln!("{msg}; osc telemetry off");
                None
            }
        },
        None => None,
    };

    let mut app = App {
        config,
        config_path,
        soak_path,
        downbeat_log_path,
        tier,
        input,
        osc,
        state: None,
    };
    if let Err(err) = event_loop.run_app(&mut app) {
        eprintln!("event loop error: {err}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::{
        INPUT_RECOVERY_ATTEMPTS, INPUT_RECOVERY_SETTLE_FRAMES, InputSource, Modal, Recovery,
        RecoveryPolicy, config, console, device_row_index, output_modal, parse_input_args_from,
        parse_osc_arg_from, preset_name_visible, resolve_input, resolve_osc,
    };

    /// **A live input costs nothing and never reopens.** The policy runs every
    /// frame of every show, so the overwhelmingly common answer has to be `Hold`.
    #[test]
    fn a_live_input_is_never_reopened() {
        let mut policy = RecoveryPolicy::default();
        for _ in 0..1000 {
            assert_eq!(policy.poll(false), Recovery::Hold);
        }
        assert_eq!(policy, RecoveryPolicy::default(), "a live input kept state");
    }

    /// **The retry is bounded and then silent.** An unbounded retry is a
    /// blocking device activation per frame against a subsystem that is not
    /// coming back, which stutters the show worse than the silence does — and
    /// `GiveUp` arrives exactly once, so neither the operator nor the verdict is
    /// told the same thing on every subsequent frame.
    #[test]
    fn a_lost_input_reopens_a_bounded_number_of_times_then_gives_up_once() {
        let mut policy = RecoveryPolicy::default();
        for attempt in 1..=INPUT_RECOVERY_ATTEMPTS {
            assert_eq!(
                policy.poll(true),
                Recovery::Reopen(attempt),
                "attempt {attempt} did not reopen"
            );
        }
        assert_eq!(policy.poll(true), Recovery::GiveUp);
        for _ in 0..500 {
            assert_eq!(
                policy.poll(true),
                Recovery::Hold,
                "the policy kept talking after it gave up"
            );
        }
    }

    /// **A recovered input gets its whole budget back — once it has settled.**
    /// An interface unplugged twice in one show is two incidents, not one long
    /// one, so the second must not inherit the remainder of the first. What the
    /// window buys is that "recovered" means a stream that kept delivering, not
    /// one that merely got constructed; the frame the window closes on is
    /// asserted from both sides, because an off-by-one here is the difference
    /// between this rule and the one below it.
    #[test]
    fn a_recovery_restores_the_full_budget_once_it_has_settled() {
        let mut policy = RecoveryPolicy::default();
        assert_eq!(policy.poll(true), Recovery::Reopen(1));
        assert_eq!(policy.poll(true), Recovery::Reopen(2));

        // One frame short of the window, the budget is still spent.
        for _ in 0..INPUT_RECOVERY_SETTLE_FRAMES - 1 {
            assert_eq!(policy.poll(false), Recovery::Hold);
        }
        assert_ne!(
            policy,
            RecoveryPolicy::default(),
            "the budget came back before the stream had settled"
        );

        // The frame that completes it restores everything.
        assert_eq!(policy.poll(false), Recovery::Hold);
        assert_eq!(policy, RecoveryPolicy::default());

        for attempt in 1..=INPUT_RECOVERY_ATTEMPTS {
            assert_eq!(policy.poll(true), Recovery::Reopen(attempt));
        }
        assert_eq!(policy.poll(true), Recovery::GiveUp);
    }

    /// **A flapping endpoint cannot outrun the bound.** A stream that opens and
    /// dies on the very next frame is the case that defeats a budget restored by
    /// a single live frame: it would hand back three attempts every cycle and
    /// reopen for the rest of the show, which is the blocking device activation
    /// per frame the bound exists to prevent, reached by a different road than
    /// a device that never comes back at all.
    #[test]
    fn a_stream_that_dies_as_fast_as_it_opens_still_gives_up() {
        let mut policy = RecoveryPolicy::default();
        let mut reopens = 0;
        let mut gave_up = 0;
        for _ in 0..10_000 {
            match policy.poll(true) {
                Recovery::Reopen(_) => {
                    reopens += 1;
                    // The reopen "worked" — for exactly one frame.
                    assert_eq!(policy.poll(false), Recovery::Hold);
                }
                Recovery::GiveUp => gave_up += 1,
                Recovery::Hold => {}
            }
        }
        assert_eq!(
            reopens, INPUT_RECOVERY_ATTEMPTS,
            "a flapping endpoint reopened past the bound"
        );
        assert_eq!(gave_up, 1, "the give-up notice did not arrive exactly once");
    }

    /// `--osc` in both spellings, and the empty value refused for the same
    /// reason `--device=` is: it reads as a request and names nothing.
    #[test]
    fn both_osc_flag_spellings_parse() {
        let argv = |args: &[&str]| args.iter().map(|a| (*a).to_owned()).collect::<Vec<_>>();

        assert_eq!(
            parse_osc_arg_from(argv(&["--osc", "192.168.0.1:9000"]).into_iter()),
            Ok(Some("192.168.0.1:9000".to_owned()))
        );
        assert_eq!(
            parse_osc_arg_from(argv(&["--osc=192.168.0.1:9000"]).into_iter()),
            Ok(Some("192.168.0.1:9000".to_owned()))
        );
        // An unaccompanied flag stays absent; the resolver decides what that means.
        assert_eq!(
            parse_osc_arg_from(argv(&["--soak", "--tier=floor"]).into_iter()),
            Ok(None)
        );
        assert!(parse_osc_arg_from(argv(&["--osc="]).into_iter()).is_err());
        assert!(parse_osc_arg_from(argv(&["--osc"]).into_iter()).is_err());
    }

    /// **The flag both aims the sink and turns it on**, and `enabled = false` in
    /// a file cannot veto it — otherwise `--osc <target>` would silently do
    /// nothing on the one machine whose config had ever been written. With no
    /// flag, the config decides, and the built-in default is off: a machine that
    /// never asked for a lighting rig binds no socket.
    #[test]
    fn the_osc_flag_overrides_the_config_and_enables_the_sink() {
        let off = config::Osc::default();
        assert!(!off.enabled, "the built-in default must be off");
        assert_eq!(resolve_osc(None, &off), None);

        assert_eq!(
            resolve_osc(Some("10.0.0.4:7700".to_owned()), &off),
            Some(("10.0.0.4:7700".to_owned(), off.rate_hz)),
            "an explicit flag did not beat `enabled = false`"
        );

        let on = config::Osc {
            enabled: true,
            target: "192.168.1.101:7000".to_owned(),
            rate_hz: 30,
        };
        assert_eq!(
            resolve_osc(None, &on),
            Some(("192.168.1.101:7000".to_owned(), 30))
        );
        // The flag moves the target and leaves the cadence to the file, which is
        // the only key it has no spelling for.
        assert_eq!(
            resolve_osc(Some("10.0.0.4:7700".to_owned()), &on),
            Some(("10.0.0.4:7700".to_owned(), 30))
        );
    }

    /// Both spellings of both flags, and the fact that an absent flag stays
    /// absent rather than resolving to a default value here — the resolver, not
    /// the parser, is what decides what an absent flag means.
    #[test]
    fn both_flag_spellings_parse() {
        let argv = |args: &[&str]| args.iter().map(|a| (*a).to_owned()).collect::<Vec<_>>();

        assert_eq!(
            parse_input_args_from(
                argv(&["--input", "line-in", "--device", "ZOOM AMS-22"]).into_iter()
            ),
            Ok((
                Some(config::InputMode::LineIn),
                Some("ZOOM AMS-22".to_owned())
            ))
        );
        assert_eq!(
            parse_input_args_from(argv(&["--input=line-in", "--device=ZOOM AMS-22"]).into_iter()),
            Ok((
                Some(config::InputMode::LineIn),
                Some("ZOOM AMS-22".to_owned())
            ))
        );
        // Case-insensitive, like `--tier`, and unaccompanied flags stay `None`.
        assert_eq!(
            parse_input_args_from(argv(&["--soak", "--input", "LOOPBACK"]).into_iter()),
            Ok((Some(config::InputMode::Loopback), None))
        );
        assert_eq!(
            parse_input_args_from(argv(&["--device=default"]).into_iter()),
            Ok((None, Some("default".to_owned())))
        );
        assert_eq!(
            parse_input_args_from(argv(&["--fullscreen"]).into_iter()),
            Ok((None, None))
        );

        // A value that names no mode is a usage error naming what it saw, not a
        // silent fall-through to loopback.
        let err = parse_input_args_from(argv(&["--input", "lineout"]).into_iter())
            .expect_err("`lineout` is not an input mode");
        assert!(err.contains("--input") && err.contains("lineout"), "{err}");
        // Including the spelling that swallows the next argument when there is
        // none to swallow: an empty value is still a value that named no mode.
        assert!(parse_input_args_from(argv(&["--input"]).into_iter()).is_err());
    }

    /// **A `--device` with no name is a usage error, in both spellings.** An
    /// empty value is not inert: `start_capture` reads `""` as the mode's
    /// default endpoint, so the flag would quietly select the opposite of what
    /// asking for a device by name means.
    #[test]
    fn a_device_flag_with_no_name_is_a_usage_error() {
        let argv = |args: &[&str]| args.iter().map(|a| (*a).to_owned()).collect::<Vec<_>>();

        // Trailing, with nothing to swallow.
        let err = parse_input_args_from(argv(&["--device"]).into_iter())
            .expect_err("a bare `--device` selected something");
        assert!(err.contains("--device"), "{err}");
        // The `=` spelling of the same mistake, and a value that is only space.
        assert!(parse_input_args_from(argv(&["--device="]).into_iter()).is_err());
        assert!(parse_input_args_from(argv(&["--device", "   "]).into_iter()).is_err());

        // The explicit word still parses: `default` is a real selection, and it
        // is the one an operator types to undo a `--device` in a launcher.
        assert_eq!(
            parse_input_args_from(argv(&["--device", "default"]).into_iter()),
            Ok((None, Some("default".to_owned())))
        );
    }

    /// **The device row is positioned by what capture reports running**, never
    /// by re-matching the configured name against the roster.
    ///
    /// The third case is the one that motivates the rule and the only one a
    /// second matcher gets wrong: `pick_device` matches exact across *every*
    /// endpoint before it tries substring across every endpoint, so an exact
    /// name that is also a substring of an earlier entry opens the later one —
    /// while a per-element exact-or-substring pass stops at the earlier entry
    /// and highlights a row the stream is not on. Neither name in the pair on
    /// the development box is a substring of the other, which is why nothing
    /// there can tell the two rules apart.
    #[test]
    fn the_device_row_follows_the_endpoint_that_is_running() {
        let roster: Vec<String> = ["default", "Headphones (2- USB Audio)", "Headphones"]
            .iter()
            .map(|s| (*s).to_owned())
            .collect();

        // The `default` word is the leading slot whatever it resolved to — the
        // row and `config.toml` name the same thing.
        assert_eq!(
            device_row_index(&roster, "default", Some("Headphones (2- USB Audio)")),
            0
        );
        assert_eq!(device_row_index(&roster, "  ", Some("Headphones")), 0);

        // The substring trap: `pick_device` opened the exact match at index 2.
        assert_eq!(
            device_row_index(&roster, "Headphones", Some("Headphones")),
            2
        );

        // A name that matched nothing degraded inside the capture layer, and the
        // row names the endpoint actually running rather than the request.
        assert_eq!(
            device_row_index(&roster, "ZOOM AMS-22", Some("Headphones (2- USB Audio)")),
            1
        );

        // Nothing running, and a roster that does not contain what is: index 0
        // rather than a panic or an out-of-range row.
        assert_eq!(device_row_index(&roster, "Headphones", None), 0);
        assert_eq!(device_row_index(&roster, "Headphones", Some("Gone")), 0);
        assert_eq!(device_row_index(&[], "Headphones", Some("Headphones")), 0);
    }

    /// **The flags win over `config.toml`, one field at a time.** This is the
    /// precedence ADR-0142 mirrors off `--tier`, and the per-field half is what
    /// lets `--device` alone keep the configured mode.
    #[test]
    fn the_flags_override_the_config_field_by_field() {
        let configured = config::Input {
            mode: config::InputMode::Loopback,
            device: "Speakers (Realtek)".to_owned(),
        };

        // Both flags: neither configured field survives, and the flag is named
        // as the source.
        let (input, source) = resolve_input(
            Some(config::InputMode::LineIn),
            Some("Line (ZOOM AMS-22 Audio)".to_owned()),
            &configured,
        );
        assert_eq!(input.mode, config::InputMode::LineIn);
        assert_eq!(input.device, "Line (ZOOM AMS-22 Audio)");
        assert_eq!(source, InputSource::Flag);

        // `--input` alone keeps the configured device name; `--device` alone
        // keeps the configured mode.
        let (input, source) = resolve_input(Some(config::InputMode::LineIn), None, &configured);
        assert_eq!(input.mode, config::InputMode::LineIn);
        assert_eq!(
            input.device, configured.device,
            "the config device was lost"
        );
        assert_eq!(source, InputSource::Flag);

        let (input, source) = resolve_input(None, Some("Line (ZOOM)".to_owned()), &configured);
        assert_eq!(input.mode, configured.mode, "the config mode was lost");
        assert_eq!(input.device, "Line (ZOOM)");
        assert_eq!(source, InputSource::Flag);
    }

    /// With no flags the config decides, and the source distinguishes a config
    /// that moved the selection from one that merely restates the built-in —
    /// which is what keeps the startup line off a run nothing chose.
    #[test]
    fn without_flags_the_config_decides_and_the_built_in_is_named_as_such() {
        let configured = config::Input {
            mode: config::InputMode::LineIn,
            device: "Line (ZOOM AMS-22 Audio)".to_owned(),
        };
        let (input, source) = resolve_input(None, None, &configured);
        assert_eq!(input.mode, config::InputMode::LineIn);
        assert_eq!(input.device, configured.device);
        assert_eq!(source, InputSource::Config);

        let (input, source) = resolve_input(None, None, &config::Input::default());
        assert_eq!(input.mode, config::InputMode::Loopback);
        assert_eq!(input.device, "default");
        assert_eq!(source, InputSource::Default);

        // A config that spells out the built-in is the same selection, so it is
        // reported the same way rather than as a choice someone made.
        let spelled_out = config::Input {
            mode: config::InputMode::Loopback,
            device: "default".to_owned(),
        };
        assert_eq!(
            resolve_input(None, None, &spelled_out).1,
            InputSource::Default
        );
    }

    /// Every source renders to a distinct, non-empty name: the startup line
    /// exists to say *what* set a surprising input, so two sources that print
    /// the same word would defeat it.
    #[test]
    fn the_three_input_sources_are_distinguishable() {
        let names = [
            InputSource::Flag.as_str(),
            InputSource::Config.as_str(),
            InputSource::Default.as_str(),
        ];
        for name in names {
            assert!(!name.is_empty());
        }
        assert_ne!(names[0], names[1]);
        assert_ne!(names[1], names[2]);
        assert_ne!(names[0], names[2]);
    }

    #[test]
    fn name_shows_when_nothing_covers_it() {
        assert!(preset_name_visible(None, false, true));
    }

    #[test]
    fn diagnostics_panel_takes_the_corner() {
        // The panel composites after the text layer, so a name drawn here would
        // be painted over rather than shown beside it.
        assert!(!preset_name_visible(None, true, true));
    }

    #[test]
    fn either_modal_suppresses_the_name() {
        assert!(!preset_name_visible(Some(Modal::Settings), false, true));
        assert!(!preset_name_visible(Some(Modal::Browse), false, true));
    }

    #[test]
    fn the_operator_switch_wins_over_everything() {
        // Off means off in every state, not just the uncovered one.
        assert!(!preset_name_visible(None, false, false));
        assert!(!preset_name_visible(None, true, false));
        assert!(!preset_name_visible(Some(Modal::Browse), false, false));
    }

    /// A modal opened on the console does not cover the show, so the corner name
    /// stays on it. Without this the operator opening a menu on their own screen
    /// would blank a line on the projector.
    #[test]
    fn a_modal_on_the_console_leaves_the_shows_name_alone() {
        use console::Console;

        for modal in [Modal::Browse, Modal::Settings] {
            assert!(preset_name_visible(
                output_modal(Some(modal), Console::Open),
                false,
                true
            ));
        }
    }

    /// With no console, the rule is exactly what it was: the modal is on the
    /// show and covers the name.
    #[test]
    fn a_modal_on_the_output_still_suppresses_the_name() {
        use console::Console;

        for modal in [Modal::Browse, Modal::Settings] {
            assert!(!preset_name_visible(
                output_modal(Some(modal), Console::Closed),
                false,
                true
            ));
        }
    }

    /// The diagnostics panel is on the show either way, so it covers the name
    /// whatever the console is doing — the console relocates modals, not F3.
    #[test]
    fn the_console_does_not_rescue_the_name_from_the_panel() {
        use console::Console;

        assert!(!preset_name_visible(
            output_modal(Some(Modal::Browse), Console::Open),
            true,
            true
        ));
    }
}
