# scripts/fixtures/npm-audit — for `check-npm-audit.mjs`

```
node scripts/check-npm-audit.mjs --self-test    # expects exit 0, 26 of 26
```

The gate's live run reads `npm audit --json` from the registry, so no tree can be seeded for it.
These files stand in for what npm printed, and the self-test feeds them through the same `readAudit`
and `judge` the exit code is taken from. The GHSA ids are made up and shaped like real ones.

| File | Stands in for | Asserted |
|------|---------------|----------|
| `clean.json` | a graph with no advisory | zero advisories, and not a failure |
| `shipped-high.json` | a `high` in `electron`, plus one package above it whose `via` is a bare name | one advisory, counted at its source; **fails** as the shipped graph, and sits under the line as a full graph |
| `dev-critical.json` | a `critical` in `vitest`, a dev-only package | **fails** through the full graph while the shipped graph is clean |
| `moderate.json` | a `moderate` | **passes**, printed under the line |
| `request-failed.json` | npm's `error` object when the advisory endpoint is unreachable | **fails**, naming the graph, and withholds the staleness report |
| `allow-reasoned.json` | an entry excusing `shipped-high.json`'s id with a reason | **passes**, reported as excused |
| `allow-no-reason.json` | the same entry with a blank reason | **fails**, naming the entry |
| `allow-stale.json` | an entry whose id no graph reports | **reported** for deletion, and passes |

Output that is not JSON, empty output, npm that did not start, and JSON that is not an audit report
are asserted inline, as is an allow file that is not JSON and an id that is not a GHSA id.
