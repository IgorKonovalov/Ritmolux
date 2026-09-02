//! The tier governor as an `impl Renderer` continuation: the two methods that
//! read the rolling frame-time history and rebuild the scene roster one tier
//! down. Precedent for the shape is `capture_api.rs`.

// Hot-path panic-denial pragma (Plan 0002 Phase 2; render/ is scanned by the
// hygiene guard).
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unreachable
)]

// A continuation of one module split across several files, so it needs the
// names `render/mod.rs` has in scope.
use super::*;

impl Renderer {
    /// Run the governor over the rolling frame-time history and demote if it says
    /// so, returning whether this call was the demotion.
    ///
    /// Nothing happens when the tier is pinned, when it is already the floor, or
    /// when the latch has already fired — so the decision is made **once per
    /// session** and the expensive part below cannot run twice.
    pub(super) fn govern_tier(&mut self) -> bool {
        if !tier::should_demote(
            self.tier.tier,
            self.tier_pinned,
            self.tier_demoted,
            self.diag.stats().samples(),
            self.frame_budget_secs,
        ) {
            return false;
        }
        self.tier_demoted = true;
        self.apply_tier(TierConfig::FLOOR);
        true
    }

    /// Rebuild the tier-dependent GPU state for `tier`.
    ///
    /// Allocation at a **reconfigure**, not on the hot path: this runs at most
    /// once in a session (the governor's latch is what guarantees that). The
    /// visible cost is one blink of the trails accumulation as the field pair is
    /// rebuilt at the smaller grid — the same blink a window resize across a grid
    /// step produces, and ADR-0045 accepts it as the price of a rare, deliberately
    /// visible event.
    ///
    /// A dissolve in flight is cancelled rather than migrated: its two sides are
    /// GPU state built at the outgoing tier, and finishing a crossfade across a
    /// tier change is a worse artifact than landing on the incoming preset.
    pub(super) fn apply_tier(&mut self, tier: TierConfig) {
        self.tier = tier;
        self.cancel_transition();
        self.incoming_side = None;
        self.side = CompositeSide::new(&self.ctx.device, COMPOSITE_FORMAT, &self.tier);
        // The scenes carry tier capacities too — particle counts, the segment
        // buffer, the trail-grid cap — and those are sized at construction, so a
        // tier change means rebuilding them. Rebuilding also resets their
        // simulation state to its seed, which is the visible half of a demotion:
        // the attractor's cloud and the swarm restart rather than losing two
        // thirds of their points mid-flight.
        self.scenes = scenes::create_all(&self.ctx.device, COMPOSITE_FORMAT, &self.tier);
        self.configure_active_scene();
    }
}
