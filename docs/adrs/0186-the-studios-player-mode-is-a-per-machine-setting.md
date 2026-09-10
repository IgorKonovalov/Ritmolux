# ADR-0186 — The studio's player mode is a per-machine setting, and the windowless path is a peer

> **Status:** proposed
> **Date:** 2026-09-10
> **Related plan(s):** [0167 — The studio becomes handable](../plans/0167-the-studio-becomes-handable.md)
> **Related:** [ADR-0183](0183-the-studio-drives-one-player-and-the-show-loop-is-extracted.md)
> (amended here — its Decision is written against a windowless path that was a subset and no longer
> is), [ADR-0175](0175-the-studio-is-a-separate-application-that-never-draws-a-frame.md) (the studio
> never renders), [ADR-0178](0178-the-studio-shell-conventions.md) (the studio shell),
> [ADR-0187](0187-the-preview-pipe-has-a-fixed-shape-and-names-its-true-format.md) (the pipe both
> modes hand the studio)

## Context

[ADR-0183](0183-the-studio-drives-one-player-and-the-show-loop-is-extracted.md) decided the studio
drives **one windowed player** that is both the show on the projector and the source of the preview
frames, and priced the cost in its own Negative:

> A user who only wants to edit still gets a player window. That window is the show, which is what a
> VJ wants; on a single-screen laptop with no projector attached it is a window in the way.

That cost arrived the first time anyone ran the packaged artifact on a single-screen machine
(backlog 0199, 2026-09-10): two windows, the show's overlapping the studio's, and no way to ask for
one. `DEFAULT_PLAYER_ARGS` is a module constant and `StudioSettings` carries exactly one key.

**What makes the choice reopenable is Plan 0159's own Phase 3.** ADR-0183 chose the windowed player
because the windowless `--stream` path was a silent subset of it: no control listener, two of the
eight events, no preset directory and so no editing loop at all. A studio pointed at it would have
had a picture and nothing to edit with. Phase 3 extracted the show loop, so `stream.rs` now builds
the same `crate::show::Show` the windowed path builds and `run.rs` binds the listener on that branch
before it greets. The mode ADR-0183 rejected as a subset is a full peer now. Nothing in the studio
asks for it, and ADR-0183's Decision sentence is still written as though it could not be asked for.

## Decision

**The studio's player mode is a per-machine setting.** `StudioSettings` gains `playerMode`, beside
`playerPath`, with two values:

- **`windowed`** (the default) spawns `--preview stdout`. The player opens a show window; the
  preview is a copy of what that window draws. This is ADR-0183's arrangement, unchanged.
- **`windowless`** spawns `--stream --sink stdout`. No window opens; the player runs the same show
  loop headless and the preview is the only picture there is.

Both modes carry `--events --control 127.0.0.1:0`, both emit the full event roster, and both hand
the studio a frame pipe described by the same `stream` event. **The studio's frame reader branches
on nothing** — it takes its geometry from `stream` in both modes, exactly as it does today.

The setting is per-machine and not per-launch, because the choice is stable per machine: a VJ with a
projector wants windowed every time, and the same person on a train wants windowless every time.

## Consequences

**Positive.**

- A single-screen editing session has one window. That is the whole of backlog 0199.
- The windowless pipe is cheaper again on top of
  [ADR-0187](0187-the-preview-pipe-has-a-fixed-shape-and-names-its-true-format.md)'s fixed shape:
  there is no swapchain, no present, and no second surface to draw.
- The mode is remembered, so the choice is made once per machine rather than argued with at every
  launch.
- `windowless` is `rgba8` by construction — the headless path renders into `HEADLESS_FORMAT`, which
  is `Rgba8UnormSrgb` — so a machine on that mode never exercises ADR-0187's second value.

**Negative.**

- **The preview is no longer by construction the audience's picture.** ADR-0183's stated benefit is
  that the thing being edited *is* the thing being shown. In `windowless` that guarantee is given up
  for the session, and the studio must not present the preview as a show feed while it holds.
- **A projector attached mid-session needs a relaunch.** The setting is read at spawn; changing it
  restarts the player, and whatever the player held — the active preset, the parameter overrides —
  is re-established from the file rather than carried across.
- **Two spawn paths to keep in step.** Every flag the studio depends on has to be present on both,
  and a flag added to one and not the other is a mode-dependent bug of exactly the kind Phase 3
  existed to end. A test asserts both argument vectors carry `--events` and `--control`.

## Alternatives considered

**A — A mid-session toggle.** Switch modes without relaunching, so plugging in a projector during a
set works. *Rejected:* it means tearing down and respawning the player live, and then deciding what
happens to unsaved edits, to the control socket, and to the frame port the renderer holds. Each of
those is a product question rather than a setting, and none of them is what backlog 0199 asked for.

**B — Windowless by default, windowed on request.** Invert it: editing is the common case and the
show is the deliberate one. *Rejected:* it revises ADR-0183's Decision rather than amending it, and
it is the wrong default for the product. The VJ with a projector is who the player is for; the
laptop session is the accommodation, not the centre.

**C — A menu item or a launch flag rather than a setting.** *Rejected:* the choice does not vary
within a machine's life. A control that has to be operated every launch to reach the same answer is
a worse form of a setting, and the backlog entry that raised this made exactly that argument.

**D — Leave it, and let the user move the show window.** *Rejected:* it is the first thing a tester
hits, the repair is spawn arguments and one settings key, and Phase 3 already paid for the hard half.

## Notes

This amends ADR-0183 rather than superseding it. What moves is the reach of one sentence — "the
studio drives one player" is now "the studio drives one player, in one of two modes" — and ADR-0183's
Outcome section already records that its Decision reads as though the windowless path were gone.
Everything else 0183 decided, including the show-loop extraction that made this possible, stands.
