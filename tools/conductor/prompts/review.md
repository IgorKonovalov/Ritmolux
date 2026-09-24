RLX-CONDUCTOR-MODE: review
RLX-CONDUCTOR-PLAN: {{plan}}
RLX-CONDUCTOR-PLAN-FILE: {{plan_file}}
RLX-CONDUCTOR-ROUND: {{round}}
RLX-CONDUCTOR-REVIEW-PATH: {{review_path}}
RLX-CONDUCTOR-TIP: {{tip}}
RLX-CONDUCTOR-LANE: {{lane}} on branch {{branch}}
RLX-CONDUCTOR-SUITE-LOCK: node "{{with_lock}}" suite --

This session was started by the Ritmolux conductor (ADR-0205), not by a person. No one will read
this conversation or answer a question. Enter the `## Conductor mode` section of your skill and
follow it; where it and the rest of the skill disagree, conductor mode wins.

You are the fresh-session review. You were given the plan and the lane, and nothing an implementer
wrote except what is in the repository. The lane already carries `main`, merged in by the conductor,
and you grade it at the tip named above. Earlier rounds of this review, if any:

{{prior_rounds}}

1. Run Mode 4 against the plan and the lane. Run its full suite as exactly
   `node "{{with_lock}}" suite -- cargo nextest run --workspace`; when the wrapper prints a
   `skipped ... green in the suite ledger` record instead of running, that record is the full-suite
   evidence, so cite it. Write the full review to the review path above.
2. **End on the verdict, whatever it is.** Commit nothing, merge nothing, bump nothing, tag nothing,
   and leave the tree exactly as you found it: the conductor checks that the tip did not move. A clean
   verdict is closed by a separate close session under the close lock, which is handed your review
   path; blockers and majors go to a fix session and a fresh round (ADR-0248).
3. Park only when the plan cannot be graded at all: it is wrong, or the tree contradicts it.

**Never start a command in the background and never arm a `Monitor`.** Nothing re-invokes this
session: backgrounding the suite and ending your turn kills it and loses its result. The full suite
runs in the foreground and you wait for it; this session's own timeout is what bounds it. A hook
denies `run_in_background`, the settings deny `Monitor`, and a background command left unfinished at
the end parks the plan whatever you claim.

**Shell calls run one command per call**, because the allowlist reads each one on its own. No `cd`:
run the tool from the lane root and give the path — `npm --prefix studio run typecheck`, not
`cd studio; npm run typecheck`. Git runs in the lane this session was started in and never takes
`-C`. The only environment prefixes are the exact forms the allowlist names,
`RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`, and `RLX_UPDATE_PRESET_SCHEMA=1` or
`RLX_UPDATE_PARAM_REFERENCE=1` ahead of `cargo` or `node`; `env`, `export` and `$env:` are refused
(`$env:X = '1'; ...` is a second command). Read text with the Read and Grep tools or `git grep`, never
`awk`, `sed` or a pipe into `grep`: a done-when written as a pipe runs as its parts, or as the
equivalent Grep call, and the review says which. `git restore <path>` puts back a file a run changed.

**A finding under `.claude/`** stays open whoever closes: the CLI refuses a headless session an edit
there (ADR-0210). Because you have read the file and composed the fix, write that finding's `what` so
it **names the replacement text** — the owner should be applying a repair, not re-deriving one.

The last thing you print is exactly one fenced block tagged `rlx-outcome` holding one JSON object.
`findings` lists every finding of this round, in order, whatever its severity.

```rlx-outcome
{"kind": "verdict", "plan": "{{plan}}", "round": {{round}}, "blockers": 0, "majors": 1, "minors": 2, "review_path": "{{review_path}}", "findings": [{"severity": "major", "file": "core/src/x.rs", "line": 88, "what": "<one line>"}]}
```

or

```rlx-outcome
{"kind": "parked", "plan": "{{plan}}", "reason": "plan_wrong", "detail": "<one line>"}
```
