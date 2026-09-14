// The lane loop end to end, against a throwaway git repository and a fake CLI whose sessions make
// real commits (lane-scenario.mjs). Each test builds its own repository, so the scenarios share
// nothing but the code under test.

import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { appendFileSync, existsSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";

import { git, resolveCommit, tagObjectType } from "../lib/git.mjs";
import { runLanes } from "../lib/lane.mjs";
import { findPlan, readPlanFile } from "../lib/plan.mjs";
import { validateQueue } from "../lib/queue.mjs";
import { loadState, statePaths } from "../lib/state.mjs";
import { FAKE, TEST_DIR, TOOL_DIR, tmp, writePlan } from "./helpers.mjs";

const SCENARIO = join(TEST_DIR, "lane-scenario.mjs");

function sh(args, cwd) {
  const r = spawnSync("git", args, { cwd, encoding: "utf8" });
  assert.equal(r.status, 0, `git ${args.join(" ")}: ${r.stderr}`);
  return r.stdout.trim();
}

export function scratch({ plans, lanes, after = {}, spec = {}, local = {} }) {
  const repo = tmp("rlx-lane-repo-");
  sh(["init", "-q", "-b", "main"], repo);
  sh(["config", "user.email", "conductor-test@example.invalid"], repo);
  sh(["config", "user.name", "Conductor Test"], repo);
  sh(["config", "commit.gpgsign", "false"], repo);
  sh(["config", "tag.gpgSign", "false"], repo);
  sh(["config", "core.autocrlf", "false"], repo);
  writeFileSync(join(repo, "VERSION"), "0.1.0\n");
  writeFileSync(join(repo, "README.md"), "scratch\n");
  for (const p of plans) writePlan(repo, p);
  sh(["add", "VERSION", "README.md", "docs"], repo);
  sh(["commit", "-q", "-m", "init"], repo);
  sh(["tag", "-a", "v0.1.0", "-m", "init"], repo);

  const stateDir = tmp("rlx-state-");
  const events = join(stateDir, "events.jsonl");
  const specFile = join(stateDir, "spec.json");
  writeFileSync(specFile, JSON.stringify({ plans: spec }));
  writeFileSync(events, "");
  process.env.FAKE_CLAUDE_SCENARIO = SCENARIO;
  process.env.FAKE_LANE_SPEC = specFile;
  process.env.FAKE_EVENTS = events;

  const queue = validateQueue({ lanes, plans: Object.fromEntries(Object.entries(after).map(([k, v]) => [k, { after: v }])) }, repo);
  assert.deepEqual(queue.errors, []);
  const ctx = {
    repo,
    worktreeRoot: tmp("rlx-lanes-"),
    stateDir,
    promptsDir: join(TOOL_DIR, "prompts"),
    settingsFile: join(TOOL_DIR, "settings.conductor.json"),
    withLockPath: join(TOOL_DIR, "with-lock.mjs"),
    claude: FAKE,
    local: { budget_usd: { implement: 5, fix: 3, review: 4 }, max_open_worktrees: 3, ...local },
    queue,
    state: loadState(stateDir),
    gate: [{ name: "marker", cmd: [process.execPath, "-e", "process.exit(require('fs').existsSync('GATE_RED')?1:0)"], lock: "suite" }],
    lockDir: tmp("rlx-locks-"),
    lockPollMs: 20,
    pollMs: 50,
    events: (name, data) => appendFileSync(events, JSON.stringify({ t: Date.now(), plan: data.plan, event: `conductor-${name}` }) + "\n"),
  };
  const readEvents = () =>
    readFileSync(events, "utf8")
      .trim()
      .split("\n")
      .filter(Boolean)
      .map((l) => JSON.parse(l));
  return { ctx, repo, readEvents };
}

const dev = (id) => ({ id, owner: "dev" });
const studio = (id) => ({ id, owner: "studio-builder" });
const human = (id) => ({ id, owner: "human" });
const kinds = (rec) => rec.steps.map((s) => (s.owner ? `${s.kind}:${s.owner}` : s.kind));

test("a dev run then a studio-builder run with a clean review merges, tagged, lane removed", async () => {
  const { ctx, repo } = scratch({ plans: [{ number: "0101", phases: [dev("1"), dev("2"), studio("3")] }], lanes: { a: ["0101"] } });
  const worktreeBefore = join(ctx.worktreeRoot, "rlx-plan-0101");
  await runLanes(ctx);
  const rec = loadState(ctx.stateDir).plans["0101"];

  assert.equal(rec.status, "merged", JSON.stringify(rec.park));
  assert.deepEqual(kinds(rec), ["implement:dev", "implement:studio-builder", "review:architect"]);
  assert.deepEqual(rec.steps[0].phases, ["1", "2"]);
  assert.equal(rec.fixRounds, 0);
  assert.equal(rec.closed.tag, "v0.1.1");
  assert.equal(tagObjectType("v0.1.1", repo), "tag");
  assert.equal(resolveCommit("v0.1.1", repo), resolveCommit("main", repo), "the tag sits on main's tip");
  assert.equal(existsSync(worktreeBefore), false);
  assert.equal(git(["branch", "--list", "plan-0101-*"], repo).stdout, "");
  const closedPlan = findPlan(repo, "0101");
  assert.ok(closedPlan.done);
  assert.ok(readPlanFile(closedPlan.path).hasCloseReview);
  assert.ok(existsSync(join(repo, "phase-0101-3.txt")));
});

test("a closed outcome whose plan has no ## Close review parks as a disagreement", async () => {
  const { ctx, repo } = scratch({
    plans: [{ number: "0101", phases: [dev("1")] }],
    lanes: { a: ["0101"] },
    spec: { "0101": { noCloseReview: true } },
  });
  const mainBefore = resolveCommit("main", repo);
  await runLanes(ctx);
  const rec = loadState(ctx.stateDir).plans["0101"];
  assert.equal(rec.status, "parked");
  assert.equal(rec.park.reason, "disagreement");
  assert.match(rec.park.detail, /no ## Close review section/);
  assert.equal(resolveCommit("main", repo), mainBefore, "main did not move");
});

test("a human phase parks with its worktree kept, the lane runs on, and a dependant is held", async () => {
  const { ctx, repo } = scratch({
    plans: [
      { number: "0101", phases: [dev("1"), human("2"), dev("3")] },
      { number: "0102", phases: [dev("1")] },
      { number: "0103", phases: [dev("1")] },
    ],
    lanes: { a: ["0101", "0102", "0103"] },
    after: { "0103": ["0101"] },
  });
  await runLanes(ctx);
  const state = loadState(ctx.stateDir);
  const parked = state.plans["0101"];
  assert.equal(parked.status, "parked");
  assert.equal(parked.park.reason, "human_phase");
  assert.equal(parked.park.phase, "2");
  assert.ok(existsSync(parked.worktree), "the parked plan keeps its worktree");
  assert.deepEqual(kinds(parked), ["implement:dev"]);
  assert.equal(state.plans["0102"].status, "merged");
  assert.equal(state.plans["0103"], undefined, "the dependant never started");
  assert.ok(findPlan(repo, "0103") && !findPlan(repo, "0103").done);

  const inbox = readFileSync(statePaths(ctx.stateDir).inbox, "utf8");
  assert.match(inbox, /plan 0101 parked: human_phase/);
  assert.match(inbox, /Resume:\*\* `node tools\/conductor\/conductor\.mjs resume 0101`/);
});

test("a review with one major takes exactly one fix round and a re-review, then merges", async () => {
  const { ctx } = scratch({
    plans: [{ number: "0101", phases: [dev("1")] }],
    lanes: { a: ["0101"] },
    spec: { "0101": { reviews: ["major", "clean"] } },
  });
  await runLanes(ctx);
  const rec = loadState(ctx.stateDir).plans["0101"];
  assert.equal(rec.status, "merged", JSON.stringify(rec.park));
  assert.deepEqual(kinds(rec), ["implement:dev", "review:architect", "fix:dev", "review:architect"]);
  assert.equal(rec.fixRounds, 1);
  assert.equal(rec.verdicts.length, 2);
  assert.equal(rec.verdicts[0].majors, 1);
  assert.equal(rec.fixes[0].resolved[0].finding, 0);
  assert.deepEqual(rec.gates.map((g) => g.label), ["pre-review", "fix-1"]);
});

test("a review still carrying a blocker after two fix rounds parks", async () => {
  const { ctx } = scratch({
    plans: [{ number: "0101", phases: [dev("1")] }],
    lanes: { a: ["0101"] },
    spec: { "0101": { reviews: ["blocker", "blocker", "blocker"] } },
  });
  await runLanes(ctx);
  const rec = loadState(ctx.stateDir).plans["0101"];
  assert.equal(rec.status, "parked");
  assert.equal(rec.park.reason, "review_failed");
  assert.deepEqual(kinds(rec), ["implement:dev", "review:architect", "fix:dev", "review:architect", "fix:dev", "review:architect"]);
  assert.equal(rec.fixRounds, 2);
});

test("an outcome claiming a commit git does not have parks as a disagreement", async () => {
  const { ctx } = scratch({
    plans: [{ number: "0101", phases: [dev("1"), dev("2")] }],
    lanes: { a: ["0101"] },
    spec: { "0101": { bogusCommit: true } },
  });
  await runLanes(ctx);
  const rec = loadState(ctx.stateDir).plans["0101"];
  assert.equal(rec.status, "parked");
  assert.equal(rec.park.reason, "disagreement");
  assert.match(rec.park.detail, /claimed commit deadbee does not exist/);
  assert.deepEqual(kinds(rec), ["implement:dev"], "no review started");
});

test("a lightweight tag on a closed plan parks rather than merging", async () => {
  const { ctx, repo } = scratch({
    plans: [{ number: "0101", phases: [dev("1")] }],
    lanes: { a: ["0101"] },
    spec: { "0101": { lightweightTag: true } },
  });
  const mainBefore = resolveCommit("main", repo);
  await runLanes(ctx);
  const rec = loadState(ctx.stateDir).plans["0101"];
  assert.equal(rec.status, "parked");
  assert.equal(rec.park.reason, "disagreement");
  assert.match(rec.park.detail, /tag v0\.1\.1 is lightweight, not annotated/);
  assert.equal(resolveCommit("main", repo), mainBefore);
});

test("main advancing on a disjoint file between close and merge re-merges once and merges", async () => {
  const { ctx, repo } = scratch({ plans: [{ number: "0101", phases: [dev("1")] }], lanes: { a: ["0101"] } });
  ctx.beforeMerge = async () => {
    writeFileSync(join(repo, "owner-note.txt"), "landed while the close ran\n");
    sh(["add", "owner-note.txt"], repo);
    sh(["commit", "-q", "-m", "docs: an unrelated commit on main"], repo);
  };
  await runLanes(ctx);
  const rec = loadState(ctx.stateDir).plans["0101"];
  assert.equal(rec.status, "merged", JSON.stringify(rec.park));
  assert.equal(rec.merge.remerged, true);
  assert.equal(tagObjectType("v0.1.1", repo), "tag", "the moved tag is still annotated");
  assert.equal(resolveCommit("v0.1.1", repo), resolveCommit("main", repo), "the tag moved onto the new tip");
  assert.equal(git(["tag", "-l", "--format=%(contents)", "v0.1.1"], repo).stdout, "chore: Release v0.1.1", "its message was kept");
  assert.ok(existsSync(join(repo, "owner-note.txt")));
  assert.deepEqual(rec.gates.map((g) => g.label), ["pre-review", "remerge"]);
});

test("main advancing on a conflicting change between close and merge parks", async () => {
  const { ctx, repo } = scratch({ plans: [{ number: "0101", phases: [dev("1")] }], lanes: { a: ["0101"] } });
  ctx.beforeMerge = async () => {
    writeFileSync(join(repo, "phase-0101-1.txt"), "a different phase 1 on main\n");
    sh(["add", "phase-0101-1.txt"], repo);
    sh(["commit", "-q", "-m", "feat: a conflicting commit on main"], repo);
  };
  await runLanes(ctx);
  const rec = loadState(ctx.stateDir).plans["0101"];
  assert.equal(rec.status, "parked");
  assert.equal(rec.park.reason, "merge_conflict");
  assert.ok(existsSync(rec.worktree));
  assert.equal(git(["status", "--porcelain"], rec.worktree).stdout, "", "the aborted merge left the worktree clean");
});

test("two lanes closing at once serialize on the close lock", async () => {
  const { ctx, readEvents } = scratch({
    plans: [
      { number: "0101", phases: [dev("1")] },
      { number: "0102", phases: [dev("1")] },
    ],
    lanes: { a: ["0101"], b: ["0102"] },
    spec: { "0101": { delayMs: { review: 400 } }, "0102": { delayMs: { review: 400 } } },
  });
  await runLanes(ctx);
  const state = loadState(ctx.stateDir);
  assert.equal(state.plans["0101"].status, "merged", JSON.stringify(state.plans["0101"].park));
  assert.equal(state.plans["0102"].status, "merged", JSON.stringify(state.plans["0102"].park));

  const ev = readEvents();
  const at = (plan, event) => ev.findIndex((e) => e.plan === plan && e.event === event);
  const reviewStarts = ev.filter((e) => e.event === "review-start");
  const first = reviewStarts[0].plan;
  const second = first === "0101" ? "0102" : "0101";
  assert.ok(at(first, "conductor-ff") >= 0);
  assert.ok(
    at(second, "review-start") > at(first, "conductor-ff"),
    `the second review started only after the first fast-forward: ${ev.map((e) => `${e.plan}:${e.event}`).join(" ")}`,
  );
  // Both versions landed in order on one main.
  assert.deepEqual([state.plans[first].closed.tag, state.plans[second].closed.tag], ["v0.1.1", "v0.1.2"]);
});

test("a budget-exhausted step parks with its spend recorded", async () => {
  const { ctx } = scratch({
    plans: [{ number: "0101", phases: [dev("1")] }],
    lanes: { a: ["0101"] },
    spec: { "0101": { budget: "implement" } },
  });
  await runLanes(ctx);
  const rec = loadState(ctx.stateDir).plans["0101"];
  assert.equal(rec.status, "parked");
  assert.equal(rec.park.reason, "budget");
  assert.equal(rec.steps[0].result.spendUsd, 7.5);
  assert.equal(rec.steps[0].result.subtype, "error_max_budget_usd");
});

test("a red conductor gate parks before any review", async () => {
  const { ctx } = scratch({ plans: [{ number: "0101", phases: [dev("1")] }], lanes: { a: ["0101"] } });
  ctx.gate = [{ name: "always-red", cmd: [process.execPath, "-e", "console.log('        FAIL [   0.010s] rlx-core::golden drifts'); process.exit(100)"] }];
  await runLanes(ctx);
  const rec = loadState(ctx.stateDir).plans["0101"];
  assert.equal(rec.status, "parked");
  assert.equal(rec.park.reason, "gate_red");
  assert.match(rec.park.detail, /always-red exited 100 - failing: rlx-core::golden drifts/);
  assert.deepEqual(kinds(rec), ["implement:dev"]);
});
