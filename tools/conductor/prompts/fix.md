RLX-CONDUCTOR-MODE: fix
RLX-CONDUCTOR-PLAN: {{plan}}
RLX-CONDUCTOR-PLAN-FILE: {{plan_file}}
RLX-CONDUCTOR-ROUND: {{round}}
RLX-CONDUCTOR-REVIEW: {{review_path}}
RLX-CONDUCTOR-LANE: {{lane}} on branch {{branch}}
RLX-CONDUCTOR-SUITE-LOCK: node "{{with_lock}}" suite --

This session was started by the Ritmolux conductor (ADR-0205), not by a person. No one will read
this conversation or answer a question. Enter the `## Conductor mode` section of your skill and
follow it; where it and the rest of the skill disagree, conductor mode wins.

A fresh architect review of this plan returned blockers or majors. The review is the file named
above. Its findings, numbered from 0 in the order the reviewer's verdict listed them:

{{findings}}

- Fix every `blocker` and `major`. A `minor` or `nit` is yours to leave.
- One `fix(...)` commit per finding or per tightly-coupled group, staged by explicit path. Add one
  line per fix to the plan's `### Notes` naming the finding and the commit; touch nothing else in
  the plan.
- Run the gate the plan's phases name, `cargo nextest` only as
  `node "{{with_lock}}" suite -- cargo nextest ...`.
- A finding you judge wrong is not yours to overrule and not yours to work around: park with
  reason `plan_wrong` naming it.
- **Shell calls run one command per call**, because the allowlist reads each one on its own. No `cd`:
  run the tool from the lane root and give the path — `npm --prefix studio run typecheck`, not
  `cd studio; npm run typecheck`. Git runs in the lane this session was started in and never takes
  `-C`. The only environment prefixes are the exact forms the allowlist names,
  `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`, and `RLX_UPDATE_PRESET_SCHEMA=1` or
  `RLX_UPDATE_PARAM_REFERENCE=1` ahead of `cargo` or `node`; `env`, `export` and `$env:` are refused
  (`$env:X = '1'; ...` is a second command). Read text with the Read and Grep tools or `git grep`,
  never `awk`, `sed` or a pipe into `grep`: a done-when written as a pipe runs as its parts, or as
  the equivalent Grep call, and the fix's `### Notes` line says which. `git clean` and
  `git checkout` name their path after `--`; `git restore <path>` puts a file back.

The last thing you print is exactly one fenced block tagged `rlx-outcome` holding one JSON object:

```rlx-outcome
{"kind": "fixed", "plan": "{{plan}}", "round": {{round}}, "commits": ["<sha>", "..."], "resolved": [{"finding": 0, "commit": "<sha>"}]}
```

or

```rlx-outcome
{"kind": "parked", "plan": "{{plan}}", "reason": "plan_wrong | question | check_red", "detail": "<one line>"}
```
