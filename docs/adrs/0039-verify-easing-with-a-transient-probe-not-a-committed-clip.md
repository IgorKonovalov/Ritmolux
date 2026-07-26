# 0039 — Verify easing with a deterministic transient probe, not a committed audio clip

> **Status:** proposed
> **Date:** 2026-07-26
> **Related:** [0035](0035-asymmetric-attack-release-easing.md) (the capability this makes checkable),
> [0019](0019-eased-parameters.md) (the `[smoothing]` surface),
> [0016](0016-warp-software-adapter-ci.md) (the determinism posture the harness holds to);
> implemented by [Plan 0037](../plans/0037-verifying-easing-transient-probe-and-dynamic-signal.md);
> closes design-backlog 0013, the unresolved half of 0008

## Context

ADR-0035 added `{ attack, release }` to `[smoothing]`. Its entire value is in the **transient** — a
parameter that snaps up and glides down. The `shot` harness cannot observe a transient, so the
capability shipped and was adopted across 20 presets with no automated check of any kind. Two
independent mechanisms cause this, both measured on 2026-07-26:

**The capture primitive holds one stimulus.** `Renderer::capture_preset(name, frame, frames)`
(`core/src/render/mod.rs:1397`) takes a single `&AnalysisFrame` and renders `frames` of it. Every
smoother therefore converges before the capture is read, so the output is the *settled* value and is
identical for any easing constant. `--report` is built entirely on this call, which is why
`fragment_kaleido` reported bass 0.228 / mid 0.153 / treb 0.131 both before and after a complete
smoothing rework — not a coincidence, an identity.

**No synthesized signal has dynamics.** Every `lmv_core::signal` generator is a steady tone or steady
noise. Measured through the band report Plan 0033 Phase 1 added: `bass:60` gives min/mean/max
0.187 / 0.187 / 0.187 — zero variance; `chord` 0.058 / 0.059 / 0.060; `noise:7`
0.012 / 0.022 / 0.039. `click_track` is the only generator with real transients and it peaks at
bass **0.011**, far below the range shipped presets are gained for.

The cost is already concrete. The `preset-author` lane could not separate five different `thickness`
values on `rose_trails` (1.10 through 2.30, including the untouched original) because that preset's
1.25 spin against a max-decay feedback saturates any held stimulus regardless of the value; it
shipped a mid-range guess rather than the value it wanted. Every easing edit in `a070f5a`, `8b5b2e0`
and `66300d6` rests solely on a human watching the running app.

Two different problems hide inside this, and conflating them is what kept it unsolved:

- **Does the easing behave?** — needs a *controlled* stimulus and a measurement of the response.
- **Are preset gains calibrated for real material?** — needs a *realistic* stimulus.

## Decision

We verify easing with a **deterministic transient probe**: the capture path gains the ability to
drive a time-varying stimulus, and `--report` gains a measurement of the response to a step — how
fast the frame reaches its new steady state on the way up, and on the way down. Asymmetry becomes
directly observable as the ratio between the two, which is a property of the capability itself rather
than an approximation of real input.

Alongside, `lmv_core::signal` gains **one synthesized generator with musical dynamics** — an
envelope-shaped, beat-gridded signal rather than a steady tone — so a filmstrip exercises the DSP
with material that rises and falls. It is synthesized, so it stays a pure function of its arguments
and adds no bytes to the repository.

We do **not** commit an audio clip. The calibration question it would answer is instead closed by a
`human` phase: the user runs `--audio` against a file of their own choosing and the measured band
levels are recorded in `docs/capturing.md` as the reference numbers authors calibrate against. The
numbers are what the lane needs; the bytes are not.

## Consequences

**Positive.**

- Easing becomes checkable by the mechanism that makes it valuable, not by proxy. A scalar
  `[smoothing]` entry and an `{ attack, release }` entry produce measurably different probe results.
- Determinism (NFR §6) is preserved end to end — a step function and a synthesized envelope are both
  pure functions of their arguments, so captures stay reproducible and WARP-stable.
- No repository weight and no licensing question.
- The core capture API becoming able to express a time-varying stimulus is reusable: any future
  behaviour that is about *change over time* rather than steady state can be probed the same way.

**Negative, and these are the price.**

- **`capture_preset`'s signature or its neighbourhood has to change**, which is a public API on
  `Renderer` that `shot` and the behavioural tests both consume. This is a testing-driven change to a
  production surface — justified because the alternative is a capability nobody can verify, but it is
  a real widening.
- **A synthesized "musical" signal is only as musical as we make it.** It will exercise dynamics, but
  it is not evidence about real loopback levels; only the `human` phase's `--audio` measurement is.
  Anyone reading a green probe result must not conclude their gains are calibrated.
- **The probe measures the frame, not the parameter.** A preset whose easing changes but whose visual
  response saturates (`rose_trails`) will still read flat. The probe is a floor on observability, not
  a guarantee of it — and that limitation should be documented rather than discovered.
- One more `--report` column to explain, on a table the lane already reads carefully.

## Alternatives considered

**A short committed reference clip driving `--audio`.** The most realistic stimulus available, and
`--audio` already reads 16-bit PCM WAV so it needs no new code at all. Rejected on two counts, either
sufficient: it puts a binary in a repository whose stated value is "lightweight is a feature" (every
clone pays for it forever), and it needs a licensing answer for the material that nobody has. The
`human` phase captures the *measurements* a clip would have provided without carrying the clip.

**A synthesized dynamic signal alone, with no probe.** Cheaper, and it would have made the filmstrip
more useful. Rejected because it does not actually close the gap: a filmstrip is still judged by a
human looking at tiles, so easing remains unverified by anything automated, and the settled-response
identity in `--report` is untouched. It treats the stimulus as the problem when the measurement is
equally the problem.

**A CI gate on transient response.** Considered and deliberately rejected for now. A floor every
embedded preset must clear would catch regressions automatically, but transient response is not a
property with a fair universal floor — a deliberately slow ambient preset legitimately has a slow
rise, and `animation.rs` already demonstrates this failure mode (design-backlog 0009, where the
gate's *resolution* rather than the look decides the outcome). The number ships as a column the
content lane reads; a gate can follow once there is evidence about what range real presets occupy.

**Making `--report`'s existing columns transient-aware instead of adding a probe.** Rejected because
the existing columns answer a different and still-useful question — settled reactivity per band is
exactly right for "does this preset respond to bass at all". Overloading them would lose that.
