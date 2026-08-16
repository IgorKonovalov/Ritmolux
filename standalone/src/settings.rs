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
    /// Whether the corner preset name is drawn at all (`[hud] preset_name`).
    pub preset_name: bool,
    /// Whether a track change announces itself (`[hud] now_playing`).
    pub now_playing: bool,
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
    /// Show or hide the corner preset name, persisted (Plan 0096 Phase 3).
    TogglePresetName,
    /// Announce track changes or not, persisted (Plan 0097 Phase 3).
    ToggleNowPlaying,
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
    PresetName,
    NowPlaying,
    Presets,
}

impl SettingsRow {
    /// Every row, in display order. The one read-only row stays last.
    pub const ALL: [SettingsRow; 10] = [
        SettingsRow::Quality,
        SettingsRow::AutoRotate,
        SettingsRow::MinDwell,
        SettingsRow::MaxDwell,
        SettingsRow::Fullscreen,
        SettingsRow::Display,
        SettingsRow::Diagnostics,
        // Beside the preset name: both are `[hud]` keys about what the shell
        // paints over the show, and an operator clearing the canvas wants them
        // in one place.
        SettingsRow::PresetName,
        SettingsRow::NowPlaying,
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
            SettingsRow::PresetName => "Preset name",
            SettingsRow::NowPlaying => "Now playing",
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
            SettingsRow::PresetName => on_off(view.preset_name).to_owned(),
            SettingsRow::NowPlaying => on_off(view.now_playing).to_owned(),
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
            SettingsRow::PresetName => SettingsAction::TogglePresetName,
            SettingsRow::NowPlaying => SettingsAction::ToggleNowPlaying,
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
mod tests;
