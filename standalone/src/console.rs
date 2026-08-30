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

// ---------------------------------------------------------------------------
// The transport strip
// ---------------------------------------------------------------------------

/// A rectangle in the console surface's device pixels, origin top-left.
///
/// The console's own geometry type rather than the core's: this describes where
/// an operator may click, which is a shell question, and nothing here should
/// couple the button layout to a render target's.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    /// Whether `(x, y)` is inside, treating the left and top edges as in and
    /// the right and bottom as out.
    ///
    /// Half-open on purpose: adjacent buttons share an edge coordinate, and a
    /// closed test would make that column belong to both — the last one checked
    /// silently winning.
    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && x < self.x + self.width && y >= self.y && y < self.y + self.height
    }
}

/// One clickable control on the console's transport strip.
///
/// Text with a hit rectangle around it, not a widget: the console draws through
/// the same glyph seam as every other surface here, and a toolkit is out of
/// scope (NFR 4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    /// Cut to the roster's predecessor.
    Prev,
    /// Cut to the roster's successor.
    Next,
    /// Cut to a preset picked at random, never the one already showing.
    Random,
    /// Rotate now, as the director's own timer would have.
    RotateNow,
    /// Turn hands-off rotation on or off.
    ToggleAuto,
    /// Shorten the maximum dwell by one step.
    DwellDown,
    /// Lengthen the maximum dwell by one step.
    DwellUp,
}

/// The strip in display order. Exhaustive and ordered here so the labels, the
/// rectangles and the hit test cannot disagree about which control is third.
pub const BUTTONS: [Button; 7] = [
    Button::Prev,
    Button::Next,
    Button::Random,
    Button::RotateNow,
    Button::ToggleAuto,
    Button::DwellDown,
    Button::DwellUp,
];

impl Button {
    /// The label drawn in this button's rectangle.
    pub fn label(self) -> &'static str {
        match self {
            Button::Prev => "< prev",
            Button::Next => "next >",
            Button::Random => "random",
            Button::RotateNow => "rotate",
            Button::ToggleAuto => "auto",
            Button::DwellDown => "dwell -",
            Button::DwellUp => "dwell +",
        }
    }
}

/// Button width at [`REFERENCE_HEIGHT`], in device pixels.
const BUTTON_W: f32 = 104.0;
/// Button height at [`REFERENCE_HEIGHT`].
const BUTTON_H: f32 = 32.0;
/// Gap between adjacent buttons at [`REFERENCE_HEIGHT`].
const BUTTON_GAP: f32 = 8.0;
/// Top of the strip at [`REFERENCE_HEIGHT`] — below the standing header, above
/// the band [`crate::overlay`]'s list starts in.
const STRIP_TOP: f32 = 44.0;

/// Where each control sits on a console surface of `width` x `height` device
/// pixels.
///
/// Scaled by the same [`scale`] factor the routed modal lines are, so a button's
/// rectangle and the label drawn in it move together — a hit test against
/// unscaled rectangles on a scaled console is the defect this shares its factor
/// to avoid.
pub fn transport(width: f32, height: f32) -> Vec<(Button, Rect)> {
    let s = scale(height);
    let (w, h, gap) = (BUTTON_W * s, BUTTON_H * s, BUTTON_GAP * s);
    let (left, top) = (crate::overlay::LIST_INSET * s, STRIP_TOP * s);
    BUTTONS
        .iter()
        .enumerate()
        .map(|(i, button)| {
            (
                *button,
                Rect {
                    x: left + i as f32 * (w + gap),
                    y: top,
                    width: w,
                    height: h,
                },
            )
        })
        // A console too narrow to hold a button whole drops it rather than
        // drawing one that runs off the edge and cannot be clicked.
        .filter(|(_, rect)| rect.x + rect.width <= width)
        .collect()
}

/// The control under `(x, y)`, or `None` in the gaps and everywhere else.
pub fn hit_test(width: f32, height: f32, x: f32, y: f32) -> Option<Button> {
    transport(width, height)
        .into_iter()
        .find(|(_, rect)| rect.contains(x, y))
        .map(|(button, _)| button)
}

/// What a click on the console asks the shell to do.
///
/// The variants split on **who owns the rule**. `Prev`, `Next` and `RotateNow`
/// are the console's own transport. Everything else is something the settings
/// menu also does, and it is carried as that menu's own
/// [`SettingsAction`](crate::settings::SettingsAction) — produced by the menu's
/// own `edit`, not by a second copy of the rule here, so the two surfaces cannot
/// drift into two behaviours.
#[derive(Clone, Debug, PartialEq)]
pub enum ConsoleAction {
    Prev,
    Next,
    /// Cut to a preset chosen at random.
    Random,
    RotateNow,
    Settings(crate::settings::SettingsAction),
}

/// Resolve a button against the live values the settings menu displays.
pub fn action_for(button: Button, view: &crate::settings::SettingsView) -> ConsoleAction {
    use crate::settings::SettingsRow;
    match button {
        Button::Prev => ConsoleAction::Prev,
        Button::Next => ConsoleAction::Next,
        Button::Random => ConsoleAction::Random,
        Button::RotateNow => ConsoleAction::RotateNow,
        // Delegated, never restated: `edit` is the one place a row's change is
        // decided, including the clamping the dwell rows apply against each
        // other.
        Button::ToggleAuto => ConsoleAction::Settings(SettingsRow::AutoRotate.edit(true, view)),
        Button::DwellDown => ConsoleAction::Settings(SettingsRow::MaxDwell.edit(false, view)),
        Button::DwellUp => ConsoleAction::Settings(SettingsRow::MaxDwell.edit(true, view)),
    }
}

// ---------------------------------------------------------------------------
// Staging — what the rotation will take
// ---------------------------------------------------------------------------

/// The preset a rotation would land on next, given the roster and the active
/// position.
///
/// **The rotation's *which* is the roster's successor, not a director
/// decision.** [`crate::director::Director`] decides *when* to rotate and
/// nothing else — it returns a reason and the shell calls `cycle_preset`, which
/// steps the roster forward and wraps. So the honest source for a "next up" line
/// is the roster, and the test that keeps it honest compares this against the
/// name `cycle_preset` then returns rather than against anything the director
/// holds.
///
/// `None` on a roster with fewer than two entries: there is no next, and saying
/// so is better than naming the preset already on screen.
pub fn next_up<'a>(names: &[&'a str], active: usize) -> Option<&'a str> {
    if names.len() < 2 {
        return None;
    }
    names.get((active + 1) % names.len()).copied()
}

/// A roster position picked from `seed`, never the one already showing.
///
/// Drawn out of the caller's own state rather than from a clock or an added
/// dependency: the operator wants *a different scene*, and the only property
/// that matters is that it is not the one on screen. Picking within the
/// `count - 1` positions that are not `active` and then stepping past `active`
/// makes "never the current one" structural, so no retry loop can spin on a
/// one-preset roster.
///
/// `None` when there is nothing else to go to.
pub fn random_index(count: usize, active: usize, seed: u32) -> Option<usize> {
    if count < 2 {
        return None;
    }
    let pick = (seed as usize) % (count - 1);
    Some(if pick >= active % count {
        pick + 1
    } else {
        pick
    })
}

/// The predecessor's index, wrapped — what the `prev` control selects.
///
/// Computed here rather than asked of the core: `Renderer` exposes a forward
/// `cycle_preset` and an indexed `select_preset`, and a wrapped decrement is
/// arithmetic the shell can do. Widening the core's surface for it would be an
/// ADR question, not a convenience.
pub fn previous_index(count: usize, active: usize) -> Option<usize> {
    if count == 0 {
        return None;
    }
    Some((active + count - 1) % count)
}

/// The standing line naming what a rotation takes next, and whether hands-off
/// rotation is even running.
///
/// Says *why* there is nothing to name rather than naming a guess: a roster of
/// one has no successor, and an operator reading "next: —" needs to know which
/// of the two states they are in.
pub fn staging_line(next: Option<&str>, auto: bool, dwell: (u32, u32)) -> Line {
    // The dwell bounds ride here because the two nudge controls change a number
    // with no other reading on the surface: a `dwell -` that reports nothing
    // leaves an operator unable to tell a press that landed from one that hit a
    // clamp. It is the same pair the settings menu's Min/Max dwell rows show.
    let (min, max) = dwell;
    let body = match (next, auto) {
        (Some(name), true) => format!("next up  -  {name}      dwell {min}-{max} s"),
        (Some(name), false) => {
            format!("next up  -  {name}  (auto off)      dwell {min}-{max} s")
        }
        (None, _) => format!(
            "next up  -  nothing to rotate to; the roster holds one preset      dwell {min}-{max} s"
        ),
    };
    Line::new(
        body,
        crate::overlay::LIST_INSET,
        STRIP_TOP + BUTTON_H + crate::overlay::LIST_INSET,
        crate::overlay::ROW_SIZE,
        STAGING_COLOR,
    )
}

/// The staging line's colour — brighter than the header, dimmer than a modal
/// row: it is standing information, not the thing being driven.
const STAGING_COLOR: [f32; 4] = [0.72, 0.78, 0.88, 0.95];

/// The transport's labels as lines, at the reference geometry.
///
/// Positioned at the reference size like every other line the console draws, so
/// [`scale_lines`] moves them onto the real window by the same factor
/// [`transport`] scales the rectangles by.
pub fn transport_lines(auto: bool) -> Vec<Line> {
    BUTTONS
        .iter()
        .enumerate()
        .map(|(i, button)| {
            let lit = !matches!(button, Button::ToggleAuto) || auto;
            Line::new(
                button.label().to_owned(),
                crate::overlay::LIST_INSET + i as f32 * (BUTTON_W + BUTTON_GAP) + BUTTON_GAP,
                STRIP_TOP + BUTTON_GAP,
                crate::overlay::ROW_SIZE,
                if lit { BUTTON_COLOR } else { BUTTON_OFF_COLOR },
            )
        })
        .collect()
}

/// A live control's label colour.
const BUTTON_COLOR: [f32; 4] = [0.86, 0.90, 0.96, 1.0];
/// The `auto` label while hands-off rotation is off — the one control whose
/// label reports a state as well as offering an action.
const BUTTON_OFF_COLOR: [f32; 4] = [0.45, 0.48, 0.55, 1.0];

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
