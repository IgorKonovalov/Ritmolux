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
        preset_name: true,
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
        assert_eq!(
            edit_at(SettingsRow::PresetName, right, &v),
            SettingsAction::TogglePresetName
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
        let SettingsAction::SetDwell { min_secs, .. } = edit_at(SettingsRow::MinDwell, false, &v)
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
    // Both states of the new row, since one of them is what an operator sets
    // the row for (Plan 0096 Phase 3).
    assert_eq!(find("Preset name", &lines), "on");
    v.preset_name = false;
    assert_eq!(find("Preset name", &s.lines(&v)), "off");
    v.preset_name = true;
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
