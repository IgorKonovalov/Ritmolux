// The lane loop end to end, against a throwaway git repository and a fake CLI whose sessions make
// real commits (lane-scenario.mjs). Each test builds its own repository, so the scenarios share
// nothing but the code under test.

import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { appendFileSync, existsSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";

import { writeDigest, writeHistory } from "../lib/digest.mjs";
import { git, resolveCommit, tagObjectType } from "../lib/git.mjs";
import { runLanes } from "../lib/lane.mjs";
import { readLedger } from "../lib/ledger.mjs";
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
  // Where a `servedClose` puts its bump, as the real close puts it: a version line and nothing else.
  writeFileSync(join(repo, "Cargo.toml"), '[workspace.package]\nversion = "0.1.0"\nedition = "2021"\n');
  // As the real repository ignores it: a lane that installs the studio's dependencies must still
  // read as a clean worktree, or the removal at the end of the plan refuses.
  writeFileSync(join(repo, ".gitignore"), "studio/node_modules/\n");
  for (const p of plans) writePlan(repo, p);
  sh(["add", "VERSION", "README.md", "Cargo.toml", ".gitignore", "docs"], repo);
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
  // Both pages live outside the scratch repository, as the real ones live in gitignored paths:
  // a file inside would make the main checkout dirty and refuse every fast-forward.
  const outDir = tmp("rlx-digest-");
  const digestPath = join(outDir, "digest.md");
  const historyPath = join(outDir, "digest-history.md");
  ctx.onChange = () => {
    writeDigest(digestPath, ctx.state, { repo, stateDir });
    writeHistory(historyPath, ctx.state, { repo, stateDir });
  };
  const sectionOf = (path) => (heading) => {
    const text = readFileSync(path, "utf8");
    if (!heading) return text;
    const start = text.indexOf(`${heading}\n`);
    assert.ok(start >= 0, `${path} has no ${heading}:\n${text}`);
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
  return { ctx, repo, readEvents, digest: sectionOf(digestPath), history: sectionOf(historyPath) };
}

const dev = (id) => ({ id, owner: "dev" });
const studio = (id) => ({ id, owner: "studio-builder" });
const human = (id) => ({ id, owner: "human" });
const kinds = (rec) => rec.steps.map((s) => (s.owner ? `${s.kind}:${s.owner}` : s.kind));

test("a dev run then a studio-builder run with a clean review merges, tagged, lane removed", async () => {
  const { ctx, repo, digest, history } = scratch({ plans: [{ number: "0101", phases: [dev("1"), dev("2"), studio("3")] }], lanes: { a: ["0101"] } });
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

  const closed = history("### Closed");
  assert.match(closed, /^- \*\*0101 - Plan 0101 fixture\*\* - 0\.1\.1, tag `v0\.1\.1` annotated, merge `[0-9a-f]{7}`, 0 fix rounds, /m);
  assert.match(closed, /Review: `docs\/plans\/done\/0101-fixture\.md` `## Close review`\./);
  assert.match(history("### Needs you"), /^- nothing: no park, and every merge was clean\.$/m);
  assert.match(digest("## Needs you"), /^Nothing: no park, no lane stopped at the worktree cap, no open finding\.$/m);
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
  const { ctx, repo, digest, history } = scratch({
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

  const needs = history("### Needs you");
  assert.match(needs, /^- \*\*0101 parked\*\* at Phase 2 \(`human_phase`\)\. Phase 2 is owned by human\. Read: docs\/plans\/0101-fixture\.md Phase 2\. Holds `[^`]*rlx-plan-0101`\.$/m);
  assert.match(needs, /^ {2}Resume: `node tools\/conductor\/conductor\.mjs resume 0101`$/m);
  assert.match(history("### Closed"), /^- \*\*0102 - Plan 0102 fixture\*\*/m);

  // The same park is the current page's whole worklist, with the command that clears it.
  const worklist = digest("## Needs you");
  assert.match(worklist, /^1 park\.$/m);
  assert.match(worklist, /^- \*\*0101\*\* \(`human_phase`\) at Phase 2 parked .* Holds `[^`]*rlx-plan-0101`\.$/m);
  assert.match(worklist, /^ {2}Resume: `node tools\/conductor\/conductor\.mjs resume 0101`$/m);

  // A park on a clean worktree records no path list and prints nothing extra.
  assert.equal("dirty" in parked.park, false);
  assert.ok(!inbox.includes("Left dirty"));
  assert.ok(!needs.includes("Left dirty"));
});

test("a park that leaves the worktree dirty names the paths, capped, in the record, the inbox and the digest", async () => {
  const { ctx, history } = scratch({
    plans: [{ number: "0101", phases: [dev("1")] }],
    lanes: { a: ["0101"] },
    spec: { "0101": { dirtyPark: { untracked: 13 } } },
  });
  await runLanes(ctx);
  const rec = loadState(ctx.stateDir).plans["0101"];
  assert.equal(rec.status, "parked");
  assert.equal(rec.park.reason, "check_red");
  const shown = ["VERSION", ...Array.from({ length: 9 }, (_, i) => `bless-${String(i + 1).padStart(2, "0")}.png`)];
  assert.deepEqual(rec.park.dirty, { paths: shown, more: 4 }, "ten paths shown, the other four counted");

  const text = `${shown.map((p) => `\`${p}\``).join(", ")} and 4 more`;
  const inbox = readFileSync(statePaths(ctx.stateDir).inbox, "utf8");
  assert.ok(inbox.includes(`- **Left dirty:** ${text}. \`resume\` refuses until the worktree is clean.`), inbox);
  const needs = history("### Needs you");
  assert.match(needs, /^- \*\*0101 parked\*\* \(`check_red`\)\. .* Holds `[^`]*rlx-plan-0101`\. Left dirty: .*$/m);
  assert.ok(needs.includes(` Left dirty: ${text}.\n`), needs);
});

test("a lane that reaches the worktree cap stops, and the run and the digest say why", async () => {
  const { ctx, readEvents, digest, history } = scratch({
    plans: [
      { number: "0101", phases: [dev("1"), human("2")] },
      { number: "0102", phases: [dev("1")] },
      { number: "0103", phases: [dev("1")] },
    ],
    lanes: { a: ["0101", "0102", "0103"] },
    after: { "0103": ["0101"] },
    local: { max_open_worktrees: 1 },
  });
  await runLanes(ctx);
  const state = loadState(ctx.stateDir);
  assert.equal(state.plans["0101"].status, "parked");
  assert.equal(state.plans["0102"], undefined, "the cap held 0102 back");

  const run = state.runs.at(-1);
  assert.equal(run.stops.length, 1);
  const { at, ...stop } = run.stops[0];
  assert.deepEqual(stop, { lane: "a", reason: "worktree_cap", plan: "0102", holding: ["0101"], max: 1 });
  assert.ok(at);
  assert.deepEqual(run.notStarted, [
    { plan: "0102", lane: "a", reason: "worktree cap" },
    { plan: "0103", lane: "a", reason: "after 0101 (parked)" },
  ]);
  assert.ok(readEvents().some((e) => e.event === "conductor-worktree-cap" && e.plan === "0102"));

  const cap = "- **Lane a stopped at the worktree cap** (`max_open_worktrees` 1): 0102 was not opened. Worktrees held by 0101.";
  const needs = history("### Needs you");
  assert.deepEqual(needs.split("\n").filter((l) => l.includes("worktree cap")), [cap]);
  assert.equal(history("### Not started").trim(), "- **0102** (lane a): worktree cap\n- **0103** (lane a): after 0101 (parked)");

  // The cap is current state too: it is still holding the slot when the run ends.
  const worklist = digest("## Needs you");
  assert.match(worklist, /^1 park, 1 lane stopped at the worktree cap\.$/m);
  assert.deepEqual(worklist.split("\n").filter((l) => l.startsWith("- **Lane ")), [cap]);
});

/** Three plans parked in an earlier run, each naming a worktree directory the test builds for real. */
function parkedHolders(ctx, { present }) {
  const recs = ["0091", "0092", "0093"].map((plan) => {
    const worktree = tmp(`rlx-held-${plan}-`);
    if (!present) rmSync(worktree, { recursive: true, force: true });
    return {
      plan,
      status: "parked",
      lane: "a",
      worktree,
      branch: `plan-${plan}-held`,
      laneRemoved: false,
      steps: [],
      park: { reason: "plan_wrong", detail: "held", phase: null, read: null, worktree, at: "2026-09-14T10:00:00.000Z" },
      parks: [],
      fixRounds: 0,
      verdicts: [],
      fixes: [],
      lockWaits: [],
    };
  });
  for (const r of recs) {
    r.parks.push(r.park);
    ctx.state.plans[r.plan] = r;
  }
  return recs;
}

test("the cap counts worktrees on disk: three parked plans whose directories are gone do not hold the next one back", async () => {
  const { ctx } = scratch({ plans: [{ number: "0101", phases: [dev("1")] }], lanes: { a: ["0101"] } });
  const held = parkedHolders(ctx, { present: false });
  const lines = [];
  ctx.live = (l) => lines.push(l);
  await runLanes(ctx);
  const state = loadState(ctx.stateDir);
  assert.equal(state.plans["0101"].status, "merged", JSON.stringify(state.plans["0101"].park));
  assert.equal(state.runs.at(-1).stops, undefined, "no cap stop");

  // Each standing-park line names the branch resume reopens from, and no worktree path.
  for (const r of held) {
    const line = lines.find((l) => l.includes(` ${r.plan} still parked`));
    assert.ok(line, lines.join("\n"));
    assert.ok(line.includes(`resume reopens it from branch ${r.branch}`), line);
    assert.ok(!line.includes("rlx-held-"), `no worktree path: ${line}`);
  }
});

test("the cap counts worktrees on disk: the same three with their directories present stop the lane and are named", async () => {
  const { ctx } = scratch({ plans: [{ number: "0101", phases: [dev("1")] }], lanes: { a: ["0101"] } });
  const held = parkedHolders(ctx, { present: true });
  const lines = [];
  ctx.live = (l) => lines.push(l);
  await runLanes(ctx);
  const state = loadState(ctx.stateDir);
  assert.equal(state.plans["0101"], undefined, "0101 was not opened");
  const { at, ...stop } = state.runs.at(-1).stops[0];
  assert.deepEqual(stop, { lane: "a", reason: "worktree_cap", plan: "0101", holding: ["0091", "0092", "0093"], max: 3 });
  for (const r of held) assert.ok(lines.some((l) => l.includes(` ${r.plan} still parked`) && l.includes(`holds ${r.worktree}`)), lines.join("\n"));
});

// ADR-0219: the ask is read where the stop request is, so the plan in flight finishes and no other
// starts. The lane's own reason for stopping is in the run record, apart from `--once` and from a
// queue that ran out.
test("a paused lane finishes the plan in flight and starts no other, and the run says it was paused", async () => {
  const { ctx } = scratch({
    plans: [
      { number: "0101", phases: [dev("1")] },
      { number: "0102", phases: [dev("1")] },
    ],
    lanes: { a: ["0101", "0102"] },
  });
  // The ask arrives while 0101 runs: it is set the first time the loop looks, so 0101 is picked and
  // run to its merge, and the second look is the one that stops the lane.
  let looks = 0;
  ctx.paused = () => looks++ > 0;
  await runLanes(ctx);

  const state = loadState(ctx.stateDir);
  assert.equal(state.plans["0101"].status, "merged", JSON.stringify(state.plans["0101"].park));
  assert.equal(state.plans["0102"], undefined, "the pause held 0102 back");
  const run = state.runs.at(-1);
  assert.deepEqual(run.paused.lanes, ["a"]);
  assert.ok(run.paused.at);
  assert.deepEqual(run.notStarted, [{ plan: "0102", lane: "a", reason: "paused" }]);
  assert.equal(run.stops, undefined, "a pause is not a worktree-cap stop");
});

test("a lane that was never paused carries no pause in the run record", async () => {
  const { ctx } = scratch({ plans: [{ number: "0101", phases: [dev("1")] }], lanes: { a: ["0101"] } });
  await runLanes(ctx);
  assert.equal(loadState(ctx.stateDir).runs.at(-1).paused, undefined);
});

test("a run that opens every queued plan it can has no Not started list", async () => {
  const { ctx, history } = scratch({ plans: [{ number: "0101", phases: [dev("1")] }], lanes: { a: ["0101"] } });
  await runLanes(ctx);
  assert.equal(loadState(ctx.stateDir).runs.at(-1).notStarted.length, 0);
  assert.ok(!history().includes("### Not started"));
});

test("a review with one major takes exactly one fix round and a re-review, then merges", async () => {
  const { ctx, history } = scratch({
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
  const closed = history("### Closed");
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
  const { ctx, history } = scratch({
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

  assert.match(history("### Failed and parked"), /^- \*\*0101\*\* spend cap hit in `0101-01-implement`: spent \$7\.50\.$/m);
  assert.match(history("### Totals"), /^- lane a: 0 merged, 1 parked, \$7\.50\.$/m);
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
  assert.match(rec.park.detail, /after the close: check-backlog-claims\.mjs/);
  assert.deepEqual(rec.gates.map((g) => [g.label, g.ok]), [["pre-review", true], ["post-close", false]]);
  assert.equal(resolveCommit("main", repo), mainBefore, "main did not move");
});

test("a close that repairs one minor and leaves one open shows exactly the open one in Needs you", async () => {
  const { ctx, digest, history } = scratch({ plans: [{ number: "0101", phases: [dev("1")] }], lanes: { a: ["0101"] }, spec: { "0101": { closeRepair: "ok" } } });
  await runLanes(ctx);
  const rec = loadState(ctx.stateDir).plans["0101"];
  assert.equal(rec.status, "merged", JSON.stringify(rec.park));
  const repaired = rec.verdicts.at(-1).findings.find((f) => f.fixed_in);
  assert.ok(repaired, "the verdict kept fixed_in");
  const open = ["- **0101 merged with 1 open finding**:", "  - minor `phase-0101-1.txt:2` a duplicated constant, left open"];
  assert.deepEqual(
    history("### Needs you")
      .split("\n")
      .filter((l) => l.trim()),
    open,
  );
  // An open finding is current state, so the page carries it and counts it in its summary.
  const worklist = digest("## Needs you");
  assert.match(worklist, /^1 merge with open findings\.$/m);
  assert.deepEqual(
    worklist
      .split("\n")
      .filter((l) => l.trim())
      .slice(1),
    open,
  );
  assert.ok(history("### Closed").includes(`  - minor \`phase-0101-1.txt:1\` a comment the plan made false - repaired by the close in \`${repaired.fixed_in.slice(0, 7)}\``));
});

test("a fixed_in commit that does not change the finding's file parks as a disagreement", async () => {
  const { ctx, repo } = scratch({ plans: [{ number: "0101", phases: [dev("1")] }], lanes: { a: ["0101"] }, spec: { "0101": { closeRepair: "wrongFile" } } });
  const mainBefore = resolveCommit("main", repo);
  await runLanes(ctx);
  const rec = loadState(ctx.stateDir).plans["0101"];
  assert.equal(rec.status, "parked");
  assert.equal(rec.park.reason, "disagreement");
  assert.match(rec.park.detail, /finding 0 is fixed_in [0-9a-f]{7}, which does not change phase-0101-1\.txt/);
  assert.equal(resolveCommit("main", repo), mainBefore);
});

test("a fixed_in commit that is not on the branch parks as a disagreement", async () => {
  const { ctx } = scratch({ plans: [{ number: "0101", phases: [dev("1")] }], lanes: { a: ["0101"] }, spec: { "0101": { closeRepair: "offBranch" } } });
  await runLanes(ctx);
  const rec = loadState(ctx.stateDir).plans["0101"];
  assert.equal(rec.status, "parked");
  assert.equal(rec.park.reason, "disagreement");
  assert.match(rec.park.detail, /finding 0 is fixed_in [0-9a-f]{40}, which is not on the branch/);
});

test("with no fix round and an unmoved main, a plan executes the full suite twice and skips it twice", async () => {
  const { ctx, history } = scratch({ plans: [{ number: "0101", phases: [dev("1"), dev("2")] }], lanes: { a: ["0101"] }, spec: { "0101": { ledgerFlow: true } } });
  ctx.gate = [{ name: "cargo nextest", cmd: [process.execPath, "-e", "console.log('     Summary [   1.000s] 3 tests run: 3 passed')"], lock: "suite", ledger: true }];
  await runLanes(ctx);
  const rec = loadState(ctx.stateDir).plans["0101"];
  assert.equal(rec.status, "merged", JSON.stringify(rec.park));
  assert.equal(rec.fixRounds, 0);
  assert.deepEqual(rec.gates.map((g) => g.label), ["pre-review", "post-close"]);

  const ledger = readLedger(join(ctx.stateDir, "suite-ledger.jsonl"));
  const executed = ledger.filter((e) => !e.skip);
  const skipped = ledger.filter((e) => e.skip);
  assert.deepEqual(executed.map((e) => e.by), ["gate 0101-pre-review", "0101-02-review"], "pre-review and the close's gate");
  assert.deepEqual(skipped.map((e) => [e.by, e.green.by]), [
    ["0101-02-review", "gate 0101-pre-review"],
    ["gate 0101-post-close", "0101-02-review"],
  ]);
  assert.deepEqual(rec.gates[1].commands.map((c) => c.skipped), [true]);
  assert.match(history("### Totals"), /; 2 suite runs skipped\.$/m);
});

// ADR-0211: the close tip differs from the reviewed tree only in prose, a version line and a merge,
// so its gate runs `-P fast` in the full suite's place — and says so on its own run-terminal line.
test("a close tip whose diff is served runs -P fast, and the run terminal names the tier and the tree", async () => {
  const { ctx, history } = scratch({ plans: [{ number: "0101", phases: [dev("1"), dev("2")] }], lanes: { a: ["0101"] }, spec: { "0101": { servedClose: true } } });
  // A script file, not `node -e`: node parses a trailing `-P` after `-e <code>` as its own option.
  const script = join(tmp("rlx-lane-suite-"), "suite.cjs");
  writeFileSync(script, "console.log('     Summary [   1.000s] 3 tests run: 3 passed');\n");
  ctx.gate = [{ name: "cargo nextest", cmd: [process.execPath, script], lock: "suite", ledger: true }];
  const out = [];
  ctx.live = (l) => out.push(l);
  await runLanes(ctx);

  const rec = loadState(ctx.stateDir).plans["0101"];
  assert.equal(rec.status, "merged", JSON.stringify(rec.park));
  assert.deepEqual(rec.gates.map((g) => g.label), ["pre-review", "post-close"]);
  assert.deepEqual(rec.gates[0].commands.map((c) => [c.suite, c.served ?? false, c.skipped ?? false]), [[true, false, false]]);
  assert.deepEqual(rec.gates[1].commands.map((c) => [c.suite, c.served ?? false, c.skipped ?? false]), [[true, true, false]]);

  const ledger = readLedger(join(ctx.stateDir, "suite-ledger.jsonl"));
  assert.deepEqual(ledger.map((e) => [e.by, e.served ?? false]), [
    ["gate 0101-pre-review", false],
    ["gate 0101-post-close", true],
  ]);
  assert.equal(ledger[1].green.tree, ledger[0].tree, "the served line names the tree it leaned on");
  // The close's own diff: the plan's move under docs/, and the version line.
  assert.deepEqual([...ledger[1].diff].sort(), ["Cargo.toml", "docs/plans/0101-fixture.md", "docs/plans/done/0101-fixture.md"]);

  const served = out.filter((l) => l.includes("served -P fast"));
  assert.equal(served.length, 1, out.join("\n"));
  assert.match(
    served[0],
    new RegExp(`^\\d\\d:\\d\\d 0101   gate   cargo nextest served -P fast: tree ${ledger[0].tree.slice(0, 7)} green by gate 0101-pre-review at \\S+, 3 served paths$`),
  );
  // It is printed while the gate is still running, before that gate's own verdict line. Whether a
  // `running` line follows is the reader's own business, pinned in live.test.mjs against a real
  // cargo vector; the stand-in here is not one.
  assert.ok(out.indexOf(served[0]) < out.findIndex((l) => /gate post-close  green/.test(l)), out.join("\n"));
  for (const l of out) assert.match(l, /^[\x20-\x7e]*$/, `ASCII: ${l}`);

  assert.match(history("### Totals"), /full suite .+ over 1 run, served -P fast .+ over 1 run, everything else /);
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

// Backlog 0229: a review that commits its close and then loses its outcome leaves the branch closed
// and the record open. Deciding from the record alone would start round 1 again on a plan already
// under done/, and write a second close, a second version and a second tag.

test("a close that landed without an outcome is adopted on the next run, with no second review", async () => {
  const { ctx, repo } = scratch({
    plans: [{ number: "0101", phases: [dev("1")] }],
    lanes: { a: ["0101"] },
    spec: { "0101": { loseOutcome: "review" } },
  });
  await runLanes(ctx);
  const parked = ctx.state.plans["0101"];
  assert.equal(parked.park.reason, "no_outcome");
  assert.equal(parked.closed, null, "the record did not learn about the close the branch carries");
  const wt = parked.worktree;
  assert.ok(readPlanFile(findPlan(wt, "0101").path).hasCloseReview, "the close itself did land");

  parked.status = "queued";
  parked.park = null;
  await runLanes(ctx);

  const rec = loadState(ctx.stateDir).plans["0101"];
  assert.equal(rec.status, "merged", JSON.stringify(rec.park));
  assert.deepEqual(kinds(rec), ["implement:dev", "review:architect"], "the second run started no session at all");
  assert.equal(rec.closed.adopted, true);
  assert.equal(rec.closed.tag, "v0.1.1");
  // One version and one tag: the whole point. A second close would have bumped to 0.1.2.
  assert.deepEqual(git(["tag", "--list", "v0.1.*"], repo).stdout.split("\n").sort(), ["v0.1.0", "v0.1.1"]);
  assert.equal(readFileSync(join(repo, "VERSION"), "utf8"), "0.1.1\n");
  assert.equal(tagObjectType("v0.1.1", repo), "tag");
  assert.equal(resolveCommit("v0.1.1", repo), resolveCommit("main", repo));
  assert.deepEqual(rec.gates.map((g) => g.label), ["pre-review", "post-close"], "the adopted tip is still gated before main moves");
  assert.equal(findPlan(repo, "0101").done, true);
});

test("a close on the branch over a dirty tree parks disagreement and names the dirt", async () => {
  const { ctx } = scratch({
    plans: [{ number: "0101", phases: [dev("1")] }],
    lanes: { a: ["0101"] },
    spec: { "0101": { loseOutcome: "review", dirtyClose: true } },
  });
  await runLanes(ctx);
  const first = ctx.state.plans["0101"];
  assert.equal(first.park.reason, "no_outcome");
  assert.deepEqual(first.park.dirty.paths, ["suite-output.log"]);

  first.status = "queued";
  first.park = null;
  await runLanes(ctx);

  const rec = loadState(ctx.stateDir).plans["0101"];
  assert.equal(rec.status, "parked");
  assert.equal(rec.park.reason, "disagreement");
  assert.match(rec.park.detail, /close found on the branch: .*worktree is not clean/);
  assert.equal(rec.closed, null, "nothing was recorded from a close that does not verify");
  assert.deepEqual(kinds(rec), ["implement:dev", "review:architect"], "and no second review ran");
  assert.deepEqual(rec.park.dirty.paths, ["suite-output.log"]);
});

// ADR-0210: a phase whose declared files include `.claude/` parks before the phase runs, with the
// edit as the detail. 0177 Phase 8 did the whole phase's work and then parked `check_red` on its own
// done-when, which is the shape this replaces.

const claudePhase = (id) => ({ id, owner: "dev", files: "`.claude/skills/dev/SKILL.md`" });

test("a phase declaring a `.claude/` file parks before any session runs, naming the edit", async () => {
  const { ctx } = scratch({ plans: [{ number: "0101", phases: [claudePhase("1"), dev("2")] }], lanes: { a: ["0101"] } });
  await runLanes(ctx);
  const rec = loadState(ctx.stateDir).plans["0101"];

  assert.equal(rec.status, "parked");
  assert.equal(rec.park.reason, "claude_dir");
  assert.equal(rec.park.phase, "1");
  assert.match(rec.park.detail, /\.claude\/skills\/dev\/SKILL\.md/);
  assert.match(rec.park.detail, /ADR-0210/);
  assert.match(rec.park.detail, /nothing was run/);
  assert.deepEqual(kinds(rec), [], "no session was started at all");
  assert.notEqual(rec.park.reason, "check_red");
  assert.deepEqual(rec.gates ?? [], [], "and no gate ran either");
});

test("the phases before a `.claude/` phase in the same run are still done, then the lane parks", async () => {
  const { ctx } = scratch({ plans: [{ number: "0101", phases: [dev("1"), claudePhase("2"), dev("3")] }], lanes: { a: ["0101"] } });
  await runLanes(ctx);
  const rec = loadState(ctx.stateDir).plans["0101"];

  assert.deepEqual(kinds(rec), ["implement:dev"], "one session, for Phase 1 alone");
  assert.deepEqual(rec.steps[0].phases, ["1"], "the range stopped in front of Phase 2");
  assert.equal(rec.park.reason, "claude_dir");
  assert.equal(rec.park.phase, "2");
  // The truncated run is not the plan's last, so that session was not asked to write a close block.
  assert.equal(readFileSync(join(ctx.stateDir, "prompts", `${rec.steps[0].label}.md`), "utf8").includes("RLX-CONDUCTOR-LAST-RUN: no"), true);
});

test("once the owner has done the `.claude/` phase and marked its row, the plan runs on to a merge", async () => {
  const { ctx, repo } = scratch({ plans: [{ number: "0101", phases: [claudePhase("1"), dev("2")] }], lanes: { a: ["0101"] } });
  await runLanes(ctx);
  const parked = ctx.state.plans["0101"];
  assert.equal(parked.park.reason, "claude_dir");

  // The owner makes the edit in the lane and marks the row, as they would for a `human` phase.
  const wt = parked.worktree;
  const planPath = join(wt, "docs", "plans", "0101-fixture.md");
  writeFileSync(join(wt, "phase-0101-1.txt"), "done by the owner\n");
  writeFileSync(planPath, readFileSync(planPath, "utf8").replace(/^\| 1 — Step 1 \| dev \| not started \|/m, "| 1 — Step 1 | dev | done |"));
  sh(["add", "phase-0101-1.txt", "docs/plans/0101-fixture.md"], wt);
  sh(["commit", "-q", "-m", "docs(plans): phase 1 done by the owner"], wt);
  parked.status = "queued";
  parked.park = null;

  await runLanes(ctx);
  const rec = loadState(ctx.stateDir).plans["0101"];
  assert.equal(rec.status, "merged", JSON.stringify(rec.park));
  assert.deepEqual(rec.steps.filter((s) => s.kind === "implement").map((s) => s.phases), [["2"]], "only Phase 2 was handed to a session");
  assert.equal(readFileSync(join(repo, "phase-0101-1.txt"), "utf8"), "done by the owner\n");
});

// ADR-0218: a lane makes its plan's preconditions true. `studio/node_modules` is gitignored and
// `git worktree add` never creates one, so without an install the gate's three studio checks skip.

const studioPhase = (id) => ({ id, owner: "studio-builder", files: "`studio/src/app.tsx`" });

/**
 * An install stand-in: it records the directory it ran in, creates the directory the gate's studio
 * steps are guarded on, and fails when FAIL exists — which is what an offline lane looks like.
 */
function installStandIn() {
  const dir = tmp("rlx-lane-install-");
  const ran = join(dir, "ran.txt");
  const fail = join(dir, "FAIL");
  const script = join(dir, "install.cjs");
  const q = (p) => JSON.stringify(p);
  writeFileSync(
    script,
    `const fs=require('fs'),path=require('path');fs.appendFileSync(${q(ran)},process.cwd()+'\\n');\n` +
      `if(fs.existsSync(${q(fail)})){console.log('npm error code ENOTFOUND');console.error('npm error network request to https://registry.npmjs.org failed');process.exit(1)}\n` +
      `fs.mkdirSync(path.join(process.cwd(),'studio','node_modules'),{recursive:true});\n`,
  );
  return {
    cmd: [process.execPath, script],
    fail: () => writeFileSync(fail, ""),
    recover: () => rmSync(fail, { force: true }),
    runs: () => (existsSync(ran) ? readFileSync(ran, "utf8").trim().split("\n").filter(Boolean) : []),
  };
}

/** The gate's three studio steps, stood in for: each records that it ran, and each is guarded. */
function studioGate() {
  const marker = join(tmp("rlx-lane-studio-gate-"), "ran.txt");
  const step = (name) => ({
    name,
    cmd: [process.execPath, "-e", `require('fs').appendFileSync(${JSON.stringify(marker)}, ${JSON.stringify(name)} + '\\n')`],
    onlyIf: "studio/node_modules",
    enabledBy: "npm --prefix studio ci",
  });
  return {
    commands: [step("studio typecheck"), step("studio lint"), step("studio test")],
    ran: () => (existsSync(marker) ? readFileSync(marker, "utf8").trim().split("\n").filter(Boolean) : []),
  };
}

test("a lane for a plan declaring studio/ installs its dependencies, and all three studio checks run", async () => {
  const { ctx } = scratch({ plans: [{ number: "0101", phases: [studioPhase("1")] }], lanes: { a: ["0101"] } });
  const install = installStandIn();
  const gate = studioGate();
  ctx.studioInstall = install.cmd;
  ctx.gate = gate.commands;
  await runLanes(ctx);
  const rec = loadState(ctx.stateDir).plans["0101"];

  assert.equal(rec.status, "merged", JSON.stringify(rec.park));
  assert.deepEqual(install.runs(), [rec.worktree], "installed once, in the lane");
  assert.deepEqual(gate.ran(), [
    "studio typecheck",
    "studio lint",
    "studio test",
    "studio typecheck",
    "studio lint",
    "studio test",
  ], "all three, at pre-review and again at post-close");
  assert.deepEqual(rec.gates[0].ran, ["studio typecheck", "studio lint", "studio test"]);
  assert.deepEqual(rec.gates[0].commands.map((c) => c.unmet ?? null), [null, null, null], "nothing was skipped");
});

test("a lane for a plan that does not name studio/ installs nothing, and the skipped checks say so", async () => {
  const { ctx } = scratch({ plans: [{ number: "0101", phases: [dev("1")] }], lanes: { a: ["0101"] } });
  const install = installStandIn();
  const gate = studioGate();
  ctx.studioInstall = install.cmd;
  ctx.gate = gate.commands;
  const out = [];
  ctx.live = (l) => out.push(l);
  await runLanes(ctx);
  const rec = loadState(ctx.stateDir).plans["0101"];

  assert.equal(rec.status, "merged", JSON.stringify(rec.park));
  assert.deepEqual(install.runs(), [], "nothing was installed");
  assert.deepEqual(gate.ran(), [], "and the three checks could not run");
  assert.deepEqual(rec.gates[0].ran, []);
  assert.deepEqual(
    rec.gates[0].commands.map((c) => [c.name, c.skipped, c.unmet]),
    [
      ["studio typecheck", true, "no studio/node_modules; run: npm --prefix studio ci"],
      ["studio lint", true, "no studio/node_modules; run: npm --prefix studio ci"],
      ["studio test", true, "no studio/node_modules; run: npm --prefix studio ci"],
    ],
    "the gate's own record carries the skip rather than only a step count",
  );
  const skipped = out.filter((l) => l.includes("skipped:"));
  assert.equal(skipped.length, 6, out.join("\n"));
  assert.match(skipped[0], /^\d\d:\d\d 0101   gate   studio typecheck skipped: no studio\/node_modules; run: npm --prefix studio ci$/);
});

test("a failed install parks the plan before any session, with the tail as the detail", async () => {
  const { ctx } = scratch({ plans: [{ number: "0101", phases: [studioPhase("1")] }], lanes: { a: ["0101"] } });
  const install = installStandIn();
  install.fail();
  ctx.studioInstall = install.cmd;
  ctx.gate = studioGate().commands;
  await runLanes(ctx);
  const rec = loadState(ctx.stateDir).plans["0101"];

  assert.equal(rec.status, "parked");
  assert.equal(rec.park.reason, "studio_install");
  assert.match(rec.park.detail, /exited 1/);
  assert.match(rec.park.detail, /registry\.npmjs\.org failed/, "the install's tail is the detail");
  assert.deepEqual(kinds(rec), [], "no session was started");
  assert.deepEqual(rec.gates ?? [], [], "and no gate ran");
  assert.equal(rec.park.dirty, undefined, "the worktree was left clean");
  assert.match(readFileSync(statePaths(ctx.stateDir).inbox, "utf8"), /plan 0101 parked: studio_install/);
});

// The park leaves the worktree OPEN, which is what makes the recovery path load-bearing: an install
// asked for only at open could never run again, and the resume would reach the merge with the three
// studio checks skipped - the state ADR-0218 refuses.
test("a studio_install park clears on resume: the install runs again and the three checks run", async () => {
  const { ctx } = scratch({ plans: [{ number: "0101", phases: [studioPhase("1")] }], lanes: { a: ["0101"] } });
  const install = installStandIn();
  install.fail();
  const gate = studioGate();
  ctx.studioInstall = install.cmd;
  ctx.gate = gate.commands;
  await runLanes(ctx);
  const parked = ctx.state.plans["0101"];
  assert.equal(parked.park.reason, "studio_install");
  assert.ok(existsSync(parked.worktree), "the lane is open, which is what the resume walks back into");

  install.recover();
  parked.status = "queued";
  parked.park = null;
  await runLanes(ctx);

  const rec = loadState(ctx.stateDir).plans["0101"];
  assert.equal(rec.status, "merged", JSON.stringify(rec.park));
  assert.deepEqual(install.runs(), [rec.worktree, rec.worktree], "the open lane was installed into a second time");
  assert.deepEqual(rec.gates[0].commands.map((c) => c.unmet ?? null), [null, null, null], "nothing was skipped");
  assert.deepEqual(gate.ran(), [
    "studio typecheck",
    "studio lint",
    "studio test",
    "studio typecheck",
    "studio lint",
    "studio test",
  ], "all three, at pre-review and again at post-close");
});

test("a second run over an installed lane does not reinstall", async () => {
  const { ctx } = scratch({
    plans: [{ number: "0101", phases: [studioPhase("1")] }],
    lanes: { a: ["0101"] },
    spec: { "0101": { budget: "implement" } },
  });
  const install = installStandIn();
  ctx.studioInstall = install.cmd;
  ctx.gate = studioGate().commands;
  await runLanes(ctx);
  const parked = ctx.state.plans["0101"];
  assert.equal(parked.park.reason, "budget", JSON.stringify(parked.park));
  assert.deepEqual(install.runs(), [parked.worktree], "installed once, as the lane opened");

  parked.status = "queued";
  parked.park = null;
  await runLanes(ctx);

  // `npm ci` deletes node_modules before it installs, so redoing a good install is not free and is
  // not harmless: the absence of studio/node_modules is the trigger, never the run.
  assert.deepEqual(install.runs(), [parked.worktree], "and not again over a lane that already has its dependencies");
});
