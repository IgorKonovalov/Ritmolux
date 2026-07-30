# ADR-0052 — The analysis diagnostics surface is native-only and does not cross the C ABI

> **Status:** proposed
> **Date:** 2026-07-30
> **Related plan(s):** 0049-analysis-diagnostics-surface
> **Supplements:** [ADR-0008](0008-c-abi-v3-diagnostics.md) (the diagnostics/metrics surface),
> [ADR-0003](0003-c-abi-v1-surface.md) (what moves `LMV_ABI_VERSION`)
> **Prompted by:** Plan 0048 Phase 6, which could not be performed because no instrument existed.

## Context

Plan 0048 Phase 6 asks a human to play real music through the live app and judge two
things: whether the new normalized levels ride the music without pumping or going numb,
and whether the downbeat estimator locks — recording the lock rate. Neither is readable.
`diag::Metrics` carries frame-time statistics and nothing else, and the 1 Hz structured
log emits exactly that plus RSS.

The gap is worse than an inconvenience. ADR-0050 accepts real research risk on the
downbeat estimator on the strength of one stopping condition: *stop if it locks wrong on
ordinary 4/4*. From outside the app a wrong lock and the counter fallback are
indistinguishable, so that condition is currently unfalsifiable — the gate ADR-0050 calls
"the design" cannot be observed at all.

Fixing it forces a decision, because `diag::Metrics` is documented as mirroring the C ABI's
`LmvMetrics` "so both frontends surface identical numbers from one computation"
(`core/src/diag/mod.rs:40`). Growing it therefore either widens the C ABI or breaks that
mirror. ADR-0008 anticipated the question and left it open deliberately: `LmvMetrics` leads
with `struct_size` + `abi_version` precisely so diagnostics fields can append, but ADR-0008
also records that such an extension "is another ABI decision, not a silent reshape of
`LmvMetrics`". This is that decision.

## Decision

We will expose the analysis diagnostics as a **separate, native-only** `AnalysisMetrics`
struct alongside `Metrics`, reached through its own accessor, and we will **not** grow
`LmvMetrics`. `LMV_ABI_VERSION` stays at 4.

It carries exactly the six values Plan 0048 Phase 6 needs to make its two judgements: the
four normalized levels `bass`/`mid`/`treb`/`onset`, and the downbeat estimator's
`confidence` and `locked`. Nothing speculative — the `*_raw` twins, `bpm`, `bar` and
`novelty` stay off it until something asks.

Two structs rather than one is the point. It makes native-only a **property of the type**
rather than a comment on a field that a later plan can quietly ignore, and it leaves
ADR-0008's "`Metrics` mirrors `LmvMetrics`" invariant literally true instead of true
with an asterisk.

## Consequences

### Positive
- Plan 0048 Phase 6 becomes a measurement instead of an impression, and ADR-0050's
  stopping condition becomes checkable — which is what made the estimator's risk
  acceptable in the first place.
- No C ABI change, so no `LMV_ABI_VERSION` bump, no separately-compiled C++ side to keep
  in step, and no obligation on the foobar shim for a debugging surface.
- Follows the standing precedent rather than inventing one: `novelty` is already
  documented "Native-API only — not exposed across the C ABI" (`core/src/dsp/mod.rs`).
  Analysis values have never crossed the boundary; this keeps that line where it is.

### Negative
- **The foobar plugin gets no analysis diagnostics.** If the estimator later needs
  validating on that path — and the plugin is the one frontend that never touches loopback
  capture, so its input differs — this decision has to be revisited by a superseding ADR.
  That is a real gap, named rather than hidden.
- Two metrics structs is one more thing to know about, and a future reader may reasonably
  ask why the split exists. That question is answered here and should be answered again in
  a doc comment on the type.
- The overlay grows rows, so the debug panel occupies more screen. It is opt-in (`F3`,
  off by default), which is why that cost is acceptable.

## Alternatives considered

### Alternative A — append the six fields to `LmvMetrics`, ABI v5
Uses the extension path ADR-0008 designed, and `struct_size` means old hosts keep working
without a recompile, so it is genuinely cheap in mechanism. Rejected because the cost is
not mechanical: per CLAUDE.md an ABI shape change "is an ADR-worthy event, not a casual
edit", it obliges the separately-compiled C++ side, and spending an ABI version on a
debugging readout is disproportionate to a surface that exists to run one listening test.
The door stays open — `struct_size` still absorbs the growth whenever a real need appears.

### Alternative B — grow `diag::Metrics` and mark the new fields native-only in a comment
One struct, least code. Rejected because it makes the mirror property false while leaving
the doc claiming it, and a comment is exactly the kind of guard that erodes: the next plan
adding a field has no reason to read it and no test to stop it.

### Alternative C — log-only, no overlay
Smallest change, and the lock rate is what Phase 6 records. Rejected because the other half
of Phase 6 — pumping versus numbness in the AGC release — is a judgement about how levels
*move* against music you are hearing, which a once-per-second line cannot show. The two
questions want two instruments.

## Notes

The six values are a Phase 6 instrument first and a general debugging surface second. If a
later plan wants the full frame on screen, that is a widening worth its own thought, not a
default — the overlay is read at a glance while music plays, and every field added costs
some of the glance.
