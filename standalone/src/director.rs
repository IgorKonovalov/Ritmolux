//! The scene director (Plan 0009 Phase 3): decides when to rotate presets for a
//! hands-off live show. Auto-rotate runs on a MilkDrop-style dwell timer biased
//! toward energy *drops* — a large downward energy shift rotates sooner, a
//! steady passage holds until the max dwell — with manual hotkey overrides.
//!
//! The decision logic is a pure function of the injected `dt` (seconds since the
//! last call, measured by the shell) plus the analysis [`AnalysisFrame`] and the
//! director's own state. It reads no wall clock of its own, so it is fully
//! deterministic and unit-testable (NFR section 6): the shell owns the clock,
//! the director owns the policy.

use rlx_core::dsp::AnalysisFrame;

use crate::config;

/// Time constant (seconds) for the smoothed energy baseline. ~1.5 s means the
/// baseline follows sustained level changes but ignores per-beat spikes, so a
/// genuine section drop stands out against it.
const ENERGY_TAU: f32 = 1.5;
/// A drop fires when the current energy falls below this fraction *under* the
/// baseline (i.e. energy < baseline * (1 - DROP_FRACTION)).
const DROP_FRACTION: f32 = 0.35;
/// The baseline must exceed this before a drop can register, so near-silence
/// noise (baseline ~0) never looks like a drop.
const DROP_FLOOR: f32 = 0.05;

/// How far past the min dwell (as a fraction of the min->max span) the drop bias
/// is gated: an energy drop can only rotate early once the dwell reaches
/// `min + DROP_GATE_FRACTION * (max - min)`. This softens the drop trigger
/// (ADR-0027) so a drop shortly after a rotation can't rapid-fire another; the
/// timer and novelty triggers are unaffected. At the 20/90 default that gate is
/// ~37.5 s. Scaling to the span keeps it sensible for custom dwell configs too.
const DROP_GATE_FRACTION: f32 = 0.25;

/// Novelty score (from the core detector's ~sqrt(2)-at-a-swap scale) that earns
/// a *full* nudge — pulling the steady-passage cap all the way to the min dwell.
/// A tuning constant; the on-rig soak (Phase 6) is where it gets calibrated.
const NOVELTY_REF: f32 = 0.8;

/// Why the director decided to rotate — surfaced for the title/log and asserted
/// by the unit tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rotation {
    /// The max dwell elapsed during a steady passage.
    AutoTimer,
    /// A large downward energy shift past the min dwell.
    AutoDrop,
    /// A track-change novelty boundary pulled the cap in past the min dwell.
    AutoBoundary,
    /// The operator forced the next scene.
    Manual,
}

/// Auto-rotate state machine. Construct from config, then drive with `advance`
/// once per rendered frame; layer manual overrides with `force_next` /
/// `toggle_auto`.
#[derive(Debug, Clone)]
pub struct Director {
    /// Whether auto-rotate is currently active.
    auto: bool,
    /// Dwell clamps (seconds); `min <= max` is enforced at construction.
    min_dwell: f32,
    max_dwell: f32,
    /// Seconds accumulated (from injected `dt`) since the last rotation.
    dwell: f32,
    /// Smoothed energy baseline (EMA of bass+mid+treb); `warm` once seeded.
    baseline: f32,
    warm: bool,
    /// Whether the experimental track-change novelty nudge is active.
    track_change: bool,
}

impl Director {
    /// Build a director from the `[rotate]` config, clamping `max >= min` so a
    /// misconfigured pair can't invert the timer.
    pub fn from_config(rotate: &config::Rotate) -> Self {
        let min_dwell = rotate.min_dwell_secs as f32;
        let max_dwell = (rotate.max_dwell_secs as f32).max(min_dwell);
        Self {
            auto: rotate.auto,
            min_dwell,
            max_dwell,
            dwell: 0.0,
            baseline: 0.0,
            warm: false,
            track_change: rotate.track_change,
        }
    }

    /// Whether auto-rotate is on.
    pub fn auto_enabled(&self) -> bool {
        self.auto
    }

    /// Re-set the dwell clamps on a **running** director (Plan 0050 Phase 4's
    /// settings rows), clamping `max >= min` exactly as `from_config` does.
    ///
    /// Deliberately preserves the running dwell clock and the auto flag. Rebuilding
    /// the whole director from the edited config would be one line shorter and
    /// would reset the timer under the operator's hand — so a nudge to the max
    /// dwell would restart the countdown, which is the opposite of what nudging a
    /// timer means. A dwell already past a freshly-lowered cap simply rotates on
    /// the next `advance`, which is the correct reading of "the cap moved".
    pub fn set_dwell_bounds(&mut self, min_secs: u32, max_secs: u32) {
        self.min_dwell = min_secs as f32;
        self.max_dwell = (max_secs as f32).max(self.min_dwell);
    }

    /// Advance the timer by `dt` seconds against this frame's analysis and
    /// decide whether to rotate. Returns `Some(reason)` exactly on the frames a
    /// rotation should happen (the caller then calls `Renderer::cycle_preset`);
    /// the dwell resets internally on each rotation.
    pub fn advance(&mut self, dt: f32, frame: &AnalysisFrame) -> Option<Rotation> {
        let energy = frame.bass + frame.mid + frame.treb;

        // Compare against the *pre-update* baseline so the current (possibly
        // dropped) sample doesn't first drag the baseline down toward itself.
        let was_warm = self.warm;
        let prev_baseline = self.baseline;
        if self.warm {
            // Frame-rate-independent EMA: alpha depends on dt, not frame count.
            let alpha = 1.0 - (-dt / ENERGY_TAU).exp();
            self.baseline += (energy - self.baseline) * alpha;
        } else {
            self.baseline = energy;
            self.warm = true;
        }

        if !self.auto {
            return None;
        }

        self.dwell += dt;
        if self.dwell < self.min_dwell {
            return None;
        }
        // Novelty nudge: an experimental track-change boundary pulls the cap from
        // the max dwell toward the min dwell, so rotation lands sooner near a
        // detected change. It only shortens the wait past the min dwell, so
        // novelty is never the sole trigger (beatmatched blends have no hard
        // edge). Disabled -> nudge is zero, cap stays at the max dwell.
        let nudge = if self.track_change {
            (frame.novelty / NOVELTY_REF).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let cap = self.max_dwell - nudge * (self.max_dwell - self.min_dwell);
        if self.dwell >= cap {
            // A cap the nudge pulled in (still below the hard max) is a boundary
            // rotation; reaching the true max is the steady-passage timer.
            let reason = if self.dwell < self.max_dwell {
                Rotation::AutoBoundary
            } else {
                Rotation::AutoTimer
            };
            self.dwell = 0.0;
            return Some(reason);
        }
        // Drop bias (softened, ADR-0027): a large downward shift rotates early,
        // but only once the dwell is well past the min dwell — gated by
        // DROP_GATE_FRACTION of the min->max span — so a drop just after a
        // rotation can't rapid-fire another.
        let drop_gate = self.min_dwell + DROP_GATE_FRACTION * (self.max_dwell - self.min_dwell);
        let dropped = was_warm
            && self.dwell >= drop_gate
            && prev_baseline > DROP_FLOOR
            && energy < prev_baseline * (1.0 - DROP_FRACTION);
        if dropped {
            self.dwell = 0.0;
            return Some(Rotation::AutoDrop);
        }
        None
    }

    /// Force the next scene now (a manual hotkey): resets the dwell so the auto
    /// timer restarts from this moment. Works whether or not auto-rotate is on.
    pub fn force_next(&mut self) -> Rotation {
        self.dwell = 0.0;
        Rotation::Manual
    }

    /// Toggle auto-rotate; returns the new state. Turning it on resets the dwell
    /// so re-enabling can't trigger an immediate surprise rotation.
    pub fn toggle_auto(&mut self) -> bool {
        self.auto = !self.auto;
        if self.auto {
            self.dwell = 0.0;
        }
        self.auto
    }
}

#[cfg(test)]
mod tests;
