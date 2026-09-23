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

use standalone::config::{self, RotateOrder, RotateSource};
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

    /// Seconds left before the steady-passage cap rotates, for the HUD.
    ///
    /// **The hard cap, not this frame's nudged one.** The nudge is a function of
    /// the audio arriving now, so a countdown taken from it would jump around
    /// under the operator's eye and still be wrong on the next frame; a drop or
    /// a track boundary can land the change sooner than this says, which is the
    /// honest reading of "by then at the latest". `None` while auto-rotate is
    /// off, because there is then no rotation to count down to.
    pub fn remaining_secs(&self) -> Option<f32> {
        self.auto.then(|| (self.max_dwell - self.dwell).max(0.0))
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

/// Which order rotation walks the eligible set in.
///
/// An enum rather than a flag on [`Traversal`]: the cycle state and the mixer
/// below belong to the shuffle alone, and a flag would leave both unread under a
/// sequential walk while the type's documented no-repeat invariant held in only
/// one of its two modes.
#[derive(Debug, Clone)]
pub enum Order {
    /// **A shuffled traversal, not a remembered-history window.** Every draw is
    /// taken uniformly from the eligible presets this cycle has not yet shown;
    /// when none are left the cycle restarts, excluding whatever was shown last
    /// so a preset never ends one cycle and begins the next. The property — *no
    /// repeat while an unseen preset remains* — needs no window length to
    /// defend, and it holds while the eligible set grows and shrinks between
    /// draws: a preset that becomes eligible mid-cycle is unseen and joins the
    /// pool immediately, and one that stops being eligible simply stops being
    /// drawn.
    ///
    /// Deterministic from its seed: no clock, no dependency, the same bit mixer
    /// the console's `random` control uses.
    Shuffled {
        /// Eligible names already drawn this cycle.
        seen: Vec<String>,
        /// lowbias32 counter state.
        rng: u32,
    },
    /// The eligible set walked in ascending name order, wrapping at the end.
    ///
    /// Carries no state of its own: the successor is computed against the set
    /// handed to *that* draw, so a mark toggled mid-walk changes what comes next
    /// without the walk losing its place — the preset after the last one drawn
    /// is still the answer whether or not that one is still eligible.
    Sequential,
}

impl Order {
    /// A shuffle with an empty cycle, seeded from `seed`.
    pub fn shuffled(seed: u32) -> Self {
        Order::Shuffled {
            seen: Vec::new(),
            rng: seed,
        }
    }
}

/// One step of the bit mixer — lowbias32, one round, so consecutive counter
/// values do not produce neighbouring positions the way a bare increment would.
fn next_rand(rng: &mut u32) -> u32 {
    let mut x = rng.wrapping_add(0x9E37_79B9);
    *rng = x;
    x ^= x >> 16;
    x = x.wrapping_mul(0x21F0_AAAD);
    x ^= x >> 15;
    x = x.wrapping_mul(0x735A_2D97);
    x ^= x >> 15;
    x
}

/// What rotation draws, and the trail of what it actually showed.
///
/// The draw itself belongs to the [`Order`]; everything here is shared between
/// the two, which is what keeps `Backspace` and the console's staging line with
/// one maintainer rather than one per order.
#[derive(Debug, Clone)]
pub struct Traversal {
    /// Which order the next draw comes from.
    order: Order,
    /// The name the last draw handed out — excluded when a shuffle's cycle
    /// restarts, and the anchor a sequential walk takes the successor of.
    last: Option<String>,
    /// The next draw, and whether taking it restarts the cycle. Held so the
    /// console can name what a rotation will take without the answer changing
    /// between the announcement and the rotation.
    upcoming: Option<(String, bool)>,
    /// The presets actually **shown**, oldest first, with the one before the
    /// current at the end. Every switch pushes here, whatever asked for it, so
    /// "previous" means the preset that was on screen rather than an index one
    /// lower — the two stopped being the same thing when rotation stopped being
    /// the roster's successor.
    trail: Vec<String>,
}

impl Traversal {
    /// A fresh shuffled traversal seeded from `seed`.
    ///
    /// Injected rather than read from a clock, so a test can state an exact
    /// sequence and a run is reproducible from its seed alone. One seed is one
    /// order: a caller that passes the same number every launch gets the same
    /// walk every launch.
    pub fn new(seed: u32) -> Self {
        Self::with_order(Order::shuffled(seed))
    }

    /// A fresh traversal walking the eligible set in ascending name order.
    pub fn new_sequential() -> Self {
        Self::with_order(Order::Sequential)
    }

    /// A fresh traversal in the order `[rotate] order` names, seeded from
    /// `seed`. `seed` is ignored by [`Order::Sequential`], which needs none.
    pub fn for_order(order: RotateOrder, seed: u32) -> Self {
        match order {
            RotateOrder::Shuffled => Self::new(seed),
            RotateOrder::Sequential => Self::new_sequential(),
        }
    }

    /// Switch the order a **running** traversal draws in, the way
    /// [`Director::set_dwell_bounds`] switches a running dwell.
    ///
    /// Keeps `trail` and `last`, so `Backspace` still walks what was shown and
    /// a sequential walk picked up mid-show continues from the preset on screen
    /// rather than from the top of the library. Drops the announced `upcoming`,
    /// because the order that chose it is no longer the one drawing — the next
    /// peek names what the new order will actually take. A call naming the order
    /// already running is a no-op, so a surface restating it cannot restart a
    /// shuffle's cycle.
    #[allow(
        dead_code,
        reason = "the live order switch is the settings row's and the hotkey's seam"
    )]
    pub fn set_order(&mut self, order: RotateOrder, seed: u32) {
        let unchanged = matches!(
            (&self.order, order),
            (Order::Shuffled { .. }, RotateOrder::Shuffled)
                | (Order::Sequential, RotateOrder::Sequential)
        );
        if unchanged {
            return;
        }
        self.order = match order {
            RotateOrder::Shuffled => Order::shuffled(seed),
            RotateOrder::Sequential => Order::Sequential,
        };
        self.upcoming = None;
    }

    fn with_order(order: Order) -> Self {
        Self {
            order,
            last: None,
            upcoming: None,
            trail: Vec::new(),
        }
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
        if let Order::Shuffled { seen, .. } = &mut self.order {
            if restarts {
                seen.clear();
            }
            seen.push(pick.clone());
            // Drop names that have left the library, so the cycle's memory
            // cannot outlive the set it is about.
            seen.retain(|name| eligible.contains(&name.as_str()));
        }
        self.last = Some(pick.clone());
        Some(pick)
    }

    /// Choose the next preset without recording it, in whichever order is
    /// running.
    ///
    /// The `bool` is whether taking this pick restarts a shuffle's cycle,
    /// carried out rather than applied here so that peeking — which a console
    /// does once a frame — never advances the traversal's own state. A
    /// sequential walk has no cycle to restart and always answers `false`.
    fn pick(&mut self, eligible: &[&str]) -> Option<(String, bool)> {
        let Self { order, last, .. } = self;
        let last = last.as_deref();
        match order {
            Order::Shuffled { seen, rng } => {
                let unseen: Vec<&str> = eligible
                    .iter()
                    .copied()
                    .filter(|name| !seen.iter().any(|held| held == name))
                    .collect();
                let (pool, restarts) = if unseen.is_empty() {
                    let fresh: Vec<&str> = eligible
                        .iter()
                        .copied()
                        .filter(|name| last != Some(*name))
                        .collect();
                    // A one-preset eligible set has nothing but the preset it
                    // just showed, and holding it is the honest answer there.
                    if fresh.is_empty() {
                        (eligible.to_vec(), true)
                    } else {
                        (fresh, true)
                    }
                } else {
                    (unseen, false)
                };
                let index = (next_rand(rng) as usize) % pool.len().max(1);
                pool.get(index).map(|name| ((*name).to_owned(), restarts))
            }
            Order::Sequential => {
                // Sorted here, against the set this draw was handed, rather
                // than cached: a mark toggled mid-walk moves the successor and
                // must not leave the walk pointing at a name that is gone.
                let mut sorted: Vec<&str> = eligible.to_vec();
                sorted.sort_unstable();
                // Strictly greater, so the anchor need not still be eligible:
                // the first name past it is the successor either way. Wrapping
                // to the first is what closes the lap.
                last.and_then(|last| sorted.iter().copied().find(|name| *name > last))
                    .or_else(|| sorted.first().copied())
                    .map(|name| (name.to_owned(), false))
            }
        }
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
