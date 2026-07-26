# Spec — Ring seam + DSP determinism

> **Subsystem:** The lock-free SPSC ring buffer that decouples audio from render, and the pure-function DSP that consumes it (FFT/spectrum, onset, tempo/beat).
> **Source:** `lmv-ring/` (the SPSC ring itself, a zero-dependency workspace member), `core/src/audio.rs` (format validation + the re-exported producer/consumer handles), `core/src/dsp/` (analysis).
> **Reconciled-through:** Plan 0005 (ring extracted to `lmv-ring`, Miri gate live in CI); DSP unchanged since Plan 0003 (bass/mid/treb + deterministic tempo). Reconciled 2026-07-25.
> **Governing ADRs:** [0001](../adrs/0001-rust-core-wgpu-cabi-foobar-shim.md) (core owns DSP + the audio/render split); CLAUDE.md non-negotiables; Plan 0005 (Miri UB gate).

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
- DSP analysis (FFT bins, onset envelope, tempo/BPM estimate, bass/mid/treb bands) MUST be a
  **pure function of its input window**: no wall-clock reads, no unseeded randomness. The same
  input window MUST produce the same analysis frame. (CLAUDE.md "determinism where it's
  testable")
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

- **Nothing crosses the whole seam in a test.** Miri proves the ring's `unsafe`, and the DSP
  tests prove determinism, but no test drives PCM through `audio::intake` → the real drain policy
  → `Analyzer` → `Renderer`. The standalone's drain loop is therefore uncovered. That end-to-end
  tier is [Plan 0032](../plans/0032-testing-strategy-e2e-coverage-and-pre-push.md) Phase 1
  ([ADR-0033](../adrs/0033-testing-strategy-coverage-ratchet-and-pre-push-gate.md)) — until it
  lands, the ring-to-pixels claim above is architecture, not assertion.
- This spec does not contract the *tempo estimator's accuracy* (how close BPM is to ground
  truth) — only its **determinism**. Better tempo tracking is a named later roadmap item.
- The overflow/underrun policy is stated behaviorally here; the exact capacity (~100 ms at
  48 kHz per the ring's sizing) and drop mechanics live in `core/src/audio.rs`.
