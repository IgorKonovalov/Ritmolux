use super::*;

/// A line whose text is `name`; the geometry is irrelevant to routing and is
/// held constant so an inequality can only ever come from the routing itself.
fn line(name: &str) -> Line {
    Line::new(name.to_owned(), 16.0, 64.0, 22.0, [1.0, 1.0, 1.0, 1.0])
}

fn lines(names: &[&str]) -> Vec<Line> {
    names.iter().map(|n| line(n)).collect()
}

// --- routing: what reaches the projector ------------------------------------

/// **The guarantee the feature rests on.** With the console open, the output's
/// lines are the same whether a modal is open or not — so opening the preset
/// browser mid-show cannot paint anything onto the screen the audience sees.
///
/// Asserted as equality against the no-modal case rather than as "contains no
/// modal text": a routing that appended an empty header, a separator or a
/// scrollbar row would pass the weaker check while still changing the show.
#[test]
fn console_open_keeps_modal_text_off_the_output() {
    let chrome = lines(&["aurora_drift"]);

    let closed = route(chrome.clone(), Vec::new(), Console::Open);
    let browse = route(
        chrome.clone(),
        lines(&["> rose_star", "  ink_bloom"]),
        Console::Open,
    );
    let menu = route(
        chrome,
        lines(&["> tier    rich", "  fps     60"]),
        Console::Open,
    );

    assert_eq!(browse.output, closed.output);
    assert_eq!(menu.output, closed.output);
}

/// The modal is not merely absent from the output — it actually arrived
/// somewhere. Without this, a routing that dropped the rows entirely would pass
/// the test above.
#[test]
fn console_open_receives_the_modal_rows() {
    let rows = lines(&["> rose_star", "  ink_bloom"]);
    let frame = route(lines(&["aurora_drift"]), rows.clone(), Console::Open);

    assert_eq!(frame.console, rows);
}

/// With the console closed, the app draws exactly what it drew before this
/// existed: chrome first, then the modal, on the output, with nothing stranded.
#[test]
fn console_closed_draws_everything_on_the_output() {
    let chrome = lines(&["aurora_drift"]);
    let rows = lines(&["> rose_star", "  ink_bloom"]);

    let frame = route(chrome.clone(), rows.clone(), Console::Closed);

    let mut expected = chrome;
    expected.extend(rows);
    assert_eq!(frame.output, expected);
    assert!(frame.console.is_empty());
}

/// Chrome is the picture's own furniture and never follows the operator to the
/// console — the corner preset name belongs on the show.
#[test]
fn chrome_stays_on_the_output_either_way() {
    let chrome = lines(&["aurora_drift"]);

    let open = route(chrome.clone(), lines(&["> rose_star"]), Console::Open);
    let closed = route(chrome.clone(), Vec::new(), Console::Closed);

    assert_eq!(open.output, chrome);
    assert_eq!(closed.output, chrome);
}

/// One state machine, two destinations: the same modal rows routed to either
/// surface are the *same lines*, not two independently built lists. This is what
/// makes a console that disagrees with the output impossible rather than merely
/// unobserved.
#[test]
fn both_surfaces_are_fed_the_same_rows() {
    let rows = lines(&["> rose_star", "  ink_bloom", "  warp_tide"]);

    let to_console = route(Vec::new(), rows.clone(), Console::Open).console;
    let to_output = route(Vec::new(), rows.clone(), Console::Closed).output;

    assert_eq!(to_console, to_output);
    assert_eq!(to_console, rows);
}

// --- dispatch: which window a key came from ---------------------------------

/// The same key delivered with the two window ids reaches two different
/// targets — the property that replaces `window_event`'s ignored `_id`.
#[test]
fn the_two_windows_dispatch_differently() {
    let (output, console) = (1u32, 2u32);

    assert_eq!(dispatch(&output, &output, Some(&console)), Target::Output);
    assert_eq!(dispatch(&console, &output, Some(&console)), Target::Console);
}

/// With no console open, its former id is not a target — a stale event from a
/// window just closed is dropped rather than routed at the output.
#[test]
fn a_closed_console_claims_no_events() {
    let (output, console) = (1u32, 2u32);

    assert_eq!(dispatch(&console, &output, None), Target::Unknown);
    assert_eq!(dispatch(&output, &output, None), Target::Output);
}

/// An id belonging to neither window resolves to neither.
#[test]
fn an_unknown_window_is_unknown() {
    assert_eq!(dispatch(&9u32, &1u32, Some(&2u32)), Target::Unknown);
}

// --- scale: fitting the list into the console's own window ------------------

/// A console smaller than the reference shrinks; one at or above it does not
/// magnify. The show's own type size is the ceiling, never a starting point to
/// grow from.
#[test]
fn a_small_console_shrinks_and_a_large_one_does_not_grow() {
    assert!(scale(640.0) < 1.0);
    assert_eq!(scale(1080.0), 1.0);
    assert_eq!(scale(2160.0), 1.0);
}

/// The shrink has a floor: an operator who cannot read the list across a desk
/// is worse off than one scrolling a larger one.
#[test]
fn the_scale_has_a_readable_floor() {
    assert_eq!(scale(1.0), MIN_SCALE);
    assert_eq!(scale(100.0), MIN_SCALE);
}

/// A degenerate size never produces a scale that would collapse the layout or
/// poison every coordinate with a NaN.
#[test]
fn a_degenerate_height_falls_back_to_unity() {
    assert_eq!(scale(0.0), 1.0);
    assert_eq!(scale(-10.0), 1.0);
    assert_eq!(scale(f32::NAN), 1.0);
}

/// **The property the fix exists for.** Laying out at the logical size and then
/// scaling by the same factor lands the result inside the real window — so the
/// grid the browser computes is one that fits, rather than the top-left corner
/// of a grid built for the projector.
#[test]
fn the_logical_size_scales_back_into_the_real_window() {
    let (w, h) = (900.0, 640.0);
    let (lw, lh) = logical_size(w, h);
    let s = scale(h);

    assert!((lw * s - w).abs() < 0.01);
    assert!((lh * s - h).abs() < 0.01);
    // And it is genuinely bigger, which is what buys the extra columns and rows.
    assert!(lw > w);
    assert!(lh > h);
}

/// Scaling moves positions and type together, so the list keeps its proportions
/// instead of shrinking the font inside a full-size grid.
#[test]
fn scaling_moves_position_and_size_together() {
    let mut rows = vec![Line::new("row".to_owned(), 100.0, 200.0, 22.0, [1.0; 4])];
    scale_lines(&mut rows, 0.5);

    let row = &rows[0];
    assert_eq!((row.x, row.y, row.size), (50.0, 100.0, 11.0));
    // Colour and text are untouched — this is geometry only.
    assert_eq!(row.text, "row");
    assert_eq!(row.color, [1.0; 4]);
}
