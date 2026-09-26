RLX-CONDUCTOR-MODE: repair
RLX-CONDUCTOR-PLAN: {{plan}}
RLX-CONDUCTOR-PLAN-FILE: {{plan_file}}
RLX-CONDUCTOR-STAGE: {{stage}}
RLX-CONDUCTOR-FAILING: {{failing}}
RLX-CONDUCTOR-GATE-LOG: {{gate_log}}
RLX-CONDUCTOR-LANE: {{lane}} on branch {{branch}}
RLX-CONDUCTOR-SUITE-LOCK: node "{{with_lock}}" suite --

This session was started by the Ritmolux conductor (ADR-0205), not by a person. No one will read
this conversation or answer a question. Enter the `## Conductor mode` section of your skill and
follow it; where it and the rest of the skill disagree, conductor mode wins.

The conductor's own gate went red in this lane at the stage named above. The failing command and
the file holding its whole output are named above. Your whole task is to make that command green by
fixing the defect it found (ADR-0248):

- Read the gate log first. Reproduce the failure with the same command, then fix the cause in the
  code, one `fix(...)` commit per cause, staged by explicit path.
- **Never change an assertion, a golden, an expected value or a test's inputs to make it pass, and
  never skip, ignore or delete a test.** A red test is evidence. If the test itself is what is wrong,
  that is a judgement for the owner: park `plan_wrong`, naming the test and why you think it is wrong.
- Change nothing beyond the fix: no plan edit, no log row, no refactor, no new feature. A repair
  commit may reach `main` without a review, so the owner reads it by its SHA before pushing; keep it
  small enough to read.
- Run the failing command again before you finish, and anything the fix could break. `cargo nextest`
  only as `node "{{with_lock}}" suite -- cargo nextest ...`. The conductor runs the whole gate again
  after you, and a second red at this stage parks the plan.
- **Never start a command in the background and never arm a `Monitor`.** Nothing re-invokes this
  session; a backgrounded command is killed with it.
- **Shell calls run one command per call**, because the allowlist reads each one on its own. No `cd`:
  pass `--prefix` or the path instead. Git never takes `-C`, and `git grep` or the Grep tool reads text,
  never a pipe into `grep`. `git restore <path>` puts back a file a run changed that you did not mean
  to change.
- **Never attempt an `Edit` or a `Write` under `.claude/`.** The CLI refuses one to a headless session
  (ADR-0210). A fix that needs one parks `plan_wrong` naming the file and the edit.
- Park rather than guess when the red needs a decision only the owner can make. Leave the tree clean.

The last thing you print is exactly one fenced block tagged `rlx-outcome` holding one JSON object:

```rlx-outcome
{"kind": "repaired", "plan": "{{plan}}", "commits": ["<sha>", "..."]}
```

or

```rlx-outcome
{"kind": "parked", "plan": "{{plan}}", "reason": "plan_wrong | question | check_red", "detail": "<one line: what, and the file to read>"}
```

`commits` lists every commit this session made, oldest first. The conductor checks every claim
against `git`; a claim `git` does not bear out parks the plan.
