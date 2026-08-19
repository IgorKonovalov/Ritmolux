# 0111 — The MilkDrop import stops washing out

> **Status:** draft
> **Created:** 2026-08-19
> **Owner skill(s):** dev, human
> **Related ADRs:** none yet, deliberately — Phase 3 writes one (**ADR-0120**) if and only if the
> Phase 2 bisect names a stage whose semantics are a decision rather than a defect. Reads against
> [0118](../adrs/0118-the-milkdrop-feedback-field-quantizes-in-the-encoded-domain.md) (the
> quantizer), [0119](../adrs/0119-the-video-echo-blends-toward-its-copy-rather-than-adding-it.md)
> (the echo blend), [0046](../adrs/0046-linear-light-hdr-composite-bloom-tonemap.md) (the tonemap) and
> [0071](../adrs/0071-a-numeric-test-contract-states-a-property-or-names-its-machine.md) / [0074](../adrs/0074-a-ratio-against-an-in-run-control-is-not-automatically-portable.md)
> (what a numeric assertion in here is allowed to claim).
> **Closes:** design-backlog 0121, 0119, 0120. **0113 is carried, not closed** — it closes only if
> Phase 3 runs *and* lands a mechanism; on the Phase 2 stop branch it takes a second dated update.

## TL;DR

The MilkDrop import's dominant remaining defect is the wash: five of Plan 0108's seven judged pairs
came back with a background that equilibrates far brighter than the reference's, and after two plans
nobody can say which stage does it. [Plan 0109](done/0109-the-milkdrop-import-gets-its-geometry-back.md)
Phase 4 built the instrument and **ruled the field itself clean**, which leaves the whole downstream
chain — present, blend, bloom, tonemap — as one undifferentiated suspect. This plan bisects that
chain with one statistic measured at five seams, stops the moment a seam separates a washed
conversion from the clean control, and fixes what the bisect names. Alongside it the three smaller
named defects land: the `decay` fallback's units error (first, because it corrupts instruments), the
`ang` branch cut's handedness, and the waveform figure's missing scale constant. A fourth look gate
judges the result.

## Context & problem

Four MilkDrop entries are live on the backlog and all four are fidelity, not reach:

| Entry | Claim | State entering this plan |
|---|---|---|
| [0113](../design-backlog.md) | the converted feedback field equilibrates far brighter than the reference's | **High.** Instrument built; field ruled clean; three hypotheses dead |
| [0119](../design-backlog.md) | `ang`'s branch cut on +x seams every per-vertex program continuous in it | Medium; shows on two MD1 presets carrying no shader block |
| [0120](../design-backlog.md) | the converted waveform figure renders larger than the reference's | Medium; `fWaveScale` applied as a bare multiply |
| [0121](../design-backlog.md) | a bundle that never names `decay` reads a per-frame default as a per-second one | Low-medium; latent for content, **has already corrupted one instrument** |

**What Plan 0109 Phase 4 actually bought.** `PingPongField` carries `COPY_SRC` and a test-only
`read_texture`; `warp_mesh/tests.rs` drives the scene directly, copies the field after every frame
and decodes it with `read_back_linear`, which does not clamp at 1. With the quantizer on, the field
converges by frame 120 and a zooming tunnel's background sits at `1e-6` linear and stays there. So
the field is not the wash, and neither is the decay multiply's domain
(`the_decay_domain_is_not_the_wash` derives both predictions and measures which one the frames ran
with). Phase 4's own live hypothesis — that a converted warp shader applies `decay` only when its
HLSL names it — was **falsified by census at the close**: the five washed presets carry no `warp_`
or `comp_` block at all.

**So the remaining suspect is everything between the field and the pixel, and nothing has looked
there.** That is four stages, in this order, and each is a plausible wash on its own arithmetic:

1. the **present pass** — the echo mix, then `× brightness × gamma`, then the four composite
   remaps, then `× occlude` on alpha;
2. the **backdrop + `layer_blend`** composite;
3. **bloom**, which sums above 1.0 freely by design;
4. the **tonemap** (ADR-0046), whose shoulder is what turns unbounded linear light into a picture —
   and therefore what turns a too-large linear value into a *bright wash* rather than a clip.

### One code fact worth stating before `dev` rediscovers it

`gamma` is applied as a **linear multiply**, not as a power:

```wgsl
// core/src/render/scenes/warp_mesh/mod.rs — PRESENT_SHADER, pp.a.z is gamma
var col = c.rgb * max(pp.a.x, 0.0) * max(pp.a.z, 0.0);
```

`milkconv/src/convert.rs:77` maps `fgammaadj` to `gamma` and `core/src/milk/outputs.rs:115` declares
it `Plain`, so the authored value arrives unconverted. A preset with `fGammaAdj = 1.9` therefore
takes a **1.9x unclamped linear gain on the whole composited field, feeding a tonemap whose shoulder
then spreads it** — which has the right sign, sits at the right place in the chain, and is named
"gamma" while performing a multiply. That last part is exactly the prose hazard ADR-0071's rule keeps
catching a level down.

**It is a lead, and the evidence is genuinely mixed.** All seven judged presets, read from the
corpus at their pinned paths (see Phase 6), against Plan 0109's Phase 5 verdicts:

| preset | `fGammaAdj` | `fDecay` | echo | 0109 verdict |
|---|---|---|---|---|
| *Blur Mix 3* | **1.0** | 1.0 | 0.0 | fixed — the clean control |
| *Songflower (Moss Posy)* | **1.0** | 1.0 | 1.0 | fixed, after the echo blend |
| *Contortion (Escher's Tunnel Mix)* | **1.0** | 0.99 | 0.0 | better, **still too bright** |
| *chasers 19 Portal* | 1.28 | 0.96 | 0.0 | fixed |
| *Fog Tunnel* | **1.8** | 0.98 | 0.4 | **still washed** |
| *Cosmic Dust 2* | **1.9** | 0.98 | 0.0 | **still washed** |
| *Cauldron painterly 5* | **2.7** | 0.98 | 0.0 | better, holds |

Gamma separates the two still-washed presets from the two clean ones and is **contradicted twice**:
*Cauldron painterly 5* carries the highest value in the set and reads fine, and *Contortion* washes
at unity — which is the backlog entry's own stated counter-evidence, and reading the file confirms it
rather than overturning it. So gamma is a lever the bisect must measure at seam B, **not a
diagnosis**, and a fix that only makes *Fog Tunnel* and *Cosmic Dust 2* better while leaving
*Contortion* bright has not found the wash.

Note also, against naive expectation, that the two clean controls are the two presets with
`fDecay = 1.0` — which converts to this engine's `MAX_DECAY` ceiling, essentially no fade at all.
Whatever the wash is, it does not track how much of the past survives.

## Decision

**We bisect the chain with one statistic and stop at the first seam that separates a washed
conversion from the clean control**, then fix the mechanism that seam names. We rejected *commit to a
fix in this plan* because every prior attempt on this defect that started from a candidate mechanism
has been falsified — three hypotheses are already dead and a fourth died at Plan 0109's close — and a
plan that must land a change ends by tuning until a picture matches. We rejected *diagnosis only, fix
in a successor plan* because the bisect's stop condition already gives the plan an honest exit, and
forbidding a fix that the evidence hands us would cost a whole extra plan cycle for nothing.

The three smaller defects run as their own phases in the same plan rather than as three plans,
because they touch the same four files and would otherwise contend. **Reach is out of scope** —
backlog [0109](../design-backlog.md) (disk textures, 1 826 files) and [0108](../design-backlog.md)
(the conversion tail) stay filed, on the ordering those entries themselves argue for: two look gates
have now returned *"still merely different"*, and reach is only worth buying once quality is judged
better.

## Architecture diagram

```mermaid
flowchart LR
    subgraph scene["warp_mesh scene - core/src/render/scenes/warp_mesh/"]
        WARP["warp pass<br/>decay^dt, quantize"] --> FIELD[("field<br/>Rgba16Float ping-pong")]
        DEP["deposit pass"] --> FIELD
        DRAW["draw layer<br/>waveform figure"] --> FIELD
        FIELD -->|"SEAM A - measured CLEAN<br/>Plan 0109 Phase 4"| PRES["present pass<br/>echo mix, x brightness, x gamma,<br/>4 remaps, x occlude"]
    end
    subgraph chain["composite chain - core/src/render/"]
        PRES -->|"SEAM B"| BLEND["backdrop + layer_blend"]
        BLEND -->|"SEAM C"| BLOOM["bloom<br/>sums above 1.0"]
        BLOOM -->|"SEAM D"| TONE["tonemap<br/>ADR-0046 shoulder"]
        TONE -->|"SEAM E"| OUT["sRGB swapchain"]
    end
```

## Implementation phases

### Phase 1 — `decay`'s fallback converts, so an instrument can be trusted

- **Owner skill:** dev
- **What:** Closes backlog 0121. `FrameOutputs::read`'s `None` arm returns the seeded default
  **unconverted**, into a field whose own doc says "per second here" — so a bundle that never names
  `decay` fades at `0.98`/s (about `0.9997` per frame) instead of `per_second_factor(0.98) = 0.5455`.
- **Why it is first:** Phase 2's probe drives bundles directly, and Plan 0109 Phase 4 already ran an
  entire experiment through this bug — its reported fade numbers could not be reproduced from the
  committed code until the close review found it. Every later measurement in this plan is worth less
  until this lands.
- **Files touched:** `core/src/milk/outputs.rs`; whichever goldens move.
- **Done when:** the `None` arm converts through the same `convert` the `Some` arm uses, so the
  fallback is in the same vocabulary as the resolved value; a test drives a bundle naming no `decay`
  and asserts the field it reads back equals `convert(default, Rate::Factor, default)` **exactly**
  (both sides computed from the table, not from a literal — the point of the table is that a value
  cannot silently read its neighbour); and the commit message **names every fixture whose golden
  moved** and why that fixture's bundle omits `decay`. If no golden moves, say so — that is a claim
  about the fixture set, and it is checkable.

### Phase 2 — the wash bisect: one statistic at five seams

- **Owner skill:** dev
- **What:** Carries backlog 0113. Measure the same quantity at each seam of the chain, for a washed
  conversion and the clean control, and name the first seam at which they separate.
- **The statistic is the one that already exists:** `FieldTrace`'s `edge` — the mean over the
  outermost ring of texels, which is the background, where a centred deposit puts nothing. Using one
  statistic at every seam is the whole design: the numbers are then comparable across stages, and the
  quantity compared is always linear light against linear light (ADR-0074's same-kind requirement,
  which the dual-live ratio failed by comparing two terms that responded to the machine differently).
- **The two subjects:** *Geiss - Fog Tunnel* (still washed, and its defect is legible — the reference
  draws a skeleton of discrete concentric rings where this draws a solid tube, so the gaps between
  the rings **are** the background) and *Geiss - Blur Mix 3* (the clean control). Same signal, same
  hop, same size, same adapter, one run.
- **Files touched:** `core/src/render/scenes/warp_mesh/tests.rs` or a new `core/tests/milk_wash.rs`
  (`dev`'s call — seams B through E need a `Renderer`, which the scene-level probe deliberately does
  not have); read-back plumbing under `core/src/render/` as the seams require.
- **Done when:** the probe reports the `edge` statistic at seams A through E for both subjects, and
  the phase **names the first seam at which the washed/control ratio departs from what that ratio was
  at seam A**. The tolerance is **derived from the instrument, not chosen**: repeat the same capture
  three times and take the observed spread as the floor a departure must clear. Record the whole
  five-seam table in the plan's implementation log, whatever it says.
- **Stop condition — an honest exit, not a failure.** If no seam separates them, the phase stops. It
  records the five-seam table on backlog 0113 as a second dated update, Phase 3 does **not** run, and
  **Phases 4, 5 and 6 run anyway** — the stop ends the wash line, not the plan.
- **A stated lead, to be measured and not assumed:** seam B carries the `× gamma` linear multiply
  above. If seam B is where they separate, that is where to look first. If seam B is clean, gamma is
  dead as a wash mechanism and the log says so — which is worth almost as much, because it is the
  last candidate anyone has named.
- **A stated assumption:** the `edge` statistic reads the background only for a figure that does not
  fill the frame. *Fog Tunnel* qualifies. Put that in the probe's doc comment, so the next person who
  points this instrument at a frame-filling preset knows what it will measure instead.

### Phase 3 — the fix the bisect names

- **Owner skill:** dev
- **What:** Conditional on Phase 2 naming a seam. Change the arithmetic at that seam so it computes
  what the reference computes.
- **Files touched:** determined by Phase 2. Most likely `core/src/render/scenes/warp_mesh/mod.rs`
  (present shader), `core/src/milk/outputs.rs`, or `milkconv/src/convert.rs`.
- **Done when:** the mechanism is written down **as arithmetic** — what the reference computes, what
  this engine computes, and the one difference — *before* any code changes; the change makes this
  engine compute the stated thing; and a test asserts **that arithmetic**, not a picture. What a
  preset's authored value maps to is a property, and a property is assertable without a capture.
- **The hard guard: no tuning to a picture.** If the bisect names a *seam* but no mechanism survives
  being written down as arithmetic, this phase stops exactly as Phase 2 does and records the seam. A
  change that makes *Fog Tunnel* look right without a stated mechanism is worse than no change,
  because at the next gate it is indistinguishable from a fix.
- **ADR trigger:** if the named stage's behaviour is a *decision* rather than a defect — the most
  likely instance being "what `fGammaAdj` means when our target is linear and the reference's was
  8-bit display-referred" — this phase writes **ADR-0120**, against Phase 2's evidence rather than
  guessed up front. That placement is Plan 0106's shape and it is deliberate.
- **Bless scope:** a change at seam B or later moves every `warp_mesh` golden and possibly others.
  **Compare adapters before blessing** — a probe that adds render targets is precisely the shape that
  has twice produced a confidently wrong WARP picture here while hardware was correct, and a golden
  suite blessed under that condition blesses garbage.

### Phase 4 — `ang`'s handedness is measured, and only then is the seam a defect

- **Owner skill:** dev
- **What:** Closes backlog 0119. `vertex_position` computes `ang = atan2(py, px)` and lifts the
  negative half by `TAU`, so `ang` is `0..tau` with a discontinuity along +x. The seam runs centre to
  right edge on *Songflower (Moss Posy)* and reads full-width on *chasers 19 Portal* (that preset's
  own fold mirrors it) — both MD1 presets with no shader block, so `emit.rs` never runs and Plan
  0108's reading of this symptom is superseded.
- **The actual question, and it is not whether the cut exists.** The reference has the same branch cut
  in `atan2`, so MilkDrop presets are *authored against* a discontinuity at +x. What this engine also
  does is **flip y deliberately** — `let py = (0.5 - y) * 2.0`, commented *"so `ang` reads the way it
  looks"* — a convention authored for **this engine's** preset grammar, not for converted content. If
  the handedness differs from the reference's, then every angle-driven per-vertex program in the
  corpus runs mirrored and the seam is a *symptom* of that, not the defect.
- **Files touched:** `core/src/render/scenes/warp_mesh/mod.rs`, `core/src/render/scenes/warp_mesh/tests.rs`,
  possibly `milkconv/src/` if the correction belongs in the conversion rather than in the scene.
- **Done when:** a test pins this engine's `ang` against the reference's construction — **derived from
  the source format's convention or from the reference implementation, with the source named in the
  test's doc comment, never from a picture** — and states in one sentence whether the cut's location
  and its handedness agree.
  - **If they agree:** the phase changes no behaviour and records that the seam is authored-against.
    That is a complete outcome. **Do not smooth the wrap** — the entry says so, and it would break
    every preset that uses the cut deliberately.
  - **If they disagree:** the phase corrects the handedness and blesses what moves, with the same
    adapter comparison Phase 3 owes.
- **Prose guard:** whatever the test concludes, it must not attribute a convention to "MilkDrop" in
  general when it was read off one version of one implementation. Name the version. That is ADR-0071's
  rule applied to a comment, which is the level down where it keeps getting missed.

### Phase 5 — the waveform's scale constant, and whether the x-extent is a second defect

- **Owner skill:** dev
- **What:** Closes backlog 0120. `draw.rs` applies `*slot = held * scale` straight from `fWaveScale`
  onto a trace already normalized to `-1..1`, with no normalization constant and no comment on units.
- **The evidence that this is not one defect, and it is stronger than the entry recorded.** Two pairs
  reported an oversized figure independently, and the complaint does not scale with the authored
  value: *Blur Mix 3* (`fWaveScale = 3.266`, `nWaveMode = 6`) read as *less* wrong than *Cauldron
  painterly 5* (`1.139`, `nWaveMode = 5`). Across the seven pinned presets the authored value spans
  **0.0117** (*chasers 19 Portal*) to **3.266** — a factor of 279. A bare multiplier on a `-1..1`
  trace would make the low end invisible, and it is not, so `fWaveScale` is not a bare multiplier and
  the constant may well be mode-dependent. Separately, *Blur Mix 3*'s **crisp trace spans roughly the
  middle 57 % of the frame** while its blurred halo reaches the edges, where the reference draws
  full-width traces — amplitude and extent are different quantities and may have different causes.
- **Files touched:** `core/src/render/scenes/warp_mesh/draw.rs`, `milkconv/tests/draw_layer.rs`.
- **Done when:** the reference's normalization is **derived**, with its source named in the code
  comment, and applied; and the x-extent question is answered either "the same defect" (shown, by the
  corrected constant also correcting the span) or "a second one" (**filed as a new backlog entry**,
  with what was measured). `milkconv/tests/draw_layer.rs` already builds the figure as geometry, so
  both the amplitude and the span are assertable without a capture.
- **If the constant is not derivable** from a source available to `dev`, the phase says so, changes
  nothing, and the entry stays live. That is the correct outcome — matching a picture would produce a
  number that is right for one preset at one wave mode and wrong for the corpus, and nothing
  downstream would ever notice.

### Phase 6 — the look gate, fourth time

- **Owner skill:** human
- **What:** The same seven pairs, the same rig (`foo_vis_milk2` 0.2.0.0, DX11, beside release
  `lmv.exe`), the same three-variant judging set as Plan 0108 and Plan 0109 Phase 5.
- **Pin the reference by full path.** Plan 0108's gate lost a pair to *Geiss - Cosmic Dust 2 - Trails
  5b* being judged against the plain preset, and authoring this plan **very nearly repeated it** — a
  loose search matched *beta106i - Contortion (Wind Up)* and reported a `fGammaAdj` that would have
  made gamma look like a clean answer. All seven sit in one directory,
  `WORK/milkdrop-corpus/milkdrop-original/Milkdrop-Original/`:

  | | file |
  |---|---|
  | 1 | `Aderrasi - Contortion (Escher's Tunnel Mix).milk` |
  | 2 | `Aderrasi - Songflower (Moss Posy).milk` |
  | 3 | `Eo.S. + Phat - chasers 19 Portal.milk` |
  | 4 | `Geiss - Blur Mix 3.milk` |
  | 5 | `Geiss - Cauldron - painterly 5.milk` |
  | 6 | `Geiss - Cosmic Dust 2.milk` |
  | 7 | `Geiss - Fog Tunnel.milk` |

- **Done when:** a verdict per pair, plus answers to five questions:
  1. **Is the wash reduced on *Fog Tunnel* and *Cosmic Dust 2*?** Phase 3's acceptance case on real
     content — and specifically, does *Fog Tunnel*'s tunnel read as **discrete concentric rings**
     rather than as a solid tube.
  2. **Does *Contortion* still read too bright?** It washes at `fGammaAdj = 1.0`, so if Phase 3 landed
     a gamma fix and this pair is unchanged, the wash has more than one home. Also: **is the black ray
     artifact on the four frame edges still there?** Backlog 0113 retracted it *provisionally* as a
     presentation of the wash (`ob_size = ib_size = 0.01` in pure black, dragged inward stroke on
     stroke); this gate can settle it.
  3. **Is the seam gone on *Songflower* and *chasers 19 Portal*** — or, if Phase 4 concluded the
     handedness agrees, does it read as something the author placed rather than something we added?
  4. **Is the waveform figure the right size on *Blur Mix 3* and *Cauldron painterly 5*,** and does
     *Blur Mix 3*'s crisp trace now reach the frame edges?
  5. **Better, or still merely different?** Plan 0100 Phase 7 and Plan 0108 Phase 2 both returned
     *merely different*. A third *merely different* is a **product finding about the import's value**
     and must be recorded as one — it is the evidence backlog 0109 says to weigh before spending a
     plan on reach.
- **Cosmic Dust 2's hue is not evidence and must not be judged.** It drives `wave_r`/`wave_g`/`wave_b`
  from three independent LFOs on `time` at incommensurate ~4-7 s periods, so two renderers started at
  different moments are simply out of phase. Judge its **background level**, not its colour.

## Data shapes

```rust
// illustrative — not the final interface
/// One subject's background level at each seam of the chain, in linear light.
/// The same statistic throughout (`FieldTrace::edge`), so the washed/control
/// comparison at any seam is same-kind (ADR-0074) and the seam-to-seam
/// comparison is a ratio of two dimensionless ratios.
struct SeamTrace {
    a_field: f32,   // after warp/deposit/draw - known clean (Plan 0109 Phase 4)
    b_present: f32, // after echo mix, x brightness, x gamma, remaps, occlude
    c_blend: f32,   // after backdrop + layer_blend
    d_bloom: f32,   // after bloom
    e_tonemap: f32, // after the tonemap shoulder
}
```

## Risks & open questions

- **The bisect names no seam.** Real, and the stop condition is the answer: the five-seam table is
  itself worth having, because it converts "somewhere downstream" into "not at any of these five",
  which is the first thing anyone will have been able to say about this defect since Plan 0100.
- **A fix at seam B or later moves goldens broadly** — every `warp_mesh` baseline at minimum. Budget a
  bless, and **compare adapters before blessing**. This project has twice had WARP produce a
  confidently wrong picture that hardware rendered correctly: once when a new pass's bind-group layout
  aliased a live pipeline's, once when building GPU resources mid-run shifted what the trails stage
  resolved to. Phase 2 adds render targets, which is exactly that shape.
- **Phase 4 may correctly change nothing.** If the handedness agrees, the phase's product is a test
  and a sentence. That is a full phase and must not be padded into a change — "the seam is
  authored-against" is a finding the next three sessions would otherwise re-litigate.
- **Phase 5 may correctly change nothing**, for the same reason, if the constant is not derivable.
- **Two of the four defects may be one.** If the wash lives in the present pass's arithmetic, the
  `ang` seam's visibility could move with it — a seam is only visible against a background. Phase 6
  judges the result, not the phases; do not attribute a pair's improvement to a phase without evidence
  that it was that phase.
- **The wash may not be a single mechanism.** *Contortion* at `fGammaAdj = 1.0` is the standing
  argument that it is not. The plan is built to survive that: the bisect names *a* seam, and if a
  second subject separates at a different seam, that is a second entry rather than a failure.

## What this plan does NOT do

- **Conversion reach.** Backlog [0109](../design-backlog.md) (disk textures — 1 826 files, 88.7 % of
  every conversion failure, and ADR territory because it reopens Plan 0100 Phase 8's deferred
  provenance question) and [0108](../design-backlog.md) (HLSL arrays, ~71 files, plus 218 MD2 presets
  that convert but render blank) both stay filed. The ordering is the entries' own: reach is worth
  buying after quality is judged better, and Phase 6 question 5 is that judgement.
- **It does not reopen** ADR-0118's quantizer or ADR-0119's echo blend. Both were measured, and both
  measurements survived Plan 0109's close review.
- **No new scene, no new param, no C ABI change, no preset content.** `preset-author` has nothing to
  do here; a converted preset is generated, not authored.
- **It does not re-judge the geometry fixes Plan 0109 landed.** That plan's Phases 1, 2 and 3 read as
  fixed on real content and are not in this plan's gate questions except where a pair is shared.

## Contention

`core/src/render/scenes/warp_mesh/**`, `core/src/milk/outputs.rs`, `milkconv/src/convert.rs`,
`milkconv/tests/draw_layer.rs`. **No active plan touches these** —
[0104](0104-the-library-stops-being-lopsided.md) authors `presets/*.toml` (including the four
`warp_mesh` worlds, which read the params this plan may re-mean: if Phase 3 changes what `gamma`
does, tell that lane), and [0106](0106-the-frame-stream-passes-through-a-diffusion-model.md) is
`tools/` + `docs/` only. Safe to run in a worktree lane alongside either.

## Followups (after this lands)

- Whatever Phase 5's x-extent question files, if it turns out to be a second defect.
- Whatever Phase 2's stop branch records, if it stops.
- The reach question, once Phase 6 question 5 has an answer: backlog 0109 wants an ADR and an
  interview, not a phase.
