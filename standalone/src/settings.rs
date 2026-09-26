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
//! A few *value types* do cross the seam — [`Tier`], [`InputMode`],
//! [`GridScale`] and [`GridScaleChoice`] — because the rows carrying them hold
//! a value `config.toml` names, and a row that re-spelled the value could
//! disagree with what the file holds. `Tier` and `GridScale` are core types a
//! config key happens to name. All are `Copy`, not the `Config` struct, so
//! nothing here reads or writes a file.
//!
//! # Why not one `ui` module shared with the browser
//!
//! The two modals' rows mean genuinely different things: the browser is
//! pick-one-and-close over a filtered roster, this is edit-a-value-in-place over
//! a fixed list. Merging them would rewrite a green module to share an
//! up/down/wrap of about ten lines. They agree where it matters — both wrap
//! vertically — and that agreement is asserted here rather than inherited.

use rlx_core::render::{GridScale, Tier};

use standalone::config::{GridScaleChoice, InputMode, RotateOrder, RotateSource};

/// The fixed values the Grid scale row steps through, smallest first; `auto`
/// sits past the top. Quarters, because a finer step is not a difference an
/// operator judges by eye, and every one of them is exact in binary, so the
/// file holds `0.75` and not a float's widening of it.
pub const GRID_SCALE_STEPS: [f32; 4] = [0.25, 0.5, 0.75, 1.0];

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
    /// Explicitly pinned — at launch (`--tier`, `RLX_TIER`, `[quality] tier`) or
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
    /// The grid scale the renderer resolved (ADR-0245) — what is on screen.
    pub grid_scale: GridScale,
    /// What decided it: `auto`, or the fixed value `[quality] grid_scale`, the
    /// flag or the environment variable named. The row steps from this, so a
    /// press moves the choice rather than the number `auto` happened to pick.
    pub grid_scale_choice: GridScaleChoice,
    pub auto_rotate: bool,
    /// The order rotation walks the library in (`[rotate] order`).
    pub rotate_order: RotateOrder,
    /// Which part of the library rotation draws from (`[rotate] source`).
    pub rotate_source: RotateSource,
    /// Whether any preset the show could draw is marked a favourite. Read by the
    /// source row alone: `Favourites` falls back to the whole eligible set while
    /// nothing is marked, so the row has to report the state the show is in
    /// rather than the word the config holds.
    pub favourites_marked: bool,
    pub min_dwell_secs: u32,
    pub max_dwell_secs: u32,
    pub fullscreen: bool,
    /// Zero-based index of the operator-selected display, and how many there are.
    pub display_index: usize,
    pub display_count: usize,
    pub display_name: String,
    pub diagnostics: bool,
    /// Which capture path is running (`[input] mode`).
    pub input_mode: InputMode,
    /// Position of the running endpoint in the shell's cached roster, and how
    /// big that roster is. A count of zero means there is no roster — an
    /// enumeration that failed and a dataflow with genuinely no active endpoint
    /// arrive here identically, and neither may make the row panic or move.
    pub input_device_index: usize,
    pub input_device_count: usize,
    pub input_device_name: String,
    /// Whether the two input rows can be *moved*. False on macOS and Linux,
    /// where the capture path takes no operator selection: the rows still
    /// render, so the menu keeps one shape everywhere and the value stays
    /// visible, but `edit` yields nothing — the treatment `Presets` already has.
    pub input_editable: bool,
    /// Whether the corner preset name is drawn at all (`[hud] preset_name`).
    pub preset_name: bool,
    /// Whether a track change announces itself (`[hud] now_playing`).
    pub now_playing: bool,
    /// Whether the countdown to the next auto-rotate is drawn
    /// (`[hud] next_rotation`).
    pub next_rotation: bool,
    /// Whether the operator console is open right now (ADR-0143). The live
    /// window state, not the config key: the row reports what is on screen, so
    /// a console opened by `--console` or the `C` hotkey reads correctly here.
    pub console: bool,
    /// Position of the running graphics adapter in the shell's cached roster,
    /// and how big that roster is (ADR-0246). A count under two means there is
    /// nowhere to move to — one adapter, or an enumeration that failed — and
    /// the row then renders and goes inert rather than asking the shell to
    /// switch onto a list that is not there.
    pub adapter_index: usize,
    pub adapter_count: usize,
    /// The running adapter's own name, as the renderer describes it.
    pub adapter_name: String,
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
    /// Set the grid scale — a fixed fraction or `auto` — and persist it as
    /// `[quality] grid_scale` (ADR-0245, ADR-0240). The value is already
    /// stepped here, so the shell writes and applies it without re-deciding.
    SetGridScale(GridScaleChoice),
    ToggleAuto,
    /// Switch the order rotation draws in. A switch rather than a toggle, for
    /// the reason the tier row is one: the value a key produces does not depend
    /// on the value it is on, so key repeat cannot walk it back and forth.
    SetOrder(RotateOrder),
    /// Switch which part of the library rotation draws from, on the same rule.
    SetSource(RotateSource),
    /// Both bounds, already clamped against each other and the floor/ceiling, so
    /// the shell writes them without re-deciding anything.
    SetDwell {
        min_secs: u32,
        max_secs: u32,
    },
    ToggleFullscreen,
    CycleDisplay,
    ToggleDiagnostics,
    /// Switch the capture path. A switch rather than a toggle, for the reason
    /// the tier row is one: the value a key produces does not depend on the
    /// value it is on, so key repeat cannot walk it back and forth.
    SetInputMode(InputMode),
    /// Advance to the next endpoint in the shell's cached roster, wrapping.
    /// Directionless, like [`CycleDisplay`](SettingsAction::CycleDisplay).
    CycleInputDevice,
    /// Show or hide the corner preset name, persisted (Plan 0096 Phase 3).
    TogglePresetName,
    /// Announce track changes or not, persisted (Plan 0097 Phase 3).
    ToggleNowPlaying,
    /// Draw the countdown to the next auto-rotate or not, persisted.
    ToggleNextRotation,
    /// Open or close the operator console (ADR-0143). The state machine says
    /// only that it changed; the shell owns the window.
    ToggleConsole,
    /// Move the running show onto the adapter at this position in the shell's
    /// cached roster, and persist it (ADR-0246). The position is already
    /// stepped and wrapped here, so the shell switches without re-deciding
    /// anything; `Left` walks down the roster and `Right` up it, wrapping at
    /// both ends like the row highlight does.
    SetAdapter(usize),
}

/// The rows, in display order. Exhaustive and ordered here so the labels, the
/// values and the key mapping cannot disagree about what row 3 is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingsRow {
    Quality,
    GridScale,
    Adapter,
    AutoRotate,
    Order,
    Source,
    MinDwell,
    MaxDwell,
    Fullscreen,
    Display,
    Diagnostics,
    InputMode,
    InputDevice,
    PresetName,
    NowPlaying,
    NextRotation,
    Console,
    Presets,
}

impl SettingsRow {
    /// Every row, in display order. The one read-only row stays last.
    pub const ALL: [SettingsRow; 18] = [
        SettingsRow::Quality,
        // Directly under the tier, because it is resolved from the tier and
        // qualifies it: `RICH` at 0.50 is a different picture from `RICH` at
        // 1.00, and the two read as one line of thought (ADR-0245).
        SettingsRow::GridScale,
        // Beside the tier: both decide what the machine spends on the picture,
        // both rebuild the GPU state when moved, and an operator whose show is
        // slow looks at the two together (ADR-0246).
        SettingsRow::Adapter,
        SettingsRow::AutoRotate,
        // Immediately after the switch that turns rotation on and before the
        // dwell pair, so the four rotation rows read together: whether it
        // rotates, in what order, out of what, and how often.
        SettingsRow::Order,
        SettingsRow::Source,
        SettingsRow::MinDwell,
        SettingsRow::MaxDwell,
        SettingsRow::Fullscreen,
        SettingsRow::Display,
        SettingsRow::Diagnostics,
        // The pair stays adjacent and in this order: the mode decides which
        // roster the device row indexes, so reading them the other way round
        // describes an endpoint list that does not exist yet.
        SettingsRow::InputMode,
        SettingsRow::InputDevice,
        // Beside the preset name: both are `[hud]` keys about what the shell
        // paints over the show, and an operator clearing the canvas wants them
        // in one place.
        SettingsRow::PresetName,
        SettingsRow::NowPlaying,
        // The third `[hud]` key, beside its two siblings for their reason.
        SettingsRow::NextRotation,
        // After the three paint switches and before the read-only row: the
        // console is also about what the operator sees rather than about the
        // show, but it opens a window rather than changing the canvas.
        SettingsRow::Console,
        SettingsRow::Presets,
    ];

    /// The `config.toml` path this row edits, as `section.key`, or `None` for a
    /// row that edits nothing.
    ///
    /// **Exhaustive on purpose** (ADR-0240): a setting is defined by a key in a
    /// user-editable file and the menu is an editor of that file, so a new row
    /// cannot be added without answering which key holds its value. The tests
    /// beside this module assert that every declared path resolves in a
    /// serialised config and that its leaf key is named in
    /// `docs/configuration.md`, which is the same property
    /// `standalone/tests/suite/configuration_doc.rs` asserts for the schema,
    /// reached from the menu side instead.
    ///
    /// A row whose change reaches the file through more than one key names the
    /// one its **value** is — `Display` names `output.display`, and the
    /// `display_name` the same write keeps in step is the index's identity
    /// rather than a second choice.
    pub(crate) fn config_path(self) -> Option<&'static str> {
        match self {
            SettingsRow::Quality => Some("quality.tier"),
            SettingsRow::GridScale => Some("quality.grid_scale"),
            SettingsRow::Adapter => Some("output.gpu"),
            SettingsRow::AutoRotate => Some("rotate.auto"),
            SettingsRow::Order => Some("rotate.order"),
            SettingsRow::Source => Some("rotate.source"),
            SettingsRow::MinDwell => Some("rotate.min_dwell_secs"),
            SettingsRow::MaxDwell => Some("rotate.max_dwell_secs"),
            SettingsRow::Fullscreen => Some("output.fullscreen"),
            SettingsRow::Display => Some("output.display"),
            SettingsRow::Diagnostics => Some("hud.diagnostics"),
            SettingsRow::InputMode => Some("input.mode"),
            SettingsRow::InputDevice => Some("input.device"),
            SettingsRow::PresetName => Some("hud.preset_name"),
            SettingsRow::NowPlaying => Some("hud.now_playing"),
            SettingsRow::NextRotation => Some("hud.next_rotation"),
            SettingsRow::Console => Some("console.enabled"),
            // The path display: it names where presets are loaded from, which
            // is a launch-time resolution (`RLX_PRESET_DIR`, then the per-user
            // dir) rather than a value the file holds.
            SettingsRow::Presets => None,
        }
    }

    fn label(self) -> &'static str {
        match self {
            SettingsRow::Quality => "Quality",
            SettingsRow::GridScale => "Grid scale",
            SettingsRow::Adapter => "Adapter",
            SettingsRow::AutoRotate => "Auto-rotate",
            SettingsRow::Order => "Order",
            SettingsRow::Source => "Draw from",
            SettingsRow::MinDwell => "Min dwell",
            SettingsRow::MaxDwell => "Max dwell",
            SettingsRow::Fullscreen => "Fullscreen",
            SettingsRow::Display => "Display",
            SettingsRow::Diagnostics => "Diagnostics",
            SettingsRow::InputMode => "Input mode",
            SettingsRow::InputDevice => "Input device",
            SettingsRow::PresetName => "Preset name",
            SettingsRow::NowPlaying => "Now playing",
            SettingsRow::NextRotation => "Next in",
            SettingsRow::Console => "Console",
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
            // The resolved number, and whether the operator chose it or the
            // engine did — `auto` resolving 1.00 and a pinned 1.00 are the same
            // picture and not the same setting.
            SettingsRow::GridScale => format!(
                "{} {}",
                view.grid_scale,
                match view.grid_scale_choice {
                    GridScaleChoice::Auto => "(auto)",
                    GridScaleChoice::Fixed(_) => "(pinned)",
                }
            ),
            SettingsRow::Adapter => {
                if view.adapter_count == 0 {
                    // No roster to hold a position in, so the row names what
                    // is running rather than inventing a `1 of 1`.
                    view.adapter_name.clone()
                } else {
                    // 1-based for the operator, like the `Display` row.
                    format!(
                        "{} of {} - {}",
                        view.adapter_index + 1,
                        view.adapter_count,
                        view.adapter_name
                    )
                }
            }
            SettingsRow::AutoRotate => on_off(view.auto_rotate).to_owned(),
            // The kebab word `config.toml` holds, like the input-mode row.
            SettingsRow::Order => view.rotate_order.as_str().to_owned(),
            // **The state, not the key.** `favourites` with nothing marked
            // already draws from the whole eligible set, and a row printing
            // `favourites` over a library that is all showing describes a filter
            // nobody applied.
            SettingsRow::Source => match (view.rotate_source, view.favourites_marked) {
                (RotateSource::Favourites, false) => {
                    "favourites - none marked, drawing from all".to_owned()
                }
                (source, _) => source.as_str().to_owned(),
            },
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
            // The kebab word `config.toml` holds, not a prettier one: the menu
            // and the file have to name the same thing.
            SettingsRow::InputMode => view.input_mode.as_str().to_owned(),
            SettingsRow::InputDevice => {
                if view.input_device_count == 0 {
                    // No roster to hold a position in, so the row names what is
                    // running rather than inventing a `1 of 1`.
                    if view.input_device_name.is_empty() {
                        "none".to_owned()
                    } else {
                        view.input_device_name.clone()
                    }
                } else {
                    // 1-based for the operator, like the `Display` row.
                    format!(
                        "{} of {} - {}",
                        view.input_device_index + 1,
                        view.input_device_count,
                        view.input_device_name
                    )
                }
            }
            SettingsRow::PresetName => on_off(view.preset_name).to_owned(),
            SettingsRow::NowPlaying => on_off(view.now_playing).to_owned(),
            SettingsRow::NextRotation => on_off(view.next_rotation).to_owned(),
            SettingsRow::Console => on_off(view.console).to_owned(),
            SettingsRow::Presets => view.preset_dir.clone(),
        }
    }

    /// The action `Left` (or `Right`, when `right`) on this row asks for.
    ///
    /// `pub(crate)` so the operator console's transport can ask for a row's
    /// change through this rather than restating the rule: two surfaces
    /// offering the same control must produce the same action value, and the
    /// only way to guarantee that is one function.
    pub(crate) fn edit(self, right: bool, view: &SettingsView) -> SettingsAction {
        // A row that edits no key edits nothing. Read-only is then a property of
        // the declaration above rather than of a hand-written arm, so the two
        // cannot disagree about which row is the read-only one.
        if self.config_path().is_none() {
            return SettingsAction::None;
        }
        match self {
            // A switch, not a cycle: `[`/`]`'s orientation, floor on the left.
            SettingsRow::Quality => {
                SettingsAction::SetTier(if right { Tier::Rich } else { Tier::Floor })
            }
            SettingsRow::GridScale => {
                SettingsAction::SetGridScale(step_grid_scale(view.grid_scale_choice, right))
            }
            // A walk over the roster, wrapping, with the target decided here
            // so the shell cannot land off the end of a list it did not size.
            // Under two entries there is nowhere to go, and the key is inert
            // rather than a switch onto the adapter already running.
            SettingsRow::Adapter if view.adapter_count > 1 => {
                let count = view.adapter_count;
                let at = view.adapter_index.min(count - 1);
                SettingsAction::SetAdapter(if right {
                    (at + 1) % count
                } else {
                    (at + count - 1) % count
                })
            }
            SettingsRow::Adapter => SettingsAction::None,
            SettingsRow::AutoRotate => SettingsAction::ToggleAuto,
            // Switches, not toggles: the shuffle is on the left because it is
            // the default, and `all` is on the left because it is the wider set.
            SettingsRow::Order => SettingsAction::SetOrder(if right {
                RotateOrder::Sequential
            } else {
                RotateOrder::Shuffled
            }),
            SettingsRow::Source => SettingsAction::SetSource(if right {
                RotateSource::Favourites
            } else {
                RotateSource::All
            }),
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
            SettingsRow::InputMode if view.input_editable => {
                SettingsAction::SetInputMode(if right {
                    InputMode::LineIn
                } else {
                    InputMode::Loopback
                })
            }
            // An empty roster has nothing to advance to, so the key is inert
            // rather than asking the shell to index into a list that is not
            // there.
            SettingsRow::InputDevice if view.input_editable && view.input_device_count > 0 => {
                SettingsAction::CycleInputDevice
            }
            // Read-only: this platform's capture path takes no selection.
            SettingsRow::InputMode | SettingsRow::InputDevice => SettingsAction::None,
            SettingsRow::PresetName => SettingsAction::TogglePresetName,
            SettingsRow::NowPlaying => SettingsAction::ToggleNowPlaying,
            SettingsRow::NextRotation => SettingsAction::ToggleNextRotation,
            SettingsRow::Console => SettingsAction::ToggleConsole,
            // Read-only, and already returned above on the strength of its
            // empty `config_path`. The arm stays because the match is
            // exhaustive, and it agrees with the guard by construction.
            SettingsRow::Presets => SettingsAction::None,
        }
    }
}

/// The grid-scale choice one step from `from`: `Right` raises it through
/// [`GRID_SCALE_STEPS`] and past the top to `auto`, `Left` lowers it, and both
/// **stop at their end** rather than wrapping — so key repeat parks on a value
/// instead of cycling through a full-resolution frame.
///
/// `Left` from `auto` lands on the top step, 1.0. A hand-written value between
/// two steps moves to the neighbouring step on the side the key points, so the
/// first press is never a no-op.
pub(crate) fn step_grid_scale(from: GridScaleChoice, right: bool) -> GridScaleChoice {
    let fixed =
        |value: f32| GridScale::new(value).map_or(GridScaleChoice::Auto, GridScaleChoice::Fixed);
    let top = GRID_SCALE_STEPS.last().copied().unwrap_or(GridScale::MAX);
    match (from, right) {
        (GridScaleChoice::Auto, true) => GridScaleChoice::Auto,
        (GridScaleChoice::Auto, false) => fixed(top),
        (GridScaleChoice::Fixed(scale), true) => GRID_SCALE_STEPS
            .iter()
            .copied()
            .find(|&step| step > scale.get())
            .map_or(GridScaleChoice::Auto, fixed),
        (GridScaleChoice::Fixed(scale), false) => GRID_SCALE_STEPS
            .iter()
            .rev()
            .copied()
            .find(|&step| step < scale.get())
            .map_or(from, fixed),
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
