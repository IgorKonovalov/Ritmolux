//! The settings modal's state — a **second pure state machine** beside
//! [`overlay`](crate::overlay), same shape and same discipline: window-free,
//! renderer-free, config-free, so every rule below is unit-testable without
//! winit or a GPU (Plan 0050 Phase 4).
//!
//! It owns a highlighted row and nothing else. The values it *shows* arrive each
//! frame in a [`SettingsView`] the shell fills in, and the changes it *asks for*
//! leave as a [`SettingsAction`] the shell executes. That is what keeps this
//! module from holding a `Renderer`, a `Window` or a `Config` — and what makes
//! the dwell clamps and the row/action mapping assertable as values.
//!
//! # Why not one `ui` module shared with the browser
//!
//! The two modals' rows mean genuinely different things: the browser is
//! pick-one-and-close over a filtered roster, this is edit-a-value-in-place over
//! a fixed list. Merging them would rewrite a green module to share an
//! up/down/wrap of about ten lines. They agree where it matters — both wrap
//! vertically — and that agreement is asserted here rather than inherited.

use lmv_core::render::Tier;

/// Dwell edit step, in seconds. Coarse on purpose: this is a live-show control
/// operated by eye, not a scheduler.
pub const DWELL_STEP: u32 = 5;
/// Floor for the minimum dwell. Below this a rotation reads as a glitch rather
/// than a change — the ~1 s dissolve alone would be a fifth of the dwell.
pub const DWELL_FLOOR: u32 = 5;
/// Ceiling for the maximum dwell (15 minutes). Not a policy, just a stop: `Right`
/// held on an unbounded counter is a way to make the row meaningless.
pub const DWELL_CEILING: u32 = 900;

/// How the active tier came to be what it is — the suffix on the Quality row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TierState {
    /// The engine resolved it and the governor may still demote it.
    Auto,
    /// Explicitly pinned — at launch (`--tier`, `LMV_TIER`, `[quality] tier`) or
    /// by this menu / the `[` `]` keys.
    Pinned,
    /// The frame-time governor took it down. Distinguished from `Pinned` because
    /// ADR-0045 requires a demotion never be silent.
    Demoted,
}

impl TierState {
    fn suffix(self) -> &'static str {
        match self {
            TierState::Auto => "(auto)",
            TierState::Pinned => "(pinned)",
            TierState::Demoted => "(demoted)",
        }
    }
}

/// The live values the rows display, read off the shell each time they are drawn
/// or edited. Passing them in rather than holding them is what makes this module
/// pure.
#[derive(Clone, Debug)]
pub struct SettingsView {
    pub tier: Tier,
    pub tier_state: TierState,
    pub auto_rotate: bool,
    pub min_dwell_secs: u32,
    pub max_dwell_secs: u32,
    pub fullscreen: bool,
    /// Zero-based index of the operator-selected display, and how many there are.
    pub display_index: usize,
    pub display_count: usize,
    pub display_name: String,
    pub diagnostics: bool,
    /// The resolved preset directory — shown, never edited.
    pub preset_dir: String,
}

/// A key the settings modal reacts to, decoded from the platform upstream so this
/// module stays free of winit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingsKey {
    /// Open when closed, close when open (`S`).
    Toggle,
    /// Previous row, wrapping.
    Up,
    /// Next row, wrapping.
    Down,
    /// Decrease / toggle the highlighted row's value.
    Left,
    /// Increase / toggle the highlighted row's value.
    Right,
    /// Close without further change (`Esc`).
    Escape,
}

impl SettingsKey {
    /// Whether OS key repeat is honoured for this key — the same rule the browse
    /// overlay uses, and for the same reason.
    ///
    /// **`Left`/`Right` are included and that is a deliberate difference from the
    /// browser**, where they only move a cursor. Here they change a value, and
    /// holding one to walk the dwell from 20 s to 90 s in 5 s steps is the
    /// interaction an operator expects. Every action a repeat can reach is either
    /// idempotent (a tier already pinned, see the shell's guard) or a bounded
    /// counter; `Toggle` and `Escape` are excluded so a held `S` cannot strobe
    /// the modal.
    pub fn is_nav(self) -> bool {
        matches!(
            self,
            SettingsKey::Up | SettingsKey::Down | SettingsKey::Left | SettingsKey::Right
        )
    }
}

/// What the shell should do after a key reaches the settings modal. The state
/// machine decides *what changes*; the shell owns every effect.
#[derive(Clone, Debug, PartialEq)]
pub enum SettingsAction {
    /// The key was ignored (a nav key while closed, or a read-only row).
    None,
    /// Visible state changed; redraw.
    Redraw,
    /// Close the modal.
    Close,
    /// Close settings and open the browse overlay (`Tab`) — one modal at a time.
    OpenBrowse,
    SetTier(Tier),
    ToggleAuto,
    /// Both bounds, already clamped against each other and the floor/ceiling, so
    /// the shell writes them without re-deciding anything.
    SetDwell {
        min_secs: u32,
        max_secs: u32,
    },
    ToggleFullscreen,
    CycleDisplay,
    ToggleDiagnostics,
}

/// The rows, in display order. Exhaustive and ordered here so the labels, the
/// values and the key mapping cannot disagree about what row 3 is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingsRow {
    Quality,
    AutoRotate,
    MinDwell,
    MaxDwell,
    Fullscreen,
    Display,
    Diagnostics,
    Presets,
}

impl SettingsRow {
    /// Every row, in display order.
    pub const ALL: [SettingsRow; 8] = [
        SettingsRow::Quality,
        SettingsRow::AutoRotate,
        SettingsRow::MinDwell,
        SettingsRow::MaxDwell,
        SettingsRow::Fullscreen,
        SettingsRow::Display,
        SettingsRow::Diagnostics,
        SettingsRow::Presets,
    ];

    fn label(self) -> &'static str {
        match self {
            SettingsRow::Quality => "Quality",
            SettingsRow::AutoRotate => "Auto-rotate",
            SettingsRow::MinDwell => "Min dwell",
            SettingsRow::MaxDwell => "Max dwell",
            SettingsRow::Fullscreen => "Fullscreen",
            SettingsRow::Display => "Display",
            SettingsRow::Diagnostics => "Diagnostics",
            SettingsRow::Presets => "Presets",
        }
    }

    /// This row's current value, rendered for display.
    fn value(self, view: &SettingsView) -> String {
        let on_off = |b: bool| if b { "on" } else { "off" };
        match self {
            SettingsRow::Quality => format!(
                "{} {}",
                view.tier.as_str().to_uppercase(),
                view.tier_state.suffix()
            ),
            SettingsRow::AutoRotate => on_off(view.auto_rotate).to_owned(),
            SettingsRow::MinDwell => format!("{} s", view.min_dwell_secs),
            SettingsRow::MaxDwell => format!("{} s", view.max_dwell_secs),
            SettingsRow::Fullscreen => on_off(view.fullscreen).to_owned(),
            SettingsRow::Display => {
                // 1-based for the operator; the config's index stays 0-based.
                format!(
                    "{} of {} - {}",
                    view.display_index + 1,
                    view.display_count.max(1),
                    view.display_name
                )
            }
            SettingsRow::Diagnostics => on_off(view.diagnostics).to_owned(),
            SettingsRow::Presets => view.preset_dir.clone(),
        }
    }

    /// The action `Left` (or `Right`, when `right`) on this row asks for.
    fn edit(self, right: bool, view: &SettingsView) -> SettingsAction {
        match self {
            // A switch, not a cycle: `[`/`]`'s orientation, floor on the left.
            SettingsRow::Quality => {
                SettingsAction::SetTier(if right { Tier::Rich } else { Tier::Floor })
            }
            SettingsRow::AutoRotate => SettingsAction::ToggleAuto,
            SettingsRow::MinDwell => {
                let min = step(view.min_dwell_secs, right).clamp(DWELL_FLOOR, view.max_dwell_secs);
                SettingsAction::SetDwell {
                    min_secs: min,
                    max_secs: view.max_dwell_secs,
                }
            }
            SettingsRow::MaxDwell => {
                let max =
                    step(view.max_dwell_secs, right).clamp(view.min_dwell_secs, DWELL_CEILING);
                SettingsAction::SetDwell {
                    min_secs: view.min_dwell_secs,
                    max_secs: max,
                }
            }
            SettingsRow::Fullscreen => SettingsAction::ToggleFullscreen,
            SettingsRow::Display => SettingsAction::CycleDisplay,
            SettingsRow::Diagnostics => SettingsAction::ToggleDiagnostics,
            // Read-only: it tells you where presets are loaded from, which is a
            // launch-time resolution (`LMV_PRESET_DIR`, then the per-user dir),
            // not a thing a menu can move.
            SettingsRow::Presets => SettingsAction::None,
        }
    }
}

/// One `DWELL_STEP` up or down, saturating rather than wrapping — the caller
/// clamps into the row's real range afterwards.
fn step(secs: u32, up: bool) -> u32 {
    if up {
        secs.saturating_add(DWELL_STEP)
    } else {
        secs.saturating_sub(DWELL_STEP)
    }
}

/// The settings modal: open/closed plus the highlighted row.
#[derive(Clone, Debug, Default)]
pub struct SettingsState {
    open: bool,
    row: usize,
}

impl SettingsState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    /// The highlighted row's index into [`SettingsRow::ALL`].
    pub fn row(&self) -> usize {
        self.row
    }

    /// Close without emitting an action — the shell's route when `Tab` takes over
    /// or another modal claims the screen.
    pub fn close(&mut self) {
        self.open = false;
    }

    /// The lines to draw, as `(label, value)` in [`SettingsRow::ALL`] order. The
    /// shell adds the highlight marker using [`row`](Self::row), exactly as it
    /// does for the browse list.
    pub fn lines(&self, view: &SettingsView) -> Vec<(&'static str, String)> {
        SettingsRow::ALL
            .iter()
            .map(|r| (r.label(), r.value(view)))
            .collect()
    }

    /// Feed one key; mutate state and report what the shell should do.
    pub fn handle_key(&mut self, key: SettingsKey, view: &SettingsView) -> SettingsAction {
        if key == SettingsKey::Toggle {
            self.open = !self.open;
            if self.open {
                self.row = 0;
            }
            return SettingsAction::Redraw;
        }
        if !self.open {
            return SettingsAction::None;
        }
        match key {
            SettingsKey::Up => {
                self.step_row(false);
                SettingsAction::Redraw
            }
            SettingsKey::Down => {
                self.step_row(true);
                SettingsAction::Redraw
            }
            SettingsKey::Left | SettingsKey::Right => {
                let Some(&row) = SettingsRow::ALL.get(self.row) else {
                    return SettingsAction::None;
                };
                row.edit(key == SettingsKey::Right, view)
            }
            SettingsKey::Escape => {
                self.open = false;
                SettingsAction::Close
            }
            SettingsKey::Toggle => SettingsAction::None, // handled above
        }
    }

    /// Move the highlight one row, **wrapping** — the same way the browse overlay
    /// does, deliberately: two modals in one app that disagree about what the end
    /// of a list means is worse than either choice.
    fn step_row(&mut self, down: bool) {
        let last = SettingsRow::ALL.len() - 1;
        self.row = if down {
            if self.row >= last { 0 } else { self.row + 1 }
        } else if self.row == 0 {
            last
        } else {
            self.row - 1
        };
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DWELL_CEILING, DWELL_FLOOR, DWELL_STEP, SettingsAction, SettingsKey, SettingsRow,
        SettingsState, SettingsView, Tier, TierState,
    };

    fn view() -> SettingsView {
        SettingsView {
            tier: Tier::Rich,
            tier_state: TierState::Auto,
            auto_rotate: false,
            min_dwell_secs: 20,
            max_dwell_secs: 90,
            fullscreen: false,
            display_index: 1,
            display_count: 3,
            display_name: "DELL U2720Q".to_owned(),
            diagnostics: false,
            preset_dir: r"C:\Users\x\AppData\Roaming\light-music-visualizer\presets".to_owned(),
        }
    }

    fn opened() -> SettingsState {
        let mut s = SettingsState::new();
        s.handle_key(SettingsKey::Toggle, &view());
        assert!(s.is_open());
        s
    }

    /// Move the highlight to `row` and press Left or Right there.
    fn edit_at(row: SettingsRow, right: bool, v: &SettingsView) -> SettingsAction {
        let mut s = opened();
        let target = SettingsRow::ALL
            .iter()
            .position(|r| *r == row)
            .expect("row is in ALL");
        for _ in 0..target {
            s.handle_key(SettingsKey::Down, v);
        }
        assert_eq!(s.row(), target);
        s.handle_key(
            if right {
                SettingsKey::Right
            } else {
                SettingsKey::Left
            },
            v,
        )
    }

    /// **Every row emits the action the plan's table names**, both directions.
    /// This is the mapping the shell executes blind, so a row silently wired to
    /// the wrong effect is invisible everywhere else.
    #[test]
    fn each_row_emits_the_action_its_table_row_names() {
        let v = view();
        assert_eq!(
            edit_at(SettingsRow::Quality, false, &v),
            SettingsAction::SetTier(Tier::Floor)
        );
        assert_eq!(
            edit_at(SettingsRow::Quality, true, &v),
            SettingsAction::SetTier(Tier::Rich)
        );
        for right in [false, true] {
            assert_eq!(
                edit_at(SettingsRow::AutoRotate, right, &v),
                SettingsAction::ToggleAuto
            );
            assert_eq!(
                edit_at(SettingsRow::Fullscreen, right, &v),
                SettingsAction::ToggleFullscreen
            );
            assert_eq!(
                edit_at(SettingsRow::Display, right, &v),
                SettingsAction::CycleDisplay
            );
            assert_eq!(
                edit_at(SettingsRow::Diagnostics, right, &v),
                SettingsAction::ToggleDiagnostics
            );
        }
        assert_eq!(
            edit_at(SettingsRow::MinDwell, true, &v),
            SettingsAction::SetDwell {
                min_secs: 25,
                max_secs: 90
            }
        );
        assert_eq!(
            edit_at(SettingsRow::MaxDwell, false, &v),
            SettingsAction::SetDwell {
                min_secs: 20,
                max_secs: 85
            }
        );
    }

    /// **The read-only row is read-only.** It shows where presets are resolved
    /// from, which is a launch-time decision a menu cannot move.
    #[test]
    fn the_presets_row_emits_nothing() {
        let v = view();
        assert_eq!(
            edit_at(SettingsRow::Presets, false, &v),
            SettingsAction::None
        );
        assert_eq!(
            edit_at(SettingsRow::Presets, true, &v),
            SettingsAction::None
        );
    }

    /// **The dwell clamp holds from both sides**, which is the property that
    /// stops the pair inverting — a `min` above `max` would make the director's
    /// window empty and rotation undefined.
    #[test]
    fn the_dwell_bounds_cannot_cross_or_go_below_the_floor() {
        // Raising min past max pins it at max.
        let mut v = view();
        v.min_dwell_secs = v.max_dwell_secs;
        assert_eq!(
            edit_at(SettingsRow::MinDwell, true, &v),
            SettingsAction::SetDwell {
                min_secs: v.max_dwell_secs,
                max_secs: v.max_dwell_secs
            },
            "min climbed past max"
        );

        // Lowering max below min pins it at min.
        let mut v = view();
        v.max_dwell_secs = v.min_dwell_secs;
        assert_eq!(
            edit_at(SettingsRow::MaxDwell, false, &v),
            SettingsAction::SetDwell {
                min_secs: v.min_dwell_secs,
                max_secs: v.min_dwell_secs
            },
            "max sank below min"
        );

        // Min never goes below its floor, including from exactly the floor and
        // from inside one step of it.
        for start in [DWELL_FLOOR, DWELL_FLOOR + 1, 0] {
            let mut v = view();
            v.min_dwell_secs = start;
            let SettingsAction::SetDwell { min_secs, .. } =
                edit_at(SettingsRow::MinDwell, false, &v)
            else {
                panic!("min dwell must emit SetDwell");
            };
            assert!(
                min_secs >= DWELL_FLOOR,
                "min fell to {min_secs} from {start}, under the {DWELL_FLOOR} s floor"
            );
        }

        // Max is bounded above, so holding Right cannot make the row meaningless.
        let mut v = view();
        v.max_dwell_secs = DWELL_CEILING;
        assert_eq!(
            edit_at(SettingsRow::MaxDwell, true, &v),
            SettingsAction::SetDwell {
                min_secs: v.min_dwell_secs,
                max_secs: DWELL_CEILING
            }
        );

        // Non-vacuity: an ordinary edit in the middle of the range does move,
        // by exactly one step — or every assertion above holds for a no-op.
        let v = view();
        assert_eq!(
            edit_at(SettingsRow::MinDwell, true, &v),
            SettingsAction::SetDwell {
                min_secs: v.min_dwell_secs + DWELL_STEP,
                max_secs: v.max_dwell_secs
            }
        );
    }

    /// Rows wrap the same way the browse overlay's do.
    #[test]
    fn the_row_highlight_wraps_like_the_browser() {
        let v = view();
        let mut s = opened();
        let last = SettingsRow::ALL.len() - 1;

        assert_eq!(s.row(), 0);
        s.handle_key(SettingsKey::Up, &v);
        assert_eq!(s.row(), last, "Up off the first row did not wrap");
        s.handle_key(SettingsKey::Down, &v);
        assert_eq!(s.row(), 0, "Down off the last row did not wrap");

        // A full lap returns to the start.
        for _ in 0..SettingsRow::ALL.len() {
            s.handle_key(SettingsKey::Down, &v);
        }
        assert_eq!(s.row(), 0);
    }

    /// Closed, everything but `Toggle` is inert, so the shell's own bindings keep
    /// working — the same contract the browse overlay has.
    #[test]
    fn keys_are_inert_while_closed() {
        let v = view();
        let mut s = SettingsState::new();
        for key in [
            SettingsKey::Up,
            SettingsKey::Down,
            SettingsKey::Left,
            SettingsKey::Right,
            SettingsKey::Escape,
        ] {
            assert_eq!(s.handle_key(key, &v), SettingsAction::None);
            assert!(!s.is_open());
        }
        assert_eq!(
            s.handle_key(SettingsKey::Toggle, &v),
            SettingsAction::Redraw
        );
        assert!(s.is_open());
        assert_eq!(s.handle_key(SettingsKey::Escape, &v), SettingsAction::Close);
        assert!(!s.is_open());
    }

    /// Reopening starts at the top rather than where it was left, and the row
    /// list is what the shell draws.
    #[test]
    fn reopening_returns_to_the_first_row() {
        let v = view();
        let mut s = opened();
        s.handle_key(SettingsKey::Down, &v);
        s.handle_key(SettingsKey::Down, &v);
        assert_eq!(s.row(), 2);
        s.handle_key(SettingsKey::Toggle, &v); // close
        s.handle_key(SettingsKey::Toggle, &v); // reopen
        assert_eq!(s.row(), 0);
    }

    /// The drawn lines cover every row, in order, and carry the values the view
    /// holds — including the tier suffix, which is the one thing on this menu
    /// that reports engine state rather than a config field.
    #[test]
    fn the_lines_show_every_row_with_its_current_value() {
        let mut v = view();
        let s = opened();
        let lines = s.lines(&v);
        assert_eq!(lines.len(), SettingsRow::ALL.len());

        let find = |label: &str, lines: &[(&'static str, String)]| -> String {
            lines
                .iter()
                .find(|(l, _)| *l == label)
                .map(|(_, val)| val.clone())
                .unwrap_or_else(|| panic!("no {label} row"))
        };
        assert_eq!(find("Quality", &lines), "RICH (auto)");
        assert_eq!(find("Min dwell", &lines), "20 s");
        assert_eq!(find("Max dwell", &lines), "90 s");
        assert_eq!(find("Auto-rotate", &lines), "off");
        // 1-based for the operator, 0-based in the config.
        assert_eq!(find("Display", &lines), "2 of 3 - DELL U2720Q");

        // The three tier states are distinguishable, which is what stops a
        // governed demotion reading as a deliberate pin (ADR-0045).
        for (state, expected) in [
            (TierState::Auto, "RICH (auto)"),
            (TierState::Pinned, "RICH (pinned)"),
            (TierState::Demoted, "RICH (demoted)"),
        ] {
            v.tier_state = state;
            assert_eq!(find("Quality", &s.lines(&v)), expected);
        }
    }

    /// Repeat reaches the value-editing keys here but not the modal keys — a
    /// deliberate difference from the browser, where `Left`/`Right` only move a
    /// cursor.
    #[test]
    fn repeat_reaches_the_editing_keys_but_not_the_modal_ones() {
        for key in [
            SettingsKey::Up,
            SettingsKey::Down,
            SettingsKey::Left,
            SettingsKey::Right,
        ] {
            assert!(key.is_nav(), "{key:?} should accept key repeat");
        }
        for key in [SettingsKey::Toggle, SettingsKey::Escape] {
            assert!(!key.is_nav(), "{key:?} must not accept key repeat");
        }
    }
}
