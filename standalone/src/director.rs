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

use standalone::config::{self, RotateSource};
use standalone::marks::{Mark, Marks};

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

// ---------------------------------------------------------------------------
// What a rotation draws from, and in what order
// ---------------------------------------------------------------------------
//
// The director decides *when*. Everything below decides *which*, and it is a
// separate concern with separate state: the eligible set is a pure function of
// the roster and the user's marks, and the traversal over it is a pure function
// of its own history plus a seeded counter. Neither reads a clock.

/// The presets a rotation may land on, in roster order.
///
/// Hidden presets are excluded from every source — that is what the mark means.
/// `Favourites` is a **hard filter with a fallback**: it narrows to the marked
/// set, and falls back to the whole eligible set while nothing is marked,
/// because a mode that can only ever show one preset is indistinguishable from
/// a hang. A library whose every preset is hidden falls back the same way, for
/// the same reason.
pub fn eligible_names<'a>(
    names: impl Iterator<Item = &'a str>,
    marks: &Marks,
    source: RotateSource,
) -> Vec<&'a str> {
    let all: Vec<&str> = names.collect();
    let visible: Vec<&str> = all
        .iter()
        .copied()
        .filter(|name| !marks.is(Mark::Hidden, name))
        .collect();
    if visible.is_empty() {
        return all;
    }
    match source {
        RotateSource::All => visible,
        RotateSource::Favourites => {
            let favourites: Vec<&str> = visible
                .iter()
                .copied()
                .filter(|name| marks.is(Mark::Favourite, name))
                .collect();
            if favourites.is_empty() {
                visible
            } else {
                favourites
            }
        }
    }
}

/// The order rotation walks the eligible set in, and the trail of what it
/// actually showed.
///
/// **A shuffled traversal, not a remembered-history window.** Every draw is
/// taken uniformly from the eligible presets this cycle has not yet shown; when
/// none are left the cycle restarts, excluding whatever was shown last so a
/// preset never ends one cycle and begins the next. The property — *no repeat
/// while an unseen preset remains* — needs no window length to defend, and it
/// holds while the eligible set grows and shrinks between draws: a preset that
/// becomes eligible mid-cycle is unseen and joins the pool immediately, and one
/// that stops being eligible simply stops being drawn.
///
/// Deterministic from its seed: no clock, no dependency, the same bit mixer the
/// console's `random` control uses.
#[derive(Debug, Clone)]
pub struct Traversal {
    /// Eligible names already drawn this cycle.
    seen: Vec<String>,
    /// The name the last draw handed out, excluded when a cycle restarts.
    last: Option<String>,
    /// The next draw, and whether taking it restarts the cycle. Held so the
    /// console can name what a rotation will take without the answer changing
    /// between the announcement and the rotation.
    upcoming: Option<(String, bool)>,
    /// The presets actually **shown**, oldest first, with the one before the
    /// current at the end. Every switch pushes here, whatever asked for it, so
    /// "previous" means the preset that was on screen rather than an index one
    /// lower — the two stopped being the same thing when rotation stopped being
    /// sequential.
    trail: Vec<String>,
    /// lowbias32 counter state.
    rng: u32,
}

impl Traversal {
    /// A fresh traversal seeded from `seed`.
    ///
    /// Seeded rather than fixed so two machines with different libraries do not
    /// walk the same order, and injected rather than read from a clock so a
    /// test can state an exact sequence.
    pub fn new(seed: u32) -> Self {
        Self {
            seen: Vec::new(),
            last: None,
            upcoming: None,
            trail: Vec::new(),
            rng: seed,
        }
    }

    /// One step of the bit mixer — lowbias32, one round, so consecutive counter
    /// values do not produce neighbouring positions the way a bare increment
    /// would.
    fn next_rand(&mut self) -> u32 {
        let mut x = self.rng.wrapping_add(0x9E37_79B9);
        self.rng = x;
        x ^= x >> 16;
        x = x.wrapping_mul(0x21F0_AAAD);
        x ^= x >> 15;
        x = x.wrapping_mul(0x735A_2D97);
        x ^= x >> 15;
        x
    }

    /// The preset the next rotation will take, computing it if there is none
    /// cached or the cached one has stopped being eligible.
    ///
    /// Recomputed on ineligibility rather than kept, so hiding the preset that
    /// was announced as next replaces it instead of drawing it anyway.
    pub fn peek(&mut self, eligible: &[&str]) -> Option<&str> {
        let stale = match &self.upcoming {
            Some((name, _)) => !eligible.contains(&name.as_str()),
            None => true,
        };
        if stale {
            self.upcoming = self.pick(eligible);
        }
        self.upcoming.as_ref().map(|(name, _)| name.as_str())
    }

    /// The cached next preset, without computing one.
    pub fn upcoming(&self) -> Option<&str> {
        self.upcoming.as_ref().map(|(name, _)| name.as_str())
    }

    /// Take the next preset and record it as drawn.
    ///
    /// `None` only on an empty eligible set, which is an empty roster: the
    /// fallbacks in [`eligible_names`] mean no combination of marks can produce
    /// one.
    pub fn draw(&mut self, eligible: &[&str]) -> Option<String> {
        let pick = self.peek(eligible)?.to_owned();
        let (_, restarts) = self.upcoming.take()?;
        if restarts {
            self.seen.clear();
        }
        self.seen.push(pick.clone());
        self.last = Some(pick.clone());
        // Drop names that have left the library, so the cycle's memory cannot
        // outlive the set it is about.
        self.seen.retain(|name| eligible.contains(&name.as_str()));
        Some(pick)
    }

    /// Choose the next preset without recording it: an unseen eligible preset
    /// if this cycle has one, otherwise a fresh cycle excluding the preset
    /// drawn last.
    ///
    /// The `bool` is whether taking this pick restarts the cycle, carried out
    /// rather than applied here so that peeking — which a console does once a
    /// frame — never advances the traversal's own state.
    fn pick(&mut self, eligible: &[&str]) -> Option<(String, bool)> {
        let unseen: Vec<&str> = eligible
            .iter()
            .copied()
            .filter(|name| !self.seen.iter().any(|held| held == name))
            .collect();
        let (pool, restarts) = if unseen.is_empty() {
            let fresh: Vec<&str> = eligible
                .iter()
                .copied()
                .filter(|name| self.last.as_deref() != Some(*name))
                .collect();
            // A one-preset eligible set has nothing but the preset it just
            // showed, and holding it is the honest answer there.
            if fresh.is_empty() {
                (eligible.to_vec(), true)
            } else {
                (fresh, true)
            }
        } else {
            (unseen, false)
        };
        let index = (self.next_rand() as usize) % pool.len().max(1);
        pool.get(index).map(|name| ((*name).to_owned(), restarts))
    }

    /// Record `name` as having been on screen, so a step backwards can return
    /// to it.
    pub fn note_shown(&mut self, name: &str) {
        if name.is_empty() {
            return;
        }
        self.trail.push(name.to_owned());
    }

    /// The preset shown before the current one, removed from the trail so
    /// repeated steps walk genuinely backwards rather than flipping between two.
    ///
    /// `None` once the trail is spent, which is a run that has not switched yet.
    pub fn step_back(&mut self) -> Option<String> {
        self.trail.pop()
    }

    /// How many steps backwards are left — what a caller checks before offering
    /// the step at all.
    pub fn trail_len(&self) -> usize {
        self.trail.len()
    }
}

#[cfg(test)]
mod tests;
