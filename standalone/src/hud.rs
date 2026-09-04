//! What the shell draws over the show, and what it hands the operator console.
//!
//! Two free functions carry the visibility rules — [`preset_name_visible`] and
//! [`output_modal`] — so both are assertable as values with no window and no
//! GPU, the same discipline [`crate::overlay`] and [`crate::settings`] keep. The
//! rest is composition: this frame's text, split by destination, and the
//! console's own present.
//!
//! The browse list's **colours and geometry both live in [`crate::overlay`]**,
//! beside the pure layout function that reasons about them, so the pixels drawn
//! here and the arithmetic tested there cannot drift.

use crate::app_state::AppState;
use crate::console;
use crate::overlay::{
    self, HEADER_COLOR, LIST_INSET, LIST_TOP, ROW_COLOR, ROW_H, ROW_HL_COLOR, ROW_SIZE,
};

/// On-canvas active-preset-name label: top-left inset (device px), font size,
/// and a light near-white color legible over most scenes.
pub(crate) const NAME_INSET: f32 = 16.0;
pub(crate) const NAME_SIZE: f32 = 28.0;
pub(crate) const NAME_COLOR: [f32; 4] = [0.9, 0.95, 1.0, 1.0];

/// Which modal, if any, currently owns the keyboard and the canvas.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Modal {
    Browse,
    Settings,
}

/// Whether the corner preset name is drawn this frame (Plan 0096 Phase 1).
///
/// **Presence-based, not timed**: the name yields to anything drawn over it and
/// returns the instant that thing closes. Two things cover it — either modal
/// (whose header starts at [`LIST_TOP`] and crowds it from below) and the core's
/// F3 diagnostics panel, which composites *after* the text layer and so paints
/// straight over it. `enabled` is the operator's own switch (`[hud] preset_name`).
///
/// A free function, not a method, so the rule is assertable as a value with no
/// window and no GPU — the same discipline [`overlay`] and [`settings`] keep.
///
/// This governs the **show furniture only**. The F3 capture line is deliberately
/// not gated on it: that line exists *because* the panel is up (Plan 0083), so
/// the flag that hides the name must not take it with it.
pub(crate) fn preset_name_visible(modal: Option<Modal>, diagnostics: bool, enabled: bool) -> bool {
    enabled && modal.is_none() && !diagnostics
}

/// The modal **as the output sees it**: `None` while the operator console is
/// open, because the rows are drawn there instead.
///
/// [`preset_name_visible`] yields the corner name to whatever is drawn over it.
/// Once the console exists, "a modal is open" and "a modal covers the show" stop
/// being the same fact, and feeding the raw one to that rule would blank the
/// name on the projector every time the operator opened a menu on their own
/// screen — a visible change to the show, caused by a surface the audience
/// cannot see.
///
/// A free function beside the rule it feeds, so both are assertable as values.
pub(crate) fn output_modal(modal: Option<Modal>, console: console::Console) -> Option<Modal> {
    if console.is_open() { None } else { modal }
}

impl AppState {
    /// Which modal owns the keyboard and the canvas, if either.
    ///
    /// **One place, consulted by both routing and drawing.** Two `is_open()`
    /// calls kept in agreement by hand is how a key gets routed to the modal that
    /// is not on screen and silently swallowed.
    pub(crate) fn modal(&self) -> Option<Modal> {
        if self.hud.settings.is_open() {
            Some(Modal::Settings)
        } else if self.hud.browse.is_open() {
            Some(Modal::Browse)
        } else {
            None
        }
    }

    /// The browse list's layout for `visible_len` rows at the window's current
    /// size — the one place the shell turns a window into
    /// [`overlay::ListLayout`], so the drawing and the `Left`/`Right` keys can
    /// never disagree about where a row is.
    pub(crate) fn list_layout(&self, visible_len: usize) -> overlay::ListLayout {
        // Laid out against whichever surface will actually draw it. With the
        // console open that is the console: laying the browser out for the
        // output's 1920x1080 and then drawing it into a 900x640 window puts
        // every column but the first off the right edge and most of the roster
        // off the bottom, which is what the operator sees as truncated names.
        //
        // The console lays out at its **logical** size and the lines are scaled
        // down on the way out (`console::scale_lines`), so a smaller window gets
        // smaller type and more of the roster rather than a clipped corner of a
        // full-size grid.
        let (w, h) = match self.renderer.aux_size() {
            Some((w, h)) => console::logical_size(w as f32, h as f32),
            None => {
                let size = self.window.inner_size();
                (size.width as f32, size.height as f32)
            }
        };
        overlay::layout(visible_len, self.hud.browse.highlight(), w, h)
    }

    /// The factor the console's text is shrunk by, or `1.0` with none attached.
    pub(crate) fn console_scale(&self) -> f32 {
        match self.renderer.aux_size() {
            Some((_, h)) => console::scale(h as f32),
            None => 1.0,
        }
    }

    /// Build this frame's on-canvas text and hand it to the renderer: the active
    /// preset name in the corner when [`preset_name_visible`] allows it, plus —
    /// while a modal is open — that modal's own rows. Strings are owned locally
    /// so the renderer's `queue_text` (which copies them) needs no live borrow of
    /// the roster.
    pub(crate) fn queue_frame_text(&mut self) {
        // Taken out and put back rather than borrowed in place: the body below
        // calls `&self` methods (`modal`, `settings_view`, `roster_names`,
        // `list_layout`) while filling them, which a live `&mut self.field`
        // borrow would forbid. `take` leaves an empty Vec behind for the
        // duration and the originals - with their retained capacity - go back at
        // the end, so a steady-state frame does no allocation here.
        //
        // Two buffers, not one: `chrome` is the picture's own furniture and
        // always lands on the output, while `modal` follows the operator to
        // whichever surface is driving. `console::route_into` is what decides,
        // and it is the only thing that decides — a branch here that skipped
        // building the modal rows when the console is open would work today and
        // silently disagree with the routing the first time the rule changes.
        let mut chrome = std::mem::take(&mut self.hud.chrome_scratch);
        let mut modal = std::mem::take(&mut self.hud.modal_scratch);
        chrome.clear();
        modal.clear();

        // `output_modal`, not `modal`: with the console open the rows are not on
        // the show, so nothing covers the corner name and it must stay.
        let console_open = self.console_state();
        if preset_name_visible(
            output_modal(self.modal(), console_open),
            self.diagnostics.overlay_on,
            self.config.hud.preset_name,
        ) {
            chrome.push(console::Line::new(
                self.renderer.preset_name().to_owned(),
                NAME_INSET,
                NAME_INSET,
                NAME_SIZE,
                NAME_COLOR,
            ));
        }

        // The capture verdict, under the core's diagnostics panel and only while
        // it is up (Plan 0083). Built from the stored token rather than from the
        // capture state, so this line and the log's `capture` column are the same
        // sentence about the same run.
        if self.diagnostics.overlay_on {
            chrome.push(console::Line::new(
                overlay::capture_line(&self.capture.capture_token),
                NAME_INSET,
                overlay::CAPTURE_TOP,
                overlay::CAPTURE_SIZE,
                overlay::CAPTURE_COLOR,
            ));
        }

        if self.modal() == Some(Modal::Settings) {
            let view = self.settings_view();
            modal.push(console::Line::new(
                "settings  -  up/down  left/right  esc".to_owned(),
                LIST_INSET,
                LIST_TOP,
                ROW_SIZE,
                HEADER_COLOR,
            ));

            // One column, always: ten rows fit any window this app opens in —
            // they start at `ROWS_TOP` (94 px) with a 30 px pitch, so the last
            // ends at 394 px — and a settings menu that reflowed would move a row
            // out from under the operator's hand mid-edit.
            for (row, (label, value)) in self.hud.settings.lines(&view).into_iter().enumerate() {
                let y = overlay::ROWS_TOP + row as f32 * ROW_H;
                let (marker, color) = if row == self.hud.settings.row() {
                    ("> ", ROW_HL_COLOR)
                } else {
                    ("  ", ROW_COLOR)
                };
                modal.push(console::Line::new(
                    format!("{marker}{label:<14}{value}"),
                    LIST_INSET,
                    y,
                    ROW_SIZE,
                    color,
                ));
            }
        } else if self.modal() == Some(Modal::Browse) {
            let names = self.roster_names();
            let name_refs: Vec<&str> = names.iter().map(String::as_str).collect();
            let visible = self.hud.browse.visible(&name_refs);
            let highlight = self.hud.browse.highlight();

            // Header echoes the filter query (or a hint) above the list, so the
            // user sees what they've typed as it narrows the roster.
            let header = if self.hud.browse.filter().is_empty() {
                "type to filter  -  arrows  enter  esc".to_owned()
            } else {
                format!("filter: {}", self.hud.browse.filter())
            };
            modal.push(console::Line::new(
                header,
                LIST_INSET,
                LIST_TOP,
                ROW_SIZE,
                HEADER_COLOR,
            ));

            // Column-major flow (Plan 0050 Phase 3): every placement decision is
            // the pure `layout`, so this loop only turns `(column, row)` into
            // pixels. Rows the layout scrolls off answer `None` and are skipped.
            let layout = self.list_layout(visible.len());
            for (row, &(_abs, name)) in visible.iter().enumerate() {
                let Some((col, r)) = layout.place(row) else {
                    continue;
                };
                let x = LIST_INSET + col as f32 * overlay::COL_W;
                let y = overlay::ROWS_TOP + r as f32 * ROW_H;
                let (marker, color) = if row == highlight {
                    ("> ", ROW_HL_COLOR)
                } else {
                    ("  ", ROW_COLOR)
                };
                modal.push(console::Line::new(
                    format!("{marker}{}", overlay::fit(name)),
                    x,
                    y,
                    ROW_SIZE,
                    color,
                ));
            }
        }

        // The console's standing header, so an idle console still reads as live.
        // Queued after the routing has cleared last frame's lines and before the
        // modal rows land under it.
        console::route_into(
            &mut self.hud.frame_text,
            &mut chrome,
            &mut modal,
            console_open,
        );
        // The standing furniture — header, transport labels, staging line —
        // only while no modal is up. A browse list or a settings menu is what
        // the operator is reading, it starts at the same inset, and the two
        // overlap into an unreadable pile. The transport is a resting-state
        // surface; the modal has its own keys.
        if console_open.is_open() && self.modal().is_none() {
            // Built at the reference geometry like every routed line, so the one
            // scaling below moves all of them together.
            let names: Vec<&str> = self.renderer.preset_names().collect();
            let staging = console::staging_line(
                console::next_up(&names, self.renderer.active_index()),
                self.director.auto_enabled(),
                (
                    self.config.rotate.min_dwell_secs,
                    self.config.rotate.max_dwell_secs,
                ),
            );
            drop(names);
            let mut furniture = vec![console::header(self.renderer.preset_name())];
            furniture.extend(console::transport_lines(self.director.auto_enabled()));
            furniture.push(staging);
            self.hud.frame_text.console.splice(0..0, furniture);
        }
        if console_open.is_open() {
            // After the header joins them, so the whole console surface is
            // scaled by one factor and the header cannot drift off the rows.
            // Read before the mutable borrow, not inside the call.
            let s = self.console_scale();
            console::scale_lines(&mut self.hud.frame_text.console, s);
        }

        let runs = self.hud.frame_text.output_runs();
        self.renderer.queue_text(&runs);

        // `runs` borrows `self.hud.frame_text`, so the scratch buffers can only go
        // home once its last use is behind us.
        drop(runs);
        self.hud.chrome_scratch = chrome;
        self.hud.modal_scratch = modal;
    }

    /// Whether the operator console is open this frame.
    pub(crate) fn console_state(&self) -> console::Console {
        if self.hud.console_window.is_some() {
            console::Console::Open
        } else {
            console::Console::Closed
        }
    }

    /// Present the console's half of this frame, if one is attached.
    ///
    /// Separate from the output's `render` and after it: the console is a
    /// monitor, so a frame it drops or a present that stalls must cost the show
    /// nothing. A failure here closes the console rather than killing the app.
    pub(crate) fn present_console(&mut self) {
        if self.hud.console_window.is_none() {
            return;
        }
        let runs = self.hud.frame_text.console_runs();
        let result = self.renderer.present_aux(&runs);
        drop(runs);
        if let Err(err) = result {
            eprintln!("console present failed, closing it: {err}");
            self.close_console();
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::{Modal, output_modal, preset_name_visible};
    use crate::console;

    #[test]
    fn name_shows_when_nothing_covers_it() {
        assert!(preset_name_visible(None, false, true));
    }

    #[test]
    fn diagnostics_panel_takes_the_corner() {
        // The panel composites after the text layer, so a name drawn here would
        // be painted over rather than shown beside it.
        assert!(!preset_name_visible(None, true, true));
    }

    #[test]
    fn either_modal_suppresses_the_name() {
        assert!(!preset_name_visible(Some(Modal::Settings), false, true));
        assert!(!preset_name_visible(Some(Modal::Browse), false, true));
    }

    #[test]
    fn the_operator_switch_wins_over_everything() {
        // Off means off in every state, not just the uncovered one.
        assert!(!preset_name_visible(None, false, false));
        assert!(!preset_name_visible(None, true, false));
        assert!(!preset_name_visible(Some(Modal::Browse), false, false));
    }

    /// A modal opened on the console does not cover the show, so the corner name
    /// stays on it. Without this the operator opening a menu on their own screen
    /// would blank a line on the projector.
    #[test]
    fn a_modal_on_the_console_leaves_the_shows_name_alone() {
        use console::Console;

        for modal in [Modal::Browse, Modal::Settings] {
            assert!(preset_name_visible(
                output_modal(Some(modal), Console::Open),
                false,
                true
            ));
        }
    }

    /// With no console, the rule is exactly what it was: the modal is on the
    /// show and covers the name.
    #[test]
    fn a_modal_on_the_output_still_suppresses_the_name() {
        use console::Console;

        for modal in [Modal::Browse, Modal::Settings] {
            assert!(!preset_name_visible(
                output_modal(Some(modal), Console::Closed),
                false,
                true
            ));
        }
    }

    /// The diagnostics panel is on the show either way, so it covers the name
    /// whatever the console is doing — the console relocates modals, not F3.
    #[test]
    fn the_console_does_not_rescue_the_name_from_the_panel() {
        use console::Console;

        assert!(!preset_name_visible(
            output_modal(Some(Modal::Browse), Console::Open),
            true,
            true
        ));
    }
}
