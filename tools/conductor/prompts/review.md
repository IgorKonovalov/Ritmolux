RLX-CONDUCTOR-MODE: review
RLX-CONDUCTOR-PLAN: {{plan}}
RLX-CONDUCTOR-PLAN-FILE: {{plan_file}}
RLX-CONDUCTOR-ROUND: {{round}}
RLX-CONDUCTOR-REVIEW-PATH: {{review_path}}
RLX-CONDUCTOR-LANE: {{lane}} on branch {{branch}}
RLX-CONDUCTOR-SUITE-LOCK: node "{{with_lock}}" suite --

This session was started by the Ritmolux conductor (ADR-0205), not by a person. No one will read
this conversation or answer a question. Enter the `## Conductor mode` section of your skill and
follow it; where it and the rest of the skill disagree, conductor mode wins.

You are the fresh-session close review. You were given the plan and the lane, and nothing an
implementer wrote except what is in the repository. Earlier rounds of this review, if any:

{{prior_rounds}}

1. Run Mode 4 against the plan and the lane. Write the full review to the review path above.
2. If it carries any `blocker` or `major`: stop. Print the `verdict` outcome and nothing after it.
3. Only if it carries none: continue into the worktree close sequence, steps 1-3, on the branch in
   this worktree — merge `main`, the whole gate (`nextest` under the suite lock), the bookkeeping,
   the version bump, the studio sync, an ANNOTATED tag on the branch tip. The close commit adds a
   `## Close review` section to the plan, after `## Implementation log`: this round's review in full,
   then one line for every finding an earlier round raised and a fix round resolved, naming the fix
   commit. Never fast-forward `main`, never remove the worktree, never push: the conductor does the
   first two and the owner the third.
4. If the merge conflicts or the gate goes red during the close, park; do not work around it.

The last thing you print is exactly one fenced block tagged `rlx-outcome` holding one JSON object.
`findings` lists every finding of this round, in order, whatever its severity.

```rlx-outcome
{"kind": "verdict", "plan": "{{plan}}", "round": {{round}}, "blockers": 0, "majors": 1, "minors": 2, "review_path": "{{review_path}}", "findings": [{"severity": "major", "file": "core/src/x.rs", "line": 88, "what": "<one line>"}]}
```

or, after a clean close:

```rlx-outcome
{"kind": "closed", "plan": "{{plan}}", "version": "<X.Y.Z or null>", "tag": "<vX.Y.Z or null>", "verdict": {"round": {{round}}, "blockers": 0, "majors": 0, "minors": 1, "review_path": "{{review_path}}", "findings": [{"severity": "minor", "file": "docs/x.md", "line": 12, "what": "<one line>"}]}}
```

or

```rlx-outcome
{"kind": "parked", "plan": "{{plan}}", "reason": "merge_conflict | check_red | plan_wrong", "detail": "<one line>"}
```
