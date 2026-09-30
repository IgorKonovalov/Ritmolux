use super::{Action, Ctx, KEYMAP, Key, Press, favourite_slot, hint_text, lookup, sheet};
use crate::overlay::OverlayKey;
use crate::settings::SettingsKey;
use winit::keyboard::KeyCode;

/// **No two rows in one context share a key.** A key bound twice fires
/// whichever row the lookup reaches first, and the sheet would list both.
#[test]
fn no_two_rows_in_one_context_share_a_key() {
    for ctx in Ctx::ALL {
        let mut seen: Vec<(Key, &str)> = Vec::new();
        for b in KEYMAP.iter().filter(|b| b.ctx == ctx) {
            for key in b.keys {
                if let Some((_, other)) = seen.iter().find(|(k, _)| k == key) {
                    panic!(
                        "{key:?} is bound twice in {ctx:?}: `{other}` and `{}`",
                        b.label
                    );
                }
                seen.push((*key, b.label));
            }
        }
    }
}

/// Every action the dispatcher can emit, listed through an exhaustive match so
/// a new [`Action`] variant fails to compile here until it is added — and then
/// fails the test below until it has a row.
fn every_action() -> Vec<Action> {
    let overlay = |key: OverlayKey| match key {
        // A typed character reaches the browser as `Action::Filter`, never as
        // a bound key.
        OverlayKey::Char(_) => None,
        OverlayKey::Toggle
        | OverlayKey::Up
        | OverlayKey::Down
        | OverlayKey::Left
        | OverlayKey::Right
        | OverlayKey::Enter
        | OverlayKey::Escape
        | OverlayKey::Backspace
        | OverlayKey::Family
        | OverlayKey::FavouritesOnly
        | OverlayKey::ShowHidden => Some(Action::Browse(key)),
    };
    let settings = |key: SettingsKey| match key {
        SettingsKey::Toggle
        | SettingsKey::Up
        | SettingsKey::Down
        | SettingsKey::Left
        | SettingsKey::Right
        | SettingsKey::Escape => Action::Settings(key),
    };
    let guard = |action: Action| match action {
        Action::NextPreset
        | Action::PreviousPreset
        | Action::SelectFavourite
        | Action::AbCompare
        | Action::MarkFavourite
        | Action::MarkHidden
        | Action::ToggleAuto
        | Action::ToggleOrder
        | Action::CycleSource
        | Action::TierFloor
        | Action::TierRich
        | Action::Fullscreen
        | Action::LeaveFullscreen
        | Action::CycleDisplay
        | Action::Diagnostics
        | Action::OpenSettings
        | Action::Console
        | Action::Help
        | Action::CloseHelp
        | Action::Browse(_)
        | Action::Filter
        | Action::Settings(_)
        | Action::SettingsToBrowse => action,
    };
    let mut out: Vec<Action> = [
        Action::NextPreset,
        Action::PreviousPreset,
        Action::SelectFavourite,
        Action::AbCompare,
        Action::MarkFavourite,
        Action::MarkHidden,
        Action::ToggleAuto,
        Action::ToggleOrder,
        Action::CycleSource,
        Action::TierFloor,
        Action::TierRich,
        Action::Fullscreen,
        Action::LeaveFullscreen,
        Action::CycleDisplay,
        Action::Diagnostics,
        Action::OpenSettings,
        Action::Console,
        Action::Help,
        Action::CloseHelp,
        Action::Filter,
        Action::SettingsToBrowse,
    ]
    .into_iter()
    .map(guard)
    .collect();
    out.extend(
        [
            OverlayKey::Toggle,
            OverlayKey::Up,
            OverlayKey::Down,
            OverlayKey::Left,
            OverlayKey::Right,
            OverlayKey::Enter,
            OverlayKey::Escape,
            OverlayKey::Backspace,
            OverlayKey::Family,
            OverlayKey::FavouritesOnly,
            OverlayKey::ShowHidden,
        ]
        .into_iter()
        .filter_map(overlay),
    );
    out.extend(
        [
            SettingsKey::Toggle,
            SettingsKey::Up,
            SettingsKey::Down,
            SettingsKey::Left,
            SettingsKey::Right,
            SettingsKey::Escape,
        ]
        .into_iter()
        .map(settings),
    );
    out
}

/// **The help sheet is complete by construction**: every action the dispatcher
/// can emit has a row, and the dispatcher emits nothing a row does not name —
/// it only ever acts on what [`lookup`] returns, plus the typed filter, which is
/// a row of its own.
#[test]
fn every_action_the_dispatcher_can_emit_has_a_row() {
    for action in every_action() {
        assert!(
            KEYMAP.iter().any(|b| b.action == action),
            "{action:?} has no row, so the help sheet cannot list it"
        );
    }
    // And every row's action is one the dispatcher knows, so no row is
    // decoration.
    let known = every_action();
    for b in KEYMAP {
        assert!(
            known.contains(&b.action),
            "{:?} is not dispatched",
            b.action
        );
    }
}

/// **`?` is free in every context before it is taken**: one row per context
/// binds it, and no shipped preset name contains it — so taking it out of the
/// browser's filter costs no search anyone can type.
#[test]
fn the_question_mark_is_bound_once_per_context_and_names_never_need_it() {
    for ctx in Ctx::ALL {
        let rows: Vec<_> = KEYMAP
            .iter()
            .filter(|b| b.ctx == ctx && b.keys.contains(&Key::Char('?')))
            .collect();
        assert_eq!(rows.len(), 1, "{ctx:?} binds `?` {} times", rows.len());
        let expected = if ctx == Ctx::Help {
            Action::CloseHelp
        } else {
            Action::Help
        };
        assert_eq!(rows[0].action, expected, "{ctx:?}");
    }
    for preset in rlx_core::preset::default_presets() {
        assert!(
            !preset.name.contains('?'),
            "`{}` needs `?` in the filter",
            preset.name
        );
    }
}

/// `?` is matched by the character it types, wherever the layout puts it: a
/// press whose physical key is some other row's still opens help.
#[test]
fn the_question_mark_matches_by_character_on_any_physical_key() {
    let press = |code, ch| Press {
        code: Some(code),
        ch: Some(ch),
    };
    // US: Shift+Slash.
    let us = lookup(Ctx::Show, &press(KeyCode::Slash, '?')).map(|b| b.action);
    assert_eq!(us, Some(Action::Help));
    // A layout that types `?` on the key `,` sits on elsewhere.
    let other = lookup(Ctx::Show, &press(KeyCode::KeyM, '?')).map(|b| b.action);
    assert_eq!(other, Some(Action::Help), "the character wins");
    // Without the character, the physical key is the letter it is.
    let m = lookup(
        Ctx::Show,
        &Press {
            code: Some(KeyCode::KeyM),
            ch: Some('m'),
        },
    );
    assert!(m.is_none(), "M is unbound in the show");
}

/// In the browser, a letter is not a row, so it falls through to the filter.
#[test]
fn a_letter_in_the_browser_is_left_for_the_filter() {
    for (code, ch) in [
        (KeyCode::KeyS, 's'),
        (KeyCode::KeyB, 'b'),
        (KeyCode::Digit1, '1'),
    ] {
        let press = Press {
            code: Some(code),
            ch: Some(ch),
        };
        assert!(lookup(Ctx::Browse, &press).is_none(), "{code:?}");
        assert!(
            lookup(Ctx::Show, &press).is_some(),
            "{code:?} works outside"
        );
    }
}

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
    assert_eq!(favourite_slot(KeyCode::Numpad3), Some(2));
    assert_eq!(favourite_slot(KeyCode::Numpad9), Some(8));
    assert_eq!(favourite_slot(KeyCode::Digit0), None);
    for code in [KeyCode::KeyB, KeyCode::Space, KeyCode::F1, KeyCode::Minus] {
        assert_eq!(favourite_slot(code), None, "{code:?} is not a slot");
    }
}

/// The browser's narrowings are function keys and **no letter is a browser
/// row**: letters and digits are filter input while it is open.
#[test]
fn the_browser_binds_function_keys_and_never_letters() {
    for b in KEYMAP.iter().filter(|b| b.ctx == Ctx::Browse) {
        for key in b.keys {
            if let Key::Code(code) = key {
                let name = format!("{code:?}");
                assert!(
                    !name.starts_with("Key") && !name.starts_with("Digit"),
                    "{code:?} would be swallowed as a filter character"
                );
            }
        }
    }
}

/// Only moving a cursor or stepping a value repeats.
#[test]
fn only_navigation_and_value_steps_repeat() {
    for b in KEYMAP.iter().filter(|b| b.repeat) {
        assert!(
            matches!(
                b.action,
                Action::Browse(
                    OverlayKey::Up | OverlayKey::Down | OverlayKey::Left | OverlayKey::Right
                ) | Action::Settings(
                    SettingsKey::Up | SettingsKey::Down | SettingsKey::Left | SettingsKey::Right
                )
            ),
            "{:?} repeats",
            b.action
        );
    }
}

/// The sheet lists every row of its context, once, under a non-empty group.
#[test]
fn the_sheet_lists_every_row_of_its_context() {
    for ctx in Ctx::ALL {
        let listed: usize = sheet(ctx).iter().map(|(_, rows)| rows.len()).sum();
        let total = KEYMAP.iter().filter(|b| b.ctx == ctx).count();
        assert_eq!(listed, total, "{ctx:?}");
        assert!(sheet(ctx).iter().all(|(_, rows)| !rows.is_empty()));
    }
}

#[test]
fn the_hint_names_the_keys_the_show_binds() {
    assert_eq!(hint_text(), "?  help     Tab  browse     S  settings");
}
