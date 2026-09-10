# ADR-0184 — The player reports what it loaded, and the studio re-derives nothing

> **Status:** proposed
> **Date:** 2026-09-10
> **Related plan(s):** [0159](../plans/0159-the-studio-opens.md) Phase 5

## Context

Plan 0159's parameter panel needs three facts the event stream does not carry, and the lane
stopped at that boundary rather than build panels against them:

| Fact | Why the panel needs it | What the roster gives today |
|------|------------------------|-----------------------------|
| The active preset's **system**, as the schema keys it | `--schema` is a document of `systems[].params`; without the key there is no roster of `ParamSpec` rows to render | `preset` carries `name` and `index` |
| The active preset's **file** | the drag's release writes the edited constant back into the file the player is watching | nothing carries one, except `preset_error` and `preset_warning`, which carry one only when a file *failed* |
| The **directory** the player watches | "save as" and a new preset from a template have to land where the watcher will see them | nothing; the player prints it as prose — `loaded N preset(s) from ...` |

Each is already in the player's hand at a site that emits an event. `reload_presets`
(`standalone/src/preset_dir.rs`) holds the directory and the paths it just loaded and emits
`roster` from there. `Show::report_active_preset` (`standalone/src/show.rs`) holds the renderer
and emits `preset` from there, and the renderer can already answer the system question —
`Renderer::active_system_name`.

Two details make this a decision rather than a transcription.

**The system has two names, and they are not the same string.** The schema keys its rosters by
`SystemKind::as_str()` — `"fragment_field"`, `"shape_collage"`, `"reaction_diffusion"`. The
accessor that exists, `Renderer::active_system_name`, returns the `Scene::name()` display string —
`"fragment field"`, `"star pattern"`, `"l-system"`. The two agree exactly on the four single-word
systems (`swarm`, `spectrum`, `emitter`, `attractor`) and on nothing else, so a panel built on the
display name resolves for a quarter of the roster and silently renders empty for the rest — and a
test written against a `swarm` or `attractor` preset passes while doing it. This is the shape
[ADR-0071](0071-a-numeric-test-contract-states-a-property-or-names-its-machine.md) and
[ADR-0037](0037-internal-grid-is-a-resolution-not-a-shape.md) both come from: two sources that
coincide on the configuration under test.

**A preset does not always have a file.** A run whose directory is unresolved, empty, or entirely
failing keeps the embedded set — `PresetDir::Unresolved` is honoured rather than treated as a
failure — and an embedded preset has no path on disk. `rlx_core::preset::Preset` carries `name` and
`system` and drops the path `load_dir` read it from. So "the active preset's file" is a fact that
genuinely may not exist, and the studio has to be able to tell "not editable, offer save-as" from
"editing failed".

Separately, the same event stream is the only honest place for a reading Plan 0159 already found
to be false. ADR-0178 rests the preview's cost on a dropped-frame count, and the studio's count
read **0** while roughly nine frames in ten were being lost: `FramePump` counts a drop only when a
frame arrives while one is in flight, so loss upstream of it — in the OS pipe, or in main's read
cadence — is invisible to it. The player already counts the truth. `PreviewPipe` in
`standalone/src/stream.rs` holds `sent` and `dropped` and writes both to `diagnostics.log`, a file
no parent reads.

## Decision

**Every fact the studio needs about what the player loaded travels on the event stream, from the
site that already holds it, and the studio re-derives none of it.** Four additive fields under the
same `v`, per [ADR-0176](0176-the-player-is-driven-over-osc-control-in-and-reports-on-its-standard-streams.md)'s
additive rule:

| Event | New field | Value | `null` when |
|-------|-----------|-------|-------------|
| `preset` | `system` | the schema's own key, `SystemKind::as_str()` | never |
| `preset` | `file` | the absolute path the active preset was loaded from | it came from the embedded set |
| `roster` | `dir` | the directory this reload read and the watcher polls | nothing resolved (`PresetDir::Unresolved`) |
| `health` | `preview_sent`, `preview_dropped` | `PreviewPipe`'s running totals | no preview pipe is open |

`roster` carries the directory rather than `hello`, because `hello` is emitted by `greet()` in
`run.rs` **before** `Show::start` resolves the directory, and `hello` must remain the first event.
Putting the directory on the catalog event needs no reordering, and it arrives in the same line as
the names it locates.

`system` is the schema's key and not the display name. The core gains the path it already reads —
`Preset` carries a `source: Option<PathBuf>`, set by `load_dir`, `None` for the embedded set — and
the renderer gains the two accessors the emission site needs.

**The gate moves with the fields.** `studio/shared/protocol.spec.test.ts` today diffs the spec's
`ev` column against the union's member names and does not look at the `Fields` column at all, so
every field above could be added on one side alone and nothing would fail. It grows to diff the
field lists both ways, which is what makes "additive under the same `v`" a checkable claim rather
than a convention.

## Consequences

### Positive

- The panel resolves its `ParamSpec` rows from a key the engine itself produced, so a system
  renamed or added in `core` reaches the studio without a second edit.
- A save lands in the directory the player is watching **because the player named it**, not
  because two resolvers agree today.
- "This preset has no file" becomes a fact the studio is told rather than one it infers from a
  failed write, which is what lets the embedded-set case degrade into save-as instead of an error.
- The preview's cost is reported by the process that pays it. A footer can show drawn, delivered
  and painted, and the gap between the last two stops being invisible.
- Every one of the four fields is read at a site that already had the value, so no new plumbing
  crosses a layer.

### Negative

- **`core` learns where a preset came from.** `Preset` grows a `source: Option<PathBuf>`, and
  `LoadReport` now hands back presets that remember their file. It is a filesystem path in a crate
  that already takes `load_dir(&Path)`, so it crosses no line this project draws — but it is one
  more field the embedded path has to leave `None`, and a future in-memory preset source has to
  answer the same question.
- **Three of the four fields are nullable, and a studio that ignores that renders a wrong thing
  rather than nothing.** `file: null` means not editable; `dir: null` means nowhere to save; a
  studio that treats either as an empty string writes into the process's working directory.
- **`health` grows a second subject.** It reported the show's own frame timing and the control
  path's refusals; it now also reports a sink's delivery. That is a coherent line — everything on
  it is per-second, from the drawn frame — but the event is no longer about one thing.
- The spec-diff test gets stricter, so every future field on either side is a two-file edit. That
  is the point, and it is a cost.

### Neutral

- The event stream stays at `v: 1`. Nothing outside this repository reads it.

## Alternatives considered

### Alternative A — The studio resolves the directory and reads the system out of the file

The studio applies `RLX_PRESET_DIR` else the OS data root plus `Ritmolux/presets`, and parses the
`system` key out of the TOML it is about to edit. It needs no player change at all, and the studio
already parses TOML for the round-tripping writer.

Rejected because it is a second copy of a resolution rule, which is the thing
[ADR-0183](0183-the-studio-drives-one-player-and-the-show-loop-is-extracted.md) was written about
one level up. Two copies that agree today diverge on the case neither author tested — the override,
the unresolved directory, the first-run seed — and the failure is a studio that edits files the
player is not watching, with both processes reporting success. The system half is worse than the
directory half: `system` may be omitted in the file, in which case the loader's default applies,
so the studio would have to reimplement the loader's defaulting to read a fact the loader already
computed.

### Alternative B — Make `roster` a catalog of `{name, file, system}` objects and move to `v: 2`

One event carries everything about every preset, `preset` becomes a pointer into it, and the
library view in Plan 0159 Phase 7 gets per-entry files and systems for free.

Rejected as speculative generality bought with a version break. Phase 7's library view is specified
in terms of names and a `ctl/preset` click, and nothing in this plan needs a file for a preset that
is not on screen. A `v` bump costs the spec's whole roster table and every Zod literal for a field
no phase has asked for; when a phase does ask, `roster` can grow the array additively then, with
the argument written down.

### Alternative C — A query address on the control channel

The studio sends `ctl/query` and the player answers on the event stream.

Rejected because it introduces request/response to a contract that deliberately has none. Spec 0003
is explicit: a refusal is counted, never answered individually, because OSC has no reply channel,
and `ctl/ping` is the single exception that exists to tell a dead player from a quiet one. A second
exception makes it a pattern, and the facts in question are not query-shaped — they change exactly
when the player reloads or switches, which is exactly when it already emits an event.

### Alternative D — A sequence number in the frame stream, so the studio counts its own loss

Each preview frame carries a header the studio counts gaps in.

Rejected because the pipe's payload is raw `rgba8` with no framing by design — `--stream --sink
stdout` is piped straight into `ffmpeg`, and a header would break that consumer to instrument a
different one. The producer already counts what is lost; the fix is to report the count, not to
change the wire.

## Notes

The readings that motivated the last row, both from Plan 0159's Phase 4 log, both on the reference
machine with `--preset Clifford` at 1920x1080 and neither a property: the pipe carries 8.29 MB per
frame and yielded about 18 frames/s to a fast reader while the show drew about 42; the studio
painted 59 frames in about 15 s and reported `0 dropped`. What Phase 10 takes properly is the
comparison; what this ADR fixes is that the instrument was reporting a number about the wrong
thing.
