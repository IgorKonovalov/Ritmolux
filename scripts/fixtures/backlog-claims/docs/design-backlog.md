# Design backlog — fixture

Not a real backlog. This file mirrors the shape of `docs/design-backlog.md` closely enough for
`scripts/check-backlog-claims.mjs` to parse it, and seeds one instance of each case the checker
must handle. See `scripts/fixtures/README.md` for the expected output.

Deliberately link-free: this tree is also walked by the doc-link checker, whose seeded breaks live
in `doc-links/` and must stay the only ones.

## Closed entries — the ledger

Everything above the `Open entries` heading is history, and the checker must not probe it. This
row exists to prove that: its probe would go red if it were read.

## 0099 — an archived entry the checker must ignore

- **Verified 2026-08-15** — this probe is deliberately violated and must never be
  reported, because it sits above the live marker:
  `absent: SEEDED_PRESENT_SYMBOL in: core/src`

## Open entries

## 0001 — a violated absent claim

The entry says the symbol is gone. It is not.

- **Verified 2026-08-15** — the symbol is nowhere in the engine:
  `absent: SEEDED_PRESENT_SYMBOL in: core/src`

## 0002 — a violated present claim

The entry cites a rule that has since moved or was never written.

- **Verified 2026-08-15** — the rule is documented, and here:
  `present: SEEDED_MISSING_RULE in: presets/README.md`

## 0003 — a malformed probe

An unclosed regex group. The checker must name the entry rather than crash, and must not
silently skip it — a probe that cannot run is not a probe that passed.

- **Verified 2026-08-15** — `absent: (unclosed in: core/src`

## 0004 — a verification bullet with no probe and no opt-out

A dated stamp with nothing machine-runnable in it is the pre-ADR-0108 convention, and it is
exactly what this gate exists to stop accepting.

- **Verified 2026-08-15** — checked it by eye and it looked fine.

## 0005 — a valid opt-out

- **Verified 2026-08-15** — `unprobeable: this is a judgement about rendered output,
  not a claim about repo contents`

## 0006 — probes that still hold

Present so that "exactly four breaks" means the checker discriminates, rather than that it
reports everything it sees.

- **Verified 2026-08-15** — the symbol was never introduced:
  `absent: SEEDED_ABSENT_SYMBOL in: core/src`
- **Verified 2026-08-15** — the rule is documented, and here:
  `present: SEEDED_PRESENT_RULE in: presets/README.md`

## 0007 — a live entry with no verification bullet at all

The pre-ADR-0108 default: an entry that makes a claim about the repo and never says anyone
checked it. Case 0004 is a bullet with nothing runnable inside it; this one has no bullet, which
a checker built out of the bullets it finds cannot see at all. Last on purpose — its absence runs
to the end of the file, which is the one position a heading-driven check could get wrong.
