# ADR-0221 — The control path reports what it did not do

> **Status:** proposed
> **Date:** 2026-09-19
> **Related plan(s):** [0198](../plans/0198-the-control-path-stops-failing-quietly.md)
> **Extends:** [0176](0176-the-player-is-driven-over-osc-control-in-and-reports-on-its-standard-streams.md)
> and [spec 0003](../specs/0003-studio-control-protocol.md) (the event roster)

## Context

Two failures on the control path have been observed and neither could be diagnosed, for the same
reason: the path reports what it did and is silent about what it did not.

A `ctl/preset` datagram sent on loopback never reached the listener's queue in 3 of 79 loaded runs
on 2026-09-14. The test's own report was as good as it could be — *"nothing reached the listener
within 5s; listener counters: rejected 0, dropped 0"* — and it still cannot say whether the listener
thread was alive, whether `recv_from` was failing, or whether any datagram arrived at all. `listen`
swallows every receive error with a bare `continue` and counts nothing, because a read timeout is the
ordinary case and is how the stop flag gets read. So a listener whose receives keep failing is
indistinguishable from an idle one, and a socket that received nothing is indistinguishable from one
that received something it discarded.

A headless walk of the system roster stalled at `emitter` twice in the same 79 runs. The player kept
drawing at 30 fps, answered the ping sent behind the ask, and emitted no `roster`, `preset` or
`preset_error` in two minutes. Four candidates remain undistinguished: the datagram never reached the
queue, `select_preset_by_name` returned `false`, it returned `true` and the dissolve never completed,
or the selection was overridden. The second of those is the one a user meets: `apply_to_renderer`
records `applied.switched` and **a `false` produces nothing** — no event, no counter, no line. A
studio click on a preset that the player will not select looks exactly like a click on a preset it is
about to select.

The constraint that produced the silence is real and stays: this is a show machine, the listener
runs beside a render loop, and a line per occurrence on a failing socket is a flood. Counting is not
logging, and the `health` event already carries three counters of exactly this kind.

## Decision

We will make the control path report its non-events. The listener counts **datagrams received** and
**receive errors that are not the ordinary timeout**, separately, and publishes whether it is still
listening; those readings join the `health` event as additive fields under the same protocol
version, alongside `ctl_rejected`, `ctl_dropped` and `ctl_refused`. And a `ctl/preset` naming a
preset the renderer does not select emits a **`preset_error`** carrying the asked-for name and a
message saying the selection did not take — the event a studio already renders, rather than a new
one, because the studio's user-visible fact is identical to a preset that failed to load: the thing
they asked for is not on screen.

## Consequences

### Positive
- The two open failures become diagnosable from their own reports rather than from re-running them
  79 times: a lost datagram, a dead listener and a failing socket are now three different readings.
- The studio's silent click gets a cause. A refused selection reaches the same surface a load error
  does, so the operator is told rather than left watching.
- `health` keeps being the one place the control path's running totals live, which is where a reader
  already looks.

### Negative
- **The counters are per-process totals and cannot name a datagram.** They say that *some* receive
  failed, not which send it belonged to, so a single lost `ctl/preset` in a stream of them is still
  only inferable. Nothing short of per-datagram sequencing would fix that, and that is a protocol
  change this decision deliberately does not make.
- **Two more fields on `health`, forever.** The event is emitted once a second for the life of a
  run, and every consumer parses them; additive under the same `v` is cheap, but the roster is a
  contract and this widens it.
- **`preset_error` now carries a case that is not a load error.** Its `file` field will hold a name
  that may not be a file. That overload is the price of not adding an event, and it is stated in
  spec 0003 rather than left for a reader to discover.

### Neutral
- The listener's error handling does not change shape — the loop still continues on a failed
  receive, because there is nothing else it can do; it simply counts first.

## Alternatives considered

### Alternative A — Log the receive error instead of counting it
One line per failure, gated behind a verbosity flag. Rejected on ADR-0176's own ground: the listener
shares a machine with a running show, a socket that fails keeps failing, and the flood arrives
exactly when something is already wrong. A counter costs one atomic and is readable a second later.

### Alternative B — Instrument under `cfg(test)` only
Keep the shipped binary unchanged and give the two tests what they need. Rejected because both
failures were observed in the real binary under real load, the studio needs the same signal to
explain a click, and an instrument that exists only in the test build cannot say anything about the
build a user runs.

### Alternative C — A new event for a refused selection
`ctl_error`, or a `preset_refused`. Rejected because it widens the roster for a fact the existing
event already expresses to the only consumer that reads it: from the studio's side, "the preset you
asked for is not on screen, and here is why" is one case, and splitting it into two events means two
render paths for one message.

## Notes

Raised as backlog 0219 and 0220, both 2026-09-14, by `dev` during the ADR-0193 diagnosis. Both
entries say the same thing in their own words — *"those two gaps are the first thing to close"* —
and both stay live after this decision until the cause is named; this ADR buys the evidence, not the
fix.
