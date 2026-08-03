# Spec — Ring seam + DSP determinism

> **Subsystem:** The lock-free SPSC ring buffer that decouples audio from render, and the pure-function DSP that consumes it (FFT/spectrum, onset, tempo/beat).
> **Source:** `lmv-ring/` (the SPSC ring itself, a zero-dependency workspace member), `core/src/audio.rs` (format validation + the re-exported producer/consumer handles), `core/src/dsp/` (analysis).
> **Reconciled-through:** Plan 0005 (ring extracted to `lmv-ring`, Miri gate live in CI); Plan 0032 (the ring→analyzer→renderer seam now has a test); Plan 0048 (analysis v2 — the dual-resolution axis, running normalization, and the beat/downbeat clock, which is what moved the determinism invariant below from *window* to *stream*). Reconciled 2026-08-03.
> **Governing ADRs:** [0001](../adrs/0001-rust-core-wgpu-cabi-foobar-shim.md) (core owns DSP + the audio/render split); [0049](../adrs/0049-analysis-v2-dual-resolution-axis-normalized-bands.md) (normalization is analysis-layer state); [0050](../adrs/0050-downbeat-and-phrase-tracking-with-confidence-fallback.md) (the beat clock and the gated bar trio); CLAUDE.md non-negotiables; Plan 0005 (Miri UB gate).

## Invariants

- The audio thread MUST NOT block, heap-allocate, lock a contended mutex, log, or do file I/O.
  It hands samples to the ring and returns. An underrun is an audible click; a blocked callback
  is a stutter. (CLAUDE.md non-negotiables)
- The seam between audio and render MUST be the lock-free **SPSC** ring buffer — exactly one
  producer (the audio thread) and one consumer (the render thread). Neither loop is driven
  directly off the other; the ring absorbs the cadence mismatch. (CLAUDE.md)
- The ring MUST be **data-race-free** under concurrent single-producer/single-consumer access.
  It lives in `lmv-ring` — a workspace member with **no dependencies**, so Miri can interpret its
  `unsafe` without compiling the wgpu/naga graph — and the CI `miri` job proves it on every push.
  (Plan 0005, `.github/workflows/ci.yml`)
- DSP analysis (FFT bins, onset envelope, tempo/BPM estimate, the band axis, the normalized
  levels, the beat/bar clock) MUST be a **pure function of the input stream**: no wall-clock
  reads, no unseeded randomness, no ambient state. The same sequence of hops fed to a freshly
  constructed `Analyzer` MUST produce a bit-identical sequence of analysis frames. (CLAUDE.md
  "determinism where it's testable")
- **The unit of determinism is the stream, not the window** (Plan 0048 / ADR-0049 + ADR-0050).
  The spectrum, `*_raw` and BPM still resolve from their window, but `bass`/`mid`/`treb`/`onset`
  divide by a running peak, and `beat_index`/`bar_index` count, so the *same* window read at two
  points in a stream legitimately yields different frames. History-dependence is the contract
  here; ambient nondeterminism is still forbidden, and the distinction is what
  `analysis_is_deterministic` asserts by running the whole signal through two fresh analyzers.
- Any visual jitter or randomness, when wanted, MUST be **explicitly seeded** so a scene is
  reproducible from its seed. (CLAUDE.md)
- Sample rate, channel count, and buffer size MUST be validated once where audio enters the
  core; the hot DSP path downstream trusts them. (CLAUDE.md "validate at the boundary")

## Scenarios

- WHEN the audio thread receives a block of PCM frames THEN it writes them into the ring and
  returns without allocating or locking; the render thread reads them on its own cadence.
- WHEN the render thread consumes faster than the audio thread produces (ring empties) THEN the
  consumer gets "no new data" and reuses the last analysis — it does not block the producer.
- WHEN the producer outruns the consumer (ring fills) THEN the overflow policy is applied at the
  ring (oldest samples dropped) rather than blocking the audio thread.
- WHEN a fixed sine-wave window is fed to the FFT path THEN the spectrum places energy in the
  expected bin(s) deterministically — the same window always yields the same bins (the
  behavioral claim the DSP tests defend).
- WHEN the same audio window is analyzed twice THEN the onset envelope, tempo/BPM estimate, and
  band energies are bit-for-bit identical (no wall-clock, no unseeded RNG in the path).
- WHEN `cargo +nightly miri test -p lmv-ring` runs (the CI `miri` job, Plan 0005) THEN the SPSC
  ring's cross-thread test reports no undefined behavior.

## Known gaps / honest nulls

- ~~**Nothing crosses the whole seam in a test.**~~ **Closed** by
  [Plan 0032](../plans/done/0032-testing-strategy-e2e-coverage-and-pre-push.md) Phase 1
  ([ADR-0033](../adrs/0033-testing-strategy-coverage-ratchet-and-pre-push-gate.md)):
  `core/tests/chain.rs` pushes synthetic PCM into a real `audio::intake` pair in
  capture-callback-sized bursts, drains it through `pop_samples`, feeds a real `Analyzer` and
  renders — so the ring-to-pixels claim above is assertion, not architecture. What that suite
  still does **not** cover is the **standalone's own** drain loop, which is shell code outside
  `core`.
- This spec does not contract the *tempo estimator's accuracy* (how close BPM is to ground
  truth) — only its **determinism**. Better tempo tracking is a named later roadmap item.
- The overflow/underrun policy is stated behaviorally here; the exact capacity (~100 ms at
  48 kHz per the ring's sizing) and drop mechanics live in `core/src/audio.rs`.
