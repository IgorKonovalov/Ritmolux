//! The preset browse overlay's modal state — pure and window-free, so it is
//! unit-testable without winit or a GPU (Plan 0008). The shell decodes platform
//! key events into [`OverlayKey`]s, feeds them here, and acts on the returned
//! [`OverlayAction`]; each frame while open it asks [`OverlayState::visible`]
//! for the (filtered) rows to draw. Typed characters narrow the list by a
//! case-insensitive substring filter, and [`OverlayAction::Select`] always
//! carries the **absolute** roster index so a filtered pick stays correct.

use std::borrow::Cow;

use rlx_core::render::theme::THEME;

use crate::console::Line;
use crate::settings::{SettingsState, SettingsView};

// ---------------------------------------------------------------------------
// The corner name plate (device px)
// ---------------------------------------------------------------------------

/// On-canvas active-preset-name label: top-left inset (device px) and font
/// size. Drawn in the theme's primary text role.
pub const NAME_INSET: f32 = 16.0;
pub const NAME_SIZE: f32 = 28.0;

/// The rotation countdown sits directly under the preset name, smaller and in
/// the dim text role: it is a status line about the show's cadence, not part of
/// the show.
pub const NEXT_TOP: f32 = NAME_INSET + NAME_SIZE + 6.0;
pub const NEXT_SIZE: f32 = 18.0;

/// How the corner name reports the marks the preset on screen carries.
///
/// Suffixed rather than prefixed so the name still starts at the same x on
/// every preset, and spelled out rather than glyphed: the browser's single
/// character has a column of them to be read against, and one floating in a
/// corner does not.
pub fn mark_suffix(favourite: bool, hidden: bool) -> &'static str {
    match (favourite, hidden) {
        (true, true) => "  (favourite, hidden)",
        (true, false) => "  (favourite)",
        (false, true) => "  (hidden)",
        (false, false) => "",
    }
}

/// The countdown line, or `None` when there is nothing to count down to.
///
/// `remaining` is the director's own answer and is already `None` while
/// auto-rotate is off, so "says nothing when it is off" is that `None` rather
/// than a second rule here. `enabled` is the operator's switch.
///
/// Whole seconds, rounded up, so the line reaches `1 s` and then the rotation
/// happens — a line that showed `0 s` for most of a second would read as a
/// stalled timer.
pub fn next_rotation_line(remaining: Option<f32>, enabled: bool) -> Option<String> {
    let secs = remaining.filter(|_| enabled)?;
    Some(format!("next in {} s", secs.ceil().max(0.0) as u32))
}

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

/// A row is three pieces at fixed offsets from its column's left edge: the
/// marker and mark glyph at `0`, the name at [`NAME_X`], the family at
/// [`FAMILY_X`]. Fixed offsets rather than space padding, because the font is
/// proportional and a padded name column is ragged.
///
/// **The name and the family are measured, not estimated.** Each is cut by
/// [`fit_width`](rlx_core::render::fit_width) against the caller's measurement
/// of the drawn font, so a name is shortened only when it really overruns its
/// slot — never by a per-character guess.
pub const NAME_X: f32 = 40.0;
/// The widest a drawn name may be.
pub const NAME_W: f32 = 290.0;
/// Left edge of the family label, a small gap past the name slot.
pub const FAMILY_X: f32 = NAME_X + NAME_W + 12.0;
/// The widest a drawn family label may be. `attractor`, the longest of the
/// closed roster [`SystemKind::family`](rlx_core::preset::SystemKind::family)
/// returns, fits it at [`ROW_SIZE`] on any common sans-serif.
pub const FAMILY_W: f32 = 100.0;
/// Gap between columns, so two full-width rows do not touch.
pub const COL_GUTTER: f32 = 24.0;
/// Horizontal pitch between columns.
pub const COL_W: f32 = FAMILY_X + FAMILY_W + COL_GUTTER;

/// Measures a run of text: `(text, font size in device px) -> width in device
/// px`, as the surface that draws it would lay it out. The shell passes the
/// renderer's own measurement; a test passes any function it likes, since
/// every property here holds for any measurement.
pub type Measure<'a> = dyn FnMut(&str, f32) -> f32 + 'a;

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

// ---------------------------------------------------------------------------
// The preview pane (ADR-0230)
// ---------------------------------------------------------------------------

/// The still's drawn size: a cached 160x90 at twice its pixels.
pub const PANE_IMAGE_W: f32 = 320.0;
pub const PANE_IMAGE_H: f32 = 180.0;
/// Gap between the image and its caption.
const PANE_CAPTION_GAP: f32 = 8.0;
/// The caption under the image, and the placeholder inside it.
pub const PANE_TEXT_SIZE: f32 = 18.0;
/// What the pane says for a preset with no picture yet. A normal state on a
/// first launch, so it reads as "not yet" rather than as a fault.
pub const PANE_PLACEHOLDER: &str = "no picture yet";

/// Where the pane draws on a surface: the image's rectangle, and the caption
/// line under it. Device px from the top-left, like every row.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pane {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    /// Top of the caption line under the image.
    pub caption_y: f32,
}

impl Pane {
    /// The placeholder's position, inside the image's rectangle.
    pub fn placeholder_at(&self) -> (f32, f32) {
        (
            self.x + PANE_TEXT_SIZE,
            self.y + (self.h - PANE_TEXT_SIZE) / 2.0,
        )
    }
}

/// The pane for a surface of `width` x `height`, anchored to its bottom-right
/// corner, or `None` when the surface cannot hold the image below the header
/// with the list's first column clear of it.
///
/// **The list is laid out exactly as it is without a pane.** [`layout`] is not
/// told about it: the columns flow top-down from the left, so the bottom-right
/// corner is the last place they reach, and the shipped roster leaves it empty
/// at 1920x1080. A library long enough to fill the last column to the bottom
/// draws its rows over the picture, text on top, rather than losing a column
/// to it.
///
/// **A function of the surface alone.** Whether a picture is cached decides what
/// is drawn inside the pane, never whether it exists or where — so nothing moves
/// as pictures arrive while the operator arrows through the list.
pub fn pane(width: f32, height: f32) -> Option<Pane> {
    let caption_y = height - LIST_INSET - ROW_H;
    let y = caption_y - PANE_CAPTION_GAP - PANE_IMAGE_H;
    let x = width - LIST_INSET - PANE_IMAGE_W;
    (x >= LIST_INSET + COL_W && y >= ROWS_TOP).then_some(Pane {
        x,
        y,
        w: PANE_IMAGE_W,
        h: PANE_IMAGE_H,
        caption_y,
    })
}

/// Which preset's picture the renderer is holding for the pane, so the cache is
/// read when the highlight moves to another preset rather than on every frame.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PaneSlot {
    /// The preset last loaded for, and whether a picture was found for it.
    loaded: Option<(String, bool)>,
}

impl PaneSlot {
    /// Whether the pane has to look `name` up again.
    pub fn needs_load(&self, name: &str) -> bool {
        self.loaded.as_ref().is_none_or(|(held, _)| held != name)
    }

    /// Record that `name` was looked up, and whether a picture was found.
    pub fn loaded(&mut self, name: &str, found: bool) {
        self.loaded = Some((name.to_owned(), found));
    }

    /// Whether the picture held is `name`'s. `false` for a preset looked up and
    /// not found — the placeholder's case.
    pub fn shows(&self, name: &str) -> bool {
        self.loaded
            .as_ref()
            .is_some_and(|(held, found)| *found && held == name)
    }

    /// Forget what was loaded, so the next frame looks it up again — for a
    /// renderer that dropped the picture with the device it lived on.
    pub fn forget(&mut self) {
        self.loaded = None;
    }

    /// A new picture of `name` was written: look it up again if it is the one
    /// held, found or not, and leave any other preset's picture alone.
    ///
    /// **The held picture stays on screen until the lookup replaces it.** The
    /// lookup happens on the next frame the pane is drawn, and the cache's
    /// reader is not held to the stamp, so a preset whose new picture is still
    /// being rendered keeps showing its previous one rather than a placeholder.
    pub fn landed(&mut self, name: &str) {
        if self.loaded.as_ref().is_some_and(|(held, _)| held == name) {
            self.loaded = None;
        }
    }
}

/// The F3 overlay's audio line, built from the **same** startup token the
/// `diagnostics.log` `capture` column carries — so a screenshot and a log from
/// one run cannot disagree about why the app is or is not hearing anything.
///
/// The label is what makes the token self-describing to a tester who has never
/// seen this app's log: `live SCK 48000/2` alone does not say it is about audio.
pub fn capture_line(token: &str) -> String {
    format!("audio  {token}")
}

/// `text` at [`ROW_SIZE`], shortened to fit `max_width` as `measure` lays it
/// out, with an ASCII ellipsis — `...` rather than `…`, because the overlay's
/// font coverage is not something this module can check.
///
/// Borrowed when it already fits, which is every shipped preset, so the common
/// case allocates nothing beyond what the caller was doing anyway.
pub fn fit<'a>(text: &'a str, max_width: f32, measure: &mut Measure<'_>) -> Cow<'a, str> {
    rlx_core::render::fit_width(text, max_width, |s| measure(s, ROW_SIZE))
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

/// One roster entry as the browser sees it: what it is called, which family it
/// belongs to, and how the user has marked it.
///
/// `family` is `&'static str` rather than borrowed from the caller because it
/// **is** static — `SystemKind::family` returns one of a closed roster compiled
/// into the binary. That is not a detail: it is what lets the shell build these
/// rows out of the marks store without the rows borrowing it, so the same call
/// can go on to take `&mut` the overlay.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Row<'a> {
    /// The preset's display name, which is also its identity (ADR-0228).
    pub name: &'a str,
    /// The filename family its system is named for — `curve`, `attractor`.
    pub family: &'static str,
    pub favourite: bool,
    pub hidden: bool,
}

/// The glyph a row's marks are drawn with, one character wide so the name
/// column starts at the same place on every row.
///
/// Hidden wins over favourite: a preset can carry both marks, and the one that
/// decides whether you see it is the one worth showing.
pub fn mark_glyph(row: &Row<'_>) -> char {
    match (row.hidden, row.favourite) {
        (true, _) => '-',
        (false, true) => '*',
        (false, false) => ' ',
    }
}

/// One drawn row whose column's left edge is `x`: the highlight marker and the
/// mark glyph, then the name and the family, each in its own slot and each cut
/// to that slot's measured width.
///
/// Built here rather than at the draw site so the pieces and the column
/// arithmetic above live together — a name that outgrew [`NAME_W`] would
/// collide with its family, and nothing on screen would say why.
pub fn row_lines(
    row: &Row<'_>,
    highlighted: bool,
    (x, y): (f32, f32),
    measure: &mut Measure<'_>,
    out: &mut Vec<Line>,
) {
    let (marker, color) = browse_row_style(highlighted, row.favourite);
    out.push(Line::new(
        format!("{marker}{}", mark_glyph(row)),
        x,
        y,
        ROW_SIZE,
        color,
    ));
    out.push(Line::new(
        fit(row.name, NAME_W, measure).into_owned(),
        x + NAME_X,
        y,
        ROW_SIZE,
        color,
    ));
    out.push(Line::new(
        fit(row.family, FAMILY_W, measure).into_owned(),
        x + FAMILY_X,
        y,
        ROW_SIZE,
        color,
    ));
}

/// The browser's header: what is typed, and every narrowing currently applied.
///
/// **Every active filter is named.** A narrowing that persists across opens and
/// says nothing is a roster that has mysteriously shrunk, which is the one way
/// this feature turns into a bug report.
pub fn header_text(state: &OverlayState) -> String {
    let mut parts: Vec<String> = Vec::new();
    if state.filter().is_empty() {
        parts.push("type to filter  -  arrows  enter  esc".to_owned());
    } else {
        parts.push(format!("filter: {}", state.filter()));
    }
    if let Some(family) = state.family() {
        parts.push(format!("family: {family}"));
    }
    if state.favourites_only() {
        parts.push("favourites".to_owned());
    }
    if state.show_hidden() {
        parts.push("+hidden".to_owned());
    }
    parts.join("   ")
}

// ---------------------------------------------------------------------------
// Composition: the lines a frame draws
// ---------------------------------------------------------------------------
//
// Pure functions of state, window size and fixed geometry. The shell calls them
// once per frame and `shot --ui` calls them against fixture state, so a headless
// capture of a modal is built by exactly the code the window draws with.

/// The marker and colour one **browse-list** row is drawn with.
///
/// **The cursor wins.** A highlighted row is the theme's `accent` whether or
/// not it is a favourite, so there is never a frame in which two rows could be
/// read as the one the keys act on. Below that, a favourite is the warm
/// `favourite` role — so a column of forty reads as marked-or-not at a glance,
/// which the one-character `*` does not survive — and everything else is plain
/// `text`.
///
/// Hidden rows have no colour of their own and keep `text` behind their `-`
/// glyph, so a preset carrying both marks draws warm with a `-` — the glyph's
/// own precedence rule meeting a colour that says otherwise. A third colour to
/// disambiguate a state this rare costs more than it returns.
///
/// The settings menu's rows are deliberately not routed through here: they carry
/// no marks, so a shared function would take a parameter that is always `false`.
pub fn browse_row_style(highlighted: bool, favourite: bool) -> (&'static str, [f32; 4]) {
    match (highlighted, favourite) {
        (true, _) => ("> ", THEME.accent.rgba()),
        (false, true) => ("  ", THEME.favourite.rgba()),
        (false, false) => ("  ", THEME.text.rgba()),
    }
}

/// The corner name plate: the active preset's name carrying its marks, and the
/// countdown under it when there is one.
///
/// The marks ride on the name rather than on a line of their own: marking is
/// worthless if you cannot see what is marked without opening the browser, and
/// a second line for two words is furniture the show does not need.
pub fn corner_lines(
    name: &str,
    favourite: bool,
    hidden: bool,
    countdown: Option<String>,
    measure: &mut Measure<'_>,
    out: &mut Vec<Line>,
) {
    let from = out.len();
    out.push(Line::new(
        format!("{name}{}", mark_suffix(favourite, hidden)),
        NAME_INSET,
        NAME_INSET,
        NAME_SIZE,
        THEME.text.rgba(),
    ));
    if let Some(text) = countdown {
        out.push(Line::new(
            text,
            NAME_INSET,
            NEXT_TOP,
            NEXT_SIZE,
            THEME.text_dim.rgba(),
        ));
    }
    push_backdrop(out, from, measure);
}

/// Append the backdrop that holds the text lines `out[from..]`, if any.
///
/// Appended **after** the lines it backs rather than before: the panels are one
/// draw under all of a frame's text whatever their place in the list, and a
/// block's first line stays the first line a reader of the list finds.
pub fn push_backdrop(out: &mut Vec<Line>, from: usize, measure: &mut Measure<'_>) {
    let backdrop = out
        .get(from..)
        .and_then(|block| Line::backdrop_around(block, |text, size| measure(text, size)));
    if let Some(backdrop) = backdrop {
        out.push(backdrop);
    }
}

/// The F3 capture line, placed under the core's diagnostics panel, in the dim
/// text role: a diagnostics line, not part of the show.
pub fn capture_verdict_line(token: &str) -> Line {
    Line::new(
        capture_line(token),
        NAME_INSET,
        CAPTURE_TOP,
        CAPTURE_SIZE,
        THEME.text_dim.rgba(),
    )
}

/// The settings menu: its key-hint header, then one row per setting with the
/// highlight marker on [`SettingsState::row`].
///
/// One column, always: the rows start at [`ROWS_TOP`] with a [`ROW_H`] pitch,
/// which fits any window this app opens in, and a settings menu that reflowed
/// would move a row out from under the operator's hand mid-edit.
pub fn settings_lines(
    state: &SettingsState,
    view: &SettingsView,
    measure: &mut Measure<'_>,
    out: &mut Vec<Line>,
) {
    let from = out.len();
    out.push(Line::new(
        "settings  -  up/down  left/right  esc".to_owned(),
        LIST_INSET,
        LIST_TOP,
        ROW_SIZE,
        THEME.text_dim.rgba(),
    ));
    for (row, (label, value)) in state.lines(view).into_iter().enumerate() {
        let (marker, color) = if row == state.row() {
            ("> ", THEME.accent.rgba())
        } else {
            ("  ", THEME.text.rgba())
        };
        let text = format!("{marker}{label:<14}{value}");
        let y = ROWS_TOP + row as f32 * ROW_H;
        if row == state.row() {
            let w = measure(&text, ROW_SIZE);
            out.push(highlight_at(LIST_INSET, y, w));
        }
        out.push(Line::new(text, LIST_INSET, y, ROW_SIZE, color));
    }
    push_backdrop(out, from, measure);
}

/// The browser: the header naming the query and every narrowing, then the
/// `visible` rows placed by `layout`, on one backdrop.
///
/// Every placement decision is [`layout`]'s, so this only turns `(column, row)`
/// into pixels. Rows the layout scrolls off answer `None` and are skipped.
pub fn browse_lines(
    state: &OverlayState,
    visible: &[(usize, Row<'_>)],
    layout: &ListLayout,
    measure: &mut Measure<'_>,
    out: &mut Vec<Line>,
) {
    let from = out.len();
    out.push(Line::new(
        header_text(state),
        LIST_INSET,
        LIST_TOP,
        ROW_SIZE,
        THEME.text_dim.rgba(),
    ));
    for (row, (_abs, entry)) in visible.iter().enumerate() {
        let Some((col, r)) = layout.place(row) else {
            continue;
        };
        let (x, y) = (LIST_INSET + col as f32 * COL_W, ROWS_TOP + r as f32 * ROW_H);
        let highlighted = row == state.highlight();
        row_lines(entry, highlighted, (x, y), measure, out);
        if highlighted {
            out.push(highlight_at(x, y, COL_W - COL_GUTTER));
        }
    }
    push_backdrop(out, from, measure);
}

/// Horizontal room a row highlight takes left of the row's text: the accent
/// bar and a gap before the marker.
const HIGHLIGHT_LEAD: f32 = 8.0;

/// The selection highlight under a row whose text starts at `(x, y)` and runs
/// `w` device px.
fn highlight_at(x: f32, y: f32, w: f32) -> Line {
    Line::highlight(x - HIGHLIGHT_LEAD, y - 1.0, w + HIGHLIGHT_LEAD, ROW_H)
}

/// The preview pane's text: the placeholder inside the image's rectangle while
/// `shown` is false, and the highlighted preset's name under it either way —
/// on one backdrop that also frames the image.
pub fn pane_lines(
    pane: &Pane,
    name: &str,
    shown: bool,
    measure: &mut Measure<'_>,
    out: &mut Vec<Line>,
) {
    let caption_w = measure(name, PANE_TEXT_SIZE);
    let backdrop = rlx_core::render::Panel::around(
        pane.x,
        pane.y,
        pane.x + pane.w.max(caption_w),
        pane.caption_y + PANE_TEXT_SIZE * rlx_core::render::text::LINE_HEIGHT_RATIO,
        THEME.space[1],
    );
    out.push(Line::backdrop(
        backdrop.x, backdrop.y, backdrop.w, backdrop.h,
    ));
    if !shown {
        let (x, y) = pane.placeholder_at();
        out.push(Line::new(
            PANE_PLACEHOLDER.to_owned(),
            x,
            y,
            PANE_TEXT_SIZE,
            THEME.text_faint.rgba(),
        ));
    }
    out.push(Line::new(
        name.to_owned(),
        pane.x,
        pane.caption_y,
        PANE_TEXT_SIZE,
        THEME.text_dim.rgba(),
    ));
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
    /// Narrow to the next family present in the roster, and off the end back to
    /// every family.
    Family,
    /// Show only favourites, or every eligible preset again.
    FavouritesOnly,
    /// Bring hidden presets back into the list, or send them away again.
    ShowHidden,
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
    /// The family the list is narrowed to, or `None` for every family.
    ///
    /// Owned rather than a roster position, because a hot-reload can replace the
    /// roster underneath: a stored index would silently become a different
    /// family, and a name the new roster lacks simply reads as no match.
    family: Option<String>,
    /// Show only presets marked favourite.
    favourites_only: bool,
    /// Show presets marked hidden, which the default view leaves out.
    ///
    /// **A mark you cannot find again is a mark you cannot undo**, which is the
    /// whole reason this state exists rather than hidden being simply gone.
    show_hidden: bool,
    /// Which preset's picture the preview pane holds. Kept beside the modal
    /// state because it follows the highlight, and touched by nothing here.
    pane: PaneSlot,
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

    /// The family the list is narrowed to, or `None` for every family.
    pub fn family(&self) -> Option<&str> {
        self.family.as_deref()
    }

    /// Whether the list is narrowed to favourites.
    pub fn favourites_only(&self) -> bool {
        self.favourites_only
    }

    /// Whether hidden presets are in the list.
    pub fn show_hidden(&self) -> bool {
        self.show_hidden
    }

    /// The preview pane's record of what it holds.
    pub fn pane_slot(&mut self) -> &mut PaneSlot {
        &mut self.pane
    }

    /// The rows to display as `(absolute roster index, row)`, narrowed by every
    /// filter that is on.
    ///
    /// The four narrowings are **independent and cumulative**: the typed query,
    /// one family, favourites only, and whether hidden presets are in the list
    /// at all. Each answers a different question, so combining them is what the
    /// browser is for rather than a mode nobody asked for.
    ///
    /// The **absolute** index is what [`OverlayAction::Select`] carries, so
    /// selection stays correct however narrow the visible list is — an
    /// off-by-one here would silently pick the wrong preset.
    pub fn visible<'a>(&self, rows: &[Row<'a>]) -> Vec<(usize, Row<'a>)> {
        let needle = self.filter.to_lowercase();
        rows.iter()
            .enumerate()
            .filter(|(_, row)| self.show_hidden || !row.hidden)
            .filter(|(_, row)| !self.favourites_only || row.favourite)
            .filter(|(_, row)| match &self.family {
                Some(family) => row.family == family,
                None => true,
            })
            .filter(|(_, row)| needle.is_empty() || row.name.to_lowercase().contains(&needle))
            .map(|(i, row)| (i, *row))
            .collect()
    }

    /// Re-clamp the highlight after the list changed under the overlay — a
    /// hot-reload swapping presets, or a mark that moved a row out of the view.
    /// Keeps the open state and every filter; only ensures the highlight still
    /// points at a visible row.
    pub fn on_roster_changed(&mut self, rows: &[Row<'_>]) {
        let len = self.visible(rows).len();
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
        rows: &[Row<'_>],
        active: usize,
        layout: &ListLayout,
    ) -> OverlayAction {
        // Toggle works regardless of open state; opening starts a fresh query.
        //
        // **The query resets and the narrowings do not.** A half-typed name is
        // a gesture that belongs to one visit; narrowing to a family or to
        // favourites is a decision, and re-making it on every `Tab` would be
        // the browser forgetting what it was asked. The header names each one
        // that is on, which is what keeps a persisted narrowing from reading as
        // a roster that shrank.
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
                self.highlight = self.row_of(active, rows).unwrap_or(0);
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
                self.step(false, rows);
                OverlayAction::Redraw
            }
            OverlayKey::Down => {
                self.step(true, rows);
                OverlayAction::Redraw
            }
            OverlayKey::Left => {
                self.step_col(false, rows, layout);
                OverlayAction::Redraw
            }
            OverlayKey::Right => {
                self.step_col(true, rows, layout);
                OverlayAction::Redraw
            }
            OverlayKey::Enter => {
                let visible = self.visible(rows);
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
            // The three narrowings. Each resets the highlight to the top for
            // the reason typing does: the row under it is about to mean a
            // different preset, and a cursor that stayed put would look like it
            // had moved on its own.
            OverlayKey::Family => {
                self.cycle_family(rows);
                self.highlight = 0;
                OverlayAction::Redraw
            }
            OverlayKey::FavouritesOnly => {
                self.favourites_only = !self.favourites_only;
                self.highlight = 0;
                OverlayAction::Redraw
            }
            OverlayKey::ShowHidden => {
                self.show_hidden = !self.show_hidden;
                self.highlight = 0;
                OverlayAction::Redraw
            }
            // Handled above; kept for exhaustiveness without `unreachable!`.
            OverlayKey::Toggle => OverlayAction::None,
        }
    }

    /// Advance the family narrowing: every family, then each family present in
    /// the roster in roster order, then back to every family.
    ///
    /// **Drawn from the rows rather than from the system roster**, so the cycle
    /// only ever offers a family the loaded library actually has — stepping
    /// through four empty lists to reach the fifth is a filter nobody uses.
    /// A narrowing whose family has left the roster restarts at the first, which
    /// is the only honest answer when the thing being stepped from is gone.
    fn cycle_family(&mut self, rows: &[Row<'_>]) {
        let mut families: Vec<&'static str> = Vec::new();
        for row in rows {
            if !families.contains(&row.family) {
                families.push(row.family);
            }
        }
        let next = match &self.family {
            None => families.first().copied(),
            Some(current) => match families.iter().position(|family| family == current) {
                Some(index) => families.get(index + 1).copied(),
                None => families.first().copied(),
            },
        };
        self.family = next.map(str::to_owned);
    }

    /// The visible row showing absolute roster index `abs`, or `None` when a
    /// filter hides it (or the index is past the roster's end).
    fn row_of(&self, abs: usize, rows: &[Row<'_>]) -> Option<usize> {
        self.visible(rows).iter().position(|&(i, _)| i == abs)
    }

    /// Move the highlight one row, **wrapping** at both ends.
    ///
    /// Wrap rather than clamp so the browser agrees with `Space`, whose
    /// `Roster::next_index` has cycled since it was written — a list that stops
    /// dead at row 0 while the key beside it cycles is two different mental
    /// models of the same roster.
    fn step(&mut self, down: bool, rows: &[Row<'_>]) {
        let len = self.visible(rows).len();
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
    fn step_col(&mut self, right: bool, rows: &[Row<'_>], layout: &ListLayout) {
        let len = self.visible(rows).len();
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
