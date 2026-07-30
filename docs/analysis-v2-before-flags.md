# Analysis v2 — the reachability record *before* the library retune

Plan 0048 Phase 5's deliverable, and Plan 0048 Phase 7's work list. This is the
state of the shipped preset library **after** the v2 semantics landed and
**before** any preset was retuned, measured rather than predicted.

Reproduce with:

```
cargo run --release -p standalone --example shot -- --report --presets presets
```

Measured **2026-07-30**, on the 12 s / 110 BPM `dynamic_groove` probe
(`REACH_SECS`, `REACH_BPM`), at the recalibrated realistic levels
`bass 0.661  mid 0.575  treb 0.281  onset 0.145`.

## The census

**26 flags: 23 `GATE` + 3 `COMP`.** They fall into two groups with completely
different meanings, and separating them is the point of this file.

### Group 1 — the standing `tempo` false positive (17 flags, *not* work)

15 `GATE` + 2 `COMP`, every one a comparison against `tempo` under a probe that
synthesizes a **single** BPM. A one-sided reading is the correct answer here, and
Plan 0042's close already identified this as an instrument limitation rather than
a content defect. Nothing for Phase 7 to fix.

| preset | flags |
|---|---|
| Storm | `field_freq`, `force`, `hue_spread`, `kaleido_order`, `palette_mix`, `spin`, `trails`, `zoom` (all `tempo > 132`) + 1 `COMP` |
| Lorenz | `fade`, `hue_spread`, `kaleido_order`, `palette_mix`, `size`, `zoom` (all `tempo > 124`) + 1 `COMP` |
| Rose Zoom | `zoom` (`tempo > 130`) |

### Group 2 — band thresholds v2 broke (9 flags, **Phase 7's work list**)

8 `GATE` + 1 `COMP`, every one a band comparison that **never goes false** any
more. These are the predicted consequence of ADR-0049: each threshold was
calibrated against *raw* levels of 0.006–0.040, and the normalized values that
replaced them sit at 0.28–0.66, so the comparison is now permanently true and the
`else` branch is dead.

| preset | binding | threshold | reads |
|---|---|---|---|
| Glacier | `kaleido_order` | `bass + mid > 0.035` | never false |
| Kaleido Field | `kaleido_order` | `bass + mid + treb > 0.075` | never false |
| Reef | `kaleido_order` | `bass + mid > 0.048` | never false |
| Storm | `kaleido_order` | `bass + mid > 0.055` (`COMP`) | never false |
| Rose Web | `mirror_order` | `mid > 0.011` | never false |
| Star Rosette | `mirror_order` | `mid > 0.011` | never false |
| Arrowhead | `mirror_order` | `mid > 0.012` | never false |
| Arrowhead | `visible_depth` | `mid + treb > 0.03` | never false |
| Fern Grow | `visible_depth` | `bass + mid > 0.05` | never false |

Every one is a **rescale, not a redesign**: the mechanism is intact and only the
constant is on the wrong scale. As a starting point, a threshold meant to sit
"somewhat above typical" now belongs near the 0.28–0.66 typical band rather than
near 0.01–0.08.

### Ceilings

Two families report one unapproached clamp ceiling each — `Drift.burst` at 91 %
and `Lorenz.size` at 89 %. Both are under the 100 % bar by a margin that Plan
0041's review already logged as a threshold question about the *check*, not about
these presets.

## What this file is not

It is not a pass/fail gate. `docs/capturing.md` explains why reachability is
advisory: a flag is a suspect, not a conviction. Group 1 above is the standing
proof of that — 17 of 26 flags are the instrument's single-BPM probe, correctly
reporting a one-sided comparison.

## Note for whoever runs Phase 7

The four preset gates (`reactivity`, `sanity`, `animation`, `distinctness`) and
the golden suite **cannot see any of this**. They synthesize `AnalysisFrame`
values directly and never run PCM through the analyzer, so they were green
throughout the v2 semantic change and will stay green whether or not the retune
happens. `--report` over the real analyzer, i.e. this file, is the only
instrument that can tell. Delete this file once its Group 2 list is empty.
