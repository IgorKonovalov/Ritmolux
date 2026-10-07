# ADR-0265 — A preset ask carries a request id, and the player answers it at the drain

> **Status:** accepted 2026-10-07 (Plan 0252)
> **Date:** 2026-10-07
> **Related plan(s):** [0252](../plans/done/0252-the-lost-preset-ask-is-located-and-answered.md)
> **Extends:** [0176](0176-the-player-is-driven-over-osc-control-in-and-reports-on-its-standard-streams.md),
> [0221](0221-the-control-path-reports-what-it-did-not-do.md) and
> [spec 0003](../specs/0003-studio-control-protocol.md) (both tables)

## Context

A `ctl/preset` datagram sent on loopback sometimes never reaches the player's socket. Backlog 0219
measured it at 3 of 79 loaded runs on 2026-09-14, and Plan 0198 Phase 4 caught it again at 2 of 19 on
2026-09-19. ADR-0221's counters then excluded every candidate on the player's side of the socket. The
listener was alive, `recv_from` reported no failure, nothing was rejected and nothing was dropped, and
`ctl_received` did not move. The loss is in front of `recv_from`, and per-process counters cannot see
there. ADR-0221's first Negative names that limit.

Backlog 0220 is the same symptom one step later. A headless walk of the system roster stalls at an
ask that never reaches the screen. Two of its four candidates remain open: the datagram was lost, or
`select_preset_by_name` returned `true` and the dissolve never completed. Nothing on the wire tells
those two apart. The `preset` event fires only when the preset on screen changes, after the dissolve,
so its absence fits both candidates.

The user-visible cost is a library click in the studio that does nothing, with no error. The test
cost is a full suite that fails roughly once in five on a datagram, with a park, a resume and an
11-minute re-run each time (0219's 2026-09-15 reading). OSC over UDP stays the transport (ADR-0176):
a lighting desk on the same machine speaks it, and its loss model was accepted for slider nudges,
where the next value replaces a lost one. A preset selection is not a nudge. It is one deliberate
request, and nothing ever replaces a lost one.

Two facts about the tree constrain the shape. First, the decoder matches **exactly one type-tag
string per address** and refuses everything else (spec 0003, "accept only what it understands"), and
the studio's `protocol.spec.test.ts` diffs the spec's vocabulary table against its own union, one
row against one member. Second, the listener must not allocate per packet: an `Action` is `Copy` and
`Pending` holds the latest preset ask in an `Option<Name>`. Anything a request id adds must fit that
shape.

## Decision

We will add one vocabulary row, **`/rlx/v1/ctl/preset/req`** with arguments `s name, i req`. It
dissolves to the named preset exactly as `ctl/preset` does, with two differences. First, the player
answers it with one **`preset_ack`** event (`req`, `name`, `outcome`) on the event stream, emitted at
the **drain** that applied it rather than when the dissolve finishes. Second, an ask naming the preset
already on screen or already dissolving in is **inert**: it starts no new transition and is answered
`current`. The outcomes are a closed set: `selected` (a dissolve began), `current` (nothing to do)
and `refused` (the roster holds no such name; ADR-0221's `preset_error` still fires beside it). A
sender keeps **one** ask outstanding. It resends the same `req` on a bounded timeout until an ack
names that `req`, gives up after a bounded number of attempts, and treats a newer ask as superseding
the older one. The player answers only the ask that landed in a drain, because `Pending` already
keeps only the last preset asked for. The existing `ctl/preset` row is unchanged, answers nothing,
and stays the row a desk uses.

Before any of this ships, the reproduction reads the operating system's own UDP drop counters at a
failure, so the plan that builds the ack also records where the datagram went or that it was not
reproduced. The ack makes delivery robust. It does not stand in for finding the cause.

## Consequences

### Positive
- A lost preset ask is **resent rather than silent**, at the two places that met it: the studio's
  library click and the suite's two control-path tests.
- **Receipt and on-screen arrival become two separate facts**, which is the split 0220 cannot make
  today. If a `preset_ack` reads `selected` and no `preset` follows, the dissolve stalled. If no
  `preset_ack` arrives, the datagram was lost. 0220's last two candidates become two different
  readings.
- A resend is **idempotent by construction**: a duplicate that lands after the original is answered
  `current` and does not cut the dissolve already under way.
- The decoder keeps one tag string per address, and the studio's spec diff keeps one member per row.

### Negative
- **The vocabulary and the event roster both widen, forever**, and each is a contract (ADR-0176). A
  second preset row means a second spelling of one intent, and a reader has to learn why both exist.
- **A resend can mask a loss.** A suite that passes on its second attempt is a suite in which a
  datagram vanished, and the run says nothing unless something prints it. The plan requires every
  resend to be counted and printed in a passing run. Without that rule this decision would turn a
  visible defect into an invisible one.
- **`current` changes the meaning of a repeated ask on the new row only.** A plain `ctl/preset`
  naming the active preset still re-cuts, as it does today. The two rows therefore differ in more
  than the answer, and spec 0003 has to say so.
- **One outstanding ask per sender is a rule the player cannot enforce.** Two senders mixing the two
  rows in one frame can leave a `req` unanswered, and its sender will resend it over the other's
  choice until its attempts run out. Only the studio uses the new row, so this is a documented edge,
  not a guard.

### Neutral
- `preset_ack` is emitted from `apply_control_rest`, beside `pong` and the refused-selection
  `preset_error`. It is the third message on the control path that is answered individually.

## Alternatives considered

### Alternative A — Move the studio's control channel off UDP
Commands on the player's standard input, or a local stream socket. Either is reliable and ordered,
and the loss would stop mattering to the studio. Rejected for two reasons. It reopens ADR-0176, whose
Alternative C kept stdin as a possible *second* channel, not a replacement, because it reaches only a
parent process. And it hides an unexplained loss on the path a desk still uses. The loss is a defect
until it has a cause, and changing transport would remove the only sender that notices it.

### Alternative B — A sequence number on every datagram
Each row gains a counter, and the player reports gaps in `health`. It locates loss at datagram
resolution, which is what ADR-0221's Negative said was missing. Rejected because it changes every row
of the vocabulary to answer a question the operating system's own counters can answer with no
protocol change. It also leaves the click that does nothing exactly as it is.

### Alternative C — Answer in the existing `preset` event
Echo the `req` in the `preset` line the dissolve already produces. Rejected because `preset` fires
only when the preset on screen changes, and only once the dissolve completes. A resend of an ask
that already landed would never be answered, so the sender would resend until it gave up. And
receipt would be merged with arrival, which is the one distinction 0220 needs.

### Alternative D — An optional `i req` on the existing `ctl/preset` row
One row and two accepted tag strings. Rejected because it breaks both one-to-one rules the tree
relies on: the decoder's single tag string per address, and the studio's spec diff of one row to one
member. It also gives an existing row two behaviours keyed off its arity, where a reader of the table
would not look for them.

### Alternative E — Build the ack and skip locating the cause
Delivery becomes robust either way. Rejected by the owner at the interview. A loopback datagram that
disappears with every counter on the player's side reading zero is unexplained behaviour on a show
machine, and the readings that would explain it cost one test helper.
