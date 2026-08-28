# ADR-0139 — The waveform is levelled at the analyzer, and it publishes the gain it removed

> **Status:** proposed
> **Date:** 2026-08-28
> **Related plan(s):** [0127](../plans/0127-the-picture-stops-depending-on-the-volume-slider.md)

## Context

`AnalysisFrame::waveform` is the one un-normalized output this analyzer publishes. Every other
headline value — the 64-band array, `bass`/`mid`/`treb`, `onset` — goes through
[`dsp::gain`](../../core/src/dsp/gain.rs)'s running peak normalizer under
[ADR-0049](0049-analysis-v2-dual-resolution-axis-normalized-bands.md), so a threshold on them means
the same thing at any gain setting. The trace does not, and `core/src/dsp/mod.rs:112` states the
reason in as many words: *"a waveform is a picture of the signal, and normalizing it would make a
quiet passage draw the same trace as a loud one — which is the opposite of what a scope is for."*

**That reason conflates a volume knob with musical dynamics.** Measured on the development box
(Windows 10, WASAPI loopback, DX12), one `lmv.exe`, one preset (*Geiss - Blur Mix 3*,
`nWaveMode = 6`, `fWaveScale = 3.266`), one clip looping, with the Windows master volume as the only
variable: at 18 % the trace is a near-flat ribbon, at 60 % it is violently active at roughly ±40 % of
frame height with a halo filling the frame. The same preset through `shot --audio` on a file — full
digital scale, no endpoint volume anywhere — saturates. The engine's response is not weak at any
point; only the absolute level arriving from loopback is. A *slow* normalization against a recent
peak, which is what ADR-0049 already does for the bands, cancels the knob while leaving a quiet
passage genuinely quieter than a loud one. The objection in that doc comment argues against
*instantaneous* normalization, and nobody is proposing that.

**And the two frontends disagree, which is what raises this above a preference.** The foobar plugin
pulls from `visualisation_stream` (`plugin-foobar/foo_lmv.cpp:3`) — the decoded stream, **before**
the output volume. The standalone pulls **post-volume** loopback. So one core, one preset and one
track give two different pictures depending on which frontend is running, and nothing levels them.
The core stays source-agnostic as [ADR-0001](0001-rust-core-wgpu-cabi-foobar-shim.md) requires; what
leaks is the *level*, and `CLAUDE.md`'s "validate at the boundary" list — sample rate, channel count,
buffer size — does not include amplitude.

The trace has exactly one consumer today: `warp_mesh`'s `wave_mode` draw, which is the light source
of most of the MilkDrop corpus. So the blast radius of levelling it is one scene family and the
converted library — but the contract is what a future consumer will read, which is why this is a
decision and not a patch.

## Decision

We will normalize `AnalysisFrame::waveform` at the published frame boundary, against a
slowly-released running peak of the trace's own magnitude, using the same `PeakNormalizer` mechanism
and the same three properties the band scalars already have — instant attack, seconds-scale release,
and a silence floor below which the output is zero rather than amplified noise. The divisor is
published beside it as `waveform_gain`, so a consumer that genuinely wants absolute amplitude
recovers the raw trace by multiplying it back, and so nothing has to be added later to make a true
oscilloscope possible. The normalizer sits with the others on the way *out*: every internal consumer
— onset, tempo, novelty, downbeat — keeps reading exactly what it reads today.

## Consequences

### Positive

- **One core, one picture.** The same track through the plugin and through the standalone draws the
  same trace, and the OS volume slider stops being a visual parameter. The frontend asymmetry that
  `CLAUDE.md`'s boundary list does not cover is cancelled at the analyzer instead of at two shells.
- **The converted corpus becomes level-portable.** Waveform-led MilkDrop presets are most of that
  library's light; they stop reading dead at ordinary listening volume.
- **Look gates become trustworthy.** Any judgement of a waveform-led preset run at an unpinned
  volume was previously unrepeatable — Plan 0111 Phase 6 was amended for exactly this.
- **No new mechanism.** `PeakNormalizer` is built, pinned by tests, allocation-free after
  construction, and already runs four times per hop.

### Negative

- **The trace no longer shows absolute level without a multiply.** A consumer wanting a true scope
  must apply `waveform_gain`, and nothing forces it to. The escape hatch exists; using it is a
  reader's responsibility.
- **A new absolute magnitude enters the code — the waveform's silence floor — and it is the one
  place gain-portability can break.** `BAND_FLOOR` learned this at 1e-3, where a -20 dB track lost
  its mid and treble entirely; the floor here is in *amplitude* units rather than band-magnitude
  units and cannot be copied from that constant. It has to be derived with the same margin rule.
- **A quiet track reads like a loud one after the release constant has run** (~2.5 s). That is the
  deliberate loss: the knob and the mastering level are indistinguishable to this normalizer, and
  cancelling one cancels the other. Dynamics *within* a track survive; dynamics *between* tracks do
  not.
- **Four more bytes on a struct that is `Copy` and copied per frame.** `AnalysisFrame` is already
  ~2.4 kB, dominated by the 512-float trace, so the addition is noise — but it is on the hot path
  and worth naming rather than discovering.

### Neutral

- The frame stays a pure function of the *sequence* of windows rather than of one window, which is
  what the band normalizers already made it. No clock is read; determinism (NFR section 6) is
  unchanged.
- The C ABI does not move. `AnalysisFrame` never crosses it, so `LMV_ABI_VERSION` stays at 6.

## Alternatives considered

### Alternative A — Level the PCM at the boundary where audio enters the core

Conceptually the tidiest: amplitude joins sample rate, channel count and buffer size on the
validate-at-the-boundary list, both frontends deliver one vocabulary, and every downstream reading
is levelled at once. **Rejected because it moves the raw magnitudes three internal consumers are
tuned against.** `dsp::gain`'s module documentation is explicit that the onset detector, the tempo
tracker and the novelty detector read raw values on purpose — autocorrelating a peak-normalized
envelope distorts the very periodicity the tempo tracker looks for, and per-band normalization
flattens the spectral shape novelty exists to measure. An input AGC applies that distortion to all
three at once, from before the FFT. It also defeats the silence floor: `BAND_FLOOR` exists so that
the difference between a quiet room and a loud one is not a full-scale visual, and an input AGC
makes it exactly that.

### Alternative B — Condition only the converted-preset path

Leave `AnalysisFrame` alone and normalize inside `warp_mesh`'s waveform draw, where MilkDrop's own
semantics live. Smallest blast radius, and it keeps a true scope available to native scenes.
**Rejected because it leaves the contract unstated and the disagreement live**: the next consumer of
the trace inherits the volume dependence and the frontend asymmetry with nothing in the type telling
it so. A per-consumer workaround for a boundary-level defect is how a codebase ends up with three of
them.

### Alternative C — Normalize outright, with no published gain

One fewer field to explain. **Rejected for four bytes.** The raw trace is genuinely wanted by a scope
scene that does not exist yet, and re-deriving it after the fact is impossible without a second
normalizer running in parallel — a gain published beside the array costs nothing and keeps the
question open.

### Alternative D — Leave it, and document the volume dependence

The status quo with a warning. **Rejected because the two frontends still disagree**, which is not
something a doc comment can level: no operator instruction makes the plugin's pre-volume tap and the
standalone's post-volume tap draw the same picture.

## Notes

- Raised as [design-backlog 0123](../design-backlog.md) by `dev` (2026-08-19), from a live-app check
  during [Plan 0111](../plans/done/0111-the-milkdrop-import-stops-washing-out.md).
- That the endpoint volume is applied before the loopback tap is a fact about the development box's
  audio stack, not a claim about Windows in general — ADR-0071's prose rule.
- The entry it reconciles: [design-backlog 0120](../design-backlog.md) reports the converted waveform
  figure rendering *larger* than the reference's, while the live app at 18 % shows it nearly flat.
  Both are true, because an un-normalized trace times an un-normalized `wave_scale` is hypersensitive
  — blown out at full scale, dead at listening volume. Levelling the trace is what makes the missing
  base amplitude constant a question with a stable answer.
