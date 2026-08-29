//! The operator console's pure half: where a frame's text goes, and which
//! window a key came from.
//!
//! Window-free and renderer-free by construction, exactly like [`crate::overlay`]
//! and [`crate::settings`] — everything here is a function of state, so the two
//! properties that matter can be asserted without a GPU or a display:
//!
//! - **Modal text destined for the console never reaches the output.** That is
//!   the guarantee the whole feature rests on: an operator opening the preset
//!   browser mid-show must not paint a list of preset names across the screen
//!   the audience is looking at.
//! - **Both surfaces are fed from one state machine.** The console does not keep
//!   a parallel copy of the modal rows; it is handed the same lines the output
//!   path would have drawn, and the routing decides which surface receives them.
//!
//! The console owns no state of its own yet — it is a second destination, not a
//! second model.

use lmv_core::render::TextRun;

/// One positioned line of text, owned so the routing can move it between
/// destinations without borrowing the roster it was built from.
///
/// The shape mirrors [`TextRun`] one field at a time; it exists because a
/// `TextRun` borrows its `&str`, and routing decides a line's destination after
/// the strings that back it are built.
#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub size: f32,
    pub color: [f32; 4],
}

impl Line {
    /// Build a line from its parts.
    pub fn new(text: String, x: f32, y: f32, size: f32, color: [f32; 4]) -> Self {
        Self {
            text,
            x,
            y,
            size,
            color,
        }
    }

    /// Borrow this line as a [`TextRun`] for the frame it is drawn in.
    pub fn as_run(&self) -> TextRun<'_> {
        TextRun {
            text: self.text.as_str(),
            x: self.x,
            y: self.y,
            size: self.size,
            color: self.color,
        }
    }
}

/// A frame's text, split by the surface it is destined for.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FrameText {
    /// Lines for the show's own surface.
    pub output: Vec<Line>,
    /// Lines for the operator console. Always empty while the console is closed.
    pub console: Vec<Line>,
}

impl FrameText {
    /// The output's lines as borrowed runs, ready for `queue_text`.
    pub fn output_runs(&self) -> Vec<TextRun<'_>> {
        self.output.iter().map(Line::as_run).collect()
    }

    /// The console's lines as borrowed runs, ready for `present_aux`.
    pub fn console_runs(&self) -> Vec<TextRun<'_>> {
        self.console.iter().map(Line::as_run).collect()
    }
}

/// Whether the operator console is currently open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Console {
    Open,
    Closed,
}

impl Console {
    pub fn is_open(self) -> bool {
        matches!(self, Self::Open)
    }
}

/// Split a frame's text between the two surfaces, draining `chrome` and `modal`
/// into `dst`'s retained vectors.
///
/// `chrome` is the show's own furniture — the corner preset name, the capture
/// verdict line — and always stays on the output: it is part of the picture, not
/// part of driving it. `modal` is the browse list or the settings menu, and it
/// follows the operator: to the console when one is open, to the output when
/// there is nowhere else to put it.
///
/// **The modal lines are moved, never copied.** A routing that cloned them into
/// both destinations would satisfy every test about the console's contents while
/// still painting the list across the show, so every line is `append`ed out of
/// its source and lands in exactly one of the two vectors.
///
/// Drains rather than allocating so the frame path reuses the same buffers every
/// frame; [`route`] is the owned-value wrapper the tests read.
pub fn route_into(
    dst: &mut FrameText,
    chrome: &mut Vec<Line>,
    modal: &mut Vec<Line>,
    console: Console,
) {
    dst.output.clear();
    dst.console.clear();
    dst.output.append(chrome);
    match console {
        Console::Open => dst.console.append(modal),
        Console::Closed => dst.output.append(modal),
    }
}

/// [`route_into`] over owned inputs, returning a fresh [`FrameText`].
///
/// The same implementation the frame path runs — it delegates rather than
/// restating the rule, so a test can never pass against a second copy of the
/// routing that the show does not use. Test-only because the frame path wants
/// the draining form; the logic under assertion is `route_into` either way.
#[cfg(test)]
pub fn route(mut chrome: Vec<Line>, mut modal: Vec<Line>, console: Console) -> FrameText {
    let mut dst = FrameText::default();
    route_into(&mut dst, &mut chrome, &mut modal, console);
    dst
}

/// The window height [`crate::overlay`]'s list constants were authored against —
/// the windowed 1080p default the app opens at, and the size the NFR 1
/// performance floor is quoted for.
///
/// It is a **reference, not a requirement**: the constants are device pixels, so
/// drawing them unscaled into a console a third the height puts three quarters
/// of the roster off the bottom edge and every column but the first off the
/// right. [`scale`] is what maps them onto whatever window the console got.
const REFERENCE_HEIGHT: f32 = 1080.0;

/// Smallest scale worth drawing: below this the rows stop being readable across
/// a desk, and an operator who cannot read the list is worse off than one
/// scrolling a larger one.
const MIN_SCALE: f32 = 0.45;

/// How much to shrink the list geometry for a console of `height` device pixels.
///
/// Capped at `1.0`: a console larger than the reference gets the same type as
/// the show, never magnified. A uniform factor rather than separate x and y
/// terms, because it scales the **font size** as well as the positions, and type
/// stretched on one axis is unreadable in a way a smaller grid is not.
pub fn scale(height: f32) -> f32 {
    if !height.is_finite() || height <= 0.0 {
        return 1.0;
    }
    (height / REFERENCE_HEIGHT).clamp(MIN_SCALE, 1.0)
}

/// The size to lay a list out against, so that `scale`-ing the result lands it
/// inside a console of `width` x `height`.
///
/// The list geometry reasons in device pixels at the reference size, so the way
/// to get more columns and more rows into a small window is to lay out against a
/// **larger logical** window and shrink what comes back — not to re-derive the
/// constants per window, which would put a second copy of the layout rule in the
/// shell.
pub fn logical_size(width: f32, height: f32) -> (f32, f32) {
    let s = scale(height);
    (width / s, height / s)
}

/// Shrink every line's position and font size by `s`, in place.
///
/// Applied to the lines the console receives and nothing else: the show's own
/// text is already sized for the show.
pub fn scale_lines(lines: &mut [Line], s: f32) {
    for line in lines {
        line.x *= s;
        line.y *= s;
        line.size *= s;
    }
}

/// The console's own standing header, drawn above whatever the routing sends it.
///
/// Present even with no modal open, so an operator can tell a console that is
/// alive and idle from one whose window is up but whose app has stopped
/// presenting to it.
pub fn header(preset: &str) -> Line {
    Line::new(
        format!("console  -  {preset}"),
        crate::overlay::LIST_INSET,
        crate::overlay::LIST_INSET,
        crate::overlay::ROW_SIZE,
        HEADER_COLOR,
    )
}

/// The console header's colour — dimmer than a modal row, so it reads as a
/// label rather than as content.
const HEADER_COLOR: [f32; 4] = [0.55, 0.62, 0.74, 0.9];

/// Which window an event arrived from, once the raw `WindowId` has been
/// resolved against the two the app owns.
///
/// A third id is possible in principle — a window closing while its events are
/// still in flight — and resolves to [`Target::Unknown`], which every caller
/// drops. That is why this is an enum rather than a bool.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    /// The show's window.
    Output,
    /// The operator console.
    Console,
    /// Neither — a stale event from a window already gone.
    Unknown,
}

/// Resolve an event's window against the app's two windows.
///
/// Pure and generic over the id type so it can be asserted without winit: the
/// property under test is that two different ids reach two different targets,
/// and that a console id resolves to the console *only while one is open*.
pub fn dispatch<Id: PartialEq>(id: &Id, output: &Id, console: Option<&Id>) -> Target {
    if id == output {
        return Target::Output;
    }
    match console {
        Some(console) if id == console => Target::Console,
        _ => Target::Unknown,
    }
}

#[cfg(test)]
mod tests;
