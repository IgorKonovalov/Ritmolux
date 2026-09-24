# ADR-0251 — A gated compile path has a named job, and the upstream reading is advisory

> **Status:** accepted 2026-09-24 (Plan 0227 close)
> **Date:** 2026-09-24
> **Related plan(s):** [0227](../plans/done/0227-the-gated-paths-get-their-jobs-and-a-red-upstream-stops-the-close.md)

> **Amended 2026-09-24, before acceptance — the upstream read is ADVISORY, never a refusal.** The
> Decision below originally parked a close over a red `origin/main`. That is withdrawn. **The
> conductor never pushes**, so `origin/main` advances only when the owner does it by hand: a blocking
> read makes every close hostage to a manual step the automation cannot perform, and one red arm stops
> the queue until a person both repairs *and* pushes. Worse, the conductor's merges accumulate
> locally, so `main` runs ahead of `origin/main` — a blocking read would stop today's close on the CI
> of an **older tree**, possibly one already repaired locally. Alternative A below is therefore
> promoted to the Decision and the blocking variant is the rejected alternative (Alternative E).
> The read still happens, it is still printed and recorded, and it never parks anything.

## Context

On 2026-09-24 `standalone/src/capture_mac/rt.rs:86` declared `struct OutputIvars` private while
`define_class!` leaked it into a public interface. That is `error[E0446]`, a hard compile error, and
it failed `check (macos-latest)` **in 50 seconds on every push to `main`**, plus the `macos` and
`studio-macos` jobs of `Release` on tags `v0.146.1` and `v0.147.0`. `linux`, `windows` and
`studio-windows` passed in both.

**Coverage was not the gap.** CI's `check` matrix is `[windows-latest, macos-latest, ubuntu-latest]`,
so macOS was compiled on every push and the failure was reported every time. What was missing is any
*consequence*: a red arm on `main` stopped nothing, and two release cycles went by.

**Nothing broken shipped, and that is the trap.** `release.yml`'s publish job carries
`needs: [macos, windows, linux, foobar, studio-macos, studio-windows]`, so the failed build skipped
publication. The release did not go out wrong — it did not go out at all, and **the absence announced
itself only as a missing artifact.** [ADR-0181](0181-the-gate-compiles-every-feature-a-release-ships.md) records
the identical shape for `v0.112.0`, which died at the same seam with
`cannot find function start_capture`. This is therefore the **second instance of one class**, and the
first lesson did not generalise past the feature that taught it.

**A close cannot check the commit it tags.** The close merges `main`, bumps, writes the tag, and the
push follows; CI only runs after that push. So the tagged commit has no CI result at the moment it is
tagged. What a close *can* read is the tip it is merging **onto** — and that would have sufficed here:
Plan 0225's close merged onto `15b08d57`, whose `check (macos-latest)` was already red.

Two questions here could each go either way. The first is **what compiles the code the default gate
cannot see**. The second is **what a red arm on `main` costs**, given that today it costs nothing.

## Decision

**A platform- or feature-gated compile path is compiled by a named job before any tag is pushed.**
A path the default gate cannot reach is invisible *by construction* rather than by omission, so it
gets a job that names it and a person who can read which subject broke. The roster today:

| Gated path | Compiled before a tag by |
|---|---|
| `cfg(target_os = "macos")` — `standalone/src/capture_mac/` | `check (macos-latest)` |
| `cfg(feature = "spout")` | the `spout` job (ADR-0181) |
| `plugin-foobar/` — C++ against the pinned SDK | **nothing; only `release.yml`'s `foobar` job, at tag time** |

That third row is the hole this ADR names. It is the same shape as the other two and has simply not
been hit yet.

**A close reports `origin/main`'s CI and never refuses over it.** Before it merges, a close reads the
newest conclusion of the `CI` workflow for `origin/main`, names the failing job when it is red, and
**proceeds either way**. Under the conductor the reading goes to the run log and to the digest's
`Needs you`; in a human-started close it is a printed line. **When the reading cannot be taken** — no
network, no `gh`, an unauthenticated `gh`, a shallow clone — it prints a notice in
[ADR-0016](0016-gpu-tests-opt-in-ci-scope.md)'s shape. **Green, red and not-read are three distinct
outputs** and none of them is silent.

**Nothing blocks on it, deliberately.** The conductor never pushes, so `origin/main` is a ref this
pipeline cannot advance; and local merges run ahead of it, so its CI describes an older tree than the
one being closed. A gate on a ref the automation cannot move is a deadlock wearing a gate's clothes.
What makes the signal land is that it is **named, attributed to a job, and repeated at every close**
until someone clears it — not that it stops work.

## Consequences

### Positive
- A red arm is named at every close, by the job that failed, instead of surfacing as two artifacts
  that never appeared. The signal already existed; this puts it where the work happens.
- The subject of the failure reaches a reader by name. `check (macos-latest)` is legible where "the
  release produced four artifacts instead of six" is not.
- The rule generalises past the instance that taught it, which ADR-0181's did not.

### Negative
- **The close gains a network dependency.** It is degraded rather than hard, so a close on a
  disconnected machine still completes — but the guarantee is then only as good as the last machine
  that could read. This is a real weakening of [ADR-0033](0033-testing-strategy-coverage-ratchet-and-pre-push-gate.md)'s
  offline stance, taken deliberately and confined to the close.
- **Nothing forces the repair.** The signal is named and repeated, and it can still be ignored — which
  is the risk Alternative A was first rejected for. It is accepted because the alternative deadlocks
  a pipeline that cannot push, and because a reading attributed to `check (macos-latest)` at every
  close is a materially louder signal than an absent release artifact.
- **"The newest CI conclusion" needs a named subject.** Three workflows run on a push — `CI`, `Pages`
  and, on a tag, `Release`. The check reads the `CI` workflow only; a red `Pages` does not stop a
  close. Written down because the obvious implementation reads "the latest run" and gets this wrong.
- A `foobar` job on every push pays the pinned-SDK fetch on every push. ADR-0181 accepted the same
  cost for `spout` and measured that a concurrent job does not move the wall clock.

### Neutral
- Nothing about the `release` job's `needs:` changes. It was correct and remains the last line.

## Alternatives considered

### Alternative A — A row in the conductor's digest, and no refusal — **ADOPTED 2026-09-24**
This was first rejected on the grounds that it is the same class of signal already missed twice. That
reasoning was wrong about the failure mode: the risk of an advisory is a signal nobody reads, and the
risk of a gate here is a pipeline that cannot run at all. See Alternative E. This is now the Decision.

### Alternative E — A close refuses over a red `origin/main` — **REJECTED 2026-09-24**
The original Decision. It lost on two counts, both structural rather than matters of taste. **The
conductor never pushes**, so `origin/main` moves only by hand: a refusal makes every close wait on an
action the pipeline cannot take, and one red arm halts the queue until a person repairs *and* pushes.
And **local merges run ahead of `origin`**, so the ref it reads describes an older tree than the one
being closed — a close could be refused over a failure already fixed in the commit it is about to
merge. A gate on a ref the automation cannot advance is a deadlock, not a gate.

### Alternative B — A watcher that polls CI after each push
It catches the tagged commit itself rather than its predecessor, which is strictly more information.
It lost on cost and shape: a long-lived process with its own failure mode, in a project whose
conductor already owns "stop and tell the owner", and whose owner pushes by hand at unpredictable
times. The close is a natural checkpoint that already exists.

### Alternative C — Teach `check-release-tag.mjs --remote` to require a green commit
The tag gate already has a remote mode. It lost because **the tag is written before the push**, so the
tagged commit has no CI result to read; the gate could only assert something about an ancestor, which
is what the close now does — and the close is where a park with a human-readable reason already
exists. Keeping the tag gate about tags keeps its subject single.

### Alternative D — Branch protection with required checks
GitHub would refuse the push itself. It lost because the owner pushes straight to `main` with no pull
request, so there is no review gate for a required check to attach to; adopting it would mean adopting
a PR workflow, which is a much larger decision than this incident supports.
