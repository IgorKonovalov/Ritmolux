// One headless session: spawn `claude -p` in a worktree with the conductor's settings, budget and
// appended prompt, keep its stream transcript under state/, and reduce the run to one result.
//
// The flags and the result-event fields used here are the ones tools/conductor/spike/README.md
// observed on the verified CLI. A session is `ok` only when it ended cleanly AND printed a
// well-formed outcome that is not itself a park; every other ending is `parked` with a reason:
//   cli_contract    a shell call with no hook log line, or an init not listing the invoked skill
//   budget          the result event's subtype is error_max_budget_usd
//   api             no result event, or an error result that is not the budget
//   lost_background a background command started and still unfinished at the result
//   no_outcome      a clean result with no rlx-outcome block
//   bad_outcome     an rlx-outcome block that fails validation, or names another plan
//   <session's own> the outcome is kind "parked"

import { spawn, spawnSync } from "node:child_process";
import { appendFileSync, mkdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { dirname } from "node:path";
import { StringDecoder } from "node:string_decoder";

import { CLI_CONTRACT, LOST_BACKGROUND, parseOutcome, readResult } from "./outcome.mjs";

/** Sessions currently running in this process, so an interrupt can end them rather than orphan them. */
export const activeChildren = new Set();

/**
 * Ends a session and everything it started. `child.kill()` alone ends only the CLI process: the
 * cargo / nextest it spawned, and a with-lock wrapper holding the suite lock, would live on — and a
 * lock whose holder is alive is never taken over. On Windows `taskkill /T` walks the tree; elsewhere
 * the session is spawned as its own process group and the whole group is signalled.
 */
export function killTree(child) {
  if (!child.pid) return;
  if (process.platform === "win32") {
    spawnSync("taskkill", ["/PID", String(child.pid), "/T", "/F"], { stdio: "ignore" });
    return;
  }
  try {
    process.kill(-child.pid, "SIGKILL");
  } catch {
    child.kill("SIGKILL");
  }
}

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
 * Splits a byte stream into lines and hands each JSON object on one to `onEvent`. A partial line
 * waits for the next chunk; a line that is not JSON is dropped; a throwing `onEvent` is ignored,
 * because nothing a display does may end a session.
 */
export function lineReader(onEvent) {
  const decoder = new StringDecoder("utf8");
  let pending = "";
  const deliver = (line) => {
    if (!line.trim()) return;
    let e;
    try {
      e = JSON.parse(line);
    } catch {
      return;
    }
    if (!e || typeof e !== "object") return;
    try {
      onEvent(e);
    } catch {}
  };
  return {
    push(chunk) {
      pending += decoder.write(chunk);
      const parts = pending.split("\n");
      pending = parts.pop();
      for (const line of parts) deliver(line);
    },
    end() {
      pending += decoder.end();
      deliver(pending);
      pending = "";
    },
  };
}

/**
 * Why a finished session breaks the headless contract, or null (ADR-0208). With `hookLog` given, a
 * transcript holding a Bash or PowerShell tool use needs that file non-empty: the project hooks ran.
 * With `skill` given, system/init must list it: the skill the prompt invokes was loaded.
 */
export function contractProblem(r, { skill, hookLog }) {
  if (hookLog && r.shellCalls > 0) {
    let size = 0;
    try {
      size = statSync(hookLog).size;
    } catch {}
    if (size === 0) return `the session made ${r.shellCalls} shell call(s) and the project hooks wrote nothing to ${hookLog}: the CLI may not be running them`;
  }
  if (skill && !(r.init?.skills ?? []).includes(skill)) {
    return `system/init does not list the skill ${skill} the prompt invokes${r.init?.skills ? "" : " (no skills list)"}: the CLI may not be loading project skills`;
  }
  return null;
}

/**
 * Runs one step. `claude` is the command vector (default ["claude"]); tests pass
 * [process.execPath, "fake-claude.mjs"]. `onStreamEvent(event)`, when given, receives every
 * stream-json event as it arrives. `skill` and `hookLog` switch on the CLI contract check
 * (contractProblem), which parks `cli_contract` before any outcome is read. Resolves to:
 *   { status: "ok"|"parked", reason?, detail?, outcome?, spendUsd, sessionId, exitCode,
 *     subtype, terminalReason, transcript, rateLimit, rateLimitFirst, numTurns }
 */
export function runStep(opts) {
  const {
    claude = ["claude"],
    cwd,
    transcriptPath,
    env = {},
    timeoutMs = 6 * 60 * 60 * 1000,
    expectPlan,
    onStreamEvent,
    skill,
    hookLog,
  } = opts;
  const [bin, ...pre] = claude;
  const args = [...pre, ...claudeArgs(opts)];
  mkdirSync(dirname(transcriptPath), { recursive: true });
  writeFileSync(transcriptPath, "");

  return new Promise((resolveStep) => {
    const child = spawn(bin, args, {
      cwd,
      env: { ...process.env, ...env, ...(hookLog ? { RLX_HOOK_LOG: hookLog } : {}), RLX_CONDUCTOR: "1" },
      stdio: ["ignore", "pipe", "pipe"],
      // Its own process group, so killTree can signal the session and its descendants together.
      // Not on Windows, where `detached` opens a console window and taskkill walks the tree anyway.
      detached: process.platform !== "win32",
    });
    activeChildren.add(child);
    let stderr = "";
    let rateLimitFirst = null;
    const lines = lineReader((e) => {
      if (e.type === "rate_limit_event" && rateLimitFirst === null) rateLimitFirst = e.rate_limit_info ?? null;
      onStreamEvent?.(e);
    });
    child.stdout.on("data", (d) => {
      appendFileSync(transcriptPath, d);
      lines.push(d);
    });
    child.stderr.on("data", (d) => {
      stderr += d;
      if (stderr.length > 20_000) stderr = stderr.slice(-20_000);
    });
    let timedOut = false;
    const timer = setTimeout(() => {
      timedOut = true;
      killTree(child);
    }, timeoutMs);
    let settled = false;
    const settle = (exitCode, spawnError) => {
      if (settled) return;
      settled = true;
      activeChildren.delete(child);
      clearTimeout(timer);
      lines.end();
      const r = readResult(readFileSync(transcriptPath, "utf8"));
      const base = {
        exitCode,
        spendUsd: r.spendUsd ?? 0,
        sessionId: r.sessionId ?? null,
        subtype: r.subtype ?? null,
        terminalReason: r.terminalReason ?? null,
        transcript: transcriptPath,
        rateLimit: r.rateLimit ?? null,
        rateLimitFirst,
        numTurns: r.numTurns ?? null,
      };
      const park = (reason, detail) => resolveStep({ ...base, status: "parked", reason, detail });

      if (spawnError) return park("api", `could not start claude: ${spawnError.message}`);
      // The CLI contract comes before anything the session claims (ADR-0208). A session that never
      // started (no init and no result) has nothing to check and falls through to the api park.
      if (r.init || r.present) {
        const broken = contractProblem(r, { skill, hookLog });
        if (broken) return park(CLI_CONTRACT, broken);
      }
      if (timedOut) return park("api", `session exceeded ${Math.round(timeoutMs / 60000)} min and was killed`);
      if (!r.present) return park("api", `session ended with no result event (exit ${exitCode}): ${stderr.trim().slice(-400)}`);
      if (r.subtype === "error_max_budget_usd") {
        return park("budget", `spend cap hit: ${r.errors.join("; ") || "error_max_budget_usd"}`);
      }
      if (r.isError) return park("api", `session ended in error (${r.subtype}): ${r.errors.join("; ")}`);
      // Before any outcome is read: a session that backgrounded a command and reached its result
      // without that command finishing lost the work, whatever it went on to claim.
      if (r.backgroundOutstanding.length > 0) {
        const named = r.backgroundOutstanding.map((b) => `"${b.command}"`).join(", ");
        return park(
          LOST_BACKGROUND,
          `the session started ${r.backgroundOutstanding.length} command(s) in the background and ended ` +
            `with them unfinished, so the work was killed with the session: ${named}`,
        );
      }
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
