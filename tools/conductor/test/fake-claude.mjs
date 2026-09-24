#!/usr/bin/env node
// A stand-in for the `claude` CLI, so no conductor test spends money or needs a network.
//
// It accepts the flags lib/step.mjs passes, emits a stream-json transcript in the shape
// tools/conductor/spike/README.md recorded (a system/init event, an assistant text event, and a
// final result event), and lets a test decide what the session "did":
//
//   FAKE_CLAUDE_SCENARIO  path to an ES module whose default export is
//                         async ({ args, cwd, env, prompt, append, vars }) =>
//                           { text?, subtype?, exitCode?, costUsd?, numTurns?, noResult?, stream?,
//                             skills?, hooks?, isError?, apiErrorStatus?, resultText? }
//                         `skills` replaces system/init's skill list (null omits it); `hooks: false`
//                         makes a shell call in `stream` leave no line in RLX_HOOK_LOG.
//                         It may run git in `cwd` to make the commits a real session would.
//                         `stream` is a list of events emitted between system/init and the final
//                         text, in the shapes a real session emits: an assistant tool_use, a user
//                         tool_result, system/task_notification, system/permission_denied,
//                         rate_limit_event. A string entry is written as a raw line, JSON or not.
//                         `isError`, `apiErrorStatus` and `resultText` shape an error result the way
//                         the API's usage limit ends a session: subtype success, is_error true, a
//                         429 status and the limit's message as the result text. `--resume <id>`
//                         continues that session id, as the real CLI does.
//   FAKE_CLAUDE_LOG       JSONL file receiving one record per invocation: args, cwd, and the
//                         RLX_ environment the session saw.
//   FAKE_CLAUDE_VERSION   what `--version` prints (default: the verified CLI's string).

import { appendFileSync, readFileSync } from "node:fs";
import { pathToFileURL } from "node:url";

const args = process.argv.slice(2);

if (args[0] === "--version" || args[0] === "-v") {
  process.stdout.write(`${process.env.FAKE_CLAUDE_VERSION ?? "2.1.270 (Claude Code)"}\n`);
  process.exit(0);
}

const flag = (name) => {
  const i = args.indexOf(name);
  return i >= 0 ? args[i + 1] : undefined;
};

const append = flag("--append-system-prompt-file");
const appendText = append ? readFileSync(append, "utf8") : "";
const vars = Object.fromEntries(
  [...appendText.matchAll(/^RLX-CONDUCTOR-([A-Z-]+):\s*(.*)$/gm)].map((m) => [m[1].toLowerCase(), m[2].trim()]),
);

if (process.env.FAKE_CLAUDE_LOG) {
  appendFileSync(
    process.env.FAKE_CLAUDE_LOG,
    JSON.stringify({
      args,
      cwd: process.cwd(),
      env: Object.fromEntries(Object.entries(process.env).filter(([k]) => k.startsWith("RLX_"))),
      vars,
    }) + "\n",
  );
}

const sessionId = flag("--resume") ?? `fake-${process.pid}-${Date.now()}`;
const emit = (e) => process.stdout.write(JSON.stringify({ ...e, session_id: sessionId }) + "\n");

let outcome = { text: "no scenario", subtype: "success" };
if (process.env.FAKE_CLAUDE_SCENARIO) {
  const mod = await import(pathToFileURL(process.env.FAKE_CLAUDE_SCENARIO).href);
  outcome = await mod.default({
    args,
    cwd: process.cwd(),
    env: process.env,
    prompt: flag("-p"),
    append: appendText,
    vars,
  });
}

const skills = outcome.skills === undefined ? ["architect", "dev", "studio-builder", "preset-author"] : outcome.skills;
emit({ type: "system", subtype: "init", cwd: process.cwd(), model: flag("--model") ?? "fake", permissionMode: flag("--permission-mode"), ...(skills ? { skills } : {}) });
for (const e of outcome.stream ?? []) {
  if (typeof e === "string") {
    process.stdout.write(`${e}\n`);
    continue;
  }
  // The real CLI runs the project's PreToolUse hooks on every shell call; the suite-lock hook logs
  // each one to RLX_HOOK_LOG. `hooks: false` stands in for a CLI that stopped running them.
  const shell = (e.message?.content ?? []).filter?.((c) => c?.type === "tool_use" && (c.name === "Bash" || c.name === "PowerShell")) ?? [];
  if (shell.length && outcome.hooks !== false && process.env.RLX_CONDUCTOR === "1" && process.env.RLX_HOOK_LOG) {
    for (const c of shell) appendFileSync(process.env.RLX_HOOK_LOG, JSON.stringify({ hook: "fake", tool: c.name, decision: "allow" }) + "\n");
  }
  emit(e);
}
emit({ type: "assistant", message: { role: "assistant", content: [{ type: "text", text: outcome.text ?? "" }] } });

if (!outcome.noResult) {
  const subtype = outcome.subtype ?? "success";
  const budget = subtype === "error_max_budget_usd";
  emit({
    type: "result",
    subtype,
    is_error: outcome.isError ?? subtype !== "success",
    terminal_reason: budget ? "budget_exhausted" : outcome.isError ? "api_error" : subtype === "success" ? "completed" : "error",
    total_cost_usd: outcome.costUsd ?? 0.01,
    num_turns: outcome.numTurns ?? 1,
    result: subtype === "success" ? outcome.resultText ?? outcome.text ?? "" : undefined,
    ...(outcome.apiErrorStatus ? { api_error_status: outcome.apiErrorStatus } : {}),
    errors: budget ? [`Reached maximum budget ($${flag("--max-budget-usd")})`] : subtype === "success" ? undefined : ["fake error"],
    permission_denials: [],
  });
}
process.exit(outcome.exitCode ?? ((outcome.subtype && outcome.subtype !== "success") || outcome.isError ? 1 : 0));
