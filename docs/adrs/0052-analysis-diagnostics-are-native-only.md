# ADR-0052 — The analysis diagnostics surface is native-only and does not cross the C ABI

> **Status:** accepted 2026-07-30 (implemented by
> [Plan 0049](../plans/done/0049-analysis-diagnostics-surface.md), which passed Mode 4 review —
> see the Outcome note below, which corrects one Consequences bullet)
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
- **The foobar plugin gets no analysis diagnostics.** *(Corrected at Plan 0049's close — see the
  Outcome section: the core-drawn overlay does reach the plugin under `LMV_DEBUG_OVERLAY`; it is
  the programmatic half that is absent.)* If the estimator later needs
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

## Outcome (added 2026-07-30 at Plan 0049's close)

The decision holds and shipped as written: `diag::AnalysisMetrics` sits beside `Metrics` with
exactly the six values, `core/src/ffi.rs` is untouched, `LMV_ABI_VERSION` is still 4 and
`LmvMetrics` still 56 bytes. The "two structs make it a property of the type" argument earned
more than it claimed — the enforcement mechanism `dev` chose is an **exhaustive destructure** of
`Metrics` in a unit test, so a later plan adding a field to the mirror does not merely trip a
reviewer, it stops compiling.

**One Consequences bullet is wrong and is corrected here.** The first Negative reads "The foobar
plugin gets no analysis diagnostics." It gets half of them. The overlay is **core-drawn** —
`render/mod.rs` paints it whenever `diag.overlay_enabled()`, and `ffi.rs` sets that flag from
`LMV_DEBUG_OVERLAY` — so a foobar host that turns the debug overlay on sees the same four level
meters and the same `LOCK`/`FREE` row the standalone's `F3` shows, over its own PCM. What the
plugin genuinely lacks is the **programmatic** half: no `lmv_get_metrics` counterpart and no log,
so nothing on that path can compute a **lock rate**. That is the gap that would justify revisiting
this ADR, and it is narrower than the bullet suggests — which matters, because a future reader
weighing Alternative A's ABI v5 against "the plugin is blind" would be weighing against something
that was never true. The type's doc comment in `core/src/diag/mod.rs` states the corrected version.

The overlay-only rejection (Alternative C) is vindicated in the implementation rather than merely
asserted: the four levels shipped as **meters** with the numbers beside them, precisely because
"pumping versus numbness" is a judgement about how a value *moves*. And the log half went further
than the ADR asked — `downbeat_locked` is written as `0`/`1`, so the lock rate Phase 6 records is
the arithmetic mean of a column rather than a string match, and a log left by an older build is
**rotated** rather than appended to, so the by-index parse can never meet two row widths in one
file.
