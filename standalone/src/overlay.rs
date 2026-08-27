//! The preset browse overlay's modal state — pure and window-free, so it is
//! unit-testable without winit or a GPU (Plan 0008). The shell decodes platform
//! key events into [`OverlayKey`]s, feeds them here, and acts on the returned
//! [`OverlayAction`]; each frame while open it asks [`OverlayState::visible`]
//! for the (filtered) rows to draw. Typed characters narrow the list by a
//! case-insensitive substring filter, and [`OverlayAction::Select`] always
//! carries the **absolute** roster index so a filtered pick stays correct.

use std::borrow::Cow;

// ---------------------------------------------------------------------------
// List geometry (device px)
// ---------------------------------------------------------------------------
//
// These live here rather than in `main.rs` because [`layout`] is the thing that
// reasons about them, and a layout function reading its constants from its caller
// is a layout function that cannot be unit-tested. `main.rs` draws with the same
// values, imported from here, so the pixels and the arithmetic cannot drift.

/// Left inset of the first column, and the header's.
pub const LIST_INSET: f32 = 16.0;
/// Top of the filter-echo header; the rows start one `ROW_H` below it.
pub const LIST_TOP: f32 = 64.0;
/// Row pitch.
pub const ROW_H: f32 = 30.0;
/// Row font size.
pub const ROW_SIZE: f32 = 22.0;

/// Top of the first row — the header occupies the band above it.
pub const ROWS_TOP: f32 = LIST_TOP + ROW_H;

/// **Column width is an estimate, not a measurement**, and deliberately.
///
/// glyphon shapes a proportional system font and `core` exposes no
/// text-measurement API; adding one to place a list is out of proportion to the
/// problem (ADR-0009 would need a supplement). So the width is derived from the
/// font size and a character budget, and [`fit`] truncates a name that overruns
/// the budget — which makes an **under**estimate cosmetic rather than a
/// collision, and an overestimate merely wasted horizontal space.
///
/// `0.62` is a conservative advance-per-character ratio for a proportional face
/// at this size: real lowercase Latin averages nearer `0.5`, and the roster's
/// names are mixed case with spaces.
const CHAR_W: f32 = ROW_SIZE * 0.62;
/// Characters a column reserves, including the two-character `"> "` marker.
const COL_CHARS: usize = 26;
/// Characters of the **name** a column can show before [`fit`] truncates it. The
/// longest shipped preset name is 15, so truncation never fires on the embedded
/// set; it exists for a custom `LMV_PRESET_DIR`.
pub const NAME_CHARS: usize = COL_CHARS - 2;
/// Gap between columns, so two full-width names do not touch.
const COL_GUTTER: f32 = 24.0;
/// Horizontal pitch between columns.
pub const COL_W: f32 = CHAR_W * COL_CHARS as f32 + COL_GUTTER;

// ---------------------------------------------------------------------------
// The F3 capture line (Plan 0083)
// ---------------------------------------------------------------------------

/// Top of the capture line, in device px, and its font size.
///
/// **Below the core's diagnostics panel, deliberately.** That panel is fixed
/// geometry (`core/src/render/overlay.rs`: a 12 px margin, five analysis rows
/// under the sparkline and the GPU bar) whose bottom edge lands at ~240 px, and
/// it composites *after* this text layer — so a line placed inside its band would
/// be painted over rather than merely crowded. This clears it, which keeps the
/// audio verdict and the frame-time block readable as one screenshot.
pub const CAPTURE_TOP: f32 = 252.0;
pub const CAPTURE_SIZE: f32 = 18.0;
/// Dimmer than the preset name, brighter than a browse row: a diagnostics line,
/// not part of the show.
pub const CAPTURE_COLOR: [f32; 4] = [0.72, 0.80, 0.90, 0.95];

/// The F3 overlay's audio line, built from the **same** startup token the
/// `diagnostics.log` `capture` column carries — so a screenshot and a log from
/// one run cannot disagree about why the app is or is not hearing anything.
///
/// The label is what makes the token self-describing to a tester who has never
/// seen this app's log: `live SCK 48000/2` alone does not say it is about audio.
pub fn capture_line(token: &str) -> String {
    format!("audio  {token}")
}

/// A name shortened to fit one column, with an ASCII ellipsis.
///
/// Borrowed when it already fits, which is every shipped preset — so the common
/// case allocates nothing beyond what the caller was doing anyway. ASCII `...`
/// rather than `…` because the overlay's font coverage is not something this
/// module can check.
pub fn fit(name: &str) -> Cow<'_, str> {
    if name.chars().count() <= NAME_CHARS {
        return Cow::Borrowed(name);
    }
    let keep = NAME_CHARS.saturating_sub(3);
    let mut out: String = name.chars().take(keep).collect();
    out.push_str("...");
    Cow::Owned(out)
}

/// How the visible rows are placed on screen: a column-major flow, as many
/// columns as fit, scrolled by whole columns when even those cannot hold the
/// roster.
///
/// A **pure function of `(visible_len, highlight, width, height)`** — no window,
/// no roster — so the whole thing is unit-testable and the eyes-on check confirms
/// pixels rather than logic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ListLayout {
    /// Columns actually drawn: what the roster needs, capped by what fits.
    pub cols: usize,
    /// Rows in one full column — today's vertical arithmetic, unchanged.
    pub rows_per_col: usize,
    /// Scroll offset in whole columns. `0` whenever the roster fits, which is
    /// every case the shipped set reaches at a normal window size.
    pub col_scroll: usize,
    /// Rows this layout was built for, so [`place`](Self::place) is **total**:
    /// a row past the end answers `None` rather than a grid cell that exists on
    /// screen but holds nothing.
    pub len: usize,
}

impl ListLayout {
    /// Where visible row `row` is drawn, as `(column on screen, row within that
    /// column)` — or `None` when there is no such row, or it is scrolled off.
    pub fn place(&self, row: usize) -> Option<(usize, usize)> {
        if self.rows_per_col == 0 || row >= self.len {
            return None;
        }
        let col = row / self.rows_per_col;
        if col < self.col_scroll || col >= self.col_scroll + self.cols {
            return None;
        }
        Some((col - self.col_scroll, row % self.rows_per_col))
    }
}

/// Lay out `visible_len` rows in a surface of `width` x `height` device px, with
/// `highlight` the row that must stay on screen.
///
/// Vertical arithmetic is the single-column list's (`floor((height -
/// ROWS_TOP) / ROW_H)`, at least one row), so a change to `ROW_H` or
/// `LIST_TOP` moves the pinned numbers in the tests deliberately.
/// Horizontally the roster asks for `ceil(len / rows_per_col)` columns and
/// gets however many fit, with scrolling as the fallback for the case where
/// the columns still cannot hold it.
pub fn layout(visible_len: usize, highlight: usize, width: f32, height: f32) -> ListLayout {
    let rows_per_col = (((height - ROWS_TOP) / ROW_H).floor().max(1.0)) as usize;
    let needed = visible_len.div_ceil(rows_per_col).max(1);
    let fits = (((width - LIST_INSET) / COL_W).floor().max(1.0)) as usize;
    let cols = needed.min(fits);

    // Window the columns so the highlighted one is on screen, pinned to the right
    // edge when scrolling (the same shape the single-column scroll had).
    let hl_col = highlight / rows_per_col;
    let col_scroll = hl_col
        .saturating_sub(cols.saturating_sub(1))
        .min(needed.saturating_sub(cols));

    ListLayout {
        cols,
        rows_per_col,
        col_scroll,
        len: visible_len,
    }
}

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
    /// Move one column left, **clamped** at the first column.
    Left,
    /// Move one column right, **clamped** at the last column.
    Right,
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
        matches!(
            self,
            OverlayKey::Up | OverlayKey::Down | OverlayKey::Left | OverlayKey::Right
        )
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
    pub fn handle_key(
        &mut self,
        key: OverlayKey,
        names: &[&str],
        active: usize,
        layout: &ListLayout,
    ) -> OverlayAction {
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
            OverlayKey::Left => {
                self.step_col(false, names, layout);
                OverlayAction::Redraw
            }
            OverlayKey::Right => {
                self.step_col(true, names, layout);
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

    /// Move the highlight one **column**, clamped at both edges.
    ///
    /// Clamped rather than wrapped, unlike the vertical: vertical wrap is what
    /// the user asked for and matches `Space`, while horizontal wrap in a
    /// column-major grid teleports you a whole roster away. `Down` already
    /// continues at the top of the next column for free — that is what
    /// column-major *is* — so the arrows are not redundant with each other.
    ///
    /// A ragged final column keeps the move rather than refusing it: stepping
    /// right onto a row the short column does not have lands on its last row,
    /// which is what every list widget does and what stops `Right` feeling
    /// broken on exactly the column the shipped roster produces.
    fn step_col(&mut self, right: bool, names: &[&str], layout: &ListLayout) {
        let len = self.visible(names).len();
        let rpc = layout.rows_per_col;
        if len == 0 || rpc == 0 {
            return;
        }
        let col = self.highlight / rpc;
        let row = self.highlight % rpc;
        if right {
            let first = (col + 1) * rpc;
            // No next column at all: stay put.
            if first < len {
                self.highlight = (first + row).min(len - 1);
            }
        } else if col > 0 {
            // The column to the left is always full, so this row exists.
            self.highlight = (col - 1) * rpc + row;
        }
    }
}

#[cfg(test)]
mod tests;
