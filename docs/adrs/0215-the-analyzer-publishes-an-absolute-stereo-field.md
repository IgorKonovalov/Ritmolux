# ADR-0215 — The analyzer publishes an absolute stereo field, and the mono path does not move

> **Status:** proposed
> **Date:** 2026-09-18
> **Related plan(s):** [0194](../plans/0194-the-analysis-gains-a-stereo-field.md)
> **Extends:** [0199](0199-a-converted-waveform-draws-the-sources-figure-at-the-hosts-scale.md)
> (a converted waveform draws the source's figure, from a stereo pair the analyzer already receives)

## Context

Stereo reaches this engine and then stops. `rlx_create` validates a channel count in `1..=8`, the
ring carries interleaved frames unchanged (`docs/specs/0002-ring-determinism.md`), and every intake
delivers the device's real channels — WASAPI's `nChannels`, ScreenCaptureKit's planes interleaved,
PulseAudio at a fixed 48 kHz stereo (ADR-0131), foobar's `visualisation_stream` the same way. Then
`Analyzer::push_interleaved` averages each frame to mono before anything spectral runs, and every
quantity a preset can bind — `bass`, `mid`, `treb`, `onset`, `beat`, `bar`, `bpm`, `bin()`, the
`spectrum` array — is that average. A hard-panned hi-hat contributes half its level and nothing
downstream knows which side it came from.

ADR-0199 opened one door and deliberately no more. It added `waveform_pair` — channels 0 and 1 as
two `WAVE_SAMPLES` traces, levelled by a single divisor so the L/R ratio survives — because
MilkDrop's `wave_mode` 1-3 and 7 are x-y oscilloscopes that cannot be drawn from a mono trace. That
pair's only consumer is `core/src/milk/` feeding `warp_mesh`'s converted draw layer. It is not in
the expression grammar, `wave_mode` is a `MilkOutput` rather than a `ParamSpec`, no shipped preset
sets it, and none of the six shipped `warp_mesh` presets carries a `[milk]` block. So stereo is
today a converter-corpus capability with no shipped user and no reach into preset content.

ADR-0199 also established the decisive fact about *where* the second channel comes from: the
interleaved frames are already at the analyzer, and `AnalysisFrame` is not part of the C ABI
surface. **A stereo quantity is an analyzer change, not a boundary change.** Nothing in
`core-cabi/`, the ring, the plugin shim or the capture backends moves.

Three forces shape what is built on top of that.

**Cost is not the constraint it looks like.** `docs/nfr.md` allocates ~11 ms per hop to the FFT and
records measured per-hop analysis cost at **31.5 µs** — about 350x headroom, after ADR-0049 nearly
doubled that figure from 17.2 µs. A second spectral pass to get per-band stereo lands near 63 µs.
The measurement is still owed rather than assumed, but the decision does not have to be designed
around a cheap approximation on the expectation that the exact mechanism is unaffordable.

**Compatibility is the stated constraint.** 113 presets ship. Stereo is an optional addition, and
no existing preset may render differently because of it. ADR-0199 made and held exactly this promise
for the mono `waveform`; the same promise here has to cover every field of `AnalysisFrame`, and it
has to be a test rather than an intention.

**Nothing in this repository can currently see stereo.** Every `--signal` kind in
`core/src/signal.rs` builds a mono buffer and `interleave`s it into both channels, so the two
channels are bit-identical in every synthetic test. A stereo implementation and a broken one produce
the same output under the whole harness. This is the pattern Mode 4's own review lens names: two
sources that agree on the one configuration we test at, where no test at that configuration can say
which source the code read.

## Decision

We will publish a **stereo field** on `AnalysisFrame` — whole-mix `balance` and `spread`, plus
per-band `bass_balance` / `mid_balance` / `treb_balance` — as **absolute quantities that are never
levelled against a running peak**, reach preset expressions as ordinary grammar variables, and are
computed on a path that **no pre-existing field reads**, so the mono analysis is a function of the
channel average alone and stays bit-for-bit what it was.

1. **The quantities are absolute.** `balance` is `(R - L) / (R + L)` over the hop's per-channel RMS,
   in `-1..=1`, `0` at centre, reading exactly `0` below `gain::WAVE_FLOOR` rather than amplifying a
   silent stream's noise into a wandering position. `spread` is `(1 - corr) / 2` from the normalized
   L/R correlation over the hop, in `0..=1`: `0` for identical channels, `0.5` for fully
   decorrelated, `1` for polarity-inverted. `<band>_balance` is the same ratio taken over that
   band's per-channel energy. A one-channel stream reads `0` for all five, as does a mono-duplicated
   stereo stream, because that is the truth about both.

   This is a deliberate break with ADR-0049's normalized bands, and the reason is that a levelled
   position is a lie an author cannot detect. Divided by a running peak, a centred mix's noise floor
   is stretched into a confident wandering pan, and a preset bound to it moves convincingly on
   material that has no stereo information at all. `balance = 0` must mean centred on every track,
   forever.

2. **The real-world range is published rather than engineered.** The price of absolute is a narrow
   usable range — real music sits well inside `±0.3` of centre most of the time — and the thing that
   makes a narrow absolute range usable is knowing what it actually is. `shot --report` gains
   min/mean/max rows for `balance` and `spread` beside the band rows it already prints, for the same
   reason those rows exist: an author calibrating a gain against a guessed magnitude ships a preset
   that does not move on music.

3. **Per-band balance takes the exact mechanism if it is affordable, and says so if it is not.**
   Plan 0194 measures the per-channel-spectrum mechanism against the baseline. **If per-hop analysis
   cost stays under 1 % of the 11 ms allocation (110 µs), the per-channel spectrum is the
   implementation** and `<band>_balance` is `bands.split()` run per channel — no new approximation,
   and the band edges are by construction the same ones `bass`/`mid`/`treb` use. Otherwise the
   implementation is per-channel bandpass energy in the time domain, and this ADR takes a dated
   `Outcome` recording the measurement, the approximation and what it costs in accuracy. The
   measurement names its machine (ADR-0071); the rule, not the number, is what `dev` follows.

4. **Isolation is a property, and the property is the test.** For any stereo stimulus `S`, let `M`
   be the stimulus whose two channels both carry `S`'s channel average. **Every field of
   `AnalysisFrame` that existed before this change reads bit-identically for `S` and `M`** — while
   the stereo field reads differently. That single assertion encodes the whole compatibility promise
   without a golden of the previous build, and it stays true as a contract for every field added
   later. The stereo quantities accumulate in their own module and feed nothing upstream of
   themselves.

5. **The grammar gains five names, and none of them is `pan`.** `pan_x` and `pan_y` are already
   view-transform params on most scenes, and a stereo variable called `pan` would sit one underscore
   from "slide the picture sideways" while frequently being bound to it. `balance` and `spread` are
   the names. They join `VAR_NAMES` additively; the schema export (ADR-0190) regenerates the editor
   rosters from the engine's own declarations, so no second copy is written by hand.

6. **The drawable figure and the placement term are decided here and deferred.** A native x-y
   goniometer over `waveform_pair`, and a scene-level term that positions a figure by the stereo
   field, are both wanted and neither is built by Plan 0194. They are scene geometry, and designing
   them before anyone has watched these numbers move on real music would be designing against a
   guess. They land in a follow-up plan under this ADR. **When the placement term is built it is an
   opt-in param defaulting to inert**, because it is the one item in this decision that cannot be
   additive by construction: an automatic placement changes where existing scenes put things.

## Consequences

### Positive

- Every one of the 113 shipped presets can gain a stereo dimension by adding one binding, with no
  engine work and no risk to what it renders today.
- `balance = 0` is trustworthy across material, which is what makes a stereo binding composable with
  the levelled band values rather than fighting them.
- The harness stops being blind to stereo, which is worth more than the feature: `waveform_pair` has
  shipped since Plan 0180 with no synthetic stimulus able to distinguish it from a mono duplicate.
- Nothing crosses the C ABI, the ring, the capture backends or the plugin shim, so foobar and the
  standalone get this on the same day for free.
- The isolation property is a reusable contract. Any future analysis quantity claiming not to
  disturb the mono path is held to the same assertion.

### Negative

- **An absolute quantity reads dead on most material, and that is the design.** A preset binding
  `balance` directly to a large visual move will look static on a centred mix and the author will
  reasonably suspect the engine. The `--report` rows and the grammar docs are the only mitigation,
  and they are documentation rather than a mechanism.
- **A mono source reads `0` forever.** A mono file in foobar, a mono capture device, or any of the
  existing `--signal` kinds gives a flat zero, so a preset that leans on stereo has a silent failure
  mode on perfectly ordinary input.
- **Surround content is invisible.** Channels 0 and 1 only, consistent with ADR-0199. A 5.1 stream's
  rear and centre channels do not reach `balance`, and nothing reports that they were dropped.
- Five more grammar names is five more things an author must learn, on a surface that ADR-0137 and
  ADR-0036 have each already widened.
- The per-hop analysis cost moves, and `docs/nfr.md`'s 31.5 µs figure with it — a number that has
  now moved twice and is cited in an argument about headroom each time.

### Neutral

- `spread`'s `0.5`-means-decorrelated landmark is unintuitive on first reading and exact on second.
  It is taught in `docs/presets.md` rather than smoothed away.
- **The field is published raw, per hop, with no smoother.** A 512-sample hop is 10.7 ms, which is
  about one cycle of a low bass note, so `spread` in particular jitters on bass-heavy material. That
  is left to the preset: `[smoothing]` already eases any binding, and a smoother inside the analyzer
  would be a second opinion an author cannot turn off. Same reasoning as the `*_raw` levels.
- Per-band `spread` is not built. It is the same machinery as per-band `balance` and a later plan
  adds it cheaply; three more names for a look nobody has named yet is not a trade worth making now.

## Alternatives considered

### Alternative A — Level the stereo field like the bands

Divide `balance` and `spread` by a running peak, as ADR-0049 does for `bass`/`mid`/`treb`, so a
subtly-panned mix stretches to full range and a preset always visibly reacts. Rejected because the
failure is undetectable from inside a preset: on a mono or near-mono track the divisor amplifies the
noise floor, and the author sees confident stereo motion that is not in the music. The band levels
survive this because loudness is always present and the question is only its scale; position can be
genuinely absent, and a normalizer cannot represent absence. A levelled twin (`balance_hot` beside
`balance`) was the obvious compromise and was rejected for doubling the vocabulary and the docs
surface for a choice most presets make once.

### Alternative B — Whole-mix scalars only, no per-band stereo

Ship `balance` and `spread` from per-channel RMS and correlation — a few hundred operations per hop,
no measurement needed, no new spectral work. Rejected because the expressive half is per-band: the
look worth having is treble sparks thrown to the side the hat is on while the bass stays centred,
and a whole-mix scalar collapses exactly that. The cost that would have justified the cut is not
there — 31.5 µs against 11 ms — so cutting it would have been caution against a number nobody had
measured.

### Alternative C — Expose `waveform_pair` to the grammar and let presets do the arithmetic

The pair is already published. A preset could read left and right and compute its own balance.
Rejected on two counts: the grammar is scalar and has no array or reduction vocabulary, so this is
not expressible today; and if it were, every preset would re-derive the same statistic with its own
floor handling and its own silence behaviour, which is the drift ADR-0170's generated reference
exists to prevent.

### Alternative D — Build the figure and the placement term now

Carry the goniometer and the stereo-placement param in the same plan, so the capability lands with
something visible using it. Rejected because the scene half is a `Scene`-seam question that ADR-0002
keeps deliberately thin, and because the figures would be designed before the quantities had been
watched on real material. Deferred under this ADR rather than dropped.

## Notes

- The interview that produced this recorded one constraint verbatim: *"we don't want to lose
  compatibility with old presets, stereo should be an optional addition."* Decision point 4 is that
  sentence turned into an assertion.
- `docs/nfr.md` "Beat-to-photon latency" carries the 31.5 µs measurement and the ~11 ms allocation
  this decision reasons from.
- Prior art for an additive analysis change that promised the existing path would not move, and
  held: ADR-0199 decision point 3.
