//! How the overlays move: envelopes that are **a view of state, never state**.
//!
//! Every modal keeps its own state machine ([`crate::overlay`],
//! [`crate::settings`]) and those change on the frame a key is pressed. What
//! this module animates is only how the lines those machines produce are
//! drawn: a modal fades and slides in and out, the selection highlight glides
//! between rows, and the corner name crossfades when the preset changes. A key
//! pressed mid-animation acts on the target state at once, because nothing here
//! is consulted to decide what a key does.
//!
//! Durations and the curve come from the theme (ADR-0252); `[ui] motion =
//! "reduced"` makes every envelope a step. Pure — `dt` is handed in, no clock is
//! read — so every property is a unit test.

use rlx_core::render::PanelKind;
use rlx_core::render::theme::THEME;

use crate::config::Motion;
use crate::console::Line;

/// How far a modal slides as it opens, device px: it arrives from this far
/// above its resting place.
pub const SLIDE_PX: f32 = 12.0;

/// Seconds the launch hint stays up, from launch or from the pointer's last
/// move over the window.
pub const HINT_SECS: f32 = 4.0;

/// The launch hint's opacity with `remaining` of its [`HINT_SECS`] left: it
/// eases in at the start, holds, and eases out over its last short envelope.
/// Under [`Motion::Reduced`] it is on for its whole life and off after.
pub fn hint_alpha(remaining: f32, motion: Motion) -> f32 {
    if !remaining.is_finite() || remaining <= 0.0 {
        return 0.0;
    }
    let d = short_secs();
    let rise = progress(HINT_SECS - remaining, d, motion);
    let fall = progress(remaining, d, motion);
    rise.min(fall)
}

/// Seconds every panel's open and close takes.
pub fn short_secs() -> f32 {
    THEME.motion_short_ms as f32 / 1000.0
}

/// Seconds a change that replaces what is shown takes — the corner name's
/// crossfade.
pub fn long_secs() -> f32 {
    THEME.motion_long_ms as f32 / 1000.0
}

/// Progress through an envelope `elapsed` seconds into one lasting `duration`:
/// `0.0` at the start, exactly `1.0` from `duration` on, the theme's curve in
/// between. Under [`Motion::Reduced`] it is `1.0` throughout — a step.
pub fn progress(elapsed: f32, duration: f32, motion: Motion) -> f32 {
    if motion == Motion::Reduced || !duration.is_finite() || duration <= 0.0 {
        return 1.0;
    }
    THEME.ease.at(elapsed / duration)
}

/// One animated value moving from where it was toward a target.
///
/// Retargeting mid-flight starts the new envelope from the value on screen, so
/// a reversed modal turns around where it is rather than jumping.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tween {
    from: f32,
    to: f32,
    elapsed: f32,
    duration: f32,
}

impl Tween {
    /// A tween already at rest at `value`.
    pub fn settled(value: f32, duration: f32) -> Self {
        Self {
            from: value,
            to: value,
            elapsed: duration,
            duration,
        }
    }

    /// The value it is heading for.
    pub fn target(&self) -> f32 {
        self.to
    }

    /// The value to draw now.
    pub fn value(&self, motion: Motion) -> f32 {
        let p = progress(self.elapsed, self.duration, motion);
        if p >= 1.0 {
            return self.to;
        }
        self.from + (self.to - self.from) * p
    }

    /// Head for `to`, starting from the value on screen now. A no-op when `to`
    /// is already the target.
    pub fn retarget(&mut self, to: f32, motion: Motion) {
        if to == self.to {
            return;
        }
        self.from = self.value(motion);
        self.to = to;
        self.elapsed = 0.0;
    }

    /// Advance by `dt` real seconds.
    pub fn advance(&mut self, dt: f32) {
        if dt.is_finite() && dt > 0.0 {
            self.elapsed = (self.elapsed + dt).min(self.duration.max(0.0));
        }
    }
}

/// Multiply every line's opacity by `k` — text and backdrops alike, so a panel
/// fades with what it holds.
pub fn fade(lines: &mut [Line], k: f32) {
    for line in lines {
        line.color[3] *= k;
    }
}

/// Move every line down by `dy` device px (up, when negative).
pub fn shift(lines: &mut [Line], dy: f32) {
    for line in lines {
        line.y += dy;
    }
}

/// One modal's open/close envelope, and its selection highlight's glide.
///
/// Holds a copy of the lines the modal last drew, because a closed modal's
/// state machine builds nothing: the close fades what was on screen.
#[derive(Clone, Debug)]
pub struct ModalMotion {
    open: Tween,
    last: Vec<Line>,
    glide: Option<(Tween, Tween)>,
}

impl Default for ModalMotion {
    fn default() -> Self {
        Self {
            open: Tween::settled(0.0, short_secs()),
            last: Vec::new(),
            glide: None,
        }
    }
}

impl ModalMotion {
    /// How open the modal is drawn, `0.0..=1.0`.
    pub fn openness(&self, motion: Motion) -> f32 {
        self.open.value(motion)
    }

    /// Animate this frame's lines for the modal.
    ///
    /// While `open`, `lines[from..]` are the modal's lines as its state machine
    /// built them this frame: the highlight glides toward where they put it, and
    /// the block fades and slides by the open envelope. While closed, nothing
    /// was built, and the lines last drawn are appended, fading out.
    pub fn frame(
        &mut self,
        open: bool,
        lines: &mut Vec<Line>,
        from: usize,
        dt: f32,
        motion: Motion,
    ) {
        self.open.retarget(if open { 1.0 } else { 0.0 }, motion);
        self.open.advance(dt);
        let v = self.open.value(motion);
        if open {
            let block = lines.get_mut(from..).unwrap_or_default();
            self.glide_highlight(block, dt, motion);
            self.last.clear();
            self.last.extend_from_slice(block);
            fade(block, v);
            shift(block, -(1.0 - v) * SLIDE_PX);
        } else {
            self.glide = None;
            if v > 0.0 {
                let start = lines.len();
                lines.extend_from_slice(&self.last);
                let block = lines.get_mut(start..).unwrap_or_default();
                fade(block, v);
                shift(block, -(1.0 - v) * SLIDE_PX);
            } else {
                self.last.clear();
            }
        }
    }

    /// Move the block's highlight from where the glide has it toward where the
    /// state machine put it. The first frame a modal shows a highlight, it
    /// starts where it is — there is nowhere to glide from.
    fn glide_highlight(&mut self, block: &mut [Line], dt: f32, motion: Motion) {
        let Some(hl) = block
            .iter_mut()
            .find(|l| l.backdrop.is_some_and(|b| b.kind == PanelKind::Highlight))
        else {
            return;
        };
        let (gx, gy) = self.glide.get_or_insert_with(|| {
            (
                Tween::settled(hl.x, short_secs()),
                Tween::settled(hl.y, short_secs()),
            )
        });
        gx.retarget(hl.x, motion);
        gy.retarget(hl.y, motion);
        gx.advance(dt);
        gy.advance(dt);
        hl.x = gx.value(motion);
        hl.y = gy.value(motion);
    }
}

/// The corner name plate's crossfade: when the name changes, the old plate
/// fades out over the new one fading in.
#[derive(Clone, Debug)]
pub struct Crossfade {
    /// The name line's text last frame, which is what a change is read from.
    key: String,
    last: Vec<Line>,
    prev: Vec<Line>,
    t: Tween,
}

impl Default for Crossfade {
    fn default() -> Self {
        Self {
            key: String::new(),
            last: Vec::new(),
            prev: Vec::new(),
            t: Tween::settled(1.0, long_secs()),
        }
    }
}

impl Crossfade {
    /// Animate this frame's plate, `lines[from..]`, whose first line is the
    /// name. No lines means the plate is hidden this frame, and it comes back
    /// without a crossfade from whatever it last showed.
    pub fn frame(&mut self, lines: &mut Vec<Line>, from: usize, dt: f32, motion: Motion) {
        let Some(name) = lines.get(from).map(|l| l.text.clone()) else {
            self.key.clear();
            self.last.clear();
            self.prev.clear();
            return;
        };
        if name != self.key {
            if !self.key.is_empty() {
                std::mem::swap(&mut self.prev, &mut self.last);
                self.t = Tween::settled(0.0, long_secs());
                self.t.retarget(1.0, motion);
            }
            self.key = name;
        }
        self.t.advance(dt);
        let v = self.t.value(motion);
        let block = lines.get_mut(from..).unwrap_or_default();
        self.last.clear();
        self.last.extend_from_slice(block);
        fade(block, v);
        if v < 1.0 && !self.prev.is_empty() {
            let start = lines.len();
            lines.extend_from_slice(&self.prev);
            fade(lines.get_mut(start..).unwrap_or_default(), 1.0 - v);
        } else {
            self.prev.clear();
        }
    }
}

/// Everything the shell animates, in one place on its state.
#[derive(Clone, Debug, Default)]
pub struct OverlayMotion {
    pub browse: ModalMotion,
    pub settings: ModalMotion,
    pub help: ModalMotion,
    pub corner: Crossfade,
}

#[cfg(test)]
mod tests;
