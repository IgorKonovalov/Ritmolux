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
- If the last-run line says `yes`, finish with the close block of the `## Implementation log`
  (full suite under the lock first), committed, and print the outcome instead of the pointer.
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
