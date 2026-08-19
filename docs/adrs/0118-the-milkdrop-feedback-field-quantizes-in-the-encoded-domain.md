# ADR-0118 — The MilkDrop feedback field quantizes in the encoded domain, per bundle

> **Status:** accepted 2026-08-17 (Plan 0108) — carries three `Outcome` entries
> **Date:** 2026-08-17
> **Related plan(s):** [0108](../plans/done/0108-the-milkdrop-import-gets-its-tone-back.md),
> [0111](../plans/0111-the-milkdrop-import-stops-washing-out.md) (third `Outcome`)

## Context

[Plan 0100](../plans/done/0100-the-engine-speaks-milkdrop.md) landed the MilkDrop import and its
Phase 7 judged seven presets side by side against `foo_vis_milk2` 0.2.0.0 in foobar2000 v2. The
structure, motion and audio reactivity survived conversion in every pair. The **tone** did not, and
the plan's own motivating claim — that this engine's HDR pipeline would make classic presets look
*better* — came back **merely different**, dominated by one defect
([design-backlog 0106](../design-backlog.md)).

The mechanism is a format difference, not a translation bug. MilkDrop's feedback target is 8-bit:
`decay` times a dim pixel **truncates to zero**, and that quantization is what keeps a classic
preset's background black and its trails finite. This engine's field is `Rgba16Float` — nothing
truncates, every dim residual survives and accumulates. One mechanism produced four presentations
across the seven pairs: pastel wash (*Songflower*, *Cosmic Dust 2*), white-hot glow (*Contortion*),
runaway to the clamp with per-channel fringing (*chasers 19 Portal*), and full tonal **inversion**
(*Fog Tunnel*, a dark preset rendered on a white plateau). The control ran the other way: *Blur Mix
3*, whose blur chain actively darkens, kept its blacks and was the one pair that looked genuinely
good — which scopes the defect to the feedback path rather than to the shader translation.

Half of the reference's bound is already reproduced and the code says so. The converted warp
epilogue (`milkconv/src/shader/emit.rs:1440`) emits
`clamp(_lmv_ret, vec3<f32>(0.0), vec3<f32>(1.0))`, and `warp_mesh/mod.rs:1824` describes it as
*"the reference's bound is its 8-bit target — which the shader epilogue's clamp reproduces."* That
is the **ceiling** half. The floor half — truncation to zero — has never existed, and the comment
reads as though the whole bound were covered.

Two forces shape the answer. First, the field is **linear light** (ADR-0046) while the reference
quantizes in its **gamma-encoded** target, so "8-bit truncation" does not name a number here until
a domain is chosen. Second, `warp_mesh` is a **native scene** as well as the import's landing
surface — [Plan 0104](../plans/0104-the-library-stops-being-lopsided.md) will author a cohort onto
it — and native authoring has no reason to want 8-bit-era quantization.

## Decision

We will emulate the reference's feedback quantization by **round-tripping through the sRGB transfer
function** in the warp epilogue — encode, quantize to 1/255 steps, decode — driven by a **runtime
uniform**, and switched on **per bundle**: on by default for a `[milk]` bundle, off for a native
`warp_mesh` preset, with a bundle key to override in either direction. Both epilogues carry it: the
converted-shader path (`emit.rs`'s emitted `fs_main`) and the built-in decay fragment
(`WARP_SHADER`'s `fs_main`), because an MD1-era preset with no custom warp shader has a feedback
field and washes out the same way.

The domain is the whole decision, and one number carries it. One 8-bit sRGB step is `1/255 =
0.00392` **encoded**, which is `0.00392 / 12.92 = 3.03e-4` in linear light. So a literal `1/255`
floor applied in linear truncates everything below encoded `0.0498` — **sRGB level ~13, thirteen
times too aggressive** — and would crush the dim trails the reference keeps rather than the dimmer
ones it discards. Encoding first is not a refinement of the linear floor; it is the difference
between emulating the reference and destroying the picture.

We rejected **a literal linear floor** because of exactly that arithmetic; **a converter-baked
constant** because it gives no runtime toggle and cannot reach the built-in fragment at all;
**engine-wide quantization with no key** because it would impose the 8-bit era on native authoring
and forfeit the HDR look the engine was built for; and **a floor without full quantization** on the
narrower ground that it is the cheaper half of the same idea rather than a different one — it is
retained as the named fallback below.

## Consequences

**Positive.**

- The dominant fidelity defect of the import gets a mechanism-level fix rather than a per-preset
  tuning, and the four presentations collapse to one cause.
- Plan 0100's central claim becomes **re-judgeable**. It is provisionally negative today and cannot
  be fairly re-read until this lands; the same seven pairs on the same rig are the instrument.
- Native `warp_mesh` authoring is untouched by construction. With the switch off the epilogue is an
  exact identity, so the assertion available is **byte-identity** against today's output rather than
  a tolerance — the [Plan 0075](../plans/done/0075-the-content-renaissance.md) Phase 2 precedent.
- The uniform-driven shape means an A/B is a parameter change, not a re-convert, which is what makes
  the tuning gradeable at the look gate at all.

**Negative, and these are the price.**

- **It reproduces banding this engine deliberately dithers away.** [ADR-0096](0096-the-display-write-dithers.md)
  dithers the display write precisely because 8-bit quantization of a wide smooth gradient reads as
  Mach bands. Quantizing the *feedback field* re-introduces that inside the loop, where the display
  dither cannot reach it — it is upstream of the composite. This is a faithful reproduction of a
  reference artifact, which is the point, and it is also the most likely reason the look gate says
  the tuning is wrong.
- **A bundle converted before this lands emits a bare clamp and silently will not quantize.** It
  degrades gracefully — nothing breaks, the preset renders as it does today — but the fix reaches it
  only on a re-convert. Nothing in the repository is affected (Plan 0100 Phase 8 decided against
  distribution, so no converted preset ships), and the cost is one `milkconv` run on a user's own
  directory.
- **It moves a golden baseline.** `core/tests/golden/warp_mesh_milk.png` binds the milk path and
  will change; `warp_mesh.png` must not. That split is the identity claim above, stated as a test.
- One encode/decode pair per fragment in the warp epilogue, on a fullscreen pass. Small against what
  a converted warp shader already costs, and unmeasured until the plan measures it.

## Alternatives considered

**A — a literal `1/255` floor in linear light.** The obvious reading of "emulate an 8-bit target",
and the one the backlog entry warns about in the same breath. It lost on arithmetic: linear
`0.00392` is encoded `0.0498`, so it truncates everything below sRGB level ~13 and removes dim
detail the reference keeps. Wrong at the low end by 13x, and wrong at the high end too, since it
imposes no step structure where the reference has one.

**B — bake the floor into the emitted WGSL as a constant.** Cheapest change: one edit to
`emit.rs`'s epilogue and nothing else moves. It lost on two counts, either of which is fatal. There
is no runtime toggle, so the per-bundle decision above becomes a re-convert and the look gate cannot
A/B a tuning it is being asked to grade. And it cannot reach the **built-in** decay fragment, which
is the path an MD1-era preset with no custom shader takes — leaving a whole era of the corpus
unfixed by a fix that claims to be the mechanism.

**C — quantize every `warp_mesh` scene, no key.** Simplest surface, no new bundle key, one code
path. Rejected because `warp_mesh` is a native scene with a content cohort coming
([Plan 0104](../plans/0104-the-library-stops-being-lopsided.md)), and this would hand that cohort an
8-bit feedback field it never asked for — forfeiting the HDR range the engine exists to have, in
order to be faithful to a reference the native presets are not imitating.

**D — a truncate-to-zero floor at one encoded step, without quantizing the levels between.** Not
rejected on its merits; it is the **named fallback**. It reproduces the half of the mechanism that
does the visible damage (dim residuals accumulating instead of dying) and costs one comparison, and
it does **not** re-introduce the banding in Consequences above. It is not the decision because it is
a partial emulation chosen before anyone has seen the full one — if the look gate says the banding
is worse than the wash, this is what the plan falls back to, and that ordering is deliberate.

**E — tone-map or grade the field toward the reference's look.** Rejected without much weighing:
it treats a format difference as a colour-correction problem, needs a per-preset curve nobody can
derive, and would have to be re-tuned against every pair rather than fixing the one mechanism the
seven pairs share.

## Outcome

**2026-08-17, Plan 0108 Phase 1 — the domain was measured and sRGB stands.** The Notes below flag
that the reference's own transfer function is a plain ~2.2 gamma rather than sRGB's piecewise curve,
and that the two differ exactly in the near-black region this decision is about. Both were built and
the same probe rendered through each
(`core/tests/fixtures/warp_mesh_quantize.toml`, 96x96, WARP, deposit gated off after 0.25 s, unit
brightness):

| frame | sRGB: byte-sum / lit px / peak | 2.2 gamma: byte-sum / lit px / peak |
|-------|-------------------------------|-------------------------------------|
| 20    | 2 250 777 / 8 902 / 191       | 2 246 072 / 9 031 / 191             |
| 40    | 496 718 / 7 705 / 58          | 490 471 / 8 179 / 58                |
| 60    | 12 042 / 2 059 / 8            | 22 788 / 4 032 / 7                  |
| field reaches exact zero | frame **68**| frame **76**             |

**The two are indistinguishable wherever the picture reads** — the peak channel agrees to within one
8-bit level at every frame and the byte-sums to 1.3 % — and they diverge only in the tail nobody can
see. There sRGB is the **stricter** floor, and by a number that is not a coincidence: it kills a
residual at linear `3.03e-4`, which is exactly 8-bit display level 1, so what dies is precisely what a
viewer could not have seen. A plain 2.2 gamma floors at `(1/255)^2.2 = 5.1e-6` — **59x lower** — and
keeps roughly six more e-foldings of invisible-but-nonzero light alive, which is the accumulation this
ADR exists to stop. So the second transfer function would buy an eight-frame-longer invisible tail in
exchange for a constant to justify, and it was not taken.

## Outcome — the look gate, and what this decision did not buy

**2026-08-17, Plan 0108 Phase 2/6 — the decision stands unchanged, and its premise was too broad.**
Seven pairs judged live against `foo_vis_milk2`, each at three settings of this ADR's own lane
(255 / off / Alternative D).

**The two questions this ADR put to the look gate both answer in its favour:**

- **The banding does not read.** Consequences names it as the designed price — quantizing inside the
  feedback loop re-introduces upstream of where [ADR-0096](0096-the-display-write-dithers.md)'s
  dither can reach. No pair showed it. Where Alternative D was picked as the closest of the three it
  was by a hair and never *because* of banding, so **Alternative D is not taken** and stays what it
  was: a recorded fallback nobody needed.
- **The control survived.** *Blur Mix 3*, the one preset that already looked right, is unharmed.

**What is falsified is not this decision but the size of the problem it was sold against.**
[Backlog 0106](../design-backlog.md) claimed one mechanism with four presentations — pastel wash,
white-hot glow, runaway with fringing, tonal inversion — and this ADR's Context repeats that claim.
The gate found otherwise. On the five pairs with no video echo the background sits **three orders of
magnitude above this quantizer's floor** (linear `3.03e-4`), so no setting of `quantize_steps` can
reach it, and A/B/C are nearly indistinguishable there. *Fog Tunnel*'s "inversion" turned out to be a
mid-grey non-additive waveform drawn over an already-washed ground, not an inversion at all.

So the mechanism recorded here is **real, measurable and worth keeping** — the field reaches exact
zero, and under a dynamic signal a preset that dissolved to flat white now holds its shading — but it
is **not** the dominant fidelity defect of the MilkDrop import. Plan 0100's HDR claim closes
**negatively**: still merely different. The dominant defect is the wash itself, cause unknown, and it
carries to [Plan 0109](../plans/done/0109-the-milkdrop-import-gets-its-geometry-back.md) with
[backlog 0113](../design-backlog.md).

## Outcome — the same field under a realistic `decay`

**2026-08-19, Plan 0111 Phase 1 — nothing here is falsified; a second configuration is added, and
the probes now state the one they run in.**

This entry replaces a first draft written the same day that claimed the Context sentence was
overstated and that Plan 0109 Phase 4's probes had been *miscalibrated*. **Both claims were wrong**,
and the correction is worth recording because the mistake is an easy one to repeat.

Plan 0111 Phase 1 fixed design-backlog 0121: `FrameSlots::read` returned the `decay` **default**
unconverted, so a bundle naming no `decay` ran at MilkDrop's per-frame `0.98` in a field that means
per-second. The two probes below passed `None` and therefore ran at that near-unity factor — and
that was **deliberate**, through an escape hatch `field_trace` documents in the comment right above
the call. Neutralizing `decay` is what makes these probes isolate the quantizer. What the fix changed
was not their calibration but the meaning of the value they got for free. They now pass an explicit
`NEUTRALIZED_DECAY = Some(0.98)`, which reproduces every recorded digit of the tables in their doc
comments — verified against the committed numbers, on hardware, where they had been recorded on WARP.

**So the Context stands as written.** With `decay` neutralized the unquantized field climbs to a mean
of `0.8331` with a peak of `6.68` at frame 300 and shows no equilibrium, which is exactly *"nothing
truncates, every dim residual survives and accumulates"*.

**What is genuinely new is the second configuration** — the converted `decay` a real bundle actually
runs at, `per_second_factor(0.98) = 0.5455`/s. There the field **does** settle, on both arms, and the
quantizer's contribution is a lower settling point and an exactly-black background rather than the
presence or absence of a bound:

| statistic (still params, converged) | quantizer off | quantizer on | |
|---|---|---|---|
| field mean | 0.2963 by f450 | 0.1298 by f120 | **2.28x lower** |
| background `edge` (zoom params) | 1.115e-4 by f450 | 1.237e-6 from f30 | **90x lower, at the floor** |

Both are dev-box hardware readings, deterministic to nine decimals across three repeats — so there is
no run-to-run spread here to derive a tolerance from, and any assertion on these must take its
tolerance from the mechanism rather than from noise (ADR-0071).

The useful way to say what this ADR buys, once `decay` is real: **this engine's field decays toward a
nonzero equilibrium where the reference's decays to black, and the floor is what closes that gap.**
That is a sharper statement than "the field is an unbounded integrator", which is true only with
`decay` neutralized — the configuration the probes choose in order to see the mechanism at all, and
not the one a converted preset runs in.

## Notes

The reference's own transfer function is not sRGB's piecewise curve — DX9-era MilkDrop wrote to an
8-bit render target with no explicit encoding, which in practice is a plain ~2.2 gamma. The
difference from sRGB is confined to the near-black region, which is exactly the region this decision
is about, so the choice between the two is a real question and the plan measures it rather than
assuming. sRGB is the starting point because the engine already has the transfer function
(ADR-0046's linear-light chain) and a second one would be a new constant to justify.
