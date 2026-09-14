// One headless session: spawn `claude -p` in a worktree with the conductor's settings, budget and
// appended prompt, keep its stream transcript under state/, and reduce the run to one result.
//
// The flags and the result-event fields used here are the ones tools/conductor/spike/README.md
// observed on the verified CLI. A session is `ok` only when it ended cleanly AND printed a
// well-formed outcome that is not itself a park; every other ending is `parked` with a reason:
//   budget          the result event's subtype is error_max_budget_usd
//   api             no result event, or an error result that is not the budget
//   no_outcome      a clean result with no rlx-outcome block
//   bad_outcome     an rlx-outcome block that fails validation, or names another plan
//   <session's own> the outcome is kind "parked"

import { spawn } from "node:child_process";
import { appendFileSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname } from "node:path";

import { parseOutcome, readResult } from "./outcome.mjs";

export function renderPrompt(template, vars) {
  const text = template.replace(/\{\{(\w+)\}\}/g, (m, k) => {
    if (!(k in vars)) throw new Error(`prompt template variable {{${k}}} has no value`);
    return String(vars[k]);
  });
  return text;
}

export function renderPromptFile(templatePath, vars, outPath) {
  const text = renderPrompt(readFileSync(templatePath, "utf8"), vars);
  mkdirSync(dirname(outPath), { recursive: true });
  writeFileSync(outPath, text);
  return outPath;
}

export function claudeArgs({ prompt, settingsFile, appendPromptFile, budgetUsd, model, addDirs = [] }) {
  const args = [
    "-p",
    prompt,
    "--output-format",
    "stream-json",
    "--verbose",
    "--permission-mode",
    "dontAsk",
    "--settings",
    settingsFile,
    "--append-system-prompt-file",
    appendPromptFile,
    "--max-budget-usd",
    String(budgetUsd),
  ];
  if (model) args.push("--model", model);
  for (const d of addDirs) args.push("--add-dir", d);
  return args;
}

/**
 * Runs one step. `claude` is the command vector (default ["claude"]); tests pass
 * [process.execPath, "fake-claude.mjs"]. Resolves to:
 *   { status: "ok"|"parked", reason?, detail?, outcome?, spendUsd, sessionId, exitCode,
 *     subtype, terminalReason, transcript, rateLimit }
 */
export function runStep(opts) {
  const {
    claude = ["claude"],
    cwd,
    transcriptPath,
    env = {},
    timeoutMs = 6 * 60 * 60 * 1000,
    expectPlan,
  } = opts;
  const [bin, ...pre] = claude;
  const args = [...pre, ...claudeArgs(opts)];
  mkdirSync(dirname(transcriptPath), { recursive: true });
  writeFileSync(transcriptPath, "");

  return new Promise((resolveStep) => {
    const child = spawn(bin, args, {
      cwd,
      env: { ...process.env, ...env, RLX_CONDUCTOR: "1" },
      stdio: ["ignore", "pipe", "pipe"],
    });
    let stderr = "";
    child.stdout.on("data", (d) => appendFileSync(transcriptPath, d));
    child.stderr.on("data", (d) => {
      stderr += d;
      if (stderr.length > 20_000) stderr = stderr.slice(-20_000);
    });
    let timedOut = false;
    const timer = setTimeout(() => {
      timedOut = true;
      child.kill();
    }, timeoutMs);
    let settled = false;
    const settle = (exitCode, spawnError) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      const r = readResult(readFileSync(transcriptPath, "utf8"));
      const base = {
        exitCode,
        spendUsd: r.spendUsd ?? 0,
        sessionId: r.sessionId ?? null,
        subtype: r.subtype ?? null,
        terminalReason: r.terminalReason ?? null,
        transcript: transcriptPath,
        rateLimit: r.rateLimit ?? null,
      };
      const park = (reason, detail) => resolveStep({ ...base, status: "parked", reason, detail });

      if (spawnError) return park("api", `could not start claude: ${spawnError.message}`);
      if (timedOut) return park("api", `session exceeded ${Math.round(timeoutMs / 60000)} min and was killed`);
      if (!r.present) return park("api", `session ended with no result event (exit ${exitCode}): ${stderr.trim().slice(-400)}`);
      if (r.subtype === "error_max_budget_usd") {
        return park("budget", `spend cap hit: ${r.errors.join("; ") || "error_max_budget_usd"}`);
      }
      if (r.isError) return park("api", `session ended in error (${r.subtype}): ${r.errors.join("; ")}`);
      const parsed = parseOutcome(r.text);
      if (!parsed.ok) {
        return park(parsed.error === "no rlx-outcome block" ? "no_outcome" : "bad_outcome", parsed.error);
      }
      const o = parsed.outcome;
      if (expectPlan && o.plan !== expectPlan) {
        return park("bad_outcome", `outcome names plan ${o.plan}, the step was for plan ${expectPlan}`);
      }
      if (o.kind === "parked") return resolveStep({ ...base, status: "parked", reason: o.reason, detail: o.detail, outcome: o });
      resolveStep({ ...base, status: "ok", outcome: o });
    };
    child.on("error", (e) => settle(null, e));
    child.on("close", (code) => settle(code));
  });
}
