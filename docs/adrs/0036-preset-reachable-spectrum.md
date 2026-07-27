# ADR-0036 — Preset-reachable spectrum: a scalar `bin(x)` function, an N-element spectrum scene, and per-element evaluation as a bounded third step

> **Status:** accepted
> **Date:** 2026-07-26
> **Accepted:** 2026-07-27 at the Plan 0034 close, **with the Outcome section below** correcting one
> claim implementation falsified
> **Related plan(s):** [0034](../plans/0034-preset-reachable-spectrum.md)
> **Supplements:** [ADR-0002](0002-layered-preset-architecture.md) (the expression layer),
> [ADR-0020](0020-preset-grammar-v2-branching-functions-tempo.md) (grammar v2),
> [ADR-0007](0007-line-geometry-generators.md) (line generators)

## Context

The `preset-author` lane's 2026-07-26 feedback names this the single most-requested capability of
the session, asked for **twice, unprompted**: "a full spectrogram in several lines... 20-30 points",
and separately "morph the attractor shape from a full spectrogram with a lot of bars". The lane's
workaround — mapping the three bands onto three separable structural levers — drew the verdict
"represents not sure what... feels very poor". Three scalars are not a spectrum.

The report frames this as needing "both a grammar addition and a scene", and estimates it as the
largest item in the batch. **Verification against the code moves that estimate down substantially,
in three steps.**

**First: the spectrum already exists as a normalized, log-spaced band array on the analysis frame.**
`dsp/mod.rs:32` declares `pub const SPECTRUM_BINS: usize = 64`, commented *"Log-frequency bands
exposed to scenes"*, and `AnalysisFrame` carries `pub spectrum: [f32; SPECTRUM_BINS]` — computed
every hop by `fft.rs::analyze` over logarithmically-spaced band edges, normalized so a full-scale
sine reads near 1.0, and already consumed in anger by `novelty.rs`. There is **no new DSP to write**,
and determinism is preserved by construction: the spectrum is already a pure function of the input
window, with no wall clock and no randomness (NFR §6).

**Second: every scene already receives it.** `Scene::update(&mut self, frame: &AnalysisFrame)` hands
each scene the whole frame — all 64 bands — every frame. A scene that draws the spectrum needs no
new channel, no trait widening, and no grammar at all to *see* the data.

**Third: the renderer for it already exists.** `LineRenderer::draw(segments: &[SegmentInstance])` is
a generic instanced-quad line renderer serving the three existing line systems (ADR-0007). N bars, an
N-point polyline, or a radial ring of N spokes are all just segment lists. A spectrum scene is a
fourth consumer of an existing idiom, not a fifth render idiom.

So the honest problem statement is narrower than the report's. What is genuinely missing is:

1. **Scalar access to a frequency region from an expression.** The nine `VAR_NAMES` scalars cannot
   name a bin, so no preset can drive *any* parameter from a specific part of the spectrum. This is
   what the attractor-morphing ask actually needs — the attractor's shape is four scalars (`a`-`d`),
   not 50 000 per-particle values.
2. **A scene that draws N elements.** Nothing in the shipped set does.
3. **Author-controlled per-element mapping** — an author writing one expression evaluated once per
   element, so the *relationship* between bin and element is preset content rather than scene code.
   This is the only genuinely new mechanism in the batch, and it is the one that forces a decision
   about the binding model: today one expression yields one scalar and reaches the scene through one
   `set_param(&str, f32)` call. There is no channel on which a per-element value travels.

Items 1 and 2 are independent of item 3, and together they already satisfy **both** of the user's
original asks. That ordering is the substance of this decision.

## Decision

We will expose the spectrum to presets in three separable steps, and we will not conflate them.

**A scalar `bin(x)` function.** `bin(0.4)` reads the already-computed log-spaced spectrum at 40 % of
the way up its range, interpolating between adjacent bands so a preset never names the engine's bin
count. It is a `Call` node in the existing grammar — no new type, no indexing syntax, and the
expression layer stays scalar-only, pure, and allocation-free. Out-of-range input clamps rather than
erroring, matching how the grammar's other total functions behave.

**A `spectrum` line system.** A fourth `SystemKind` built on the existing `LineRenderer`, reading
`frame.spectrum` in `update` and drawing N elements under a declarative `[spectrum]` config — the
element count, and a layout among bars / polyline / radial ring — styled by ordinary named scalar
params (`thickness`, `hue`, `brightness`, `scale`, the shared view transform, and the palette). It
needs **no** grammar change and **no** `Scene` widening.

**Per-element expression evaluation, third and bounded.** Only after the two above land: a binding
may be evaluated once per element with the element's normalized position bound to an implicit
`index` variable, so `thickness = "0.01 + bin(index) * 0.05"` is authorable. This is the piece that
requires a new channel from the render layer to a scene, and it is deliberately last so the question
"do authors actually need this, given a scene that already maps bins to elements sensibly?" is
answered by evidence rather than by guess.

The waterfall spectrogram (time × frequency, a scrolling history field) is a **separate later scene**
built on the same `bin`/spectrum surface, using the `PingPongField` idiom ADR-0012 already
established. It is not part of this decision beyond the requirement that nothing here blocks it.

## Consequences

### Positive
- Both of the user's asks are satisfied by the first two steps, which together contain no new DSP,
  no new render idiom, and no change to the `Scene` trait or the C ABI.
- `bin(x)` is immediately useful far beyond the spectrum scene: any parameter of any system can be
  driven from a chosen frequency region — `a = "1.4 + bin(0.15) * 0.4"` morphs the attractor from the
  low-mids, which is the attractor ask, met by a grammar function rather than by a per-particle
  mechanism.
- Determinism is untouched. The spectrum is already a pure function of the input window, so a
  captured show stays frame-for-frame reproducible (NFR §6) and the golden fixtures stay meaningful.
- The scalar-only invariant of the expression language survives. That invariant is what makes every
  binding testable as a function of its inputs, and `bin` does not dent it.

### Negative
- **`Variables` grows.** Carrying 64 floats alongside nine scalars makes the per-frame variable
  bundle ~264 bytes instead of ~36. It is built **once per frame** and read per binding, so the cost
  is a single memcpy against an existing per-binding `exp()` — but `Variables` is `Copy`, and if it
  is ever passed by value on the per-binding path that becomes 264 bytes per binding per frame. The
  plan pins it to by-reference on that path.
- **`bin(x)` invites aliasing mistakes.** 64 log-spaced bands over 20 Hz–Nyquist means a single
  `bin()` call covers a wide musical interval at the top and a narrow one at the bottom. An author
  expecting "the kick" from `bin(0.02)` will sometimes get rumble. This is inherent to log spacing
  and is a documentation obligation, not a defect.
- **A fourth line system is a fourth thing to keep current** — a `PARAMS` const, a `SystemKind`
  arm, a golden fixture, and rows in `presets/README.md`. The exhaustive-match roster makes drift a
  compile error, so the cost is real but bounded.
- **Step three widens a seam and is not free.** Evaluating a binding N times per frame changes an
  invariant that has held since ADR-0002 — that an expression is evaluated exactly once per frame —
  and needs a channel carrying N values to a scene. At 30 elements and 6 bindings that is 180
  evaluations per frame, which is affordable, but the *shape* change is what matters and it is why
  this is sequenced last rather than bundled.
- The three steps mean the full capability lands across two plans, so the richest version of the
  feature is not available on day one.

### Neutral
- `SPECTRUM_BINS` stays 64 and stays an engine constant. The scene's element count is separate and
  preset-configurable; the scene downsamples 64 bands to the author's 20-30 elements.

## Alternatives considered

### Alternative A — `spectrum[i]` indexing
Reads most naturally, and is what comparable preset languages do. Rejected because it introduces the
first non-scalar type into a language that deliberately has none: an array value implies bounds
semantics, a rule for what bare `spectrum` means, and a type check in an evaluator whose totality
today comes from every node being an `f32`. `bin(x)` buys the same capability at the cost of one
`Call` arm.

### Alternative B — N flat variables (`band0`..`band63`)
No new syntax at all, just more names. Rejected on two counts: `VAR_COUNT` would jump from 9 to ~73
and put that payload on the per-frame path with no interpolation and no way to address a bin
computed from an expression — so the per-element mapping in step three would still need a separate
mechanism, and the flat names would be dead weight beside it.

### Alternative C — Upload the spectrum to the GPU as a small texture and let shaders sample it
The natural way to give the compute-particle and fullscreen-field scenes true per-element spectral
response, and cheap to build — the palette system already bakes a 256×1 LUT texture, so the idiom is
present. Rejected **for this decision, not permanently**: the attractor ask is satisfied by driving
its four shape scalars from `bin(x)`, and adding a GPU-side spectrum before any scene has been shown
to need per-pixel or per-particle spectral detail would be building a mechanism ahead of its use
case. It is the obvious next step if a scene later wants 50 000 particles each reacting to their own
frequency.

### Alternative D — Ship only `bin(x)` and no scene
Cheapest possible answer, and it does unlock the attractor morphing. Rejected because it leaves the
literally-stated request — a visible spectrum readout — unmet, and because the scene is inexpensive
given that the data, the renderer, and the delivery channel all already exist.

## Outcome (added 2026-07-27 at acceptance)

The decision held: `bin(x)` + a `spectrum` system + per-element `index` shipped in five phases with
no new DSP, no new render idiom, no `Scene`-trait change and no C-ABI change, exactly as scoped. The
`Variables` copy concern above was answered better than the plan proposed — by **borrowing with a
lifetime** (`Variables<'a>` holding `spectrum: &'a [f32]`), so the per-binding path carries a fat
pointer and the 264-byte bundle never exists.

**One claim in Consequences is wrong and is corrected here rather than edited above** (this ADR is
append-only; same treatment as [ADR-0034](0034-internal-resolution-follows-the-target.md)):

> "64 log-spaced bands over 20 Hz–Nyquist means a single `bin()` call covers a wide musical interval
> at the top and a narrow one at the bottom."

**Both halves are false**, verified against `core/src/dsp/fft.rs:56-76` and by independent numerical
replication at 48 kHz:

- **The range is 35 Hz to 18 kHz** (`BAND_LO_HZ` / `BAND_HI_HZ`, the top additionally clamped to
  `0.45 * sample_rate`), not 20 Hz to Nyquist.
- **The array is not log-spaced over half its length.** `new()` computes log edges and then floors
  every band at one FFT bin; at a 2048-point window that floor is 23.4 Hz and it binds from band 1 to
  **band 30 (~750 Hz)**, so **31 of the 64 bands are linear 23.4 Hz slices**.
- **The resolution profile is therefore backwards from the claim.** The *bottom* is the coarsest part
  musically — band 0 spans 23–47 Hz, a full octave in one number — resolution peaks around
  500–800 Hz (band 30 is 0.55 semitones), and the *top* is a constant ~1.7 semitones per band. The
  aliasing hazard the ADR predicted is real but sits at the **bottom**, not the top.
- **A consequence the ADR did not anticipate:** below the crossover the mapping depends on
  `sample_rate / 2048`, so the same `bin(x)` names a different frequency at 44.1 kHz than at 48 kHz.

The documentation obligation the ADR correctly identified is discharged in `4d41884`
(`docs/presets.md` and `presets/README.md` carry a measured position table and both consequences),
and the surviving question — whether the half-linear axis is a defect to fix or a characteristic to
live with — is **[design-backlog 0015](../design-backlog.md)**, with three weighed alternatives. It
is ADR-worthy if acted on.

Also learned, and now documented rather than left to be rediscovered: a band value is the **peak**
linear bin within the band while `bass`/`mid`/`treb` are **means** over wide ranges, so a single
`bin()` measures comparably to a band scalar on broadband material (~0.02–0.05 against ~0.022) but
reaches far higher on tonal content. Gains transfer between the two unchanged; what differs is
selectivity. Averaging a few `bin()` calls **spot-samples** a region rather than integrating it —
measured against a 6.5 kHz tone, `bin(0.84)` reads 0.094 while `bin(0.82)` and `bin(0.88)` read
exactly zero — which is the strongest argument yet for the deferred `bin_range(lo, hi)`.

## Notes

- The report's framing ("the FFT already exists — it's just not reachable from a preset") is correct
  and understates itself: it is not raw FFT magnitudes that exist, but a **normalized, log-spaced,
  64-band spectrum already documented as being for scenes**.
- The per-element `index` in step three is deliberately **normalized 0..1**, not an integer, so an
  expression composes with `bin(index)` directly and does not need to know the element count.
