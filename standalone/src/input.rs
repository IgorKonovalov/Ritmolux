//! Keyboard and pointer routing.
//!
//! One rule decides everything here: whichever modal owns the keyboard sees a
//! key first, and while it is open every key it does not claim is **swallowed**
//! rather than falling through. `AppState::modal` is the single place that fact
//! is decided, so routing and drawing cannot disagree about which menu is on
//! screen.

use winit::event::KeyEvent;
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{KeyCode, PhysicalKey};

use std::time::Instant;

use crate::app_state::AppState;
use crate::console;
use crate::hud::Modal;
use crate::overlay::{OverlayAction, OverlayKey};
use crate::settings::{SettingsAction, SettingsKey};
use rlx_core::render::Tier;

/// How close two left-button presses have to be to read as a double-click
/// (fullscreen toggle) rather than two separate clicks.
pub(crate) const DOUBLE_CLICK: std::time::Duration = std::time::Duration::from_millis(400);

/// Map a physical key to the settings modal's abstract key, or `None` for keys
/// the modal does not own (which are then swallowed while it is open).
pub(crate) fn decode_settings_key(code: KeyCode) -> Option<SettingsKey> {
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
pub(crate) fn decode_overlay_key(code: KeyCode) -> Option<OverlayKey> {
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

impl AppState {
    /// Route a pressed key. Overlay control keys (toggle / nav / enter / esc /
    /// backspace) go through its state machine first; while the overlay is open,
    /// printable characters narrow the type-to-filter query and every other key
    /// is swallowed. When it is closed, non-overlay keys fall through to the
    /// shell's own bindings — Space-cycle and the F3 diagnostics toggle.
    pub(crate) fn handle_key(&mut self, event_loop: &ActiveEventLoop, event: &KeyEvent) {
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
            let action = self.hud.settings.handle_key(key, &view);
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
        if event.repeat
            && !(self.hud.browse.is_open() && overlay_key.is_some_and(OverlayKey::is_nav))
        {
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
            let layout = self.list_layout(self.hud.browse.visible(&refs).len());
            match self.hud.browse.handle_key(key, &refs, active, &layout) {
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
        if self.hud.browse.is_open() {
            if let Some(text) = &event.text {
                let name_refs = self.roster_names();
                let refs: Vec<&str> = name_refs.iter().map(String::as_str).collect();
                let active = self.renderer.active_index();
                let layout = self.list_layout(self.hud.browse.visible(&refs).len());
                let mut changed = false;
                for c in text
                    .chars()
                    .filter(|c| !c.is_control() && !c.is_whitespace())
                {
                    self.hud
                        .browse
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
                self.rotate_to_next();
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
                if !self.hud.settings.is_open() {
                    self.refresh_input_roster();
                }
                let view = self.settings_view();
                let action = self.hud.settings.handle_key(SettingsKey::Toggle, &view);
                self.apply_settings_action(action);
            }
            KeyCode::KeyF => self.toggle_fullscreen(),
            KeyCode::KeyD => self.cycle_display(),
            // The operator console. Out here only, like `S`: while the browser
            // is open `C` is a filter character and the branch above has already
            // returned.
            // Not on repeat: holding the key would flap a window open and shut
            // rather than scroll a list, and creating a swapchain per repeat is
            // the most expensive thing any binding here can do.
            KeyCode::KeyC if !event.repeat => self.toggle_console(event_loop, true),
            // Quality, live (ADR-0054). `[` down a tier, `]` up — the bracket
            // pair reads as a range with the floor on the left.
            KeyCode::BracketLeft => self.swap_tier(Tier::Floor),
            KeyCode::BracketRight => self.swap_tier(Tier::Rich),
            _ => {}
        }
    }

    /// One step of the console's `random` control, returning the seed for this
    /// press.
    ///
    /// **lowbias32**, the bit-mixer `core` already uses for deterministic
    /// pseudo-randomness — one round of it, so consecutive counter values do not
    /// produce neighbouring roster positions the way a bare increment would.
    pub(crate) fn next_random(&mut self) -> u32 {
        let mut x = self.hud.random_state.wrapping_add(0x9E37_79B9);
        self.hud.random_state = x;
        x ^= x >> 16;
        x = x.wrapping_mul(0x21F0_AAAD);
        x ^= x >> 15;
        x = x.wrapping_mul(0x735A_2D97);
        x ^= x >> 15;
        x
    }

    /// A left press on the console: resolve it against the transport strip and
    /// act, or ignore it.
    ///
    /// **The console is never a second source of truth.** A control that the
    /// settings menu also offers is carried here as that menu's own action and
    /// handed to the same applier the menu's keys reach, so the two surfaces
    /// cannot drift into two behaviours (`console::action_for`).
    pub(crate) fn handle_console_press(&mut self) {
        let Some(window) = self.hud.console_window.as_ref() else {
            return;
        };
        // Not drawn under a modal, so not clickable under one either: an
        // invisible control that still fires is worse than no control.
        if self.modal().is_some() {
            return;
        }
        let size = window.inner_size();
        let (x, y) = self.hud.console_cursor;
        let Some(button) = console::hit_test(size.width as f32, size.height as f32, x, y) else {
            return;
        };
        match console::action_for(button, &self.settings_view()) {
            console::ConsoleAction::Next => self.rotate_to_next(),
            console::ConsoleAction::Prev => {
                let count = self.renderer.preset_names().count();
                if let Some(index) = console::previous_index(count, self.renderer.active_index()) {
                    self.renderer.select_preset(index);
                    self.on_preset_switched();
                }
            }
            console::ConsoleAction::Random => {
                let count = self.renderer.preset_names().count();
                let seed = self.next_random();
                if let Some(index) =
                    console::random_index(count, self.renderer.active_index(), seed)
                {
                    self.renderer.select_preset(index);
                    self.on_preset_switched();
                }
            }
            // The director's own reset comes with it, so the dwell restarts from
            // this moment exactly as a hotkey rotation does.
            console::ConsoleAction::RotateNow => {
                self.director.force_next();
                self.rotate_to_next();
            }
            console::ConsoleAction::Settings(action) => self.apply_settings_action(action),
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
    pub(crate) fn handle_left_press(&mut self) {
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
}
