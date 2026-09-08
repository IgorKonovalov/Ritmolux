# ADR-0174 — The Art-Net path is verified against a simulated rig

> **Status:** proposed
> **Date:** 2026-09-08
> **Related plan(s):** [0133](../plans/0133-the-engine-drives-the-lights.md)
> **Related ADRs:** [0145](0145-the-engine-drives-the-fixtures-directly-over-art-net.md) (the
> decision this verifies), [0072](0072-the-c-abi-ships-from-its-own-crate.md) and
> [0113](0113-milkdrop-presets-are-translated-ahead-of-time-onto-a-warp-mesh-idiom.md) (the
> out-of-`default-members` precedent this crate follows)

## Context

[ADR-0145](0145-the-engine-drives-the-fixtures-directly-over-art-net.md) puts an Art-Net emitter
inside `ritmolux.exe`, and [Plan 0133](../plans/0133-the-engine-drives-the-lights.md) was written
with the rig in the loop at every step: *"order the plan so the rig lights up before anything clever
happens"*. Six of its nine phases carry a done-when that only the physical fixtures can answer — a
colour on real sticks, a frame-time measurement *"against the real rig rather than a loopback"*, a
look that *"drives the rig"*, a human verdict that the room is the same room.

**That ordering assumed the rig is reachable, and it is not.** The fixtures are two BinColor
Pixel-16 controllers on a physical LAN that has to be set up; the plan cannot start at all while
they are out of reach, and every phase after the first would stall on the same wall.

**The rig is not one instrument, though — it is two.** Some of what it answers is *the bytes on the
wire*: whether the encoder is a conformant `ArtDmx` packet, whether all 24 universes are populated,
whether a frame is 510 channels, whether the last thing sent before exit is black, whether a look's
values change over time in the shape the author intended. None of that is a property of the
hardware; it is a property of what leaves the socket, and a receiver can read it more exactly than a
person can watch it. The rest is genuinely physical — whether the nodes latch, what refresh rate
they prefer, whether a chain's last stick goes stale, and every perceptual question about whether a
room reads.

**Art-Net is fire-and-forget UDP over port 6454. A node never replies.** So a sender exercises the
identical code path whether or not anything is listening, which is what makes the split above
mechanically available rather than merely conceptual.

**The workspace already has the shape for a tool like this, twice.** `core-cabi` and `milkconv` are
full workspace members held **outside `default-members`** so the everyday `cargo build` never
compiles them, while `--workspace` (CI, pre-push) and `-p <crate>` do. And the one dependency such a
tool needs — `image = "=0.25.10"` with the `png` feature — is already pinned in both `core` and
`standalone`, so this costs the dependency budget nothing.

## Decision

We will build **`rlx-artnet-sim`**, a workspace member outside `default-members` that never ships:
an `ArtDmx` **decoder**, a UDP **receiver** that reassembles datagrams into the rig's 24 x 170
raster, and a **PNG writer**. Its library is a `dev-dependency` of `standalone`'s integration tests,
which point a configured sink at an ephemeral loopback port and assert the decoded stream; its
binary is a viewer an author runs by hand to look at a frame. **Plan 0133's phases are reordered so
that every phase but the last human one verifies against this simulated rig**, and the physical rig
is asked only the questions it alone can answer, batched into a single session.

Two properties make the substitution honest rather than convenient, and both are load-bearing:

- **The decoder is validated against the Art-Net specification before our encoder exists**, using a
  hand-built byte fixture. If the simulator's only input were our own encoder, a shared misreading
  of the spec would round-trip perfectly and be invisible. This is why the simulator is Plan 0133's
  *first* phase rather than a companion to the sink.
- **Every property the simulator cannot see is enumerated, and moves to the rig phase rather than
  being dropped.** The list is in Consequences below; it is the price of this decision and the plan
  states it in the same words.

The send-cost measurement is the one case that needs neither the rig nor the simulator: it goes to a
**dead destination on a live LAN segment**, with the ARP entry pre-populated, because a node
contributes nothing to send cost except being an address that resolves.

## Consequences

### Positive

- **Plan 0133 becomes takeable today**, and eight of its ten phases never need the fixtures.
- **Several done-whens get stronger, not weaker.** *"Quitting the app leaves the rig black, verified
  by watching it"* becomes an assertion that the last datagram on every universe is all-zero — which
  a person watching a dark room cannot distinguish from a node that simply stopped receiving. Same
  for the 510-channel rule, which was a comment warning about a symptom and is now a test.
- **The look port gets a real comparison instead of a memory.** The existing Python bridge emits
  Art-Net too, so it can be pointed at the simulator on the same telemetry and captured as PNGs.
  Phase 6's *"a human says it is the same look"* becomes two image sequences side by side, which is
  the strongest form that judgement has ever been available in.
- **The instrument outlives the outage.** A regression in the fixture map or the encoder is caught
  in CI from now on, not on the evening of a set.
- **No new dependency**, and no new artifact in the everyday build.

### Negative

- **The simulator can pass while the rig fails, and the known ways are specific.** It cannot see:
  whether the nodes latch; whether a short frame leaves a chain's later sticks holding stale data
  (we assert the length instead, which is the cause but not the symptom); what refresh rate the
  hardware prefers or whether `ArtSync` matters; the sticks' actual gamma and primaries; and every
  perceptual question — whether the room reads, whether a resolved picture is too dim.
- **The fixture map is not confirmed until the last phase.** Six phases rest on a mapping —
  universe index is height, chains are 170 pixels — established by probe on 2026-08-29 and recorded
  nowhere in version control. This is the deliberate cost of the reordering: the user accepted it
  knowingly, on the argument that if the map is wrong the rework is confined to the config surface
  and does not reach the look grammar.
- **One evening now carries what five would have.** The rig session is large, and a plan whose
  human phase is large is a plan whose human phase is more likely to be partly done.
- **A third crate outside `default-members`.** Each one is a thing `--workspace` must keep green
  that the everyday loop never compiles, so it fails later and in CI rather than at the desk.

### Neutral

- The external Python bridge stays supported (ADR-0145 already keeps it) and gains a second use as
  the reference emitter for Phase 6's comparison.

## Alternatives considered

### Alternative A — Rig in the loop at every phase, as Plan 0133 was written

The strongest option on evidence quality and the reason the plan was ordered that way: every phase
would be verified against the thing it is actually for. Rejected because **the rig is unreachable,
so this option's cost is not "slower" but "not startable"** — and because most of what it verifies
per phase is bytes on a wire that a receiver reads more exactly than a person reads a room.

### Alternative B — Loopback receiver in tests only, no picture

The cheapest option: a UDP socket in an integration test, assertions on the decoded stream, no
viewer and no PNG. Rejected because it leaves **Phases 5 and 6 with no instrument at all** — a look
is a time-varying picture across 24 x 170 pixels, and *"editing the file changes the room"* has no
assertable form. Those two phases would have had to defer to the rig session, which is most of the
delay this decision exists to remove.

### Alternative C — A Python sidecar under `tools/`, on the `sd-filter` precedent

Reuses the receiver code already written in `WORK/lmv-lighting-probes/`, so the viewer would exist
in an afternoon. Rejected because **the assertions would then live outside `cargo`**: the decoder is
the encoder's inverse and its highest value is as a `dev-dependency` of a `standalone` test that CI
runs on every push. A sidecar that renders is useful; a sidecar that is the only thing checking the
wire format is a gate nothing runs.

### Alternative D — An off-the-shelf Art-Net visualizer

No code and no maintenance. Rejected for Alternative C's reason and one more: nothing about it is in
version control, so what it verified on one machine is not reproducible on another and cannot be
cited in a done-when.

## Notes

The one thing to get right in the receiver is **frame delimitation**, because Art-Net without
`ArtSync` has no frame boundary — 24 unsynchronized datagrams simply arrive. The simulator closes a
frame when a universe index repeats, which is exact for a sender that emits each universe once per
frame and is the sender we are building. `ArtSync` remains unexamined, as ADR-0145 and Plan 0133
both record.
