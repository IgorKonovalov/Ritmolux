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
import { askResume, loadState, planRecord, statePaths } from "../lib/state.mjs";
import { FAKE, REPO, TEST_DIR, TOOL_DIR, tmp, writePlan } from "./helpers.mjs";

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
    local: { budget_usd: { readiness: 1, implement: 5, fix: 3, review: 4, close: 3, merge: 2, repair: 3 }, max_open_worktrees: 3, ...local },
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
  assert.deepEqual(kinds(rec), ["readiness:architect", "implement:dev", "implement:studio-builder", "review:architect", "close:architect"]);
  assert.deepEqual(rec.steps[1].phases, ["1", "2"]);
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
  assert.deepEqual(kinds(parked), ["readiness:architect", "implement:dev"]);
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
  assert.deepEqual(kinds(rec), ["readiness:architect", "implement:dev", "review:architect", "fix:dev", "review:architect", "close:architect"]);
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
  assert.deepEqual(kinds(rec), ["readiness:architect", "implement:dev", "review:architect", "fix:dev", "review:architect", "fix:dev", "review:architect"]);
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
  assert.deepEqual(kinds(rec), ["readiness:architect", "implement:dev"], "no review started");
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

test("a gate still red on the close tip after its one repair parks, and main does not move", async () => {
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
  assert.match(rec.park.detail, /^red-after-close exited 3; still red after one repair session at post-close$/);
  assert.deepEqual(rec.gates.map((g) => g.label), ["pre-review", "post-close", "post-close"]);
  assert.deepEqual(kinds(rec), ["readiness:architect", "implement:dev", "review:architect", "close:architect", "repair:dev"]);
  assert.equal(resolveCommit("main", repo), mainBefore, "main did not move");
  assert.ok(existsSync(rec.worktree), "the parked plan keeps its worktree");
});

// ADR-0251: the close reads origin/main's CI through `gh` before it merges. The scratch repository
// gains a GitHub `origin`, and `gh` is the fake the script's own self-test uses, answering from one of
// its scenarios — so these read exactly what `node scripts/check-upstream-ci.mjs` would print.
const UPSTREAM_FIXTURES = join(REPO, "scripts", "fixtures", "upstream-ci");

function withUpstream(ctx, repo, scenario) {
  sh(["remote", "add", "origin", "https://github.com/example/ritmolux.git"], repo);
  ctx.upstreamEnv = { ...process.env, RLX_GH: join(UPSTREAM_FIXTURES, "fake-gh.mjs"), RLX_FAKE_GH: join(UPSTREAM_FIXTURES, `${scenario}.json`) };
}

test("a red origin/main is reported and the close still merges, naming the failing job in the log and the digest", async () => {
  const { ctx, repo, digest } = scratch({ plans: [{ number: "0101", phases: [dev("1")] }], lanes: { a: ["0101"] } });
  withUpstream(ctx, repo, "red");
  const lines = [];
  ctx.live = (line) => lines.push(line);
  const mainBefore = resolveCommit("main", repo);
  await runLanes(ctx);
  const rec = loadState(ctx.stateDir).plans["0101"];
  assert.equal(rec.status, "merged", JSON.stringify(rec.park));
  assert.deepEqual(rec.parks, [], "a red reading never parks");
  assert.ok(kinds(rec).includes("close:architect"), `the close session ran: ${kinds(rec)}`);
  assert.notEqual(resolveCommit("main", repo), mainBefore, "main moved");
  assert.equal(tagObjectType("v0.1.1", repo), "tag", "the close tagged");
  assert.deepEqual(rec.upstream.map((u) => [u.state, u.run, u.jobs]), [["red", 2001, ["check (macos-latest)"]]]);
  const red = lines.filter((l) => l.includes("upstream CI: RED"));
  assert.equal(red.length, 1, lines.join("\n"));
  assert.match(red[0], /run 2001 at 2222222 concluded failure, failing check \(macos-latest\); closing anyway/);
  const needs = digest("## Needs you");
  assert.match(needs, /^origin\/main red\.$/m, needs);
  assert.match(needs, /origin\/main's CI is red\*\*: run 2001 at `2222222`, failing check \(macos-latest\), read by 0101's close/);
});

test("a green origin/main lets the close run, and the reading is recorded", async () => {
  const { ctx, repo, digest } = scratch({ plans: [{ number: "0101", phases: [dev("1")] }], lanes: { a: ["0101"] } });
  withUpstream(ctx, repo, "green");
  await runLanes(ctx);
  const rec = loadState(ctx.stateDir).plans["0101"];
  assert.equal(rec.status, "merged", JSON.stringify(rec.park));
  assert.deepEqual(rec.upstream.map((u) => [u.state, u.run]), [["green", 1001]]);
  assert.doesNotMatch(digest("## Needs you"), /origin\/main/);
});

test("an origin/main that cannot be read proceeds to the merge, and never parks", async () => {
  for (const [scenario, why] of [
    ["offline", "no network"],
    ["unauthenticated", "gh unauthenticated"],
  ]) {
    const { ctx, repo } = scratch({ plans: [{ number: "0101", phases: [dev("1")] }], lanes: { a: ["0101"] } });
    withUpstream(ctx, repo, scenario);
    const lines = [];
    ctx.live = (line) => lines.push(line);
    await runLanes(ctx);
    const rec = loadState(ctx.stateDir).plans["0101"];
    assert.equal(rec.status, "merged", `${scenario}: ${JSON.stringify(rec.park)}`);
    assert.deepEqual(rec.parks, [], scenario);
    assert.deepEqual(rec.upstream.map((u) => [u.state, u.case]), [["unread", why]]);
    assert.ok(lines.some((l) => l.includes(`upstream CI: skipped: not read (${why})`)), `${scenario}: the notice is printed`);
  }
  // `gh` absent, and no origin at all: the scratch repository as every other test here builds it.
  const { ctx, repo } = scratch({ plans: [{ number: "0101", phases: [dev("1")] }], lanes: { a: ["0101"] } });
  ctx.upstreamEnv = { ...process.env, RLX_GH: join(UPSTREAM_FIXTURES, "no-such-gh") };
  await runLanes(ctx);
  const rec = loadState(ctx.stateDir).plans["0101"];
  assert.equal(rec.status, "merged", JSON.stringify(rec.park));
  assert.deepEqual(rec.upstream.map((u) => u.case), ["no origin remote"]);
  assert.ok(resolveCommit("v0.1.1", repo));
});

test("main advancing on a conflicting change between close and merge runs one merge session, and merges", async () => {
  const { ctx, repo } = scratch({ plans: [{ number: "0101", phases: [dev("1")] }], lanes: { a: ["0101"] } });
  ctx.beforeMerge = async () => {
    writeFileSync(join(repo, "phase-0101-1.txt"), "a different phase 1 on main\n");
    sh(["add", "phase-0101-1.txt"], repo);
    sh(["commit", "-q", "-m", "feat: a conflicting commit on main"], repo);
  };
  await runLanes(ctx);
  const rec = loadState(ctx.stateDir).plans["0101"];
  assert.equal(rec.status, "merged", JSON.stringify(rec.park));
  assert.deepEqual(kinds(rec), ["readiness:architect", "implement:dev", "review:architect", "close:architect", "merge:dev"]);
  assert.deepEqual(rec.steps[4].paths, ["phase-0101-1.txt"]);
  assert.deepEqual(rec.merges.map((m) => [m.where, m.session]), [["remerge", true]]);
  assert.deepEqual(rec.gates.map((g) => g.label), ["pre-review", "post-close", "remerge"]);
  assert.equal(readFileSync(join(repo, "phase-0101-1.txt"), "utf8"), "resolved by the merge session\n");
  assert.equal(resolveCommit("v0.1.1", repo), resolveCommit("main", repo), "the tag moved onto the resolved tip");
});

test("a conflict the owner resolves in the lane is gated and re-tagged before main moves", async () => {
  // The merge session parks, which is what leaves the conflict to the owner.
  const { ctx, repo } = scratch({ plans: [{ number: "0101", phases: [dev("1")] }], lanes: { a: ["0101"] }, spec: { "0101": { mergeParks: true } } });
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

test("two lanes' reviews run at once, and their closes serialize on the close lock", async () => {
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
  const order = ev.map((e) => `${e.plan}:${e.event}`).join(" ");
  // No lock over a review (ADR-0248): the second review starts before the first one ends.
  const reviewer = ev.find((e) => e.event === "review-start").plan;
  const other = reviewer === "0101" ? "0102" : "0101";
  assert.ok(at(other, "review-start") < at(reviewer, "review-end"), `the two reviews overlapped: ${order}`);
  // The close lock: the second close starts only after the first plan's fast-forward.
  const first = ev.find((e) => e.event === "close-start").plan;
  const second = first === "0101" ? "0102" : "0101";
  assert.ok(at(first, "conductor-ff") >= 0);
  assert.ok(at(second, "close-start") > at(first, "conductor-ff"), `the second close started only after the first fast-forward: ${order}`);
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
  assert.equal(rec.steps[1].result.spendUsd, 7.5);
  assert.equal(rec.steps[1].result.subtype, "error_max_budget_usd");

  assert.match(history("### Failed and parked"), /^- \*\*0101\*\* spend cap hit in `0101-02-implement`: spent \$7\.50\.$/m);
  assert.match(history("### Totals"), /^- lane a: 0 merged, 1 parked, \$7\.80\.$/m);
});

test("a session the usage limit ends waits for the reset, continues the same session, and merges", async () => {
  const { ctx, repo, readEvents } = scratch({
    plans: [{ number: "0101", phases: [dev("1"), dev("2")] }],
    lanes: { a: ["0101"] },
    spec: { "0101": { usageLimit: { mode: "implement", resetsInS: 600 } } },
  });
  const slept = [];
  ctx.sleep = async (ms) => slept.push(ms);
  ctx.usageMarginMs = 0;
  const calls = join(ctx.stateDir, "calls.jsonl");
  process.env.FAKE_CLAUDE_LOG = calls;
  await runLanes(ctx);
  delete process.env.FAKE_CLAUDE_LOG;

  const rec = loadState(ctx.stateDir).plans["0101"];
  assert.equal(rec.status, "merged");
  assert.deepEqual(rec.parks, []);
  assert.equal(slept.length, 1);
  assert.ok(slept[0] > 590_000 && slept[0] <= 600_000, `waited ${slept[0]} ms for a reset 600 s off`);

  const step = rec.steps[1];
  assert.equal(step.result.status, "ok");
  assert.equal(step.result.usageWaits.length, 1);
  assert.match(step.result.usageWaits[0].transcript, /0101-02-implement\.jsonl$/);
  assert.match(step.result.transcript, /0101-02-implement-resume-1\.jsonl$/);
  assert.equal(step.result.numTurns, 11, "the turns of both invocations");

  // The second invocation continued the first one's session rather than starting a new one.
  const implement = readFileSync(calls, "utf8").trim().split("\n").map((l) => JSON.parse(l)).filter((c) => c.vars.mode === "implement");
  assert.equal(implement.length, 2);
  assert.ok(!implement[0].args.includes("--resume"));
  const firstId = JSON.parse(readFileSync(join(ctx.stateDir, "transcripts", "0101-02-implement.jsonl"), "utf8").split("\n")[0]).session_id;
  assert.equal(implement[1].args[implement[1].args.indexOf("--resume") + 1], firstId);
  assert.ok(readEvents().some((e) => e.event === "implement-resumed"));
  assert.equal(sh(["show", "main:LIMIT_WIP"], repo), "half a phase", "the half-done work survived into the merge");
  assert.equal(loadState(ctx.stateDir).lanes.a.waitingUntil, undefined);
});

test("a usage limit whose reset is too far off parks usage_limit with the CLI's message", async () => {
  const { ctx } = scratch({
    plans: [{ number: "0101", phases: [dev("1")] }],
    lanes: { a: ["0101"] },
    spec: { "0101": { usageLimit: { mode: "implement", resetsInS: 3 * 24 * 3600 } } },
  });
  ctx.sleep = async () => assert.fail("a seven-day reset is not waited out");
  await runLanes(ctx);
  const rec = loadState(ctx.stateDir).plans["0101"];
  assert.equal(rec.status, "parked");
  assert.equal(rec.park.reason, "usage_limit");
  assert.match(rec.park.detail, /You've hit your session limit/);
  assert.match(rec.park.detail, /more than 6 h away/);
  assert.deepEqual(rec.park.dirty.paths, ["LIMIT_WIP"], "the half-done work is left for the owner, not reverted");
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

test("a probe the close leaves red, and its repair cannot turn, parks gate_red at post-close and main does not move", async () => {
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
  assert.match(rec.park.detail, /^check-backlog-claims\.mjs exited 1; still red after one repair session at post-close$/);
  assert.deepEqual(rec.gates.map((g) => [g.label, g.ok]), [["pre-review", true], ["post-close", false], ["post-close", false]]);
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
  assert.deepEqual(executed.map((e) => e.by), ["gate 0101-pre-review", "0101-04-close"], "pre-review and the close's gate");
  assert.deepEqual(skipped.map((e) => [e.by, e.green.by]), [
    ["0101-03-review", "gate 0101-pre-review"],
    ["gate 0101-post-close", "0101-04-close"],
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

// ADR-0248 item 4: a red gets one repair session and one re-run before it parks.
test("a red conductor gate runs one repair session, the gate re-runs green, and the plan merges", async () => {
  const { ctx, repo } = scratch({ plans: [{ number: "0101", phases: [dev("1")] }], lanes: { a: ["0101"] }, spec: { "0101": { gateRed: true } } });
  const calls = join(ctx.stateDir, "calls.jsonl");
  process.env.FAKE_CLAUDE_LOG = calls;
  try {
    await runLanes(ctx);
  } finally {
    delete process.env.FAKE_CLAUDE_LOG;
  }
  const rec = loadState(ctx.stateDir).plans["0101"];
  assert.equal(rec.status, "merged", JSON.stringify(rec.park));
  assert.deepEqual(rec.parks, []);
  assert.deepEqual(kinds(rec), ["readiness:architect", "implement:dev", "repair:dev", "review:architect", "close:architect"]);
  assert.deepEqual(rec.gates.map((g) => [g.label, g.ok]), [["pre-review", false], ["pre-review", true], ["post-close", true]]);
  assert.deepEqual(rec.repairs.map((r) => [r.stage, r.unreviewed]), [["pre-review", false]]);
  assert.equal(existsSync(join(repo, "GATE_RED")), false);

  // The session was handed the failing command and the log that holds its output.
  const repair = readFileSync(calls, "utf8").trim().split("\n").map((l) => JSON.parse(l)).find((c) => c.vars.mode === "repair");
  assert.equal(repair.vars["gate-log"], rec.gates[0].failed.log);
  assert.match(repair.vars.failing, /^marker \(.+\) exited 1$/);
  assert.match(readFileSync(join(ctx.stateDir, "prompts", `${rec.steps[2].label}.md`), "utf8"), /Never change an assertion/);
});

test("a repair that leaves the gate red parks gate_red with the second gate's log", async () => {
  const { ctx } = scratch({ plans: [{ number: "0101", phases: [dev("1")] }], lanes: { a: ["0101"] }, spec: { "0101": { gateRed: true, repairFails: true } } });
  await runLanes(ctx);
  const rec = loadState(ctx.stateDir).plans["0101"];
  assert.equal(rec.status, "parked");
  assert.equal(rec.park.reason, "gate_red");
  assert.match(rec.park.detail, /still red after one repair session at pre-review$/);
  assert.deepEqual(rec.gates.map((g) => [g.label, g.ok]), [["pre-review", false], ["pre-review", false]]);
  assert.equal(rec.park.read, rec.gates[1].failed.log);
  assert.notEqual(rec.gates[1].failed.log, rec.gates[0].failed.log);
  assert.deepEqual(kinds(rec), ["readiness:architect", "implement:dev", "repair:dev"]);
});

test("a red post-close repaired once merges with the tag on the repaired tip, and the digest names the repair unreviewed", async () => {
  const { ctx, repo, digest } = scratch({ plans: [{ number: "0101", phases: [dev("1")] }], lanes: { a: ["0101"] }, spec: { "0101": { closeRed: true } } });
  await runLanes(ctx);
  const rec = loadState(ctx.stateDir).plans["0101"];
  assert.equal(rec.status, "merged", JSON.stringify(rec.park));
  assert.deepEqual(kinds(rec), ["readiness:architect", "implement:dev", "review:architect", "close:architect", "repair:dev"]);
  assert.deepEqual(rec.gates.map((g) => [g.label, g.ok]), [["pre-review", true], ["post-close", false], ["post-close", true]]);
  const [repair] = rec.repairs;
  assert.equal(repair.stage, "post-close");
  assert.equal(repair.unreviewed, true);
  assert.equal(resolveCommit("v0.1.1", repo), repair.commits[0], "the tag moved onto the repaired tip");
  assert.equal(tagObjectType("v0.1.1", repo), "tag");
  assert.equal(resolveCommit("main", repo), repair.commits[0]);
  const needs = digest("## Needs you");
  assert.match(needs, /^1 unreviewed repair\.$/m);
  assert.ok(needs.includes(`- **0101 reached main with an unreviewed repair**: \`${repair.commits[0].slice(0, 7)}\` at post-close.`), needs);
});

test("a fourth red in one plan parks without a repair session", async () => {
  const { ctx } = scratch({ plans: [{ number: "0101", phases: [dev("1")] }], lanes: { a: ["0101"] }, spec: { "0101": { gateRed: true } } });
  const rec0 = planRecord(ctx.state, "0101");
  rec0.repairs = [1, 2, 3].map((n) => ({ stage: `fix-${n}`, commits: [], unreviewed: false, at: "2026-09-24T10:00:00.000Z" }));
  await runLanes(ctx);
  const rec = loadState(ctx.stateDir).plans["0101"];
  assert.equal(rec.status, "parked");
  assert.equal(rec.park.reason, "gate_red");
  assert.match(rec.park.detail, /no repair session, the plan has run 3 already$/);
  assert.deepEqual(kinds(rec), ["readiness:architect", "implement:dev"]);
});

// Backlog 0229: a review that commits its close and then loses its outcome leaves the branch closed
// and the record open. Deciding from the record alone would start round 1 again on a plan already
// under done/, and write a second close, a second version and a second tag.

test("a close that landed without an outcome is adopted on the next run, with no second review", async () => {
  const { ctx, repo } = scratch({
    plans: [{ number: "0101", phases: [dev("1")] }],
    lanes: { a: ["0101"] },
    spec: { "0101": { loseOutcome: "close" } },
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
  assert.deepEqual(kinds(rec), ["readiness:architect", "implement:dev", "review:architect", "close:architect"], "the second run started no session at all");
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
    spec: { "0101": { loseOutcome: "close", dirtyClose: true } },
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
  assert.deepEqual(kinds(rec), ["readiness:architect", "implement:dev", "review:architect", "close:architect"], "and no second review ran");
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

  assert.deepEqual(kinds(rec), ["readiness:architect", "implement:dev"], "one session, for Phase 1 alone");
  assert.deepEqual(rec.steps[1].phases, ["1"], "the range stopped in front of Phase 2");
  assert.equal(rec.park.reason, "claude_dir");
  assert.equal(rec.park.phase, "2");
  // The truncated run is not the plan's last, so that session was not asked to write a close block.
  assert.equal(readFileSync(join(ctx.stateDir, "prompts", `${rec.steps[1].label}.md`), "utf8").includes("RLX-CONDUCTOR-LAST-RUN: no"), true);
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
  // Gate lines only: the close's upstream read prints its own `skipped: not read` notice here.
  const skipped = out.filter((l) => / gate {3}.* skipped: /.test(l));
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

// ADR-0250: a resident run. Each of these ends through `stopRequested` once the scenario has played
// out, or after a bounded number of idle looks so a regression fails rather than hangs.

/** Marks `id`'s log row done in the lane and commits it, as the owner does after a human phase. */
function markDone(wt, plan, id) {
  const planPath = join(wt, "docs", "plans", `${plan}-fixture.md`);
  writeFileSync(planPath, readFileSync(planPath, "utf8").replace(new RegExp(`^\\| ${id} — Step ${id} \\| human \\| not started \\|`, "m"), `| ${id} — Step ${id} | human | done |`));
  sh(["add", `docs/plans/${plan}-fixture.md`], wt);
  sh(["commit", "-q", "-m", `docs(plans): phase ${id} done by the owner`], wt);
}

/** Makes `ctx` resident, stopping once `done()` holds or after `maxLooks` idle looks. */
function resident(ctx, { done, maxLooks = 200, onLook = () => {} }) {
  ctx.resident = true;
  ctx.idlePollMs = 20;
  let looks = 0;
  ctx.onIdleLook = (lane) => {
    looks += 1;
    onLook(lane, looks);
  };
  ctx.stopRequested = () => done() || looks >= maxLooks;
  return { looks: () => looks };
}

const selfResumeEntries = (ctx) => (readFileSync(statePaths(ctx.stateDir).inbox, "utf8").match(/^## .* resumed itself from .*$/gm) ?? []);

test("a resident run resumes a human_phase park the lane's log settles while it is up, and merges it in the same run", async () => {
  const { ctx } = scratch({ plans: [{ number: "0101", phases: [dev("1"), human("2"), dev("3")] }], lanes: { a: ["0101"] } });
  let marked = false;
  const r = resident(ctx, {
    done: () => ctx.state.plans["0101"]?.status === "merged",
    onLook: () => {
      const rec = ctx.state.plans["0101"];
      if (marked || rec?.status !== "parked") return;
      markDone(rec.worktree, "0101", "2");
      marked = true;
    },
  });
  await runLanes(ctx);
  const rec = loadState(ctx.stateDir).plans["0101"];

  assert.equal(rec.status, "merged", JSON.stringify(rec.park));
  assert.ok(r.looks() < 200, "the run ended on the merge, not on the look bound");
  assert.equal(loadState(ctx.stateDir).runs.length, 1, "one runLanes call");
  assert.deepEqual(kinds(rec), ["readiness:architect", "implement:dev", "implement:dev", "review:architect", "close:architect"]);
  assert.deepEqual(rec.parks.map((p) => p.reason), ["human_phase"]);
  assert.deepEqual(rec.selfResumes.map((x) => x.reason), ["human_phase"]);
  const entries = selfResumeEntries(ctx);
  assert.equal(entries.length, 1, entries.join("\n"));
  assert.match(readFileSync(statePaths(ctx.stateDir).inbox, "utf8"), /^- \*\*Settled:\*\* Phase 2 reads done in the plan's ## Implementation log$/m);
});

test("a resident run leaves a settled human_phase park alone while its worktree is dirty", async () => {
  const { ctx } = scratch({ plans: [{ number: "0101", phases: [dev("1"), human("2"), dev("3")] }], lanes: { a: ["0101"] } });
  let markedAt = null;
  resident(ctx, {
    done: () => false,
    maxLooks: 15,
    onLook: (lane, looks) => {
      const rec = ctx.state.plans["0101"];
      if (markedAt !== null || rec?.status !== "parked") return;
      markDone(rec.worktree, "0101", "2");
      writeFileSync(join(rec.worktree, "scratch-notes.txt"), "the owner's working file\n");
      markedAt = looks;
    },
  });
  await runLanes(ctx);
  const rec = loadState(ctx.stateDir).plans["0101"];
  assert.ok(markedAt !== null && markedAt < 15, "the row was marked while the run was up");
  assert.equal(rec.status, "parked");
  assert.equal(rec.park.reason, "human_phase");
  assert.equal(rec.selfResumes, undefined);
  assert.deepEqual(kinds(rec), ["readiness:architect", "implement:dev"]);
  assert.deepEqual(selfResumeEntries(ctx), []);
});

test("a gate_red park never resumes itself, even once the gate would pass", async () => {
  const { ctx } = scratch({ plans: [{ number: "0101", phases: [dev("1")] }], lanes: { a: ["0101"] } });
  ctx.gate = [{ name: "always-red", cmd: [process.execPath, "-e", "process.exit(1)"] }];
  resident(ctx, {
    done: () => false,
    maxLooks: 10,
    onLook: () => {
      ctx.gate = [{ name: "green", cmd: [process.execPath, "-e", "0"] }];
    },
  });
  await runLanes(ctx);
  const rec = loadState(ctx.stateDir).plans["0101"];
  assert.equal(rec.status, "parked");
  assert.equal(rec.park.reason, "gate_red");
  assert.equal(rec.selfResumes, undefined);
  assert.deepEqual(kinds(rec), ["readiness:architect", "implement:dev", "repair:dev"]);
});

test("a resume the owner asks for while the run is up is taken on the lane's next look", async () => {
  const { ctx } = scratch({ plans: [{ number: "0101", phases: [dev("1")] }], lanes: { a: ["0101"] } });
  ctx.gate = [{ name: "always-red", cmd: [process.execPath, "-e", "process.exit(1)"] }];
  let asked = false;
  resident(ctx, {
    done: () => ctx.state.plans["0101"]?.status === "merged",
    onLook: () => {
      if (asked || ctx.state.plans["0101"]?.status !== "parked") return;
      ctx.gate = [{ name: "green", cmd: [process.execPath, "-e", "0"] }];
      askResume(ctx.stateDir, "0101");
      asked = true;
    },
  });
  await runLanes(ctx);
  const rec = loadState(ctx.stateDir).plans["0101"];
  assert.equal(rec.status, "merged", JSON.stringify(rec.park));
  assert.deepEqual(rec.parks.map((p) => p.reason), ["gate_red"]);
  assert.equal(existsSync(statePaths(ctx.stateDir).resumeAsks), false, "the ask was taken");
});

test("a lane at the worktree cap waits for a holder in flight, and starts its plan in the same run once the holder merges", async () => {
  const { ctx, digest } = scratch({
    plans: [
      { number: "0101", phases: [dev("1")] },
      { number: "0102", phases: [dev("1")] },
    ],
    lanes: { a: ["0101"], b: ["0102"] },
    local: { max_open_worktrees: 1 },
  });
  const events = ctx.events;
  let waiting = null;
  ctx.events = (name, data) => {
    events(name, data);
    if (name === "worktree-wait") waiting = digest("## Now");
  };
  await runLanes(ctx);
  const state = loadState(ctx.stateDir);
  assert.equal(state.plans["0101"].status, "merged", JSON.stringify(state.plans["0101"].park));
  assert.equal(state.plans["0102"].status, "merged", JSON.stringify(state.plans["0102"].park));
  assert.equal(state.runs.at(-1).stops, undefined, "a wait is not a stop");
  assert.ok(waiting, "lane b waited");
  assert.match(waiting, /^- lane b: waiting at the worktree cap \(`max_open_worktrees` 1\) to start 0102; the slots are held by 0101\.$/m);
});

test("a plan appended to the queue while a resident run is up is started", async () => {
  const { ctx, repo, digest } = scratch({
    plans: [
      { number: "0101", phases: [dev("1")] },
      { number: "0102", phases: [dev("1")] },
    ],
    lanes: { a: ["0101"] },
  });
  let lanes = { a: ["0101"] };
  ctx.reloadQueue = () => validateQueue({ lanes }, repo, new Set(Object.keys(ctx.state.plans)));
  let watching = null;
  resident(ctx, {
    done: () => ctx.state.plans["0102"]?.status === "merged",
    onLook: () => {
      if (watching) return;
      watching = digest("## Now");
      lanes = { a: ["0101", "0102"] };
    },
  });
  await runLanes(ctx);
  const state = loadState(ctx.stateDir);
  assert.equal(state.plans["0101"].status, "merged");
  assert.equal(state.plans["0102"].status, "merged", JSON.stringify(state.plans["0102"]?.park));
  assert.equal(state.runs.length, 1);
  assert.match(watching, /^- lane a: idle, watching the queue\.$/m);
});

test("a run whose spend reaches run_budget_usd pauses: the plan in flight merges and no other starts", async () => {
  const { ctx } = scratch({
    plans: [
      { number: "0101", phases: [dev("1")] },
      { number: "0102", phases: [dev("1")] },
    ],
    lanes: { a: ["0101", "0102"] },
    local: { run_budget_usd: 1.5 },
  });
  resident(ctx, { done: () => false, maxLooks: 50 });
  await runLanes(ctx);
  const state = loadState(ctx.stateDir);
  assert.equal(state.plans["0101"].status, "merged", JSON.stringify(state.plans["0101"].park));
  assert.equal(state.plans["0102"], undefined, "the spent budget held 0102 back");
  const run = state.runs.at(-1);
  assert.equal(run.paused.reason, "run_budget");
  assert.deepEqual(run.paused.lanes, ["a"]);
  assert.deepEqual(run.notStarted, [{ plan: "0102", lane: "a", reason: "paused" }]);
});

test("selfResumeWhy: usage_limit once its reset passes, studio_install hourly and three times, the owner's reasons never", async () => {
  const { selfResumeWhy, STUDIO_RETRIES } = await import("../lib/lane.mjs");
  const at = "2026-09-24T10:00:00.000Z";
  const t0 = Date.parse(at);
  const rec = (park, extra = {}) => ({ plan: "0101", worktree: null, park: { phase: null, at, ...park }, ...extra });
  const resetsAt = t0 / 1000 + 600;
  assert.equal(selfResumeWhy(rec({ reason: "usage_limit", resetsAt }), "/nowhere", t0 + 60_000), null);
  assert.match(selfResumeWhy(rec({ reason: "usage_limit", resetsAt }), "/nowhere", t0 + 20 * 60_000), /usage window reopened/);
  assert.equal(selfResumeWhy(rec({ reason: "usage_limit" }), "/nowhere", t0 + 24 * 3600_000), null, "no recorded reset is the owner's");

  assert.equal(selfResumeWhy(rec({ reason: "studio_install" }), "/nowhere", t0 + 30 * 60_000), null);
  assert.match(selfResumeWhy(rec({ reason: "studio_install" }), "/nowhere", t0 + 61 * 60_000), /retry 1 of 3/);
  const spent = Array.from({ length: STUDIO_RETRIES }, () => ({ reason: "studio_install" }));
  assert.equal(selfResumeWhy(rec({ reason: "studio_install" }, { selfResumes: spent }), "/nowhere", t0 + 61 * 60_000), null);

  for (const reason of ["gate_red", "review_failed", "disagreement", "plan_wrong", "question", "stop_condition", "cli_contract", "budget", "api", "merge_conflict"]) {
    assert.equal(selfResumeWhy(rec({ reason }), "/nowhere", t0 + 48 * 3600_000), null, reason);
  }
});

test("parkStillTrue: an owed row settles a human_phase park only on a phase marked Blocks merge: no", async () => {
  const { parkStillTrue, selfResumeWhy } = await import("../lib/lane.mjs");
  const at = "2026-09-24T10:00:00.000Z";
  const rec = { plan: "0101", worktree: null, park: { reason: "human_phase", phase: "2", at } };
  const refused = /^Phase 2 is still not marked done in the ## Implementation log of docs\/plans\/0101-fixture\.md in .*; commit the row there first$/;
  const withRow = (blocksMerge, state) => {
    const repo = tmp();
    writePlan(repo, { number: "0101", phases: [dev("1"), { id: "2", owner: "human", blocksMerge }, dev("3")], rows: { 2: { state } } });
    return repo;
  };

  const owedNonBlocking = withRow("no", "owed");
  assert.equal(parkStillTrue(rec, owedNonBlocking), null);
  assert.equal(selfResumeWhy(rec, owedNonBlocking), "Phase 2 reads owed in the plan's ## Implementation log");
  assert.match(parkStillTrue(rec, withRow("no", "not started")), refused);
  assert.match(parkStillTrue(rec, withRow(undefined, "owed")), refused, "a bare owed row does not pass a blocking phase");
  assert.match(parkStillTrue(rec, withRow("yes", "owed")), refused, "nor does one on a phase marked Blocks merge: yes");
  assert.equal(parkStillTrue(rec, withRow(undefined, "done")), null);
});

// ADR-0249.
test("a human phase marked Blocks merge: no is owed: the plan merges with the phases after it, the row reads owed, and the digest says so", async () => {
  const phases = [dev("1"), { id: "2", owner: "human", blocksMerge: "no" }, dev("3")];
  const { ctx, repo, digest } = scratch({ plans: [{ number: "0101", phases }], lanes: { a: ["0101"] } });
  await runLanes(ctx);
  const rec = loadState(ctx.stateDir).plans["0101"];
  assert.equal(rec.status, "merged", JSON.stringify(rec.park));
  assert.deepEqual(rec.parks, []);
  assert.deepEqual(kinds(rec), ["readiness:architect", "implement:dev", "implement:dev", "review:architect", "close:architect"]);
  assert.deepEqual(rec.steps.map((s) => s.phases ?? null), [null, ["1"], ["3"], null, null]);
  assert.deepEqual(rec.owed.map((o) => o.phase), ["2"]);
  assert.ok(existsSync(join(repo, "phase-0101-3.txt")));

  const closed = findPlan(repo, "0101");
  assert.ok(closed.done);
  assert.deepEqual(readPlanFile(closed.path).log.rows.map((r) => [r.id, r.state.split(" ")[0]]), [["1", "committed"], ["2", "owed"], ["3", "committed"]]);
  const owedLines = () => digest("## Needs you").split("\n").filter((l) => l.includes(" owes Phase "));
  assert.equal(owedLines().length, 1, digest("## Needs you"));
  assert.match(owedLines()[0], /^- \*\*0101 owes Phase 2\*\* \(Step 2\): merged without it/);

  // The owner does the phase and marks the row done on main: the line goes, and state/ is not written.
  const stateBefore = readFileSync(statePaths(ctx.stateDir).file, "utf8");
  writeFileSync(closed.path, readFileSync(closed.path, "utf8").replace("| human | owed |", "| human | done |"));
  sh(["commit", "-q", "-am", "docs(plans): 0101 Phase 2 done on the device"], repo);
  ctx.onChange();
  assert.equal(owedLines().length, 0, digest("## Needs you"));
  assert.equal(readFileSync(statePaths(ctx.stateDir).file, "utf8"), stateBefore);
});

test("the same plan without Blocks merge parks human_phase at Phase 2, as before", async () => {
  const { ctx } = scratch({ plans: [{ number: "0101", phases: [dev("1"), human("2"), dev("3")] }], lanes: { a: ["0101"] } });
  await runLanes(ctx);
  const rec = loadState(ctx.stateDir).plans["0101"];
  assert.equal(rec.status, "parked");
  assert.equal(rec.park.reason, "human_phase");
  assert.equal(rec.park.phase, "2");
  assert.equal(rec.owed, undefined);
});

// ADR-0248 items 2 and 3: main merges into the lane before pre-review, and a conflict gets one merge
// session wherever it happens.

/** Commits `text` as phase-0101-1.txt on main, which conflicts with the lane's own Phase 1. */
function conflictOnMain(repo, text) {
  writeFileSync(join(repo, "phase-0101-1.txt"), text);
  sh(["add", "phase-0101-1.txt"], repo);
  sh(["commit", "-q", "-m", "feat: a conflicting commit on main"], repo);
}

/** Commits a conflict on main while the implement session runs, so the early merge meets it. */
function conflictDuringImplement(ctx, repo) {
  const events = ctx.events;
  ctx.events = (name, data) => {
    events(name, data);
    if (name === "implement-step") conflictOnMain(repo, "main's own phase 1\n");
  };
}

test("a conflict on main before pre-review runs one merge session, then the gate, and the review sees the merged tip", async () => {
  const { ctx, repo } = scratch({ plans: [{ number: "0101", phases: [dev("1")] }], lanes: { a: ["0101"] } });
  conflictDuringImplement(ctx, repo);
  const calls = join(ctx.stateDir, "calls.jsonl");
  process.env.FAKE_CLAUDE_LOG = calls;
  try {
    await runLanes(ctx);
  } finally {
    delete process.env.FAKE_CLAUDE_LOG;
  }
  const rec = loadState(ctx.stateDir).plans["0101"];
  assert.equal(rec.status, "merged", JSON.stringify(rec.park));
  assert.deepEqual(kinds(rec), ["readiness:architect", "implement:dev", "merge:dev", "review:architect", "close:architect"]);
  assert.deepEqual(rec.merges.map((m) => [m.where, m.session]), [["pre-review", true]]);
  assert.deepEqual(rec.gates.map((g) => g.label), ["pre-review", "post-close"]);
  assert.ok(Date.parse(rec.gates[0].at) >= Date.parse(rec.steps[2].ended), "the gate ran after the merge session");

  const merge = rec.merges[0].commit;
  const review = readFileSync(calls, "utf8").trim().split("\n").map((l) => JSON.parse(l)).find((c) => c.vars.mode === "review");
  assert.equal(review.args[review.args.indexOf("-p") + 1], `/architect conductor review plan 0101 round 1 at ${merge}`);
  assert.equal(readFileSync(join(repo, "phase-0101-1.txt"), "utf8"), "resolved by the merge session\n");
});

test("a merge session that leaves a conflict marker parks disagreement", async () => {
  const { ctx, repo } = scratch({ plans: [{ number: "0101", phases: [dev("1")] }], lanes: { a: ["0101"] }, spec: { "0101": { mergeMarker: true } } });
  conflictDuringImplement(ctx, repo);
  await runLanes(ctx);
  const rec = loadState(ctx.stateDir).plans["0101"];
  assert.equal(rec.status, "parked");
  assert.equal(rec.park.reason, "disagreement");
  assert.match(rec.park.detail, /^merge session at pre-review: conflict markers left in phase-0101-1\.txt:1/);
  assert.deepEqual(kinds(rec), ["readiness:architect", "implement:dev", "merge:dev"], "no gate and no review on a tree with markers");
  assert.deepEqual(rec.gates ?? [], []);
});

test("a second conflict in the same plan gets a second merge session rather than a park", async () => {
  const { ctx, repo } = scratch({ plans: [{ number: "0101", phases: [dev("1")] }], lanes: { a: ["0101"] } });
  conflictDuringImplement(ctx, repo);
  ctx.beforeMerge = async () => conflictOnMain(repo, "main's phase 1, changed again\n");
  await runLanes(ctx);
  const rec = loadState(ctx.stateDir).plans["0101"];
  assert.equal(rec.status, "merged", JSON.stringify(rec.park));
  assert.deepEqual(kinds(rec), ["readiness:architect", "implement:dev", "merge:dev", "review:architect", "close:architect", "merge:dev"]);
  assert.deepEqual(rec.merges.map((m) => m.where), ["pre-review", "remerge"]);
  assert.deepEqual(rec.parks, []);
  assert.equal(resolveCommit("v0.1.1", repo), resolveCommit("main", repo));
});

// ADR-0248 item 5: the review ends on its verdict with no lock held, the close is its own session
// under the lock, and a clean verdict outlives a close-time park.

test("a clean plan's steps read implement, review, close, and the close lock is waited for only after the review ends", async () => {
  const { ctx } = scratch({ plans: [{ number: "0101", phases: [dev("1")] }], lanes: { a: ["0101"] } });
  await runLanes(ctx);
  const rec = loadState(ctx.stateDir).plans["0101"];
  assert.equal(rec.status, "merged", JSON.stringify(rec.park));
  assert.deepEqual(kinds(rec), ["readiness:architect", "implement:dev", "review:architect", "close:architect"]);
  const review = rec.steps[2];
  const waits = rec.lockWaits.filter((w) => w.lock === "close");
  assert.equal(waits.length, 1, "one close-lock take, for the close");
  assert.ok(Date.parse(waits[0].at) - waits[0].ms >= Date.parse(review.ended), "the wait began after the review ended");
  assert.equal(rec.verdicts.length, 1);
  assert.ok(rec.verdicts[0].graded, "the verdict carries the tip it graded");
});

test("a review that commits parks disagreement", async () => {
  const { ctx } = scratch({ plans: [{ number: "0101", phases: [dev("1")] }], lanes: { a: ["0101"] }, spec: { "0101": { reviewCommits: true } } });
  await runLanes(ctx);
  const rec = loadState(ctx.stateDir).plans["0101"];
  assert.equal(rec.park.reason, "disagreement");
  assert.match(rec.park.detail, /a review commits nothing/);
});

/** Runs 0101 to a close that parks check_red, then hands back the parked record for the resume. */
async function parkedAtClose() {
  const s = scratch({ plans: [{ number: "0101", phases: [dev("1")] }], lanes: { a: ["0101"] }, spec: { "0101": { closeParksOnce: "clippy" } } });
  await runLanes(s.ctx);
  const rec = s.ctx.state.plans["0101"];
  assert.equal(rec.park.reason, "check_red", JSON.stringify(rec.park));
  assert.deepEqual(kinds(rec), ["readiness:architect", "implement:dev", "review:architect", "close:architect"]);
  return { ...s, rec };
}

test("a close that parks, resumed with only a merge of main added, runs no review and one close", async () => {
  const { ctx, repo, rec } = await parkedAtClose();
  // main moves, and the owner merges it into the lane: a merge of main, nothing else.
  writeFileSync(join(repo, "owner-note.txt"), "landed on main meanwhile\n");
  sh(["add", "owner-note.txt"], repo);
  sh(["commit", "-q", "-m", "docs: an unrelated commit on main"], repo);
  sh(["merge", "-q", "--no-edit", "main"], rec.worktree);
  rec.status = "queued";
  rec.park = null;
  await runLanes(ctx);
  const done = loadState(ctx.stateDir).plans["0101"];
  assert.equal(done.status, "merged", JSON.stringify(done.park));
  assert.deepEqual(kinds(done), ["readiness:architect", "implement:dev", "review:architect", "close:architect", "close:architect"]);
  assert.equal(done.verdicts.length, 1, "the round-1 verdict was reused");
});

test("the same resume after a non-merge fix commit in the lane runs a round-2 review", async () => {
  const { ctx, rec } = await parkedAtClose();
  writeFileSync(join(rec.worktree, "owner-fix.txt"), "a fix nobody reviewed\n");
  sh(["add", "owner-fix.txt"], rec.worktree);
  sh(["commit", "-q", "-m", "fix: the owner's hand fix"], rec.worktree);
  rec.status = "queued";
  rec.park = null;
  await runLanes(ctx);
  const done = loadState(ctx.stateDir).plans["0101"];
  assert.equal(done.status, "merged", JSON.stringify(done.park));
  assert.deepEqual(kinds(done), ["readiness:architect", "implement:dev", "review:architect", "close:architect", "review:architect", "close:architect"]);
  assert.deepEqual(done.verdicts.map((v) => v.round), [1, 2]);
});

test("a close-time code conflict runs one merge session, then one close, and merges", async () => {
  const { ctx, repo } = scratch({ plans: [{ number: "0101", phases: [dev("1")] }], lanes: { a: ["0101"] } });
  const events = ctx.events;
  ctx.events = (name, data) => {
    events(name, data);
    if (name === "review-step") conflictOnMain(repo, "main's own phase 1, landed during the review\n");
  };
  await runLanes(ctx);
  const rec = loadState(ctx.stateDir).plans["0101"];
  assert.equal(rec.status, "merged", JSON.stringify(rec.park));
  assert.deepEqual(kinds(rec), ["readiness:architect", "implement:dev", "review:architect", "close:architect", "merge:dev", "close:architect"]);
  assert.deepEqual(rec.merges.map((m) => [m.where, m.session]), [["close", true]]);
  assert.deepEqual(rec.parks, []);
  assert.equal(readFileSync(join(repo, "phase-0101-1.txt"), "utf8"), "resolved by the merge session\n");
  assert.equal(resolveCommit("v0.1.1", repo), resolveCommit("main", repo));
});

// ADR-0248 item 1: a read-only readiness session before the first implement session.

test("a readiness session that parks plan_wrong leaves the plan with no implement step", async () => {
  const { ctx } = scratch({ plans: [{ number: "0101", phases: [dev("1"), dev("2")] }], lanes: { a: ["0101"] }, spec: { "0101": { readiness: "plan_wrong" } } });
  await runLanes(ctx);
  const rec = loadState(ctx.stateDir).plans["0101"];
  assert.equal(rec.status, "parked");
  assert.equal(rec.park.reason, "plan_wrong");
  assert.equal(rec.park.phase, "1");
  assert.match(rec.park.detail, /Phase 1's What and Done when name different stages/);
  assert.deepEqual(kinds(rec), ["readiness:architect"]);
  assert.equal(rec.readiness, undefined, "a park is never remembered as passing");
});

test("a readiness session that commits parks disagreement", async () => {
  const { ctx } = scratch({ plans: [{ number: "0101", phases: [dev("1")] }], lanes: { a: ["0101"] }, spec: { "0101": { readinessCommits: true } } });
  await runLanes(ctx);
  const rec = loadState(ctx.stateDir).plans["0101"];
  assert.equal(rec.park.reason, "disagreement");
  assert.match(rec.park.detail, /the readiness session changed the lane/);
  assert.deepEqual(kinds(rec), ["readiness:architect"]);
});

test("resuming after a readiness park with the plan unchanged runs readiness again", async () => {
  const { ctx } = scratch({ plans: [{ number: "0101", phases: [dev("1")] }], lanes: { a: ["0101"] }, spec: { "0101": { readiness: "plan_wrong" } } });
  await runLanes(ctx);
  const parked = ctx.state.plans["0101"];
  assert.equal(parked.park.reason, "plan_wrong");
  parked.status = "queued";
  parked.park = null;
  await runLanes(ctx);
  const rec = loadState(ctx.stateDir).plans["0101"];
  assert.deepEqual(kinds(rec), ["readiness:architect", "readiness:architect"]);
});

test("resuming a plan that parked after readiness, its contract unchanged, runs no second readiness; an edited phase does", async () => {
  const { ctx } = scratch({ plans: [{ number: "0101", phases: [dev("1"), human("2"), dev("3"), human("4"), dev("5")] }], lanes: { a: ["0101"] } });
  await runLanes(ctx);
  const rec = ctx.state.plans["0101"];
  assert.equal(rec.park.reason, "human_phase");
  assert.ok(rec.readiness?.hash, "the ready verdict is recorded with the contract's hash");

  // The owner does Phase 2 and marks its row: the log moved, the contract did not.
  markDone(rec.worktree, "0101", "2");
  rec.status = "queued";
  rec.park = null;
  await runLanes(ctx);
  assert.equal(rec.park.phase, "4");
  assert.deepEqual(kinds(rec), ["readiness:architect", "implement:dev", "implement:dev"], "no second readiness");

  // The owner then edits Phase 5's contract before resuming: the old verdict no longer holds.
  const planPath = join(rec.worktree, "docs", "plans", "0101-fixture.md");
  writeFileSync(planPath, readFileSync(planPath, "utf8").replace("- **What:** phase 5.", "- **What:** phase 5, now with a second output."));
  sh(["commit", "-q", "-am", "docs(plans): the owner edits Phase 5"], rec.worktree);
  markDone(rec.worktree, "0101", "4");
  rec.status = "queued";
  rec.park = null;
  await runLanes(ctx);
  const done = loadState(ctx.stateDir).plans["0101"];
  assert.equal(done.status, "merged", JSON.stringify(done.park));
  assert.deepEqual(kinds(done), [
    "readiness:architect",
    "implement:dev",
    "implement:dev",
    "readiness:architect",
    "implement:dev",
    "review:architect",
    "close:architect",
  ]);
});
