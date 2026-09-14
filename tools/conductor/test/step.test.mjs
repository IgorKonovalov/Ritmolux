// One step against the fake CLI: what it is spawned with, where its transcript goes, and how every
// ending that is not a clean, well-formed outcome becomes a park.

import assert from "node:assert/strict";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";

import { parseOutcome } from "../lib/outcome.mjs";
import { renderPrompt, runStep } from "../lib/step.mjs";
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

test("a prompt template refuses an unfilled variable", () => {
  assert.equal(renderPrompt("plan {{plan}}", { plan: "0101" }), "plan 0101");
  assert.throws(() => renderPrompt("plan {{plan}} {{phases}}", { plan: "0101" }), /\{\{phases\}\} has no value/);
});
