//! What the shell draws over the show, and what it hands the operator console.
//!
//! Two free functions carry the visibility rules — [`preset_name_visible`] and
//! [`output_modal`] — so both are assertable as values with no window and no
//! GPU, the same discipline [`crate::overlay`] and [`crate::settings`] keep. The
//! rest is composition: this frame's text, split by destination, and the
//! console's own present.
//!
//! The browse list's and the name plate's **colours, geometry and line-building
//! all live in [`crate::overlay`]**, beside the pure layout function that
//! reasons about them, so the pixels drawn here, the arithmetic tested there and
//! a headless `shot --ui` capture cannot drift apart.

use rlx_core::render::{ImageRect, OverlayImage};
use standalone::marks::Mark;

use crate::app_state::AppState;
use crate::console;
use crate::overlay::{self, next_rotation_line};
use crate::thumbs;

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
/// (whose header starts at [`LIST_TOP`](overlay::LIST_TOP) and crowds it from below) and the core's
/// F3 diagnostics panel, which composites *after* the text layer and so paints
/// straight over it. `enabled` is the operator's own switch (`[hud] preset_name`).
///
/// A free function, not a method, so the rule is assertable as a value with no
/// window and no GPU — the same discipline [`overlay`] and [`crate::settings`]
/// keep.
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

    /// The preview pane on the output, or `None` when the browser is on the
    /// console — which has no image layer — or the window is too small for one.
    fn output_pane(&self) -> Option<overlay::Pane> {
        if self.renderer.aux_size().is_some() {
            return None;
        }
        let size = self.window.inner_size();
        overlay::pane(size.width as f32, size.height as f32)
    }

    /// Draw the highlighted preset's cached still into the pane, or its
    /// placeholder, and its name under it.
    ///
    /// The cache is read only when the highlight lands on another preset; every
    /// other frame queues the rectangle of a picture the renderer already holds.
    fn queue_pane(&mut self, pane: overlay::Pane, name: &str, lines: &mut Vec<console::Line>) {
        // A renderer that moved adapters dropped the picture with the device.
        if self.hud.browse.pane_slot().shows(name) && self.renderer.overlay_image_size().is_none() {
            self.hud.browse.pane_slot().forget();
        }
        if self.hud.browse.pane_slot().needs_load(name) {
            let still = thumbs::cached_still(name);
            let shown = still.as_ref().is_some_and(|entry| {
                self.renderer
                    .set_overlay_image(Some(OverlayImage {
                        rgba: &entry.rgba,
                        width: entry.width,
                        height: entry.height,
                    }))
                    .is_ok()
            });
            self.hud.browse.pane_slot().loaded(name, shown);
        }

        let shown = self.hud.browse.pane_slot().shows(name);
        if shown {
            self.renderer.queue_image(ImageRect {
                x: pane.x,
                y: pane.y,
                w: pane.w,
                h: pane.h,
            });
        }
        let renderer = &mut self.renderer;
        overlay::pane_lines(
            &pane,
            name,
            shown,
            &mut |text, size| renderer.measure_text(text, size),
            lines,
        );
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
    ///
    /// `dt` real seconds advance [`standalone::motion`]'s envelopes, which only
    /// change how the built lines are drawn — never which lines are built.
    pub(crate) fn queue_frame_text(&mut self, dt: f32) {
        let motion = self.config.ui.motion;
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
            // Owned: the backdrop's measurement borrows the renderer mutably.
            let name = self.renderer.preset_name().to_owned();
            let marks = self.show.marks();
            let (favourite, hidden) = (
                marks.is(Mark::Favourite, &name),
                marks.is(Mark::Hidden, &name),
            );
            let renderer = &mut self.renderer;
            // The countdown under it, on the same visibility rule: it belongs
            // to the same corner and yields to the same things.
            overlay::corner_lines(
                &name,
                favourite,
                hidden,
                next_rotation_line(
                    self.show.director.remaining_secs(),
                    self.config.hud.next_rotation,
                ),
                &mut |text, size| renderer.measure_text(text, size),
                &mut chrome,
            );
        }
        // Before anything else joins `chrome`: the crossfade reads the plate as
        // everything from the start.
        self.hud.motion.corner.frame(&mut chrome, 0, dt, motion);

        // The capture verdict, under the core's diagnostics panel and only while
        // it is up (Plan 0083). Built from the stored token rather than from the
        // capture state, so this line and the log's `capture` column are the same
        // sentence about the same run.
        if self.diagnostics.overlay_on {
            chrome.push(overlay::capture_verdict_line(&self.capture.capture_token));
        }

        // Each modal's envelope runs every frame, open or not: a closed one
        // appends the lines it last drew while it fades out. Each is handed the
        // index its own block starts at, so the one fading out and the one
        // opening never animate each other's lines.
        let settings_from = modal.len();
        let settings_open = self.modal() == Some(Modal::Settings);
        if settings_open {
            let view = self.settings_view();
            let renderer = &mut self.renderer;
            overlay::settings_lines(
                &self.hud.settings,
                &view,
                &mut |text, size| renderer.measure_text(text, size),
                &mut modal,
            );
        }
        self.hud
            .motion
            .settings
            .frame(settings_open, &mut modal, settings_from, dt, motion);

        let browse_from = modal.len();
        let browse_open = self.modal() == Some(Modal::Browse);
        if browse_open {
            let names = self.roster_names();
            let rows = self.browse_rows(&names);
            let visible = self.hud.browse.visible(&rows);
            let highlight = self.hud.browse.highlight();

            // The header echoes the filter query (or a hint) above the list,
            // plus every narrowing that is on — a list that shrank and said
            // nothing reads as a roster that lost presets.
            let layout = self.list_layout(visible.len());
            let renderer = &mut self.renderer;
            overlay::browse_lines(
                &self.hud.browse,
                &visible,
                &layout,
                &mut |text, size| renderer.measure_text(text, size),
                &mut modal,
            );

            // The pane beside the list, for the highlighted row. An empty
            // list highlights nothing, and then the pane is simply absent.
            let highlighted = visible
                .get(highlight)
                .map(|(_, entry)| entry.name.to_owned());
            if let (Some(pane), Some(name)) = (self.output_pane(), highlighted) {
                self.queue_pane(pane, &name, &mut modal);
            }
        }
        self.hud
            .motion
            .browse
            .frame(browse_open, &mut modal, browse_from, dt, motion);

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
            let preset = self.renderer.preset_name().to_owned();
            let renderer = &mut self.renderer;
            let furniture = console::standing_lines(
                &preset,
                self.show.director.auto_enabled(),
                self.show.next_up(),
                (
                    self.config.rotate.min_dwell_secs,
                    self.config.rotate.max_dwell_secs,
                ),
                &mut |text, size| renderer.measure_text(text, size),
            );
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
        self.renderer
            .queue_panels(&self.hud.frame_text.output_panels());

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
    /// monitor, so a frame it drops costs the show no pixels and no state.
    ///
    /// **After the show's present is not outside the show's frame.** This is
    /// the same thread, so whatever this costs lands in the next frame's
    /// budget — being ordered last buys correctness, not freedom. What it
    /// actually costs was measured rather than asserted (Plan 0147 Phase 4):
    /// inside noise in three frame-time regimes on this box's integrated
    /// adapter, each arm witnessed by a non-zero present count. A failure here
    /// closes the console rather than killing the app.
    pub(crate) fn present_console(&mut self) {
        if self.hud.console_window.is_none() {
            return;
        }
        let runs = self.hud.frame_text.console_runs();
        let panels = self.hud.frame_text.console_panels();
        let result = self.renderer.present_aux(&runs, &panels);
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
    use crate::overlay::{browse_row_style, mark_suffix, next_rotation_line};
    use rlx_core::render::theme::THEME;

    /// **A favourite reads as a warm row, and the cursor still wins on it.** The
    /// one-character `*` is unchanged and does not survive a scan down a column
    /// of forty; the colour is what does.
    #[test]
    fn a_favourite_row_is_warm_and_the_highlight_outranks_it() {
        let (fav, row, hl) = (
            THEME.favourite.rgba(),
            THEME.text.rgba(),
            THEME.accent.rgba(),
        );
        assert_ne!(
            fav, row,
            "a favourite drawn in the plain row colour is the state this closes"
        );
        assert_ne!(fav, hl, "a favourite must not read as the cursor");

        assert_eq!(browse_row_style(false, true), ("  ", fav));
        assert_eq!(browse_row_style(false, false), ("  ", row));

        for favourite in [false, true] {
            assert_eq!(
                browse_row_style(true, favourite),
                ("> ", hl),
                "the highlighted row must be unambiguous whether or not it is \
                 marked (favourite={favourite})"
            );
        }
    }

    /// **The operator console draws the same colours.** True by construction —
    /// `console::route_into` moves the built lines rather than rebuilding
    /// them — and asserted rather than assumed, because "by construction" is a
    /// claim about a function that could be changed.
    #[test]
    fn the_console_receives_the_row_lines_unchanged() {
        use console::Console;

        let (marker, color) = browse_row_style(false, true);
        let rows = vec![console::Line::new(
            format!("{marker}aurora"),
            16.0,
            94.0,
            20.0,
            color,
        )];

        let routed = console::route(Vec::new(), rows.clone(), Console::Open);
        assert!(
            routed.output.is_empty(),
            "a browse row reached the show's own surface"
        );
        assert_eq!(
            routed.console, rows,
            "the console's copy of the row is not the line that was built, so \
             its colours can differ from the output's"
        );

        // And with no console open the same line lands on the output untouched.
        let routed = console::route(Vec::new(), rows.clone(), Console::Closed);
        assert_eq!(routed.output, rows);
    }

    /// **The HUD says when the next rotation lands, and says nothing when
    /// auto-rotate is off.** The console already names *what* comes next and the
    /// show window named neither, which is the asymmetry this closes.
    #[test]
    fn the_countdown_appears_only_while_something_is_counting_down() {
        assert_eq!(
            next_rotation_line(Some(42.0), true).as_deref(),
            Some("next in 42 s")
        );
        assert_eq!(
            next_rotation_line(None, true),
            None,
            "auto-rotate is off, so there is no rotation to count down to"
        );
        assert_eq!(
            next_rotation_line(Some(42.0), false),
            None,
            "the operator's switch must win over a running timer"
        );

        // Rounded **up**, so the line reaches `1 s` and the rotation happens —
        // a `0 s` held for most of a second reads as a stalled timer.
        assert_eq!(
            next_rotation_line(Some(0.2), true).as_deref(),
            Some("next in 1 s")
        );
        assert_eq!(
            next_rotation_line(Some(0.0), true).as_deref(),
            Some("next in 0 s"),
            "a cap already reached is the one moment zero is the honest answer"
        );
    }

    /// **The corner says whether the preset on screen is marked**, in all four
    /// combinations — marking is worthless if you cannot see what is marked
    /// without opening the browser.
    #[test]
    fn the_corner_name_reports_both_marks_and_neither() {
        assert_eq!(mark_suffix(false, false), "", "an unmarked preset is bare");
        assert!(mark_suffix(true, false).contains("favourite"));
        assert!(mark_suffix(false, true).contains("hidden"));
        let both = mark_suffix(true, true);
        assert!(both.contains("favourite") && both.contains("hidden"));
    }

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
