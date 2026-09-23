use super::{
    DWELL_CEILING, DWELL_FLOOR, DWELL_STEP, InputMode, RotateOrder, RotateSource, SettingsAction,
    SettingsKey, SettingsRow, SettingsState, SettingsView, Tier, TierState,
};

use std::path::Path;

use standalone::config::Config;

fn view() -> SettingsView {
    SettingsView {
        tier: Tier::Rich,
        tier_state: TierState::Auto,
        auto_rotate: false,
        rotate_order: RotateOrder::Shuffled,
        rotate_source: RotateSource::All,
        favourites_marked: true,
        min_dwell_secs: 20,
        max_dwell_secs: 90,
        fullscreen: false,
        display_index: 1,
        display_count: 3,
        display_name: "DELL U2720Q".to_owned(),
        diagnostics: false,
        input_mode: InputMode::LineIn,
        input_device_index: 1,
        input_device_count: 2,
        input_device_name: "Line (ZOOM AMS-22 Audio)".to_owned(),
        input_editable: true,
        preset_name: true,
        now_playing: true,
        next_rotation: true,
        console: false,
        adapter_index: 1,
        adapter_count: 2,
        adapter_name: "NVIDIA GeForce RTX 3080 Laptop GPU".to_owned(),
        preset_dir: r"C:\Users\x\AppData\Roaming\Ritmolux\presets".to_owned(),
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

/// **Every row emits its own action**, in both directions. This is the
/// mapping the shell executes blind, so a row silently wired to the wrong
/// effect is invisible everywhere else.
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
        assert_eq!(
            edit_at(SettingsRow::NowPlaying, right, &v),
            SettingsAction::ToggleNowPlaying
        );
        assert_eq!(
            edit_at(SettingsRow::NextRotation, right, &v),
            SettingsAction::ToggleNextRotation
        );
        assert_eq!(
            edit_at(SettingsRow::InputDevice, right, &v),
            SettingsAction::CycleInputDevice
        );
    }
    // The two rotation rows are switches for the same reason the mode row is.
    assert_eq!(
        edit_at(SettingsRow::Order, false, &v),
        SettingsAction::SetOrder(RotateOrder::Shuffled)
    );
    assert_eq!(
        edit_at(SettingsRow::Order, true, &v),
        SettingsAction::SetOrder(RotateOrder::Sequential)
    );
    assert_eq!(
        edit_at(SettingsRow::Source, false, &v),
        SettingsAction::SetSource(RotateSource::All)
    );
    assert_eq!(
        edit_at(SettingsRow::Source, true, &v),
        SettingsAction::SetSource(RotateSource::Favourites)
    );
    // The mode row is a switch, not a toggle: each direction names one value,
    // so a held key settles rather than oscillating.
    assert_eq!(
        edit_at(SettingsRow::InputMode, false, &v),
        SettingsAction::SetInputMode(InputMode::Loopback)
    );
    assert_eq!(
        edit_at(SettingsRow::InputMode, true, &v),
        SettingsAction::SetInputMode(InputMode::LineIn)
    );
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

/// **The adapter row walks the roster and wraps at both ends**, with the
/// target decided in the state machine so the shell never indexes off the end
/// of a list it did not size. `Right` walks up, `Left` walks down — the
/// orientation the row highlight and the `Display` row share.
#[test]
fn the_adapter_row_walks_the_roster_and_wraps() {
    let mut v = view();
    v.adapter_count = 3;

    v.adapter_index = 1;
    assert_eq!(
        edit_at(SettingsRow::Adapter, true, &v),
        SettingsAction::SetAdapter(2)
    );
    assert_eq!(
        edit_at(SettingsRow::Adapter, false, &v),
        SettingsAction::SetAdapter(0)
    );

    // Off the last entry `Right` wraps to the first, and off the first `Left`
    // wraps to the last.
    v.adapter_index = 2;
    assert_eq!(
        edit_at(SettingsRow::Adapter, true, &v),
        SettingsAction::SetAdapter(0),
        "Right off the last adapter did not wrap"
    );
    v.adapter_index = 0;
    assert_eq!(
        edit_at(SettingsRow::Adapter, false, &v),
        SettingsAction::SetAdapter(2),
        "Left off the first adapter did not wrap"
    );

    // A position past the roster — a cache that shrank under the row — is
    // clamped rather than indexed.
    v.adapter_index = 7;
    assert_eq!(
        edit_at(SettingsRow::Adapter, true, &v),
        SettingsAction::SetAdapter(0)
    );
}

/// **With fewer than two adapters the row renders and does not move.** One
/// adapter and a failed enumeration reach the state machine identically, and
/// neither may ask the shell to switch onto a roster it cannot walk — a switch
/// onto the adapter already running would restart every accumulation for no
/// change.
#[test]
fn the_adapter_row_is_inert_without_a_second_adapter() {
    let s = opened();
    let value = |v: &SettingsView| -> String {
        s.lines(v)
            .iter()
            .find(|(l, _)| *l == "Adapter")
            .map(|(_, val)| val.clone())
            .expect("no Adapter row")
    };

    let mut v = view();
    v.adapter_count = 1;
    v.adapter_index = 0;
    for right in [false, true] {
        assert_eq!(
            edit_at(SettingsRow::Adapter, right, &v),
            SettingsAction::None,
            "a one-adapter roster moved"
        );
    }
    assert_eq!(value(&v), "1 of 1 - NVIDIA GeForce RTX 3080 Laptop GPU");

    v.adapter_count = 0;
    for right in [false, true] {
        assert_eq!(
            edit_at(SettingsRow::Adapter, right, &v),
            SettingsAction::None,
            "an empty roster moved"
        );
    }
    // With no roster it names what is running rather than a `1 of 0`.
    assert_eq!(value(&v), "NVIDIA GeForce RTX 3080 Laptop GPU");

    // Non-vacuity: the same row with two adapters does move.
    let v = view();
    assert_eq!(
        edit_at(SettingsRow::Adapter, true, &v),
        SettingsAction::SetAdapter(0)
    );
    assert_eq!(value(&v), "2 of 2 - NVIDIA GeForce RTX 3080 Laptop GPU");
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

/// **The row roster is the contract**, so it is asserted as a list rather than
/// as a count: every other test here reaches its row through `ALL`, which means
/// a reordering would move them all in step and go unnoticed.
#[test]
fn the_rows_are_the_ones_the_menu_promises_in_order() {
    assert_eq!(
        SettingsRow::ALL,
        [
            SettingsRow::Quality,
            SettingsRow::Adapter,
            SettingsRow::AutoRotate,
            SettingsRow::Order,
            SettingsRow::Source,
            SettingsRow::MinDwell,
            SettingsRow::MaxDwell,
            SettingsRow::Fullscreen,
            SettingsRow::Display,
            SettingsRow::Diagnostics,
            SettingsRow::InputMode,
            SettingsRow::InputDevice,
            SettingsRow::PresetName,
            SettingsRow::NowPlaying,
            SettingsRow::NextRotation,
            SettingsRow::Console,
            SettingsRow::Presets,
        ]
    );
    // The read-only row stays last, which is what keeps a menu lap from ending
    // on a key that does nothing.
    assert_eq!(SettingsRow::ALL.last(), Some(&SettingsRow::Presets));
}

/// A serialised config with every `Option` key set.
///
/// ADR-0240's Negative section: an optional key is absent from a default
/// serialisation, so walking `Config::default()` would report `[output] gpu` —
/// the key the `Adapter` row edits — as unresolved. The same workaround
/// `standalone/tests/suite/configuration_doc.rs` uses on the schema side.
fn populated_config() -> toml::Value {
    let mut config = Config::default();
    config.output.display_name = Some("a monitor".to_owned());
    config.output.gpu = Some("an adapter".to_owned());
    config.console.display_name = Some("another monitor".to_owned());
    config.rotate.seed = Some(7);
    let text = toml::to_string(&config).expect("the config type must serialise");
    toml::from_str(&text).expect("its own output must parse as a value tree")
}

/// Whether `section.key` names something in the tree.
fn resolves(root: &toml::Value, path: &str) -> bool {
    let mut cursor = root;
    for segment in path.split('.') {
        match cursor.get(segment) {
            Some(next) => cursor = next,
            None => return false,
        }
    }
    true
}

/// `docs/configuration.md`, resolved from the manifest rather than the cwd — a
/// test's working directory is not a contract.
fn configuration_doc() -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../docs/configuration.md");
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

/// **Every row edits a key that exists** (ADR-0240). This is the menu-side half
/// of the property `configuration_doc.rs` asserts from the schema side: that
/// one walks the keys and finds the document, this one walks the rows and finds
/// the keys, so a row whose value lives nowhere is a failure rather than a
/// habit nobody wrote down.
#[test]
fn every_row_edits_a_key_the_config_holds() {
    let config = populated_config();
    let unresolved: Vec<String> = SettingsRow::ALL
        .iter()
        .filter_map(|row| {
            let path = row.config_path()?;
            (!resolves(&config, path)).then(|| format!("{row:?} -> {path}"))
        })
        .collect();
    assert!(
        unresolved.is_empty(),
        "these settings rows declare a config.toml path that does not resolve: {unresolved:?}\n\
         A row is an editor of a key; a row whose key is gone is a choice reachable only from \
         inside the running window, which is what ADR-0240 refuses."
    );
}

/// **The one read-only row is the `Presets` path display, and `edit` agrees.**
/// Asserted as a list rather than a count, for the reason the roster above is:
/// a row that quietly stopped declaring a key would otherwise read as the same
/// answer.
#[test]
fn the_presets_row_is_the_only_row_that_edits_nothing() {
    let read_only: Vec<SettingsRow> = SettingsRow::ALL
        .iter()
        .copied()
        .filter(|row| row.config_path().is_none())
        .collect();
    assert_eq!(
        read_only,
        vec![SettingsRow::Presets],
        "a row declaring no config.toml path is a setting with no file behind it"
    );

    // And the declaration is what makes the row inert, in both directions.
    let v = view();
    for right in [false, true] {
        assert_eq!(
            edit_at(SettingsRow::Presets, right, &v),
            SettingsAction::None
        );
    }
}

/// **Every key a row edits is named in `docs/configuration.md`, in backticks.**
/// Reached from the menu side: `configuration_doc.rs` holds the document to the
/// schema, and a key the schema holds but no row reaches is a different
/// property from a row whose key a reader cannot look up.
#[test]
fn every_key_a_row_edits_is_documented() {
    let doc = configuration_doc();
    let missing: Vec<String> = SettingsRow::ALL
        .iter()
        .filter_map(|row| {
            let path = row.config_path()?;
            let (section, key) = path.split_once('.').expect("a path is `section.key`");
            let section_named = doc.contains(&format!("`[{section}]`"));
            let key_named = doc.contains(&format!("`{key}`"));
            (!section_named || !key_named).then(|| format!("{row:?} -> {path}"))
        })
        .collect();
    assert!(
        missing.is_empty(),
        "docs/configuration.md does not name the key these rows edit, in backticks: {missing:?}"
    );
}

/// **The input rows are read-only where capture takes no selection.** They
/// still *render* — the menu keeps one shape on every platform, and a Mac user
/// sees what the app is listening to rather than finding a row missing — but
/// neither key moves them.
#[test]
fn the_input_rows_render_but_do_not_move_when_they_are_not_editable() {
    let mut v = view();
    v.input_editable = false;

    for right in [false, true] {
        assert_eq!(
            edit_at(SettingsRow::InputMode, right, &v),
            SettingsAction::None,
            "an uneditable input mode moved"
        );
        assert_eq!(
            edit_at(SettingsRow::InputDevice, right, &v),
            SettingsAction::None,
            "an uneditable input device moved"
        );
    }

    // Still drawn, and still carrying their values.
    let lines = opened().lines(&v);
    let find = |label: &str| -> String {
        lines
            .iter()
            .find(|(l, _)| *l == label)
            .map(|(_, val)| val.clone())
            .unwrap_or_else(|| panic!("no {label} row"))
    };
    assert_eq!(find("Input mode"), "line-in");
    assert_eq!(find("Input device"), "2 of 2 - Line (ZOOM AMS-22 Audio)");
}

/// **An empty roster is a live-show state, not a crash.** A failed enumeration
/// and a dataflow with no active endpoint reach this module identically, and a
/// modal that panicked on either would take the show down with it.
#[test]
fn an_empty_device_roster_renders_and_yields_nothing() {
    let mut v = view();
    v.input_device_count = 0;
    v.input_device_index = 0;

    for right in [false, true] {
        assert_eq!(
            edit_at(SettingsRow::InputDevice, right, &v),
            SettingsAction::None,
            "the device row advanced into an empty roster"
        );
        // The mode row is *not* gated on the roster: switching mode is what
        // makes the shell go and enumerate the other dataflow.
        assert!(matches!(
            edit_at(SettingsRow::InputMode, right, &v),
            SettingsAction::SetInputMode(_)
        ));
    }

    let s = opened();
    let value = |v: &SettingsView| -> String {
        s.lines(v)
            .iter()
            .find(|(l, _)| *l == "Input device")
            .map(|(_, val)| val.clone())
            .expect("no Input device row")
    };
    // With a name it says what is running; with nothing at all it still says
    // something, rather than reading as a truncated `1 of 1 - `.
    assert_eq!(value(&v), "Line (ZOOM AMS-22 Audio)");
    v.input_device_name = String::new();
    assert_eq!(value(&v), "none");
}

/// The mode row shows the **kebab word the config file holds**, both ways round,
/// so an operator comparing the menu against `config.toml` reads one string.
#[test]
fn the_input_mode_row_shows_the_config_word() {
    let mut v = view();
    let s = opened();
    let value = |v: &SettingsView| -> String {
        s.lines(v)
            .iter()
            .find(|(l, _)| *l == "Input mode")
            .map(|(_, val)| val.clone())
            .expect("no Input mode row")
    };
    assert_eq!(value(&v), "line-in");
    v.input_mode = InputMode::Loopback;
    assert_eq!(value(&v), "loopback");
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
    // Likewise for the banner row (Plan 0097 Phase 3) — it sits beside the
    // preset name because both are `[hud]` keys about what covers the show.
    assert_eq!(find("Now playing", &lines), "on");
    v.now_playing = false;
    assert_eq!(find("Now playing", &s.lines(&v)), "off");
    v.now_playing = true;
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

/// **Toggling the console off from the console's own S-menu leaves that menu
/// open**, so the next frame's routing draws it on the output.
///
/// This is the one interaction where the modal move has to reverse
/// mid-keystroke, and the obvious implementation loses the menu along with the
/// window. Asserted here at the state machine, which is where "the menu is
/// still open" is decidable without a window: the row's action is
/// `ToggleConsole` and **not** `Close`, and the state is still open after the
/// key that produced it. The routing half — that an open modal with no console
/// draws on the output — is `console::tests`'
/// `console_closed_draws_everything_on_the_output`.
#[test]
fn closing_the_console_from_its_own_menu_leaves_the_menu_open() {
    let mut state = opened();
    let view = view();

    // Walk to the console row the way an operator does, so this covers the row
    // being reachable as well as the action it produces.
    let mut guard = 0;
    while SettingsRow::ALL.get(state.row()) != Some(&SettingsRow::Console) {
        state.handle_key(SettingsKey::Down, &view);
        guard += 1;
        assert!(
            guard <= SettingsRow::ALL.len(),
            "the console row is not reachable by walking the menu"
        );
    }

    let action = state.handle_key(SettingsKey::Right, &view);
    assert_eq!(
        action,
        SettingsAction::ToggleConsole,
        "the console row must ask the shell to toggle the window, and nothing else"
    );
    assert_ne!(
        action,
        SettingsAction::Close,
        "the row must not close the menu — the operator would lose it along \
         with the window it just closed"
    );
    assert!(
        state.is_open(),
        "the menu closed itself when the console was toggled, so an operator \
         closing the console from the console loses the menu entirely"
    );
}

/// **Every row says something.** `label`, `value` and `edit` are exhaustive over
/// `ALL` by construction — the compiler refuses a missing arm — so the part a
/// compiler cannot check is that a row's value is not the empty string, which is
/// what a row added with a `String::new()` placeholder would draw.
#[test]
fn every_row_shows_a_value_for_a_default_view() {
    let v = view();
    let lines = opened().lines(&v);
    assert_eq!(lines.len(), SettingsRow::ALL.len());
    for (label, value) in &lines {
        assert!(!label.is_empty(), "a row drew no label");
        assert!(!value.is_empty(), "the `{label}` row drew no value");
    }
}

/// **The two rotation rows show the words `config.toml` holds**, and the source
/// row names the fallback when the show is in it.
///
/// `favourites` with nothing marked already draws from the whole eligible set,
/// so a row printing `favourites` over a library that is all showing would
/// describe a filter nobody applied.
#[test]
fn the_rotation_rows_show_the_config_words_and_the_source_names_its_fallback() {
    let mut v = view();
    let s = opened();
    let value = |row: SettingsRow, v: &SettingsView| -> String {
        s.lines(v)
            .iter()
            .find(|(l, _)| *l == row.label())
            .map(|(_, val)| val.clone())
            .unwrap_or_else(|| panic!("no {:?} row", row))
    };

    assert_eq!(value(SettingsRow::Order, &v), "shuffled");
    v.rotate_order = RotateOrder::Sequential;
    assert_eq!(value(SettingsRow::Order, &v), "sequential");

    assert_eq!(value(SettingsRow::Source, &v), "all");
    v.rotate_source = RotateSource::Favourites;
    assert_eq!(value(SettingsRow::Source, &v), "favourites");

    v.favourites_marked = false;
    let fallback = value(SettingsRow::Source, &v);
    assert!(
        fallback.contains("none marked") && fallback.contains("all"),
        "with nothing marked the source row must say the show is drawing from \
         the whole library, not print `favourites`: {fallback}"
    );

    // With `all` selected the flag is irrelevant — there is no fallback to be in.
    v.rotate_source = RotateSource::All;
    assert_eq!(value(SettingsRow::Source, &v), "all");
}

/// The console row reports the live window, not the stored preference: a
/// console opened by the hotkey or by `--console` reads as on here.
#[test]
fn the_console_row_reports_whether_a_console_is_open() {
    let mut open_view = view();
    open_view.console = true;
    assert_eq!(SettingsRow::Console.value(&open_view), "on");

    let mut shut_view = view();
    shut_view.console = false;
    assert_eq!(SettingsRow::Console.value(&shut_view), "off");
}
