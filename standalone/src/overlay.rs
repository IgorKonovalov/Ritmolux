//! The preset browse overlay's modal state — pure and window-free, so it is
//! unit-testable without winit or a GPU (Plan 0008). The shell decodes platform
//! key events into [`OverlayKey`]s, feeds them here, and acts on the returned
//! [`OverlayAction`]; each frame while open it asks [`OverlayState::visible`]
//! for the (filtered) rows to draw. Typed characters narrow the list by a
//! case-insensitive substring filter, and [`OverlayAction::Select`] always
//! carries the **absolute** roster index so a filtered pick stays correct.

/// A key the overlay reacts to, decoded from the platform's input upstream so
/// this module stays free of winit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverlayKey {
    /// Open the overlay when closed, close it when open.
    Toggle,
    /// Move the highlight up one row, **wrapping** past the top to the last row.
    Up,
    /// Move the highlight down one row, **wrapping** past the last row to the top.
    Down,
    /// Commit the highlighted preset and close.
    Enter,
    /// Close without selecting.
    Escape,
    /// Append a printable character to the type-to-filter query.
    Char(char),
    /// Delete the last character of the filter query.
    Backspace,
}

impl OverlayKey {
    /// Whether this key only moves the highlight — the set the shell honours OS
    /// **key repeat** for (Plan 0050 Phase 2).
    ///
    /// Deliberately narrow. Repeat reaches nothing else, because holding `Space`
    /// would machine-gun preset switches through a ~1 s dissolve each, holding
    /// `Toggle` would strobe the modal, and holding `Enter` would commit
    /// repeatedly. A nav key only moves a cursor, so there is nothing expensive
    /// to repeat and no throttle is needed.
    pub fn is_nav(self) -> bool {
        matches!(self, OverlayKey::Up | OverlayKey::Down)
    }
}

/// What the shell should do after a key is fed to the overlay.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverlayAction {
    /// The key was ignored (e.g. a nav key while closed) — the shell's normal
    /// bindings (Space-cycle, …) stay in effect.
    None,
    /// Visible state changed; the shell should request a redraw.
    Redraw,
    /// Close the overlay without changing the preset.
    Close,
    /// Select the preset at this **absolute** roster index, then the overlay
    /// has closed itself.
    Select(usize),
}

/// The overlay's modal state: whether it is open and which visible row is
/// highlighted. The roster is not owned here — the caller passes the current
/// preset names into each method, so a hot-reload that swaps the roster needs no
/// coordination with this state (Phase 4 leans on that).
#[derive(Clone, Debug, Default)]
pub struct OverlayState {
    open: bool,
    /// Index into the *visible* (filtered) list, not the absolute roster.
    highlight: usize,
    /// Case-insensitive substring filter; empty means "show the whole roster".
    filter: String,
}

impl OverlayState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether the overlay is currently open (the shell draws its list and
    /// suppresses Space-cycle while it is).
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// The highlighted row's index into the visible list.
    pub fn highlight(&self) -> usize {
        self.highlight
    }

    /// The current filter query (for the shell to echo, e.g. in the list header).
    pub fn filter(&self) -> &str {
        &self.filter
    }

    /// The rows to display as `(absolute roster index, name)`, narrowed by the
    /// case-insensitive substring filter (empty filter → the whole roster in
    /// order). The **absolute** index is what [`OverlayAction::Select`] carries,
    /// so selection stays correct even when the visible list is a filtered
    /// subset — an off-by-one here would silently pick the wrong preset.
    pub fn visible<'a>(&self, names: &[&'a str]) -> Vec<(usize, &'a str)> {
        if self.filter.is_empty() {
            return names.iter().enumerate().map(|(i, &n)| (i, n)).collect();
        }
        let needle = self.filter.to_lowercase();
        names
            .iter()
            .enumerate()
            .filter(|(_, n)| n.to_lowercase().contains(&needle))
            .map(|(i, &n)| (i, n))
            .collect()
    }

    /// Re-clamp the highlight after the roster changed under the overlay (a
    /// hot-reload swapped presets). Keeps the open state and the filter — only
    /// ensures the highlight still points at a visible row, so a shrunk roster
    /// or a filter that now matches fewer rows never leaves a stale highlight.
    pub fn on_roster_changed(&mut self, names: &[&str]) {
        let len = self.visible(names).len();
        if len == 0 {
            self.highlight = 0;
        } else if self.highlight >= len {
            self.highlight = len - 1;
        }
    }

    /// Feed one key; mutate state and report what the shell should do. `names`
    /// is the current roster in order, `active` the **absolute** roster index of
    /// the preset currently playing — the row a fresh open highlights.
    ///
    /// `active` is passed in rather than held, which is what keeps this module
    /// roster-free and window-free: the shell reads it off the renderer, and this
    /// state machine never learns what a preset is.
    pub fn handle_key(&mut self, key: OverlayKey, names: &[&str], active: usize) -> OverlayAction {
        // Toggle works regardless of open state; opening starts a fresh query.
        if key == OverlayKey::Toggle {
            self.open = !self.open;
            if self.open {
                self.filter.clear();
                // Open **where the show is**, not at row 0. With the filter just
                // cleared the visible list is the whole roster, so this is a
                // lookup rather than a special case — and it falls back to the
                // top when `active` names no visible row (an empty roster, or an
                // index past its end), which is what keeps the highlight inside
                // the list the same way `on_roster_changed` does.
                self.highlight = self.row_of(active, names).unwrap_or(0);
            }
            return OverlayAction::Redraw;
        }
        // Every other key is inert while closed, so Space-cycle et al. are
        // unaffected (the shell only cycles when this returns `None`).
        if !self.open {
            return OverlayAction::None;
        }
        match key {
            OverlayKey::Up => {
                self.step(false, names);
                OverlayAction::Redraw
            }
            OverlayKey::Down => {
                self.step(true, names);
                OverlayAction::Redraw
            }
            OverlayKey::Enter => {
                let visible = self.visible(names);
                self.open = false;
                match visible.get(self.highlight) {
                    Some(&(abs, _)) => OverlayAction::Select(abs),
                    None => OverlayAction::Close, // empty list: nothing to pick
                }
            }
            OverlayKey::Escape => {
                self.open = false;
                OverlayAction::Close
            }
            // Type-to-filter: narrowing resets the highlight to the first match.
            OverlayKey::Char(c) => {
                self.filter.push(c);
                self.highlight = 0;
                OverlayAction::Redraw
            }
            OverlayKey::Backspace => {
                self.filter.pop();
                self.highlight = 0;
                OverlayAction::Redraw
            }
            // Handled above; kept for exhaustiveness without `unreachable!`.
            OverlayKey::Toggle => OverlayAction::None,
        }
    }

    /// The visible row showing absolute roster index `abs`, or `None` when the
    /// filter hides it (or the index is past the roster's end).
    fn row_of(&self, abs: usize, names: &[&str]) -> Option<usize> {
        self.visible(names).iter().position(|&(i, _)| i == abs)
    }

    /// Move the highlight one row, **wrapping** at both ends.
    ///
    /// Wrap rather than clamp so the browser agrees with `Space`, whose
    /// `Roster::next_index` has cycled since it was written — a list that stops
    /// dead at row 0 while the key beside it cycles is two different mental
    /// models of the same roster.
    fn step(&mut self, down: bool, names: &[&str]) {
        let len = self.visible(names).len();
        if len == 0 {
            self.highlight = 0;
            return;
        }
        let last = len - 1;
        self.highlight = if down {
            if self.highlight >= last {
                0
            } else {
                self.highlight + 1
            }
        } else if self.highlight == 0 {
            last
        } else {
            self.highlight - 1
        };
    }
}

#[cfg(test)]
mod tests {
    use super::{OverlayAction, OverlayKey, OverlayState};

    const NAMES: [&str; 4] = ["alpha", "bravo", "charlie", "delta"];

    fn names() -> Vec<&'static str> {
        NAMES.to_vec()
    }

    /// The roster's first preset is active. Most tests below predate open-on-
    /// active and only care about navigation, so they open on row 0 as they did.
    const FIRST: usize = 0;

    #[test]
    fn nav_keys_are_inert_while_closed() {
        let mut s = OverlayState::new();
        let n = names();
        // A closed overlay ignores nav keys so the shell's Space-cycle still runs.
        assert_eq!(
            s.handle_key(OverlayKey::Down, &n, FIRST),
            OverlayAction::None
        );
        assert_eq!(
            s.handle_key(OverlayKey::Enter, &n, FIRST),
            OverlayAction::None
        );
        assert_eq!(
            s.handle_key(OverlayKey::Escape, &n, FIRST),
            OverlayAction::None
        );
        assert!(!s.is_open());
    }

    #[test]
    fn open_navigate_and_select_emits_the_absolute_index() {
        let mut s = OverlayState::new();
        let n = names();
        assert_eq!(
            s.handle_key(OverlayKey::Toggle, &n, FIRST),
            OverlayAction::Redraw
        );
        assert!(s.is_open());
        assert_eq!(s.highlight(), 0);
        assert_eq!(
            s.handle_key(OverlayKey::Down, &n, FIRST),
            OverlayAction::Redraw
        );
        assert_eq!(s.highlight(), 1);
        assert_eq!(
            s.handle_key(OverlayKey::Down, &n, FIRST),
            OverlayAction::Redraw
        );
        assert_eq!(s.highlight(), 2); // the third row
        // Enter selects the third preset's absolute index and closes.
        assert_eq!(
            s.handle_key(OverlayKey::Enter, &n, FIRST),
            OverlayAction::Select(2)
        );
        assert!(!s.is_open());
    }

    #[test]
    fn escape_closes_without_selecting() {
        let mut s = OverlayState::new();
        let n = names();
        s.handle_key(OverlayKey::Toggle, &n, FIRST);
        assert_eq!(
            s.handle_key(OverlayKey::Escape, &n, FIRST),
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
            s.handle_key(OverlayKey::Toggle, &n, 7),
            OverlayAction::Redraw
        );
        assert_eq!(
            s.highlight(),
            7,
            "opened on row 0 instead of the active row"
        );
        assert_eq!(
            s.handle_key(OverlayKey::Enter, &n, 7),
            OverlayAction::Select(7)
        );

        // ...and it is genuinely reading `active` rather than remembering the
        // last highlight: a second open on a different active lands elsewhere.
        s.handle_key(OverlayKey::Toggle, &n, 2);
        assert_eq!(s.highlight(), 2);
    }

    /// Opening can never leave the highlight outside the visible list — the same
    /// invariant `on_roster_changed` keeps, now that a second path sets it.
    #[test]
    fn opening_on_an_unreachable_active_index_falls_back_into_the_list() {
        let n = names();
        let mut s = OverlayState::new();
        // Past the end of the roster (a stale index from a shrunk hot-reload).
        s.handle_key(OverlayKey::Toggle, &n, 99);
        assert_eq!(s.highlight(), 0);
        assert!(s.visible(&n).len() > s.highlight());

        // An empty roster has no row to land on at all.
        let mut s = OverlayState::new();
        s.handle_key(OverlayKey::Toggle, &[], 3);
        assert_eq!(s.highlight(), 0);
        assert!(s.visible(&[]).is_empty());

        // And a filter narrowing the list under an open overlay still re-clamps
        // through the existing path, which open-on-active must not have broken.
        let mut s = OverlayState::new();
        s.handle_key(OverlayKey::Toggle, &n, 3);
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
        s.handle_key(OverlayKey::Toggle, &n, FIRST);

        // Down off the last row lands on the first.
        for _ in 0..3 {
            s.handle_key(OverlayKey::Down, &n, FIRST);
        }
        assert_eq!(s.highlight(), 3, "precondition: sitting on the last row");
        s.handle_key(OverlayKey::Down, &n, FIRST);
        assert_eq!(s.highlight(), 0, "Down off the last row did not wrap");

        // Up off the first row lands on the last.
        s.handle_key(OverlayKey::Up, &n, FIRST);
        assert_eq!(s.highlight(), 3, "Up off row 0 did not wrap");

        // A full lap returns to where it started, so the wrap is not off by one.
        for _ in 0..4 {
            s.handle_key(OverlayKey::Down, &n, FIRST);
        }
        assert_eq!(s.highlight(), 3);
    }

    /// A one-row list wraps onto itself rather than going out of range — the
    /// degenerate case a `len - 1` wrap gets wrong.
    #[test]
    fn a_single_row_list_wraps_onto_itself() {
        let mut s = OverlayState::new();
        let n = vec!["only"];
        s.handle_key(OverlayKey::Toggle, &n, FIRST);
        for key in [OverlayKey::Down, OverlayKey::Up, OverlayKey::Down] {
            s.handle_key(key, &n, FIRST);
            assert_eq!(s.highlight(), 0);
        }
        assert_eq!(
            s.handle_key(OverlayKey::Enter, &n, FIRST),
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
            s.handle_key(OverlayKey::Char(c), names, FIRST);
        }
    }

    #[test]
    fn typing_filters_case_insensitively_and_selects_absolute_index() {
        let mut s = OverlayState::new();
        // "warp" sits at absolute index 2 and is capitalized.
        let n = vec!["Aurora", "Ember", "Warp", "Glacier"];
        s.handle_key(OverlayKey::Toggle, &n, FIRST);
        // Lowercase "war" matches "Warp" case-insensitively, nothing else.
        type_str(&mut s, "war", &n);
        let visible = s.visible(&n);
        assert_eq!(visible, [(2, "Warp")]);
        // Enter must carry Warp's ABSOLUTE index (2), not its filtered row (0).
        assert_eq!(
            s.handle_key(OverlayKey::Enter, &n, FIRST),
            OverlayAction::Select(2)
        );
    }

    #[test]
    fn backspace_widens_the_filtered_list() {
        let mut s = OverlayState::new();
        let n = vec!["alpha", "altair", "beta"];
        s.handle_key(OverlayKey::Toggle, &n, FIRST);
        type_str(&mut s, "alt", &n);
        assert_eq!(s.visible(&n).len(), 1); // only "altair"
        s.handle_key(OverlayKey::Backspace, &n, FIRST); // -> "al"
        assert_eq!(s.visible(&n).len(), 2); // "alpha" + "altair" restored
    }

    #[test]
    fn no_match_filter_yields_an_empty_list_not_a_stale_one() {
        let mut s = OverlayState::new();
        let n = names();
        s.handle_key(OverlayKey::Toggle, &n, FIRST);
        type_str(&mut s, "zzz", &n);
        assert!(s.visible(&n).is_empty());
        // Enter on an empty list closes without selecting.
        assert_eq!(
            s.handle_key(OverlayKey::Enter, &n, FIRST),
            OverlayAction::Close
        );
    }

    #[test]
    fn reopening_clears_the_prior_filter() {
        let mut s = OverlayState::new();
        let n = names();
        s.handle_key(OverlayKey::Toggle, &n, FIRST); // open
        type_str(&mut s, "zzz", &n); // filters to nothing
        assert!(s.visible(&n).is_empty());
        s.handle_key(OverlayKey::Toggle, &n, FIRST); // close
        s.handle_key(OverlayKey::Toggle, &n, FIRST); // reopen -> fresh query
        assert_eq!(s.filter(), "");
        assert_eq!(s.visible(&n).len(), 4);
    }

    #[test]
    fn roster_change_reclamps_the_highlight_and_keeps_open() {
        let mut s = OverlayState::new();
        let big = vec!["a", "b", "c", "d"];
        s.handle_key(OverlayKey::Toggle, &big, FIRST);
        for _ in 0..3 {
            s.handle_key(OverlayKey::Down, &big, FIRST);
        }
        assert_eq!(s.highlight(), 3);
        // A hot-reload shrinks the roster under the open overlay.
        let small = vec!["a", "b"];
        s.on_roster_changed(&small);
        assert_eq!(s.highlight(), 1); // clamped to the new last row
        assert!(s.is_open()); // still open
    }
}
