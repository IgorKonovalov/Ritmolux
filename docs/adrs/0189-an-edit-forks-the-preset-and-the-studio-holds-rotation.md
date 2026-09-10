# ADR-0189 — An edit forks the preset on first touch, and the studio holds rotation while attached

> **Status:** proposed
> **Date:** 2026-09-10
> **Related plan(s):** [0168 — The studio stops surprising the author](../plans/0168-the-studio-stops-surprising-the-author.md),
> [0167 — The studio becomes handable](../plans/0167-the-studio-becomes-handable.md) (its smoke run
> is this ADR's evidence, and its Phase 7 is blocked on this)
> **Related:** [ADR-0184](0184-the-player-reports-what-it-loaded-and-the-studio-re-derives-nothing.md)
> (the file under the editor is the player's own `preset.file`, and stays so),
> [ADR-0183](0183-the-studio-drives-one-player-and-the-show-loop-is-extracted.md) (the picture being
> edited is the picture the audience sees),
> [ADR-0175](0175-the-studio-is-a-separate-application-that-never-draws-a-frame.md),
> [ADR-0178](0178-the-studio-shell-conventions.md),
> [ADR-0186](0186-the-studios-player-mode-is-a-per-machine-setting.md)

## Context

Plan 0159 Phase 6 gave the studio its editing loop and priced it as *the picture follows the finger
and the disk sees one write per gesture*: a slider's **release** writes the edited constant back into
the file the player is watching, atomically, and the reload that follows is what puts the new value
on screen. Every editing surface added since has been built on that model — a palette name, a stop
drag, a stop recolour, any structure-tab key, any map row (Plan 0167 Phase 6), and `Ctrl+S`.

The model assumes **the preset under the editor is the one the author meant to edit**. Rotation
falsifies that assumption, and nothing in the studio or in the plans records it. Rotation is on by
default with a 20–130 s dwell from the operator config, so the preset under the editor changes by
itself while the author works, and an edit lands in whichever file rotation most recently brought in.

A smoke run on 2026-09-10 measured what that costs. In six minutes, **three different presets each
received `system = "swarm"`** from what the owner experienced as clicking the system picker once —
`attractor_cliffordgallery` at 20:37, `attractor_dejonggallery` at 20:38, `attractor_ink` at 20:39.
One gesture per file, one file per dwell. Four files in the watched directory were modified in all;
`attractor_clifford.toml` had all five `[palette] stops` recoloured, 49 differing lines. There was no
prompt, no undo and no record: the author's own account of the session is of clicking a picker, not
of editing three presets. Recovery was by hand, from the shipped set.

Two further facts decide the shape rather than merely motivating it:

- **A palette or structure edit reaches the picture only through a file write.** `ctl/param` carries
  named parameters, so a recolour that is not written is not visible. Any model that defers all
  writes to an explicit save gives up the live loop for exactly the edits the smoke run named.
- **Nothing in the tree asserts the destructive half.** `ParamPanel.test.tsx` pins that `onCommit`
  fires once on release — a callback, not a write. `toml.test.ts`'s byte-equality tests
  (`changes exactly one line, and only the value on it`; `writes back byte for byte when the value
  does not move`) are about **edit fidelity**, not about which path the edit lands on, and they hold
  unchanged under any destination. The contract being reversed is Plan 0159 Phase 6's done-when
  sentence, not a test.

An edit to a preset from the **embedded** set has nowhere to land at all: `useActivePreset` reports
`status: 'embedded'` and disables writing, which is correct under the current model and is a dead end
under any model that has somewhere else to put the bytes.

## Decision

**A preset the studio did not create is never written to. The first editing gesture against a preset
forks it, and every later gesture in that session writes the fork.**

Concretely, three parts:

1. **Fork on first touch.** The first gesture that would change a preset — a slider release, a
   palette edit, a structure or map key, `Ctrl+S` — prompts once for a new name, writes the whole
   forked document to the watched directory under that name, and switches the editor to it. Every
   subsequent gesture against that fork writes the fork **silently**, at the cadence Plan 0159 Phase 6
   established. One prompt per preset per editing session, not one per gesture: the live loop is the
   reason the studio exists and consent is asked once, where it means something.

2. **The fork's source is the document the studio already holds, captured at gesture time.** Not
   re-read after the author answers the prompt. `useActivePreset` holds `text` for `path`; the fork is
   taken from that pair, so a preset change between the gesture and the answer cannot redirect the
   write. The path is still the player's own `preset.file` — ADR-0184's refusal of a second resolver
   stands and is not reopened here.

3. **The studio holds rotation for as long as it is attached.** It sends `ctl/transport hold` on
   attach and says so on the surface, with a control to resume. `hold` is already a spec-0003 verb and
   is a **position rather than a press**, so sending it is idempotent and repeating it cannot toggle
   rotation back on. This is a studio-side change: no event, no field, no widening of spec 0003.

An **embedded** preset takes the same path as any other. It has no file, so the fork is its first
file, and `status: 'embedded'` stops being a dead end — the case the current model cannot answer
becomes the ordinary one.

The fork is named, not generated silently: the author sees and can change the suggested name. What
the suggestion is, where the prompt lives, and how a fork is re-opened later are the implementing
lane's calls.

## Consequences

### Positive

- **A curated preset cannot be damaged by the studio.** The failure the smoke run measured is not
  made rarer or recoverable; it is made unreachable, because the destination of a write is always a
  file the author named.
- **Rotation stops deciding what gets edited.** With `hold` on attach, the preset under the editor
  changes only when the author changes it — and the gesture-time capture closes the residual race
  the prompt itself opens.
- **The embedded set becomes editable.** A user who likes a shipped preset can tune it without first
  knowing that a per-user directory exists, which is one of the things a tester will hit in Plan
  0167 Phase 7.
- **The live loop is intact.** After the fork, the disk still sees one write per gesture and the
  picture still follows the finger, for parameters *and* for palettes and structure.
- **Cheap to build.** No test asserts the behaviour being removed, no protocol field moves, and the
  TOML editor and its byte-equality tests are untouched.

### Negative

- **The prompt is one more thing between the author and the picture,** and it arrives at the moment
  they are least expecting a dialog — mid-drag-release. A badly placed prompt will read as the studio
  refusing to work. This is the cost, and the implementing lane owns making it small.
- **A VJ loses rotation when they open the studio.** That is a real change to a running show, which
  is why it is said on the surface and why `[Resume]` is there. It is also unavoidable: a preset that
  changes under the author cannot be edited at all.
- **The watched directory grows forks.** Backlog 0172 already records that the seeded directory is
  never pruned and can drift from the shipped set; this adds a second, faster source of drift. Not
  repaired here.
- **Editing an existing fork still needs a decision the studio does not yet make** — whether
  re-opening a preset the studio previously created counts as "a preset the studio did not create".
  The rule above says the fork is written silently *within a session*; across sessions the studio has
  no memory of what it authored. The plan resolves this; the ADR does not pretend it is free.

### Neutral

- Plan 0159 Phase 6's done-when sentence is superseded in its *destination* and kept in its
  *cadence*. The write is still once per gesture, still atomic, still followed by the player's own
  reload — only the file is different.

## Alternatives considered

### Alternative A — Keep the write-through model (the status quo)

Every gesture writes the preset the player named, with no prompt. This is what Plan 0159 Phase 6
decided and what shipped; it is the cheapest thing, it has no dialog, and the disk is always in sync
with the picture. **It lost on measured damage**: three presets altered in six minutes from gestures
the author did not experience as edits, no prompt, no undo, no record, and recovery only because the
shipped set happened to be a git checkout away. A model whose correctness depends on rotation being
off, while rotation is on by default, is not a model.

### Alternative B — Prompt on every write

The literal reading of *"prompt user to save it under another name"*. Safest possible: no write is
ever unconsented. **It lost on the loop**: a tuning session is twenty slider releases, and twenty
modal prompts is not an editing surface. It would make the studio's central capability — watching the
music move the picture as a value changes — unusable, to defend against a case the fork already
defends against completely.

### Alternative C — Explicit save only, nothing writes until `Ctrl+S`

The ordinary text-editor model, and it makes "never write without asking" true by construction.
**It lost on the palette case**, which is the case the owner actually named: only named parameters
ride `ctl/param`, so a palette recolour or a structure change that is not written to disk does not
reach the picture at all. The author would recolour a stop and see nothing until they saved, which
inverts the whole point of a live editor for precisely the edits that motivated this ADR.

### Alternative D — Pin the preset, keep writing through

Send `hold`, bind the edit to the captured path, and otherwise leave Plan 0159 Phase 6 alone. This
removes rotation's role entirely and is the smallest change that addresses the measured evidence.
**It lost because it fixes *which* file, not *whether* consent was given.** A slider release would
still silently and irreversibly rewrite a curated preset the author is merely auditioning, and it
leaves the embedded case with no answer at all. Pinning is necessary, which is why it is part 3 of
the Decision; it is not sufficient.

### Alternative E — Write through, but keep a session journal for undo

Keep the current model and make the damage recoverable: back every pre-edit document up, offer undo.
**It lost because recovery is not consent.** The author still finds their curated preset changed,
still has to notice it changed, and the smoke run is the proof that they do not — the session's own
account was of clicking a picker. It also adds durable state the studio has nowhere to keep, for a
worse outcome than the fork.

## Notes

- The evidence is Plan 0167's `## What the developer-machine smoke run found`, finding A, which
  carries the per-file table and the measurement that the repository's own `presets/` was never at
  risk — the damage was confined to the per-user seeded copy at `%APPDATA%\Ritmolux\presets`.
- Plan 0167's write-up attributes the reversed contract to **Plan 0159 Phase 3**. Phase 3 is the
  show-loop extraction; the write-on-release model and its byte-equality test are **Phase 6**. Cited
  correctly above.
- `hold` and `auto` being positions rather than presses is spec 0003's own invariant, written for the
  console strip's toggle. The studio is the second consumer to depend on it, and the first to depend
  on it for correctness rather than for ergonomics.
