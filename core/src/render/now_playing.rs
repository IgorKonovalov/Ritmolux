//! The now-playing banner: a core-owned, transient announcement of the current
//! track (ADR-0110, Plan 0097).
//!
//! A shell pushes in a UTF-8 string and nothing else — SMTC on the standalone,
//! foobar's `titleformat` through the C ABI — and everything downstream of that
//! string is decided here: the fade envelope, the artist/title split, the
//! placement, and the truncation rule. That is the whole point of the ADR: two
//! frontends whose metadata sources have nothing in common cannot drift on what
//! a track change looks like, because neither of them draws it.
//!
//! The envelope is a **pure function of accumulated `dt`** (Plan 0014's injected
//! real seconds), so it runs for the same wall-clock duration on a 60 Hz and a
//! 165 Hz display and is testable without a GPU. Nothing here reads a clock.
//!
//! This module is **not** behind the `text` feature. A build without it keeps
//! the state and simply never asks for a [`layout`](NowPlaying::layout), which
//! is what lets the plugin build turn the feature on without touching any of
//! this.

// Hot-path panic-denial pragma (Plan 0002 Phase 2; `render/` scan set). The
// layout runs every frame a banner is up; a panic here is a visible crash.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

use std::borrow::Cow;

/// Seconds the banner takes to reach full opacity.
pub const FADE_IN_SECS: f32 = 0.5;
/// Seconds the banner holds at full opacity.
pub const HOLD_SECS: f32 = 4.0;
/// Seconds the banner takes to fade back out.
pub const FADE_OUT_SECS: f32 = 1.0;
/// Total lifetime of one announcement, after which nothing is drawn.
pub const TOTAL_SECS: f32 = FADE_IN_SECS + HOLD_SECS + FADE_OUT_SECS;

/// The separator a shell puts between artist and title. Both sources agree on
/// it: the plugin renders `%artist% - %title%` (ADR-0110) and the standalone
/// joins its SMTC fields the same way, so this one rule splits both.
pub const SEPARATOR: &str = " - ";

/// Left inset in device pixels — the same 16 px the shell's corner furniture
/// uses, so the banner lines up with the preset name's column.
const INSET_X: f32 = 16.0;
/// Gap between the title's descender box and the bottom of the surface.
const INSET_BOTTOM: f32 = 24.0;
/// Font size of the artist line (the quieter of the two).
const ARTIST_SIZE: f32 = 24.0;
/// Font size of the title line.
const TITLE_SIZE: f32 = 32.0;
/// Must match `text::LINE_HEIGHT_RATIO` — the vertical extent glyphon gives a
/// run, which is what stacks the two lines without them overlapping.
const LINE_HEIGHT_RATIO: f32 = 1.25;
/// Dimmer than the title: an attribution, not the announcement itself.
const ARTIST_COLOR: [f32; 3] = [0.72, 0.80, 0.92];
/// Near-white, matching the preset name's weight in the corner.
const TITLE_COLOR: [f32; 3] = [0.95, 0.97, 1.0];

/// Mean glyph advance as a fraction of the font size, used only to pick a
/// character budget for truncation. A sans-serif estimate, deliberately not a
/// shaped measurement: the core must decide the budget in a build that has no
/// `text` feature and therefore no font system at all. Erring wide would let a
/// title run under the clip bound and vanish mid-word, so this rounds toward
/// truncating slightly early.
const AVG_ADVANCE_RATIO: f32 = 0.5;

/// Never truncate below this many characters, however narrow the surface.
const MIN_CHARS: usize = 8;

/// One positioned line of the banner, ready to become a `TextRun`. The `Cow`
/// borrows the stored string whenever the line fits and owns only the truncated
/// copy, so a steady banner frame allocates nothing.
pub struct BannerLine<'a> {
    /// The line's text, already truncated to the surface width.
    pub text: Cow<'a, str>,
    /// Left edge, device pixels from the surface's top-left.
    pub x: f32,
    /// Top edge, device pixels from the surface's top-left.
    pub y: f32,
    /// Font size in device pixels.
    pub size: f32,
    /// Linear RGBA in `0.0..=1.0`, with the envelope already applied to alpha.
    pub color: [f32; 4],
}

/// The banner's whole state: what to announce, and how long ago it was set.
#[derive(Default)]
pub struct NowPlaying {
    /// The string a shell pushed in. Empty means there is nothing to draw.
    text: String,
    /// Seconds since [`set`](Self::set) accepted a new string, accumulated from
    /// injected `dt` and clamped at [`TOTAL_SECS`] so it cannot grow unbounded
    /// across a long session.
    elapsed: f32,
}

impl NowPlaying {
    /// Announce `text`, restarting the envelope.
    ///
    /// Setting the string that is **already** set is a no-op, so a source that
    /// re-reports the current track — SMTC fires `MediaPropertiesChanged` for
    /// artwork and position updates too — cannot re-trigger the banner. An empty
    /// or whitespace-only string clears it immediately.
    pub fn set(&mut self, text: &str) {
        let text = text.trim();
        if text == self.text {
            return;
        }
        self.text.clear();
        self.text.push_str(text);
        // A cleared banner is finished, not starting: jumping `elapsed` to the
        // end means `alpha` is zero on the same frame rather than one frame of
        // full opacity before the empty string is noticed.
        self.elapsed = if text.is_empty() { TOTAL_SECS } else { 0.0 };
    }

    /// Advance the envelope by `dt` real seconds. Non-finite or non-positive
    /// steps are ignored, matching the parameter smoother's rule.
    pub fn advance(&mut self, dt: f32) {
        if !dt.is_finite() || dt <= 0.0 || self.elapsed >= TOTAL_SECS {
            return;
        }
        self.elapsed = (self.elapsed + dt).min(TOTAL_SECS);
    }

    /// The current opacity in `0.0..=1.0`; zero when there is nothing to draw.
    pub fn alpha(&self) -> f32 {
        if self.text.is_empty() {
            return 0.0;
        }
        alpha_at(self.elapsed)
    }

    /// The string currently announced (`""` when the banner is unset).
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The two positioned lines to draw on a `width`×`height` surface, or
    /// `[None, None]` while the banner is invisible.
    ///
    /// The string splits on the **first** [`SEPARATOR`] into artist and title; a
    /// string with no separator draws as a single title line. Both are truncated
    /// to a character budget derived from the surface width, so a long title
    /// ends in `...` rather than running off the edge.
    pub fn layout(&self, width: f32, height: f32) -> [Option<BannerLine<'_>>; 2] {
        let alpha = self.alpha();
        if alpha <= 0.0 {
            return [None, None];
        }

        let (artist, title) = split(&self.text);

        // Bottom-up: the title sits one line off the floor, the artist directly
        // above it. Placing from the bottom keeps the banner clear of the
        // top-left furniture (the preset name and the F3 panel both live there).
        let title_y = height - INSET_BOTTOM - TITLE_SIZE * LINE_HEIGHT_RATIO;
        let artist_y = title_y - ARTIST_SIZE * LINE_HEIGHT_RATIO;

        let title_line = Some(BannerLine {
            text: fit(title, budget(width, TITLE_SIZE)),
            x: INSET_X,
            y: title_y,
            size: TITLE_SIZE,
            color: rgba(TITLE_COLOR, alpha),
        });
        let artist_line = artist.map(|artist| BannerLine {
            text: fit(artist, budget(width, ARTIST_SIZE)),
            x: INSET_X,
            y: artist_y,
            size: ARTIST_SIZE,
            color: rgba(ARTIST_COLOR, alpha),
        });

        [artist_line, title_line]
    }
}

/// The envelope: ramp in, hold, ramp out, then nothing. A pure function of
/// elapsed seconds, which is what makes it identical at any refresh rate and
/// testable without a device.
pub fn alpha_at(elapsed: f32) -> f32 {
    if !elapsed.is_finite() || elapsed <= 0.0 {
        return 0.0;
    }
    if elapsed < FADE_IN_SECS {
        elapsed / FADE_IN_SECS
    } else if elapsed < FADE_IN_SECS + HOLD_SECS {
        1.0
    } else if elapsed < TOTAL_SECS {
        (TOTAL_SECS - elapsed) / FADE_OUT_SECS
    } else {
        0.0
    }
}

/// Split a pushed string into `(artist, title)` on the first [`SEPARATOR`].
/// Without one — or with an empty half — the whole string is the title, because
/// a lone name reads better large than it does as an attribution to nothing.
fn split(text: &str) -> (Option<&str>, &str) {
    match text.split_once(SEPARATOR) {
        Some((artist, title)) if !artist.is_empty() && !title.is_empty() => (Some(artist), title),
        _ => (None, text),
    }
}

/// How many characters of a `size`-pixel line fit across a `width`-pixel
/// surface, inset on both sides.
fn budget(width: f32, size: f32) -> usize {
    let usable = width - 2.0 * INSET_X;
    if !usable.is_finite() || usable <= 0.0 {
        return MIN_CHARS;
    }
    ((usable / (size * AVG_ADVANCE_RATIO)) as usize).max(MIN_CHARS)
}

/// Truncate `s` to `max_chars`, ending in `...` when it had to cut. Character-
/// counted rather than byte-counted, so a CJK or accented title cannot be sliced
/// mid-codepoint. Follows the browse overlay's ASCII ellipsis rather than `…`.
fn fit(s: &str, max_chars: usize) -> Cow<'_, str> {
    if s.chars().count() <= max_chars {
        return Cow::Borrowed(s);
    }
    let keep = max_chars.saturating_sub(3);
    let mut out: String = s.chars().take(keep).collect();
    out.push_str("...");
    Cow::Owned(out)
}

/// Apply the envelope to a base colour.
fn rgba([r, g, b]: [f32; 3], alpha: f32) -> [f32; 4] {
    [r, g, b, alpha]
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    /// Step the envelope at `dt` until it returns to zero, and report how many
    /// real seconds that took. The property Plan 0014 bought, measured: this
    /// number must not depend on `dt`.
    fn visible_duration(dt: f32) -> f32 {
        let mut np = NowPlaying::default();
        np.set("Artist - Title");
        let mut steps = 0u32;
        // Stepped before the test, not after: alpha is legitimately zero at
        // `elapsed = 0` (the banner starts transparent), so a leading check
        // would exit before the first frame. Bounded well past the envelope so a
        // regression fails the assertion rather than hanging the suite.
        loop {
            np.advance(dt);
            steps += 1;
            if np.alpha() <= 0.0 || steps >= 100_000 {
                break;
            }
        }
        steps as f32 * dt
    }

    #[test]
    fn the_envelope_rises_plateaus_and_returns_to_zero() {
        let mut np = NowPlaying::default();
        np.set("Artist - Title");

        // Starts dark, before any dt has been injected.
        assert_eq!(np.alpha(), 0.0, "the banner must start transparent");

        // Rises monotonically through the fade-in.
        let mut prev = np.alpha();
        for _ in 0..30 {
            np.advance(FADE_IN_SECS / 30.0);
            let a = np.alpha();
            assert!(a >= prev, "alpha must not fall during the fade-in");
            prev = a;
        }
        assert!(
            (np.alpha() - 1.0).abs() < 1e-3,
            "alpha must reach full at the end of the fade-in, got {}",
            np.alpha()
        );

        // Plateaus for the whole hold.
        for _ in 0..40 {
            np.advance(HOLD_SECS / 40.0);
            assert_eq!(np.alpha(), 1.0, "alpha must stay full through the hold");
        }

        // Falls monotonically through the fade-out.
        let mut prev = np.alpha();
        for _ in 0..20 {
            np.advance(FADE_OUT_SECS / 20.0);
            let a = np.alpha();
            assert!(a <= prev, "alpha must not rise during the fade-out");
            prev = a;
        }
        assert_eq!(np.alpha(), 0.0, "alpha must return to zero");

        // And stays there — a finished banner does not come back.
        np.advance(10.0);
        assert_eq!(np.alpha(), 0.0, "a finished banner must stay finished");
    }

    /// The frame-rate independence Plan 0014 bought, stated as a property: the
    /// banner is on screen for the same number of *seconds* at 60 Hz and at
    /// 165 Hz, not for the same number of frames.
    #[test]
    fn the_envelope_lasts_the_same_time_at_60_and_165_hz() {
        let at_60 = visible_duration(1.0 / 60.0);
        let at_165 = visible_duration(1.0 / 165.0);

        // Each is within one of its own steps of the nominal lifetime, and the
        // two agree with each other within the coarser step.
        assert!(
            (at_60 - TOTAL_SECS).abs() <= 1.0 / 60.0,
            "60 Hz ran for {at_60} s, expected {TOTAL_SECS} s"
        );
        assert!(
            (at_165 - TOTAL_SECS).abs() <= 1.0 / 165.0,
            "165 Hz ran for {at_165} s, expected {TOTAL_SECS} s"
        );
        assert!(
            (at_60 - at_165).abs() <= 1.0 / 60.0,
            "the two refresh rates disagreed: {at_60} s vs {at_165} s"
        );
    }

    #[test]
    fn setting_the_same_string_does_not_restart_the_envelope() {
        let mut np = NowPlaying::default();
        np.set("Artist - Title");
        np.advance(FADE_IN_SECS + HOLD_SECS + FADE_OUT_SECS / 2.0);
        let mid_fade = np.alpha();
        assert!(
            mid_fade > 0.0 && mid_fade < 1.0,
            "expected a mid-fade alpha"
        );

        // The same track re-reported: SMTC fires for artwork and position too.
        np.set("Artist - Title");
        assert_eq!(
            np.alpha(),
            mid_fade,
            "re-reporting the current track must not re-trigger the banner"
        );

        // Whitespace differences are not a new track either.
        np.set("  Artist - Title  ");
        assert_eq!(
            np.alpha(),
            mid_fade,
            "trimming must happen before the compare"
        );
    }

    #[test]
    fn setting_a_new_string_restarts_the_envelope() {
        let mut np = NowPlaying::default();
        np.set("Artist - First");
        np.advance(FADE_IN_SECS + HOLD_SECS);
        assert_eq!(np.alpha(), 1.0);

        np.set("Artist - Second");
        assert_eq!(np.alpha(), 0.0, "a new track restarts from transparent");
        np.advance(FADE_IN_SECS);
        assert!((np.alpha() - 1.0).abs() < 1e-3);
        assert_eq!(np.text(), "Artist - Second");
    }

    #[test]
    fn an_empty_string_clears_the_banner_immediately() {
        let mut np = NowPlaying::default();
        np.set("Artist - Title");
        np.advance(FADE_IN_SECS);
        assert_eq!(np.alpha(), 1.0);

        np.set("");
        assert_eq!(np.alpha(), 0.0, "clearing must not leave one lit frame");
        let [artist, title] = np.layout(1920.0, 1080.0);
        assert!(artist.is_none() && title.is_none());
    }

    #[test]
    fn the_first_separator_splits_artist_from_title() {
        assert_eq!(
            split("Boards of Canada - Roygbiv"),
            (Some("Boards of Canada"), "Roygbiv")
        );
        // Only the first one splits — a title may contain the separator itself.
        assert_eq!(
            split("Godspeed - Storm - Levez Vos Skinny Fists"),
            (Some("Godspeed"), "Storm - Levez Vos Skinny Fists")
        );
        // No separator, or an empty half, is a lone title.
        assert_eq!(split("Untitled"), (None, "Untitled"));
        assert_eq!(split(" - Roygbiv"), (None, " - Roygbiv"));
    }

    #[test]
    fn a_long_title_truncates_rather_than_running_off_the_surface() {
        let long = "A".repeat(400);
        let np = {
            let mut np = NowPlaying::default();
            np.set(&format!("Artist - {long}"));
            np.advance(FADE_IN_SECS);
            np
        };

        let width = 1280.0;
        let [_, title] = np.layout(width, 800.0);
        let title = title.expect("a visible banner must produce a title line");
        let chars = title.text.chars().count();

        assert!(chars < 400, "the title must be cut, kept {chars} chars");
        assert!(
            title.text.ends_with("..."),
            "a cut title must say so: {}",
            title.text
        );
        // The kept run fits inside the insets at its own font size.
        let drawn = chars as f32 * title.size * AVG_ADVANCE_RATIO;
        assert!(
            drawn <= width - 2.0 * INSET_X,
            "{drawn} px of text does not fit {width} px"
        );
    }

    #[test]
    fn a_short_line_is_borrowed_rather_than_copied() {
        let mut np = NowPlaying::default();
        np.set("Air - La Femme d'Argent");
        np.advance(FADE_IN_SECS);
        let [artist, title] = np.layout(1920.0, 1080.0);
        assert!(matches!(artist.unwrap().text, Cow::Borrowed(_)));
        assert!(matches!(title.unwrap().text, Cow::Borrowed(_)));
    }

    #[test]
    fn the_banner_sits_in_the_lower_left_and_stacks_upward() {
        let mut np = NowPlaying::default();
        np.set("Artist - Title");
        np.advance(FADE_IN_SECS);

        let (w, h) = (1920.0, 1080.0);
        let [artist, title] = np.layout(w, h);
        let (artist, title) = (artist.unwrap(), title.unwrap());

        assert_eq!(artist.x, INSET_X);
        assert_eq!(title.x, INSET_X);
        assert!(artist.y < title.y, "the artist line sits above the title");
        assert!(
            title.y + title.size * LINE_HEIGHT_RATIO <= h,
            "the title must not hang off the bottom"
        );
        // Clear of the top-left furniture the shell owns (the preset name at
        // y = 16 and the F3 panel below it).
        assert!(artist.y > h * 0.5, "the banner belongs in the lower half");
    }

    #[test]
    fn a_lone_name_draws_as_one_title_line() {
        let mut np = NowPlaying::default();
        np.set("Untitled Broadcast");
        np.advance(FADE_IN_SECS);
        let [artist, title] = np.layout(1920.0, 1080.0);
        assert!(artist.is_none(), "no separator means no artist line");
        assert_eq!(title.unwrap().text, "Untitled Broadcast");
    }

    #[test]
    fn a_non_finite_or_backwards_step_is_ignored() {
        let mut np = NowPlaying::default();
        np.set("Artist - Title");
        np.advance(FADE_IN_SECS);
        let full = np.alpha();
        np.advance(f32::NAN);
        np.advance(-1.0);
        np.advance(0.0);
        assert_eq!(np.alpha(), full, "a bad dt must not move the envelope");
    }
}
