# ADR-0267 — Judging a preset set is a studio session over an isolated player

> **Status:** proposed
> **Date:** 2026-10-07
> **Related plan(s):** [0254](../plans/0254-the-studio-judges-a-preset-set.md)

## Context

Plan 0232 judged the whole library with two throwaway scripts in the gitignored `target/p0232/`
(backlog 0277). `walk.sh` copied one family's `presets/<family>_*.toml` into a scratch directory and
launched the player with `RLX_PRESET_DIR` on that copy and `XDG_DATA_HOME` on a scratch data root.
It then moved the window to a Hyprland workspace and made it fullscreen. The owner pressed F1
(favourite) to keep a preset and F2 (hidden) to cut it, and left the rest unmarked, which meant
tune. `apply.py` then read the scratch `marks.toml` and wrote `keep`, `cut` or `tune` into the
plan's ledger table. `retune.sh` was the same launch over a named list of stems, and the presets
were edited while the player hot-reloaded them. The owner called the walk a large speed-up, and
every Phase 4 verdict came from it. A `cargo clean` deletes both scripts.

Four facts shape the replacement:

- **The scratch data root did two jobs, and only one is wanted.** It kept the walk's marks out of
  the owner's real `marks.toml`. It also replaced `config.toml`, so each walk ran on default audio,
  adapter and tier rather than on the owner's rig. On Windows the same trick would mean overriding
  `%APPDATA%`, because `preset_data_root` reads `XDG_DATA_HOME` only on Linux
  (`standalone/src/lib.rs`).
- **Marks are already on the wire.** ADR-0229 made `ctl/mark` and the `marks` event part of the
  control protocol. The `marks` event fires for a change made from the player's own keyboard too,
  and it carries both sets whole. So a parent sees every F1 and F2 the moment it happens, without
  opening the file. The same ADR makes the player the only writer of the marks file and says the
  studio never opens it.
- **The studio already drives exactly one player** (ADR-0175, ADR-0183). It edits presets in
  whatever directory the `roster` event names and resolves none of its own (ADR-0184). The walk is
  that player pointed at a different directory.
- **Window placement was Hyprland-specific**, and it was only there to put the window on a
  workspace and fullscreen it. The player's own `[output]` keys and its `F` key already do the
  fullscreen part on every platform.

## Decision

We will make judging a **studio session**, not a script and not a player mode. A session copies a
chosen set of preset files into a session directory under the studio's `userData`. It records a
content hash for each source file. It then restarts the studio's one player on the **windowed**
vector, whatever `playerMode` says, because the owner judges on the show window. The player runs
with `RLX_PRESET_DIR` on the copy and a new player flag, **`--marks <path>`**, on a session-owned
marks file. The owner's own `config.toml` stays in force, so the rig is the real one. The studio
sends `ctl/transport hold` so rotation does not move on by itself, and the owner steps with the
player's own keys.

The verdict rule is the one `apply.py` used: **favourite means keep, hidden means cut, neither means
tune, and favourite wins if a preset carries both.** The session folds the latest `marks` event into
that rule and nothing else. It never reads the marks file, which keeps ADR-0229's single-writer rule
whole. Ending the session appends one record per preset in the set to
**`<userData>/judging/ledger.jsonl`**. That file is state that outlives the session without being a
choice, so per ADR-0240 it has a file of its own and no settings key. Each line is one JSON object:
`v`, `run` (the session's ISO start), `set` (its label), `stem`, `name` and `verdict`. The file is
append-only and never rewritten.

The walk and the retune are **one mechanism**. A session that edited files offers to write the
changed copies back to their sources when it ends. It refuses any file whose source hash moved
during the session and names it. The **one user choice** is where sets are drawn from. It is
`settings.json`'s `judging.sourceDir`, which the studio's Settings panel edits. A set within it is
picked per session from three kinds: a family prefix (`<prefix>_*.toml`), the `proposed/`
subdirectory, or an explicit list of files. That pick is momentary view state and gets no key.

**No control-protocol row and no event field changes.** `--marks` is a launch flag. It is second
priority under ADR-0240: it overrides where the marks file lives for one run, it writes nothing to
`config.toml`, and it is added because a judging run must not touch the owner's real marks.

## Consequences

### Positive

- The loop survives `cargo clean`, runs on Windows, macOS and Linux, and needs no window manager.
  Every verdict comes from the owner's real rig rather than from a default config.
- One mechanism replaces two scripts. A retune is a walk in which files changed, and the write-back
  is explicit and checked against the source hashes.
- The ledger outlives any plan. A preset's verdict history across runs can be read in one place
  instead of being scattered across plan tables.
- No protocol change. The fold reads an event the studio already receives.

### Negative

- **A judging session takes over the studio's only player.** Ordinary editing stops for the whole
  session, and ending it restarts the player on its normal vector. A second player is ruled out by
  ADR-0183.
- **The ledger no longer writes into a plan.** `apply.py` filled a column of 0232's table directly.
  The studio must never write into `docs/`, so the view copies a Markdown table that someone pastes
  in. That is one manual step the script did not have.
- **Edits made outside the studio during a session do not reach the player.** The player reads the
  session copy, not the source. An author editing `presets/` directly in an editor sees nothing
  until they edit the copy instead, and the write-back refuses the clash rather than merging it.
- The player gains a CLI flag. That is one more row in `docs/configuration.md` and in the help
  text, and it is visible to anyone, not only to the studio.

## Alternatives considered

### Alternative A — A Node script in `scripts/`

This was the recommended shape at the interview: a portable `scripts/judge.mjs` doing what
`walk.sh` and `apply.py` did, with window placement as an optional hook. It lost because the owner
judges from the studio's world, which already has the library, the mark buttons, the editor and the
player connection. A script would build a second, parallel launcher for the same player. It would
also have needed the same `--marks` flag or the same data-root override to isolate marks. So it
saved no player work and gave up the editor the retune loop wants.

### Alternative B — A player CLI mode

A `ritmolux` flag that points at a set and writes verdicts to a named file. It lost because the
fold, the set selection, the ledger and the write-back are all editing-tool concerns. ADR-0175 put
those in the studio precisely so the shipped player stays lean. The player's part is the one thing
only it can do, keeping marks out of the owner's file, and that is the narrow `--marks` flag.

### Alternative C — Isolate by overriding the data root, as the scripts did

Keep the walk's `XDG_DATA_HOME` / `APPDATA` override and add no player flag. It lost because the
override also replaces `config.toml`, which put every 0232 verdict on a default rig. It also
needs a different variable on each OS, and on Windows it redirects everything else under
`%APPDATA%` for that child.
