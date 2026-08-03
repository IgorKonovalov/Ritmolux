//! Quality tiers: the engine's capacity constants, resolved once (ADR-0045).
//!
//! NFR §1 has always specified two quality levels — a reduced tier holding 60 fps
//! at 1080p on the ~2015-iGPU baseline, and a richer presentation on capable
//! hardware — and only the first was ever built, so every machine ran the weakest
//! machine's numbers. This module is the second half.
//!
//! # What a tier is
//!
//! A [`TierConfig`] is a plain struct of **capacity** values: how many particles,
//! how many segments, how large an internal grid may get. Nothing here changes
//! *what* the engine draws, only how much of it — which is what makes
//! [`Tier::Floor`] byte-identical to the pre-tier engine and what lets captures
//! pin it (see below). A value that changes the *content* of a frame — the
//! reaction-diffusion simulation grid, whose pattern scale moves with its
//! resolution (ADR-0034) — deliberately does **not** live here.
//!
//! **That separation is a property of the consuming scene, not of this struct,
//! and one scene broke it for four plans.** The attractor draws its particles
//! with an *additive* blend into a linear accumulation, so
//! [`attractor_particles`](TierConfig::attractor_particles) set the total light in
//! the frame as directly as it set the sample count: `Rich` rendered every
//! attractor preset three stops hot, behind a green suite, because no capture
//! could pin `Rich` to see it. Fixed at its cause in Plan 0057 —
//! [`deposit_scale`](super::scenes::particles::deposit_scale) divides the deposit
//! by the count (ADR-0065), so the claim above holds again. The lesson
//! generalizes: **a count feeding an accumulating pass is a look value until
//! something normalizes it.** If a future field lands here for such a pass, that
//! normalization is part of adding it.
//!
//! # Where the numbers come from
//!
//! [`TierConfig::FLOOR`] is the pre-tier engine, constant for constant: each
//! value's former definition site now reads this struct, so no number exists
//! twice. Its justifications came with it and are on the fields.
//! [`TierConfig::RICH`] is calibrated against a midrange discrete GPU
//! (RTX 3060 / RX 6600 class) on device — Plan 0044 Phase 4 — rather than
//! asserted from a multiplier.
//!
//! # Resolution and the governor
//!
//! The tier resolves **once, at renderer construction**, from an optional pin
//! ([`RendererOptions`](super::RendererOptions)); unpinned resolves [`Tier::Rich`].
//! An unpinned renderer may then be demoted to [`Tier::Floor`] by the frame-time
//! governor — one way, once per session, never silently. A pinned tier never
//! moves.
//!
//! Headless capture is [`Tier::Floor`] **by construction**:
//! [`Renderer::new_headless`](super::Renderer::new_headless) cannot produce any
//! other tier, so every golden baseline stays byte-reproducible on the WARP
//! software adapter and the suite's cost does not scale with the rich tier.
//! [`Renderer::new_headless_tiered`](super::Renderer::new_headless_tiered) is the
//! deliberate opt-in the `shot` CLI's `--tier` reaches.
//!
//! Pure and GPU-free throughout — a tier is a set of numbers, so it is decided
//! without a device and tested without one.

// Hot-path panic-denial pragma (Plan 0002 Phase 2; render/ is scanned by the
// hygiene guard). The governor runs once per displayed frame.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

/// Which quality tier a renderer is running (ADR-0045).
///
/// Two named levels rather than a continuum: the output of a preset has to be
/// predictable enough to baseline, document, and reproduce in a bug report, which
/// a load-history-dependent feature-shedding scheme cannot deliver (ADR-0045
/// Alternative B).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Tier {
    /// The NFR §1/§2 iGPU floor — the pre-tier engine's exact constants. The
    /// default here because it is the safe answer: a `Tier` value that appeared
    /// from nowhere should not raise anyone's budgets.
    #[default]
    Floor,
    /// Calibrated for a midrange discrete GPU: higher particle, segment and
    /// resolution budgets, same visual grammar.
    Rich,
}

impl Tier {
    /// The lowercase name the CLI, the env var and the config file all use.
    pub fn as_str(self) -> &'static str {
        match self {
            Tier::Floor => "floor",
            Tier::Rich => "rich",
        }
    }

    /// The uppercase name the 5x7 diagnostics overlay paints. Separate from
    /// [`as_str`](Self::as_str) only because that font has no lowercase glyphs;
    /// both come off the same match so there is no second spelling to drift.
    pub fn label(self) -> &'static str {
        match self {
            Tier::Floor => "FLOOR",
            Tier::Rich => "RICH",
        }
    }

    /// Parse a tier name, case-insensitively. `None` for anything else — callers
    /// surface that as a usage error rather than guessing a tier.
    pub fn from_name(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "floor" => Some(Tier::Floor),
            "rich" => Some(Tier::Rich),
            _ => None,
        }
    }
}

/// The capacity values a tier sets. Resolved once at renderer construction and
/// read at construction/reconfigure time only — never branched on per frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TierConfig {
    /// Which tier these values are, so a demotion and the overlay have one thing
    /// to read rather than a parallel field to keep in step.
    pub tier: Tier,

    /// Cap on a post stage's internal grid (ADR-0034), width then height.
    ///
    /// The floor value is NFR §12 memory arithmetic, **redone for the linear-light
    /// composite** (Plan 0045 Phase 3 / ADR-0046). Every intermediate upstream of
    /// the tonemap is now [`COMPOSITE_FORMAT`](super::COMPOSITE_FORMAT) — 8
    /// bytes/texel, not 4 — so the stage offscreens that used to be charged at the
    /// surface format doubled, while the trails accumulation
    /// ([`PingPongField`](super::feedback::PingPongField), two textures) was
    /// already float and did not move.
    ///
    /// Per chain, both stages live, at this cap (1920x1080, 8 bytes/texel =
    /// 16.6 MB a texture):
    ///
    /// | buffer                     | before | after |
    /// |----------------------------|--------|-------|
    /// | trails composited          | 8.3    | 16.6  |
    /// | trails accumulation (x2)   | 33.2   | 33.2  |
    /// | kaleidoscope source        | 8.3    | 16.6  |
    /// | **per chain**              | **50** | **66** |
    ///
    /// Plan 0023's dual-live dissolve holds two whole `PostChain`s, so the peak is
    /// ~133 MB rather than ~100. Outside the chains the frame carries one more
    /// surface-sized float buffer than it used to — the tonemap's input, 16.6 MB,
    /// which is the one genuinely *new* allocation this plan adds — plus the
    /// blend's snapshot/live pair at 16.6 MB each while a dissolve runs (was 8.3),
    /// and ink's 8.3 MB input, which stays 8-bit because the tonemap hands it
    /// display-referred pixels. Worst case — dual-live, both stages, ink on —
    /// is ~191 MB against NFR §12's ~350 MB soft ceiling, which is mostly driver
    /// floor already.
    ///
    /// At the rich cap (2560x1440) the same arithmetic is ~118 MB per chain and
    /// ~236 MB dual-live, up from ~88 and ~177 — the trade ADR-0034 priced and
    /// declined at floor budgets and the rich tier takes.
    ///
    /// **This cap is the relief lever** if the float chain misses NFR §1 on a
    /// floor-tier iGPU: lower it rather than re-fixing the grids (ADR-0046), since
    /// bandwidth roughly doubled with the format and the grid policy is shared.
    ///
    /// **Bloom adds to this only when a preset switches it on** (Plan 0045
    /// Phase 4), and it is **two** allocations, not one. Its pyramid is two
    /// textures per level, each level a quarter of the last, so the pyramid
    /// converges to `2 * (1/4 + 1/16 + …) ≈ 2/3` of one grid-sized texture — ~11 MB
    /// at this cap. On top of that the stage owns its own **grid-sized `bloom-src`
    /// offscreen**, a full 16.6 MB at this cap, because a `PostStage` reads its
    /// input from a texture it owns. So the stage costs **16.6 + ~11 ≈ 28 MB** on
    /// top of the ~66 MB per chain above, and ~55 MB in the dual-live worst case —
    /// which is what NFR §12's table charges and what the ~246 MB worst case there
    /// is computed from. It is charged only against presets that bind
    /// `bloom_amount`, since an inactive stage builds nothing.
    pub post_cap: (u32, u32),

    /// How many levels deep the bloom pyramid goes
    /// ([`Bloom`](super::bloom::Bloom), ADR-0046).
    ///
    /// This is a **capacity**, not a look: each level doubles the halo's reach and
    /// costs three passes at a quarter of the previous level's area, so the tail
    /// is cheap in pixels and not free in passes. The floor runs four (a halo
    /// reaching ~16 of the grid's texels at the default radius); rich runs six,
    /// which is where the widest levels start to matter on a 1440p-class grid.
    ///
    /// [`level_sizes`](super::bloom) clamps this down on a small render target, so
    /// a value here is an upper bound rather than a promise.
    pub bloom_levels: u32,

    /// How many GPU-resident particles the attractor integrates and draws.
    ///
    /// State is ~16 bytes each, so even the rich count is well under 3 MB of
    /// storage; the real ceiling is **additive-blend fill rate**, which is why the
    /// floor value was described as the number to validate against the 60 fps @
    /// 1080p floor (ADR-0015 Risks).
    ///
    /// This is a sample count and **not** a brightness, which it was until Plan
    /// 0057: the additive draw now divides its deposit by this value
    /// ([`deposit_scale`](super::scenes::particles::deposit_scale), ADR-0065), so
    /// raising it buys a smoother figure rather than a brighter one. Changing it
    /// changes shot noise and cost; it does not change exposure.
    pub attractor_particles: u32,

    /// Upper bound on each axis of the attractor's trail accumulation grid.
    ///
    /// The floor is the ceiling Plan 0027/0029 chose for a high-DPI display while
    /// keeping the worst case bounded on the iGPU: every frame pays a decay pass
    /// plus the full additive instance draw over this grid, so cost scales with
    /// its area. Rich lifts it to 4K so a 4K or ultrawide display sizes near 1:1
    /// instead of degrading to a uniform upscale.
    pub attractor_trail_cap: (u32, u32),

    /// How many particles the swarm simulates.
    ///
    /// Plan 0043 left the floor value at an **unmeasured** iGPU cost (+0.5 ms per
    /// frame of depth math on the dev box, and not fill rate), which is why the
    /// plans index calls this a live tier candidate rather than a settled
    /// constant. If the on-device floor check misses, this is the lever — on the
    /// floor value, and that routes back through `architect`.
    pub swarm_particles: usize,

    /// Ceiling on the segment count a line scene may draw in one frame, after
    /// generation and mirror replication.
    ///
    /// The one capacity value here that a preset can *see*: past it geometry is
    /// truncated, and ADR-0007 requires that be surfaced rather than silently cut.
    /// So a preset whose mirror pushes over the floor cap reports an overflow at
    /// the floor and not at rich — the message is the tier's most visible edge for
    /// the content lane, which is why shipped presets are authored against the
    /// floor.
    pub max_segments: usize,
}

impl TierConfig {
    /// The iGPU floor: the pre-tier engine's constants, unchanged.
    pub const FLOOR: Self = Self {
        tier: Tier::Floor,
        post_cap: (1920, 1080),
        bloom_levels: 4,
        attractor_particles: 50_000,
        attractor_trail_cap: (2560, 1440),
        swarm_particles: 10_000,
        max_segments: 20_000,
    };

    /// The midrange-discrete tier.
    ///
    /// **These are provisional multipliers, not measurements.** Plan 0044 Phase 4
    /// runs the standalone pinned here on the target GPU at native fullscreen
    /// across the heaviest preset of each family and records the frame times; the
    /// values that ship are the ones that hold the display rate. A number that
    /// misses gets lowered, and no number here is invented upward to look good.
    /// Until that phase closes, treat every field below as a starting point.
    pub const RICH: Self = Self {
        tier: Tier::Rich,
        post_cap: (2560, 1440),
        bloom_levels: 6,
        attractor_particles: 150_000,
        attractor_trail_cap: (3840, 2160),
        swarm_particles: 30_000,
        max_segments: 60_000,
    };

    /// The config for `tier`.
    pub const fn for_tier(tier: Tier) -> Self {
        match tier {
            Tier::Floor => Self::FLOOR,
            Tier::Rich => Self::RICH,
        }
    }
}

impl Default for TierConfig {
    fn default() -> Self {
        Self::FLOOR
    }
}

// ---------------------------------------------------------------------------
// The frame-time governor (Plan 0044 Phase 2)
// ---------------------------------------------------------------------------

/// The display budget assumed when the frontend has not named a refresh rate —
/// 60 Hz, the rate NFR §1's floor is quoted at.
pub const DEFAULT_DISPLAY_HZ: f32 = 60.0;

/// How far past the display budget a single frame must run to count as a miss.
///
/// Not 1.0. A frame landing a hair over the budget is the ordinary condition of a
/// vsynced renderer — the measured interval *is* the refresh interval, plus
/// scheduling noise — so a bare comparison would read a perfectly healthy 60 fps
/// run as missing on half its frames. 1.25 is "missing the budget by a quarter",
/// which at 60 Hz is 20.8 ms: past it the run is visibly not holding the rate.
pub const MISS_FACTOR: f32 = 1.25;

/// What fraction of the observed frames must be misses before the governor
/// demotes. Three quarters: high enough that an intermittently-heavy passage
/// rides through, low enough that a genuine overload does not have to be
/// unanimous (a demotion is triggered by *sustained* pressure, and a real
/// overload still has fast frames in it — a cheap preset in the rotation, a
/// dissolve that ended).
pub const MISS_FRACTION: f32 = 0.75;

/// Frames of history required before the governor will demote at all.
///
/// This is the hysteresis, and it is a *count* rather than a smoothing constant
/// on purpose: it makes "a single spike must not demote" true by arithmetic
/// instead of by tuning. With 180 frames required and 75 % of them needing to
/// miss, no run of fewer than 135 consecutive bad frames can demote — so a window
/// drag, a driver hiccup, or a shader compile cannot, whatever their magnitude.
/// At 60 Hz it is also a 3 s warm-up, which keeps the pathological first frames
/// of a session (pipeline creation, first-use resource builds) out of the verdict.
///
/// **This number is only satisfiable because of a constant in another module.**
/// The only series the renderer ever hands [`sustained_miss`] is
/// [`FrameStats::samples`](crate::diag::FrameStats::samples), which yields at
/// most [`crate::diag::RING`] items — so a `MIN_SAMPLES` above the ring's
/// capacity makes the governor a permanent no-op. See the assertion below.
pub const MIN_SAMPLES: usize = 180;

/// The governor must be able to *reach* its own threshold from its real input.
///
/// A build failure rather than a test, because the failure it guards is silent:
/// nothing observable happens when the governor stops demoting — a machine that
/// cannot hold the rich budget simply stutters for the rest of the session, which
/// is the exact NFR §1 outcome ADR-0045 built the governor to prevent. A runtime
/// check could not fire (there is nothing to fire *on*), and a unit test over an
/// injected series cannot see this at all: the series in the tests below are
/// `Vec`s of any length we like, so they would keep passing while the real
/// producer had gone too short to ever trigger a demotion.
const _: () = assert!(
    MIN_SAMPLES <= crate::diag::RING,
    "the frame-time ring is shorter than the governor's minimum sample count, \
     so the governor can never demote"
);

/// Whether a frame-time series shows a **sustained** miss of the display budget,
/// which is the one condition that demotes [`Tier::Rich`] to [`Tier::Floor`]
/// (ADR-0045).
///
/// Pure and total: a function of the series and the budget with no clock, no
/// state and no allocation, so the policy is unit-testable against injected
/// series (which is how the spike-versus-overload distinction above is checked
/// rather than asserted). `frame_secs` is the rolling history in **seconds**, the
/// unit [`FrameStats::samples`](crate::diag::FrameStats::samples) yields; order
/// does not matter, only the counts.
///
/// Says `false` for a non-positive or non-finite budget, and for a series shorter
/// than [`MIN_SAMPLES`] — the safe direction, since a wrong `true` costs the user
/// the rich tier for the rest of the session and a wrong `false` costs nothing but
/// another second of measurement.
pub fn sustained_miss(frame_secs: impl Iterator<Item = f32>, budget_secs: f32) -> bool {
    if !budget_secs.is_finite() || budget_secs <= 0.0 {
        return false;
    }
    let threshold = budget_secs * MISS_FACTOR;
    let mut total = 0usize;
    let mut missed = 0usize;
    for dt in frame_secs {
        total += 1;
        if dt.is_finite() && dt > threshold {
            missed += 1;
        }
    }
    total >= MIN_SAMPLES && missed as f32 >= total as f32 * MISS_FRACTION
}

/// **The governor's whole decision**: whether to demote `tier` right now.
///
/// Pure — every input is a value, so the three properties ADR-0045 asks of the
/// governor are unit-testable together rather than one being a fact about
/// `Renderer`'s field layout: a pin never demotes, an already-demoted session
/// never demotes again (the one-way latch), and only a sustained miss demotes.
///
/// The caller owns the latch: it sets its "demoted" flag and rebuilds when this
/// says `true`, and passes that flag back in on every later frame. Keeping the
/// flag out here is what makes "exactly once" a property of the decision instead
/// of a property of the call site.
pub fn should_demote(
    tier: Tier,
    pinned: bool,
    already_demoted: bool,
    frame_secs: impl Iterator<Item = f32>,
    budget_secs: f32,
) -> bool {
    // Ordered cheapest-first: three flag reads settle the steady state, and the
    // series is only walked on a governed rich session that has not yet demoted.
    if pinned || already_demoted || tier == Tier::Floor {
        return false;
    }
    sustained_miss(frame_secs, budget_secs)
}

/// **Whether a runtime tier change is allowed at all** (ADR-0054): only on a
/// context that has a surface.
///
/// A surface-less context is exactly the headless capture path, and
/// [ADR-0045](../../../docs/adrs/0045-quality-tiers-floor-and-rich.md)'s
/// guarantee is that a capture is `Tier::Floor` **by construction** —
/// `Renderer::new_headless` takes no tier argument, so no baseline can be blessed
/// at another tier by forgetting a field. `Renderer::set_tier` is a public
/// mutator on the very type the golden suite renders through, so it is the one
/// hole that guarantee was shaped to exclude, and this predicate is what keeps it
/// closed.
///
/// Pure, and separate from `set_tier`, deliberately. A `Renderer` **with** a
/// surface cannot be constructed in CI — there is no window — so a test that only
/// observed the headless no-op would pass equally well against a `set_tier` that
/// did nothing at all. Expressed as a value-in/value-out function, both
/// directions are assertable.
pub fn tier_change_permitted(has_surface: bool) -> bool {
    has_surface
}

/// The frame budget for a display running at `hz`, in seconds. Falls back to
/// [`DEFAULT_DISPLAY_HZ`] for a value that is not a usable rate, so a frontend
/// that cannot read its monitor still gets a governed session rather than an
/// ungoverned one.
pub fn budget_secs(hz: f32) -> f32 {
    let hz = if hz.is_finite() && hz > 0.0 {
        hz
    } else {
        DEFAULT_DISPLAY_HZ
    };
    1.0 / hz
}

#[cfg(test)]
mod tests {
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
}
