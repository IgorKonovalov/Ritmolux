use super::{
    CAPTURE_TOP, ListLayout, NAME_CHARS, OverlayAction, OverlayKey, OverlayState, capture_line,
    fit, layout,
};

const NAMES: [&str; 4] = ["alpha", "bravo", "charlie", "delta"];

fn names() -> Vec<&'static str> {
    NAMES.to_vec()
}

/// The roster's first preset is active. Most tests below predate open-on-
/// active and only care about navigation, so they open on row 0 as they did.
const FIRST: usize = 0;

/// A layout tall enough that every test roster above is one column, so the
/// vertical-navigation tests are unaffected by column flow — `Left`/`Right`
/// are the only keys that read it.
const WIDE: ListLayout = ListLayout {
    cols: 1,
    rows_per_col: 1000,
    col_scroll: 0,
    len: 1000,
};

/// The display this project is built and demoed on, and the one the roster
/// already overflowed at.
const HD: (f32, f32) = (1920.0, 1080.0);
/// A 1440p display, where the same roster fits one column.
const QHD: (f32, f32) = (2560.0, 1440.0);
/// The shipped roster's size at the time this was written. Pinned as a
/// *number* rather than read from the library, because the arithmetic below
/// is the claim — the library growing must not silently retire it.
const SHIPPED: usize = 34;

/// **The overlay line carries the log's token verbatim** — both the failure
/// reason and the negotiated format — so a tester's screenshot and their
/// `diagnostics.log` cannot tell two different stories about one run.
#[test]
fn the_capture_line_carries_the_token_it_was_given() {
    let failed = capture_line("failed SCK screen recording permission denied");
    assert!(
        failed.contains("failed SCK screen recording permission denied"),
        "the reason did not survive the line: {failed:?}"
    );
    let live = capture_line("live WASAPI 44100/2");
    assert!(
        live.contains("live WASAPI 44100/2"),
        "the negotiated format did not survive the line: {live:?}"
    );
    assert_ne!(failed, live, "a failed capture reads as a live one");
    // Labelled, because `live SCK 48000/2` alone does not say it is about audio
    // to someone who has never seen this app's log.
    assert!(
        live.starts_with("audio"),
        "the line is unlabelled: {live:?}"
    );
}

/// The line clears the core's diagnostics panel, which composites *after* the
/// text layer — a line inside that band would be painted over, not crowded.
/// Pinned as arithmetic over the core's own layout so a panel that grows fails
/// this deliberately rather than silently hiding the verdict.
#[test]
fn the_capture_line_sits_below_the_core_diagnostics_panel() {
    // core/src/render/overlay.rs: MARGIN 12 + PAD 8 -> text at 20, TEXT_H 14,
    // sparkline 72, GPU bar 12, then five analysis rows of pitch 19, + PAD.
    const PANEL_BOTTOM: f32 =
        12.0 + 8.0 + 14.0 + 8.0 + 72.0 + 8.0 + 12.0 + 8.0 + (19.0 * 5.0 - 5.0) + 8.0;
    // A `const` block: both sides are constants, so this is a compile-time
    // check — the panel growing past the line fails the build, not a run.
    const {
        assert!(
            CAPTURE_TOP >= PANEL_BOTTOM,
            "the capture line starts inside the core diagnostics panel, which composites over it"
        );
    }
}

/// **The numbers the plan pinned**, and they are this file's constants'
/// arithmetic rather than an implementation's output: a change to `ROW_H` or
/// `LIST_TOP` fails this deliberately.
#[test]
fn the_shipped_roster_flows_into_two_columns_at_1080p_and_one_at_1440p() {
    let (w, h) = HD;
    let l = layout(SHIPPED, 0, w, h);
    assert_eq!(
        l.rows_per_col, 32,
        "floor((1080 - 94) / 30) = 32 — the arithmetic the single-column list already used"
    );
    assert_eq!(l.cols, 2, "34 rows into 32-row columns needs 2");
    assert_eq!(l.col_scroll, 0, "it fits, so nothing scrolls");
    assert!(
        l.cols * l.rows_per_col >= SHIPPED,
        "the whole roster must be on screen at once — that is the point"
    );

    // The second column holds exactly the 2 rows that used to be past the fold.
    assert_eq!(l.place(31), Some((0, 31)), "last row of column 1");
    assert_eq!(l.place(32), Some((1, 0)), "first row of column 2");
    assert_eq!(l.place(33), Some((1, 1)));
    assert_eq!(l.place(34), None, "one past the roster is not placed");

    // Taller display: 44 rows per column is already >= 34, so one column.
    let (w, h) = QHD;
    let l = layout(SHIPPED, 0, w, h);
    assert_eq!(l.rows_per_col, 44, "floor((1440 - 94) / 30) = 44");
    assert_eq!(l.cols, 1);
    assert_eq!(l.place(33), Some((0, 33)));
}

/// `Right` crosses a column at the same row; `Right` off the last column does
/// not move. Both on a roster whose columns are **full**, so "the same row"
/// is exact rather than approximately.
#[test]
fn right_steps_one_column_and_clamps_at_the_last() {
    let n: Vec<&str> = (0..20).map(|_| "x").collect();
    let l = ListLayout {
        cols: 2,
        rows_per_col: 10,
        col_scroll: 0,
        len: 20,
    };
    let mut s = OverlayState::new();
    s.handle_key(OverlayKey::Toggle, &n, 9, &l); // last row of column 1

    assert_eq!(s.highlight(), 9);
    s.handle_key(OverlayKey::Right, &n, 9, &l);
    assert_eq!(
        s.highlight(),
        19,
        "Right did not land in column 2, same row"
    );

    // Off the last column: no move.
    s.handle_key(OverlayKey::Right, &n, 9, &l);
    assert_eq!(s.highlight(), 19, "Right off the last column moved");

    // And back.
    s.handle_key(OverlayKey::Left, &n, 9, &l);
    assert_eq!(s.highlight(), 9);
    s.handle_key(OverlayKey::Left, &n, 9, &l);
    assert_eq!(s.highlight(), 9, "Left off the first column moved");
}

/// A ragged last column — the shape the shipped roster actually produces
/// (32 + 2) — keeps the move instead of refusing it, landing on the short
/// column's last row.
#[test]
fn right_into_a_short_column_lands_on_its_last_row() {
    let n: Vec<&str> = (0..SHIPPED).map(|_| "x").collect();
    let (w, h) = HD;
    let l = layout(SHIPPED, 0, w, h);
    let mut s = OverlayState::new();
    s.handle_key(OverlayKey::Toggle, &n, 20, &l);
    assert_eq!(s.highlight(), 20);

    // Column 2 has only rows 32 and 33; row 20 of it does not exist.
    s.handle_key(OverlayKey::Right, &n, 20, &l);
    assert_eq!(
        s.highlight(),
        33,
        "Right into the short column should land on its last row, not refuse"
    );
    // Left returns into column 1, which is full, so the row exists.
    s.handle_key(OverlayKey::Left, &n, 20, &l);
    assert_eq!(s.highlight(), 1);
}

/// **Scrolling survives as the fallback.** When even the columns that fit
/// cannot hold the roster, the list scrolls by *whole columns* and the
/// highlighted row is always on screen.
#[test]
fn a_roster_too_big_for_its_columns_scrolls_by_whole_columns() {
    // A small window: few rows per column, and only room for two columns.
    let (w, h) = (900.0, 274.0); // floor((274 - 94) / 30) = 6 rows
    let len = 60; // 10 columns needed
    let l0 = layout(len, 0, w, h);
    assert_eq!(l0.rows_per_col, 6);
    assert!(
        l0.cols < len.div_ceil(l0.rows_per_col),
        "precondition: the roster must NOT fit, or this tests nothing"
    );
    assert_eq!(
        l0.col_scroll, 0,
        "no scroll while the highlight is on screen"
    );

    // Every row is reachable and on screen when it is the highlight — the
    // property that matters, checked exhaustively rather than at a few spots.
    for hl in 0..len {
        let l = layout(len, hl, w, h);
        assert!(
            l.place(hl).is_some(),
            "highlight {hl} scrolled off screen (scroll {}, cols {})",
            l.col_scroll,
            l.cols
        );
        // Scrolling is in whole columns: the leftmost drawn row is a column
        // boundary, so no column is ever half-shown.
        assert_eq!(l.place(l.col_scroll * l.rows_per_col), Some((0, 0)));
    }

    // At the far end the last column is flush with the right edge rather than
    // scrolled past it.
    let l = layout(len, len - 1, w, h);
    assert_eq!(l.col_scroll, len.div_ceil(l.rows_per_col) - l.cols);
}

/// A window too short for even one row still yields a usable layout rather
/// than a division by zero or an empty screen.
#[test]
fn a_degenerate_window_still_lays_out_one_row_in_one_column() {
    for (w, h) in [(1920.0, 0.0), (1920.0, 94.0), (0.0, 1080.0), (10.0, 10.0)] {
        let l = layout(SHIPPED, 0, w, h);
        assert!(l.rows_per_col >= 1, "{w}x{h} produced no rows");
        assert!(l.cols >= 1, "{w}x{h} produced no columns");
        assert_eq!(l.place(0), Some((0, 0)), "{w}x{h} could not place row 0");
    }
}

/// Truncation is cosmetic and only fires past the budget, so no shipped name
/// is ever shortened.
#[test]
fn a_name_is_only_shortened_past_the_column_budget() {
    assert_eq!(fit("Spectrum Corona"), "Spectrum Corona");
    let exact: String = "a".repeat(NAME_CHARS);
    assert_eq!(fit(&exact), exact, "a name that exactly fits is untouched");

    let long: String = "a".repeat(NAME_CHARS + 10);
    let cut = fit(&long);
    assert_eq!(cut.chars().count(), NAME_CHARS);
    assert!(cut.ends_with("..."));

    // Multi-byte: counted in characters, so this must not panic or split a
    // code point.
    let wide: String = "é".repeat(NAME_CHARS + 5);
    assert_eq!(fit(&wide).chars().count(), NAME_CHARS);
}

#[test]
fn nav_keys_are_inert_while_closed() {
    let mut s = OverlayState::new();
    let n = names();
    // A closed overlay ignores nav keys so the shell's Space-cycle still runs.
    assert_eq!(
        s.handle_key(OverlayKey::Down, &n, FIRST, &WIDE),
        OverlayAction::None
    );
    assert_eq!(
        s.handle_key(OverlayKey::Enter, &n, FIRST, &WIDE),
        OverlayAction::None
    );
    assert_eq!(
        s.handle_key(OverlayKey::Escape, &n, FIRST, &WIDE),
        OverlayAction::None
    );
    assert!(!s.is_open());
}

#[test]
fn open_navigate_and_select_emits_the_absolute_index() {
    let mut s = OverlayState::new();
    let n = names();
    assert_eq!(
        s.handle_key(OverlayKey::Toggle, &n, FIRST, &WIDE),
        OverlayAction::Redraw
    );
    assert!(s.is_open());
    assert_eq!(s.highlight(), 0);
    assert_eq!(
        s.handle_key(OverlayKey::Down, &n, FIRST, &WIDE),
        OverlayAction::Redraw
    );
    assert_eq!(s.highlight(), 1);
    assert_eq!(
        s.handle_key(OverlayKey::Down, &n, FIRST, &WIDE),
        OverlayAction::Redraw
    );
    assert_eq!(s.highlight(), 2); // the third row
    // Enter selects the third preset's absolute index and closes.
    assert_eq!(
        s.handle_key(OverlayKey::Enter, &n, FIRST, &WIDE),
        OverlayAction::Select(2)
    );
    assert!(!s.is_open());
}

#[test]
fn escape_closes_without_selecting() {
    let mut s = OverlayState::new();
    let n = names();
    s.handle_key(OverlayKey::Toggle, &n, FIRST, &WIDE);
    assert_eq!(
        s.handle_key(OverlayKey::Escape, &n, FIRST, &WIDE),
        OverlayAction::Close
    );
    assert!(!s.is_open());
}

/// **The browser opens where the show is** (Plan 0050 Phase 2). It used to
/// open on row 0, which loses your place in a roster the size of the shipped
/// one every time `Tab` is pressed.
///
/// The second half is the one that makes this a behaviour rather than a
/// cursor position: `Enter` straight after opening must re-select the preset
/// already playing, so `Tab`-`Enter` is a no-op instead of a jump to the
/// first preset in the list.
#[test]
fn opening_highlights_the_active_preset_and_enter_reselects_it() {
    let n: Vec<&str> = (0..10).map(|_| "x").collect();
    let mut s = OverlayState::new();
    assert_eq!(
        s.handle_key(OverlayKey::Toggle, &n, 7, &WIDE),
        OverlayAction::Redraw
    );
    assert_eq!(
        s.highlight(),
        7,
        "opened on row 0 instead of the active row"
    );
    assert_eq!(
        s.handle_key(OverlayKey::Enter, &n, 7, &WIDE),
        OverlayAction::Select(7)
    );

    // ...and it is genuinely reading `active` rather than remembering the
    // last highlight: a second open on a different active lands elsewhere.
    s.handle_key(OverlayKey::Toggle, &n, 2, &WIDE);
    assert_eq!(s.highlight(), 2);
}

/// Opening can never leave the highlight outside the visible list — the same
/// invariant `on_roster_changed` keeps, now that a second path sets it.
#[test]
fn opening_on_an_unreachable_active_index_falls_back_into_the_list() {
    let n = names();
    let mut s = OverlayState::new();
    // Past the end of the roster (a stale index from a shrunk hot-reload).
    s.handle_key(OverlayKey::Toggle, &n, 99, &WIDE);
    assert_eq!(s.highlight(), 0);
    assert!(s.visible(&n).len() > s.highlight());

    // An empty roster has no row to land on at all.
    let mut s = OverlayState::new();
    s.handle_key(OverlayKey::Toggle, &[], 3, &WIDE);
    assert_eq!(s.highlight(), 0);
    assert!(s.visible(&[]).is_empty());

    // And a filter narrowing the list under an open overlay still re-clamps
    // through the existing path, which open-on-active must not have broken.
    let mut s = OverlayState::new();
    s.handle_key(OverlayKey::Toggle, &n, 3, &WIDE);
    assert_eq!(s.highlight(), 3);
    s.on_roster_changed(&["alpha", "bravo"]);
    assert_eq!(s.highlight(), 1);
}

/// Wrap, both ends. **Replaces the two tests that asserted the clamp** — a
/// wrap with no test is the same as no wrap.
///
/// It wraps because `Space` does: `Roster::next_index` has always cycled, and
/// a browser that stops dead where the key beside it cycles is two mental
/// models of one roster.
#[test]
fn the_highlight_wraps_at_both_ends() {
    let mut s = OverlayState::new();
    let n = names();
    s.handle_key(OverlayKey::Toggle, &n, FIRST, &WIDE);

    // Down off the last row lands on the first.
    for _ in 0..3 {
        s.handle_key(OverlayKey::Down, &n, FIRST, &WIDE);
    }
    assert_eq!(s.highlight(), 3, "precondition: sitting on the last row");
    s.handle_key(OverlayKey::Down, &n, FIRST, &WIDE);
    assert_eq!(s.highlight(), 0, "Down off the last row did not wrap");

    // Up off the first row lands on the last.
    s.handle_key(OverlayKey::Up, &n, FIRST, &WIDE);
    assert_eq!(s.highlight(), 3, "Up off row 0 did not wrap");

    // A full lap returns to where it started, so the wrap is not off by one.
    for _ in 0..4 {
        s.handle_key(OverlayKey::Down, &n, FIRST, &WIDE);
    }
    assert_eq!(s.highlight(), 3);
}

/// A one-row list wraps onto itself rather than going out of range — the
/// degenerate case a `len - 1` wrap gets wrong.
#[test]
fn a_single_row_list_wraps_onto_itself() {
    let mut s = OverlayState::new();
    let n = vec!["only"];
    s.handle_key(OverlayKey::Toggle, &n, FIRST, &WIDE);
    for key in [OverlayKey::Down, OverlayKey::Up, OverlayKey::Down] {
        s.handle_key(key, &n, FIRST, &WIDE);
        assert_eq!(s.highlight(), 0);
    }
    assert_eq!(
        s.handle_key(OverlayKey::Enter, &n, FIRST, &WIDE),
        OverlayAction::Select(0)
    );
}

/// Only the highlight-moving keys are repeatable (Plan 0050 Phase 2). The
/// shell gates OS key repeat on this, so a key slipping into the `true` set
/// is a preset switch or a fullscreen toggle firing 30 times a second.
#[test]
fn only_navigation_keys_accept_key_repeat() {
    assert!(OverlayKey::Up.is_nav());
    assert!(OverlayKey::Down.is_nav());
    for key in [
        OverlayKey::Toggle,
        OverlayKey::Enter,
        OverlayKey::Escape,
        OverlayKey::Backspace,
        OverlayKey::Char('a'),
    ] {
        assert!(!key.is_nav(), "{key:?} must not be honoured on key repeat");
    }
}

fn type_str(s: &mut OverlayState, text: &str, names: &[&str]) {
    for c in text.chars() {
        s.handle_key(OverlayKey::Char(c), names, FIRST, &WIDE);
    }
}

#[test]
fn typing_filters_case_insensitively_and_selects_absolute_index() {
    let mut s = OverlayState::new();
    // "warp" sits at absolute index 2 and is capitalized.
    let n = vec!["Aurora", "Ember", "Warp", "Glacier"];
    s.handle_key(OverlayKey::Toggle, &n, FIRST, &WIDE);
    // Lowercase "war" matches "Warp" case-insensitively, nothing else.
    type_str(&mut s, "war", &n);
    let visible = s.visible(&n);
    assert_eq!(visible, [(2, "Warp")]);
    // Enter must carry Warp's ABSOLUTE index (2), not its filtered row (0).
    assert_eq!(
        s.handle_key(OverlayKey::Enter, &n, FIRST, &WIDE),
        OverlayAction::Select(2)
    );
}

#[test]
fn backspace_widens_the_filtered_list() {
    let mut s = OverlayState::new();
    let n = vec!["alpha", "altair", "beta"];
    s.handle_key(OverlayKey::Toggle, &n, FIRST, &WIDE);
    type_str(&mut s, "alt", &n);
    assert_eq!(s.visible(&n).len(), 1); // only "altair"
    s.handle_key(OverlayKey::Backspace, &n, FIRST, &WIDE); // -> "al"
    assert_eq!(s.visible(&n).len(), 2); // "alpha" + "altair" restored
}

#[test]
fn no_match_filter_yields_an_empty_list_not_a_stale_one() {
    let mut s = OverlayState::new();
    let n = names();
    s.handle_key(OverlayKey::Toggle, &n, FIRST, &WIDE);
    type_str(&mut s, "zzz", &n);
    assert!(s.visible(&n).is_empty());
    // Enter on an empty list closes without selecting.
    assert_eq!(
        s.handle_key(OverlayKey::Enter, &n, FIRST, &WIDE),
        OverlayAction::Close
    );
}

#[test]
fn reopening_clears_the_prior_filter() {
    let mut s = OverlayState::new();
    let n = names();
    s.handle_key(OverlayKey::Toggle, &n, FIRST, &WIDE); // open
    type_str(&mut s, "zzz", &n); // filters to nothing
    assert!(s.visible(&n).is_empty());
    s.handle_key(OverlayKey::Toggle, &n, FIRST, &WIDE); // close
    s.handle_key(OverlayKey::Toggle, &n, FIRST, &WIDE); // reopen -> fresh query
    assert_eq!(s.filter(), "");
    assert_eq!(s.visible(&n).len(), 4);
}

#[test]
fn roster_change_reclamps_the_highlight_and_keeps_open() {
    let mut s = OverlayState::new();
    let big = vec!["a", "b", "c", "d"];
    s.handle_key(OverlayKey::Toggle, &big, FIRST, &WIDE);
    for _ in 0..3 {
        s.handle_key(OverlayKey::Down, &big, FIRST, &WIDE);
    }
    assert_eq!(s.highlight(), 3);
    // A hot-reload shrinks the roster under the open overlay.
    let small = vec!["a", "b"];
    s.on_roster_changed(&small);
    assert_eq!(s.highlight(), 1); // clamped to the new last row
    assert!(s.is_open()); // still open
}
