# ADR-0049 — Analysis v2: a dual-resolution low end makes the band axis truly logarithmic, and `bass`/`mid`/`treb` become normalized (with `*_raw` escapes)

> **Status:** proposed
> **Date:** 2026-07-30
> **Related plan(s):** 0048-analysis-v2-and-the-retune (R5 of [docs/roadmap-visual-richness.md](../roadmap-visual-richness.md))
> **Supersedes in part:** the axis description in [ADR-0036's Outcome](0036-preset-reachable-spectrum.md); **resolves:** design-backlog 0015 and the structural half of 0020.

## Context

Two defects in the analysis surface each cost the library months of dead or feeble content.

**The axis** (backlog 0015, user-confirmed with an 808 test): the 2048-sample window's
one-bin floor is 23.4 Hz at 48 kHz, which binds from band 1 to band 30 — so 31 of 64
"log-spaced" bands are linear slices, band 0 spans a full octave, the kick-and-sub region
collapses into one or two elements on every hit, and the linear half's meaning moves with
the sample rate.

**The gain** (backlog 0020): raw band means on real music sit at 0.006–0.040 while the
authoring stimuli reached 0.187–0.8, so the shipped library was gained 6–100x hot and nine
headline mechanisms never ran. Plans 0041/0042 fixed the *instrument*; nothing yet fixes the
*cause* — every threshold is a magic number against a measured table that itself changed
three times in one week.

The 2026-07-30 interview chose the aggressive form of both fixes: a dual-resolution low end
(over a single 4096 window and over leaving it documented), and **replacing** the semantics
of `bass`/`mid`/`treb` with normalized values (over parallel `*_n` variables and over a
stateful `norm()` grammar function). Replacement is breaking for effectively the whole
library; the user accepted a one-time full retune as the price of a clean end state.

## Decision

We will run **two analysis windows**: the existing 2048 window keeps feeding the bands above
the crossover, and a longer window (4096 or 8192, chosen in-plan by measurement) feeds only
the low bands — so the 64 band edges are laid out truly logarithmically across 35 Hz–18 kHz
with every band at least one *achievable* bin wide, and the axis stops depending on sample
rate in its bottom half. The low bands inherit the longer window's slower time response;
that is physics, stated in the docs, not compensated away.

We will make **`bass`, `mid`, `treb` normalized**: each scalar is its raw band mean divided
by its own slowly-decaying running peak (deterministic function of input history, the tempo
tracker's class), with a silence floor so quiet rooms do not amplify noise, instant attack
and a slow release measured in seconds — properties pinned by test, constants tuned against
real material in the plan's `human` phase. The raw values remain reachable as `bass_raw`,
`mid_raw`, `treb_raw` for absolute-level uses and for harness continuity. `onset` is
normalized the same way (it is today raw spectral flux with peak ~0.016, the single most
mis-thresholded variable in the library). After this, `> 0.5` means "loud relative to this
track's recent past" — the thing every author has been trying to write all along.

One library-wide retune follows in the same plan, once, after both changes land.

## Consequences

### Positive
- The most-reached-for octaves become the best-resolved instead of the worst; `bin()` and
  the spectrum scenes finally show the kick and the bass line as structure.
- Thresholds become portable across tracks, gain staging, and stimulus levels — the whole
  class of "dead gate" defects (nine found by audit, five invisible to the checker) stops
  being writable.
- The `--report` calibration ladder stops being load-bearing folklore; its tables shrink to
  documentation of shape, not of magic numbers.

### Negative
- **Breaking, library-wide, on purpose.** Every band threshold and gain in all shipped
  presets changes meaning; the eight `bin()` presets also shift positions. One retune pass,
  verified through `--report`'s two-level columns and reachability flags, is the paid price.
- A second FFT per hop on the hot path — CPU cost measured in-plan against NFR §3's budget,
  not assumed.
- Normalized values hide absolute dynamics: a quiet track and a loud one read alike. That is
  the point, but looks that *should* scale with absolute level must use `*_raw` — an
  authoring nuance the docs must carry.
- AGC state is analysis-layer state: capture reproducibility now depends on the stimulus
  history within a run (it already did for tempo/novelty; the surface grows).

## Alternatives considered

### Alternative A — one longer window for everything
Halves the linear span in one move, but slows every band's time response and smears
transients across the whole spectrum; NFR §3 would need re-arguing globally instead of only
for the bands where slowness is inherent anyway. Rejected for collateral damage to the
mid/high transient feel.

### Alternative B — leave the axis documented
Free, and the tables exist — but the user-confirmed complaint stands and the spectrum
family stays flat exactly where the music lives. Rejected by mandate.

### Alternative C — parallel `bass_n`/`mid_n`/`treb_n` variables (the interview's recommended option)
Non-breaking, migrate at leisure — and recorded here as the road not taken: it leaves two
level systems alive forever, every future preset chooses wrongly half the time, and the
calibration-table folklore survives. The user explicitly priced the retune and chose the
clean end state.

### Alternative D — a stateful `norm(x)` in the grammar
Most general, but running state inside the expression layer breaks the purity invariant
ADR-0019 and ADR-0035 each defended; the analysis layer is where input-history state lives.

## Notes

The retune is sequenced *after* both semantic changes so it happens once — the lesson Plans
0041→0042 paid for ("fix the measurement, then do the content once instead of twice").
