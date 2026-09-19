# ADR-0219 — The conductor can be asked to finish and stop, and the ask does not outlive the run

> **Status:** proposed
> **Date:** 2026-09-19
> **Related plan(s):** [0197](../plans/0197-the-conductor-becomes-operable.md)
> **Amends:** [0205](0205-an-approved-plan-runs-under-a-conductor-and-every-judgement-it-cannot-make-parks-the-plan.md)
> (the run's control surface)

## Context

`abort` stops the conductor and every session under it, and a step in flight runs again on the next
`run`. That is right for *"stop now"* — Ctrl+C does the same — and it is the only stopping behaviour
there is. What an operator asks for far more often is *"finish what you are doing, then stop"*: the
machine is wanted for something else, a `human` phase is ready and the GPU is busy, or the day is
over and nothing new should start.

Done by hand that means watching the record and timing an `abort`, because both failure modes sit
either side of one moment. Abort early and a review session's spend is lost and its step re-runs from
scratch; abort late and the next plan's session has already started, costing the same. On 2026-09-18
this was done three times in one session — once by polling `state/conductor.json` in a loop until
`0178` read `merged` and aborting inside the gap, and twice by noticing that the queue happened to
hold exactly one plan, so the run would end by itself.

The seam already exists and is already expressed once. `laneLoop` reads `ctx.stopRequested?.()` at
the top of every iteration, and `run --once` says the same thing at start time: run the plan you
pick, then stop. What is missing is saying it *during* a run. Signals cannot carry it — on Windows a
signal cannot run the conductor's handler at all, which is why `abort` kills the process tree — so
the ask has to be state the running process polls.

One question decides the shape rather than the plumbing: whether the ask survives into the next
`run`. A pause that persists turns the next morning's `run` into a process that starts, does nothing
and exits, which reads exactly like a hang and is a trap for the one operator this tool has.

## Decision

We will add a `pause` command that records the ask in the conductor's state directory, where the
lane loop already reads its stop request. A paused run **starts no further plan**; the plan in flight
runs through to its merge or its park, the lane records why it stopped, and the run ends normally.
The ask **does not outlive the run** — it is cleared when the run ends, and the run's terminal output
says it ended paused rather than exhausted. `pause` prints what it is now waiting for (the plan and
the step in flight, and how long that step has been running), because the gap between asking and
stopping is the suite's twelve minutes and an operator who cannot see it will reach for `abort`
anyway. `pause --off` clears the ask while the run is still live.

## Consequences

### Positive
- The operation the owner performs most often stops being a stopwatch exercise, and stops risking a
  lost session's spend.
- It reuses the seam `--once` already proves: one flag read in one place, with no new control over
  how a plan runs.
- A paused run ends with a reason in the record, so the digest can say the queue was not exhausted —
  which an aborted run today cannot distinguish from a crash.

### Negative
- **The wait can be long and is unbounded from the operator's side.** A plan that has just started a
  fix round will finish it first. The print makes the wait legible; it does not shorten it, and an
  operator who needs the machine now still wants `abort`.
- **The ask is a file the process polls, not a signal.** A conductor that has already died leaves the
  file behind, so `run` must treat a stale ask as absent rather than as an instruction — one more
  piece of state with a recovery rule.
- **Pausing is per run, so "do not start anything tomorrow" is not expressible.** That is deliberate,
  and the answer is not to start a run; anyone who wants the other behaviour will have to argue
  against this ADR's Context rather than add a flag quietly.

### Neutral
- Granularity is the plan, not the phase: the loop's existing stop check sits between plans, and a
  mid-plan stop is what `abort` plus `resume` already does.

## Alternatives considered

### Alternative A — A persistent paused state
The ask survives `abort`, a crash and a reboot until it is cleared. Rejected for the trap in its own
description: a paused conductor that has forgotten by morning is indistinguishable, from the
terminal, from one that has hung — and this project has one operator, who will be reading it at
07:00 rather than reading this ADR.

### Alternative B — Only `--once`, documented better
`run --once` already produces "one plan, then stop", so the operator could start every run that way.
Rejected because it is a decision made before the information arrives: the reason to stop — a `human`
phase coming due, the machine being wanted — is learned an hour into the run, and `--once` cannot be
said then.

### Alternative C — Pause at the step boundary
Stop after the session in flight rather than after the plan, leaving the lane open and the plan
half-run. Rejected because it delivers nothing `abort` plus `resume` does not, at the price of a
second stop granularity to explain, and because the operator's stated need is to reach a *merged*
plan — the state at which the machine is genuinely free.

## Notes

Raised as backlog 0253, 2026-09-18, by the owner after the third hand-timed stop of one day.
