# Preset authoring — the seeded roster

Four bare citations live in this file, one per form the gate must match, so a
dropped branch of the alternation shows up as a count that moved rather than as
a silence nobody noticed.

Presets are TOML files (ADR-0002). A preset names a built-in system and binds
that system's named parameters to expression strings over the audio analysis.

Measured at Plan 0063 Phase 5, `depth_fade` above `0.8` flattens the figure
because the far end of the attractor stops separating from the near end.

The smart-dash form reads identically and must match too: ADR‑0127 governs code
comments, and this sentence cites it with a U+2011 non-breaking hyphen.

### Ink on paper — `ink_amount`, `paper_*` (Plans 0027, 0078)

The plural is the form that reached a built route name unnoticed, so it is
seeded here as a heading rather than as prose.

## The silences — every one of these is correct and must NOT be reported

An inline link is the shape the rule asks for:
[ADR-0021](../../../../docs/adrs/0021-shared-palette-system.md) is the palette
decision, and [Plan 0020](../../../../docs/plans/done/0020-shared-palette-system.md)
is the plan that carried it.

A full reference link is the same thing spelled differently: see [the join
rule][ADR-0041].

A collapsed reference is the form the real roster uses, and the citation *is*
the link text: [ADR-0098].

> A definition inside a blockquote is why the gate's definition pattern allows a
> `>` prefix — the roster's one collapsed reference is written exactly this way,
> and a definition the scanner cannot see makes every use of that label look
> bare.
>
> [ADR-0098]: ../../../../docs/adrs/0098-the-line-renderer-draws-arcs-as-per-pixel-distance-fields.md

[ADR-0041]: ../../../../docs/adrs/0041-line-joins-are-per-endpoint-on-the-segment-instance.md

A fenced block is a command, not a claim:

```sh
# Plan 0155 and ADR-0168 are a path and a filename here, not a citation.
cat docs/plans/0155-the-reader-documents-stop-explaining-themselves.md
grep -n "ADR-0168" docs/adrs/0168-*.md
```

A backlog reference is a different corpus with its own form rule, and this gate
says nothing about it: design-backlog 0062 stays exactly as written.
