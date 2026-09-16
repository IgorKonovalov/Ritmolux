// One step against the fake CLI: what it is spawned with, where its transcript goes, and how every
// ending that is not a clean, well-formed outcome becomes a park.

import assert from "node:assert/strict";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";

import { parseOutcome } from "../lib/outcome.mjs";
import { activeChildren, renderPrompt, runStep } from "../lib/step.mjs";
import { pidAlive } from "../with-lock.mjs";
import { FAKE, outcomeBlock, tmp } from "./helpers.mjs";

function scenario(dir, body) {
  const path = join(dir, `scenario-${Math.random().toString(16).slice(2)}.mjs`);
  writeFileSync(path, `export default async (ctx) => (${body});\n`);
  return path;
}

async function step(result, extra = {}) {
  const dir = tmp();
  const worktree = tmp("rlx-worktree-");
  const log = join(dir, "calls.jsonl");
  const append = join(dir, "append.md");
  writeFileSync(append, "RLX-CONDUCTOR-MODE: implement\nRLX-CONDUCTOR-PLAN: 0101\n");
  process.env.FAKE_CLAUDE_LOG = log;
  process.env.FAKE_CLAUDE_SCENARIO = scenario(dir, JSON.stringify(result));
  const r = await runStep({
    claude: FAKE,
    cwd: worktree,
    prompt: "/dev conductor implement plan 0101 phases 1-2",
    settingsFile: join(dir, "settings.conductor.json"),
    appendPromptFile: append,
    budgetUsd: 4.5,
    transcriptPath: join(dir, "state", "transcripts", "0101-1-implement.jsonl"),
    env: { RLX_LOCK_LOG: join(dir, "locks.jsonl") },
    expectPlan: "0101",
    ...extra,
  });
  delete process.env.FAKE_CLAUDE_SCENARIO;
  delete process.env.FAKE_CLAUDE_LOG;
  const calls = existsSync(log) ? readFileSync(log, "utf8").trim().split("\n").map((l) => JSON.parse(l)) : [];
  return { r, calls, dir, worktree };
}

const DONE = { kind: "phases_done", plan: "0101", through: "2", commits: ["abc1234", "def5678"] };

test("a step spawns the CLI in the worktree with the conductor flags, and returns the outcome", async () => {
  const { r, calls, dir, worktree } = await step({ text: outcomeBlock(DONE), costUsd: 1.25 });
  assert.equal(r.status, "ok");
  assert.deepEqual(r.outcome, DONE);
  assert.equal(r.spendUsd, 1.25);
  assert.equal(calls.length, 1);
  const [call] = calls;
  assert.equal(call.cwd.toLowerCase(), worktree.toLowerCase());
  assert.equal(call.env.RLX_CONDUCTOR, "1");
  assert.equal(call.env.RLX_LOCK_LOG, join(dir, "locks.jsonl"));
  const arg = (name) => call.args[call.args.indexOf(name) + 1];
  assert.equal(arg("-p"), "/dev conductor implement plan 0101 phases 1-2");
  assert.equal(arg("--settings"), join(dir, "settings.conductor.json"));
  assert.equal(arg("--max-budget-usd"), "4.5");
  assert.equal(arg("--permission-mode"), "dontAsk");
  assert.equal(arg("--output-format"), "stream-json");
  assert.ok(call.args.includes("--verbose"));
  assert.equal(call.vars.mode, "implement");
  // The transcript is kept under state/ and holds the stream the fake emitted.
  const transcript = readFileSync(r.transcript, "utf8");
  assert.equal(r.transcript, join(dir, "state", "transcripts", "0101-1-implement.jsonl"));
  assert.match(transcript, /"type":"result"/);
});

test("a session that times out is killed with everything it started", async () => {
  const dir = tmp();
  const pidFile = join(dir, "grandchild.pid");
  // A session that starts a long-lived child of its own (the cargo / nextest a real one runs), then hangs.
  const scenarioPath = join(dir, "hang.mjs");
  const grandchildCode = `require("fs").writeFileSync(${JSON.stringify(pidFile)}, String(process.pid)); setInterval(() => {}, 1000);`;
  writeFileSync(
    scenarioPath,
    `import { spawn } from "node:child_process";
     export default async () => {
       // On Windows a Node parent keeps its children in a job object that dies with it, which a
       // native CLI's cargo does not get; detaching escapes that job, so only a tree kill ends it.
       spawn(process.execPath, ["-e", ${JSON.stringify(grandchildCode)}], { stdio: "ignore", detached: process.platform === "win32" });
       await new Promise(() => {});
     };\n`,
  );
  process.env.FAKE_CLAUDE_SCENARIO = scenarioPath;
  const append = join(dir, "append.md");
  writeFileSync(append, "RLX-CONDUCTOR-MODE: implement\n");
  const running = runStep({
    claude: FAKE,
    cwd: tmp("rlx-worktree-"),
    prompt: "/dev conductor implement plan 0101 phases 1",
    settingsFile: join(dir, "settings.json"),
    appendPromptFile: append,
    budgetUsd: 1,
    transcriptPath: join(dir, "t.jsonl"),
    timeoutMs: 10_000,
  });
  const until = async (cond, ms) => {
    const end = Date.now() + ms;
    while (!cond() && Date.now() < end) await new Promise((r) => setTimeout(r, 50));
    return cond();
  };
  assert.ok(await until(() => existsSync(pidFile) && readFileSync(pidFile, "utf8").length > 0, 9_000), "the grandchild started");
  const grandchild = Number(readFileSync(pidFile, "utf8"));
  assert.ok(pidAlive(grandchild));
  const r = await running;
  delete process.env.FAKE_CLAUDE_SCENARIO;
  assert.equal(r.status, "parked");
  assert.equal(r.reason, "api");
  assert.match(r.detail, /exceeded .* and was killed/);
  assert.equal(activeChildren.size, 0);
  assert.ok(await until(() => !pidAlive(grandchild), 10_000), `grandchild ${grandchild} outlived its session`);
});

test("a clean session with no outcome block parks, never passes", async () => {
  const { r } = await step({ text: "All phases implemented." });
  assert.equal(r.status, "parked");
  assert.equal(r.reason, "no_outcome");
});

test("a malformed outcome block parks", async () => {
  const bad = await step({ text: "```rlx-outcome\n{kind: phases_done}\n```" });
  assert.equal(bad.r.status, "parked");
  assert.equal(bad.r.reason, "bad_outcome");
  assert.match(bad.r.detail, /not JSON/);

  const noCommits = await step({ text: outcomeBlock({ ...DONE, commits: [] }) });
  assert.equal(noCommits.r.reason, "bad_outcome");

  const otherPlan = await step({ text: outcomeBlock({ ...DONE, plan: "0102" }) });
  assert.equal(otherPlan.r.reason, "bad_outcome");
  assert.match(otherPlan.r.detail, /names plan 0102/);
});

test("a session's own park is carried through with its reason", async () => {
  const { r } = await step({
    text: outcomeBlock({ kind: "parked", plan: "0101", phase: "2", reason: "stop_condition", detail: "Phase 2's stop condition: golden moved" }),
  });
  assert.equal(r.status, "parked");
  assert.equal(r.reason, "stop_condition");
  assert.equal(r.detail, "Phase 2's stop condition: golden moved");
});

test("a budget-exhausted session parks as budget with its spend", async () => {
  const { r } = await step({ subtype: "error_max_budget_usd", costUsd: 4.61, text: "" });
  assert.equal(r.status, "parked");
  assert.equal(r.reason, "budget");
  assert.equal(r.spendUsd, 4.61);
  assert.equal(r.exitCode, 1);
  assert.equal(r.terminalReason, "budget_exhausted");
});

test("a session that ends with no result event parks as an API failure", async () => {
  const { r } = await step({ noResult: true, exitCode: 1 });
  assert.equal(r.status, "parked");
  assert.equal(r.reason, "api");
});

test("the last outcome block wins, and verdict counts must agree with the findings", () => {
  const text = outcomeBlock({ ...DONE, through: "1" }) + outcomeBlock(DONE);
  assert.equal(parseOutcome(text).outcome.through, "2");
  const verdict = {
    kind: "verdict",
    plan: "0101",
    round: 1,
    blockers: 0,
    majors: 1,
    minors: 0,
    review_path: "state/reviews/0101-round-1.md",
    findings: [{ severity: "minor", file: "a.rs", line: 1, what: "x" }],
  };
  assert.match(parseOutcome(outcomeBlock(verdict)).error, /counts disagree/);
  verdict.findings[0].severity = "major";
  assert.equal(parseOutcome(outcomeBlock(verdict)).ok, true);
  const closedWithMajor = { kind: "closed", plan: "0101", version: "1.2.0", tag: "v1.2.0", verdict };
  assert.match(parseOutcome(outcomeBlock(closedWithMajor)).error, /closed carries blockers or majors/);
});

const SHELL_CALL = { type: "assistant", message: { content: [{ type: "tool_use", id: "toolu_s1", name: "Bash", input: { command: "git status" } }] } };

test("a session that makes a shell call and writes no hook log parks cli_contract before its outcome is read", async () => {
  const dir = tmp();
  const hookLog = join(dir, "hooks", "0101-01-implement.log");
  const { r } = await step({ text: outcomeBlock(DONE), stream: [SHELL_CALL], hooks: false }, { hookLog, skill: "dev" });
  assert.equal(r.status, "parked");
  assert.equal(r.reason, "cli_contract");
  assert.match(r.detail, /made 1 shell call\(s\) and the project hooks wrote nothing/);
  assert.equal(r.outcome, undefined, "no outcome was accepted");
});

test("a session whose init lists no invoked skill parks cli_contract", async () => {
  const { r } = await step({ text: outcomeBlock(DONE), skills: ["architect", "preset-author"] }, { hookLog: join(tmp(), "h.log"), skill: "dev" });
  assert.equal(r.status, "parked");
  assert.equal(r.reason, "cli_contract");
  assert.match(r.detail, /system\/init does not list the skill dev/);

  const none = await step({ text: outcomeBlock(DONE), skills: null }, { skill: "dev" });
  assert.equal(none.r.reason, "cli_contract");
  assert.match(none.r.detail, /no skills list/);
});

test("a session that makes no shell call is not parked for the hook log, and one whose hooks ran passes", async () => {
  const quiet = await step({ text: outcomeBlock(DONE) }, { hookLog: join(tmp(), "never-written.log"), skill: "dev" });
  assert.equal(quiet.r.status, "ok", quiet.r.detail);

  const hookLog = join(tmp(), "ran.log");
  const ran = await step({ text: outcomeBlock(DONE), stream: [SHELL_CALL] }, { hookLog, skill: "dev" });
  assert.equal(ran.r.status, "ok", ran.r.detail);
  assert.equal(ran.calls[0].env.RLX_HOOK_LOG, hookLog, "the session is handed the step's hook log");
  assert.equal(readFileSync(hookLog, "utf8").trim().split("\n").length, 1);
});

// A session that backgrounds a command and ends its turn loses that work: the process exits and the
// task is killed. The park must name it rather than read as the generic `no_outcome`.

const bgUse = (id, command) => ({
  type: "assistant",
  message: { content: [{ type: "tool_use", id, name: "Bash", input: { command, run_in_background: true } }] },
});
const bgStarted = (id) => ({
  type: "user",
  message: { content: [{ type: "tool_result", tool_use_id: id, content: `Command running in background with ID: ${id}` }] },
});
const notified = (id) => ({ type: "system", subtype: "task_notification", tool_use_id: id, status: "completed", summary: "Bash (exit code 0)" });

test("a session holding an unfinished background command parks lost_background, whatever it claims", async () => {
  const { r } = await step({
    text: outcomeBlock(DONE),
    stream: [bgUse("toolu_bg1", "cargo nextest run --workspace"), bgStarted("toolu_bg1")],
  });
  assert.equal(r.status, "parked");
  assert.equal(r.reason, "lost_background");
  assert.match(r.detail, /cargo nextest run --workspace/);
  assert.equal(r.outcome, undefined, "no outcome was accepted");
});

test("a background command whose completion notification arrived is not a lost one", async () => {
  const { r } = await step({
    text: outcomeBlock(DONE),
    stream: [bgUse("toolu_bg2", "cargo nextest run --workspace"), bgStarted("toolu_bg2"), notified("toolu_bg2")],
  });
  assert.equal(r.status, "ok", r.detail);
  assert.deepEqual(r.outcome, DONE);
});

test("a session that starts nothing in the background is untouched, and a refused start is not a lost one", async () => {
  const none = await step({ text: outcomeBlock(DONE) });
  assert.equal(none.r.status, "ok");
  assert.deepEqual(none.r.outcome, DONE);

  // What the hook produces: the call is denied, so no command ever ran.
  const denied = await step({
    text: outcomeBlock(DONE),
    stream: [
      bgUse("toolu_bg3", "cargo nextest run --workspace"),
      { type: "system", subtype: "permission_denied", tool_use_id: "toolu_bg3", tool_name: "Bash" },
    ],
  });
  assert.equal(denied.r.status, "ok", denied.r.detail);

  // And the same call refused as an error result rather than a permission event.
  const errored = await step({
    text: outcomeBlock(DONE),
    stream: [
      bgUse("toolu_bg4", "cargo nextest run --workspace"),
      { type: "user", message: { content: [{ type: "tool_result", tool_use_id: "toolu_bg4", is_error: true, content: "Permission denied" }] } },
    ],
  });
  assert.equal(errored.r.status, "ok", errored.r.detail);
});

test("the CLI's other background result shape is read as a start too", async () => {
  const { r } = await step({
    text: outcomeBlock(DONE),
    stream: [
      { type: "assistant", message: { content: [{ type: "tool_use", id: "toolu_bg5", name: "Bash", input: { command: "cargo doc" } }] } },
      { type: "user", message: { content: [{ type: "tool_result", tool_use_id: "toolu_bg5", content: "Command moved to the background (ID: toolu_bg5)" }] } },
    ],
  });
  assert.equal(r.status, "parked");
  assert.equal(r.reason, "lost_background");
  assert.match(r.detail, /cargo doc/);
});

test("a prompt template refuses an unfilled variable", () => {
  assert.equal(renderPrompt("plan {{plan}}", { plan: "0101" }), "plan 0101");
  assert.throws(() => renderPrompt("plan {{plan}} {{phases}}", { plan: "0101" }), /\{\{phases\}\} has no value/);
});
