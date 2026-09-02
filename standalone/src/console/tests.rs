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

// --- the transport strip ----------------------------------------------------

/// A console large enough that every control fits, at a height that is not the
/// reference — so a layout that forgot to scale is visible in the numbers.
const CONSOLE: (f32, f32) = (900.0, 640.0);

/// `hit_test` finds each control inside its own rectangle and nothing outside
/// it.
///
/// Asserted at all four corners and one pixel past all four edges, per control,
/// because the interesting failures are on the boundary: an inclusive right edge
/// makes the shared column between two buttons belong to both, and an off-by-one
/// top makes the first row of pixels dead.
#[test]
fn hit_test_finds_each_control_and_only_inside_it() {
    let (w, h) = CONSOLE;
    let strip = transport(w, h);
    assert_eq!(
        strip.len(),
        BUTTONS.len(),
        "a {w}x{h} console is wide enough for the whole strip"
    );

    for (button, rect) in &strip {
        // Inside: the top-left corner is in, and so is a point just inside the
        // far corner. The far corner *itself* is out — the rectangle is
        // half-open so adjacent buttons cannot both claim a shared edge.
        for (x, y) in [
            (rect.x, rect.y),
            (rect.x + rect.width - 0.5, rect.y),
            (rect.x, rect.y + rect.height - 0.5),
            (rect.x + rect.width - 0.5, rect.y + rect.height - 0.5),
        ] {
            assert_eq!(
                hit_test(w, h, x, y),
                Some(*button),
                "{button:?} should own ({x}, {y}) inside {rect:?}"
            );
        }

        // One pixel past each edge is not this control. It may be the
        // neighbour — the strip is contiguous but for its gaps — so the
        // assertion is "not this one" rather than "nothing".
        for (x, y) in [
            (rect.x - 1.0, rect.y + rect.height / 2.0),
            (rect.x + rect.width + 1.0, rect.y + rect.height / 2.0),
            (rect.x + rect.width / 2.0, rect.y - 1.0),
            (rect.x + rect.width / 2.0, rect.y + rect.height + 1.0),
        ] {
            assert_ne!(
                hit_test(w, h, x, y),
                Some(*button),
                "{button:?} claims ({x}, {y}), which is outside {rect:?}"
            );
        }
    }
}

/// The gaps between controls belong to nobody, and so does the empty space
/// below the strip.
#[test]
fn the_gaps_and_the_empty_console_hit_nothing() {
    let (w, h) = CONSOLE;
    let strip = transport(w, h);
    for pair in strip.windows(2) {
        let [(_, left), (_, right)] = pair else {
            continue;
        };
        let mid = (left.x + left.width + right.x) / 2.0;
        assert_eq!(
            hit_test(w, h, mid, left.y + left.height / 2.0),
            None,
            "the gap at x={mid} between two controls hit one of them"
        );
    }
    assert_eq!(hit_test(w, h, w / 2.0, h - 1.0), None);
    assert_eq!(hit_test(w, h, -1.0, -1.0), None);
}

/// The strip scales with the console, and the rectangles move with it.
///
/// A console at half the reference height must not lay its buttons out at full
/// size: the labels drawn into them are scaled by the same factor, and a hit
/// test against unscaled rectangles would find a control nowhere near where the
/// operator can see it.
#[test]
fn the_strip_scales_with_the_console_it_is_drawn_on() {
    let reference = transport(1920.0, 1080.0);
    let small = transport(900.0, 640.0);
    let (_, big_first) = reference.first().expect("the reference strip is not empty");
    let (_, small_first) = small.first().expect("the small strip is not empty");
    assert!(
        small_first.width < big_first.width,
        "a smaller console must lay out smaller controls: {small_first:?} \
         against {big_first:?}"
    );
    let ratio = small_first.width / big_first.width;
    assert!(
        (ratio - scale(640.0)).abs() < 1e-3,
        "the strip scaled by {ratio}, which is not the {} the routed lines are \
         scaled by — the labels and their hit rectangles would disagree",
        scale(640.0)
    );
}

/// A console too narrow for the whole strip drops the controls that would not
/// fit rather than drawing them off the edge.
#[test]
fn a_narrow_console_drops_what_it_cannot_draw() {
    let narrow = transport(220.0, 640.0);
    assert!(
        narrow.len() < BUTTONS.len(),
        "a 220 px console cannot hold all {} controls",
        BUTTONS.len()
    );
    for (_, rect) in &narrow {
        assert!(
            rect.x + rect.width <= 220.0,
            "{rect:?} runs off a 220 px console"
        );
    }
}

// --- the transport's actions ------------------------------------------------

/// The live values a settings row is edited against. Built here rather than
/// borrowed from `settings/tests.rs`, so the two modules' test fixtures stay
/// independent and this one can pick the values the assertions below need — a
/// dwell well inside its range, so a step is a step and not a clamp.
fn view() -> crate::settings::SettingsView {
    crate::settings::SettingsView {
        tier: rlx_core::render::Tier::Rich,
        tier_state: crate::settings::TierState::Pinned,
        auto_rotate: true,
        min_dwell_secs: 20,
        max_dwell_secs: 90,
        fullscreen: false,
        display_index: 0,
        display_count: 1,
        display_name: "display".to_owned(),
        diagnostics: false,
        input_mode: crate::config::InputMode::Loopback,
        input_device_index: 0,
        input_device_count: 0,
        input_device_name: String::new(),
        input_editable: true,
        preset_name: true,
        now_playing: true,
        console: false,
        preset_dir: String::new(),
    }
}

/// **The two surfaces cannot drift.** A rotation control on the console and the
/// equivalent settings row produce the *same* action value for the same change,
/// because the console asks the row rather than restating its rule.
///
/// Asserted on the value and not on the behaviour: two paths that happen to
/// agree today are one edit away from not, and the value is what the shell acts
/// on.
#[test]
fn a_transport_control_and_its_settings_row_ask_for_the_same_thing() {
    use crate::settings::{SettingsAction, SettingsRow};
    let view = view();

    assert_eq!(
        action_for(Button::ToggleAuto, &view),
        ConsoleAction::Settings(SettingsRow::AutoRotate.edit(true, &view))
    );
    assert_eq!(
        action_for(Button::DwellUp, &view),
        ConsoleAction::Settings(SettingsRow::MaxDwell.edit(true, &view))
    );
    assert_eq!(
        action_for(Button::DwellDown, &view),
        ConsoleAction::Settings(SettingsRow::MaxDwell.edit(false, &view))
    );

    // And the delegation carries the row's clamping, not just its shape: the
    // dwell rows clamp against each other, so this is a value and not a
    // coincidence of enum variants.
    let ConsoleAction::Settings(SettingsAction::SetDwell { max_secs, .. }) =
        action_for(Button::DwellUp, &view)
    else {
        panic!("the dwell control must ask for a dwell change");
    };
    assert!(
        max_secs > view.max_dwell_secs,
        "dwell + must lengthen the dwell, got {max_secs} from {}",
        view.max_dwell_secs
    );
}

/// The three cut controls are the console's own and reach no settings row.
#[test]
fn the_cut_controls_are_the_consoles_own() {
    let view = view();
    assert_eq!(action_for(Button::Prev, &view), ConsoleAction::Prev);
    assert_eq!(action_for(Button::Next, &view), ConsoleAction::Next);
    assert_eq!(
        action_for(Button::RotateNow, &view),
        ConsoleAction::RotateNow
    );
}

// --- staging ----------------------------------------------------------------

#[test]
fn the_predecessor_wraps_and_an_empty_roster_has_none() {
    assert_eq!(previous_index(4, 0), Some(3));
    assert_eq!(previous_index(4, 3), Some(2));
    assert_eq!(previous_index(1, 0), Some(0));
    assert_eq!(previous_index(0, 0), None);
}

#[test]
fn a_roster_of_one_stages_nothing_and_says_so() {
    assert_eq!(next_up(&["only"], 0), None);
    assert_eq!(next_up(&[], 0), None);
    let line = staging_line(None, true, DWELL);
    assert!(
        line.text.contains("nothing to rotate to"),
        "a roster with no successor must say why rather than name a guess: {}",
        line.text
    );
}

/// The staging line reports whether hands-off rotation is running, so an
/// operator reading a name knows whether anything will act on it.
#[test]
fn the_staging_line_names_the_successor_and_the_rotation_state() {
    let names = ["a", "b", "c"];
    let next = next_up(&names, 1).expect("a three-preset roster has a successor");
    assert_eq!(next, "c");

    let on = staging_line(Some(next), true, DWELL);
    assert!(on.text.contains('c'), "{}", on.text);
    assert!(!on.text.contains("auto off"), "{}", on.text);

    let off = staging_line(Some(next), false, DWELL);
    assert!(off.text.contains("auto off"), "{}", off.text);
}

/// **The announced name is the one the rotation then takes.**
///
/// The one assertion here that cannot be made against `next_up` alone: a pure
/// test of the successor rule would only restate the rule. So this drives a real
/// [`Director`](crate::director::Director) to a real rotation and compares what
/// the console announced *beforehand* with the name `cycle_preset` returns
/// *afterwards*, against the shipped roster.
///
/// Needs a GPU device for the roster, so it takes ADR-0016's skip shape.
///
/// **There is no shuffled or random rotation policy to cover.** The director
/// decides only *when*; `cycle_preset` steps the roster forward and wraps, so a
/// single next preset always exists. The phase's "says so rather than naming a
/// guess" branch is reachable only through a roster too short to have a
/// successor, which `a_roster_of_one_stages_nothing_and_says_so` covers.
#[test]
fn the_staged_name_is_the_one_the_rotation_then_takes() {
    use rlx_core::dsp::AnalysisFrame;
    use rlx_core::render::{HeadlessOptions, RenderError, Renderer};

    let mut renderer = match Renderer::new_headless(HeadlessOptions {
        width: 64,
        height: 64,
        prefer_software: true,
    }) {
        Ok(r) => r,
        Err(RenderError::RequestAdapter(_)) => {
            eprintln!("skipped: no GPU adapter on this runner (ADR-0016)");
            return;
        }
        Err(e) => panic!("headless renderer build failed: {e}"),
    };

    // Zero dwell bounds so the timer fires on the first advance: this test is
    // about *which* preset a rotation takes, and the *when* has its own tests
    // next door in `director/tests.rs`.
    let mut director = crate::director::Director::from_config(&crate::config::Rotate {
        auto: true,
        min_dwell_secs: 0,
        max_dwell_secs: 0,
        track_change: false,
    });

    let names: Vec<&str> = renderer.preset_names().collect();
    assert!(
        names.len() > 1,
        "the embedded roster needs a successor for this to say anything"
    );
    let announced = next_up(&names, renderer.active_index()).map(str::to_owned);
    assert!(
        announced.is_some(),
        "a multi-preset roster stages a successor"
    );

    let fired = director.advance(1.0 / 60.0, &AnalysisFrame::default());
    assert!(
        fired.is_some(),
        "a director at zero dwell must rotate on its first advance, or this \
         test is comparing against a rotation that never happened"
    );

    let taken = renderer.cycle_preset().to_owned();
    assert_eq!(
        announced.as_deref(),
        Some(taken.as_str()),
        "the console announced a different preset than the rotation took — the \
         staging line is computed from something other than what cycle_preset \
         steps through"
    );
}

/// The predecessor the `prev` control selects is the one `select_preset` lands
/// on, and stepping back then forward returns to where it started.
///
/// **`prev` adds nothing to the core's public surface**, which is the phase's
/// own constraint: the wrapped decrement is arithmetic done here and handed to
/// the indexed selector the core already exposes.
#[test]
fn prev_lands_on_the_predecessor_using_only_the_indexed_selector() {
    use rlx_core::render::{HeadlessOptions, RenderError, Renderer};

    let mut renderer = match Renderer::new_headless(HeadlessOptions {
        width: 64,
        height: 64,
        prefer_software: true,
    }) {
        Ok(r) => r,
        Err(RenderError::RequestAdapter(_)) => {
            eprintln!("skipped: no GPU adapter on this runner (ADR-0016)");
            return;
        }
        Err(e) => panic!("headless renderer build failed: {e}"),
    };

    let count = renderer.preset_names().count();
    let start = renderer.active_index();
    let names: Vec<String> = renderer.preset_names().map(str::to_owned).collect();

    // From index 0 the predecessor is the last entry — the wrap is the case a
    // saturating decrement would get wrong, and index 0 is where a fresh
    // renderer starts.
    let back = previous_index(count, start).expect("a non-empty roster has a predecessor");
    assert_eq!(back, count - 1, "the predecessor of the first entry wraps");
    let landed = renderer.select_preset(back).to_owned();
    assert_eq!(
        Some(landed.as_str()),
        names.get(back).map(String::as_str),
        "select_preset landed somewhere other than the computed predecessor"
    );

    // And forward again returns to where it started, so the two directions are
    // inverses rather than two independent walks.
    let forward = renderer.cycle_preset().to_owned();
    assert_eq!(
        Some(forward.as_str()),
        names.get(start).map(String::as_str),
        "stepping back then forward did not return to the starting preset"
    );
}

/// The dwell bounds the staging assertions read, well inside the row's range so
/// the numbers in the line are the numbers passed in.
const DWELL: (u32, u32) = (20, 90);

/// **The dwell nudges report the value they move.** Two controls that change a
/// number with no reading of it on the surface leave an operator unable to tell
/// a press that landed from one that hit a clamp.
#[test]
fn the_staging_line_reports_the_dwell_the_nudges_move() {
    let line = staging_line(Some("b"), true, (20, 90));
    assert!(
        line.text.contains("20-90 s"),
        "the strip must show the dwell bounds its own controls change: {}",
        line.text
    );

    // And it is the pair passed in, not a constant: a line that always printed
    // the defaults would pass the assertion above forever.
    let moved = staging_line(Some("b"), true, (15, 45));
    assert!(moved.text.contains("15-45 s"), "{}", moved.text);
}

/// The random cut never lands on the preset already showing, at every position
/// of the roster and for every seed the mixer can produce a residue from.
///
/// **Asserted exhaustively rather than sampled**, because "never the current
/// one" is the whole contract: a retry-loop implementation is correct almost
/// always and spins forever on a one-preset roster, and a modulo that forgets
/// to step past `active` returns it exactly once per lap.
#[test]
fn the_random_cut_never_returns_the_preset_already_showing() {
    for count in 2..12usize {
        for active in 0..count {
            for seed in 0..64u32 {
                let picked = random_index(count, active, seed)
                    .expect("a roster of two or more always has somewhere else to go");
                assert!(picked < count, "{picked} is off a roster of {count}");
                assert_ne!(
                    picked, active,
                    "count {count}, active {active}, seed {seed}: the random cut returned the preset already on screen"
                );
            }
        }
    }
}

/// Every other position is reachable, so the control is a cut and not a shuffle
/// between two scenes.
#[test]
fn the_random_cut_can_reach_every_other_preset() {
    let count = 6;
    let active = 2;
    let mut seen = std::collections::BTreeSet::new();
    for seed in 0..256u32 {
        if let Some(picked) = random_index(count, active, seed) {
            seen.insert(picked);
        }
    }
    let expected: std::collections::BTreeSet<usize> = (0..count).filter(|i| *i != active).collect();
    assert_eq!(
        seen, expected,
        "the random cut cannot reach every preset that is not the active one"
    );
}

/// A roster with nowhere else to go has no random cut, rather than a cut back to
/// where it already is.
#[test]
fn a_roster_with_no_alternative_has_no_random_cut() {
    assert_eq!(random_index(1, 0, 7), None);
    assert_eq!(random_index(0, 0, 7), None);
}

/// The random control is the console's own, like the other two cuts.
#[test]
fn the_random_control_is_a_cut_and_not_a_settings_row() {
    assert_eq!(action_for(Button::Random, &view()), ConsoleAction::Random);
}
