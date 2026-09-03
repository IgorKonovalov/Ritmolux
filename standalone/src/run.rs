//! The launch path and the winit application handler.
//!
//! [`App`] is what exists before a window does: the config plus every flag this
//! launch resolved, held beside it rather than written into it, so a flag pins
//! one run without persisting itself into the file the operator keeps. `resumed`
//! turns that into the [`AppState`] the show runs on.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use rlx_core::render::{AdapterChoice, Tier};
use standalone::osc::OscSink;
use standalone::{AppDirMigration, migrate_app_dir, resolve_tier, tier_env};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::monitor::MonitorHandle;
use winit::window::{Fullscreen, Window, WindowId};

use crate::app_state::{APP_TITLE, AppState, HIDDEN_TICK};
use crate::capture_start::list_devices_and_exit;
use crate::cli::{
    InputSource, missing_companion, parse_console_flag, parse_downbeat_log_arg, parse_input_args,
    parse_osc_arg, parse_soak_arg, parse_tier_arg, print_help, resolve_config_path, resolve_input,
    resolve_osc, unrecognized_flag, valued_valueless_flag, windowed_flag,
};
use crate::config::{self, Config};
use crate::console;
use crate::preset_dir::startup_preset_names;
use crate::stream;

pub(crate) struct App {
    /// Loaded once at startup; the window is created from it on `resumed` and
    /// it is then handed to the `AppState` for live edits + persistence.
    pub(crate) config: Config,
    pub(crate) config_path: Option<PathBuf>,
    /// Soak-log path from `--soak`, or `None` when the mode is off.
    pub(crate) soak_path: Option<PathBuf>,
    /// Per-beat downbeat-log path from `--downbeat-log`, or `None` when the mode
    /// is off (Plan 0086 Phase 1).
    pub(crate) downbeat_log_path: Option<PathBuf>,
    /// The quality-tier pin, already resolved across `--tier` / `RLX_TIER` /
    /// config (Plan 0044). `None` is auto — rich, governed.
    pub(crate) tier: Option<Tier>,
    /// Which adapter `--gpu` named, already resolved.
    /// [`AdapterChoice::Default`] is the unflagged request and is exactly what
    /// the window asked for before the flag could reach it (ADR-0155). Held
    /// beside `config` like the other per-launch flags, so it pins this run
    /// without persisting itself.
    pub(crate) adapter: AdapterChoice,
    /// The preset `--preset` holds for this run, already checked against the
    /// roster this launch will load. `None` rotates on the operator's config.
    pub(crate) held_preset: Option<String>,
    /// The capture selection, already resolved across `--input` / `--device` /
    /// `[input]` (Plan 0130). Held beside `config` rather than written into it,
    /// so a flag pins this launch without persisting itself on the next save.
    pub(crate) input: config::Input,
    /// The telemetry sink, already bound in `main` — so a bad target is a
    /// startup error rather than a window that opens and then reports one.
    /// `None` when the sink is off.
    pub(crate) osc: Option<OscSink>,
    /// `--console` was passed. Held beside `config` rather than written into it,
    /// so the flag opens the console for this launch without persisting itself
    /// — the same shape `--input` / `--device` / `--osc` follow (ADR-0142).
    pub(crate) console_flag: bool,
    pub(crate) state: Option<AppState>,
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
                    let mut state = AppState::new(Arc::new(window), self, display_index);
                    state.window.request_redraw();
                    // `--console` and `[console] enabled` are one path with the
                    // hotkey and the settings row: both set the same pending
                    // flag, which `service_console_request` turns into the same
                    // `toggle_console` call. Nothing here opens a window
                    // directly, so no two routes can disagree about whether the
                    // console is open.
                    // Opened without persisting: `[console] enabled` is already
                    // what it is, and `--console` must not write itself into the
                    // file. An operator's own toggle is the only thing that
                    // changes the stored value.
                    if state.config.console.enabled || self.console_flag {
                        state.hud.console_request = Some(false);
                        state.service_console_request(event_loop);
                    }
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
            state.hud.console_window.as_ref().map(|w| w.id()).as_ref(),
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
                // `!is_synthetic` is load-bearing here, not tidiness. winit
                // replays the currently-held keys at a window that gains focus,
                // and this window gains focus the instant `C` creates it - while
                // `C` is still physically down. Handling that replay ran the
                // toggle a second time and the console closed a few milliseconds
                // after it opened.
                WindowEvent::KeyboardInput {
                    event,
                    is_synthetic: false,
                    ..
                } if event.state == ElementState::Pressed => {
                    state.handle_key(event_loop, &event);
                }
                // The transport strip. Position is tracked on move rather than
                // read at the press, because winit's `MouseInput` carries no
                // coordinates.
                WindowEvent::CursorMoved { position, .. } => {
                    state.hud.console_cursor = (position.x as f32, position.y as f32);
                }
                WindowEvent::CursorLeft { .. } => {
                    // Off the surface entirely: park the cursor somewhere no
                    // control can claim, so a press arriving after the pointer
                    // left cannot land on a stale rectangle.
                    state.hud.console_cursor = (-1.0, -1.0);
                }
                WindowEvent::MouseInput {
                    state: ElementState::Pressed,
                    button: MouseButton::Left,
                    ..
                } => state.handle_console_press(),
                _ => {}
            }
            // The console's own S-menu can ask to close the console. Serviced
            // after the event rather than inside it, because the handler that
            // decided is several frames down a call chain with no event loop.
            state.service_console_request(event_loop);
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
            // Synthetic events are the focus-change key-state replay, never a
            // press the operator made; no binding wants them (see the console
            // arm above, where handling them closed the window on open).
            WindowEvent::KeyboardInput {
                event,
                is_synthetic: false,
                ..
            } if event.state == ElementState::Pressed => {
                state.handle_key(event_loop, &event);
            }
            _ => {}
        }
        // The show window's S-menu asks for the console the same way the
        // console's own does; one servicing point per window arm keeps the row,
        // the hotkey and the launch flag on one path.
        state.service_console_request(event_loop);
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

/// Pick the monitor for the configured output, returning its index in
/// `monitors` and a handle. A stored *name* wins over the raw index (winit's
/// monitor ordering isn't stable across boot/hotplug — plan Risks); an
/// out-of-range index falls back to the first monitor. `None` only when no
/// monitors are enumerated at all.
pub(crate) fn resolve_monitor(
    monitors: &[MonitorHandle],
    output: &config::Output,
) -> Option<(usize, MonitorHandle)> {
    resolve_display(monitors, output.display_name.as_deref(), output.display)
}

/// The name-over-index rule itself, over the two fields any display-bearing
/// config section carries.
///
/// `[output]` and `[console]` both name a display and both fall back to an
/// index, and they must not answer that question two different ways: winit's
/// monitor ordering is not stable across boot or hotplug, so a stored index
/// alone can point at the wrong screen, and a second implementation of the
/// fallback is a second set of edge cases. An index past the end degrades to
/// the first monitor rather than to nothing — a console is better on the wrong
/// screen than absent.
pub(crate) fn resolve_display(
    monitors: &[MonitorHandle],
    name: Option<&str>,
    index: usize,
) -> Option<(usize, MonitorHandle)> {
    if monitors.is_empty() {
        return None;
    }
    if let Some(name) = name
        && let Some((at, monitor)) = monitors
            .iter()
            .enumerate()
            .find(|(_, m)| m.name().as_deref() == Some(name))
    {
        return Some((at, monitor.clone()));
    }
    if let Some(monitor) = monitors.get(index) {
        return Some((index, monitor.clone()));
    }
    monitors.first().map(|monitor| (0, monitor.clone()))
}

/// Print every graphics adapter, from **both** rosters, and exit (ADR-0146).
///
/// Both, because they are separate enumerations that are never assumed to agree
/// on order: the renderer selects through wgpu and the Spout sender through the
/// sender API, and `--gpu` is resolved against each independently. Printing them
/// side by side is also the only way to see whether the two describe the same
/// GPU with the same string, which is what the no-flag default rests on.
pub(crate) fn list_adapters_and_exit() {
    let renderer_roster = rlx_core::render::list_adapters();
    if renderer_roster.is_empty() {
        eprintln!("renderer (wgpu): no adapters enumerated");
    } else {
        eprintln!("renderer (wgpu):");
        for (index, adapter) in renderer_roster.iter().enumerate() {
            eprintln!("  [{index}] {}", adapter.detail);
        }
    }

    #[cfg(all(feature = "spout", windows))]
    {
        let sender_roster = standalone::spout::adapters();
        if sender_roster.is_empty() {
            eprintln!("spout sender: no adapters enumerated");
        } else {
            eprintln!("spout sender:");
            for (index, name) in sender_roster.iter().enumerate() {
                eprintln!("  [{index}] {name}");
            }
        }
        eprintln!();
        eprintln!(
            "pass --gpu with a name from either list (or an index) to put the renderer and \
             the sender on one GPU. The sender's adapter decides whether a receiver can open \
             the stream at all; the renderer's decides how fast it runs."
        );
    }
    #[cfg(not(all(feature = "spout", windows)))]
    {
        eprintln!();
        eprintln!("built without the 'spout' feature, so there is no sender roster to list.");
    }
}

pub fn run() {
    // Ahead of the roster gate: someone asking what the flags are is the one
    // caller to answer rather than refuse, so `--help` wins over a typo sharing
    // the command line with it.
    if std::env::args()
        .skip(1)
        .any(|arg| arg == "--help" || arg == "-h")
    {
        print_help();
        return;
    }

    // Refuse an argument no scanner will claim, before any scanner runs
    // (ADR-0148). Without this the app starts normally while doing less than it
    // was asked to, which on a show floor presents as a cable or controller
    // fault rather than as a typo. Exit 2 is what this file already uses for an
    // argument list that is wrong in shape, against 1 for a recognized flag
    // whose value or effect failed.
    if let Some((arg, nearest)) = unrecognized_flag(std::env::args().skip(1)) {
        eprintln!("unrecognized argument `{arg}`");
        if let Some(spec) = nearest {
            eprintln!("did you mean `{}`? {}", spec.name, spec.help);
        }
        std::process::exit(2);
    }

    // A valueless flag given a value, refused for the same reason and in the
    // same shape. Ahead of the companion check because it is the cause and that
    // one would report the symptom: `--stream=1 --fps 30` has a well-formed
    // `--fps` and a `--stream` that will not be seen, so blaming `--fps` sends
    // the operator to the wrong token.
    if let Some(spec) = valued_valueless_flag(std::env::args().skip(1)) {
        eprintln!(
            "`{}` takes no value, so `{}=...` is read by nothing",
            spec.name, spec.name
        );
        eprintln!("drop the `=` and its value: `{}`", spec.name);
        std::process::exit(2);
    }

    // A rostered flag whose companion is absent, refused for the same reason
    // and in the same shape (ADR-0155). Second, because a misspelt flag is the
    // worse diagnosis and should be the one reported: `--fpz 30` is an
    // unrecognized argument, not `--fps` missing `--stream`.
    if let Some((spec, companion)) = missing_companion(std::env::args().skip(1)) {
        eprintln!(
            "`{}` is only read with `{companion}`, which is not in this command line",
            spec.name
        );
        eprintln!("either add `{companion}` or drop `{}`", spec.name);
        std::process::exit(2);
    }

    // Carry a per-user directory left under the earlier name across to
    // APP_DIR_NAME, before anything reads or seeds it. It runs here rather than
    // inside the resolver so that resolving a path never moves a directory, and
    // ahead of the `--list-*` aids so that even a run that exits early leaves
    // the machine in the migrated state.
    match migrate_app_dir() {
        AppDirMigration::Moved { from, to } => {
            eprintln!("moved {} to {}", from.display(), to.display());
        }
        AppDirMigration::BothPresent { legacy } => {
            eprintln!(
                "{} also exists and is not being read; delete it once you have \
                 anything you want from it",
                legacy.display()
            );
        }
        AppDirMigration::Failed { from, error } => {
            eprintln!(
                "could not move {}: {error}; continuing with a fresh directory, \
                 the old one is untouched",
                from.display()
            );
        }
        AppDirMigration::NotNeeded => {}
    }

    // Startup aid: print the enumerable audio endpoints and exit, so the
    // operator can copy a friendly name into `input.device` (Plan 0009 Phase 2).
    if std::env::args().skip(1).any(|arg| arg == "--list-devices") {
        list_devices_and_exit();
        return;
    }

    // The same aid for GPUs, and it answers a sharper question than a
    // preference: on a hybrid machine the Spout sender's adapter decides
    // whether a receiver can open the stream at all (ADR-0146).
    if std::env::args().skip(1).any(|arg| arg == "--list-adapters") {
        list_adapters_and_exit();
        return;
    }

    // The headless live source, decided BEFORE the event loop exists: this mode
    // has no window and must not create one, so it cannot be a branch taken
    // later inside the app (ADR-0125).
    let argv: Vec<String> = std::env::args().skip(1).collect();
    match stream::parse(&argv) {
        Ok(Some(request)) => {
            let config = resolve_config_path()
                .as_deref()
                .map(Config::load)
                .unwrap_or_default();
            if let Err(message) = stream::run(&request, &config.input, &config.rotate) {
                eprintln!("{message}");
                std::process::exit(1);
            }
            return;
        }
        Ok(None) => {}
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(2);
        }
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

    // Quality tier, highest precedence first: `--tier`, `RLX_TIER`, config
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

    // Resolved before the event loop exists, so a `--gpu` with no value is a
    // usage error rather than a window that opens and then reports one — the
    // shape `--tier` and `--osc` already follow.
    let adapter = match windowed_flag("--gpu") {
        Ok(wanted) => standalone::gpu::window_choice(wanted.as_deref()),
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(2);
        }
    };

    // Judged here, against the set this launch will actually hold, so an
    // unknown name costs no window (ADR-0155). Rotation stays on the operator's
    // config when the flag is absent.
    let held_preset = match windowed_flag("--preset") {
        Ok(None) => None,
        Ok(Some(name)) => {
            let roster = startup_preset_names();
            if !roster.contains(&name) {
                eprintln!("--preset `{name}`: no preset by that name");
                eprintln!("this launch holds {} preset(s): {}", roster.len(), {
                    let mut sorted = roster;
                    sorted.sort();
                    sorted.join(", ")
                });
                std::process::exit(2);
            }
            Some(name)
        }
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(2);
        }
    };

    let mut app = App {
        config,
        config_path,
        soak_path,
        downbeat_log_path,
        tier,
        adapter,
        held_preset,
        input,
        osc,
        console_flag: parse_console_flag(),
        state: None,
    };
    if let Err(err) = event_loop.run_app(&mut app) {
        eprintln!("event loop error: {err}");
        std::process::exit(1);
    }
}
