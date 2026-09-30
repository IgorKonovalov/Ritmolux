use super::{Crossfade, ModalMotion, SLIDE_PX, Tween, progress, short_secs};
use crate::config::Motion;
use crate::console::Line;
use crate::overlay::{self, OverlayKey, OverlayState, ROW_H, Row};
use rlx_core::render::PanelKind;

const FRAME: f32 = 1.0 / 60.0;

/// **The envelope is monotone, and ends exactly on time.** Never falls, never
/// passes its end, is below its end one step before the duration and exactly at
/// it from the duration on.
#[test]
fn the_envelope_is_monotone_and_reaches_its_end_at_exactly_the_duration() {
    let d = short_secs();
    assert_eq!(progress(0.0, d, Motion::Full), 0.0);
    let mut prev = 0.0;
    for i in 0..=1000 {
        let p = progress(d * i as f32 / 1000.0, d, Motion::Full);
        assert!(p >= prev, "the envelope fell at step {i}: {prev} -> {p}");
        assert!(p <= 1.0, "the envelope overshot at step {i}: {p}");
        prev = p;
    }
    assert_eq!(
        progress(d, d, Motion::Full),
        1.0,
        "exactly 1 at the duration"
    );
    assert_eq!(progress(d * 3.0, d, Motion::Full), 1.0, "and after it");
    // The curve's settle is long and flat, so its last few hundredths round to
    // 1 in `f32`; nine tenths in, it is visibly short of the end.
    assert!(
        progress(d * 0.9, d, Motion::Full) < 1.0,
        "not before the duration"
    );

    // The same, through a tween: at rest on its target exactly at the duration.
    let mut t = Tween::settled(0.0, d);
    t.retarget(10.0, Motion::Full);
    t.advance(d * 0.9);
    assert!(t.value(Motion::Full) < 10.0);
    // Past the duration in one step: the clock clamps at it, and the value is
    // the target exactly, not a float's approach to it.
    t.advance(d * 0.2);
    assert_eq!(t.value(Motion::Full), 10.0, "on its target at the duration");
    t.advance(d);
    assert_eq!(t.value(Motion::Full), 10.0, "and never past it");
}

/// **Under `reduced`, every envelope is a step**: at its end from the first
/// instant.
#[test]
fn reduced_motion_makes_every_envelope_a_step() {
    let d = short_secs();
    for elapsed in [0.0, d * 0.01, d * 0.5, d, d * 2.0] {
        assert_eq!(progress(elapsed, d, Motion::Reduced), 1.0);
    }
    let mut t = Tween::settled(0.0, d);
    t.retarget(1.0, Motion::Reduced);
    assert_eq!(t.value(Motion::Reduced), 1.0, "no frame in between");

    // A modal opens fully on its first frame and is gone on the first frame
    // after it closes.
    let mut m = ModalMotion::default();
    let mut lines = vec![Line::new("row".into(), 16.0, 94.0, 22.0, [1.0; 4])];
    m.frame(true, &mut lines, 0, FRAME, Motion::Reduced);
    assert_eq!(lines[0].color[3], 1.0);
    assert_eq!(lines[0].y, 94.0, "no slide");
    let mut closed: Vec<Line> = Vec::new();
    m.frame(false, &mut closed, 0, FRAME, Motion::Reduced);
    assert!(closed.is_empty(), "a step leaves nothing behind");
}

/// A retarget mid-flight turns around from where the value is, rather than
/// jumping to the start of the new envelope.
#[test]
fn a_reversed_envelope_turns_around_where_it_is() {
    let d = short_secs();
    let mut t = Tween::settled(0.0, d);
    t.retarget(1.0, Motion::Full);
    t.advance(d * 0.3);
    let mid = t.value(Motion::Full);
    assert!(mid > 0.0 && mid < 1.0);
    t.retarget(0.0, Motion::Full);
    assert_eq!(t.value(Motion::Full), mid, "no jump on the reversal");
}

fn rows() -> Vec<Row<'static>> {
    ["Aurora", "Basalt", "Coral", "Dune", "Ember"]
        .into_iter()
        .map(|name| Row {
            name,
            family: "curve",
            favourite: false,
            hidden: false,
        })
        .collect()
}

fn mono(text: &str, size: f32) -> f32 {
    text.chars().count() as f32 * size * 0.55
}

/// One frame of the browser as the shell draws it: its lines, animated.
fn browse_frame(state: &OverlayState, rows: &[Row<'_>], m: &mut ModalMotion) -> Vec<Line> {
    let visible = state.visible(rows);
    let layout = overlay::layout(visible.len(), state.highlight(), 1920.0, 1080.0);
    let mut lines = Vec::new();
    if state.is_open() {
        overlay::browse_lines(state, &visible, &layout, &mut mono, &mut lines);
    }
    m.frame(state.is_open(), &mut lines, 0, FRAME, Motion::Full);
    lines
}

/// The text line carrying the highlight marker.
fn marker_y(lines: &[Line]) -> f32 {
    lines
        .iter()
        .find(|l| l.text.starts_with('>'))
        .map(|l| l.y)
        .expect("a highlighted row")
}

fn highlight(lines: &[Line]) -> &Line {
    lines
        .iter()
        .find(|l| l.backdrop.is_some_and(|b| b.kind == PanelKind::Highlight))
        .expect("a highlight")
}

/// **A key pressed mid-animation acts at once.** The browser is still opening
/// when `Down` arrives, and on that same frame the state machine has moved the
/// selection and the marker is drawn on the new row — the envelope changes how
/// the block is drawn, never what the key did.
#[test]
fn a_key_during_the_open_animation_moves_the_selection_on_that_frame() {
    let rows = rows();
    let mut state = OverlayState::new();
    let mut m = ModalMotion::default();
    let probe = overlay::layout(rows.len(), 0, 1920.0, 1080.0);
    state.handle_key(OverlayKey::Toggle, &rows, 0, &probe);
    let first = browse_frame(&state, &rows, &mut m);
    let open = m.openness(Motion::Full);
    assert!(open > 0.0 && open < 1.0, "still opening, at {open}");
    let before = marker_y(&first);

    state.handle_key(OverlayKey::Down, &rows, 0, &probe);
    assert_eq!(state.highlight(), 1, "the state moved on the key");
    let second = browse_frame(&state, &rows, &mut m);
    assert!(
        m.openness(Motion::Full) < 1.0,
        "the key landed mid-animation"
    );
    // Both frames carry the same slide, so the marker moved by one row pitch
    // plus whatever the slide advanced — and it moved down, onto row 1.
    let moved = marker_y(&second) - before;
    assert!(
        (ROW_H..=ROW_H + SLIDE_PX).contains(&moved),
        "the marker moved {moved} px, not onto the next row"
    );
}

/// The highlight glides: the frame after the selection moves it is between the
/// two rows, and it rests on the new row once the envelope has run.
#[test]
fn the_selection_highlight_glides_between_rows() {
    let rows = rows();
    let mut state = OverlayState::new();
    let mut m = ModalMotion::default();
    let probe = overlay::layout(rows.len(), 0, 1920.0, 1080.0);
    state.handle_key(OverlayKey::Toggle, &rows, 0, &probe);
    // Let the open finish, so only the glide moves anything.
    for _ in 0..60 {
        browse_frame(&state, &rows, &mut m);
    }
    let from = highlight(&browse_frame(&state, &rows, &mut m)).y;

    state.handle_key(OverlayKey::Down, &rows, 0, &probe);
    let mid = highlight(&browse_frame(&state, &rows, &mut m)).y;
    assert!(
        mid > from && mid < from + ROW_H,
        "mid-glide at {mid}, from {from}"
    );
    for _ in 0..60 {
        browse_frame(&state, &rows, &mut m);
    }
    let rest = highlight(&browse_frame(&state, &rows, &mut m)).y;
    assert_eq!(rest, from + ROW_H, "the glide rests on the new row");
}

/// A closed modal fades out what it last drew, then draws nothing.
#[test]
fn a_closing_modal_fades_its_last_lines_out() {
    let mut m = ModalMotion::default();
    let built = vec![Line::new("row".into(), 16.0, 94.0, 22.0, [1.0; 4])];
    for _ in 0..60 {
        let mut lines = built.clone();
        m.frame(true, &mut lines, 0, FRAME, Motion::Full);
    }
    let mut prev = 1.0;
    let mut frames = 0;
    loop {
        let mut lines = Vec::new();
        m.frame(false, &mut lines, 0, FRAME, Motion::Full);
        frames += 1;
        let Some(line) = lines.first() else { break };
        assert_eq!(line.text, "row");
        assert!(line.color[3] <= prev, "the close must not brighten");
        prev = line.color[3];
        assert!(frames < 120, "the close never finished");
    }
    let expected = (short_secs() / FRAME).ceil() as i32;
    assert!(
        (frames - expected).abs() <= 1,
        "the close took {frames} frames, the theme says about {expected}"
    );
}

/// The corner name crossfades: the frame after it changes both names are drawn,
/// the new one rising and the old one falling, and the old one is gone once the
/// envelope has run.
#[test]
fn a_new_preset_name_crossfades_over_the_old_one() {
    let plate = |name: &str| vec![Line::new(name.to_owned(), 16.0, 16.0, 28.0, [1.0; 4])];
    let mut c = Crossfade::default();
    let mut lines = plate("Aurora");
    c.frame(&mut lines, 0, FRAME, Motion::Full);
    assert_eq!(lines.len(), 1, "the first name has nothing to fade from");

    let mut lines = plate("Basalt");
    c.frame(&mut lines, 0, FRAME, Motion::Full);
    assert_eq!(lines.len(), 2);
    let (new, old) = (&lines[0], &lines[1]);
    assert_eq!((new.text.as_str(), old.text.as_str()), ("Basalt", "Aurora"));
    assert!(new.color[3] > 0.0 && new.color[3] < 1.0);
    assert!((new.color[3] + old.color[3] - 1.0).abs() < 1e-6);

    for _ in 0..60 {
        let mut lines = plate("Basalt");
        c.frame(&mut lines, 0, FRAME, Motion::Full);
    }
    let mut lines = plate("Basalt");
    c.frame(&mut lines, 0, FRAME, Motion::Full);
    assert_eq!(lines.len(), 1, "the old name is gone");
    assert_eq!(lines[0].color[3], 1.0);
}
