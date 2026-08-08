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
