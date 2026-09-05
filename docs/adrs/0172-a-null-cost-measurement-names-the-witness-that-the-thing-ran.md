# ADR-0172 — A null-cost measurement names the witness that the thing ran

> **Status:** proposed 2026-09-06
> **Related plan(s):** [0147 — What the show costs, and what its numbers mean](../plans/0147-what-the-show-costs-and-what-its-numbers-mean.md) Phase 3b
> **Related:** [ADR-0071](0071-a-numeric-test-contract-states-a-property-or-names-its-machine.md) (a
> numeric assertion states a property or names its machine),
> [ADR-0143](0143-the-operator-console-is-a-second-surface-and-the-shell-owns-its-meaning.md)
> (whose cadence claim is what this rule was needed for)

## Context

Plan 0147 Phase 4 set out to price the operator console: five arms, one hands-off window, mean fps
with the console closed against the console open. The first window ran on 2026-09-06 and reported
165.0 -> 165.0 fps, 40.0 -> 40.4, and 29.4 -> 28.7. Read as a measurement, that is *"the console is
free"* — the opposite of the 61.7 -> 33.1 halving that filed backlog 0164.

It is not readable as a measurement, and the reason is structural rather than a mistake anyone made
in that window. `AuxTarget::present` returns `Ok(())` on `Timeout | Occluded` and again on
`Outdated | Lost`, both **before any encoder work**; `present_aux` propagates that `Ok`; nothing
counts. A console that presented every frame and a console whose surface was occluded for the whole
run produce **the same `Ok`, the same log, and the same frame rate**. The instrument reports zero
cost in both cases, and no surface in this project distinguishes them.

The show's own path is not built this way, and the contrast is what makes the rule obvious rather
than clever. `Renderer::render` calls `record_dropped()` on the identical skip and reaches
`record_frame()` only after `queue.present`, so `fps` counts presents rather than loop iterations
and `frames_dropped` is a column in `diagnostics.log`. The show side of that same window is
therefore fully witnessed — `frames_dropped` is 0 across all 429 sampled rows — and its numbers
stand. One surface was instrumented and one was not, and only the uninstrumented one produced a
result that could not be believed.

The general shape: **a measurement that a thing costs nothing is the one measurement its own
absence also produces.** Every other reading falsifies the "it did not run" hypothesis by existing
— a cost of 6 ms is evidence something happened. A cost of zero is exactly what a no-op looks like,
so it carries no evidence of its own subject, and the burden it cannot discharge on its own has to
be discharged beside it.

## Decision

**A measurement whose reportable outcome includes "it cost nothing" must publish, alongside the
cost, a count that the thing under measurement actually ran.** The count is part of the
measurement, not a debugging aid: an arm without it is void rather than negative, and a `done when`
that admits a null outcome states the counter it will be read against.

Concretely, for anything this project prices:

- **The skip path is counted, not swallowed.** A code path that returns success without doing the
  work increments something. `record_dropped()` is the pattern; a bare `return Ok(())` on a
  transient is the anti-pattern.
- **The counts reconcile against a total that is independently known.** Presents plus skips plus
  whatever the caller decimated equals the frames the caller ran. Two counters that do not add up
  to a known total move the ambiguity rather than removing it.
- **A null result names its counter and its regime.** "Neither lever moved it" is a legitimate
  finding when the arm reports a non-zero run count *and* ran where the effect could have been
  seen. Outside either condition it is not a finding at all.

The rule binds the **measurement**, not every code path. A transient skip that returns `Ok` is fine
until someone prices the thing it skips; the counter is owed when the price is asked for.

## Consequences

### Positive

- A null becomes a result. Phase 4's *"neither lever moves the cost"* is one of its anticipated
  outcomes and the one that routes to an ADR for presenting the console off the display thread —
  an expensive design change that would have been argued from an unwitnessed zero.
- The instrument is cheap and already has a template in this repository, so the rule costs a
  counter and a log column rather than a new mechanism.
- It generalizes past this plan. Any future "does X cost anything" — a post stage, a tap, a second
  encoder — inherits the same obligation, and the counter is the artifact that lets a later reader
  re-read the number without re-running the window.

### Negative

- **Counters in a render path are not free**, and this puts two of them there. They are integer
  increments off the hot arithmetic, but the rule does place instrumentation in code whose whole
  purpose is to be cheap. The alternative is a measurement nobody can act on.
- **It adds a `done when` clause to human measurement phases**, which are the phases most likely to
  be run under time pressure on a box that is briefly idle. A phase that cannot satisfy the clause
  has to be rescheduled rather than reported.
- **It does not make an un-witnessed historical number wrong**, and it cannot repair one. Every
  figure taken before the counter existed keeps whatever standing it had; the rule only governs
  what is taken next.

## Alternatives considered

**Force the window forward during the run and trust it** — raise the console, keep it unobscured,
assert by construction that it must have been presenting, and take the number. Rejected: it
substitutes an assumption for a reading at exactly the point where the reading is load-bearing, and
it is unverifiable after the fact — a log that records "the window was raised" records an intent,
not a present. It is also fragile in the specific way this project's measurement windows are run:
they are scripted, hands-off and unattended by design (an operator touching the app voids the arm),
so *nobody is watching the screen* during the only minutes that matter. The cheaper instrument does
not depend on anyone having looked.

**Infer the run from the cost** — treat a non-zero delta as proof the thing ran, and only distrust
an exact zero. Rejected: it is circular in the case that matters. The Meter Mono arm moved by
0.0 fps and the Clifford arm moved by *negative* 0.4, both of which are inside noise; there is no
threshold that separates "cheap" from "absent" without already knowing which one is true.

**Assert it in a test instead** — write a test that proves the present path executes, and let the
measurement rely on it. Rejected: the present path needs a real surface and a real presentation
engine, so no test in this repository can drive it (nothing anywhere touches `AuxTarget`,
`attach_aux` or `present_aux` outside the shell). A test that could exist would in any case prove
the path works, not that it ran *in this window on this box* — which is the claim a measurement
needs.

**Widen `frames_dropped` to cover the second surface** — reuse the show's existing counter rather
than adding one. Rejected: the two surfaces skip for independent reasons and one number covering
both would make an occluded console read as a dropped show frame, which is a worse ambiguity than
the one being removed.
