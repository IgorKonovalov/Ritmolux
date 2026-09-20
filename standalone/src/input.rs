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

use crate::app_state::{AppState, Trail};
use crate::console;
use crate::hud::Modal;
use crate::overlay::{OverlayAction, OverlayKey};
use crate::settings::{SettingsAction, SettingsKey};
use rlx_core::render::Tier;
use standalone::marks::Mark;

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

/// The favourite `1`-`9` selects, as a zero-based position, or `None` for every
/// other key.
///
/// The **top row and the numpad both**, because a key labelled `3` means `3`
/// wherever it is on the board; `0` is deliberately not in the set, since a
/// tenth slot would have to be either "the tenth" (reading `0` as ten) or a
/// hole, and nine slots with no ambiguity is the better of the three.
pub(crate) fn favourite_slot(code: KeyCode) -> Option<usize> {
    Some(match code {
        KeyCode::Digit1 | KeyCode::Numpad1 => 0,
        KeyCode::Digit2 | KeyCode::Numpad2 => 1,
        KeyCode::Digit3 | KeyCode::Numpad3 => 2,
        KeyCode::Digit4 | KeyCode::Numpad4 => 3,
        KeyCode::Digit5 | KeyCode::Numpad5 => 4,
        KeyCode::Digit6 | KeyCode::Numpad6 => 5,
        KeyCode::Digit7 | KeyCode::Numpad7 => 6,
        KeyCode::Digit8 | KeyCode::Numpad8 => 7,
        KeyCode::Digit9 | KeyCode::Numpad9 => 8,
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
        // The three narrowings. Function keys because **letters and digits are
        // filter input while the browser is open** and this app binds no
        // modifier combination anywhere — so `Ctrl`-anything would be new input
        // plumbing rather than a free choice, and a letter would be swallowed by
        // the query. `F3` was already a toggle, so the row is established.
        KeyCode::F4 => OverlayKey::FavouritesOnly,
        KeyCode::F5 => OverlayKey::Family,
        KeyCode::F6 => OverlayKey::ShowHidden,
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

        // A step backwards through what was actually shown, intercepted here for
        // `Escape`'s reason: `decode_overlay_key` maps `Backspace`
        // unconditionally, so with the browser **closed** the dispatch below
        // would land on `OverlayAction::None => return` and this would never
        // reach the shell's own match. With the browser open it stays the filter
        // key it has always been, because the dispatch runs first.
        if code == KeyCode::Backspace && self.modal().is_none() {
            self.step_previous();
            return;
        }

        // Marking, before the overlay dispatch so the **same key** works with
        // the browser open and closed (ADR-0228). A function key is never a
        // filter character, which is what lets one binding serve both contexts
        // without introducing modifier handling this app has nowhere else.
        if code == KeyCode::F1 {
            self.toggle_mark(Mark::Favourite);
            return;
        }
        if code == KeyCode::F2 {
            self.toggle_mark(Mark::Hidden);
            return;
        }

        if let Some(key) = overlay_key {
            let names = self.roster_names();
            let rows = self.browse_rows(&names);
            let active = self.renderer.active_index();
            let layout = self.list_layout(self.hud.browse.visible(&rows).len());
            match self.hud.browse.handle_key(key, &rows, active, &layout) {
                OverlayAction::None => return, // closed + non-toggle: let it fall away
                OverlayAction::Redraw | OverlayAction::Close => {}
                OverlayAction::Select(index) => {
                    self.renderer.select_preset(index);
                    self.on_preset_switched(Trail::Record);
                }
            }
            self.window.request_redraw();
            return;
        }

        // While open, printable characters filter the list; anything else is
        // consumed so it can't reach Space-cycle / F3.
        if self.hud.browse.is_open() {
            if let Some(text) = &event.text {
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
            return;
        }

        match code {
            KeyCode::Space => {
                // Manual next scene: reset the director's dwell so the auto
                // timer restarts from this moment.
                self.show.director.force_next();
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
            // A/B compare. Out here only, like `S`, `C` and `F`: while the
            // browser is open `b` is a filter character and the branch above
            // has already returned.
            KeyCode::KeyB => self.flip_ab(),
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
            // `1`-`9` land on the first nine favourites, in the browser's own
            // order. Digits are filter characters while the browser is open,
            // exactly as letters are, so this binding applies outside it — the
            // branch above has already returned by here.
            other => {
                if let Some(nth) = favourite_slot(other) {
                    self.select_favourite(nth);
                }
            }
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
                    self.renderer.select_preset(index);
                    self.on_preset_switched(Trail::Record);
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

#[cfg(test)]
mod tests {
    use super::{decode_overlay_key, favourite_slot};
    use crate::overlay::OverlayKey;
    use winit::keyboard::KeyCode;

    /// **Nine slots, both number rows, and nothing else.** A key past the ninth
    /// must not wrap onto a favourite: a key that means a different preset
    /// depending on how many are marked is worse than a key that means nothing.
    #[test]
    fn the_number_keys_map_to_nine_slots_and_no_more() {
        let digits = [
            KeyCode::Digit1,
            KeyCode::Digit2,
            KeyCode::Digit3,
            KeyCode::Digit4,
            KeyCode::Digit5,
            KeyCode::Digit6,
            KeyCode::Digit7,
            KeyCode::Digit8,
            KeyCode::Digit9,
        ];
        for (nth, code) in digits.into_iter().enumerate() {
            assert_eq!(favourite_slot(code), Some(nth), "{code:?}");
        }
        // The numpad's `3` is a `3`, wherever it is on the board.
        assert_eq!(favourite_slot(KeyCode::Numpad3), Some(2));
        assert_eq!(favourite_slot(KeyCode::Numpad9), Some(8));

        assert_eq!(
            favourite_slot(KeyCode::Digit0),
            None,
            "`0` would be a tenth slot or a hole; nine with no ambiguity is better"
        );
        for code in [KeyCode::KeyB, KeyCode::Space, KeyCode::F1, KeyCode::Minus] {
            assert_eq!(favourite_slot(code), None, "{code:?} is not a slot");
        }
    }

    /// The three narrowing keys reach the browser and **no letter does**, which
    /// is the binding constraint the whole set was chosen under: letters and
    /// digits are filter input while it is open.
    #[test]
    fn the_browser_keys_are_function_keys_and_never_letters() {
        assert_eq!(
            decode_overlay_key(KeyCode::F4),
            Some(OverlayKey::FavouritesOnly)
        );
        assert_eq!(decode_overlay_key(KeyCode::F5), Some(OverlayKey::Family));
        assert_eq!(
            decode_overlay_key(KeyCode::F6),
            Some(OverlayKey::ShowHidden)
        );

        for code in [
            KeyCode::KeyF,
            KeyCode::KeyH,
            KeyCode::KeyS,
            KeyCode::KeyB,
            KeyCode::Digit1,
        ] {
            assert_eq!(
                decode_overlay_key(code),
                None,
                "{code:?} would be swallowed as a filter character"
            );
        }

        // `F1` and `F2` are deliberately **not** overlay keys: the shell
        // intercepts them before the dispatch so one binding marks the
        // highlighted row inside the browser and the preset on screen outside
        // it.
        assert_eq!(decode_overlay_key(KeyCode::F1), None);
        assert_eq!(decode_overlay_key(KeyCode::F2), None);
    }
}
