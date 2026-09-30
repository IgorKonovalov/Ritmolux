# ADR-0256 — A parameter declares its group and whether it is main

> **Status:** accepted 2026-09-30 (Plan 0231's close)
> **Date:** 2026-09-30
> **Related plan(s):** [0231](../plans/done/0231-the-interface-is-audited-then-learns-one-look.md) (Phases 8 and 11)
> **Extends:** [ADR-0170](0170-a-parameters-reference-row-is-generated-from-the-declaration-the-engine-reads.md)
> (the declaration is the source), [ADR-0176](0176-the-player-is-driven-over-osc-control-in-and-reports-on-its-standard-streams.md)
> (the schema the studio reads)

## Context

Plan 0231's audit (2026-09-30) found the studio's parameter panel overwhelming. The panel lists every
parameter a system declares, flat, in declaration order: 20 to 40 rows, bound or not. The owner asked
for the main parameters to be visible, and for the rest to be tucked into collapsible groups.

The studio cannot tell a main parameter from a secondary one, because nothing declares it. It sees
what `ritmolux --schema` exports from each scene's `ParamSpec` declarations: name, default, range,
doc and kind. The owner chose to combine two things: the parameters **a preset binds** stay on top,
and the rest arrive in **groups the engine declares**, collapsed.

## Decision

We will add two fields to `ParamSpec` in `core/src/render/scenes/mod.rs`: `group`, one of `Shape`,
`Motion`, `Colour`, `Light` and `Post`, and `main`, a boolean. Every declaration states both. They
are required fields, so a declaration that omits either fails to compile. `ritmolux --schema` exports
both beside the existing fields, and `docs/specs/player-schema.json` records the widened shape. The
studio groups the parameters a preset does not bind by `group`, one collapsed accordion each, and
opens a group's `main` rows first when it expands. The engine-wide stages (`bg_*`, `trails`,
`kaleido_*`, `bloom_*`, `exposure`, the ink pass) declare `Post`, except `exposure` and the
background's brightness, which declare `Light`.

## Consequences

**Positive.**

- The grouping lives where every other fact about a parameter lives, so it cannot drift from the
  engine the way a map kept in the studio would.
- The generated parameter reference in `presets/README.md` can print the group, so the page authors
  read and the panel the studio draws agree.

**Negative.**

- Every scene's declarations change, which touches many files at once. The compiler finds every
  one, but judging `main` is a judgement per parameter, made once, by `dev`, and reviewed at the
  close.
- The schema the studio reads widens. The studio must accept a schema with the fields and reject
  one without, and `EXPECTED_PLAYER_VERSION` already pins the two together.
- `main` is one global flag per parameter, not per preset. A parameter that is secondary in general
  but central to one preset is shown by being bound, which the bound-first rule already covers.

## Alternatives considered

### Alternative A — the studio keeps its own grouping map

It lost because it is a second copy of the parameter roster, kept by hand in another language, and
it goes stale the first time a scene gains a parameter. That drift is what ADR-0170 exists to end.

### Alternative B — group by name prefix

`bloom_*`, `bg_*` and `kaleido_*` group themselves. It lost because most scene parameters carry no
prefix (`warp`, `zoom`, `hue`), so it groups the engine stages and leaves the scene's own parameters,
which are the overwhelming part, in one flat list.

### Alternative C — bound-first alone, with no groups

This needs no engine change. The owner declined it as the whole answer, because the unbound
remainder is still 20 or more rows in one list.
