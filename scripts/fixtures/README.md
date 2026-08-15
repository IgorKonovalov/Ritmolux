# scripts/fixtures — the trees the doc checkers bite on

Two checkers in `scripts/` take an optional `root` argument so they can be run against a tree
other than this repository. This directory is that tree. Every file under it is **deliberately
wrong in a named way**, so that "the checker still catches things" is a command anyone can run
rather than a property nobody has re-tested since the day it was written.

`check-doc-links.mjs` skips this tree **by path** on an ordinary repo walk — `scripts/fixtures`,
enumerated once in the script — and scans it when it **is** the root, which is the only way the
seeded breaks below are reachable. Without that skip, this directory would red the link gate on
every push. The skip matched the directory *name* until Plan 0094 Phase 1, which meant it also
swallowed `core/tests/fixtures/` and its twelve relative links; a second seeded tree has to add
its own path to that list rather than inherit the skip from what it is called.

The seeded cases are kept **per checker, in per-checker subdirectories**, because a shared
fixture that drifts into serving one of them stops being an instrument for the other and says
nothing about it.

## `backlog-claims/` — for `check-backlog-claims.mjs`

```
node scripts/check-backlog-claims.mjs scripts/fixtures/backlog-claims
```

Expect **exit 1 and exactly four breaks**. Six entries, five seeded cases:

| Entry | Case | Expected |
|-------|------|----------|
| 0001 | a violated `absent:` probe | reported, naming the contradicting `file:line` |
| 0002 | a violated `present:` probe | reported, naming the path searched |
| 0003 | a malformed probe (unclosed regex group) | reported as malformed, **never a crash and never a silent skip** |
| 0004 | a verification bullet with no probe and no opt-out | reported |
| 0005 | a valid `unprobeable:` opt-out | **not** reported; rostered in the summary |
| 0006 | two probes that still hold | not reported |

The same six run inside `node scripts/check-backlog-claims.mjs --self-test`, together with the
non-vacuity assertion that is pinned to the real repository rather than to this tree.

## `doc-links/` — for `check-doc-links.mjs`

```
node scripts/check-doc-links.mjs scripts/fixtures
```

Expect **exit 1 and exactly three breaks**, one per class the checker knows about. Class 1 was
the only one it had until Plan 0084, and checking one of markdown's two link forms was a green
light over 85 broken links of the other:

| File | Class | Seeded as |
|------|-------|-----------|
| `doc-links/broken.md` | 1 — inline | a target that does not exist |
| `doc-links/broken.md` | 2 — definition | a definition whose target does not exist |
| `doc-links/broken.md` | 3 — use with no definition in this file | a label `doc-links/defines.md` defines and this file does not |

Class 3 is scoped per file because that is markdown's own scope, and it is what a close ceremony
breaks when it moves link-dense prose between documents: the *uses* travel with the paragraph and
the *definitions* stay behind.

Note the root: this checker is pointed at `scripts/fixtures`, not at `scripts/fixtures/doc-links`,
so the run also asserts that the backlog fixture's own markdown is link-clean. Keep it that way —
a broken link seeded outside `doc-links/` would make the count above wrong for the wrong reason.
