# ADR-0199 — A converted waveform draws the released source's figure at the scale its host renders, from a stereo pair the analyzer already receives

> **Status:** proposed
> **Date:** 2026-09-14
> **Related plan(s):** [0180](../plans/0180-the-converted-picture-follows-the-source.md)
> **Supplements:** [0113](0113-milkdrop-presets-are-translated-ahead-of-time-onto-a-warp-mesh-idiom.md)
> (MilkDrop presets are translated ahead of time, with `foo_vis_milk2` as the fidelity reference)
> **Extends:** [0139](0139-the-waveform-is-levelled-at-the-analyzer-and-publishes-its-gain.md)
> (the waveform is levelled at the analyzer and publishes its gain)

## Context

`warp_mesh`'s built-in waveform is the light source of most of the converted MilkDrop corpus, and
since Plan 0173 read MilkDrop 2's released source (`xeiraex/milkdrop2` at `d4c843a`, v2.25c) it is
known to follow **neither** of the two references this project has. Backlog 0216 carries the table.
In short:

- **Modes 0-5 draw other figures.** `CPlugin::DrawWave` draws mode 0 as a turning circle of radius
  `0.5 + 0.4 * fR[i] + wave_mystery` clip units, modes 1-3 as x-y oscilloscopes plotting one channel
  against the other 32 samples later, mode 4 as a horizontal "script" and mode 5 as a rotating
  product figure. `core/src/render/scenes/warp_mesh/draw.rs` draws a non-turning circle, two rings,
  a horizontal line, a vertical line, a lagged self-Lissajous and a mirrored line. Its comments
  describe these as the reference's figures, which is ADR-0071's prose error: a claim about a
  reference that nobody had read.
- **Modes 6-7 are the same figure at three different scales.** Per unit sample the released source
  offsets by `0.125` frame heights, this engine by `0.15`, and `foo_vis_milk2` 0.2.0.0 drew `0.158`
  on Plan 0127's capture (`0.316` peak-to-peak from a full-scale sine at `fWaveScale = 1`). The mode-6/7
  angle range (`1.57 * wave_mystery` against `pi * wave_mystery`) and the mode-7 separation
  (`(wave_y*0.5+0.5)^2` clip units against a fixed `0.03`) differ as well.

So the question is which reference is the contract, and it is a real one. ADR-0113 names
`foo_vis_milk2` as the thing a converted preset is judged against side by side. That host is what
people run. But only one of its modes has ever been measured, and measuring the other seven means a
rig session per mode. The source settles every figure without a rig, but not the scale the host
actually renders at. Plan 0173 found where the gap is: the host does not feed samples through the
Winamp 8-bit path (`CPluginShell::AnalyzeNewSound`, `pluginshell.cpp` l.2018) that the source scales
from, so the level a trace arrives at is not in the source.

**Modes 1-3 and 7 need a second channel, and the analysis is mono.** `Analyzer::push_interleaved`
(`core/src/dsp/mod.rs`) averages the channels of every frame before anything reads them, and
`MilkRuntime::run_wave_point` (`core/src/milk/mod.rs`) documents the loss. What decides whether a
stereo trace is affordable is **where the second channel would have to come from.** The interleaved
frames already reach the analyzer:

- `rlx_push_samples` takes interleaved floats, and `rlx_create` validates a channel count in
  `[1, 8]` (`core-cabi/include/rlx_core.h`).
- The ring carries those frames unchanged (spec 0002).
- `AnalysisFrame` is not part of the C ABI surface.

So a stereo pair is an analyzer change, not a boundary change.

## Decision

We will draw each converted `wave_mode` as **the figure the released source's `DrawWave` builds**,
scale **the sample term** of every mode by **one host factor** fitted to `foo_vis_milk2`, and feed
the two-channel modes from **a levelled left/right pair the analyzer publishes beside the unchanged
mono trace**.

1. **The figure is the source's.** For each mode, the construction at `d4c843a` is the contract: the
   base geometry, which channel each point reads and at what offset, the angle and separation terms,
   the mode-0 turn at `time * 0.2`, and `SmoothWave`'s midpoint kernel. Comments in `draw.rs` cite
   the source by file, function and line, and never "the reference" or "MilkDrop" (ADR-0071).
   Plan 0109 removed the `time` term from modes 6 and 7 because the source has none there. That
   removal stands, and mode 0 gains the turn because the source has one.
2. **The scale is the host's, through one factor.** Let `k` be the ratio of the host's per-unit-sample
   offset to the source's, taken once from Plan 0127's mode-6 capture:
   `k = ((0.316 - w) / 2) / 0.125`. Here `w` is the stroke-width term, which that plan's reading of
   this engine implies is `0.3019 - 0.30 = 0.0019` frame heights. That gives `k ≈ 1.256`. `k`
   multiplies **only the term that scales with the sample**, in every mode, and never the base
   geometry (mode 0's resting radius, mode 7's separation, the scope's extent). The reason is where
   the gap lives: the source and host diverge in how a sample reaches the draw, and every mode shares
   that path. `k` is a measurement, so it is recorded as a named constant with its capture: host,
   version, mode, stimulus and date.
3. **The two channels come from the analyzer, if and only if nothing below it moves.**
   - `AnalysisFrame` gains a left/right pair of `WAVE_SAMPLES` traces, taken from channels 0 and 1
     of the interleaved frames. A one-channel stream gives both slots channel 0.
   - The pair is levelled by **one** running peak tracked over both channels, so their ratio (the
     x-y figure's shape) survives. That divisor is published beside the pair, in ADR-0139's shape.
   - The mono `waveform` and `waveform_gain` are unchanged, bit for bit, for every existing consumer.
   - Spec 0002's determinism list gains the pair.

   **This clause carries a hard stop.** If the pair turns out to need any change to the C ABI, to
   the ring's contract, or to work on the audio thread, it is not built. Modes 1-3 and 7 then draw
   the source's figure from the mono trace in both slots, and this ADR's `Outcome` records the stop.
   The stand-in still draws a figure: modes 1-3 plot a sample against the one 32 samples later, which
   traces a loop rather than a line.

## Consequences

### Positive

- **The figure question is settled without a rig.** Every mode's shape comes from a read of one
  public commit, and the one number the source cannot give comes from a capture that already exists.
- **Mode 6 lands where the host draws it.** `0.125 * k ≈ 0.157` frame heights per unit sample, which
  is the `~0.157` Plan 0127 derived, reached by stating the source's constant and the host's gap
  separately rather than by fitting one mode. Plan 0127's top-decile objection is answered by the
  same read: the source does not clamp at the frame edge either.
- **The custom waves stop drawing a diagonal.** `run_wave_point`'s `value1`/`value2` become two
  channels, so a custom wave that plots one against the other draws a figure instead of the line its
  doc comment apologises for.
- **The boundary is untouched.** No C ABI function, ring invariant or audio-callback line changes.
  What the core already received is simply read instead of averaged away.

### Negative

- **`k` rests on one capture of one mode on one host version.** That is `foo_vis_milk2` 0.2.0.0,
  mode 6, a full-scale 200 Hz sine, at the level Plan 0127 verified. That modes 0-5 share the same
  gap is an inference from where the gap lives, not a measurement. Plan 0142's Phase 4 rig session
  is asked to capture mode 0 at unit scale as a confirmation. Until one lands, this ADR says so.
  The stroke-width term `w` assumes the host's stroke is as wide as ours. It is not measured, and it
  moves `k` by under 1 % (`0.0019 / 0.316 = 0.6 %`).
- **Every waveform-led converted preset's picture moves.** That includes both fixtures of the wash
  instrument Plan 0142 builds on: the washed *Fog Tunnel* draws mode 0, and its clean control
  *Blur Mix 3* draws mode 6.
  The converted goldens that draw a waveform re-bless. This is why Plan 0180 is sequenced before
  Plan 0142.
- **`AnalysisFrame` roughly doubles.** It grows by `2 * 512 + 1` floats, about 4.1 kB. By the doc
  comment's own arithmetic for the first 512 floats (~100 ns of memcpy at 60 Hz), that is on the
  order of 200 ns more per frame. The analyzer's frame loop also does two more writes per incoming
  frame into fixed buffers. Neither allocates, but both are on the render thread's analysis path,
  and Plan 0180 measures the per-hop cost rather than asserting it.
- **Mode 0 reads `time` again.** `draw.rs` stops being a pure function of the trace and the outputs
  (Plan 0109 Phase 2's contract on `draw::build`), and its time-independence test narrows to the
  other seven modes. That was a property worth having; the source's figure turns, so it goes.
- **A 5.1 stream reads front-left and front-right.** A layout that does not put them at channels 0
  and 1 draws the wrong pair. Both frontends' current sources (WASAPI loopback, foobar's
  `visualisation_stream`) use that order. Nothing validates it.

### Neutral

- Modes whose source figures happen to coincide lose `every_wave_mode_builds_a_different_figure`'s
  distinctness claim for that pair. The claim was this engine's invention, and the source is the
  contract.

## Alternatives considered

### Alternative A — The released source, strictly

Every constant from `milkdropfs.cpp`, including the `0.125` sample offset. **Rejected because it
would draw every waveform about 20 % smaller than the host people run** (`0.125` against `0.157`).
ADR-0113 names that host as the side-by-side reference. A contract its own look gate would fail is
not a contract.

### Alternative B — The `foo_vis_milk2` host, strictly

Fit every mode's figure and scale to host captures. **Rejected because it needs a rig session per
mode**, for figures whose shape the source already specifies and the host has no reason to change.
The one thing only the host can say is the scale, and one capture carries it, subject to the
confirmation in Consequences.

### Alternative C — A second channel through a new C ABI entry, or a second ring

**Rejected by the hard stop, and unnecessary.** The frames that carry both channels already cross the
boundary through `rlx_push_samples`. A new entry would widen a versioned contract (ADR-0003) to
deliver data the core is already handed.

### Alternative D — Defer the waveform until Plan 0142's verdict

**Rejected because the verdict would judge the wrong figure.** *Fog Tunnel*'s wash is measured on
a field whose source term is the mode-0 circle. A verdict taken on a figure this engine invented
could not say whether the import is worth more reach.

## Notes

- Source facts, all at `xeiraex/milkdrop2` `d4c843a`, are recorded in the archived bodies of backlog
  0119 and 0120 ([archive](../design-backlog-archive.md)) and in Plan 0180's Phase 1 log. Nothing is
  copied into the repository (Plan 0100 Phase 8's provenance rule).
- ADR-0071 governs this ADR twice: `k` is a measurement that names its capture, and `draw.rs`'s prose
  attributes a figure only to a source that was read.
