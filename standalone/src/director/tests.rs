use super::*;

/// A frame whose bass/mid/treb sum to `energy` (split evenly). Other fields
/// don't affect the director.
fn frame(energy: f32) -> AnalysisFrame {
    let third = energy / 3.0;
    AnalysisFrame {
        bass: third,
        mid: third,
        treb: third,
        ..AnalysisFrame::default()
    }
}

/// A frame with a given energy and novelty score.
fn frame_nov(energy: f32, novelty: f32) -> AnalysisFrame {
    AnalysisFrame {
        novelty,
        ..frame(energy)
    }
}

fn make(auto: bool, min: u32, max: u32, track_change: bool) -> Director {
    Director::from_config(&config::Rotate {
        auto,
        min_dwell_secs: min,
        max_dwell_secs: max,
        track_change,
        ..config::Rotate::default()
    })
}

fn director(auto: bool, min: u32, max: u32) -> Director {
    make(auto, min, max, false)
}

#[test]
fn steady_passage_rotates_at_max_dwell() {
    let mut d = director(true, 20, 90);
    let steady = frame(1.5);
    // No rotation for the first 89 seconds of a steady, high-energy passage.
    for step in 1..90 {
        assert_eq!(d.advance(1.0, &steady), None, "rotated early at {step}s");
    }
    // The 90th second hits the max-dwell cap.
    assert_eq!(d.advance(1.0, &steady), Some(Rotation::AutoTimer));
}

#[test]
fn energy_drop_rotates_earlier_than_the_cap() {
    let mut d = director(true, 20, 90);
    let loud = frame(1.5);
    // Warm the baseline high and pass the softened drop gate (~37.5 s at the
    // 20/90 default) with a steady passage -> no rotation yet.
    for _ in 0..40 {
        assert_eq!(d.advance(1.0, &loud), None);
    }
    // A sharp drop, now past the drop gate, rotates well before the 90 s cap.
    assert_eq!(d.advance(1.0, &frame(0.1)), Some(Rotation::AutoDrop));
}

#[test]
fn drop_before_min_dwell_holds() {
    let mut d = director(true, 20, 90);
    let loud = frame(1.5);
    for _ in 0..10 {
        assert_eq!(d.advance(1.0, &loud), None);
    }
    // A drop at ~11 s is still inside the min dwell: hold, don't rotate.
    assert_eq!(d.advance(1.0, &frame(0.1)), None);
}

#[test]
fn drop_between_min_dwell_and_gate_is_held() {
    // The softened drop gate (ADR-0027): a drop that lands past the min dwell
    // but before the gate (~37.5 s at the 20/90 default) must NOT rotate, so
    // a drop shortly after a rotation can't rapid-fire another.
    let mut d = director(true, 20, 90);
    let loud = frame(1.5);
    // Warm high and settle past the min dwell but short of the drop gate.
    for _ in 0..25 {
        assert_eq!(d.advance(1.0, &loud), None);
    }
    // A sharp drop at ~26 s (past min 20, before gate ~37.5) is held.
    assert_eq!(d.advance(1.0, &frame(0.1)), None);
}

#[test]
fn manual_next_resets_the_dwell() {
    let mut d = director(true, 20, 90);
    let steady = frame(1.5);
    // Approach the cap...
    for _ in 0..89 {
        assert_eq!(d.advance(1.0, &steady), None);
    }
    // ...then force a manual rotation, which resets the countdown.
    assert_eq!(d.force_next(), Rotation::Manual);
    // The very next steady second must NOT rotate (dwell restarted at 0).
    assert_eq!(d.advance(1.0, &steady), None);
}

#[test]
fn auto_off_never_auto_rotates_but_manual_still_works() {
    let mut d = director(false, 8, 40);
    let loud = frame(1.5);
    // Long steady run plus a drop: no automatic rotation while frozen.
    for _ in 0..100 {
        assert_eq!(d.advance(1.0, &loud), None);
    }
    assert_eq!(d.advance(1.0, &frame(0.1)), None);
    // Manual override still fires.
    assert_eq!(d.force_next(), Rotation::Manual);
}

#[test]
fn default_config_holds_one_scene_but_manual_overrides_work() {
    // ADR-0027: a fresh install (default config) holds one scene — auto is
    // off, so no automatic rotation ever fires, even through a long steady
    // run and a sharp drop.
    let mut d = Director::from_config(&config::Rotate::default());
    assert!(!d.auto_enabled());
    let loud = frame(1.5);
    for _ in 0..200 {
        assert_eq!(d.advance(1.0, &loud), None);
    }
    assert_eq!(d.advance(1.0, &frame(0.1)), None);
    // But the manual next-scene hotkey still fires...
    assert_eq!(d.force_next(), Rotation::Manual);
    // ...and toggling auto on enables rotation live.
    assert!(d.toggle_auto());
    assert!(d.auto_enabled());
}

#[test]
fn toggle_auto_flips_and_reports_state() {
    let mut d = director(true, 8, 40);
    assert!(d.auto_enabled());
    assert!(!d.toggle_auto());
    assert!(!d.auto_enabled());
    assert!(d.toggle_auto());
    assert!(d.auto_enabled());
}

#[test]
fn novelty_boundary_rotates_before_the_cap() {
    let mut d = make(true, 20, 90, true);
    // Steady, no novelty, past the min dwell: still holds toward the cap.
    for _ in 0..25 {
        assert_eq!(d.advance(1.0, &frame_nov(1.0, 0.0)), None);
    }
    // A strong novelty boundary pulls the cap to the min dwell and rotates.
    assert_eq!(
        d.advance(1.0, &frame_nov(1.0, 1.0)),
        Some(Rotation::AutoBoundary)
    );
}

#[test]
fn novelty_before_min_dwell_holds() {
    let mut d = make(true, 20, 90, true);
    for _ in 0..10 {
        assert_eq!(d.advance(1.0, &frame_nov(1.0, 0.0)), None);
    }
    // A boundary at ~11 s is still inside the min dwell: novelty is never the
    // sole trigger, so it holds.
    assert_eq!(d.advance(1.0, &frame_nov(1.0, 1.0)), None);
}

#[test]
fn steady_signal_never_rotates_on_novelty() {
    // Nudge enabled, but a steady low-novelty signal only rotates at the
    // hard max-dwell cap, never early.
    let mut d = make(true, 20, 90, true);
    for step in 1..90 {
        assert_eq!(
            d.advance(1.0, &frame_nov(1.0, 0.0)),
            None,
            "rotated early at {step}s"
        );
    }
    assert_eq!(
        d.advance(1.0, &frame_nov(1.0, 0.0)),
        Some(Rotation::AutoTimer)
    );
}

#[test]
fn disabled_track_change_ignores_novelty() {
    // With the nudge off, even a sustained boundary novelty can't rotate
    // before the cap.
    let mut d = make(true, 20, 90, false);
    for step in 1..90 {
        assert_eq!(
            d.advance(1.0, &frame_nov(1.0, 1.0)),
            None,
            "novelty rotated with the nudge disabled at {step}s"
        );
    }
    assert_eq!(
        d.advance(1.0, &frame_nov(1.0, 1.0)),
        Some(Rotation::AutoTimer)
    );
}

/// **A live dwell edit does not restart the clock** (Plan 0050 Phase 4). The
/// settings menu changes these bounds while the show runs, and re-deriving a
/// whole `Director` from the edited config — the obvious implementation —
/// would reset the timer under the operator every time they nudged a number.
#[test]
fn setting_dwell_bounds_keeps_the_running_clock_and_the_auto_flag() {
    let mut d = director(true, 20, 90);
    let steady = frame(1.0);
    for _ in 0..50 {
        assert_eq!(d.advance(1.0, &steady), None);
    }

    // 50 s are on the clock. Lower the cap to 60 s: the remaining wait must
    // be 10 s, not a fresh 60.
    d.set_dwell_bounds(20, 60);
    assert!(
        d.auto_enabled(),
        "the auto flag was collateral of a dwell edit"
    );
    for step in 1..10 {
        assert_eq!(
            d.advance(1.0, &steady),
            None,
            "the dwell clock restarted (rotated at {step}s after the edit)"
        );
    }
    assert_eq!(
        d.advance(1.0, &steady),
        Some(Rotation::AutoTimer),
        "50 s already elapsed + 10 s should reach the new 60 s cap"
    );

    // And it clamps the same way the constructor does, so the menu cannot
    // invert the pair even if a caller hands it a bad one.
    let mut d = director(true, 20, 90);
    d.set_dwell_bounds(30, 5);
    let steady = frame(1.0);
    for _ in 1..30 {
        assert_eq!(d.advance(1.0, &steady), None);
    }
    assert_eq!(d.advance(1.0, &steady), Some(Rotation::AutoTimer));
}

#[test]
fn inverted_dwell_config_is_clamped() {
    // max < min: the constructor clamps max up to min, so the timer is a
    // fixed min-dwell rather than an inverted, always-firing one.
    let mut d = director(true, 30, 5);
    let steady = frame(1.0);
    for step in 1..30 {
        assert_eq!(d.advance(1.0, &steady), None, "rotated early at {step}s");
    }
    assert_eq!(d.advance(1.0, &steady), Some(Rotation::AutoTimer));
}

// ---------------------------------------------------------------------------
// The eligible set, and the traversal over it
// ---------------------------------------------------------------------------

/// A library of five, so a cycle is short enough to walk exhaustively.
const LIBRARY: [&str; 5] = ["alpha", "bravo", "charlie", "delta", "echo"];

fn library() -> Vec<&'static str> {
    LIBRARY.to_vec()
}

fn marked(favourite: &[&str], hidden: &[&str]) -> Marks {
    let mut marks = Marks::default();
    for name in favourite {
        marks.apply(Mark::Favourite, name, true);
    }
    for name in hidden {
        marks.apply(Mark::Hidden, name, true);
    }
    marks
}

/// **Hidden presets never appear in auto-rotate, in either source.** The whole
/// point of the mark, and the one property that must hold whatever else is set.
#[test]
fn a_hidden_preset_is_eligible_under_neither_source() {
    let marks = marked(&["bravo", "delta"], &["bravo", "charlie"]);

    for source in [RotateSource::All, RotateSource::Favourites] {
        let eligible = eligible_names(library().into_iter(), &marks, source);
        assert!(
            !eligible.contains(&"charlie"),
            "a hidden preset reached the {source:?} source"
        );
        assert!(
            !eligible.contains(&"bravo"),
            "a preset that is both favourite and hidden must stay hidden under \
             {source:?}: hiding is 'stop showing me this', and a promotion \
             cannot outrank it"
        );
    }

    // And a draw over many rotations never lands on one either — the property
    // stated over the traversal rather than over the filter it reads.
    let mut traversal = Traversal::new(7);
    let eligible = eligible_names(library().into_iter(), &marks, RotateSource::All);
    for _ in 0..50 {
        let pick = traversal.draw(&eligible).expect("a non-empty eligible set");
        assert!(
            pick != "charlie" && pick != "bravo",
            "rotation drew a hidden preset: {pick}"
        );
    }
}

/// **Favourites-only is a hard filter with a fallback.** With favourites marked
/// it draws from them and nothing else; with none marked it draws from the whole
/// eligible set rather than holding one preset forever — a mode that looks
/// exactly like a hang is a mode nobody can debug from the window.
#[test]
fn favourites_only_narrows_and_falls_back_when_nothing_is_marked() {
    let marks = marked(&["alpha", "echo"], &[]);
    let eligible = eligible_names(library().into_iter(), &marks, RotateSource::Favourites);
    assert_eq!(eligible, ["alpha", "echo"], "the filter is not hard");

    let mut traversal = Traversal::new(3);
    for _ in 0..20 {
        let pick = traversal.draw(&eligible).expect("two favourites");
        assert!(
            pick == "alpha" || pick == "echo",
            "favourites-only drew {pick}"
        );
    }

    // Nothing marked: the fallback is the whole eligible set, hidden excluded.
    let none = marked(&[], &["delta"]);
    let eligible = eligible_names(library().into_iter(), &none, RotateSource::Favourites);
    assert_eq!(
        eligible,
        ["alpha", "bravo", "charlie", "echo"],
        "an empty favourite set must fall back to the whole eligible set"
    );

    // And a library hidden in its entirety falls back the same way, for the
    // same reason: no combination of marks may leave the show with nothing.
    let all_hidden = marked(&[], &LIBRARY);
    assert_eq!(
        eligible_names(library().into_iter(), &all_hidden, RotateSource::All).len(),
        LIBRARY.len(),
        "hiding everything left rotation with nothing to draw"
    );
}

/// **Auto-rotate does not repeat while unseen presets remain**, and a preset
/// never ends one cycle and begins the next.
///
/// Stated as a property over many cycles rather than as a fixed sequence: the
/// order is seeded and a different seed is a different order, but neither claim
/// above may depend on which.
#[test]
fn a_cycle_shows_every_eligible_preset_once_before_any_of_them_twice() {
    for seed in [0u32, 1, 7, 12345, u32::MAX] {
        let mut traversal = Traversal::new(seed);
        let eligible = library();
        let mut previous_cycle_last: Option<String> = None;

        for cycle in 0..4 {
            let mut drawn = Vec::new();
            for _ in 0..eligible.len() {
                drawn.push(traversal.draw(&eligible).expect("a non-empty set"));
            }
            if let Some(last) = &previous_cycle_last {
                assert_ne!(
                    drawn.first(),
                    Some(last),
                    "seed {seed}, cycle {cycle}: a preset ended one cycle and \
                     began the next, which reads as a repeat however shuffled \
                     the rest is"
                );
            }
            let mut sorted = drawn.clone();
            sorted.sort();
            sorted.dedup();
            assert_eq!(
                sorted.len(),
                eligible.len(),
                "seed {seed}, cycle {cycle}: {drawn:?} repeated before the \
                 cycle was exhausted"
            );
            previous_cycle_last = drawn.last().cloned();
        }
    }
}

/// **The traversal tolerates its set changing between draws** — the case the
/// plan calls most likely to be got subtly wrong: a mark toggled mid-show grows
/// or shrinks the universe under a cycle that is already running.
#[test]
fn the_traversal_survives_its_set_growing_and_shrinking_between_draws() {
    let mut traversal = Traversal::new(11);

    // Two eligible presets, one cycle's worth drawn.
    let small = vec!["alpha", "bravo"];
    let first = traversal.draw(&small).expect("two eligible");
    assert!(small.contains(&first.as_str()));

    // The set grows mid-cycle: the new names are unseen, so they are drawable
    // immediately rather than waiting for the cycle to end.
    let grown = library();
    let mut seen_new = false;
    for _ in 0..4 {
        let pick = traversal.draw(&grown).expect("a grown set");
        assert!(grown.contains(&pick.as_str()));
        seen_new |= !small.contains(&pick.as_str());
    }
    assert!(
        seen_new,
        "a preset that became eligible mid-cycle was never drawn, so the cycle \
         is walking a snapshot rather than the live set"
    );

    // And the set shrinking to one leaves that one, rather than an empty draw
    // or a name the set does not hold.
    let single = vec!["delta"];
    for _ in 0..3 {
        assert_eq!(traversal.draw(&single).as_deref(), Some("delta"));
    }

    // An empty roster is the only `None`: no combination of marks reaches it,
    // but an empty library does.
    assert_eq!(traversal.draw(&[]), None);
}

/// **What the console announces is what the rotation then takes.**
///
/// The traversal is shuffled, so a roster successor is not the answer and the
/// peek is. The announcement must also survive being asked twice,
/// or the line would name a different preset on every frame it is drawn.
#[test]
fn the_announced_preset_is_the_one_the_next_draw_takes() {
    let mut traversal = Traversal::new(99);
    let eligible = library();

    for _ in 0..10 {
        let announced = traversal
            .peek(&eligible)
            .expect("a non-empty set has a next")
            .to_owned();
        assert_eq!(
            traversal.peek(&eligible),
            Some(announced.as_str()),
            "peeking twice named two different presets, so the console line \
             would change under the operator between frames"
        );
        assert_eq!(
            traversal.draw(&eligible),
            Some(announced.clone()),
            "the rotation took a preset other than the one announced"
        );
    }
}

/// An announcement whose preset stops being eligible is **replaced**, not drawn
/// anyway — hiding the preset that is next up is the one way to reach this.
#[test]
fn hiding_the_announced_preset_replaces_it() {
    let mut traversal = Traversal::new(5);
    let eligible = library();
    let announced = traversal.peek(&eligible).expect("a next").to_owned();

    let remaining: Vec<&str> = eligible
        .iter()
        .copied()
        .filter(|name| *name != announced)
        .collect();
    let replacement = traversal.draw(&remaining).expect("four left");
    assert_ne!(
        replacement, announced,
        "a preset hidden after it was announced was drawn anyway"
    );
}

/// **"Previous" is the preset actually shown before this one**, and repeated
/// steps walk genuinely backwards rather than flipping between the last two.
#[test]
fn stepping_back_walks_what_was_shown_rather_than_an_index() {
    let mut traversal = Traversal::new(1);
    assert_eq!(
        traversal.step_back(),
        None,
        "a run that has shown nothing has nowhere to step back to"
    );

    // Four switches: each records the preset it left.
    for name in ["alpha", "bravo", "charlie", "delta"] {
        traversal.note_shown(name);
    }
    assert_eq!(traversal.trail_len(), 4);

    assert_eq!(traversal.step_back().as_deref(), Some("delta"));
    assert_eq!(
        traversal.step_back().as_deref(),
        Some("charlie"),
        "the second step returned to where the first came from, so the two \
         keys flip rather than walk"
    );
    assert_eq!(traversal.step_back().as_deref(), Some("bravo"));
    assert_eq!(traversal.step_back().as_deref(), Some("alpha"));
    assert_eq!(traversal.step_back(), None, "the trail is spent");
    assert_eq!(traversal.trail_len(), 0);
}

/// The same seed walks the same order, and a different one does not: the
/// traversal reads no clock, so a run is reproducible from its seed alone.
#[test]
fn the_order_is_a_pure_function_of_the_seed() {
    let walk = |seed| {
        let mut traversal = Traversal::new(seed);
        let eligible = library();
        (0..12)
            .map(|_| traversal.draw(&eligible).expect("a non-empty set"))
            .collect::<Vec<_>>()
    };
    assert_eq!(walk(42), walk(42), "the same seed produced two orders");
    assert_ne!(
        walk(42),
        walk(43),
        "two seeds produced one order, so the seed is not reaching the mixer"
    );
}

/// A rotation the director asks for must reach the renderer and change the
/// scene — the contract `advance`'s own doc states and the halves of which
/// drifted apart.
///
/// It pairs the two ends deliberately. `advance` returning `Some` is a
/// *decision*, and `cycle_preset` is the only thing that carries it out; a
/// caller holding one without the other produces an app whose auto-rotate
/// announces switches that never happen, which is what shipped between
/// 2026-07-26 and this test.
///
/// **What it does not cover, stated so nobody reads it as more than it is:**
/// the call site in the shell. `AppState` needs a real window and lives in the
/// binary, so nothing here can assert that the event loop's rotation branch
/// calls `rotate_to_next`. That branch is guarded structurally instead — the
/// helper pairs the change with its bookkeeping so neither can be reached
/// alone.
#[test]
fn a_rotation_the_director_asks_for_changes_the_preset() {
    use rlx_core::render::{HeadlessOptions, Renderer};

    let Ok(mut renderer) = Renderer::new_headless(HeadlessOptions {
        width: 64,
        height: 48,
        prefer_software: true,
    }) else {
        eprintln!("skipped: no GPU adapter available for a headless renderer");
        return;
    };
    assert!(
        renderer.preset_names().count() > 1,
        "a rotation is only observable across at least two presets"
    );
    let before = renderer.preset_name().to_owned();

    // Auto-rotate with a one-second cap, driven past it on steady energy.
    let mut director = make(true, 0, 1, false);
    let mut rotated = None;
    for _ in 0..120 {
        if let Some(reason) = director.advance(1.0 / 60.0, &frame(0.5)) {
            rotated = Some(reason);
            break;
        }
    }
    let reason = rotated.expect("a 1 s cap must fire within 2 s of frames");

    // `cycle_preset` returns the INCOMING name straight away; `preset_name()`
    // still reads the outgoing one until the dissolve's capture frame has
    // rendered, which is what the shell's deferred title read exists for. So
    // the incoming name is what says the roster advanced.
    let incoming = renderer.cycle_preset().to_owned();
    assert_ne!(
        incoming, before,
        "the director asked to rotate ({reason:?}) and the roster did not advance"
    );
    assert_eq!(
        renderer.preset_name(),
        before,
        "the outgoing preset stays active until the dissolve's capture frame"
    );
}
