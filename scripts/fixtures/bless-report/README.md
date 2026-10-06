# scripts/fixtures/bless-report — for `bless-report.mjs`

```
node scripts/bless-report.mjs --self-test    # expects exit 0, 33 of 33
```

The script reads the two logs `bless.yml` writes on a Windows runner, which no tree here can
produce. These files stand in for them, and the self-test feeds them through the same `readCompare`,
`readBlessed`, `judge` and `render` the job's report and exit code are taken from.

| File | Stands in for | Asserted |
|------|---------------|----------|
| `compare.log` | the compare run: **real** `--no-capture` output of the golden and pinned tests, captured on llvmpipe rather than WARP with the bless job's test filter | all 53 comparison lines read, the `line_joints` probe lines and the skip notices not read, the `attractor_trails` and `warp_mesh_wide` suffixes tolerated; `waterfall` and `parametric_torus_knot` named and over tolerance, six unnamed baselines over tolerance and listed as still failing |
| `bless.log` | the bless run: written by hand in the shape the tests print, `blessed <path>` with a Windows path | the stem is the path's file name; a named stem with no `blessed` line fails the report |

llvmpipe drifts from the WARP baselines on the stateful and line fixtures, which is what gives this
log baselines over tolerance at all. Its line shapes are the ones every adapter prints.
