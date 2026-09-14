# ADR-0192 — A preset warning names the parameter it is about

> **Status:** accepted 2026-09-14 (Plan 0172), with an Outcome
> **Date:** 2026-09-11
> **Related plan(s):** [0172](../plans/done/0172-the-studios-readings-become-true.md)
> **Extends:** [ADR-0176](0176-the-player-is-driven-over-osc-control-in-and-reports-on-its-standard-streams.md)
> — one field on one event of [spec 0003](../specs/0003-studio-control-protocol.md).

## Context

Spec 0003 gives `preset_error` five fields — `file`, `message`, `line`, `col`, `param` — and
`preset_warning` two: `file` and `message`. The studio places an editor marker only where an event
put one, because it runs no parser of its own, so a warning is invisible in the file tab by
construction. The author learns that a preset loaded with a non-fatal problem, and not where.

Warnings are not rare. Changing a preset's system keeps the outgoing system's bindings and tables,
and each one the incoming system does not declare is its own warning; one click produces several.
That is backlog 0202.

The position is lost inside the core, not on the wire. `Preset::warnings` is a `Vec<String>`, built
by about sixteen `warnings.push(format!(..))` sites in `core/src/preset/schema/load.rs`. Roughly half
concern a named binding, and have the name in hand when they format the message. The error beside it
already solved the same shape: an expression error is raised after the document became values that
no longer hold a position, so it carries `param`, and the studio finds the line through the
`[params]` table.

## Decision

We will add an optional `param` field to `preset_warning`: the name of the binding the warning is
about, or `null` when it is about no single binding. The core carries each warning as a message plus
that optional name, and every push site that concerns a named binding sets it. The studio anchors a
warning through `param` by the same route it already uses for an expression error. Spec 0003 says
adding a field is additive under the same `v`, so no protocol version moves.

## Consequences

### Positive
- The commonest warning, a binding the system does not declare, becomes a marker on its own line.
- No new mechanism on either side: the anchor route exists and is tested for errors.
- `ritmolux --check` (Plan 0169) gets a structured warning to print, instead of a string.

### Negative
- **A warning about no single binding stays unanchored** — a structural table key, a rule across two
  parameters. Those keep `param = null` and still appear only in the list.
- **Each push site is classified by hand.** A site that should set `param` and does not is silent;
  only the tests Plan 0172 writes, one per class, notice.
- The core's warning type changes shape, which ripples through `LoadReport`, `core/tests/preset.rs`
  and every caller that prints a warning.

## Alternatives considered

### Alternative A — A true `line`/`col` span on every warning
Exact for every warning, including the ones with no parameter. It lost because it needs the warning
raised where the document still holds positions, and the loader drops them after parsing. Plan 0169
builds spans for its own checker through `toml::de::DeTable`; a span can be added later on top of
that without undoing this field.

### Alternative B — The studio parses the message text
It lost on ADR-0176's premise: the studio runs no parser of its own, and a message is prose written
for a person, which any rewording would break.

### Alternative C — Leave it, since the modal lists every warning
The modal makes a warning readable. It cannot make it locatable, and the author's next question is
which line.

## Outcome (2026-09-14, Plan 0172)

The Decision landed as written, with two refinements the text above does not state:

- **`param` is a label, not a bare name.** It is spelled as `preset_error`'s `param` is: `glow`,
  `[layer] glow`, `[per_vertex] x`, `[layer] [per_vertex] x`. That spelling is what lets
  `ritmolux --check` place a warning through the same key lookup it uses for an error.
- **On the wire the field is always present**, a string or `null`, never omitted. "Optional" above
  means nullable.

The first Positive holds for a **top-level** binding only. The studio's `markersFor` matches a bare
`[params]` name, so a `[layer]` or `[per_vertex]` label stays in the problems list, as an expression
error with that label already did. `--check` places all four label shapes.
