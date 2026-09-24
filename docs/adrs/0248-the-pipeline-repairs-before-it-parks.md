# ADR-0248 — The pipeline repairs before it parks: a readiness check at the front, one repair per red, and a close split from its review

> **Status:** proposed
> **Date:** 2026-09-24
> **Related plan(s):** [0226](../plans/0226-the-conductor-stops-waiting-for-the-owner.md)
> **Amends:** [0205](0205-an-approved-plan-runs-under-a-conductor-and-every-judgement-it-cannot-make-parks-the-plan.md)
> (the park triggers, the close lock's span, the shape of the close)

## Context

ADR-0205 made every judgement the conductor cannot make a park, and made that list closed. Two of
its entries are not judgements. **A red gate** and **a merge that conflicts** are failures a session
can usually repair, and the conductor already runs sessions that repair: the review's fix rounds. It
parks on these two anyway, and waits for the owner to do the repair by hand.

The run record for 2026-09-22 to 2026-09-24 (`state/conductor.json`, audited 2026-09-24) holds 10
parks across 9 plans. The machine ran about 8 h of the 42 elapsed. Three of those parks were this
shape, and a fourth was preventable before any spend:

- **0224 `gate_red`** at pre-review: one test red after the owner's hand commit for the plan's
  `human` phase. It waited 1.25 h for a fix a `dev` session could have written.
- **0224 `merge_conflict`**: the review was clean with 0 blockers and 0 majors, and the close's
  `git merge main` conflicted in two docs and in `standalone/src/config.rs` and `settings.rs`. The
  owner resolved it by hand in 11 minutes. `resume` then re-ran **the whole review from round 1**,
  which cost $15.29 and 29 minutes, because a review session that parks records no verdict. The clean
  verdict it had reached was lost with the park.
- **0206 and 0223 `plan_wrong`**: an implement session found the plan inconsistent with itself or
  with the tree. For 0223 that was Phase 4's *What* and *Done when* naming different stages, after
  $55.94 of implementation. For 0206 it was Phase 2 needing a seam outside its *Files touched*. Both
  are readable from the plan and the tree before a line is written.

The close lock has a cost of its own. ADR-0205 takes it before the review starts, so that the version
bump and tag the same session writes land on the `main` they were computed against. Reviews ran
8 to 29 minutes, and 0224's lane waited 15 minutes on 0207's lock. The lock guards the bump and the
fast-forward. The review is under it only because it shares a session with the close.

## Decision

We will make five changes to the conductor's pipeline. **The owner's approval and the push are
unchanged**, and everything not named here parks exactly as ADR-0205 says.

1. **A readiness check runs before the first implement session.** It is a fresh, read-only
   `architect` session handed the plan and the lane. It checks that each phase's *What*,
   *Files touched* and *Done when* agree with each other and with the tree. It checks that every
   path named either exists or is declared new, that each seam a phase relies on is inside some
   phase's files, that each done-when is runnable under the session allowlist, and that no phase
   depends on a `human` phase marked `**Blocks merge:** no` (ADR-0249). It grades consistency, not
   the design. It ends `ready`, or parks `plan_wrong` naming the phase and the contradiction, before
   any implementation spend. It runs once per plan and again on resume only if the plan file changed
   since.
2. **The lane merges `main` before the pre-review gate.** The conductor runs `git merge main` itself.
   The gate and the review then see the tree that will merge, and conflicts surface while the plan is
   still an implementer's.
3. **A merge that conflicts gets one `merge` session.** A fresh `dev` session is handed the
   conflicted paths, or a `studio-builder` one when every path is under `studio/`. It resolves the
   conflict, commits, and the gate runs. This applies wherever the conductor or a session merges:
   the early merge, the close, and the re-merge before the fast-forward. The conductor starts one
   merge session per conflict and parks `merge_conflict` if that session does. **The close session
   never resolves a code conflict itself.** It is `architect`, and the architect writes no code. It
   parks the conflict back to the conductor, which runs the merge session and then starts the close
   again.
4. **A red gate gets one `repair` session per stage.** At any stage, a red conductor gate starts a
   fresh implementer session handed the gate log. It is `studio-builder` when the failing command is
   one of the studio's checks, and `dev` otherwise. It commits and the stage's gate runs again. A
   second red at the same stage parks `gate_red`. At most three repairs run per plan. A repair on a
   tip a close produced (`post-close`, `remerge`) is followed by the conductor moving the annotated
   tag, which `moveTag` already does. The repair commit is never reviewed, so the digest lists it
   under **Needs you** by SHA.
5. **The review and the close are separate sessions, and a clean verdict outlives a park.** The
   review session runs Mode 4 without the close lock and ends on a verdict. The conductor records it
   with the tip it graded. Blockers and majors go to fix rounds exactly as before. A verdict with none
   takes the close lock and starts a **close session**, which repairs what ADR-0209 lets a close
   repair, merges `main`, does the bookkeeping and the bump, runs the gate and tags. A close that
   parks keeps the verdict, and `resume` starts the close again rather than a review. The verdict is
   reused while every commit after its graded tip is a merge of `main` or a commit the conductor
   recorded as made by a close, merge or repair session. **Any other commit starts a fresh review
   round**: an owner's hand fix is new code, and nothing has reviewed it. **The close lock is taken
   before the close session and held until `main` has fast-forwarded, and never over a review.**

## Consequences

### Positive

- The four parks above would not have waited on the owner. 0224's two would have been one merge
  session and one repair session. 0206's and 0223's would have parked before any implement spend,
  $55.94 of it in 0223's case.
- A resume after a close-time park costs a close, not a review.
- Two lanes no longer serialize on each other's reviews, only on each other's bumps.
- The review sees the merged tree, so what it approves is closer to what reaches `main`.

### Negative

- **More sessions per plan.** A readiness session and a separate close session join every plan.
  The close must re-read the plan to do its bookkeeping, context the review already held. This is an
  estimate, not a measurement: a few dollars per plan, against the $15 one lost verdict cost.
- **A repair on a closed tip reaches `main` unreviewed.** The gate is the only judge of a
  `post-close` or `remerge` repair. The digest names each one by SHA, but the owner reading it
  before the push is the whole mitigation.
- **A repair can make a gate green the wrong way**, for example by weakening a test. The repair
  prompt forbids editing an assertion to pass. The review catches it at pre-review and fix stages.
  After the close, only the owner's reading catches it, for the reason above.
- **The readiness check is a judgement too, and it can be wrong in both directions.** A false
  `plan_wrong` parks a good plan before any work. A missed contradiction costs what it costs today.
  Its verdict names the phase and the contradiction, so the owner can overrule it by editing the plan
  or by resuming.
- **The pipeline grows five new session kinds' worth of prompts, outcomes and tests** in a tool that
  has already taken a dozen ADRs of its own.

## Alternatives considered

### Alternative A — Keep parking, but make the parks cheaper to act on

Better inbox text and a one-command resume. Rejected because the audit's cost is the wait for a
person, not the reading. 0224's red waited 1.25 h, and the repair itself was a few minutes of
session work.

### Alternative B — Let the close session resolve every conflict itself

This is what the owner first chose in the interview. It is one session fewer. Rejected on the lane
split: the close session is `architect`, and 0224's conflict was in Rust. Routing the code half to a
`dev` session keeps the outcome the owner asked for, with every conflict resolved and the gate
deciding, without the architect writing code.

### Alternative C — Unlimited repair rounds

It would converge on more reds. Rejected because a red that survives one repair is usually a wrong
plan or a wrong test, which is a judgement. A loop would spend on it until the budget cap parks it
anyway, later and dearer.

### Alternative D — Readiness as an architect checklist at approval time only

It costs no session. Rejected because the architect approving a plan is the same context that wrote
it, and both of 2026-09-23's contradictions survived approval. The check has to be a fresh reader,
and the conductor is the one place that runs every plan.
