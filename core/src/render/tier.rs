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
    /// The floor value is NFR §12 memory arithmetic. The trails accumulation is a
    /// [`PingPongField`](super::feedback::PingPongField) of `Rgba16Float`
    /// (8 bytes/texel, **two** textures), and Plan 0023's dual-live dissolve holds
    /// two whole `PostChain`s at once, so the field pair is charged twice at the
    /// peak. At 1920x1080 that is ~50 MB per chain and ~100 MB dual-live, against
    /// NFR §12's ~350 MB soft ceiling that is mostly driver floor already; at
    /// 2560x1440 it is ~88 MB and ~177 MB — which is exactly the trade ADR-0034
    /// priced and declined at floor budgets, and which the rich tier now takes.
    pub post_cap: (u32, u32),
}

impl TierConfig {
    /// The iGPU floor: the pre-tier engine's constants, unchanged.
    pub const FLOOR: Self = Self {
        tier: Tier::Floor,
        post_cap: (1920, 1080),
    };

    /// The midrange-discrete tier.
    pub const RICH: Self = Self {
        tier: Tier::Rich,
        post_cap: (2560, 1440),
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

#[cfg(test)]
mod tests {
    // Test asserts panic on failure; allowed here over the file's pragma.
    #![allow(clippy::panic)]

    use super::{Tier, TierConfig};

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
    }

    /// The floor is the pre-tier engine. These are the literals the constants
    /// carried before they moved here (ADR-0045 Context lists them with
    /// file:line), asserted so a later edit to `FLOOR` has to be a deliberate
    /// change to the floor commitment rather than a tuning that slipped in.
    #[test]
    fn the_floor_is_the_pre_tier_engine() {
        assert_eq!(TierConfig::FLOOR.post_cap, (1920, 1080));
    }
}
