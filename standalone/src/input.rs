//! Keyboard and pointer routing.
//!
//! One rule decides everything here: whichever surface owns the keyboard sees a
//! key first, and while a menu is open every key it does not claim is
//! **swallowed** rather than falling through. `AppState::modal` is the single
//! place that fact is decided, so routing and drawing cannot disagree about
//! which menu is on screen; [`standalone::keymap`] is the single place a key is
//! given a meaning, so the dispatch and the help sheet cannot disagree about it.

use winit::event::KeyEvent;
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{Key as LogicalKey, PhysicalKey};

use std::time::Instant;

use crate::app_state::{AppState, Trail};
use crate::console;
use crate::hud::Modal;
use crate::overlay::{OverlayAction, OverlayKey};
use crate::settings::{SettingsAction, SettingsKey};
use rlx_core::render::Tier;
use standalone::keymap::{self, Action, Ctx, Press};
use standalone::marks::Mark;

/// How close two left-button presses have to be to read as a double-click
/// (fullscreen toggle) rather than two separate clicks.
pub(crate) const DOUBLE_CLICK: std::time::Duration = std::time::Duration::from_millis(400);

/// The press a key event carries: its physical key, and the one character the
/// layout made of it — which is what `?` is matched by.
fn press_of(event: &KeyEvent) -> Press {
    let code = match event.physical_key {
        PhysicalKey::Code(code) => Some(code),
        PhysicalKey::Unidentified(_) => None,
    };
    let ch = match &event.logical_key {
        LogicalKey::Character(s) => {
            let mut chars = s.chars();
            chars.next().filter(|_| chars.next().is_none())
        }
        _ => None,
    };
    Press { code, ch }
}

impl AppState {
    /// Which surface owns the keyboard, as the keymap names it.
    pub(crate) fn key_context(&self) -> Ctx {
        match self.modal() {
            Some(Modal::Help) => Ctx::Help,
            Some(Modal::Settings) => Ctx::Settings,
            Some(Modal::Browse) => Ctx::Browse,
            None => Ctx::Show,
        }
    }

    /// Route a pressed key through [`keymap::KEYMAP`].
    ///
    /// The row the key fires in the current context is the whole decision. A
    /// key no row claims is swallowed, except in the browser, where a printable
    /// character narrows the type-to-filter query — that is the one binding not
    /// keyed to a key, and it has a row of its own for the help sheet.
    ///
    /// **OS key repeat fires a row only when the row says so** (Plan 0050 Phase
    /// 2): moving a cursor and stepping a value. A held `Space` would
    /// machine-gun preset switches through a ~1 s dissolve each, and a held `F`
    /// would thrash fullscreen.
    pub(crate) fn handle_key(&mut self, event_loop: &ActiveEventLoop, event: &KeyEvent) {
        let ctx = self.key_context();
        let press = press_of(event);
        match keymap::lookup(ctx, &press) {
            Some(row) => {
                if event.repeat && !row.repeat {
                    return;
                }
                self.dispatch(ctx, row.action, press, event_loop);
            }
            None if ctx == Ctx::Browse && !event.repeat => {
                if let Some(text) = &event.text {
                    self.filter_browser(text);
                }
            }
            None => {}
        }
    }

    /// Carry out one row's action, pressed in `ctx`.
    fn dispatch(&mut self, ctx: Ctx, action: Action, press: Press, event_loop: &ActiveEventLoop) {
        match action {
            Action::NextPreset => {
                // Manual next scene: reset the director's dwell so the auto
                // timer restarts from this moment.
                self.show.director.force_next();
                self.rotate_to_next();
            }
            Action::PreviousPreset => self.step_previous(),
            // `1`-`9` land on the first nine favourites, in the browser's own
            // order.
            Action::SelectFavourite => {
                if let Some(nth) = press.code.and_then(keymap::favourite_slot) {
                    self.select_favourite(nth);
                }
            }
            Action::AbCompare => self.flip_ab(),
            // One binding marks the highlighted row inside the browser and the
            // preset on screen outside it (ADR-0228).
            Action::MarkFavourite => self.toggle_mark(Mark::Favourite),
            Action::MarkHidden => self.toggle_mark(Mark::Hidden),
            Action::ToggleAuto => self.toggle_auto_rotate(),
            Action::ToggleOrder => self.toggle_rotate_order(),
            Action::CycleSource => self.cycle_rotate_source(),
            // Quality, live (ADR-0054).
            Action::TierFloor => self.swap_tier(Tier::Floor),
            Action::TierRich => self.swap_tier(Tier::Rich),
            Action::Fullscreen => self.toggle_fullscreen(),
            // Windowed it does nothing, and it **never quits**: one stray
            // keypress ending a running show is the failure mode this binding
            // is worth avoiding (Plan 0096 Phase 2).
            Action::LeaveFullscreen => {
                if self.window.fullscreen().is_some() {
                    self.toggle_fullscreen();
                }
            }
            Action::CycleDisplay => self.cycle_display(),
            Action::Diagnostics => self.toggle_diagnostics(),
            Action::OpenSettings => {
                // One of the two refresh points (the other is a mode change).
                // Enumeration is COM, so it happens on the keypress that makes
                // the roster visible, not on the frames that draw it. The
                // adapter roster is refreshed on the same keypress for the
                // same reason: it builds a graphics instance.
                self.refresh_input_roster();
                self.refresh_adapter_roster();
                let view = self.settings_view();
                let action = self.hud.settings.handle_key(SettingsKey::Toggle, &view);
                self.apply_settings_action(action);
            }
            Action::Console => self.toggle_console(event_loop, true),
            // The sheet lists the context it was opened from, and closing it
            // hands the keyboard back to that context's menu, still open under
            // it.
            Action::Help => {
                self.hud.help = Some(ctx);
                self.window.request_redraw();
            }
            Action::CloseHelp => {
                self.hud.help = None;
                self.window.request_redraw();
            }
            Action::Browse(key) => self.browse_key(key),
            // Reached only through the typed text; see `handle_key`.
            Action::Filter => {}
            Action::Settings(key) => {
                let view = self.settings_view();
                let action = self.hud.settings.handle_key(key, &view);
                self.apply_settings_action(action);
            }
            // Hands over rather than stacking: one menu at a time.
            Action::SettingsToBrowse => self.apply_settings_action(SettingsAction::OpenBrowse),
        }
    }

    /// Feed one key to the browser's state machine and act on what it says.
    fn browse_key(&mut self, key: OverlayKey) {
        let names = self.roster_names();
        let rows = self.browse_rows(&names);
        let active = self.renderer.active_index();
        let layout = self.list_layout(self.hud.browse.visible(&rows).len());
        match self.hud.browse.handle_key(key, &rows, active, &layout) {
            OverlayAction::None => return,
            OverlayAction::Redraw | OverlayAction::Close => {}
            OverlayAction::Select(index) => {
                let incoming = self.renderer.select_preset(index).to_owned();
                self.on_preset_selected(&incoming);
            }
        }
        self.window.request_redraw();
    }

    /// Narrow the open browser by the printable characters of `text`.
    fn filter_browser(&mut self, text: &str) {
        let names = self.roster_names();
        let rows = self.browse_rows(&names);
        let active = self.renderer.active_index();
        let layout = self.list_layout(self.hud.browse.visible(&rows).len());
        let mut changed = false;
        for c in text
            .chars()
            .filter(|c| !c.is_control() && !c.is_whitespace())
        {
            self.hud
                .browse
                .handle_key(OverlayKey::Char(c), &rows, active, &layout);
            changed = true;
        }
        if changed {
            self.window.request_redraw();
        }
    }

    /// The pointer moved over the show's window: bring the launch hint back,
    /// when `[ui] hints` wants it.
    pub(crate) fn pointer_moved(&mut self) {
        if self.config.ui.hints {
            self.hud.hint_secs = standalone::motion::HINT_SECS;
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
        let action = console::action_for(button, &self.settings_view());
        self.apply_console_action(action);
    }

    /// Carry out a resolved [`console::ConsoleAction`].
    ///
    /// Split from the click handler above because the console's pointer is no
    /// longer its only source: `ctl/transport` (ADR-0176) resolves the **same**
    /// `ConsoleAction` and lands here, so a verb sent over the wire and a click
    /// on the strip are one code path rather than two that agree today.
    pub(crate) fn apply_console_action(&mut self, action: console::ConsoleAction) {
        match action {
            console::ConsoleAction::Next => self.rotate_to_next(),
            // One `prev`, whichever surface asked — the strip, the wire and the
            // `Backspace` key — so the three cannot mean three things.
            console::ConsoleAction::Prev => self.step_previous(),
            console::ConsoleAction::Random => {
                let count = self.renderer.preset_names().count();
                let seed = self.next_random();
                if let Some(index) =
                    console::random_index(count, self.renderer.active_index(), seed)
                {
                    let incoming = self.renderer.select_preset(index).to_owned();
                    self.on_preset_selected(&incoming);
                }
            }
            // The director's own reset comes with it, so the dwell restarts from
            // this moment exactly as a hotkey rotation does.
            console::ConsoleAction::RotateNow => {
                self.show.director.force_next();
                self.rotate_to_next();
            }
            console::ConsoleAction::Settings(action) => self.apply_settings_action(action),
        }
    }

    /// Drain the control-in queue and apply one frame's worth of it (ADR-0176).
    ///
    /// A `None` listener costs a branch, which is the whole of what an
    /// unconfigured run pays for this feature existing.
    ///
    /// **The window's half is the transport verbs, and only those.** Everything
    /// else in a drained frame is applied by the show, so the two run modes
    /// cannot disagree about it. A verb is resolved here because it resolves the
    /// operator console's own action against the live settings view — one
    /// applier for a click and for the wire, which is what spec 0003 requires —
    /// and a settings view needs a window.
    ///
    /// The order the two halves run in is load-bearing: a preset switch drops
    /// every override, so transport has to precede the values the show applies.
    /// The verbs are moved out of `self` for the duration rather than borrowed
    /// from it, because every applier below takes `&mut self`.
    pub(crate) fn apply_control(&mut self) {
        let mut verbs = std::mem::take(&mut self.control_transports);
        if self.show.take_control_transports(&mut verbs) {
            for verb in &verbs {
                let view = self.settings_view();
                let auto = self.show.director.auto_enabled();
                if let Some(action) = console::action_for_transport(*verb, auto, &view) {
                    self.apply_console_action(action);
                }
            }
            if self.show.apply_control_rest(&mut self.renderer) {
                self.on_preset_switched(Trail::Record);
            }
        }
        self.control_transports = verbs;
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
