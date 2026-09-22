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

1. Run Mode 4 against the plan and the lane. Run its full suite as exactly
   `node "{{with_lock}}" suite -- cargo nextest run --workspace`; when the wrapper prints a
   `skipped ... green in the suite ledger` record instead of running, that record is the full-suite
   evidence, so cite it. Write the full review to the review path above.
2. If it carries any `blocker` or `major`: stop. Print the `verdict` outcome and nothing after it.
3. Only if it carries none: close on the branch in this worktree, in this order (the conductor-mode
   order of your skill):
   1. repair every `minor` or `nit` your skill's conductor mode lets a close repair (comment text,
      assertion or panic message text, Markdown prose **except under `.claude/`**), committed, and
      mark each with `fixed_in`;
   2. `git merge main`;
   3. the bookkeeping, the version bump and the studio sync, committed. The close commit adds a
      `## Close review` section to the plan, after `## Implementation log`: this round's review in
      full, then one line for every finding an earlier round raised and a fix round resolved, naming
      the fix commit;
   4. the whole gate on that tip, `nextest` as exactly the wrapped command above;
   5. an ANNOTATED tag on the branch tip;
   6. `node scripts/check-release-tag.mjs`.
   Never fast-forward `main`, never remove the worktree, never push: the conductor does the first two
   and the owner the third.
4. If the merge conflicts or the gate goes red during the close, park; do not work around it.

**Never start a command in the background and never arm a `Monitor`.** Nothing re-invokes this
session: backgrounding the suite and ending your turn kills it and loses its result, after your close
commits have landed. The full suite runs in the foreground and you wait for it; this session's own
timeout is what bounds it. A hook denies `run_in_background`, the settings deny `Monitor`, and a
background command left unfinished at the end parks the plan whatever you claim.

**Shell calls run one command per call**, because the allowlist reads each one on its own. No `cd`:
run the tool from the lane root and give the path — `npm --prefix studio run typecheck`, not
`cd studio; npm run typecheck`. Git runs in the lane this session was started in and never takes
`-C`. The only environment prefixes are the exact forms the allowlist names,
`RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`, and `RLX_UPDATE_PRESET_SCHEMA=1` or
`RLX_UPDATE_PARAM_REFERENCE=1` ahead of `cargo` or `node`; `env`, `export` and `$env:` are refused
(`$env:X = '1'; ...` is a second command). Read text with the Read and Grep tools or `git grep`, never
`awk`, `sed` or a pipe into `grep`: a done-when written as a pipe runs as its parts, or as the
equivalent Grep call, and the review says which. `git clean` and `git checkout` name their path after
`--`; `git restore <path>` is the ordinary way to put a file back.

The last thing you print is exactly one fenced block tagged `rlx-outcome` holding one JSON object.
`findings` lists every finding of this round, in order, whatever its severity.

```rlx-outcome
{"kind": "verdict", "plan": "{{plan}}", "round": {{round}}, "blockers": 0, "majors": 1, "minors": 2, "review_path": "{{review_path}}", "findings": [{"severity": "major", "file": "core/src/x.rs", "line": 88, "what": "<one line>"}]}
```

or, after a clean close:

```rlx-outcome
{"kind": "closed", "plan": "{{plan}}", "version": "<X.Y.Z or null>", "tag": "<vX.Y.Z or null>", "verdict": {"round": {{round}}, "blockers": 0, "majors": 0, "minors": 2, "review_path": "{{review_path}}", "findings": [{"severity": "minor", "file": "docs/x.md", "line": 12, "what": "<one line>", "fixed_in": "<sha of the repair commit>"}, {"severity": "minor", "file": "core/src/y.rs", "line": 40, "what": "<one line, left open>"}]}}
```

`fixed_in` goes only on a finding the close repaired, naming the commit that changes that finding's
file; every other finding carries none.

**Never attempt an `Edit` or a `Write` under `.claude/`.** The CLI refuses one to a headless session
whatever the allowlist says (ADR-0210). A finding there stays **open** and carries no `fixed_in`: the
owner applies it, reading it in the digest. Because you have read the file and composed the fix,
write that finding's `what` so it **names the replacement text** — the owner should be applying a
repair, not re-deriving one.

or

```rlx-outcome
{"kind": "parked", "plan": "{{plan}}", "reason": "merge_conflict | check_red | plan_wrong", "detail": "<one line>"}
```
