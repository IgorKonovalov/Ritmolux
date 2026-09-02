//! [`Easing`]: the attack/release pair every binding is smoothed with
//! (ADR-0019, widened to a pair by ADR-0035).
//!
//! Render-time arithmetic rather than schema, and it lives beside the schema
//! because a preset's `[smoothing]` table is what produces it -- `raw::RawSmoothing`
//! is the on-disk form.

/// A binding's easing time constants in **seconds** (ADR-0019, widened to a pair
/// by ADR-0035).
///
/// `attack` applies while the incoming value is **above** the held one and
/// `release` while it is at or below — so a percussive parameter can reach its
/// target in a frame or two and then glide back over most of a second, which no
/// single constant expresses at any value.
///
/// The scalar `[smoothing]` form builds [`Easing::symmetric`], which is the
/// low-pass ADR-0019 shipped: with both constants equal the direction test picks
/// the same number either way, so the arithmetic is bit-for-bit unchanged.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Easing {
    /// Constant used while the raw value is **above** the held value (rising).
    pub attack: f32,
    /// Constant used while the raw value is at or **below** the held value.
    pub release: f32,
}

impl Easing {
    /// No smoothing on either side: the value is applied instantly. The default
    /// for a parameter absent from `[smoothing]`.
    pub const INSTANT: Self = Self {
        attack: 0.0,
        release: 0.0,
    };

    /// One constant in both directions — the scalar `[smoothing]` form.
    pub const fn symmetric(tau: f32) -> Self {
        Self {
            attack: tau,
            release: tau,
        }
    }

    /// One frame of the one-pole envelope: ease `held` toward `raw` over `dt`
    /// real seconds, using whichever constant the direction of travel selects.
    ///
    /// **The single implementation of this vocabulary.** The render layer's
    /// per-binding smoother and the spectrum scene's per-element smoother both
    /// call it, so "smoothing in seconds, frame-rate independent, asymmetric by
    /// direction" means exactly one thing everywhere (ADR-0019 / ADR-0035, Plan
    /// 0034 Phase 3).
    ///
    /// The direction test is against the **held** value, not the raw signal's own
    /// derivative: a value already above its new target releases toward it even
    /// while the input is still rising. That is the envelope-follower convention,
    /// and it is what keeps the behavior stable under a noisy input.
    ///
    /// A selected constant of `<= 0` (the default) or non-finite, or a
    /// non-positive `dt`, passes `raw` through unchanged. Total and
    /// allocation-free — it runs per element per frame.
    ///
    /// **A non-finite `held` or `raw` also passes `raw` through** — a snap,
    /// which is what a smoother with no valid state should do (Plan 0038
    /// Phase 9). This is not a theoretical edge: `log(0)` is `-inf` and silence
    /// produces it every time the music stops, so a `[smoothing]`-listed binding
    /// reaches this on ordinary material. Without the guard the arithmetic below
    /// is `-inf + alpha * (-inf - -inf)` = `NaN`, and `NaN` is **absorbing**
    /// here — `raw > held` is false for every `raw`, so the release branch is
    /// taken and the state stays `NaN` forever. The binding would be dead for
    /// the rest of the preset's run, recovering only on a switch.
    ///
    /// Both operands are checked because guarding `raw` alone does not fix it:
    /// a stored `-inf` against a *finite* `raw` selects `attack` and computes
    /// `-inf + inf`, which is `NaN` on the very next frame.
    pub fn step(self, held: f32, raw: f32, dt: f32) -> f32 {
        if !held.is_finite() || !raw.is_finite() {
            return raw;
        }
        let tau = if raw > held {
            self.attack
        } else {
            self.release
        };
        if tau <= 0.0 || !tau.is_finite() || dt <= 0.0 {
            return raw;
        }
        // alpha = 1 - exp(-dt/tau): the fraction of the gap closed this frame,
        // frame-rate-independent because `dt` is real elapsed time (ADR-0019).
        let alpha = 1.0 - (-dt / tau).exp();
        held + alpha * (raw - held)
    }
}

impl Default for Easing {
    fn default() -> Self {
        Self::INSTANT
    }
}
