#[cfg(target_os = "macos")]
mod capture_mac;
mod capture_verdict;
#[cfg(windows)]
mod capture_win;
mod config;
mod diaglog;
mod director;
mod downbeatlog;
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
use lmv_core::render::{CapOverflow, Renderer, RendererOptions, TextRun, Tier};
use overlay::{LIST_INSET, LIST_TOP, OverlayAction, OverlayKey, OverlayState, ROW_H, ROW_SIZE};
use settings::{SettingsAction, SettingsKey, SettingsState, SettingsView, TierState};
use soak::SoakLog;
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
    /// The startup capture verdict, already rendered to its one-line token
    /// (Plan 0083). Built once and borrowed by both durable surfaces — the
    /// `diagnostics.log` `capture` column and the F3 overlay — so the two can
    /// never disagree about the same run.
    capture_token: String,
    scratch: Vec<f32>,
    occluded: bool,
    /// Frames since the last title refresh (title shows core-sourced fps + p99).
    title_tick: u32,
    /// Whether the diagnostics debug overlay is currently painted (toggled by F3).
    overlay_on: bool,
    /// The preset browse overlay's modal state (Tab toggles; Plan 0008).
    browse: OverlayState,
    /// The settings modal's state (`S` toggles; Plan 0050 Phase 4). A second,
    /// independent pure state machine — see [`settings`] for why it is not the
    /// same one.
    settings: SettingsState,
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
    frame_texts: Vec<String>,
    /// `(x, y, size, color)` parallel to [`Self::frame_texts`].
    frame_text_meta: Vec<(f32, f32, f32, [f32; 4])>,
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

        let capture = start_capture(&config.input);
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
            scratch: vec![0.0; 32_768],
            occluded: false,
            title_tick: 0,
            overlay_on: false,
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
            last_click: None,
            pending_switch_settle: false,
            // Seeded from what `reload_presets` already printed above, so the
            // frame loop does not re-announce the startup preset's truncation.
            reported_overflow: renderer_overflow,
            reported_demotion: false,
            frame_texts: Vec::new(),
            frame_text_meta: Vec::new(),
            config,
            config_path,
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

        // Queue the on-canvas text for this frame (active name + browse list).
        self.queue_frame_text();

        if let Err(err) = self.renderer.render(&frame, dt) {
            eprintln!("render error: {err}");
        }
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

    /// Toggle auto-rotate and **persist it** — the one path for both the `A`
    /// hotkey and the settings row.
    ///
    /// The hotkey used to change the director and print to stderr without
    /// writing `[rotate] auto`, unlike `F` and `D` which both persist. That left
    /// the two controls able to disagree: set it with `A`, restart, and the
    /// config's value came back. One path is what makes that impossible rather
    /// than merely fixed.
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
            preset_name: self.config.hud.preset_name,
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
            // Persisted, unlike diagnostics: a clean canvas is a staging choice,
            // not a debugging state, so it should survive the restart.
            SettingsAction::TogglePresetName => {
                self.config.hud.preset_name = !self.config.hud.preset_name;
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
        let mut texts = std::mem::take(&mut self.frame_texts);
        let mut meta = std::mem::take(&mut self.frame_text_meta);
        texts.clear();
        meta.clear();

        if preset_name_visible(self.modal(), self.overlay_on, self.config.hud.preset_name) {
            texts.push(self.renderer.preset_name().to_owned());
            meta.push((NAME_INSET, NAME_INSET, NAME_SIZE, NAME_COLOR));
        }

        // The capture verdict, under the core's diagnostics panel and only while
        // it is up (Plan 0083). Built from the stored token rather than from the
        // capture state, so this line and the log's `capture` column are the same
        // sentence about the same run.
        if self.overlay_on {
            texts.push(overlay::capture_line(&self.capture_token));
            meta.push((
                NAME_INSET,
                overlay::CAPTURE_TOP,
                overlay::CAPTURE_SIZE,
                overlay::CAPTURE_COLOR,
            ));
        }

        if self.modal() == Some(Modal::Settings) {
            let view = self.settings_view();
            texts.push("settings  -  up/down  left/right  esc".to_owned());
            meta.push((LIST_INSET, LIST_TOP, ROW_SIZE, HEADER_COLOR));

            // One column, always: nine rows fit any window this app opens in —
            // they start at `ROWS_TOP` (94 px) with a 30 px pitch, so the last
            // ends at 364 px — and a settings menu that reflowed would move a row
            // out from under the operator's hand mid-edit.
            for (row, (label, value)) in self.settings.lines(&view).into_iter().enumerate() {
                let y = overlay::ROWS_TOP + row as f32 * ROW_H;
                let (marker, color) = if row == self.settings.row() {
                    ("> ", ROW_HL_COLOR)
                } else {
                    ("  ", ROW_COLOR)
                };
                texts.push(format!("{marker}{label:<14}{value}"));
                meta.push((LIST_INSET, y, ROW_SIZE, color));
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
            texts.push(header);
            meta.push((LIST_INSET, LIST_TOP, ROW_SIZE, HEADER_COLOR));

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
                texts.push(format!("{marker}{}", overlay::fit(name)));
                meta.push((x, y, ROW_SIZE, color));
            }
        }

        let runs: Vec<TextRun<'_>> = texts
            .iter()
            .zip(meta.iter())
            .map(|(t, &(x, y, size, color))| TextRun {
                text: t.as_str(),
                x,
                y,
                size,
                color,
            })
            .collect();
        self.renderer.queue_text(&runs);

        // `runs` borrows `texts`, so the buffers can only go home once its last
        // use is behind us.
        drop(runs);
        self.frame_texts = texts;
        self.frame_text_meta = meta;
    }

    /// Route a pressed key. Overlay control keys (toggle / nav / enter / esc /
    /// backspace) go through its state machine first; while the overlay is open,
    /// printable characters narrow the type-to-filter query and every other key
    /// is swallowed. When it is closed, non-overlay keys fall through to the
    /// shell's own bindings — Space-cycle and the F3 diagnostics toggle.
    fn handle_key(&mut self, event: &KeyEvent) {
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
        // Phase 2). The event loop used to drop every repeat before it got here,
        // which is why holding an arrow in the browser did nothing. Widening that
        // to "all keys" is what must not happen: a held `Space` would machine-gun
        // preset switches through a ~1 s dissolve each, and a held `F` would
        // thrash fullscreen. So the gate is here, where the key's role is known,
        // rather than at the event site, where it is not.
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
                let view = self.settings_view();
                let action = self.settings.handle_key(SettingsKey::Toggle, &view);
                self.apply_settings_action(action);
            }
            KeyCode::KeyF => self.toggle_fullscreen(),
            KeyCode::KeyD => self.cycle_display(),
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
}

/// The format both platform arms fall back to when capture fails, so the analyzer
/// has something valid to start on. **The verdict never reports it** — a log
/// stating a format nothing is delivering is worse than no log.
const FALLBACK_FORMAT: AudioFormat = AudioFormat {
    sample_rate: 48_000,
    channels: 2,
};

#[cfg(windows)]
fn start_capture(input: &config::Input) -> CaptureStart {
    let selector = capture_win::CaptureSelector {
        mode: match input.mode {
            config::InputMode::Loopback => capture_win::CaptureMode::Loopback,
            config::InputMode::LineIn => capture_win::CaptureMode::LineIn,
        },
        // "default" (or empty) means the mode's default endpoint — no name match.
        device: (!input.device.trim().is_empty() && !input.device.eq_ignore_ascii_case("default"))
            .then(|| input.device.clone()),
    };
    match capture_win::start(&selector) {
        Ok((handle, consumer)) => {
            let format = handle.format();
            CaptureStart {
                handle: Some(handle),
                consumer: Some(consumer),
                format,
                verdict: CaptureVerdict::Live {
                    backend: "WASAPI",
                    format,
                },
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
                verdict: CaptureVerdict::failed("WASAPI", err),
            }
        }
    }
}

#[cfg(target_os = "macos")]
fn start_capture(_input: &config::Input) -> CaptureStart {
    match capture_mac::start() {
        Ok((handle, consumer)) => {
            let format = handle.format();
            CaptureStart {
                handle: Some(handle),
                consumer: Some(consumer),
                format,
                verdict: CaptureVerdict::Live {
                    backend: "SCK",
                    format,
                },
            }
        }
        Err(err) => {
            eprintln!("ScreenCaptureKit capture unavailable ({err}); rendering without audio");
            CaptureStart {
                handle: None,
                consumer: None,
                format: FALLBACK_FORMAT,
                verdict: CaptureVerdict::failed("SCK", err),
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

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(state) = self.state.as_mut() else {
            if let WindowEvent::CloseRequested = event {
                event_loop.exit();
            }
            return;
        };
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
                state.handle_key(&event);
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
/// too: the preset still loads and renders, but the mistake is no longer silent
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
/// per the plan; other platforms note that device selection isn't wired there.
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

    let mut app = App {
        config,
        config_path,
        soak_path,
        downbeat_log_path,
        tier,
        state: None,
    };
    if let Err(err) = event_loop.run_app(&mut app) {
        eprintln!("event loop error: {err}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::{Modal, preset_name_visible};

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
}
