# ADR-0183 — The studio drives one player, and the show loop is extracted so every mode runs it

> **Status:** accepted 2026-09-10 (Plan 0159), with an Outcome
> **Date:** 2026-09-10
> **Related plan(s):** [0159 — The studio opens](../plans/done/0159-the-studio-opens.md)
> **Related:** [ADR-0175](0175-the-studio-is-a-separate-application-that-never-draws-a-frame.md)
> (the studio never renders; its Decision named the headless run as the preview — see its
> `Outcome`), [ADR-0176](0176-the-player-is-driven-over-osc-control-in-and-reports-on-its-standard-streams.md)
> (the vocabulary and the event roster, both unchanged here),
> [ADR-0178](0178-the-studio-shell-conventions.md) (the studio shell),
> [ADR-0143](0143-the-operator-console-is-a-second-surface-and-the-shell-owns-its-meaning.md)
> (the preview intermediate the windowed path already draws through)

## Context

Plan 0159's first two phases landed: the studio opens, spawns a player, and paints its frames.
Phase 1 spawned it the way the plan and ADR-0175 both say — headless, `--stream --sink stdout
--events --control`. Building the phases on top of that found three things about the path.

**It binds no control listener.** `standalone/src/run.rs` returns from the headless branch
before `resolve_control` is reached, so `--control` is accepted as a token and no socket is
opened. `hello` reports `"control":null`, which is the player answering honestly; nothing is
broken, and nothing can be driven.

**It emits two of the eight events.** `hello` and `stream` only. `preset`, `roster`,
`preset_error`, `preset_warning` and `health` are emitted from `standalone/src/app_state.rs`,
which is the windowed path.

**Decisively, it has no editing loop at all.**
`standalone/src/preset_dir.rs` — the module that resolves the per-user preset directory, seeds
it, watches it and reloads an edited file — is imported by `app_state.rs` and by nothing else.
`reload_presets` is also where `preset_error`, `preset_warning` and `roster` are raised. So on
the headless path the studio's whole reason to exist, *save the file and the player picks it
up*, is not merely unreported: it does not happen.

**The headless path is not missing machinery, it is missing management.** `stream::run` builds a
full `Renderer`, a `Director` with rotation, and preset-selection-by-name; it holds the state the
missing events describe and never reports it. What it lacks is the show management the windowed
path performs around the same state: a preset directory, a watcher, a listener, and the
emissions. Those two loops are not two different jobs — one is a strict subset of the other's
responsibilities, and the subset is silent.

Meanwhile the windowed path has every piece already, and Plan 0158 Phase 6 shipped
`--preview stdout`, which writes the identical pipe format the studio's frame reader consumes.

Plan 0159 assumed **two** players at once: the studio's headless editing child, and a separate
fullscreen show player on the projector. It records "two audio captures on one machine" as a
risk to confirm rather than to remove.

## Decision

Two halves, and the second is what stops this recurring.

**The studio drives one player.** Not an editing child beside a show process: **one windowed
player** that is both the show on the projector and the source of the studio's preview frames,
spawned `--preview stdout --events --control`. One capture, one adapter, one preset directory,
one thing to reason about — and the picture the author is editing is, by construction, the
picture the audience is watching.

**The show loop is extracted once, and every mode runs it.** Preset-directory resolution and
seeding, the watcher and its reload, the `Director`, the event emissions and the control drain
move into one place that the windowed and the headless paths both call. What stays different
between the two is the window and which sink the frames go to. The headless path gains the
listener, the watcher and the full event roster **by construction rather than by a second
copy** — and `shot --render`, the Spout sender and a headless CI run become drivable and
observable for the same reason.

The vocabulary and the event roster do not change. This ADR is about which code paths perform
them, not about what they say.

## Consequences

### Positive

- **The editing loop rests on the watcher that already ships**, rather than on a second one
  written for the studio. `preset_dir.rs` reloads an edited file in about 150 ms and has been
  the preset author's loop for many plans.
- **Plan 0159's two-capture risk is discharged rather than mitigated.** One player captures once.
- **A preview that is a different process is a preview that can differ.** One player removes the
  class: adapter, tier, preset directory and rotation state are shared because they are the same.
- **The silent subset stops being silent.** After the extraction there is no path that holds show
  state and declines to report it, which is the property that cost Plan 0159 two phases.

### Negative

- **A user who only wants to edit still gets a player window.** That window is the show, which is
  what a VJ wants; on a single-screen laptop with no projector attached it is a window in the way.
- **The extraction is refactoring with no visible feature**, in `run.rs` and `app_state.rs` — two
  of the files most likely to be in flight in another lane. It is the phase most likely to
  conflict and the one with the least to show for itself.
- **The headless path gains a preset-directory dependency it did not have.** A `--stream` run in
  CI, or on a machine with no per-user directory, must degrade to the embedded set rather than
  fail; `PresetDir::Unresolved` already exists to say so and the extraction has to honour it.
- **Preview frames now arrive at the display's rate and the readback's size**, not the headless
  default of 640x360 at 30. The studio reads the geometry from the `stream` event either way, so
  nothing in it changes — but the preview's cost is now the show's cost, which is what
  Plan 0158 Phase 6 measured and Plan 0159's on-device phase reads.

## Alternatives considered

### Alternative A — Widen the headless path in place
Bind the listener and add the watcher, the preset directory and the missing emissions to
`stream::run`, leaving `app_state.rs` alone. Rejected on drift, which this project already
legislates against for exactly this shape: spec 0003 requires a `ctl/transport` verb to reach the
same applier the console's strip reaches, because *"two surfaces that agreed by having two copies
of the rule would drift"*. Here the duplication would not even be partial — the two loops'
responsibilities are identical.

### Alternative B — The studio drives a windowed player, and nothing else changes
Free, and it unblocks Plan 0159's remaining phases today, because `--preview stdout` already
works. Rejected **as a complete answer** and adopted as the studio's half: on its own it leaves
the headless path silently lacking a listener, a watcher and five of eight events, which is the
trap that produced this ADR. The next person to point a tool at `--stream` finds it the same way.

### Alternative C — A hidden window
`--preview stdout` with the window never shown, so the windowed path runs unchanged and the child
stays invisible. Rejected twice over. It buys invisibility only, and invisibility stops being
wanted the moment the studio drives one player that *is* the show. It also rests on an unproven
property: the windowed loop is paced by the swapchain, and a window the compositor never presents
may throttle or stall it — a preview that stops is worse than one in a window.

### Alternative D — Two players, as Plan 0159 assumed
The studio's own headless editing child plus a separate fullscreen show. Rejected: two loopback
captures, two adapters, two preset directories, two rotation states, and a preview whose picture
is only ever *similar* to the show's. The plan carried it as a risk to confirm; the better answer
is not to have it.

## Notes

ADR-0175's Decision named the live preview as *"the player itself, run headless with its frame
tap writing raw frames to the studio over a pipe, and later the windowed player with a preview
copy of its output"*. This inverts that ordering: the windowed preview copy is the mechanism, and
the headless run is what the extraction repairs. ADR-0175 is accepted and append-only, so that is
recorded as a dated `Outcome` there rather than by editing its body.

## Outcome — 2026-09-10, Plan 0159

**The Decision reads as though the windowless path were gone, and it is not.** Plan 0159 Phase 3
extracted the show loop precisely so `--stream --sink stdout` runs the same management the window
does, and it does; what shipped is a studio whose `DEFAULT_PLAYER_ARGS` are fixed at
`--preview stdout`, with no way to ask for the windowless one. On a one-screen machine that is a
show window permanently in the way of the editor — a cost this ADR's own Negative priced, and which
turned out to be the first thing a user hit. [backlog 0199](../design-backlog.md) carries it, and
the repair is mostly already built.

**A resize of the show window silently ends the preview.** `open_preview_pipe` runs once, at window
creation; `Renderer::resize` rebuilds the preview target at the new size; `StdoutSink::send` refuses
a frame whose size is not the announced one; and the writer thread breaks on that error, discarding
its message. So the preview this ADR makes a copy of the show's own output stops the first time the
show's output changes shape, and nothing says so. [backlog 0201](../design-backlog.md) carries the
diagnosis. Whichever repair is taken — re-emitting `stream` on resize, or refusing the resize while
a preview is open — the first is a change to [spec 0003](../specs/0003-studio-control-protocol.md),
because it makes `stream` a repeatable event rather than a once-before-the-first-frame one.
