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

The last thing you print is exactly one fenced block tagged `rlx-outcome` holding one JSON object:

```rlx-outcome
{"kind": "fixed", "plan": "{{plan}}", "round": {{round}}, "commits": ["<sha>", "..."], "resolved": [{"finding": 0, "commit": "<sha>"}]}
```

or

```rlx-outcome
{"kind": "parked", "plan": "{{plan}}", "reason": "plan_wrong | question | check_red", "detail": "<one line>"}
```
