// The lane loop end to end, against a throwaway git repository and a fake CLI whose sessions make
// real commits (lane-scenario.mjs). Each test builds its own repository, so the scenarios share
// nothing but the code under test.

import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { appendFileSync, existsSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";

import { writeDigest } from "../lib/digest.mjs";
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
    gate: [
      { name: "marker", cmd: [process.execPath, "-e", "process.exit(require('fs').existsSync('GATE_RED')?1:0)"], lock: "suite" },
      { name: "check-backlog-claims.mjs", cmd: [process.execPath, "-e", "process.exit(require('fs').existsSync('PROBE_RED')?1:0)"], afterClose: true },
    ],
    lockDir: tmp("rlx-locks-"),
    lockPollMs: 20,
    pollMs: 50,
    events: (name, data) => appendFileSync(events, JSON.stringify({ t: Date.now(), plan: data.plan, event: `conductor-${name}` }) + "\n"),
  };
  // The digest lives outside the scratch repository, as the real one lives in a gitignored path:
  // a file inside would make the main checkout dirty and refuse every fast-forward.
  const digestPath = join(tmp("rlx-digest-"), "digest.md");
  ctx.onChange = () => writeDigest(digestPath, ctx.state, { repo, stateDir });
  const digest = (heading) => {
    const text = readFileSync(digestPath, "utf8");
    if (!heading) return text;
    const start = text.indexOf(`${heading}\n`);
    assert.ok(start >= 0, `digest has no ${heading}:\n${text}`);
    const rest = text.slice(start + heading.length + 1);
    const end = rest.search(/^#{2,3} /m);
    return end < 0 ? rest : rest.slice(0, end);
  };
  const readEvents = () =>
    readFileSync(events, "utf8")
      .trim()
      .split("\n")
      .filter(Boolean)
      .map((l) => JSON.parse(l));
  return { ctx, repo, readEvents, digest };
}

const dev = (id) => ({ id, owner: "dev" });
const studio = (id) => ({ id, owner: "studio-builder" });
const human = (id) => ({ id, owner: "human" });
const kinds = (rec) => rec.steps.map((s) => (s.owner ? `${s.kind}:${s.owner}` : s.kind));

test("a dev run then a studio-builder run with a clean review merges, tagged, lane removed", async () => {
  const { ctx, repo, digest } = scratch({ plans: [{ number: "0101", phases: [dev("1"), dev("2"), studio("3")] }], lanes: { a: ["0101"] } });
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
  assert.deepEqual(rec.gates.map((g) => g.label), ["pre-review", "post-close"], "the close tip is gated before main moves");
  assert.equal(rec.gatedHead, resolveCommit("main", repo), "what reached main is what the conductor gated");

  const closed = digest("### Closed");
  assert.match(closed, /^- \*\*0101 - Plan 0101 fixture\*\* - 0\.1\.1, tag `v0\.1\.1` annotated, merge `[0-9a-f]{7}`, 0 fix rounds, /m);
  assert.match(closed, /Review: `docs\/plans\/done\/0101-fixture\.md` `## Close review`\./);
  assert.match(digest("### Needs you"), /^- nothing: no park, and every merge was clean\.$/m);
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
  const { ctx, repo, digest } = scratch({
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

  const needs = digest("### Needs you");
  assert.match(needs, /^- \*\*0101 parked\*\* at Phase 2 \(`human_phase`\)\. Phase 2 is owned by human\. Read: docs\/plans\/0101-fixture\.md Phase 2\. Holds `[^`]*rlx-plan-0101`\.$/m);
  assert.match(needs, /^ {2}Resume: `node tools\/conductor\/conductor\.mjs resume 0101`$/m);
  assert.match(digest("### Closed"), /^- \*\*0102 - Plan 0102 fixture\*\*/m);
});

test("a review with one major takes exactly one fix round and a re-review, then merges", async () => {
  const { ctx, digest } = scratch({
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
  assert.deepEqual(rec.gates.map((g) => g.label), ["pre-review", "fix-1", "post-close"]);

  const fixSha = rec.fixes[0].resolved[0].commit.slice(0, 7);
  const closed = digest("### Closed");
  assert.match(closed, /, 1 fix round, /);
  assert.ok(
    closed.includes(`  - major \`phase-0101-1.txt:1\` major finding in round 1 - resolved in \`${fixSha}\``),
    `the major is listed as resolved by the fix commit:\n${closed}`,
  );
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

test("a numeric `through` in an implementer's outcome is the same phase as its string id", async () => {
  const { ctx } = scratch({
    plans: [{ number: "0101", phases: [dev("1"), dev("2")] }],
    lanes: { a: ["0101"] },
    spec: { "0101": { numericThrough: true } },
  });
  await runLanes(ctx);
  const rec = loadState(ctx.stateDir).plans["0101"];
  assert.equal(rec.status, "merged", JSON.stringify(rec.park));
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
  assert.deepEqual(rec.gates.map((g) => g.label), ["pre-review", "post-close", "remerge"]);
});

test("a red gate on the close tip parks and main does not move", async () => {
  const { ctx, repo } = scratch({ plans: [{ number: "0101", phases: [dev("1")] }], lanes: { a: ["0101"] } });
  // The close session claims its own gate passed; the conductor's run on the close tip is what counts.
  ctx.beforeMerge = async () => {
    ctx.gate = [{ name: "red-after-close", cmd: [process.execPath, "-e", "process.exit(3)"] }];
  };
  const mainBefore = resolveCommit("main", repo);
  await runLanes(ctx);
  const rec = loadState(ctx.stateDir).plans["0101"];
  assert.equal(rec.status, "parked");
  assert.equal(rec.park.reason, "gate_red");
  assert.match(rec.park.detail, /after the close: red-after-close/);
  assert.deepEqual(rec.gates.map((g) => g.label), ["pre-review", "post-close"]);
  assert.equal(resolveCommit("main", repo), mainBefore, "main did not move");
  assert.ok(existsSync(rec.worktree), "the parked plan keeps its worktree");
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

test("a conflict the owner resolves in the lane is gated and re-tagged before main moves", async () => {
  const { ctx, repo } = scratch({ plans: [{ number: "0101", phases: [dev("1")] }], lanes: { a: ["0101"] } });
  ctx.beforeMerge = async () => {
    writeFileSync(join(repo, "phase-0101-1.txt"), "a different phase 1 on main\n");
    sh(["add", "phase-0101-1.txt"], repo);
    sh(["commit", "-q", "-m", "feat: a conflicting commit on main"], repo);
  };
  await runLanes(ctx);
  const rec = ctx.state.plans["0101"];
  assert.equal(rec.park.reason, "merge_conflict");

  // The owner merges main in the lane, resolves, commits, and resumes.
  const wt = rec.worktree;
  assert.notEqual(git(["merge", "main"], wt).code, 0);
  writeFileSync(join(wt, "phase-0101-1.txt"), "resolved by the owner\n");
  sh(["add", "phase-0101-1.txt"], wt);
  sh(["commit", "-q", "--no-edit"], wt);
  const tagMessageBefore = git(["tag", "-l", "--format=%(contents)", "v0.1.1"], repo).stdout;
  rec.status = "queued";
  rec.park = null;
  ctx.beforeMerge = undefined;

  await runLanes(ctx);
  const done = loadState(ctx.stateDir).plans["0101"];
  assert.equal(done.status, "merged", JSON.stringify(done.park));
  assert.deepEqual(
    done.gates.map((g) => g.label),
    ["pre-review", "post-close", "post-close"],
    "the close tip was gated before the conflict, and the resolved tip once after it",
  );
  assert.equal(done.merge.remerged, false);
  assert.equal(tagObjectType("v0.1.1", repo), "tag");
  assert.equal(resolveCommit("v0.1.1", repo), resolveCommit("main", repo), "the tag moved onto the resolved tip");
  assert.equal(git(["tag", "-l", "--format=%(contents)", "v0.1.1"], repo).stdout, tagMessageBefore);
  assert.equal(readFileSync(join(repo, "phase-0101-1.txt"), "utf8"), "resolved by the owner\n");
});

test("a fast-forward refused while main is already in the branch parks without a re-merge or a gate", async () => {
  const { ctx, repo } = scratch({ plans: [{ number: "0101", phases: [dev("1")] }], lanes: { a: ["0101"] } });
  const lock = join(repo, ".git", "index.lock");
  ctx.beforeMerge = async () => writeFileSync(lock, "");
  const mainBefore = resolveCommit("main", repo);
  try {
    await runLanes(ctx);
  } finally {
    if (existsSync(lock)) rmSync(lock);
  }
  const rec = loadState(ctx.stateDir).plans["0101"];
  assert.equal(rec.status, "parked");
  assert.equal(rec.park.reason, "merge_failed");
  assert.match(rec.park.detail, /main is already in plan-0101-fixture/);
  assert.deepEqual(rec.gates.map((g) => g.label), ["pre-review", "post-close"], "no gate ran for a re-merge that could not help");
  assert.equal(resolveCommit("main", repo), mainBefore);
  assert.equal(resolveCommit("HEAD", rec.worktree), rec.closed.head, "no merge commit was made on the branch");
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
  // The second close merged the first plan's code in; the conductor gated that combination itself.
  const later = state.plans[second];
  assert.deepEqual(later.gates.map((g) => g.label), ["pre-review", "post-close"]);
  assert.equal(later.gatedHead, later.merge.head, "the tip that reached main is the one the conductor gated");
});

test("a budget-exhausted step parks with its spend recorded", async () => {
  const { ctx, digest } = scratch({
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

  assert.match(digest("### Failed and parked"), /^- \*\*0101\*\* spend cap hit in `0101-01-implement`: spent \$7\.50\.$/m);
  assert.match(digest("### Totals"), /^- lane a: 0 merged, 1 parked, \$7\.50\.$/m);
});

test("a probe the implement commit breaks and the close repairs is not gated before the review, and merges", async () => {
  const { ctx, repo } = scratch({
    plans: [{ number: "0101", phases: [dev("1")] }],
    lanes: { a: ["0101"] },
    spec: { "0101": { breaksProbe: true } },
  });
  await runLanes(ctx);
  const rec = loadState(ctx.stateDir).plans["0101"];
  assert.equal(rec.status, "merged", JSON.stringify(rec.park));
  assert.deepEqual(rec.parks, []);
  assert.deepEqual(rec.gates.map((g) => [g.label, g.ok]), [["pre-review", true], ["post-close", true]]);
  assert.ok(!rec.gates[0].ran.includes("check-backlog-claims.mjs"), "the pre-review gate ran no probe");
  assert.ok(rec.gates[1].ran.includes("check-backlog-claims.mjs"), "the post-close gate ran the probe");
  assert.equal(existsSync(join(repo, "PROBE_RED")), false);
});

test("a probe the close leaves red parks gate_red at post-close and main does not move", async () => {
  const { ctx, repo } = scratch({
    plans: [{ number: "0101", phases: [dev("1")] }],
    lanes: { a: ["0101"] },
    spec: { "0101": { breaksProbe: true, probeStaysRed: true } },
  });
  const mainBefore = resolveCommit("main", repo);
  await runLanes(ctx);
  const rec = loadState(ctx.stateDir).plans["0101"];
  assert.equal(rec.status, "parked");
  assert.equal(rec.park.reason, "gate_red");
  assert.match(rec.park.detail, /after the close: check-backlog-claims.mjs/);
  assert.deepEqual(rec.gates.map((g) => [g.label, g.ok]), [["pre-review", true], ["post-close", false]]);
  assert.equal(resolveCommit("main", repo), mainBefore, "main did not move");
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
