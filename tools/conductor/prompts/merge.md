RLX-CONDUCTOR-MODE: merge
RLX-CONDUCTOR-PLAN: {{plan}}
RLX-CONDUCTOR-PLAN-FILE: {{plan_file}}
RLX-CONDUCTOR-WHERE: {{where}}
RLX-CONDUCTOR-MAIN: {{main_tip}}
RLX-CONDUCTOR-CONFLICTED: {{conflicted}}
RLX-CONDUCTOR-LANE: {{lane}} on branch {{branch}}
RLX-CONDUCTOR-SUITE-LOCK: node "{{with_lock}}" suite --

This session was started by the Ritmolux conductor (ADR-0205), not by a person. No one will read
this conversation or answer a question. Enter the `## Conductor mode` section of your skill and
follow it; where it and the rest of the skill disagree, conductor mode wins.

The conductor merged `main` into this lane, at the point named above, and the merge conflicted in the
paths listed above. It aborted that merge, so the tree is clean. Your whole task is the merge
(ADR-0248):

- Run `git merge --no-edit main` yourself, resolve every conflict, and commit the merge with
  `git commit --no-edit`. Keep both sides' intent: this plan's work and what reached `main` meanwhile.
  Where the two cannot both hold, keep `main`'s behaviour and adapt this plan's side to it.
- Change nothing beyond what resolving the conflict needs. No refactor, no plan edit, no new feature.
  The conductor's gate runs next, on the tree you leave, and a review reads it after.
- Run the checks the conflicted files need to be sure the resolution builds, `cargo nextest` only as
  `node "{{with_lock}}" suite -- cargo nextest ...`.
- **Never start a command in the background and never arm a `Monitor`.** Nothing re-invokes this
  session; a backgrounded command is killed with it.
- **Shell calls run one command per call**, because the allowlist reads each one on its own. No `cd`,
  git never takes `-C`, and `git grep` or the Grep tool reads text, never a pipe into `grep`.
- **Never attempt an `Edit` or a `Write` under `.claude/`.** The CLI refuses one to a headless session
  (ADR-0210). A conflict there is the owner's: park `merge_conflict` naming the file.
- Park rather than guess when a conflict needs a decision only the owner can make, or when the two
  sides contradict each other's design. Abort your merge first (`git merge --abort`) so the tree is
  clean.

The conductor checks your claim against `git`: the commit must be a merge whose second parent is
`main`, the tree must be clean, and no path listed above may still carry a conflict marker.

The last thing you print is exactly one fenced block tagged `rlx-outcome` holding one JSON object:

```rlx-outcome
{"kind": "merged", "plan": "{{plan}}", "commit": "<sha of the merge commit>"}
```

or

```rlx-outcome
{"kind": "parked", "plan": "{{plan}}", "reason": "merge_conflict | plan_wrong | question | check_red", "detail": "<one line: what, and the file to read>"}
```
