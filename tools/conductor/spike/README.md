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

## Re-verified on 2.1.278, on Linux

- **Date:** 2026-09-22
- **CLI:** `claude --version` -> `2.1.278 (Claude Code)`
- **Machine:** Arch Linux (Omarchy, Hyprland), Node v26.8.2, worktree at `~/Work/rlx-probe-0187`
- **Run:** `node tools/conductor/spike/probe.mjs --model haiku`, all four sessions: $0.085 (A) +
  $0.056 (B) + $0.064 (C) + $0.068 (D), 80 s. Raw output under `target/conductor-spike/<stamp>/`.

This is the first reading off Windows, and every row holds. Each table below now has a Linux column
beside the Windows one. Two things are new:

- **The three values the 2.1.272 and 2.1.273 notes carried over are now read.** `terminal_reason`,
  `apiKeySource` and `permission_denials` all match what 2.1.270 recorded (Evidence, `stream-json`
  row).
- **`result` gained five keys and lost none**, and every field `lib/outcome.mjs` reads is still
  there.

**The foreground rule is not a probe row.** `conductor-no-background.js` is held by its cases in
`tools/conductor/test/hooks.test.mjs`, which are green on this machine. The probe shows only that
the project's `PreToolUse:Bash` hooks run in a Linux headless session. No live `run_in_background`
call was made.

## Re-verified on 2.1.280, on Linux

- **Date:** 2026-09-24
- **CLI:** `claude --version` -> `2.1.280 (Claude Code)`
- **Machine:** Arch Linux (Omarchy, Hyprland), Node v26.8.2, worktree at `~/Work/rlx-probe-0187`
- **Run:** `node tools/conductor/spike/probe.mjs --model haiku`, all four sessions: $0.090 (A) +
  $0.060 (B) + $0.067 (C) + $0.052 (D). Raw output under `target/conductor-spike/<stamp>/`.

Every row holds. Session A: the `dev` skill loads, six `PreToolUse:Bash` hooks per Bash call (24
`hook_started` for four calls), the `git add -A` denial comes back as the readable hook error, `cargo
--version` runs, the `node -e` call is denied without a stall, `Write` then `Edit` land.
`result/success`, `terminal_reason: "completed"`, `apiKeySource: "none"`, two `Bash` entries in
`permission_denials`. Session C refuses `Edit` and `Write` under `.claude/`, allows the `Read`, and
the control `Write` outside `.claude/` succeeds. `git worktree remove` exits 0. **The `result` event
carries the same 20 keys as 2.1.278**, none gained and none lost.

**One thing moved: the budget stop lets the tool call already requested run first.** Session B still
ends exit 1, `error_max_budget_usd`, `terminal_reason: "budget_exhausted"`, `stop_reason:
"tool_use"`, one model message. On 2.1.278 the stream went from that message's `tool_use` straight
to `result`. On 2.1.280 a `user` event carrying the `Read`'s `tool_result` comes between them, and
`num_turns` reads 2 rather than 1. So the overrun the budget row describes is still one model turn,
and the tool call that turn asked for now runs before the stop. A step stopped by its budget can
therefore leave that tool call's effect in the lane, a commit included. The conductor already
reads the lane from `git` after any park, so nothing in it depends on the old behaviour.

**Session D's control step is not a regression.** Its `Write` to `probe-control.txt` failed with
*"File has not been read yet"*, because session C had created that file. That is the tool's
read-before-write rule, not a permission refusal. On 2.1.278 the model read the file first, and
this time it did not. Session C's control write is the reading that counts.

## Re-verified on 2.1.282, on Linux

- **Date:** 2026-09-26
- **CLI:** `claude --version` -> `2.1.282 (Claude Code)`
- **Machine:** Arch Linux (Omarchy, Hyprland), Node v26.8.2, worktree at `~/Work/rlx-probe-0187`
- **Run:** `node tools/conductor/spike/probe.mjs --model haiku`, all four sessions: $0.093 (A) +
  $0.058 (B) + $0.069 (C) + $0.062 (D). Raw output under `target/conductor-spike/<stamp>/`.

Every row holds, with nothing moved since 2.1.280. Session A: the `dev` skill loads, six
`PreToolUse:Bash` hooks per Bash call (24 `hook_started` for four calls), the `git add -A` denial
comes back as the readable hook error, `cargo --version` runs, the `node -e` call is denied without a
stall, and `Write` then `Edit` land (`probe-out.txt` reads back `beta`). The result is
`result/success` after 7 turns in 29.7 s. Session B ends exit 1, `error_max_budget_usd`, after 2
turns, with the `Read` its one model turn asked for run before the stop, as on 2.1.280. `git worktree
remove` exits 0 with empty stderr and leaves no directory. **The `result` event's key set is
identical to 2.1.278's and 2.1.280's**, compared against the raw output of both runs: 25 keys on a
success and 20 on the budget stop. Every field `lib/outcome.mjs` reads is present.

**`.claude/` is still write-denied to a headless session.** Session D, under settings naming `.claude/`
paths, read `NOTES.md`, and was refused `Edit` and `Write` there with the don't-ask-mode denial. Its
control `Write` outside `.claude/` succeeded. Session C, under `settings.conductor.json`, was refused
the `Write` the same way, and its control write succeeded. **C's `Edit` is not a reading:** the model
wrote the path with a backslash, so the tool answered *"File does not exist"* rather than ruling on
the permission. Its first `Read` failed on the same path, and its relative-path `Read` succeeded.
D is the reading that counts for `Edit`, as C was for the control write on 2.1.280.

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

| Row | Observed (Windows 10, Node 22, 2.1.270) | Linux (Arch, Node 26.8.2, 2.1.278) |
|---|---|---|
| A `-p` prompt beginning `/dev …` loads the project skill | The init event lists it: `"skills":["architect","dev","preset-author",…,"studio-builder",…]`, `slash_commands` contains `"dev"`. The session quoted the skill's own heading, which exists only in `SKILL.md`: *"Step 1: First markdown heading of the skill instructions: `# dev — Ritmolux`"*. The skill body is **not** echoed into the stream as a user event. | Holds. `slash_commands_has_dev: true`, `dev` in `skills`, and the session quoted `# dev — Ritmolux`. |
| `CLAUDE.md` is in context | The session quoted it verbatim: *"Python sidecar for the diffusion-filter pass (ADR-0122). Not a cargo crate, not in the workspace, never shipped; its cost figures live in exactly one page (docs/diffusion-filter.md) and check-filter-figures.mjs holds them there."* | Holds. The session quoted *"Python sidecar for the diffusion-filter pass (ADR-0122)."* |
| The project deny hooks fire, and the denial reaches the session as a readable tool error | Four `PreToolUse:Bash` hooks ran per Bash call (the two project hooks, the user-level hook, the `--settings` hook). For `git add -A`: `hook_response` with `permissionDecision":"deny"`, then a `tool_result` with `"is_error":true` and content *"Blocked broad staging: \"git add -A\". Broad \"git add -A / --all / . / :/\" sweeps untracked…"*, `tool_result_meta: [{"non_execution_kind":"permission-rule"}]`. The session reported the text back. Hooks from `--settings` **add to** the project's; they do not replace them. | Holds. **Six** `PreToolUse:Bash` hooks per Bash call, the same count as 2.1.272 and 2.1.273: 24 `hook_started` for four calls. The `git add -A` denial came back as *"PreToolUse:Bash hook error: Blocked broad staging: \"git add -A\"…"*, `is_error: true`. |
| `--permission-mode dontAsk` + `--settings` allowlist: an allowed `Bash(cargo …)` runs | `cargo --version` → `tool_result` `"is_error":false`, content `cargo 1.97.1 (c980f4866 2026-06-30)`. | Holds. `cargo 1.97.1 (c980f4866 2026-06-30)`, `is_error: false`. |
| … a disallowed call is denied rather than hanging | `node -e "console.log(42)"` → a `system/permission_denied` event (`"decision_reason_type":"mode"`) and a `tool_result` `"is_error":true`: *"Permission to use Bash has been denied because Claude Code is running in don't ask mode. …"*. No stall: session A's whole run took 32.0 s. | Holds. The `node -e` call got `system/permission_denied` and the same *"…running in don't ask mode"* error, with no stall. Session A ran in 27.6 s. |
| … `Edit` / `Write` inside the worktree work | `Write` → *"File created successfully at: …\rlx-probe-0187\probe-out.txt"*; `Edit` → *"…has been updated successfully"*. Read from the parent after exit: the file holds `beta`. | Holds. `Write` created `/home/igor/Work/rlx-probe-0187/probe-out.txt`, then `Edit` updated it, and the parent read back `beta`. |
| `stream-json`: where the final text, session id, cost and error flag are | The **last line** is `{"type":"result", …}`. It carries `result` (the final assistant text — session A's ends with the fenced `rlx-outcome` block), `session_id`, `total_cost_usd` (`0.083473`), `is_error` (`false`), `subtype` (`"success"`), `num_turns` (`8`), `stop_reason` (`"end_turn"`), `terminal_reason` (`"completed"`), and `permission_denials` (a list naming both denied calls with their `tool_input`). `session_id` is also on `system/init` and on every event. Other event kinds seen: `system/thinking_tokens`, `assistant`, `user`, `system/hook_started`, `system/hook_response`, `system/permission_denied`, `rate_limit_event`. | Holds. Every field the conductor reads is present. New since 2.1.273: `api_error_status`, `ttft_ms`, `ttft_stream_ms`, `time_to_request_ms` and `first_content_frame_ms`. None was removed. First recorded values: `stop_reason: "end_turn"`, `terminal_reason: "completed"`, `apiKeySource: "none"`, and `permission_denials` naming both denied calls with their `tool_input`. A cost $0.085, 7 turns. |
| A run ended by `--max-budget-usd` is distinguishable from a clean one | Session B: **exit code 1** (A: 0); final event `"type":"result","subtype":"error_max_budget_usd","is_error":true,"terminal_reason":"budget_exhausted","errors":["Reached maximum budget ($0.0001)"]`, `"num_turns":1`, `"total_cost_usd":0.051112`. **The cap applies under subscription auth, and it is checked between turns, not within one:** a $0.0001 cap still spent $0.051, the cost of the first turn. A step can overrun its budget by one turn. | Holds. Session B: exit 1, `error_max_budget_usd`, `errors: ["Reached maximum budget ($0.0001)"]`, `stop_reason: "tool_use"`, `terminal_reason: "budget_exhausted"`, 1 turn, $0.056. |
| `--append-system-prompt-file` content is visible to the skill | The flag is accepted although `--help` lists only `--append-system-prompt`. The session reported the marker it carried: *"RLX-PROBE-MARKER value: `kestrel-4417`"*, and followed its step list in place of the `/dev` skill's restate-and-wait. | Holds. The session reported `kestrel-4417` and followed the appended steps. |
| An environment variable set by the parent is visible to hooks | The probe hook recorded, for all four Bash calls, `{"RLX_CONDUCTOR":"1","RLX_PROBE_TOKEN":"c746df622c6e",…}` — the token matches the one the parent generated for this run. | Holds. All four records carry `RLX_CONDUCTOR: "1"` and the run's token, `baef9bbae757`. |
| On Windows, after the child exits, `git worktree remove` on its cwd succeeds | `git worktree remove --force …\rlx-probe-0187` → exit 0, empty stderr; the directory no longer exists; `git branch -D` succeeded. No handle was left. | Holds on Linux as well. `git worktree remove --force /home/igor/Work/rlx-probe-0187` exits 0 with empty stderr, and afterwards the directory does not exist. |

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

| Tool call attempted | Under `settings.conductor.json` (session C) | Under settings naming `.claude/` (session D) | Linux, 2.1.278 (C / D) |
|---|---|---|---|
| `Read <worktree>/.claude/skills/probe-scratch/NOTES.md` | **allowed** — returned the file | **allowed** | **allowed** / **allowed** |
| `Edit` that same file, `alpha` → `beta` | **denied** — *"Permission to use Edit has been denied because Claude Code is running in don't ask mode."* | **denied**, same text | **denied** / **denied**, the same text |
| `Write <worktree>/.claude/skills/probe-scratch/NEW.md` | **denied**, same text | **denied**, same text | **denied** / **denied**, the same text |
| `Write <worktree>/probe-control.txt` — the control, same worktree, same turn, outside `.claude/` | **allowed** — *"File created successfully at: …\rlx-probe-0187\probe-control.txt"* | **allowed** | **allowed** / **allowed**. The parent read back `delta` |

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
  per window. The conductor records the readings as they arrive, and acts only on the one that
  ends a session.
- **A session the usage limit ends has a shape of its own** (plan 0215's first implement session,
  2.1.278, Linux). The last `rate_limit_event` reads `"status":"rejected"` with `resetsAt` in epoch
  seconds. The result event is `"subtype":"success"` with `"is_error":true`,
  `"terminal_reason":"api_error"`, `"api_error_status":429`, and `result` holding the message:
  *"You've hit your session limit · resets 9:50pm (Europe/Belgrade)"*. `errors` is absent. The
  conductor waits for that reset and then continues the session.
- **`-p --resume <session_id>` continues a headless session** (2.1.280, Linux, `--model haiku`,
  2026-09-24). A second invocation answered from the first one's context (the word it had been
  asked to remember) under the **same** `session_id`, and it emitted a fresh `system/init` whose
  `skills` list was complete. Its `total_cost_usd` was **cumulative**: $0.0233 against the first
  invocation's $0.0201, where the second invocation's own tokens price at about $0.003. `num_turns`
  counted that invocation only (1). So a continued step's spend is the last result's figure, and its
  turns are the sum.
- **User-level settings apply to `-p` sessions**: the user's own `PreToolUse` hook ran, and
  `~/.claude/settings.json`'s model was overridden by `--model`.
- **The budget stop can land mid-turn.** Session B's `stop_reason` was `"tool_use"`: the turn had
  asked for a tool that was never run.
