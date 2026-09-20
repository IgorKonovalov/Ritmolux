# ADR-0229 — The studio marks a preset over the control protocol, never by opening the file

> **Status:** accepted 2026-09-20 ([Plan 0205](../plans/done/0205-the-library-becomes-navigable.md))
> **Date:** 2026-09-19
> **Related plan(s):** [0205](../plans/done/0205-the-library-becomes-navigable.md)
> **Relates to:** [ADR-0176](0176-the-player-is-driven-over-osc-control-in-and-reports-on-its-standard-streams.md)
> (the protocol), [ADR-0177](0177-a-fourth-skill-lane-builds-the-studio.md) (the lane boundary),
> [ADR-0228](0228-a-preset-mark-is-user-state-keyed-by-name-in-its-own-file.md) (what a mark is),
> [spec 0003](../specs/0003-studio-control-protocol.md) (the contract this widens)

## Context

[ADR-0228](0228-a-preset-mark-is-user-state-keyed-by-name-in-its-own-file.md) puts preset marks in
a file the standalone owns. The studio must show and set them too — that was the owner's scope call
in the same interview — and the studio is a **separate process**:
[ADR-0176](0176-the-player-is-driven-over-osc-control-in-and-reports-on-its-standard-streams.md)
has it drive the player over OSC and read the player's standard streams, with
[spec 0003](../specs/0003-studio-control-protocol.md) as the contract.

So there are two processes and one mutable file, and the question is who may touch it. The tempting
answer is "both" — the file is small, well-formed and sitting right there, and the studio already
knows the preset names. That is the answer this ADR exists to refuse.

Two facts make it a real decision rather than an obvious one. The studio and the player run on the
same machine and can genuinely both open the file, so nothing physical prevents it. And spec 0003
is deliberately minimal — its whole control vocabulary is `/ctl/param`, `/ctl/param/clear`,
`/ctl/params/clear`, `/ctl/ping`, `/ctl/preset` and `/ctl/transport` — so adding to it is a cost
that has to be justified rather than a formality.

## Decision

We will widen spec 0003 with a **mark message**, and the **player remains the only writer** of the
marks file. The studio never opens it, for reading or for writing.

The player reports the current mark sets on its event stream, so the studio renders from what the
player told it rather than from its own copy of the truth. A studio with no player attached shows
no marks — which is correct, because without a player it does not know which library is loaded.

This is the same boundary [ADR-0177](0177-a-fourth-skill-lane-builds-the-studio.md) already draws
for the `studio-builder` lane: *"a protocol widening it needs is a feedback note to architect,
never a shim on its side."* Reaching around a missing message by opening the player's file is that
shim, wearing a filesystem instead of a function call.

## Consequences

### Positive

- **One writer, so there is no race to reason about.** Two processes writing one file needs either
  a lock, an atomic-rename discipline, or a last-writer-wins shrug; this needs none of them,
  because the second writer does not exist.
- **The studio cannot drift from the player.** What it displays is what the player reported, so a
  mark set in the app appears in the studio and a mark set in the studio is applied by the same
  code path that applies a hotkey.
- **The file's location stays private.** The studio needs no knowledge of where the standalone puts
  its user state, which means moving that file later is not a two-process migration.

### Negative

- **The protocol grows, and spec 0003's minimality was a stated virtue.** This is the first message
  that is about *library state* rather than about the frame being rendered, which is a genuine
  widening of what the protocol is for, not just of how much it carries.
- **A round trip for data the studio could have read in a millisecond.** Marking in the studio is
  now bounded by the player answering, and a player that is busy or wedged makes the studio's mark
  UI feel unresponsive in a way a direct file read never would.
- **The studio shows nothing before a player attaches.** A library view that could have listed
  favourites from the file alone now waits, which is a real UX cost in the one screen where it
  bites.
- **The player must report marks it did not change.** A mark set from the app has to reach a studio
  that did not ask, so the event stream carries unsolicited state — the player cannot simply reply
  to requests.

### Neutral

- The C ABI is untouched. The foobar component does not participate, exactly as in ADR-0228.

## Alternatives considered

### Alternative A — the studio reads and writes the marks file directly

Skip the protocol; both processes open the file. **Rejected because it makes two writers out of
one, for a saving measured in one message.** It also puts the standalone's private user-state path
into the studio's source, so the two are coupled through a filesystem layout that neither one's
contract mentions — the worst kind of coupling, because nothing breaks at build time when it moves.
ADR-0177's lane boundary names this shape specifically.

### Alternative B — the studio keeps its own marks

Let each surface mark independently. **Rejected because two answers to "is this a favourite" is
not a feature.** The marks would diverge on the first use, and the `hidden` set — which
[backlog 0256](../design-backlog.md) intends to read as evidence — would be split across two files
with no rule for combining them.

### Alternative C — the studio asks the player to open a mark editor

Have the studio send a "show marks UI" command and let the player render it. **Rejected because it
inverts the two programs.** The studio is the editing surface and the player draws frames
([ADR-0175](0175-the-studio-is-a-separate-application-that-never-draws-a-frame.md)); putting library
management UI in the player so the studio can avoid a message is a worse protocol than the one
message.
