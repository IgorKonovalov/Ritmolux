# The headless contract — observed

Evidence for the conductor's design (Plan 0187 Phase 1). Every row below was **observed** by running
`probe.mjs` against a disposable worktree, not read from documentation. The conductor refuses a CLI
version this table was not produced on; re-running the probe is how a new version gets verified.

- **Date:** 2026-09-14
- **CLI:** `claude --version` → `2.1.270 (Claude Code)`
- **Auth:** claude.ai subscription (`apiKeySource: "none"` in the init event), not an API key
- **Model:** `--model haiku` → `claude-haiku-4-5-20251001`
- **Machine:** Windows 10, Node v22.22.2, worktree at `WORK/rlx-probe-0187`
- **Run:** `node tools/conductor/spike/probe.mjs --model haiku`; raw output under
  `target/conductor-spike/<stamp>/` (never committed)

## Re-verified on 2.1.272

- **Date:** 2026-09-15
- **CLI:** `claude --version` -> `2.1.272 (Claude Code)`
- **Run:** `node tools/conductor/spike/probe.mjs --model haiku`; $0.098 (session A) + $0.053 (session B)

Every row of the table below was re-observed and holds unchanged: the skill loads, the deny hook
reaches the session as a readable tool error, an allowed `Bash` runs and a disallowed one is denied
without stalling, `Edit`/`Write` land, the `result` event carries the same fields, the budget stop is
still exit 1 with `error_max_budget_usd` after one turn, and `git worktree remove` leaves no handle.

**One count in the table is no longer this repository's.** The deny-hook row says four
`PreToolUse:Bash` hooks run per Bash call. Six ran: `.claude/settings.json` has gained
`block-push-and-history-rewrite.js` and `conductor-suite-lock.js`, so it is four project hooks plus
the user-level one plus the `--settings` one. That is this repository changing, not the CLI, and the
hooks compose the same way - the `--settings` hook adds to the project's rather than replacing them.

**Not re-observed:** the probe records `result.keys` but not the values of `terminal_reason`,
`apiKeySource` or `permission_denials`. Those three readings are carried over from 2.1.270.

## Re-verified on 2.1.273

- **Date:** 2026-09-16
- **CLI:** `claude --version` -> `2.1.273 (Claude Code)`
- **Run:** `node tools/conductor/spike/probe.mjs --model haiku --sessions a,b`; $0.090 (session A, 8
  turns, 37.4 s) + $0.055 (session B). Raw output under `target/conductor-spike/<stamp>/`.

Every row of the table below was re-observed and holds unchanged. Session A: the `dev` skill loads
(`slash_commands_has_dev: true`), `system/hook_started` and `hook_response` bracket every `Bash`
call, the hook process saw `RLX_CONDUCTOR=1` and the run's own `RLX_PROBE_TOKEN` on all four, the
denied `node -e` produced a `system/permission_denied` with no stall, and `Write` then `Read` then
`Edit` all landed on the same file (`probe-out.txt` read back `beta`). `result/success`, exit 0.
Session B still ends exit 1 with `error_max_budget_usd` after one turn. `git worktree remove`
returned 0 with empty stderr and the directory was gone: no handle left.

**The `result` event gained five keys and lost none**, which is the one thing that moved. New on
2.1.273: `fast_mode_state`, `fast_mode_disabled_reason`, `subagent_stats`, `queued_turn_count`,
`result_index`. **Every field the conductor actually reads is still there** — `result`,
`session_id`, `subtype`, `total_cost_usd`, `is_error`, `num_turns`, `errors`, `stop_reason`,
`terminal_reason`, `permission_denials` — checked against `lib/outcome.mjs`'s readers one by one.
`errors` appears on the budget stop and not on the success, and `result` the other way round, which
is the same split 2.1.270 recorded.

**Sessions C and D were not re-run here because they were already run on this version**: the
`.claude/` section below *is* the 2.1.273 reading, taken the same day. Between that section and this
one, all four sessions have now been observed on 2.1.273.

**Not re-observed**, unchanged from the 2.1.272 note: the probe records `result.keys` but not the
values of `terminal_reason`, `apiKeySource` or `permission_denials`. Those three readings are still
carried over from 2.1.270.

## What the probe does

Four sessions. Two of them, A and B, are `claude -p "/dev implement plan 9999"` with the worktree as cwd, and with
`--output-format stream-json --verbose --include-hook-events --permission-mode dontAsk --settings
<file>` and `RLX_CONDUCTOR=1` plus a random `RLX_PROBE_TOKEN` in the child's environment. The
settings file allows `Read Glob Grep Edit Write Skill`, `Bash(cargo --version)`, `Bash(git add *)` and
`Bash(git status *)`, and registers the probe itself as an extra `PreToolUse` hook that records the
environment it sees. `fixture-plan.md` is copied into the worktree as
`docs/plans/9999-probe-fixture.md`.

- **Session A** (clean run) adds `--append-system-prompt-file` carrying a marker token and an ordered
  list of eight steps, and `--max-budget-usd 0.50`.
- **Session B** (budget stop) has no appended prompt and `--max-budget-usd 0.0001`.
- **Sessions C and D** ask the `.claude/` question below. They invoke no skill — their steps are the
  `-p` prompt itself, because under `/dev` the session obeyed that skill's restate-and-wait instead —
  and they use no probe hook. C runs under `tools/conductor/settings.conductor.json` as the conductor
  passes it; D runs under a generated file that additionally names `.claude/` paths.

After all four exit, the parent runs `git worktree remove --force` on the worktree and deletes the branch.

## Evidence

| Row | Observed |
|---|---|
| A `-p` prompt beginning `/dev …` loads the project skill | The init event lists it: `"skills":["architect","dev","preset-author",…,"studio-builder",…]`, `slash_commands` contains `"dev"`. The session quoted the skill's own heading, which exists only in `SKILL.md`: *"Step 1: First markdown heading of the skill instructions: `# dev — Ritmolux`"*. The skill body is **not** echoed into the stream as a user event. |
| `CLAUDE.md` is in context | The session quoted it verbatim: *"Python sidecar for the diffusion-filter pass (ADR-0122). Not a cargo crate, not in the workspace, never shipped; its cost figures live in exactly one page (docs/diffusion-filter.md) and check-filter-figures.mjs holds them there."* |
| The project deny hooks fire, and the denial reaches the session as a readable tool error | Four `PreToolUse:Bash` hooks ran per Bash call (the two project hooks, the user-level hook, the `--settings` hook). For `git add -A`: `hook_response` with `permissionDecision":"deny"`, then a `tool_result` with `"is_error":true` and content *"Blocked broad staging: \"git add -A\". Broad \"git add -A / --all / . / :/\" sweeps untracked…"*, `tool_result_meta: [{"non_execution_kind":"permission-rule"}]`. The session reported the text back. Hooks from `--settings` **add to** the project's; they do not replace them. |
| `--permission-mode dontAsk` + `--settings` allowlist: an allowed `Bash(cargo …)` runs | `cargo --version` → `tool_result` `"is_error":false`, content `cargo 1.97.1 (c980f4866 2026-06-30)`. |
| … a disallowed call is denied rather than hanging | `node -e "console.log(42)"` → a `system/permission_denied` event (`"decision_reason_type":"mode"`) and a `tool_result` `"is_error":true`: *"Permission to use Bash has been denied because Claude Code is running in don't ask mode. …"*. No stall: session A's whole run took 32.0 s. |
| … `Edit` / `Write` inside the worktree work | `Write` → *"File created successfully at: …\rlx-probe-0187\probe-out.txt"*; `Edit` → *"…has been updated successfully"*. Read from the parent after exit: the file holds `beta`. |
| `stream-json`: where the final text, session id, cost and error flag are | The **last line** is `{"type":"result", …}`. It carries `result` (the final assistant text — session A's ends with the fenced `rlx-outcome` block), `session_id`, `total_cost_usd` (`0.083473`), `is_error` (`false`), `subtype` (`"success"`), `num_turns` (`8`), `stop_reason` (`"end_turn"`), `terminal_reason` (`"completed"`), and `permission_denials` (a list naming both denied calls with their `tool_input`). `session_id` is also on `system/init` and on every event. Other event kinds seen: `system/thinking_tokens`, `assistant`, `user`, `system/hook_started`, `system/hook_response`, `system/permission_denied`, `rate_limit_event`. |
| A run ended by `--max-budget-usd` is distinguishable from a clean one | Session B: **exit code 1** (A: 0); final event `"type":"result","subtype":"error_max_budget_usd","is_error":true,"terminal_reason":"budget_exhausted","errors":["Reached maximum budget ($0.0001)"]`, `"num_turns":1`, `"total_cost_usd":0.051112`. **The cap applies under subscription auth, and it is checked between turns, not within one:** a $0.0001 cap still spent $0.051, the cost of the first turn. A step can overrun its budget by one turn. |
| `--append-system-prompt-file` content is visible to the skill | The flag is accepted although `--help` lists only `--append-system-prompt`. The session reported the marker it carried: *"RLX-PROBE-MARKER value: `kestrel-4417`"*, and followed its step list in place of the `/dev` skill's restate-and-wait. |
| An environment variable set by the parent is visible to hooks | The probe hook recorded, for all four Bash calls, `{"RLX_CONDUCTOR":"1","RLX_PROBE_TOKEN":"c746df622c6e",…}` — the token matches the one the parent generated for this run. |
| On Windows, after the child exits, `git worktree remove` on its cwd succeeds | `git worktree remove --force …\rlx-probe-0187` → exit 0, empty stderr; the directory no longer exists; `git branch -D` succeeded. No handle was left. |

## May a headless session edit `.claude/`? Observed on 2.1.273

- **Date:** 2026-09-16
- **CLI:** `claude --version` → `2.1.273 (Claude Code)`
- **Model:** `--model haiku`
- **Run:** `node tools/conductor/spike/probe.mjs --model haiku --sessions c` then `--sessions d`;
  $0.065 and $0.056. Raw output under `target/conductor-spike/<stamp>/` (never committed).
- **Subject:** `.claude/skills/probe-scratch/NOTES.md` inside the probe worktree, holding the word
  `alpha`, created by the probe before the session. Paths given to the session **absolute**.

Backlog 0230 recorded that the CLI refuses a `claude -p` session's `Edit` under `.claude/` although
`settings.conductor.json` allows the tool, and ADR-0209 grants a close every file under
`.claude/skills/`. Two sessions, differing only in the settings in force:

| Tool call attempted | Under `settings.conductor.json` (session C) | Under settings naming `.claude/` (session D) |
|---|---|---|
| `Read <worktree>/.claude/skills/probe-scratch/NOTES.md` | **allowed** — returned the file | **allowed** |
| `Edit` that same file, `alpha` → `beta` | **denied** — *"Permission to use Edit has been denied because Claude Code is running in don't ask mode."* | **denied**, same text |
| `Write <worktree>/.claude/skills/probe-scratch/NEW.md` | **denied**, same text | **denied**, same text |
| `Write <worktree>/probe-control.txt` — the control, same worktree, same turn, outside `.claude/` | **allowed** — *"File created successfully at: …\rlx-probe-0187\probe-control.txt"* | **allowed** |

Read back from the parent after both sessions exited: `NOTES.md` still holds `alpha`, `NEW.md` does
not exist, `probe-control.txt` holds `delta`. Neither session touched anything under `.claude/`.

**Which settings were in force.** Session C ran under `tools/conductor/settings.conductor.json`
exactly as the conductor passes it, which allows the bare tools `Edit` and `Write`. Session D ran
under a generated file allowing, in addition, `Edit(.claude/**)`, `Write(.claude/**)`,
`Edit(//.claude/**)`, `Write(//.claude/**)` and the same two spelled with the worktree's absolute
path. Both sessions used `--permission-mode dontAsk`. **No spelling reached it.**

**So it is a CLI restriction, not a configuration gap** — on 2.1.273, by the settings surface probed.
A write under a project's `.claude/` is refused whatever the allowlist says, while a read is not, and
a write anywhere else in the same worktree is not. Anything that would add a setting to this table has
to come from a later CLI, and re-running the probe is how that gets noticed.

**One thing that is not a CLI rule.** In an earlier run the session was given the path *relative*
(`.claude/skills/probe-scratch/NOTES.md`) and expanded it itself to
`C:\Users\<user>\.claude\skills\…`, the user's own configuration directory, so the tool answered
*"File does not exist"*. That is the model choosing a path, not the CLI resolving one: step 5 of
session C reads the same relative path and the tool resolves it against the worktree and returns the
file. Give `.claude/` paths absolute when it matters.

## Also observed, and relevant to the conductor

- **Usage limits are visible in the stream before they bite.** `rate_limit_event` carries
  `rate_limit_info.status` (`"allowed_warning"` here), `rateLimitType`, `utilization` and `resetsAt`
  per window. The conductor records these; it does not act on them.
- **User-level settings apply to `-p` sessions**: the user's own `PreToolUse` hook ran, and
  `~/.claude/settings.json`'s model was overridden by `--model`.
- **The budget stop can land mid-turn.** Session B's `stop_reason` was `"tool_use"`: the turn had
  asked for a tool that was never run.
