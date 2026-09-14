#!/usr/bin/env node
// A stand-in for the `claude` CLI, so no conductor test spends money or needs a network.
//
// It accepts the flags lib/step.mjs passes, emits a stream-json transcript in the shape
// tools/conductor/spike/README.md recorded (a system/init event, an assistant text event, and a
// final result event), and lets a test decide what the session "did":
//
//   FAKE_CLAUDE_SCENARIO  path to an ES module whose default export is
//                         async ({ args, cwd, env, prompt, append, vars }) =>
//                           { text?, subtype?, exitCode?, costUsd?, noResult? }
//                         It may run git in `cwd` to make the commits a real session would.
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

const sessionId = `fake-${process.pid}-${Date.now()}`;
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

emit({ type: "system", subtype: "init", cwd: process.cwd(), model: flag("--model") ?? "fake", permissionMode: flag("--permission-mode") });
emit({ type: "assistant", message: { role: "assistant", content: [{ type: "text", text: outcome.text ?? "" }] } });

if (!outcome.noResult) {
  const subtype = outcome.subtype ?? "success";
  const budget = subtype === "error_max_budget_usd";
  emit({
    type: "result",
    subtype,
    is_error: subtype !== "success",
    terminal_reason: budget ? "budget_exhausted" : subtype === "success" ? "completed" : "error",
    total_cost_usd: outcome.costUsd ?? 0.01,
    num_turns: 1,
    result: subtype === "success" ? outcome.text ?? "" : undefined,
    errors: budget ? [`Reached maximum budget ($${flag("--max-budget-usd")})`] : subtype === "success" ? undefined : ["fake error"],
    permission_denials: [],
  });
}
process.exit(outcome.exitCode ?? (outcome.subtype && outcome.subtype !== "success" ? 1 : 0));
