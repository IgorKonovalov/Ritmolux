RLX-CONDUCTOR-MODE: implement
RLX-CONDUCTOR-PLAN: {{plan}}
RLX-CONDUCTOR-PLAN-FILE: {{plan_file}}
RLX-CONDUCTOR-PHASES: {{phases}}
RLX-CONDUCTOR-LAST-RUN: {{last_run}}
RLX-CONDUCTOR-LANE: {{lane}} on branch {{branch}}
RLX-CONDUCTOR-SUITE-LOCK: node "{{with_lock}}" suite --

This session was started by the Ritmolux conductor (ADR-0205), not by a person. No one will read
this conversation or answer a question. Enter the `## Conductor mode` section of your skill and
follow it; where it and the rest of the skill disagree, conductor mode wins.

- The phases listed above are your "go". Implement exactly those, in order, one commit per phase,
  each with its `## Implementation log` row, and nothing outside them.
- Do not restate the plan, do not wait, do not ask. Do not invoke any other skill through the Skill
  tool: the conductor starts the next run itself.
- Run every `cargo nextest` / `cargo test` as `node "{{with_lock}}" suite -- cargo nextest ...`.
- **Never start a command in the background and never arm a `Monitor`.** Nothing re-invokes this
  session: backgrounding a command and ending your turn kills that command and loses its result,
  after the commits you already made have landed. A long command runs in the foreground, and this
  session's own timeout is what bounds it. A hook denies `run_in_background`, the settings deny
  `Monitor`, and a background command left unfinished at the end parks the plan whatever you claim.
- **Shell calls run one command per call**, because the allowlist reads each one on its own. No `cd`:
  run the tool from the lane root and give the path — `npm --prefix studio run typecheck`, not
  `cd studio; npm run typecheck`. Git runs in the lane this session was started in and never takes
  `-C`. The only environment prefixes are the exact forms the allowlist names,
  `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`, and `RLX_UPDATE_PRESET_SCHEMA=1` or
  `RLX_UPDATE_PARAM_REFERENCE=1` ahead of `cargo` or `node`; `env`, `export` and `$env:` are refused
  (`$env:X = '1'; ...` is a second command). Read text with the Read and Grep tools or `git grep`,
  never `awk`, `sed` or a pipe into `grep`: a done-when written as a pipe runs as its parts, or as
  the equivalent Grep call, and the log row's note says which. `git clean` and `git checkout` name
  their path after `--`. Making and removing a scratch file or directory inside the lane is allowed; a path
  that leaves the lane is refused, whatever it is for.
- If the last-run line says `yes`, finish with the close block of the `## Implementation log`,
  committed, and print the outcome instead of the pointer. Do not run the full workspace suite: the
  conductor's `pre-review` gate runs it next on the same tree, so the close block's `Full suite:`
  bullet reads *owed to the conductor's pre-review gate (ADR-0207)*.
- **Never attempt an `Edit` or a `Write` under `.claude/`.** The CLI refuses one to a headless session
  whatever the allowlist says, measured on 2.1.273 (ADR-0210): a read is allowed, a write is not, and
  no spelling of a permission rule reaches it. The conductor parks a phase whose files include such a
  path before the phase starts, so a range you were handed does not contain one. If you find you need
  one anyway, park `plan_wrong` naming the exact file and the edit — do not work around it by writing
  the file some other way.
- Stop and park, rather than work around it, on: a `human` phase inside the range, a stop condition
  the plan states, a plan that is wrong, a question only a person can answer, or a check you cannot
  make green within the phase. Commit what is finished first; leave the tree clean.

The last thing you print is exactly one fenced block tagged `rlx-outcome` holding one JSON object:

```rlx-outcome
{"kind": "phases_done", "plan": "{{plan}}", "through": "<last phase id done>", "commits": ["<sha>", "..."]}
```

or

```rlx-outcome
{"kind": "parked", "plan": "{{plan}}", "phase": "<phase id>", "reason": "human_phase | stop_condition | plan_wrong | question | check_red", "detail": "<one line: what, and the file to read>"}
```

`commits` lists every commit this session made, oldest first, as short or full SHAs. The conductor
checks every claim against `git`; a claim `git` does not bear out parks the plan.
