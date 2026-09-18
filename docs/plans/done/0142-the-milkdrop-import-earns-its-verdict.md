# 0142 — The MilkDrop import earns its verdict

> **Status:** done — closed 2026-09-18. Six phases landed (`20ba8731`, `cc2488cd`, `42b4bb97`,
> `15514a8a`, `7299cc90`, `9d149505`), plus `f43a2257`, the close review's four record repairs.
> Round-1 verdict: **no blockers, no majors, five minors and one nit**, four repaired at the close.
> Phase 2 read the reference's loop at `xeiraex/milkdrop2` `d4c843a` and named two divergences in the
> decay term; Phase 3 repaired both behind ADR-0118's gate, moving two converted goldens and no
> native one; Phase 4's human look gate read the plan's own subject from *washed* to *fixed*; ADR-0113
> carries its third `Outcome` and backlog 0109 its third dated no-go.
> **Created:** 2026-08-29
> **Owner skill(s):** dev, human
> **Related ADRs:** [0113](../../adrs/0113-milkdrop-presets-are-translated-ahead-of-time-onto-a-warp-mesh-idiom.md)
> (accepted — this plan appends its third `Outcome`)
> **Closes:** design-backlog 0113, 0124. **0109 is not taken — this plan decides whether it may be.**
> **Runs after:** [Plan 0180](0180-the-converted-picture-follows-the-source.md), all of it.

> **Amended 2026-09-14**, at a validity sweep of the active roster. Edited in place:
>
> - **Sequencing.** Plan 0180 runs first. It re-draws the waveform, which is a source term of the
>   field this plan measures, and it re-draws the pictures Phase 4 judges.
> - **Phase 1.** It extends the instrument Plan 0111 left in `core/src/render/milk_wash.rs` rather
>   than rebuilding it. It reads that module's `edge` statistic, not `metrics::mean_lit_level`,
>   which excludes the background by design.
> - **Decision.** It names the source commit, `xeiraex/milkdrop2` `d4c843a`. Plan 0173 did not read
>   the feedback loop, so Phase 2's read is still new.
> - **Phase 2.** It names the source's aspect-corrected uv chain as a candidate cause (backlog 0214).
> - **Phase 4.** It gains a "seam present?" column (backlog 0215) and one unit-scale mode-0 capture
>   for ADR-0199.
> - **Phase 5.** Its `Outcome` names backlog 0216's residue if any remains.
> - **Phase 6.** It re-ranks backlog 0108, whose 0106/0107 gate has expired.

> **Amended 2026-09-16, at [Plan 0180](0180-the-converted-picture-follows-the-source.md)'s
> close. That plan landed, and it moved this plan's measurement subject, not just its pictures.**
> The 2026-09-14 amendment above anticipated the waveform being redrawn. Two things it did not:
>
> - **The deposit term is gone from both `milk_wash` fixtures.** Plan 0180 Phase 7 found that
>   `milkconv` emitted a `[params]` comment saying the scene's deposit stayed off and never emitted
>   the key, so `DEFAULT_DEPOSIT = 1.6` laid a ring into every converted preset; it now binds
>   `deposit = "0.0"`, and `milk_wash_fog_tunnel.toml` and `milk_wash_blur_mix_3.toml` bind it too.
>   **Phase 1's settled level is therefore a different number from every reading in this repository
>   that predates 2026-09-16**, and Phase 2's `source / (1 - decay)` arithmetic is taken on a field
>   with one fewer source term. Re-measure; cite no historical figure.
> - **The clean control has no wash left to measure.** The bisect probe now reads
>   `blur mix 3` at **exactly `0.00000000`** at all three seams, where it read
>   `0.0202 / 0.0885 / 0.2521` before. *Fog Tunnel* keeps a residue (`0.1304`, down from `0.2972`),
>   and that residue is backlog 0113 proper. A control that is exactly zero cannot bound anything by
>   ratio, so Phase 1 may need a second control or a stated reason it does not.
>
> Phase 4's rig session also inherits two questions from that plan:
> [ADR-0199](../../adrs/0199-a-converted-waveform-draws-the-sources-figure-at-the-hosts-scale.md)'s
> unit-scale mode-0 capture (its `k` is confirmed on mode 6 alone) and whether the reference shows
> the seam at all.

## TL;DR

`docs/design-backlog.md` entry 0113 is the **only High** in the backlog: the converted feedback field
equilibrates far brighter than the reference's, it is the dominant fidelity defect of the MilkDrop
import, and three hypotheses are already dead. Meanwhile ADR-0113's founding claim — *"the same
preset should look better here"* — has read **"provisionally negative"** since 2026-08-16 on evidence
that is now two plans and four ADRs old, because two look gates ran and neither produced a third
`Outcome`. This plan repairs the wash, then re-takes that verdict, and the verdict is what decides
whether backlog 0109's 1,826-file reach work is worth buying at all.

## Context & problem

These two entries are one plan because **each is the other's blocker.**

Backlog 0124 asks for a third `Outcome` on ADR-0113 and says honestly that a re-take today *"may
honestly still read merely different, with the wash dominating"* — because backlog 0113 is live,
having survived Plan 0111's bisect and reversed back to the field, and three of 0109 Phase 5's seven
pairs still read washed. So the verdict cannot be taken cleanly while the wash stands.

And backlog 0109 — disk textures, **1,826 files, 88.7 % of every conversion failure** — carries an
explicit ordering instruction: *"Do not take it before Plan 0108's Phase 2, whose verdict on whether
these presets read as better or merely different is exactly the evidence for how much reach is
worth."* That verdict has been taken twice and both times read **"still merely different"**. So the
reach work's own precondition is currently **unmet**, and the thing that would change it is the wash.

That chain is the plan: **fix the wash → re-take the verdict → the verdict decides 0109.**

What is already known about 0113, so nobody re-runs it: the warp pass has no mechanism bounding the
field's equilibrium level, only a per-frame decay and a ceiling clamp. Plan 0111 built an instrument,
ruled the field clean, and killed three hypotheses; the defect reversed back to the field afterward.
The evidence is seven side-by-side pairs against `foo_vis_milk2` 0.2.0.0 (DX11), recorded in Plan
0108's look-gate section — **it lives in no greppable line**, which is why the entry carries an
`unprobeable:`.

## Decision

**Instrument first, diagnose second, repair third, and only then judge.** Phase 1 extends the
equilibrium instrument Plan 0111 left in `core/src/render/milk_wash.rs`, and Phase 2 reads the
source against it. Three hypotheses died at single seams already. Phase 3 is the repair. Phase 4 is the look gate against the
reference rig. Phase 5 writes ADR-0113's third `Outcome` — **which is a legitimate deliverable even
if the answer is still "merely different"**, dated and naming what remains, rather than leaving
2026-08-16's silence to stand for it. Phase 6 records the go/no-go for backlog 0109.

We rejected taking 0109 in this plan. Its own entry forbids it before the verdict, it *"wants an ADR
and an interview rather than a phase"*, and its four routes differ on a provenance question Plan 0100
Phase 8 deferred — *decide later, nothing third-party in the repository or a release* — which is the
same decision seen from two sides, since a texture is third-party content exactly as a preset is.

**Revised 2026-09-11: Phase 2 reads the source before it infers.** MilkDrop 2's source was released
under BSD-3-Clause on 2013-05-13 and is public, with the feedback loop in `vis_milk2/milkdropfs.cpp`.
Every earlier attempt on 0113 inferred the reference's behaviour from pictures, because no source was
reachable. Phase 2 now derives the equilibrium the reference's own warp, decay, echo and gamma path
implies for a preset with no warp shader (all five washed presets are that kind), and compares it with
our measured field — *Fog Tunnel*'s background reads 0.298 linear at the field. It repairs only a
divergence that arithmetic names, and stops as written if there is none. Nothing from the source is
copied into the repository.

**The commit is named.** [Plan 0173](0173-the-milkdrop-geometry-reads-the-source.md) read
`xeiraex/milkdrop2` at `d4c843a` (v2.25c) for the mesh's `ang` and the waveform, and this plan reads
the same commit. **Plan 0173 did not read the feedback loop**, meaning the decay, echo, gamma and
their order and domain. Its log says so, and Phase 2's read is still new work.

**Plan 0180 runs first, and why (added 2026-09-14).** That plan makes three changes:

- It re-draws the built-in waveform from the source.
- It aspect-corrects the per-vertex `x`/`y` a converted program reads.
- It gives the comp stage the source's polar pair.

The waveform is a **source term** of the equilibrium this plan computes. `milk_wash.rs` renders *Fog
Tunnel* (mode 0) and *Blur Mix 3* (mode 6) on a silent frame, where mode 0's resting circle still
deposits light. And every change above moves the pictures Phase 4 judges. Plan 0180 needs no rig, so
the ordering costs this plan no rig time.

## Architecture diagram

```mermaid
flowchart LR
    subgraph conv["milkconv/ — ahead of time"]
        MILK[".milk source"] --> EMIT["shader/emit.rs"]
    end
    subgraph rt["core/src/milk/ + warp_mesh — per frame"]
        VM["bytecode VM"] --> WARP["warp pass"]
        WARP -->|"per-frame decay<br/>+ ceiling clamp"| FIELD["feedback field"]
        FIELD -->|"NO mechanism bounds<br/>the EQUILIBRIUM level"| FIELD
    end
    EMIT --> VM
    FIELD --> OUT["rendered frame"]
    REF["foo_vis_milk2 0.2.0.0 (DX11)<br/>the reference rig"] -.->|"seven side-by-side pairs —<br/>the ONLY evidence, ungreppable"| OUT
    OUT --> V{"Phase 5: third Outcome<br/>better, or merely different?"}
    V -->|better| REACH["backlog 0109 unlocked<br/>1,826 files — its own plan + ADR"]
    V -->|still merely different| HOLD["0109 stays unbought<br/>and the entry says why"]
```

## Implementation phases

### Phase 1 — The equilibrium instrument, across the whole chain
- **Owner skill:** dev
- **What:** Extend the instrument Plan 0111 left behind so it measures the settled field level at
  every seam of the chain, for a washed subject and a clean one, on the tree Plan 0180 produced.
- **Files touched:** `core/src/render/milk_wash.rs` (the three-seam bisect,
  `the_wash_bisect_reports_every_seam`); its fixtures `core/tests/fixtures/milk_wash_fog_tunnel.toml`
  and `milk_wash_blur_mix_3.toml`; `FieldTrace` in `core/src/render/scenes/warp_mesh/tests.rs`, the
  per-frame field probe.
- **Notes for the implementer:**
  - **The instrument exists, and so does the pair.** `milk_wash.rs` already renders the washed *Fog
    Tunnel* and the clean control *Blur Mix 3* for 300 frames, three of the ~100-frame time
    constants Plan 0111 Phase 1 measured. It reads one statistic at every seam that exists for these
    subjects: the field, the present pass and the display. Its module docs say why the backdrop and
    bloom seams collapse for them. **Extend it; do not rebuild it.** Read its dated table and the
    Plan 0111 log first, and record which seams it already covered.
  - **Its readings predate Plan 0180.** That plan re-draws mode 0's resting circle, which these
    fixtures deposit even on a silent frame. Re-take the table rather than citing the 2026-08-19
    one.
  - Measure in **linear light**, not code values, **with `milk_wash`'s `edge` statistic**, the mean
    over the outermost ring of texels of an `Rgba16Float` intermediate. Do **not** use Plan 0137's
    `metrics::mean_lit_level`: it decodes 8-bit captures and excludes the background by design, and
    its own doc states the blind spot, *"a preset that goes wrong by changing its background is
    invisible here"*. The wash is a background defect in a float field. `edge` has its own caveat
    for *Fog Tunnel*, since it may sample the solid tube that is the defect. Carry that caveat
    forward rather than dropping it.
  - The defect is an **equilibrium**, not a frame: the field converges to the wrong level over time.
    A single-frame measurement is what makes a seam look clean, so the instrument must report a
    settled level over many frames.
  - Pair a washed preset with one that reads correctly. A measurement with no control cannot separate
    "this seam is bright" from "this preset is bright".
- **Done when:** the instrument reports a settled field level at each seam for both a washed and a
  clean preset, and the table is in the implementation log.

### Phase 2 — Name the mechanism
- **Owner skill:** dev
- **What:** Identify what sets the equilibrium, or state precisely that the instrument cannot see it.
- **Files touched:** `docs/design-backlog.md` (a dated update on 0113).
- **Notes for the implementer:**
  - **Start from the reference's source** (revised 2026-09-11, see Decision). Write down its per-frame
    arithmetic for the built-in warp path — decay, echo, gamma, the order they apply in and the domain
    each multiplies in (8-bit encoded or linear) — and the equilibrium it implies for *Fog Tunnel*'s
    bundle. Compare that with Phase 1's settled level. A divergence the arithmetic names is Phase 3's
    target; no divergence is the honest stop this phase already allows. Cite file, function, line and
    commit, and copy nothing.
  - **A named candidate that is not level: the uv chain's space.** The source runs zoom, `sx`/`sy`,
    rotation and `dx`/`dy` in an aspect-corrected space it undoes at the end
    (`milkdropfs.cpp` `CPlugin::ComputeGridAlphaValues`, l.1839-1916, backlog 0214). *Fog Tunnel*'s
    defect reads as a solid tube where the reference draws discrete rings, and that could be
    geometry as easily as brightness: a resample landing on the wrong texels fills the gaps between
    rings. Plan 0180 Phase 1 records whether this engine's chain (`vs_main` in
    `core/src/render/scenes/warp_mesh/shaders.rs`) matches, and Phase 3 repairs any stage it names.
    Read that log before attributing the tube to level. The per-frame `decay` exponent is applied in
    `upload_uniforms` (`warp_mesh/encode.rs`) since the scene's `mod.rs` was split on 2026-09-02.
  - The known fact is that only a per-frame decay and a ceiling clamp exist — **nothing bounds the
    equilibrium level**. A decay plus a source term has an equilibrium at `source / (1 - decay)`, so
    the candidates are the decay's units, the source's scale, or the clamp interacting with both.
  - Backlog 0121 (closed) found MilkDrop's `decay` read as a per-second value when it is per-frame —
    **that was corrected, and it silently corrupted Plan 0109 Phase 4's own instrument and every
    measurement before it.** Any historical number predating that fix is suspect; re-measure rather
    than citing.
  - **A phase that ends "the instrument cannot see it" is a legitimate outcome** and stops the plan
    honestly at Phase 5, which then writes an Outcome saying the claim is not yet answerable. That is
    better than a speculative repair.
- **Done when:** backlog 0113 carries a dated update naming the mechanism with the measurement behind
  it, or stating what was ruled out and what instrument would be needed next.

### Phase 3 — Bound the equilibrium
- **Owner skill:** dev
- **What:** Repair the mechanism Phase 2 named.
- **Files touched:** `core/src/render/scenes/warp_mesh/`.
- **Notes for the implementer:**
  - **Runs only if Phase 2 named a mechanism.** If it did not, skip to Phase 5 and say so.
  - This moves converted-preset output by design, so **the goldens covering `warp_mesh` will move**.
    Bless deliberately and state which baselines moved and why. Nothing outside `warp_mesh` should
    move; anything that does is a finding.
  - `warp_mesh` ships no preset of its own, so the visible surface is converted `.milk` content plus
    whatever fixture the suite uses — check what the golden set actually covers before assuming a
    moved baseline is expected.
- **Done when:** the washed pairs' settled field level lands within the reference's, measured by
  Phase 1's instrument, and the moved goldens are blessed with reasons.

### Phase 4 — The look gate
- **Owner skill:** human
- **What:** Re-run the seven side-by-side pairs against the reference rig.
- **Files touched:** none.
- **Notes for the implementer:**
  - The rig is `foo_vis_milk2` 0.2.0.0 (DX11), and it reads only from
    `%APPDATA%\foobar2000-v2\milkdrop2\`. The same seven pairs, the same rig — comparability with
    0108's and 0109's gates is the whole value, so **do not change the pair set**.
  - 0109 Phase 5 read three of seven as **fixed**, including the portal and *Blur Mix 3*'s traces,
    and three still washed. Those three are the ones this plan is about.
  - **This needs a free machine and the rig staged.** Not a show-night task.
  - Record per-pair verdicts, not an overall impression — the per-pair table is what Phase 5 writes
    its Outcome from.
  - **Add a "seam present?" column** for *Aderrasi - Songflower (Moss Posy)* and *Eo.S. + Phat -
    chasers 19 Portal*, in both renderers (backlog 0215). Plan 0180 Phase 4 owns the diagnosis and
    has already located the seam's ray on this engine. What only this session can say is whether the
    reference shows the same seam: a left-edge seam in both is authored-against. Record it; do not
    diagnose here.
  - **One extra capture for ADR-0199:** a purpose-authored unit-scale `nWaveMode = 0` preset
    (`fWaveScale = 1`, `fWaveSmoothing = 0`, neutral warp, thin white line on black), on the same
    full-scale 200 Hz sine Plan 0127 used for mode 6. Record the circle's radius swing in frame
    heights. It confirms or refutes that one host factor, fitted on mode 6, carries to the other
    modes. It is recorded here and written into ADR-0199 as an `Outcome`; it is not a verdict on
    this plan's pairs.
- **Done when:** a per-pair table exists for all seven, comparable to 0108's and 0109's, with the
  seam column filled for the two seam presets and the mode-0 reading recorded.

### Phase 5 — ADR-0113's third Outcome
- **Owner skill:** dev
- **What:** Close backlog 0124. Append a dated `Outcome` to ADR-0113 recording the current verdict on
  its founding claim.
- **Files touched:** `docs/adrs/0113-milkdrop-presets-are-translated-ahead-of-time-onto-a-warp-mesh-idiom.md`.
- **Notes for the implementer:**
  - **This phase runs whatever Phases 2-4 produced.** Backlog 0124 is explicit: a re-take that still
    reads *"merely different, with the wash dominating"* is *"a perfectly good third Outcome"* — dated,
    naming what remains, and saying the claim is not yet answerable rather than leaving silence.
  - The ADR is accepted and **append-only**: a dated `Outcome` section, never an edit to the body.
    That is the ADR-0054 / ADR-0074 precedent.
  - Quote the load-bearing sentence being updated — *"merely different, not better"* — so a reader
    sees what moved.
  - `dev` writes the section; **the verdict itself is the user's from Phase 4.** Do not invent one.
  - **Name what is still different that is not the wash**, so a "merely different" verdict is not
    blamed on the wash alone. That means whatever Plan 0180 left open or stopped on (backlog 0216's
    waveform contract, including the mono stand-in if its Phase 5 stopped; 0215's seam if its ray
    matched neither branch), plus anything the seam column or the mode-0 capture turned up.
- **Done when:** ADR-0113 carries a third dated `Outcome` citing Phase 4's per-pair table, and
  backlog 0124's premise — that no gate produced one — is false.

### Phase 6 — The reach decision
- **Owner skill:** dev
- **What:** Record whether backlog 0109 is now buyable.
- **Files touched:** `docs/design-backlog.md`, `docs/plans/README.md`.
- **Notes for the implementer:**
  - 0109's precondition is a verdict that converted presets are **worth having more of**. Two gates
    have said "still merely different"; Phase 4 is the third.
  - **If the verdict is better:** 0109 is unlocked and wants **its own plan with an ADR and an
    interview** — its four routes (user's own `textures/` directory; procedural substitution; a
    curated shipped set; keep the exclusion and stop calling it a corner) differ on the provenance
    question Plan 0100 Phase 8 deferred, not on mechanism. Do not start it here.
  - **If the verdict is still merely different:** say so on 0109 with the date, so the third
    "unbought" is recorded rather than the entry looking merely un-picked-up.
  - Either way, note that 0109 sits **above** backlog 0108 by its own arithmetic — ~1,826 files
    against ~71, a 25x difference — so if reach is ever bought, this is the one to buy.
  - **Re-rank backlog 0108 in the same edit.** Its priority line reads Low "until 0106/0107 land",
    and both are archived, so that gate has expired and the line points at nothing. Give it a dated
    update tying its priority to this phase's verdict, behind 0109. Its 218 "convert but render
    blank" count predates every fidelity plan since and is un-recounted; say so rather than restating
    it.
- **Done when:** backlog 0109 carries a dated go/no-go with the verdict behind it, backlog 0108 a
  dated re-rank against it, and the plans README's MilkDrop sequencing note reflects both.

## Risks & open questions

- **Phase 2 may not name a mechanism**, and this is the likeliest way the plan underdelivers. Three
  hypotheses are already dead and the field was ruled clean once before reversing back to it. The
  plan is built to stop honestly at Phase 5 rather than ship a speculative repair.
- **The only evidence is a human look against an external reference**, which no CI can run and no
  probe can hold — hence 0113's `unprobeable:`. Every verdict here is a judgement, and the mitigation
  is that the pair set and rig are fixed so verdicts are comparable across four gates.
- **Any measurement predating backlog 0121's fix is suspect.** The `decay` units bug corrupted Plan
  0109 Phase 4's own instrument. Re-measure; do not cite historical numbers.
- **Phases 1, 3 and 4 need a free GPU and the reference rig staged.** This is the least
  show-compatible plan on the roster.
- **Starting before Plan 0180 closes takes Phase 1's table on figures that plan will change.** If it
  happens anyway, the log says so, and Phase 1 re-takes the table after 0180 lands.
- **A "still merely different" verdict is a real possible outcome of the whole plan**, and it would
  leave the import's founding claim unvindicated after five plans. That is information worth having,
  and it is what Phase 5 exists to record.

## What this plan does NOT do

- **It does not take backlog 0109.** Phase 6 decides whether it may be taken; the work itself is a
  separate plan with an ADR and an interview.
- **It does not take backlog 0108** (the conversion tail — HLSL arrays, ~71 files, and 218 MD2
  presets that convert but render blank). It is 25x smaller than 0109 by 0109's own arithmetic and
  waits behind it.
- **It does not reopen ADR-0113's translation approach.** Phase 5 records the verdict on its
  motivating claim; superseding the decision would be a new ADR and is not in scope.
- **It does not change the converter.** Everything here is runtime — `core/src/milk/` and
  `warp_mesh` — not `milkconv/`. The one converter fidelity fix in view, the comp stage's `rad`/`ang`
  (backlog 0214), is Plan 0180 Phase 2.
- **It does not repair the seam or the waveform** (backlog 0215, 0216). Plan 0180 owns both; this
  plan only records what the reference shows.

## Implementation log

> Written by `dev` — one row per phase as that phase's commit lands, and the close block after the
> last one. **The phases above are the contract; everything here is what happened.**

**Lane:** `WORK/rlx-plan-0142`, on `plan-0142-the-milkdrop-import-earns-its-verdict`

| phase | owner | state | commit |
|---|---|---|---|
| 1 — The equilibrium instrument | dev | done | `20ba8731` |
| 2 — Name the mechanism | dev | done | `cc2488cd` |
| 3 — Bound the equilibrium | dev | done | `42b4bb97` |
| 4 — The look gate | human | done | `15514a8a` |
| 5 — ADR-0113's third Outcome | dev | done | `7299cc90` |
| 6 — The reach decision | dev | done | `9d149505` |

### Phase 1 — the re-taken table

Dev box (hardware adapter), 128x128, `AnalysisFrame::default()`, quantizer at
`DEFAULT_QUANTIZE_STEPS = 255`. `edge` in linear light at A and B, display-referred at E. `settled`
is the mean over f100/f200/f300 and `spread` its half-spread as a fraction of that mean.

```text
  subject      seam               f30          f100          f200          f300       settled  spread
  fog tunnel   A field     0.09276785    0.13548748    0.14085685    0.13043343    0.13559258   3.84%
  fog tunnel   B present*  0.17703269    0.25443807    0.26188728    0.24317567    0.25316700   3.70%
  fog tunnel   E display   0.33025211    0.40570471    0.41156110    0.39624831    0.40450469   1.89%
  blur mix 3   A field     0.00000000    0.00000000    0.00000000    0.00000000    0.00000000   0.00%
  blur mix 3   B present*  0.00000000    0.00000000    0.00000000    0.00000000    0.00000000   0.00%
  blur mix 3   E display   0.00000000    0.00000000    0.00000000    0.00000000    0.00000000   0.00%

  present-pass gain B/A on the settled level: fog tunnel 1.867, blur mix 3 n/a
  * B is also seams C and D: no post stage is active and the backdrop is unbound
```

Seams covered before this phase: A, B (collapsed with C and D) and E, each at one frame count
(300). What the phase added is the checkpoint set, the settled band and its spread, the transient
probe at f30, and the present-pass gain.

- **The washed level is not stationary at any frame.** f200 is the highest of the three band
  readings, so the residual is not a residual climb; the band is +-3.8 % at the field.
- **The control reads exactly `0.00000000` at every seam and every checkpoint**, f30 included, so
  the washed/control ratio the earlier bisect was built on has no value at any seam.

### Phase 3 — the same table, after the repair

Same box, same fixture, same statistic as Phase 1's table above.

```text
  subject      seam           Phase 1        Phase 3   ratio
  fog tunnel   A field     0.13559258     0.08623820   1.572
  fog tunnel   B present*  0.25316700     0.16459735   1.538
  fog tunnel   E display   0.40450469     0.32970616   1.227
  blur mix 3   every seam  0.00000000     0.00000000   n/a

  present-pass gain B/A on the settled level: 1.867 -> 1.909
```

The undeposited-fade probe, in the reference's own encoded domain over the same
2.650 s window: `0.3034` before, `0.1948` after, against the reference's
arithmetic `0.2007` — **-2.9 %**, where a pure-linear multiply predicts `0.4819`.

### Phase 4 — the look gate, as run (2026-09-18)

`foo_vis_milk2` 0.2.0.0 (DX11) in foobar2000 v2 beside this lane's release build (`42b4bb97`,
reported as 0.129.0), one track through foobar2000 feeding both, ours taking it over loopback at
164-165 fps. The seven were re-converted by `milkconv` at `main` 0.131.0 into `WORK/rlx-gate-0142/`
and loaded through `RLX_PRESET_DIR`; that directory also holds the fixture and stimulus below and is
outside the repository, as Plan 0127's material is. **Verdicts are the owner's, taken live.**

| pair | verdict | what dominates | seam |
|---|---|---|---|
| *chasers 19 Portal* | washed | ground saturates to near-white; the traces survive under it | **none in either** |
| *Blur Mix 3* (control) | washed | traces horizontal and correct; ground grey with blown blobs, not black | n/a |
| *Songflower (Moss Posy)* | wrong, and not on brightness | the reference's woven lattice is **absent**: ours draws the bare grid with no nesting | **none in either** |
| *Cauldron painterly 5* | better, centre blown | terrain and spiro both read; the core clips white |  n/a |
| *Contortion (Escher's Tunnel Mix)* | good | sphere, tunnel and arcs all read; ground comparable | n/a |
| *Cosmic Dust 2* | good, ours darker | ours is **sparser** than the reference rather than brighter | n/a |
| *Fog Tunnel* | **fixed** | black ground, tube reads against the reference's structure | n/a |

**The plan's own subject moved:** *Fog Tunnel* read "still washed" at both earlier gates and now
reads fixed. **Hue is ruled out of every verdict** — both renderers animate their palettes off their
own clock, so two stills sit at different palette phases; that is why *Contortion* and *Cosmic
Dust 2* read good against obviously different colour.

**The mode-0 capture (ADR-0199) does not confirm what that ADR asks it to.**
`!LMV-0142-mode0-unit.milk` is `!LMV-0127-A-crisp.milk` with `nWaveMode` 6 -> 0 and nothing else
moved, over a 60 s full-scale 200 Hz sine verified at 0.0 dBFS peak / -3.0 dB mean, at 2000x1125 —
Plan 0127's own size. Radius is taken about the trace's own bounding-box centre, because the figure
translates between frames, over 720 angle bins. Swing at thresholds 140 and 200: **0.1366 / 0.1364 H
over a 0.3055 H base radius** (the reading of record), corroborated by a second capture at
0.1295 / 0.1288 H over 0.3067 H — stable to ~0.5 % on both. For `r(theta) = R0 + k*s(theta)` with
`s` in `[-1, 1]` the swing is `2k`, so **`k ~ 0.068`** against the **`0.158`** Plan 0127 derived from
mode 6 on this same rig and stimulus, a ratio of **0.43**. ADR-0199's Negative calls modes 0-5
sharing mode 6's gap *"an inference from where the gap lives, not a measurement"*; this does not
support it. **Bound:** MilkDrop gains the waveform before drawing, so full-scale input is not
necessarily a unit sample at the draw call — which weakens the absolute `k`, not the ratio.

### Notes

- **Phase 3's done-when is not measurable as stated, and what was measured
  instead.** It reads *"the washed pairs' settled field level lands within the
  reference's, measured by Phase 1's instrument"*, and there is no instrument on
  the reference: it is an external renderer, which is why backlog 0113 carries an
  `unprobeable:`. What is measurable is the loop's own per-frame factor, and that
  is what the repair matches — `the_field_fades_at_the_references_own_rate`
  asserts the field's fade against `d^T`, the reference's arithmetic, and
  `the_converted_decay_is_truncated_on_the_nominal_frame` asserts the factor
  itself against `(int)(fDecay*255)/255` at seven authored values. The settled
  levels are reported above rather than asserted.
- **Phase 2's arithmetic predicted a `2.6x` to `7.0x` fall at the field and the
  instrument read `1.572x`.** The prediction takes the equilibrium as
  `s / (1 - d)`, which assumes the warp resample's dominant eigenvalue is 1 — true
  of a still field, not of this subject. Solving the measured ratio for it gives
  about `0.948` at the edge ring, which is what *Fog Tunnel*'s `zoom = 1.042`
  does to light there. The mechanism and the direction stand; the magnitude was an
  upper bound on a still field and is not a prediction for this preset.
- **The repair's gate is `quantize_steps`, not the presence of a bundle**, and the
  first attempt used the latter. Under it both arms of
  `the_field_equilibrates_only_when_the_quantizer_runs` moved — its unquantized
  control converged (`0.2586` at f120 to `0.3553` at f300, under the probe's
  `1.5x` bar), so the probe would have been asserting that a field with an
  equilibrium has none. On `quantize_steps` both probes' OFF arms are unchanged to
  the digit against their recorded tables, and only the ON arms move.
- **Two comment blocks in `warp_mesh/tests.rs` were rewritten beyond the
  repair.** The first is the *"Dead hypothesis: the decay multiply's domain"*
  paragraph, which the repair makes false. The second is the *"live hypothesis"*
  paragraph beside it, which claimed the shader-`decay` gap *"predicts the look
  gate's own pattern: the five washed presets are shader presets"* — backlog
  0113's own 2026-08-19 census already recorded the inverse, and the source read
  settles it (`WarpedBlit_Shaders` applies no host decay either, so that path is
  not a divergence from the reference).
- **The converted-warp-shader path was not touched**, and it carries the same
  domain question: a preset whose HLSL says `ret *= decay` multiplies linear light
  here and encoded values there. It reaches 1 253 corpus files, none of them among
  the seven pairs, and it is noted under Followups rather than repaired.
- **Phase 1 took no second control**, which the 2026-09-16 amendment left as a choice against a
  stated reason. The reason is in `milk_wash.rs`'s module docs: a control at exactly zero rules an
  *additive* stage out of the whole chain, which is stronger than a ratio, and leaves a
  *multiplicative* stage invisible — bounded instead by the washed subject's own seam-to-seam gain,
  now printed. A third fixture would also not be one of the seven pairs the look gates judge.
- **`FieldTrace` in `warp_mesh/tests.rs` was listed under Files touched and was not changed.** It is
  the synthetic per-frame probe driven by an empty bundle, and nothing the re-taken table needed
  reached it.
- **Phase 2's dated update went to `docs/design-backlog-archive.md`, not to the
  `docs/design-backlog.md` its `Files touched` names.** Backlog 0113's body moved to the archive on
  2026-09-15 on promotion (ADR-0206), the day after the amendment that wrote that line; the live
  file carries no 0113 body to update. Nothing was added to the live file.
- **Phase 2 named a mechanism, so Phase 3 runs rather than skipping to Phase 5.** The reading is
  backlog 0113's `### Update 2026-09-17` in the archive: the reference's per-frame arithmetic with
  file, function and line, the two terms of the divergence, the bound each puts on it, and the one
  source-scale term the arithmetic does not settle.
- **`git` outside the lane is denied to this session**, so the source checkout's commit was not
  re-verified here; the read is of the tree at `WORK/milkdrop2-src`, which the owner's session
  recorded as `d4c843a`. The rig the look gate uses is the later `foo_vis_milk2` 0.2.0.0 DX11 port
  of the same project, and whether that port kept `D3DCOLOR_RGBA_01`'s truncation is not readable
  from this tree.

- **Phase 4 — what the session could not settle, with the test that would.** Ours ran at 164-165 fps
  against the rig's own frame cap. Transforms convert per second and the owner read the motion as
  matching, which tests that conversion; the **deposit is per frame and is not converted**, so a rate
  mismatch raises the field by `1/(1 - d)` — hardest on high-`fDecay` presets, which is the shape of
  the result above. Not settleable by eye: render one washed preset through `shot --render` at
  `--fps 30` and at `--fps 165` and compare the ground level.
- **Phase 4 — *Songflower*'s defect is not this plan's.** It sets `fDecay = 1.000`, so no decay runs
  and Phase 3's repair cannot reach it. It sets `fVideoEchoAlpha = 1.0` with `echo_zoom` and
  `echo_orient` driven from its per-frame code, and the reference's nested weave is that echo
  compositing the previous frame zoomed and flipped. The engine carries `echo_alpha`/`echo_zoom`/
  `echo_orient` as params, so the reading is "the echo is bound and is not producing the nesting" —
  a different finding from Plan 0108's "no video-echo stage".
- **Phase 4 — the seam question is answered negatively for both presets** (backlog 0215): none in the
  reference and none in ours. Plan 0180 Phase 4 located a ray on our side; nothing shown here had one.
- **Phase 5 — the mode-0 capture was not written into ADR-0199, and no phase's `Files touched` allows
  it.** Phase 4's note says the reading "is recorded here and written into ADR-0199 as an `Outcome`";
  Phase 4 is `human` with `Files touched: none` and Phase 5's list is ADR-0113 alone. ADR-0113's
  third `Outcome` carries the reading and the `0.43` ratio, so it is on the record; ADR-0199 still
  says its `k` rests on one mode and asks Plan 0142 for the confirmation. Under Followups.
- **Phase 5 — *Blur Mix 3* is read as the gate read it, and that is not reconciled with Phase 1's
  instrument.** It was the control at both earlier gates (Plan 0100: *"the one pair whose tone
  survived looked genuinely good"*; Plan 0109 Phase 5: traces fixed) and this gate reads its ground
  washed, while `milk_wash_blur_mix_3.toml` reads exactly `0.00000000` at every seam. The two are not
  the same subject — the fixture is a 128x128 silent frame of a cut-down bundle, the gate is the
  whole converted preset on a track — so nothing here contradicts, and nothing here explains it
  either. The Outcome states the verdict and does not attempt the reconciliation.
- **Phase 6 — backlog 0108's expired priority line is left standing with the re-rank appended under
  it**, rather than rewritten. The phase asks for "a dated update tying its priority to this phase's
  verdict"; the file's own form is dated updates that supersede, and the new one says in its first
  sentence that it replaces the line above as the live priority.
- **The close block rode inside Phase 6's commit** (`9d149505`) rather than following it as its own
  `docs(plans):` commit; the commit that carries this line backfills Phase 6's SHA and nothing else.

### Close triggers

- **`presets/` touched:** no. No file under `presets/` changed in any phase.
- **Plan header `Closes:`** design-backlog 0113 and 0124. Both bodies are in
  `docs/design-backlog-archive.md`, archived as **Promoted** on 2026-09-15 (ADR-0206), so neither is
  live, neither carries a probe and neither moves at the close; the `CLOSED` markers are step 3c's.
  0113's archived body gained Phase 2's `### Update 2026-09-17`. **0109 is not taken.** It and 0108
  are live, and each carries a dated Phase 6 update in `docs/design-backlog.md`.
- **What shipped:** a fix to what a converted preset renders, plus documentation. Phase 3
  (`42b4bb97`) changed the factor and the domain the built-in warp fragment decays in, gated on
  `quantize_steps`, and re-blessed `core/tests/golden/warp_mesh_milk.png` and
  `warp_mesh_stroke.png` — the only two baselines that moved. No native preset's output moved, no
  new surface was added, and Phases 1, 2, 4, 5 and 6 are instrument, docs and ADR text.
- **Operator docs touched:** none. `docs/milkdrop-conversion.md` was not edited; its rate section
  and Phase 3's truncation are under Followups.
- **Backlog probes (`node scripts/check-backlog-claims.mjs`):** exit 0 — 89 stated reductions across
  37 live entries, 3 unprobeable, 36 advisory "path moved" rows. It names 0109 twice among those
  advisories (`milkconv/src/shader/emit.rs`, `docs/milkdrop-conversion.md`), both stamped 2026-08-17
  and both moved by plans before this one.
- **The other Node gates, this session:** `check-doc-links.mjs`, `toc.mjs --check`,
  `check-index-rows.mjs` and `check-reader-prose.mjs` all exit 0.
- **Full suite:** owed to the conductor's pre-review gate (ADR-0207). No suite ran in this session —
  Phases 5 and 6 changed no code. The upward override ADR-0156 allows was taken at Phase 3, whose
  commit carries the two re-blessed goldens.
- **Outstanding `human` phases:** none. Phase 4 is the plan's only `human` phase and it ran
  2026-09-18; its table is above, committed in `15514a8a`.

## Close review

> Mode 4, conductor mode (ADR-0205), **round 1**, run 2026-09-18 in a fresh session handed the plan
> and the lane and nothing an implementer wrote outside the repository. No earlier round, so no
> carried findings. Reviewed `20ba8731..7bbf52bc` (9 commits): `git diff main...HEAD` = 11 files,
> +905 / -129.

### Verdict

**Plan 0142 landed cleanly: no blockers, no majors, five minors and one nit.** The plan's central
claim is carried. Phase 2 read the reference's own source and named two divergences in one term;
Phase 3 repaired both behind ADR-0118's own gate, with the native path left bit-identical by
construction rather than by tolerance; Phase 4's human look gate ran and moved the plan's own
subject from *washed* to *fixed*; Phase 5 wrote the third `Outcome` the plan exists to produce, and
it records "not better" honestly rather than reading the repair as a verdict. Every finding below is
documentation accuracy or test strength — none of them changes what the engine renders.

### Evidence this review ran on

- **Full suite.** `node tools/conductor/with-lock.mjs suite -- cargo nextest run --workspace` printed
  the ledger record rather than re-running (ADR-0207):
  `with-lock: skipped cargo nextest run --workspace: tree 737b6b8 is green in the suite ledger, run
  by gate 0142-pre-review at 2026-09-18T14:02:41.357Z: 1988 tests run: 1988 passed (6 slow), 6
  skipped`. That record is the full-suite evidence for lens 1. The log's `Full suite:` bullet says it
  is owed to the conductor's pre-review gate, which is correct in this mode and not a missing run.
- **Docs.** `cargo --config "build.rustdocflags=['-D','warnings']" doc --workspace --no-deps` — exit
  0 across all five crates. (The `RUSTDOCFLAGS=` form is unavailable to a conductor session —
  design-backlog 0250 — so the flag is passed through `--config`.)
- **Node gates.** `check-doc-links.mjs`, `check-index-rows.mjs`, `check-comment-hygiene.mjs`,
  `check-reader-prose.mjs`, `check-backlog-claims.mjs`, `check-translations.mjs` and
  `toc.mjs --check` — all exit 0.
- **One instrument re-run by hand**, to check a table this review suspected:
  `... suite -- cargo nextest run -p rlx-core --no-capture -E "test(the_wash_bisect_reports_every_seam)"`.

### Lens 1 — alignment with the plan and the ADR

Every phase carries exactly one in-vocabulary `**Owner skill:**` tag (`dev` x5, `human` x1). No
blocker here.

The log's phase-to-commit table is accurate: all six SHAs are on the branch and each touches what
its row claims. Three deviations from `Files touched` are disclosed by `dev` rather than found here,
and all three are correct calls — `FieldTrace` was listed and not needed; Phase 2's dated update
went to `design-backlog-archive.md` because 0113's body moved there on promotion (ADR-0206) and the
live file carries no 0113 body; the close block rode inside Phase 6's commit.

**The tests the plan named, read rather than trusted.**

- `the_wash_bisect_reports_every_seam` (`core/src/render/milk_wash.rs`) is genuinely extended, not
  rebuilt: four checkpoints where there was one frame, a settled band with a half-spread, a
  transient probe and the present-pass gain. It asserts **no** threshold on any level (correct under
  ADR-0071 — the level is what the phase measures), and what it does assert is that the instrument
  reports an equilibrium: every seam finite at every checkpoint, the band inside `SETTLED_SPREAD`,
  and the transient larger than the band. That last assertion is the one that would catch "settled"
  being a slow climb, and it is the right one.
- `the_field_fades_at_the_references_own_rate` and
  `the_converted_decay_is_truncated_on_the_nominal_frame` are the two gates on Phase 3. The second
  is exact and correct: a round trip at seven authored `fDecay` values against
  `(int)(fDecay*255)/255`, an identity check on the off arm, and the `42.5` equilibrium gain that is
  the point of the repair. The first has a real weakness — finding **M2** below.
- **Phase 3's done-when is not measurable as stated**, and `dev` says so in the log instead of
  quietly satisfying something else. That is the right disposition: there is no instrument on an
  external renderer, which is why backlog 0113 carries an `unprobeable:`. What was substituted — the
  loop's own per-frame factor, asserted against the reference's arithmetic — is the measurable
  restatement, and the settled levels are reported rather than asserted.

**Ruling out a silent ADR reversal.** ADR-0118's quantizer is not widened: the repair is gated on
`quantize_steps`, and a preset with no bundle gets `0.0`, so every native preset takes the untouched
expression. ADR-0019's per-second vocabulary is preserved — `milk_decay` takes the factor back to
the nominal frame, truncates, and returns it per second, which is exactly what stops the display's
refresh entering the equilibrium. Nothing from `xeiraex/milkdrop2` is copied into the repository;
the Phase 2 read cites file, function and line throughout.

**Log length.** `## Implementation log` is 218 lines against `## Implementation phases`' 154. That is
finding **M5**.

### Lens 2 — layering, coupling, real-time safety

Nothing to report. No platform, audio-source or windowing type enters `core/`; the diff touches no
capture path, no C ABI surface (spec 0001 unchanged) and no OSC address (spec 0003 unchanged). No
allocation, lock or logging is added on any audio path — the diff is a shader, one pure function, a
test instrument and prose. `milk_decay` is total: `powf` on a clamped non-negative factor, with an
explicit early return for the off gate, and no `unwrap`/`expect` outside `#[cfg(test)]`.

### Lens 3 — doc freshness and release bookkeeping

- **ADR-0113 carries its third `Outcome`**, dated, quoting the sentence it updates
  (*"merely different, not better"*), naming the per-pair result and — the part that makes it worth
  having — discharging the previous `Outcome`'s own undischarged commitment about re-judging after
  0106. It does not reopen the Decision. This is what Phase 5 promised.
- **ADR-0199 was not updated, and this plan is the session it was waiting on** — finding **M3**.
- **`docs/milkdrop-conversion.md` was not swept** — finding **M4**.
- The plan's diagram is untouched and still accurate: the `FIELD -> "NO mechanism bounds the
  EQUILIBRIUM level"` self-edge is what Phase 3 changed, and the plan moves to `done/` carrying it
  as the record of the problem it was written against. No reader diagram changed.
- No `presets/` file changed, so step 3b's curation sweep has nothing to judge.
- **Version bump owed: `patch`.** Phase 3 changed what a converted preset renders and added no
  surface; Phases 1, 2, 4, 5 and 6 are instrument, docs and ADR text.

### Lens 4 — correctness and determinism

- **The mechanism is right, and the arithmetic behind it is checkable.** `1/(1 - d)` carries
  `(1 - d)` in the denominator, so a term a fade ratio compresses is amplified at the equilibrium.
  That is what reconciles this repair with `the_decay_domain_is_not_the_wash`, which killed the
  domain as a hypothesis *about a fade* and whose reading still stands. The plan's Notes admit the
  prediction (`2.6x`-`7.0x`) overshot the measurement (`1.572x`) and give the reason — the
  `s/(1 - d)` form assumes a resample eigenvalue of 1, which *Fog Tunnel*'s `zoom = 1.042` violates.
  Admitting that in the log rather than burying it is the correct handling of a falsified
  prediction.
- **The ceiling the repair introduces is not new state.** `rlx_quantize` at positive `steps` already
  clamps to `[0,1]` before encoding, so `rlx_milk_decay`'s clamp changes nothing on that arm. On
  ADR-0118's Alternative D (`steps < 0`) it does — finding **N1**.
- **Premultiplication survives.** `rgb` and `a` take the same monotone map, so `rgb <= a` (ADR-0026)
  holds through the decay, and the comment says so.
- **No aspect is taken from a grid.** The only `aspect` in the diff is `wu.misc.x`, pre-existing and
  target-derived. `milk_wash.rs` renders 128x128, where MilkDrop's aspect pair is the identity — the
  archived Phase 2 read names that explicitly when ruling the uv chain out of its reading. The
  standing "square fixture" blind spot is already live as design-backlog 0245, and this diff does not
  widen it.
- **The instrument is deterministic.** No wall-clock read enters it: `capture_preset` resets the
  clock, `field_trace` drives `set_time(i * dt)`, and `Settled`'s band is a pure function of the
  checkpoints.
- **Numeric assertions.** `(gain - 42.5).abs() < 0.05` and `(got - expected).abs() < 1e-5` are
  properties of exact arithmetic. `SETTLED_SPREAD = 0.20` is documented as deliberately generous and
  guards the instrument's claim to report an equilibrium rather than where the equilibrium is. The
  `1.25x` / `0.5x` band in `the_field_fades_at_the_references_own_rate` is where this lens has a
  finding: see **M2**.

### Lens 5 — design integrity

The shape holds. `milk_decay` is a `pub(super)` function in the module that already owns uniform
upload; `rlx_milk_decay` is a WGSL helper beside the fragment it serves; neither adds a seam. The
`Scene` trait is untouched, no scene branches on a backend, and no shell reaches past `core`'s API.
The gate is a *bundle* property (`quantize_steps`) rather than a `WarpMeshScene` special case, which
keeps "is this an 8-bit-era field?" a question about the preset rather than about the engine — that
is the right place for it, and it is the reason the native golden did not move.

One note under OCP, in the repair's favour: gating on `quantize_steps` rather than on "does a bundle
exist" was reconsidered mid-phase after the first attempt moved **both** arms of
`the_field_equilibrates_only_when_the_quantizer_runs`, which would have left that probe asserting
that a field with an equilibrium has none. The log records the false start and the reason. That is
the kind of thing a close normally has to find.

### Findings

#### minor

**M1 — `core/src/render/milk_wash.rs:271` — the instrument's own measurement table is the
pre-repair tree's, inside the tree that repaired it.**
The `# What it measured` block reported `0.13559258 / 0.25316700 / 0.40450469` for *Fog Tunnel* and
a present-pass gain of `1.867`. `milk_wash.rs` was last touched by Phase 1 (`20ba8731`); Phase 3
(`42b4bb97`) moved every one of those numbers and did not come back. Re-run at the close, the tree
prints `0.08623820 / 0.16459735 / 0.32970616`, gain `1.909`, and the prose built on the table was
falsified with it: *"The transient from black is `0.093 -> 0.136`"* is now `0.073 -> 0.086`, and
*"`f200` is the highest of the three"* is now `f100`. The table's own last bullet warned that
*"Readings dated before 2026-09-16 are a different measurement"*, which invites a reader to trust
anything dated after it. **Repaired in `f43a2257`**, with the re-taken numbers and both dated
boundaries named.

**M2 — `core/src/render/scenes/warp_mesh/tests.rs:1619` —
`the_field_fades_at_the_references_own_rate` does not compute the reference's own rate, and the
truncation half of the repair is effectively ungated by it.**
`reference = d.powf(elapsed)` takes `d` from `trace.decay`, which is `FieldTrace`'s *"per-second
`decay` in force on the last frame"* — the value the probe set, `0.98^30 = 0.5455/s`, **before**
`milk_decay` truncates it. The reference truncates: its rate is `(249/255)^30 = 0.4895/s`, giving
`0.1506` over the probe's 2.650 s, not the `0.2007` the test calls *"the reference's arithmetic"*.
Separately, `measured` restates the linear ratio through `ENCODE_GAMMA = 2.2` while the shader
encodes with `rlx_srgb_encode`'s piecewise curve; at this field's levels that inflates the statistic
by roughly a third. The two errors run opposite ways and largely cancel, which is why the probe
reads `0.1948` and passes comfortably.

What survives: the **domain** half is gated well — restore the linear-light multiply and the same
statistic reads about `0.42`, 2.1x the `1.25x` bar. What does not: revert `encode::milk_decay`
alone, keeping the encoded domain, and the statistic lands near `0.2516` against a bar of
`0.2007 * 1.25 = 0.2509` — inside a quarter of a percent of passing. The truncation is separately and
exactly gated by `the_converted_decay_is_truncated_on_the_nominal_frame` at the function level, so
nothing ships wrong; what is not true is this test's claim to hold the field to the reference's rate.
**Left open** — the repair is the assertion and the constant, which are code, not the comment above
them. The shape it wants: build `reference` from the truncated factor (`milk_decay(d, 255.0)`), and
either restate `measured` through `rlx_srgb_encode`'s actual curve or say in the doc that
`ENCODE_GAMMA` is an approximation whose error is comparable to the band.

**M3 — ADR-0199 still asked this plan's rig session for a capture the rig session took and that
refutes its inference.**
That ADR's Negative reads *"Plan 0142's Phase 4 rig session is asked to capture mode 0 at unit scale
as a confirmation. Until one lands, this ADR says so"*, and its own `Outcome` repeats it under
**Still open, and owed elsewhere**. The capture landed — `k ~ 0.068` against the `0.158` fitted on
mode 6, a ratio of `0.43` — and says the inference is **not** supported. `dev` flagged this under
Followups rather than acting, correctly: no phase lists ADR-0199 under `Files touched`. It is close
bookkeeping — an accepted ADR whose recorded claim the plan falsified takes a dated `Outcome`, the
ADR-0054 / ADR-0074 precedent. **Repaired in `f43a2257`.**

**M4 — `docs/milkdrop-conversion.md` — the operator doc's rate section did not carry Phase 3's
truncation.**
*"Rates are converted"* said *"a factor becomes `v^30`, a rate `v * 30`"* and listed `decay` among
the factors. That is now incomplete for `decay` alone: it is taken back to the nominal frame,
truncated where `D3DCOLOR_RGBA_01` truncates it, and returned per second, so a `.milk` at
`fDecay = 0.98` runs at `249/255` per frame. A reader working a converted bundle backwards got the
wrong number. **Repaired in `f43a2257`.**

**M5 — this plan's `## Implementation log` outweighs the contract it reports against.**
218 lines against `## Implementation phases`' 154. Nothing gates this property, which is why the
close checks it. **Left open**: the log's content is genuinely load-bearing here — the false start on
the gate, the falsified `2.6x`-`7.0x` prediction, the *Blur Mix 3* reconciliation that is declined
rather than faked — and trimming a record to satisfy a ratio would cost more than the ratio is
worth. What it says about the next plan of this kind is that some of this belongs in the phases as
done-whens rather than in the report.

#### nit

**N1 — `core/src/render/scenes/warp_mesh/shaders.rs:217` — the comment justified the gate on the
quantizer's floor, and the clamp also reaches the arm that has no floor.**
The block reads *"The ceiling `rlx_milk_decay` reproduces and the floor below are two halves of one
thing"*, which is right for `steps > 0`, where `rlx_quantize` already clamps to `[0,1]` before
encoding. Under ADR-0118's Alternative D (`steps < 0`) `rlx_quantize` returns its argument
**unclamped**, so on that arm the new clamp is a ceiling the field did not previously have. The
behaviour is defensible and nothing in the repository sets a negative `quantize_steps`, but the
comment did not cover it. **Repaired in `f43a2257`.**

### Close bookkeeping notes

- **Backlog probes** re-run at the close: exit 0, 89 reductions across 37 live entries, 3
  unprobeable. Two advisory "path moved" rows are this plan's own — 0245 (`warp_mesh/tests.rs`) and
  0249 (`warp_mesh/shaders.rs`), both stamped just before this lane touched those files. Neither
  claim is falsified by the diff: 0245 is about square golden fixtures and 0249 about `zoom`'s doc,
  and this plan touched neither.
- **Translations:** `check-translations.mjs` exits 0 on five stamped translations. Its advisory was
  empty on the lane and carries **one row** after `git merge main`:
  `packaging/foobar/READ-ME-FIRST.ru.md`, stamped `f2b0048b`, against an English source now at
  `d6e275e6` — which is [Plan 0103](0103-the-project-gets-an-audience.md)'s close, not this plan's
  work. It is named here because reading the advisory is a close's duty whoever moved the source;
  correcting the Russian is content work and is routed, not done here.
- **Preset curation (step 3b):** no file under `presets/` changed. The standing workaround grep over
  `presets/*.toml` turns up nothing this plan's engine fix makes stale — the repair is confined to
  the converted path, and no shipped preset carries a `[milk]` table.

## Followups (after this lands)

- **The converted-warp-shader path applies `decay` in linear light too.** Phase 3
  repaired only the built-in fragment, gated on `quantize_steps`. A preset whose
  own HLSL says `ret *= decay` runs that multiply on this engine's linear field
  and on the reference's 8-bit one, which is the same domain divergence at the
  same `1/(1 - d)` amplification. 1 253 of the corpus's 8 162 warp-shader files
  name `decay`; none of the seven look-gate pairs is one, which is why it is here
  rather than in the phase. It is a `milk/shader.rs` epilogue question and it
  wants the reference on screen before it is answered.
- **ADR-0199 owes an `Outcome` for the mode-0 capture.** That ADR's Negative asks this plan's rig
  session for it and says *"Until one lands, this ADR says so"*. The capture landed (Phase 4's table:
  `k ~ 0.068` against `0.158`, ratio `0.43`, which does not support the inference) and is quoted in
  ADR-0113's third `Outcome`, but ADR-0199 itself was not edited: no phase of this plan lists it
  under `Files touched`. A one-section append is all it wants.
- **`docs/milkdrop-conversion.md`'s rate section does not carry Phase 3's truncation.** *"Rates are
  converted, and that is why a preset moves at the right speed"* says a factor becomes `v^30`, which
  is now incomplete for `decay` alone: `milk_decay` takes it back to the nominal frame, truncates it
  where `D3DCOLOR_RGBA_01` does and returns it per second, so a `.milk` at `fDecay = 0.98` runs at
  `249/255` per frame. Two sentences in the operator doc; no phase of this plan lists that file.
