# ADR-0064 — A capture may pin the `Rich` tier: `shot --tier`

> **Status:** accepted (2026-08-03) — see Outcome below: `shot --tier` already existed; what Plan 0057 Phase 1 built is `--at` and the `onset` row
> **Date:** 2026-08-03
> **Related plan(s):** [0057-the-attractors-compute-path](../plans/done/0057-the-attractors-compute-path.md) (Phase 1)
> **Supplements:** [ADR-0045](0045-quality-tiers-floor-and-rich.md) — quality tiers

## Context

ADR-0045 states that "captures and goldens pin `Floor`" and, two lines later, that "`Rich` is
verified by **capture-level spot checks** plus the on-device checklist". The first half was built:
`Renderer::new_headless` takes no tier argument, and `shot` deliberately does not read `LMV_TIER`,
so a capture cannot be blessed at another tier by forgetting a field (Plan 0044's design property,
the same shape as Plan 0047's `SaltMode`). The second half was never built. There is no `--tier`
flag and no other way to render `Rich` headlessly, so the capture-level spot checks the ADR names
as `Rich`'s verification path **have never been possible**.

That gap has now cost something specific. Four shipped attractor presets rendered as flat
single-tone masses at `Rich` — the tier the app *starts on* — for as long as they have shipped,
behind a 388-test green suite, until a user said so ([backlog 0047](../design-backlog.md)).
`attractor_particles` is 50 000 at `Floor` and 150 000 at `Rich` against an unnormalized additive
deposit, so `Rich` puts three times the light into the same texels; every golden baseline, every
behavioral gate and every `--report` column describes a configuration the application does not
start in. The content lane's workaround was to multiply `exposure` by 3 — exact, because
accumulation is linear and the tonemap terminal, but exact *only* for this scene family, and
reachable by no gate.

The property ADR-0045 is actually protecting is that a capture is a **pure function of its
inputs** — that no ambient environment can silently move a baseline. A tier read from
`LMV_TIER` would break that. A tier named explicitly on the command line does not: it *is* an
input, in the same sense `--preset-file` and `--size` are.

## Decision

`shot` accepts `--tier floor|rich`, defaulting to `floor`, and the core exposes a second,
explicitly-named headless construction path that takes a tier. `Renderer::new_headless` keeps its
signature and keeps pinning `Floor`, so a capture path added later that does not think about tiers
still gets `Floor` — the compile-time property Plan 0044 bought stays bought, and only a caller
that names the tier in its own source can leave it. `shot` continues to ignore `LMV_TIER`.

A `Rich` capture is an **instrument, not a baseline**. No golden is blessed at `Rich`, because the
`Rich` multipliers are still the provisional values Plan 0044 shipped (its Phase 4 calibration has
never run), and a baseline pinned to provisional constants would have to be re-blessed the moment
they are calibrated.

## Consequences

### Positive
- The verification path ADR-0045 already promised becomes available, four plans after it was
  promised. The class of defect that produced backlog 0047 becomes capturable, which is the
  precondition for any gate that could catch the next one.
- Every "how does this look at `Rich`" question stops requiring the running app. `[`/`]` (Plan
  0050) remains the right instrument for a *judgement in motion*; this is the one for a measurement.
- The exposure-times-three proxy retires. It was correct arithmetic about one scene family, and
  nothing about it announced that scope.

### Negative
- **The QA surface gains a configuration it does not cover.** ADR-0045 predicted the surface would
  double; this ADR makes that possible while explicitly declining it, which is a discipline
  question rather than a mechanism. The mitigation is naming: a `Rich` capture has no bless path,
  and the plan that adds the flag says so in `docs/capturing.md`.
- **One more thing a capture command can differ by.** A measurement recorded without its tier is
  now ambiguous where before it could not be. Any recorded number owes its tier.
- The second constructor is a second way to build a headless renderer, which is a small widening of
  a surface Plan 0031 deliberately narrowed to one construction path.

### Neutral
- No C ABI change. The plugin's tier behaviour is untouched (ADR-0045 keeps it automatic).

## Alternatives considered

### Alternative A — keep `Floor`-only captures and document the 3x
Backlog 0047's own third option, and what the four repaired presets already do: leave authors to
hold headroom for a tier they cannot render. Rejected because it is what just failed. A rule that
depends on every author remembering an unmeasurable coupling is the rule that produced the flat
discs, and it leaves the next family to pay the same price.

### Alternative B — have `shot` read `LMV_TIER`
Free, and consistent with how `LMV_PRESET_DIR` works. Rejected because it re-arms exactly the
hazard ADR-0045's purity clause exists for: an ambient environment variable that moves a baseline
without appearing in the command that produced it. The explicit flag buys the capability without
the hazard.

### Alternative C — bless `Rich` golden baselines alongside `Floor`
The thorough answer, and the one ADR-0045 predicted would double the suite. Rejected on timing:
`TierConfig::RICH`'s own doc comment says its values are provisional pending Plan 0044 Phase 4, so
every `Rich` baseline would be a re-bless waiting on a calibration that has not run. Revisit after
that calibration.

### Alternative D — promote the content lane's `exposure * 3` proxy to a documented technique
Cheapest of all, and it reproduced the user's frame exactly. Rejected because its correctness is a
property of *linear accumulation into a terminal tonemap* — true for the attractor, silently false
for any scene that clamps, thresholds or feeds back nonlinearly — and because no automated check
can use it.

## Notes

The defect that motivated this is fixed under [ADR-0065](0065-the-attractor-deposit-is-normalized-by-particle-count.md);
this ADR is about being able to see the next one. The two land in the same plan on purpose: the
instrument goes in before the fix, so the fix's done-when is measurable rather than eyeballed —
the sequencing [ADR-0049](0049-analysis-v2-dual-resolution-axis-normalized-bands.md) paid for.

## Outcome (Plan 0057 Phase 1, 2026-08-03)

**Accepted, and half of it was already built — this ADR's Context is wrong and the correction is
the finding.** "There is no `--tier` flag and no other way to render `Rich` headlessly" was false
when written. [Plan 0044](../plans/done/0044-quality-tiers.md) Phase 3 shipped
`Renderer::new_headless_tiered` and `shot --tier floor|rich`, and `docs/capturing.md` documented
both. The flag was absent only from `shot --help`, which is how four documents — this ADR, the
plans index, `design-backlog` 0031 and 0047, and the `preset-author` skill's footgun list — came to
assert a capability the repo had for four plans. Verified rather than rebuilt at Phase 1:
`--tier rich` and `--tier floor` on `attractor_clifford` produce different frames, and omitting the
flag is byte-identical to `--tier floor` (md5 `4278677a…` both ways, `871fbc38…` for rich).

Everything the Decision asks for therefore holds as written, unchanged: `Renderer::new_headless`
still takes no tier and still pins `Floor`, `shot` still ignores `LMV_TIER`, and no `Rich` golden
exists. `docs/capturing.md` now carries the "instrument, never a baseline" sentence explicitly.

**What Phase 1 actually built is the other instrument, and the premise there was also false.** The
plan asked for a `--signal` kind whose onset crosses the shipped reseed gates, on
[ADR-0066](0066-a-reseed-disturbs-the-cloud-rather-than-replacing-it.md)'s claim that `click:120`
never does. Measured: it crosses `0.75` on **7 hops out of 375**, one per beat — see that ADR's own
Outcome. No generator was added. The real gap was **aiming**: `--strip N` samples evenly, so a strip
of 8 lands on a reseed by luck, and a working reseed and a broken one render identically when it
misses. So the filmstrip's level table gained an `onset` row naming the hop it peaked on, and `shot`
gained `--at <hop>,...` to capture that hop. With both, `attractor_ink --tier rich --at 44,46,48,54`
renders the speckled rectangle ADR-0066 describes, which no capture in this project could previously
produce.

**The lesson generalizes past this ADR.** A capability that exists but is missing from `--help`
is, for practical purposes, a capability that does not exist: four documents reasoned from its
absence and one of them (this ADR) proposed building it. The cheap guard is that a flag's help text
is part of the flag.
