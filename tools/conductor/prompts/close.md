RLX-CONDUCTOR-MODE: close
RLX-CONDUCTOR-PLAN: {{plan}}
RLX-CONDUCTOR-PLAN-FILE: {{plan_file}}
RLX-CONDUCTOR-ROUND: {{round}}
RLX-CONDUCTOR-REVIEW-PATH: {{review_path}}
RLX-CONDUCTOR-GRADED: {{tip}}
RLX-CONDUCTOR-LANE: {{lane}} on branch {{branch}}
RLX-CONDUCTOR-SUITE-LOCK: node "{{with_lock}}" suite --

This session was started by the Ritmolux conductor (ADR-0205), not by a person. No one will read
this conversation or answer a question. Enter the `## Conductor mode` section of your skill and
follow it; where it and the rest of the skill disagree, conductor mode wins.

You are the close. A fresh review graded this plan clean at the tip named above, with no blocker and
no major, and wrote its review to the path above. Read that review and the plan first: the review is
done, and you do not grade the plan again. Earlier rounds of the review, if any:

{{prior_rounds}}

The conductor holds the close lock for you, so the version you bump lands on the `main` it was
computed against. Close on the branch in this worktree, in this order (the conductor-mode close of your
skill):

1. Repair every `minor` or `nit` of the review that your skill's conductor mode lets a close repair
   (comment text, assertion or panic message text, Markdown prose **except under `.claude/`**),
   committed, and mark each with `fixed_in`.
2. `git merge main`. **A conflict only in Markdown under `docs/`** (the plans index, a README) is
   yours to resolve. **Any other conflicted path** — code, a test, a preset, a config, a script,
   anything under `.claude/` — is not: run `git merge --abort` and park `merge_conflict` naming the
   paths. The conductor runs a merge session and starts a close again (ADR-0248).
3. The bookkeeping, the version bump and the studio sync, committed. The close commit adds a
   `## Close review` section to the plan, after `## Implementation log`: the review at the path above
   in full, then one line for every finding an earlier round raised and a fix round resolved, naming
   the fix commit. A row reading `owed` stays owed; the `Status:` line and the `## Close review` name
   it (ADR-0249).
4. The whole gate on that tip, `nextest` as exactly
   `node "{{with_lock}}" suite -- cargo nextest run --workspace`. A red parks `check_red`: do not tag,
   and do not work around it.
5. An ANNOTATED tag on the branch tip.
6. `node scripts/check-release-tag.mjs`.

Never fast-forward `main`, never remove the worktree, never push: the conductor does the first two and
the owner the third.

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
`awk`, `sed` or a pipe into `grep`. `git clean` and `git checkout` name their path after `--`;
`git restore <path>` is the ordinary way to put a file back.

**Never attempt an `Edit` or a `Write` under `.claude/`.** The CLI refuses one to a headless session
whatever the allowlist says (ADR-0210). A finding there stays **open** and carries no `fixed_in`.

The last thing you print is exactly one fenced block tagged `rlx-outcome` holding one JSON object: the
review's verdict, every finding of it in order, with `fixed_in` only on a finding you repaired, naming
the commit that changes that finding's file.

```rlx-outcome
{"kind": "closed", "plan": "{{plan}}", "version": "<X.Y.Z or null>", "tag": "<vX.Y.Z or null>", "verdict": {"round": {{round}}, "blockers": 0, "majors": 0, "minors": 2, "review_path": "{{review_path}}", "findings": [{"severity": "minor", "file": "docs/x.md", "line": 12, "what": "<one line>", "fixed_in": "<sha of the repair commit>"}, {"severity": "minor", "file": "core/src/y.rs", "line": 40, "what": "<one line, left open>"}]}}
```

or

```rlx-outcome
{"kind": "parked", "plan": "{{plan}}", "reason": "merge_conflict | check_red | plan_wrong", "detail": "<one line: what, and the paths or file to read>"}
```
