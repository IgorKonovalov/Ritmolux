//! Every key binding the standalone has, in one declarative table.
//!
//! [`KEYMAP`] is read by **both** the input dispatch and the help sheet: a key
//! does what its row says because the dispatcher looks the row up, and the help
//! sheet lists exactly the rows the dispatcher reads, so the sheet cannot fall
//! out of date with the keys. Each row holds the keys, the context it applies
//! in, the action, a one-line label and a group.
//!
//! Window-free apart from winit's key codes: which modal is open is handed in as
//! a [`Ctx`], and what the action does is the shell's.

use winit::keyboard::KeyCode;

use crate::overlay::OverlayKey;
use crate::settings::SettingsKey;

/// One key a row answers to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    /// A physical key, whatever the layout prints on it.
    Code(KeyCode),
    /// A character the layout produces — `?`, which sits on a different
    /// physical key on different layouts and is only ever matched by what it
    /// types.
    Char(char),
}

/// A key press as the dispatcher sees it: the physical key, and the character
/// the layout made of it, if any.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Press {
    pub code: Option<KeyCode>,
    pub ch: Option<char>,
}

/// Which surface owns the keyboard.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ctx {
    /// No menu is open: the keys drive the show.
    Show,
    /// The preset browser is open.
    Browse,
    /// The settings menu is open.
    Settings,
    /// The help sheet is open.
    Help,
}

impl Ctx {
    /// Every context.
    pub const ALL: [Ctx; 4] = [Ctx::Show, Ctx::Browse, Ctx::Settings, Ctx::Help];

    /// What the help sheet calls this context in its header.
    pub fn title(self) -> &'static str {
        match self {
            Ctx::Show => "keys",
            Ctx::Browse => "keys in the browser",
            Ctx::Settings => "keys in settings",
            Ctx::Help => "keys in this sheet",
        }
    }
}

/// The heading a row is listed under on the help sheet, in listing order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Group {
    Presets,
    Rotation,
    Marks,
    Display,
    Menus,
    Navigate,
    Narrow,
    Change,
}

impl Group {
    /// Every group, in the order the help sheet lists them.
    pub const ALL: [Group; 8] = [
        Group::Presets,
        Group::Rotation,
        Group::Marks,
        Group::Display,
        Group::Menus,
        Group::Navigate,
        Group::Narrow,
        Group::Change,
    ];

    /// The heading's text.
    pub fn title(self) -> &'static str {
        match self {
            Group::Presets => "presets",
            Group::Rotation => "rotation",
            Group::Marks => "marks",
            Group::Display => "display",
            Group::Menus => "menus",
            Group::Navigate => "move",
            Group::Narrow => "narrow the list",
            Group::Change => "change",
        }
    }
}

/// What a row does. The shell carries each one out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// Cut to the next preset, restarting the auto-rotate dwell.
    NextPreset,
    /// Step back to the preset shown before.
    PreviousPreset,
    /// Cut to the favourite the pressed digit names.
    SelectFavourite,
    /// Flip between this preset and the one stashed for comparison.
    AbCompare,
    /// Mark or unmark a favourite.
    MarkFavourite,
    /// Hide or unhide.
    MarkHidden,
    ToggleAuto,
    ToggleOrder,
    CycleSource,
    TierFloor,
    TierRich,
    Fullscreen,
    /// Leave fullscreen; windowed, nothing. Never quits.
    LeaveFullscreen,
    CycleDisplay,
    Diagnostics,
    OpenSettings,
    Console,
    /// Open the help sheet for the context the key was pressed in.
    Help,
    /// Close the help sheet, back to where it was opened.
    CloseHelp,
    /// A key the preset browser's own state machine takes.
    Browse(OverlayKey),
    /// A typed character narrowing the browser. Not bound to a key: every
    /// printable character the rows above leave free does it.
    Filter,
    /// A key the settings menu's own state machine takes.
    Settings(SettingsKey),
    /// Hand the keyboard from settings to the browser.
    SettingsToBrowse,
}

/// One binding.
#[derive(Clone, Copy, Debug)]
pub struct Binding {
    /// The keys that fire it. Empty for [`Action::Filter`], which is listed on
    /// the sheet but fired by typing.
    pub keys: &'static [Key],
    /// How the help sheet spells the keys.
    pub shown: &'static str,
    pub ctx: Ctx,
    pub action: Action,
    /// What it does, in a few words.
    pub label: &'static str,
    pub group: Group,
    /// Whether an OS key repeat fires it again. Only moving a cursor or
    /// stepping a value repeats: a held `Space` would machine-gun preset
    /// switches, a held `C` would flap a window open and shut.
    pub repeat: bool,
}

const fn row(
    keys: &'static [Key],
    shown: &'static str,
    ctx: Ctx,
    action: Action,
    label: &'static str,
    group: Group,
) -> Binding {
    Binding {
        keys,
        shown,
        ctx,
        action,
        label,
        group,
        repeat: false,
    }
}

const fn repeating(binding: Binding) -> Binding {
    Binding {
        repeat: true,
        ..binding
    }
}

use Key::{Char as Ch, Code as K};
use KeyCode as KC;

const DIGITS: &[Key] = &[
    K(KC::Digit1),
    K(KC::Digit2),
    K(KC::Digit3),
    K(KC::Digit4),
    K(KC::Digit5),
    K(KC::Digit6),
    K(KC::Digit7),
    K(KC::Digit8),
    K(KC::Digit9),
    K(KC::Numpad1),
    K(KC::Numpad2),
    K(KC::Numpad3),
    K(KC::Numpad4),
    K(KC::Numpad5),
    K(KC::Numpad6),
    K(KC::Numpad7),
    K(KC::Numpad8),
    K(KC::Numpad9),
];

/// Every binding, grouped by context.
///
/// **In the browser, letters and digits are filter input**, which is why its
/// narrowings and the two marks sit on function keys: this app binds no
/// modifier combination anywhere, and a letter there would be swallowed by the
/// query. `?` is the one character taken from the query, and no preset name
/// uses it.
#[rustfmt::skip]
pub const KEYMAP: &[Binding] = &[
    // --- the show ---
    row(&[K(KC::Space)], "Space", Ctx::Show, Action::NextPreset, "next preset", Group::Presets),
    row(&[K(KC::Backspace)], "Backspace", Ctx::Show, Action::PreviousPreset, "back to the preset before", Group::Presets),
    row(DIGITS, "1-9", Ctx::Show, Action::SelectFavourite, "jump to favourite 1-9", Group::Presets),
    row(&[K(KC::KeyB)], "B", Ctx::Show, Action::AbCompare, "A/B compare with the stashed preset", Group::Presets),
    row(&[K(KC::KeyA)], "A", Ctx::Show, Action::ToggleAuto, "auto-rotate on or off", Group::Rotation),
    row(&[K(KC::KeyR)], "R", Ctx::Show, Action::ToggleOrder, "shuffled or sequential order", Group::Rotation),
    row(&[K(KC::KeyL)], "L", Ctx::Show, Action::CycleSource, "rotate through all or favourites", Group::Rotation),
    row(&[K(KC::F1)], "F1", Ctx::Show, Action::MarkFavourite, "mark or unmark as favourite", Group::Marks),
    row(&[K(KC::F2)], "F2", Ctx::Show, Action::MarkHidden, "hide or unhide", Group::Marks),
    row(&[K(KC::BracketLeft)], "[", Ctx::Show, Action::TierFloor, "quality: floor", Group::Display),
    row(&[K(KC::BracketRight)], "]", Ctx::Show, Action::TierRich, "quality: rich", Group::Display),
    row(&[K(KC::KeyF)], "F", Ctx::Show, Action::Fullscreen, "fullscreen on or off", Group::Display),
    row(&[K(KC::Escape)], "Esc", Ctx::Show, Action::LeaveFullscreen, "leave fullscreen", Group::Display),
    row(&[K(KC::KeyD)], "D", Ctx::Show, Action::CycleDisplay, "move to the next display", Group::Display),
    row(&[K(KC::F3)], "F3", Ctx::Show, Action::Diagnostics, "diagnostics panel", Group::Display),
    row(&[K(KC::Tab)], "Tab", Ctx::Show, Action::Browse(OverlayKey::Toggle), "browse presets", Group::Menus),
    row(&[K(KC::KeyS)], "S", Ctx::Show, Action::OpenSettings, "settings", Group::Menus),
    row(&[K(KC::KeyC)], "C", Ctx::Show, Action::Console, "operator console on another display", Group::Menus),
    row(&[Ch('?')], "?", Ctx::Show, Action::Help, "this sheet", Group::Menus),
    // --- the browser ---
    repeating(row(&[K(KC::ArrowUp)], "Up", Ctx::Browse, Action::Browse(OverlayKey::Up), "previous row", Group::Navigate)),
    repeating(row(&[K(KC::ArrowDown)], "Down", Ctx::Browse, Action::Browse(OverlayKey::Down), "next row", Group::Navigate)),
    repeating(row(&[K(KC::ArrowLeft)], "Left", Ctx::Browse, Action::Browse(OverlayKey::Left), "previous column", Group::Navigate)),
    repeating(row(&[K(KC::ArrowRight)], "Right", Ctx::Browse, Action::Browse(OverlayKey::Right), "next column", Group::Navigate)),
    row(&[K(KC::Enter), K(KC::NumpadEnter)], "Enter", Ctx::Browse, Action::Browse(OverlayKey::Enter), "switch to the highlighted preset", Group::Navigate),
    row(&[], "type", Ctx::Browse, Action::Filter, "narrow by name", Group::Narrow),
    row(&[K(KC::Backspace)], "Backspace", Ctx::Browse, Action::Browse(OverlayKey::Backspace), "delete a typed character", Group::Narrow),
    row(&[K(KC::F4)], "F4", Ctx::Browse, Action::Browse(OverlayKey::FavouritesOnly), "favourites only", Group::Narrow),
    row(&[K(KC::F5)], "F5", Ctx::Browse, Action::Browse(OverlayKey::Family), "one family at a time", Group::Narrow),
    row(&[K(KC::F6)], "F6", Ctx::Browse, Action::Browse(OverlayKey::ShowHidden), "hidden presets back in", Group::Narrow),
    row(&[K(KC::F1)], "F1", Ctx::Browse, Action::MarkFavourite, "mark or unmark the highlighted row", Group::Marks),
    row(&[K(KC::F2)], "F2", Ctx::Browse, Action::MarkHidden, "hide or unhide the highlighted row", Group::Marks),
    row(&[K(KC::Tab)], "Tab", Ctx::Browse, Action::Browse(OverlayKey::Toggle), "close the browser", Group::Menus),
    row(&[K(KC::Escape)], "Esc", Ctx::Browse, Action::Browse(OverlayKey::Escape), "close without switching", Group::Menus),
    row(&[Ch('?')], "?", Ctx::Browse, Action::Help, "this sheet", Group::Menus),
    // --- settings ---
    repeating(row(&[K(KC::ArrowUp)], "Up", Ctx::Settings, Action::Settings(SettingsKey::Up), "previous row", Group::Navigate)),
    repeating(row(&[K(KC::ArrowDown)], "Down", Ctx::Settings, Action::Settings(SettingsKey::Down), "next row", Group::Navigate)),
    repeating(row(&[K(KC::ArrowLeft)], "Left", Ctx::Settings, Action::Settings(SettingsKey::Left), "lower or switch the value", Group::Change)),
    repeating(row(&[K(KC::ArrowRight)], "Right", Ctx::Settings, Action::Settings(SettingsKey::Right), "raise or switch the value", Group::Change)),
    row(&[K(KC::KeyS)], "S", Ctx::Settings, Action::Settings(SettingsKey::Toggle), "close settings", Group::Menus),
    row(&[K(KC::Escape)], "Esc", Ctx::Settings, Action::Settings(SettingsKey::Escape), "close settings", Group::Menus),
    row(&[K(KC::Tab)], "Tab", Ctx::Settings, Action::SettingsToBrowse, "over to the browser", Group::Menus),
    row(&[Ch('?')], "?", Ctx::Settings, Action::Help, "this sheet", Group::Menus),
    // --- the help sheet ---
    row(&[Ch('?'), K(KC::Escape)], "? or Esc", Ctx::Help, Action::CloseHelp, "close this sheet", Group::Menus),
];

impl Binding {
    /// Whether `press` fires this row.
    pub fn fires(&self, press: &Press) -> bool {
        self.keys.iter().any(|key| match key {
            Key::Code(code) => press.code == Some(*code),
            Key::Char(c) => press.ch == Some(*c),
        })
    }
}

/// The row `press` fires in `ctx`, or `None` when no row does — which, in the
/// browser, is a character for the filter, and everywhere else is a key that
/// does nothing.
///
/// A character row is matched **before** a physical one, so `?` is `?` on a
/// layout that puts it on a key some other row names.
pub fn lookup(ctx: Ctx, press: &Press) -> Option<&'static Binding> {
    let in_ctx = || KEYMAP.iter().filter(move |b| b.ctx == ctx);
    in_ctx()
        .find(|b| {
            b.keys
                .iter()
                .any(|k| matches!(k, Key::Char(c) if press.ch == Some(*c)))
        })
        .or_else(|| in_ctx().find(|b| b.fires(&Press { ch: None, ..*press })))
}

/// The rows the help sheet lists for `ctx`, grouped in [`Group::ALL`] order and
/// in table order within a group.
pub fn sheet(ctx: Ctx) -> Vec<(Group, Vec<&'static Binding>)> {
    Group::ALL
        .iter()
        .map(|&group| {
            let rows: Vec<&'static Binding> = KEYMAP
                .iter()
                .filter(|b| b.ctx == ctx && b.group == group)
                .collect();
            (group, rows)
        })
        .filter(|(_, rows)| !rows.is_empty())
        .collect()
}

/// The favourite a `1`-`9` key names, as a zero-based position, or `None` for
/// every other key.
///
/// The **top row and the numpad both**, because a key labelled `3` means `3`
/// wherever it is on the board; `0` is deliberately not in the set, since a
/// tenth slot would have to be either "the tenth" (reading `0` as ten) or a
/// hole, and nine slots with no ambiguity is the better of the three.
pub fn favourite_slot(code: KeyCode) -> Option<usize> {
    DIGITS
        .iter()
        .position(|k| *k == Key::Code(code))
        .map(|at| at % 9)
}

/// The launch hint: the three keys a first launch needs, spelled from the
/// rows that bind them, so the hint names the keys the dispatcher reads.
pub fn hint_text() -> String {
    let find = |action: Action| {
        KEYMAP
            .iter()
            .find(|b| b.ctx == Ctx::Show && b.action == action)
            .map_or("", |b| b.shown)
    };
    format!(
        "{}  help     {}  browse     {}  settings",
        find(Action::Help),
        find(Action::Browse(OverlayKey::Toggle)),
        find(Action::OpenSettings)
    )
}

#[cfg(test)]
mod tests;
