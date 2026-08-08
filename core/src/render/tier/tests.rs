// Test asserts panic on failure; allowed here over the file's pragma.
#![allow(clippy::panic)]

use super::{Tier, TierConfig, tier_change_permitted};

/// The ADR-0054 guard, **both directions**.
///
/// The refusal alone is the half that is easy to satisfy by accident: a
/// `set_tier` whose body were empty would also never leave a headless
/// renderer at `Floor`. The permit is what says the entry point does
/// something on the path it exists for — and that path (a `Renderer` holding
/// a real surface) cannot be built in CI, which is the whole reason the
/// condition is a value here rather than a branch buried in the mutator.
#[test]
fn a_tier_change_is_permitted_only_where_there_is_a_surface() {
    assert!(
        !tier_change_permitted(false),
        "a surface-less context is the capture path — ADR-0045 pins it to Floor \
         by construction, and this is the condition that keeps that true"
    );
    assert!(
        tier_change_permitted(true),
        "a surfaced context is the live app, which is the point of ADR-0054"
    );
}

/// A name round-trips, and an unknown name is rejected rather than defaulted
/// — a typo in `LMV_TIER` must be a usage error, not a silent floor.
#[test]
fn tier_names_round_trip_and_reject_junk() {
    for tier in [Tier::Floor, Tier::Rich] {
        assert_eq!(Tier::from_name(tier.as_str()), Some(tier));
    }
    assert_eq!(Tier::from_name("RICH"), Some(Tier::Rich));
    assert_eq!(Tier::from_name("  floor "), Some(Tier::Floor));
    assert_eq!(Tier::from_name("ultra"), None);
    assert_eq!(Tier::from_name(""), None);
}

/// Every value the rich tier sets is at least the floor's, and `for_tier`
/// agrees with the constant it names. The first half is the invariant that
/// makes "Rich raises capacity, never lowers it" checkable rather than
/// stated: a calibration pass (Phase 4) that overshoots downward past the
/// floor is a mistake, not a tuning.
#[test]
fn rich_is_never_below_the_floor() {
    assert_eq!(TierConfig::for_tier(Tier::Floor), TierConfig::FLOOR);
    assert_eq!(TierConfig::for_tier(Tier::Rich), TierConfig::RICH);
    assert_eq!(TierConfig::default(), TierConfig::FLOOR);

    let (floor, rich) = (TierConfig::FLOOR, TierConfig::RICH);
    assert!(rich.post_cap.0 >= floor.post_cap.0);
    assert!(rich.post_cap.1 >= floor.post_cap.1);
    assert!(rich.bloom_levels >= floor.bloom_levels);
    assert!(rich.attractor_particles >= floor.attractor_particles);
    assert!(rich.attractor_trail_cap.0 >= floor.attractor_trail_cap.0);
    assert!(rich.attractor_trail_cap.1 >= floor.attractor_trail_cap.1);
    assert!(rich.swarm_particles >= floor.swarm_particles);
    assert!(rich.emitter_objects >= floor.emitter_objects);
    assert!(rich.max_segments >= floor.max_segments);
}

/// The floor is the pre-tier engine. These are the literals the constants
/// carried before they moved here (ADR-0045 Context lists them with
/// file:line), asserted so a later edit to `FLOOR` has to be a deliberate
/// change to the floor commitment rather than a tuning that slipped in.
#[test]
fn the_floor_is_the_pre_tier_engine() {
    let floor = TierConfig::FLOOR;
    assert_eq!(floor.post_cap, (1920, 1080)); // post.rs:88
    assert_eq!(floor.attractor_particles, 50_000); // particles/mod.rs:66
    assert_eq!(floor.attractor_trail_cap, (2560, 1440)); // particles/mod.rs:87
    assert_eq!(floor.swarm_particles, 10_000); // swarm.rs:28
    assert_eq!(floor.max_segments, 20_000); // lines/mod.rs:51
}

// -----------------------------------------------------------------------
// The governor (Plan 0044 Phase 2)
// -----------------------------------------------------------------------

use super::{DEFAULT_DISPLAY_HZ, MIN_SAMPLES, budget_secs, should_demote, sustained_miss};

/// A 60 Hz budget, and the frame times either side of its miss threshold.
const BUDGET: f32 = 1.0 / 60.0;
/// Comfortably inside the budget — a healthy vsynced frame.
const GOOD: f32 = BUDGET;
/// Well past `BUDGET * MISS_FACTOR` — a frame that genuinely missed.
const BAD: f32 = BUDGET * 2.0;

/// A series of `n` frames, `bad` of them missing, the rest healthy.
fn series(n: usize, bad: usize) -> Vec<f32> {
    (0..n).map(|i| if i < bad { BAD } else { GOOD }).collect()
}

/// **An isolated spike does not demote** — the first of Phase 2's three
/// cases. Neither one catastrophic frame nor a burst of them is enough,
/// whatever their magnitude: a 100 ms stall and a 10 s one read identically
/// here, because the verdict is a count and not an average.
#[test]
fn an_isolated_spike_does_not_demote() {
    let full = MIN_SAMPLES * 2;
    for spike in [1, 2, 10, 60] {
        assert!(
            !sustained_miss(series(full, spike).into_iter(), BUDGET),
            "{spike} bad frames out of {full} must not demote"
        );
    }
    // Magnitude is irrelevant — one frame that took a whole second is still
    // one frame. This is the window-drag / driver-hiccup case ADR-0045 names
    // as the one-way latch's main risk.
    let one_huge = (0..full).map(|i| if i == full / 2 { 1.0 } else { GOOD });
    assert!(!sustained_miss(one_huge, BUDGET));
}

/// **A sustained miss demotes** — the second case. Everything above the
/// fraction demotes; everything below it does not, so the threshold is
/// bracketed rather than only exceeded.
#[test]
fn a_sustained_miss_demotes() {
    let full = MIN_SAMPLES * 2;
    assert!(sustained_miss(series(full, full).into_iter(), BUDGET));
    // Exactly at the fraction, and one frame either side of it.
    let at = (full as f32 * super::MISS_FRACTION) as usize;
    assert!(sustained_miss(series(full, at).into_iter(), BUDGET));
    assert!(sustained_miss(series(full, at + 1).into_iter(), BUDGET));
    assert!(
        !sustained_miss(series(full, at - 1).into_iter(), BUDGET),
        "one frame below the fraction must not demote — otherwise the \
         fraction is decorative"
    );
}

/// Below [`MIN_SAMPLES`] nothing demotes, however bad the frames are. This is
/// the warm-up half of the hysteresis: a session's opening frames pay for
/// pipeline creation and first-use resource builds, and demoting on those
/// would mean a capable machine could never reach the rich tier at all.
#[test]
fn a_short_series_never_demotes_however_bad() {
    for n in [0, 1, 60, MIN_SAMPLES - 1] {
        assert!(
            !sustained_miss(series(n, n).into_iter(), BUDGET),
            "{n} all-bad frames is below the {MIN_SAMPLES}-frame minimum"
        );
    }
    // And the very next frame count does.
    assert!(sustained_miss(
        series(MIN_SAMPLES, MIN_SAMPLES).into_iter(),
        BUDGET
    ));
}

/// A healthy vsynced run does **not** demote, which is the failure mode a
/// bare `dt > budget` comparison would have: the measured interval on a
/// vsynced 60 Hz display *is* ~16.7 ms, so half those frames land a hair over
/// and a factor-free threshold would demote every capable machine in 3 s.
#[test]
fn a_healthy_vsynced_run_does_not_demote() {
    let jittery: Vec<f32> = (0..MIN_SAMPLES * 2)
        .map(|i| BUDGET * if i % 2 == 0 { 0.98 } else { 1.06 })
        .collect();
    assert!(!sustained_miss(jittery.into_iter(), BUDGET));
}

/// A degenerate budget is not a licence to demote — and a non-finite frame
/// time counts as observed but not as a miss, so a poisoned sample cannot
/// tip the verdict either way.
#[test]
fn a_degenerate_budget_or_sample_never_demotes() {
    let full = MIN_SAMPLES * 2;
    for bad_budget in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        assert!(!sustained_miss(series(full, full).into_iter(), bad_budget));
    }
    assert!(!sustained_miss(vec![f32::NAN; full].into_iter(), BUDGET));

    // The budget helper is total in the same direction: junk falls back to
    // the 60 Hz default rather than producing a degenerate budget.
    assert!((budget_secs(60.0) - BUDGET).abs() < 1e-6);
    assert!((budget_secs(144.0) - 1.0 / 144.0).abs() < 1e-6);
    for junk in [0.0, -60.0, f32::NAN, f32::INFINITY] {
        assert!(
            (budget_secs(junk) - 1.0 / DEFAULT_DISPLAY_HZ).abs() < 1e-6,
            "budget_secs({junk}) must fall back to {DEFAULT_DISPLAY_HZ} Hz"
        );
    }
}

/// **A pin never demotes** — the third of Phase 2's three cases — and it holds
/// under the worst series there is, not just a marginal one. This is the
/// escape hatch ADR-0045 names for the one-way latch's main risk: an operator
/// whose transient stall cost them the rich tier passes `--tier rich` and the
/// governor is out of the picture for good.
#[test]
fn a_pinned_tier_never_demotes() {
    let full = MIN_SAMPLES * 2;
    let all_bad = || series(full, full).into_iter();
    // The series alone *would* demote — so this is a test of the pin and not
    // of a series that was never going to fire.
    assert!(sustained_miss(all_bad(), BUDGET));
    assert!(!should_demote(Tier::Rich, true, false, all_bad(), BUDGET));
    assert!(!should_demote(Tier::Floor, true, false, all_bad(), BUDGET));
}

/// **A sustained miss demotes exactly once** — the one-way latch. The second
/// call sees the same catastrophic series and says no, because the caller has
/// recorded the first demotion; and a tier already at the floor never demotes,
/// which is what keeps the rebuild from firing on an iGPU every four seconds.
#[test]
fn the_demotion_is_one_way_and_happens_once() {
    let full = MIN_SAMPLES * 2;
    let all_bad = || series(full, full).into_iter();

    // Frame N: governed, rich, not yet demoted, sustained miss -> demote.
    assert!(should_demote(Tier::Rich, false, false, all_bad(), BUDGET));
    // Frame N+1 onward: the caller latched, so nothing fires again however
    // bad the frames stay. No oscillation design, by construction.
    assert!(!should_demote(Tier::Floor, false, true, all_bad(), BUDGET));
    // And the latch alone is enough — even if the tier somehow read rich.
    assert!(!should_demote(Tier::Rich, false, true, all_bad(), BUDGET));
    // A floor session is never a demotion candidate, latch or not. This is
    // the case that matters for cost: the iGPU the floor exists for will miss
    // its budget sometimes, and it must not pay a GPU rebuild for it.
    assert!(!should_demote(Tier::Floor, false, false, all_bad(), BUDGET));
}

/// A healthy governed rich session stays rich — the decision's negative case
/// at the level the renderer actually calls it.
#[test]
fn a_healthy_governed_session_stays_rich() {
    let steady = || vec![GOOD; MIN_SAMPLES * 2].into_iter();
    assert!(!should_demote(Tier::Rich, false, false, steady(), BUDGET));
}

/// **The governor can actually fire on the series the renderer feeds it.**
///
/// Every other test in this module builds its own `Vec` of `MIN_SAMPLES * 2`
/// frames — 1.5x the whole capacity of the ring the renderer reads from, and
/// therefore a series the production call site cannot produce. So they pin the
/// *policy* and say nothing about the *wiring*: had `MIN_SAMPLES` been written
/// as 300, all of them would still pass and the governor would be dead on
/// arrival, with no symptom but a machine stuttering instead of demoting.
///
/// This one drives the real producer through its real API — `FrameStats`,
/// `record`, `samples` — exactly as `Renderer::govern_tier` does. The
/// `assert!` on the sample count is the load-bearing line: it is what fails if
/// the ring is ever shortened past the threshold. (The compile-time assertion
/// beside `MIN_SAMPLES` catches that first; this is the behavioral half, and
/// it also covers the case where `samples` stops yielding a full ring for some
/// reason unrelated to its capacity.)
#[test]
fn a_real_frame_stats_ring_can_reach_the_governors_threshold() {
    use crate::diag::FrameStats;

    // Fill the ring the way the renderer does: one `record` per rendered
    // frame, every one of them missing the budget.
    let mut stats = FrameStats::new();
    for _ in 0..crate::diag::RING {
        stats.record(BAD);
    }

    assert!(
        stats.samples().count() >= MIN_SAMPLES,
        "the frame-time ring yields {} samples, below the governor's {MIN_SAMPLES}-frame \
         minimum — the governor can never demote",
        stats.samples().count()
    );
    assert!(
        sustained_miss(stats.samples(), BUDGET),
        "a full ring of missing frames must demote"
    );
    // And the same producer, healthy, does not — so the assertion above is
    // reading the frame times and not merely the sample count.
    let mut healthy = FrameStats::new();
    for _ in 0..crate::diag::RING {
        healthy.record(GOOD);
    }
    assert!(!sustained_miss(healthy.samples(), BUDGET));

    // The decision at the level the renderer calls it, on the real series.
    assert!(should_demote(
        Tier::Rich,
        false,
        false,
        stats.samples(),
        BUDGET
    ));
}

/// A faster display is a tighter budget: frames that pass at 60 Hz miss at
/// 144. The governor reads the *display's* rate, not a hardcoded 60.
#[test]
fn the_budget_follows_the_display_rate() {
    let steady: Vec<f32> = vec![BUDGET; MIN_SAMPLES * 2];
    assert!(!sustained_miss(steady.iter().copied(), budget_secs(60.0)));
    assert!(
        sustained_miss(steady.iter().copied(), budget_secs(144.0)),
        "16.7 ms frames are a sustained miss of a 144 Hz budget"
    );
}
