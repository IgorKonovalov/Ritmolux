// The operator surface: `run`, `status`, `digest`, `resume`, `park`, `finding` and `abort` against a
// scratch repository and the fake CLI, driven through the same `main` the command line calls.

import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";

import { VERIFIED_CLI, cliVerdict, main, paths } from "../conductor.mjs";
import { resumeCommand } from "../lib/inbox.mjs";
import { loadState, statePaths } from "../lib/state.mjs";
import { FAKE, TEST_DIR, TOOL_DIR, tmp, writePlan } from "./helpers.mjs";

const QUEUE_NOTICE = "conductor: notice: plan 0090: already merged (0090-fixture.md is under docs/plans/done/); `prune` drops it from the queue";

function sh(args, cwd) {
  const r = spawnSync("git", args, { cwd, encoding: "utf8" });
  assert.equal(r.status, 0, `git ${args.join(" ")}: ${r.stderr}`);
  return r.stdout.trim();
}

const dev = (id) => ({ id, owner: "dev" });
const human = (id) => ({ id, owner: "human" });

function setup(plans, lanes, { gate, maxOpenWorktrees = 3, spec = {}, stopRequested, idlePollMs } = {}) {
  const repo = tmp("rlx-cli-repo-");
  sh(["init", "-q", "-b", "main"], repo);
  for (const [k, v] of [["user.email", "t@example.invalid"], ["user.name", "T"], ["commit.gpgsign", "false"], ["tag.gpgSign", "false"], ["core.autocrlf", "false"]]) {
    sh(["config", k, v], repo);
  }
  writeFileSync(join(repo, "VERSION"), "0.1.0\n");
  for (const plan of plans) writePlan(repo, plan);
  sh(["add", "VERSION", "docs"], repo);
  sh(["commit", "-q", "-m", "init"], repo);

  // The tool directory sits outside the repository, so its state never dirties the main checkout.
  const toolDir = tmp("rlx-cli-tool-");
  writeFileSync(join(toolDir, "queue.json"), JSON.stringify({ lanes }));
  writeFileSync(join(toolDir, "local.json"), JSON.stringify({ budget_usd: { implement: 5, fix: 3, review: 4, merge: 2 }, run_budget_usd: 60, max_open_worktrees: maxOpenWorktrees }));
  const p = { ...paths({ repo, toolDir }), settings: join(TOOL_DIR, "settings.conductor.json"), prompts: join(TOOL_DIR, "prompts"), withLock: join(TOOL_DIR, "with-lock.mjs") };

  const specFile = join(toolDir, "spec.json");
  writeFileSync(specFile, JSON.stringify({ plans: spec }));
  process.env.FAKE_CLAUDE_SCENARIO = join(TEST_DIR, "lane-scenario.mjs");
  process.env.FAKE_LANE_SPEC = specFile;
  process.env.FAKE_EVENTS = join(toolDir, "events.jsonl");
  process.env.FAKE_CLAUDE_VERSION = "2.1.270 (Claude Code)";

  const out = [];
  const err = [];
  const o = {
    p,
    claude: FAKE,
    gate: gate?.(p) ?? [{ name: "noop", cmd: [process.execPath, "-e", "0"] }],
    worktreeRoot: tmp("rlx-cli-lanes-"),
    lockDir: tmp("rlx-cli-locks-"),
    lockPollMs: 20,
    pollMs: 50,
    idlePollMs,
    stopRequested,
    signals: false,
    log: (s) => out.push(s),
    err: (s) => err.push(s),
  };
  const cli = async (...argv) => {
    out.length = 0;
    err.length = 0;
    const code = await main(argv, o);
    return { code, out: [...out], err: [...err] };
  };
  return { repo, p, cli };
}

test("run --once runs one plan; a second run skips the park and merges the next; the inbox has one entry per park", async () => {
  const { p, cli } = setup(
    [
      { number: "0101", phases: [dev("1"), human("2")] },
      { number: "0102", phases: [dev("1")] },
    ],
    { a: ["0101", "0102"] },
  );
  const first = await cli("run", "--lane", "a", "--once");
  assert.equal(first.code, 0, first.err.join("\n"));
  let state = loadState(p.stateDir);
  assert.equal(state.plans["0101"].status, "parked");
  assert.equal(state.plans["0102"], undefined, "--once stopped after one plan");
  assert.equal(existsSync(join(p.stateDir, "conductor.pid")), false, "the pid file is removed when the run ends");
  assert.ok(first.out.at(-1).startsWith("digest: "));

  const second = await cli("run", "--lane", "a", "--until-idle");
  assert.equal(second.code, 0, second.err.join("\n"));
  state = loadState(p.stateDir);
  assert.equal(state.plans["0102"].status, "merged");
  assert.equal(state.plans["0101"].status, "parked");
  assert.match(second.out.join("\n"), /1 merged, 1 parked\. Nothing was pushed\./);

  const inbox = readFileSync(statePaths(p.stateDir).inbox, "utf8");
  assert.equal((inbox.match(/^## .* parked: /gm) ?? []).length, 1);
  assert.match(inbox, /plan 0101 parked: human_phase/);
  assert.match(inbox, /\*\*Read:\*\* docs\/plans\/0101-fixture\.md Phase 2/);
});

test("status names each lane, every parked plan with its reason, and ends with the digest path", async () => {
  const { p, cli } = setup([{ number: "0101", phases: [dev("1"), human("2")] }], { a: ["0101"], b: [] });
  await cli("run", "--once");
  const s = await cli("status");
  assert.equal(s.code, 0);
  assert.deepEqual(s.out, [
    "conductor: not running",
    "lane a: idle",
    "lane b: idle",
    "parked:",
    "- 0101 (human_phase): Phase 2 is owned by human",
    `digest: ${p.digest}`,
  ]);
});

test("status regenerates a deleted digest byte for byte", async () => {
  const { p, cli } = setup([{ number: "0101", phases: [dev("1")] }], { a: ["0101"] });
  await cli("run", "--until-idle");
  const before = readFileSync(p.digest, "utf8");
  assert.match(before, /^## Needs you\n\nNothing: no park, no lane stopped at the worktree cap, no open finding\.$/m);
  rmSync(p.digest);
  await cli("status");
  assert.equal(readFileSync(p.digest, "utf8"), before);
});

// ADR-0214: the per-run account is one command away rather than on the page, and the flag is what
// creates the file — nothing writes it until it is asked for.
test("digest rewrites the page, and digest --history writes the per-run account beside it", async () => {
  const { p, cli } = setup([{ number: "0101", phases: [dev("1")] }], { a: ["0101"] });
  await cli("run", "--until-idle");
  assert.equal(existsSync(p.digestHistory), false, "no run writes the history page");

  rmSync(p.digest);
  const plain = await cli("digest");
  assert.equal(plain.code, 0);
  assert.deepEqual(plain.out, [`digest: ${p.digest}`]);
  assert.match(readFileSync(p.digest, "utf8"), /^# Conductor digest$/m);

  const history = await cli("digest", "--history");
  assert.equal(history.code, 0);
  assert.deepEqual(history.out, [`history: ${p.digestHistory}`]);
  const text = readFileSync(p.digestHistory, "utf8");
  assert.match(text, /^# Conductor history$/m);
  assert.match(text, /### Closed\n\n- \*\*0101 - Plan 0101 fixture\*\*/);
  assert.ok(!readFileSync(p.digest, "utf8").includes("### Closed"), "the page carries no run's own sections");
});

test("resume refuses a human-phase park the log does not mark done, and accepts the digest's command once it does", async () => {
  const { p, cli } = setup([{ number: "0101", phases: [dev("1"), human("2"), dev("3")] }], { a: ["0101"] });
  await cli("run", "--until-idle");
  const rec = loadState(p.stateDir).plans["0101"];
  assert.equal(rec.park.reason, "human_phase");

  // The exact command the digest and the inbox print.
  const printed = resumeCommand("0101").split(" ").slice(2);
  assert.deepEqual(printed, ["resume", "0101"]);
  const refused = await cli(...printed);
  assert.equal(refused.code, 1);
  assert.match(refused.err[0], /refusing to resume 0101 - its park reason \(human_phase\) still holds: Phase 2 is still not marked done/);
  assert.equal(loadState(p.stateDir).plans["0101"].status, "parked");

  // The owner does the phase and commits its row in the lane.
  const planPath = join(rec.worktree, "docs", "plans", "0101-fixture.md");
  writeFileSync(planPath, readFileSync(planPath, "utf8").replace(/^\| 2 — Step 2 \| human \| not started \|/m, "| 2 — Step 2 | human | done |"));
  sh(["commit", "-q", "-am", "docs(plans): phase 2 done by the owner"], rec.worktree);

  const accepted = await cli(...printed);
  assert.equal(accepted.code, 0, accepted.err.join("\n"));
  assert.equal(loadState(p.stateDir).plans["0101"].status, "queued");

  const finish = await cli("run", "--until-idle");
  assert.equal(finish.code, 0);
  const done = loadState(p.stateDir).plans["0101"];
  assert.equal(done.status, "merged", JSON.stringify(done.park));
  assert.deepEqual(done.steps.map((s) => s.kind), ["implement", "implement", "review"]);
});

// ADR-0214: the page and `status` read the same verdict, so the owner cannot be told a record is
// stale on one and a park on the other.
test("status marks a park the lane has already settled exactly as the digest does", async () => {
  const { p, cli } = setup([{ number: "0101", phases: [dev("1"), human("2"), dev("3")] }], { a: ["0101"] });
  await cli("run", "--until-idle");
  const rec = loadState(p.stateDir).plans["0101"];
  assert.equal(rec.park.reason, "human_phase");

  const before = await cli("status");
  assert.ok(before.out.includes("- 0101 (human_phase): Phase 2 is owned by human"), before.out.join("\n"));
  assert.ok(!readFileSync(p.digest, "utf8").includes("Already settled"));

  // The owner does the phase and commits its row in the lane; nothing tells the conductor.
  const planPath = join(rec.worktree, "docs", "plans", "0101-fixture.md");
  writeFileSync(planPath, readFileSync(planPath, "utf8").replace(/^\| 2 — Step 2 \| human \| not started \|/m, "| 2 — Step 2 | human | done |"));
  sh(["commit", "-q", "-am", "docs(plans): phase 2 done by the owner"], rec.worktree);

  const after = await cli("status");
  assert.ok(
    after.out.includes(
      "- 0101 (human_phase): Phase 2 is owned by human - already settled: Phase 2 now reads `done` in the plan's `## Implementation log`; " +
        "`resume 0101` clears the record",
    ),
    after.out.join("\n"),
  );
  const page = readFileSync(p.digest, "utf8");
  assert.match(page, /^1 already settled\.$/m);
  assert.match(page, /^### Already settled, clear the record\n\n- \*\*0101\*\* \(`human_phase`\) at Phase 2 parked /m);
  assert.equal(loadState(p.stateDir).plans["0101"].status, "parked", "the renderer wrote nothing back");
});

test("run starts again after a merge, and a resumed sibling of the merged plan runs to its merge", async () => {
  const { p, cli } = setup(
    [
      { number: "0101", phases: [dev("1")] },
      { number: "0102", phases: [dev("1"), human("2")] },
    ],
    { a: ["0101", "0102"] },
  );
  const first = await cli("run", "--until-idle");
  assert.equal(first.code, 0, first.err.join("\n"));
  let state = loadState(p.stateDir);
  assert.equal(state.plans["0101"].status, "merged");
  assert.equal(state.plans["0102"].status, "parked");

  // The merged plan stays listed in queue.json and now sits under done/; that must not refuse a run.
  const again = await cli("run", "--until-idle");
  assert.equal(again.code, 0, again.err.join("\n"));
  assert.equal((await cli("check")).code, 0);

  const rec = loadState(p.stateDir).plans["0102"];
  const planPath = join(rec.worktree, "docs", "plans", "0102-fixture.md");
  writeFileSync(planPath, readFileSync(planPath, "utf8").replace(/^\| 2 — Step 2 \| human \| not started \|/m, "| 2 — Step 2 | human | done |"));
  sh(["commit", "-q", "-am", "docs(plans): phase 2 done by the owner"], rec.worktree);
  assert.equal((await cli("resume", "0102")).code, 0);

  const finish = await cli("run", "--until-idle");
  assert.equal(finish.code, 0, finish.err.join("\n"));
  state = loadState(p.stateDir);
  assert.equal(state.plans["0102"].status, "merged", JSON.stringify(state.plans["0102"].park));
});

test("resume refuses a park whose worktree is dirty, naming the paths, and accepts once it is clean", async () => {
  const { p, cli } = setup([{ number: "0101", phases: [dev("1")] }], { a: ["0101"] });
  writeFileSync(join(p.toolDir, "spec.json"), JSON.stringify({ plans: { "0101": { dirtyPark: { untracked: 0 } } } }));
  await cli("run", "--until-idle");
  const rec = loadState(p.stateDir).plans["0101"];
  assert.equal(rec.park.reason, "check_red");
  assert.deepEqual(rec.park.dirty, { paths: ["VERSION"], more: 0 });

  const refused = await cli("resume", "0101");
  assert.equal(refused.code, 1);
  assert.equal(
    refused.err[0],
    `conductor: refusing to resume 0101 - its park reason (check_red) still holds: the worktree ${rec.worktree} has uncommitted changes: \`VERSION\`; commit them, or \`git restore\` them there, first`,
  );
  assert.equal(loadState(p.stateDir).plans["0101"].status, "parked");

  sh(["restore", "VERSION"], rec.worktree);
  const accepted = await cli("resume", "0101");
  assert.equal(accepted.code, 0, accepted.err.join("\n"));
  assert.equal(loadState(p.stateDir).plans["0101"].status, "queued");
});

test("run prints a lane's stop at the worktree cap as it happens", async () => {
  const { cli } = setup(
    [
      { number: "0101", phases: [dev("1"), human("2")] },
      { number: "0102", phases: [dev("1")] },
    ],
    { a: ["0101", "0102"] },
    { maxOpenWorktrees: 1 },
  );
  const r = await cli("run", "--lane", "a", "--until-idle");
  assert.equal(r.code, 0, r.err.join("\n"));
  const stop = r.out.indexOf("conductor: lane a stopped at the worktree cap (max_open_worktrees 1, held by 0101); 0102 not started");
  assert.ok(stop >= 0, r.out.join("\n"));
  assert.ok(stop > r.out.indexOf("conductor: 0101 parked (human_phase)"), "after the park that filled the cap");
  assert.ok(stop < r.out.findIndex((l) => l.startsWith("conductor: run ended")), "before the run ends");
});

test("park parks a queued plan with an inbox entry, and resume queues it again", async () => {
  const { p, cli } = setup([{ number: "0101", phases: [dev("1")] }], { a: ["0101"] });
  const parked = await cli("park", "0101");
  assert.equal(parked.code, 0);
  assert.equal(loadState(p.stateDir).plans["0101"].park.reason, "owner");
  assert.match(readFileSync(statePaths(p.stateDir).inbox, "utf8"), /plan 0101 parked: owner/);
  assert.equal((await cli("park", "0101")).code, 1, "parking twice is refused");
  assert.equal((await cli("resume", "0101")).code, 0);
  assert.equal(loadState(p.stateDir).plans["0101"].status, "queued");
});

test("run refuses a second conductor, and abort with nothing running recovers in-flight steps", async () => {
  const { p, cli } = setup([{ number: "0101", phases: [dev("1")] }], { a: ["0101"] });
  mkdirSync(p.stateDir, { recursive: true });
  writeFileSync(join(p.stateDir, "conductor.pid"), String(process.pid));
  const refused = await cli("run", "--until-idle");
  assert.equal(refused.code, 1);
  assert.match(refused.err.join("\n"), /a conductor is already running \(pid \d+\)/);
  rmSync(join(p.stateDir, "conductor.pid"));

  const state = loadState(p.stateDir);
  state.plans["0101"] = { plan: "0101", status: "running", lane: "a", steps: [{ kind: "implement", started: "2026-09-14T10:00:00.000Z", ended: null }], parks: [], verdicts: [], fixes: [], lockWaits: [] };
  writeFileSync(statePaths(p.stateDir).file, JSON.stringify(state));
  const aborted = await cli("abort");
  assert.equal(aborted.code, 0);
  assert.equal(aborted.out[0], "conductor: not running");
  assert.equal(loadState(p.stateDir).plans["0101"].steps[0].result.status, "interrupted");
});

// ADR-0219. The ask is written by another process while a plan is in flight, so these drive it from
// a gate command: the gate is a child process running inside the lane, mid-run, exactly as `pause`
// would be run from another terminal.
const writesPause = (p) => ({
  name: "ask-pause",
  cmd: [process.execPath, "-e", `require("fs").writeFileSync(${JSON.stringify(statePaths(p.stateDir).pause)}, '{"at":"2026-09-19T09:00:00.000Z"}')`],
});

test("pause asked mid-run lets the plan in flight merge, starts no other, and clears the ask", async () => {
  const { p, cli } = setup(
    [
      { number: "0101", phases: [dev("1")] },
      { number: "0102", phases: [dev("1")] },
    ],
    { a: ["0101", "0102"] },
    { gate: (p) => [writesPause(p)] },
  );
  // Resident, with nothing else to end it: the pause is what ends the run.
  const r = await cli("run", "--lane", "a");
  assert.equal(r.code, 0, r.err.join("\n"));
  const state = loadState(p.stateDir);
  assert.equal(state.plans["0101"].status, "merged", JSON.stringify(state.plans["0101"].park));
  assert.equal(state.plans["0102"], undefined, "the second plan never started");
  assert.deepEqual(state.runs.at(-1).paused.lanes, ["a"]);
  assert.deepEqual(state.runs.at(-1).notStarted, [{ plan: "0102", lane: "a", reason: "paused" }]);
  assert.equal(existsSync(statePaths(p.stateDir).pause), false, "the ask does not outlive the run");
  assert.match(r.out.join("\n"), /^conductor: paused - the plan in flight finished and no further plan was started; the ask is cleared\.$/m);
  assert.match(r.out.join("\n"), /run ended - 1 merged, 0 parked/);
});

test("an ask cancelled before the lane looks again lets the next plan start", async () => {
  // The second gate step is `afterClose`, so it runs only on the close tip — after the ask above and
  // before the lane's next look. It removes the same file `pause --off` removes.
  const { p, cli } = setup(
    [
      { number: "0101", phases: [dev("1")] },
      { number: "0102", phases: [dev("1")] },
    ],
    { a: ["0101", "0102"] },
    {
      gate: (p) => [
        writesPause(p),
        {
          name: "cancel-pause",
          afterClose: true,
          cmd: [process.execPath, "-e", `require("fs").rmSync(${JSON.stringify(statePaths(p.stateDir).pause)}, { force: true })`],
        },
      ],
    },
  );
  const r = await cli("run", "--lane", "a", "--until-idle");
  assert.equal(r.code, 0, r.err.join("\n"));
  const state = loadState(p.stateDir);
  assert.equal(state.plans["0102"].status, "merged", JSON.stringify(state.plans["0102"].park));
  assert.equal(state.runs.at(-1).paused, undefined, "no lane stopped paused");
  assert.doesNotMatch(r.out.join("\n"), /conductor: paused/);
});

test("pause needs a live run, writes the ask the lane reads, names what it waits for, and --off removes it", async () => {
  const { p, cli } = setup([{ number: "0101", phases: [dev("1")] }], { a: ["0101"] });

  const noRun = await cli("pause");
  assert.equal(noRun.code, 1);
  assert.deepEqual(noRun.err, ["conductor: no conductor is running, and a pause does not outlive a run; start the run you want with `run --once`"]);
  assert.equal(existsSync(statePaths(p.stateDir).pause), false, "nothing was written");

  // This test process stands in for the live conductor, and the record for a lane mid-step.
  mkdirSync(p.stateDir, { recursive: true });
  writeFileSync(join(p.stateDir, "conductor.pid"), String(process.pid));
  const state = loadState(p.stateDir);
  state.lanes = { a: { plan: "0101", step: "0101-03-review", stepStarted: new Date(Date.now() - 4 * 60_000).toISOString() }, b: { plan: null, step: null } };
  writeFileSync(statePaths(p.stateDir).file, JSON.stringify(state));

  const asked = await cli("pause");
  assert.equal(asked.code, 0, asked.err.join("\n"));
  assert.deepEqual(asked.out, [
    "conductor: paused - each lane finishes the plan in flight and starts no other. `pause --off` cancels. Waiting for:",
    "- lane a: plan 0101, step 0101-03-review for 4 min",
  ]);
  assert.ok(existsSync(statePaths(p.stateDir).pause));

  // Asking twice keeps the first ask's time rather than resetting it.
  const first = readFileSync(statePaths(p.stateDir).pause, "utf8");
  const again = await cli("pause");
  assert.equal(again.code, 0);
  assert.match(again.out[0], /^conductor: already paused \(asked at .+\)\. Waiting for:$/);
  assert.equal(readFileSync(statePaths(p.stateDir).pause, "utf8"), first);

  const off = await cli("pause", "--off");
  assert.equal(off.code, 0, off.err.join("\n"));
  assert.deepEqual(off.out, ["conductor: the pause is off; a lane starts its next queued plan again"]);
  assert.equal(existsSync(statePaths(p.stateDir).pause), false);
  assert.deepEqual((await cli("pause", "--off")).out, ["conductor: no pause was asked for"]);
  rmSync(join(p.stateDir, "conductor.pid"));
});

test("a run started with an ask left behind by a dead conductor runs normally and says it cleared it", async () => {
  const { p, cli } = setup([{ number: "0101", phases: [dev("1")] }], { a: ["0101"] });
  mkdirSync(p.stateDir, { recursive: true });
  writeFileSync(statePaths(p.stateDir).pause, '{"at":"2026-09-18T20:00:00.000Z","pid":4242}');
  const r = await cli("run", "--lane", "a", "--until-idle");
  assert.equal(r.code, 0, r.err.join("\n"));
  assert.equal(r.out[0], "conductor: a pause left behind by an earlier run was cleared; a pause does not outlive its run");
  const state = loadState(p.stateDir);
  assert.equal(state.plans["0101"].status, "merged", JSON.stringify(state.plans["0101"].park));
  assert.equal(state.runs.at(-1).paused, undefined);
});

// ADR-0220: the committed queue stands alone, and `prune` is the carrier for the one discipline it
// still needs. `0090` stands for a plan merged in an earlier run and never taken off the list.
test("a queue listing a merged plan starts with a notice, and prune drops exactly that entry", async () => {
  const { repo, p, cli } = setup([{ number: "0101", phases: [dev("1")] }], { a: ["0090", "0101"], b: [] });
  writePlan(repo, { number: "0090", phases: [dev("1")], status: "done — closed" }, { done: true });

  const check = await cli("check");
  assert.equal(check.code, 0, check.err.join("\n"));
  assert.deepEqual(check.out, [
    "conductor: notice: plan 0090: already merged (0090-fixture.md is under docs/plans/done/); `prune` drops it from the queue",
    "conductor: preflight OK",
  ]);

  const before = JSON.stringify({ lanes: { a: ["0090", "0101"], b: [] }, plans: { "0101": { after: ["0090"] } } }, null, 2) + "\n";
  writeFileSync(p.queue, before);
  const pruned = await cli("prune");
  assert.equal(pruned.code, 0, pruned.err.join("\n"));
  assert.deepEqual(pruned.out, [
    "conductor: dropped plan 0090 from lane a (0090-fixture.md is under docs/plans/done/)",
    `conductor: ${p.queue} rewritten; commit it.`,
  ]);
  assert.equal(
    readFileSync(p.queue, "utf8"),
    JSON.stringify({ lanes: { a: ["0101"], b: [] }, plans: { "0101": { after: ["0090"] } } }, null, 2) + "\n",
    "only the lane entry moved",
  );
  assert.deepEqual((await cli("check")).out, ["conductor: preflight OK"]);

  // An already-tidy queue is not rewritten: this spelling is not the one `prune` would write.
  const compact = JSON.stringify({ lanes: { a: ["0101"], b: [] } });
  writeFileSync(p.queue, compact);
  const again = await cli("prune");
  assert.equal(again.code, 0, again.err.join("\n"));
  assert.deepEqual(again.out, ["conductor: the queue lists no merged plan; nothing to prune"]);
  assert.equal(readFileSync(p.queue, "utf8"), compact);
});

// The configuration ADR-0220 exists to enable: the committed queue and a state/ that knows nothing.
// The notice must not turn into a session on a plan that merged before this checkout existed.
test("a run with no state of its own starts no plan already under done/", async () => {
  const { repo, p, cli } = setup([{ number: "0101", phases: [dev("1")] }], { a: ["0090", "0101"] });
  writePlan(repo, { number: "0090", phases: [dev("1")], status: "done — closed" }, { done: true });
  sh(["add", "docs"], repo);
  sh(["commit", "-q", "-m", "0090 merged in an earlier run"], repo);
  assert.equal(existsSync(join(p.stateDir, "conductor.json")), false, "the state the picker would read does not exist");

  const r = await cli("run", "--lane", "a", "--until-idle");
  assert.equal(r.code, 0, r.err.join("\n"));
  assert.equal(r.out[0], QUEUE_NOTICE);
  const state = loadState(p.stateDir);
  assert.equal(state.plans["0090"], undefined, "no record, so no worktree and no session for the merged plan");
  assert.equal(state.plans["0101"].status, "merged", JSON.stringify(state.plans["0101"].park));
  assert.deepEqual(state.runs.at(-1).notStarted ?? [], [], "a merged plan is not owed a not-started reason either");
});

test("prune is refused while a conductor runs, and says so when the queue cannot be read", async () => {
  const { p, cli } = setup([{ number: "0101", phases: [dev("1")] }], { a: ["0101"] });
  mkdirSync(p.stateDir, { recursive: true });
  writeFileSync(join(p.stateDir, "conductor.pid"), String(process.pid));
  const refused = await cli("prune");
  assert.equal(refused.code, 1);
  assert.match(refused.err.join("\n"), /^conductor: a run is in progress and reads the queue it started with/);
  rmSync(join(p.stateDir, "conductor.pid"));

  writeFileSync(p.queue, "{ not json");
  const broken = await cli("prune");
  assert.equal(broken.code, 1);
  assert.match(broken.err.join("\n"), /queue\.json is not valid JSON/);
});

test("run on a checkout with no state/ writes the pid file for the run and removes it after", async () => {
  // The gate runs mid-run, inside the lane: it is green only while the pid file exists.
  const { p, cli } = setup([{ number: "0101", phases: [dev("1")] }], { a: ["0101"] }, {
    gate: (p) => [{ name: "pid-file-present", cmd: [process.execPath, "-e", `process.exit(require("fs").existsSync(${JSON.stringify(join(p.stateDir, "conductor.pid"))}) ? 0 : 1)`] }],
  });
  assert.equal(existsSync(p.stateDir), false);
  const r = await cli("run", "--until-idle");
  assert.equal(r.code, 0, r.err.join("\n"));
  const rec = loadState(p.stateDir).plans["0101"];
  assert.equal(rec.status, "merged", JSON.stringify(rec.park));
  assert.ok(rec.gates.length > 0 && rec.gates.every((g) => g.ok), "the pid file existed while the gate ran");
  assert.equal(existsSync(join(p.stateDir, "conductor.pid")), false, "the pid file is removed when the run ends");
});

test("status, resume, park and abort each run on a checkout with no state/", async () => {
  const { p, cli } = setup([{ number: "0101", phases: [dev("1")] }], { a: ["0101"] });
  const fresh = async (...argv) => {
    rmSync(p.stateDir, { recursive: true, force: true });
    return cli(...argv);
  };

  const status = await fresh("status");
  assert.equal(status.code, 0, status.err.join("\n"));
  assert.equal(status.out[0], "conductor: not running");

  const resume = await fresh("resume", "0101");
  assert.equal(resume.code, 1);
  assert.deepEqual(resume.err, ["conductor: plan 0101 is not parked (never started)"]);

  const park = await fresh("park", "0101");
  assert.equal(park.code, 0, park.err.join("\n"));
  assert.equal(loadState(p.stateDir).plans["0101"].park.reason, "owner");

  const abort = await fresh("abort");
  assert.equal(abort.code, 0, abort.err.join("\n"));
  assert.deepEqual(abort.out, ["conductor: not running"]);
});

test("a patch above a verified version warns; another minor or major is refused; a verified one is silent", () => {
  // Every version below is derived from this list, never written out, so the test states the rule
  // rather than one release's numbers.
  const VERIFIED = ["2.1.272"];
  const [major, minor, patch] = VERIFIED[0].split(".").map(Number);
  const unlisted = (v) => `claude ${v} is not a verified CLI version (verified: ${VERIFIED.join(", ")})`;

  const higherPatch = `${major}.${minor}.${patch + 8}`;
  const warned = cliVerdict(higherPatch, VERIFIED);
  assert.deepEqual(Object.keys(warned), ["warning"]);
  assert.ok(warned.warning.startsWith(`${unlisted(higherPatch)}; a patch update of a verified version runs with this warning (ADR-0208)`), warned.warning);

  for (const refused of [`${major}.${minor + 1}.0`, `${major + 1}.${minor}.${patch}`, `${major}.${minor}.${patch - 1}`]) {
    const v = cliVerdict(refused, VERIFIED);
    assert.deepEqual(Object.keys(v), ["error"], refused);
    assert.ok(v.error.startsWith(`${unlisted(refused)}; re-run tools/conductor/spike/probe.mjs`), v.error);
  }
  assert.deepEqual(cliVerdict(VERIFIED[0], VERIFIED), {});
});

test("run on a patch above the verified CLI prints the warning, records it on the run, and the digest carries it", async () => {
  const { p, cli } = setup([{ number: "0101", phases: [dev("1")] }], { a: ["0101"] });
  const top = VERIFIED_CLI.map((v) => v.split(".").map(Number)).sort((a, b) => a[0] - b[0] || a[1] - b[1] || a[2] - b[2]).at(-1);
  const version = `${top[0]}.${top[1]}.${top[2] + 1}`;
  process.env.FAKE_CLAUDE_VERSION = `${version} (Claude Code)`;
  try {
    const r = await cli("run", "--until-idle");
    assert.equal(r.code, 0, r.err.join("\n"));
    const { warning } = cliVerdict(version);
    assert.deepEqual(r.err, [`conductor: warning: ${warning}`]);
    const state = loadState(p.stateDir);
    assert.deepEqual(state.runs.at(-1).cli, { version, warning });
    assert.equal(state.plans["0101"].status, "merged", JSON.stringify(state.plans["0101"].park));
    assert.ok(readFileSync(p.digest, "utf8").includes(`- **claude ${version} is not a verified CLI version** - the last run went ahead with a warning (ADR-0208): ${warning}.`));
    assert.equal((await cli("digest", "--history")).code, 0);
    assert.ok(readFileSync(p.digestHistory, "utf8").includes(`- **claude ${version} is not a verified CLI version** - the run went ahead with a warning (ADR-0208): ${warning}.`));

    // The next run on a verified version leaves the page with no warning at all, and the history
    // keeps it on the run that carried it.
    process.env.FAKE_CLAUDE_VERSION = `${VERIFIED_CLI[0]} (Claude Code)`;
    await cli("run", "--until-idle");
    assert.ok(!readFileSync(p.digest, "utf8").includes("is not a verified CLI version"));
    await cli("digest", "--history");
    const [newest, earlier] = readFileSync(p.digestHistory, "utf8").split(/^## Run /m).slice(1);
    assert.ok(!newest.includes("is not a verified CLI version"), newest);
    assert.ok(earlier.includes("is not a verified CLI version"), earlier);
  } finally {
    process.env.FAKE_CLAUDE_VERSION = "2.1.270 (Claude Code)";
  }
});

test("unknown commands and malformed plan numbers are usage errors", async () => {
  const { cli } = setup([{ number: "0101", phases: [dev("1")] }], { a: ["0101"] });
  assert.equal((await cli("launch")).code, 2);
  assert.equal((await cli("resume", "175")).code, 2);
});

test("adopt-close records the close a lane already carries, and refuses a lane with none", async () => {
  const { p, cli } = setup([{ number: "0101", phases: [dev("1")] }], { a: ["0101"] }, { spec: { "0101": { loseOutcome: "review" } } });
  await cli("run", "--until-idle");
  const parked = loadState(p.stateDir).plans["0101"];
  assert.equal(parked.park.reason, "no_outcome");
  assert.equal(parked.closed, null);

  const adopt = await cli("adopt-close", "0101");
  assert.equal(adopt.code, 0, adopt.err.join("\n"));
  assert.match(adopt.out.join("\n"), /recorded closed, tag v0\.1\.1/);
  assert.match(adopt.out.join("\n"), /resume 0101/);
  const after = loadState(p.stateDir).plans["0101"];
  assert.equal(after.closed.tag, "v0.1.1");
  assert.equal(after.closed.adopted, true);
  assert.equal(after.status, "parked", "adopting records the close; unparking stays the owner's `resume`");

  // A second adoption has nothing to do, and the run that follows merges without a review.
  const again = await cli("adopt-close", "0101");
  assert.equal(again.code, 1);
  assert.match(again.err.join("\n"), /already recorded closed/);
  assert.equal((await cli("resume", "0101")).code, 0);
  assert.equal((await cli("run", "--until-idle")).code, 0);
  const merged = loadState(p.stateDir).plans["0101"];
  assert.equal(merged.status, "merged", JSON.stringify(merged.park));
  assert.equal(merged.steps.filter((s) => s.kind === "review").length, 1, "no second review session ran");
});

test("adopt-close on a lane with no close review changes nothing, and a bad plan number is a usage error", async () => {
  const { p, cli } = setup([{ number: "0101", phases: [dev("1"), human("2")] }], { a: ["0101"] });
  await cli("run", "--until-idle");
  const before = readFileSync(statePaths(p.stateDir).file, "utf8");
  assert.equal(loadState(p.stateDir).plans["0101"].park.reason, "human_phase");

  const r = await cli("adopt-close", "0101");
  assert.equal(r.code, 1);
  assert.match(r.err.join("\n"), /no close to adopt/);
  assert.match(r.err.join("\n"), /docs\/plans\/done\//);
  assert.equal(readFileSync(statePaths(p.stateDir).file, "utf8"), before, "no state was written");

  assert.equal((await cli("adopt-close")).code, 2);
  assert.equal((await cli("adopt-close", "nope")).code, 2);
  const never = await cli("adopt-close", "0199");
  assert.equal(never.code, 1);
  assert.match(never.err.join("\n"), /no lane on disk/);
});

// ADR-0216: a finding leaves the page only when the owner says why, so every refusal below is a
// finding that stays on it rather than one closed by a guess.

/** A plan record straight into state/, so a refusal has the exact findings it is about. */
function seedPlan(p, plan, rec) {
  mkdirSync(p.stateDir, { recursive: true });
  const state = loadState(p.stateDir);
  // A merged plan always carries the close its last verdict closed with, so that is the default; a
  // seed of a plan that never closed passes `closed: null` back.
  const closed = { version: "0.1.0", tag: "v0.1.0", head: "0".repeat(40), at: "2026-09-18T00:00:00.000Z" };
  state.plans[plan] = { plan, status: "merged", lane: "a", worktree: null, branch: null, steps: [], park: null, parks: [], fixRounds: 0, verdicts: [], fixes: [], gates: [], closed, merge: null, lockWaits: [], started: null, ended: null, ...rec };
  writeFileSync(statePaths(p.stateDir).file, JSON.stringify(state, null, 2));
}

const verdictOf = (findings) => [{ round: 2, blockers: 0, majors: 0, minors: findings.length, review_path: "r.md", findings }];

test("finding lists a merged plan's closing verdict, and a disposition records verb, reason and date", async () => {
  const { p, cli } = setup([{ number: "0101", phases: [dev("1")] }], { a: ["0101"] }, { spec: { "0101": { minors: 2 } } });
  await cli("run", "--until-idle");
  assert.equal(loadState(p.stateDir).plans["0101"].status, "merged");

  const before = await cli("finding", "0101");
  assert.equal(before.code, 0, before.err.join("\n"));
  assert.deepEqual(before.out, [
    "conductor: plan 0101, closing verdict round 1, 2 findings:",
    "  [0] minor phase-0101-1.txt:1 - minor finding 1",
    "  [1] minor phase-0101-1.txt:2 - minor finding 2",
  ]);

  const closed = await cli("finding", "0101", "1", "--wontfix", "assertion message, no reader");
  assert.equal(closed.code, 0, closed.err.join("\n"));
  assert.deepEqual(closed.out, ["conductor: plan 0101 finding 1 (minor phase-0101-1.txt:2) is closed wontfix: assertion message, no reader"]);

  const d = loadState(p.stateDir).plans["0101"].verdicts.at(-1).findings[1].disposition;
  assert.equal(d.verb, "wontfix");
  assert.equal(d.reason, "assertion message, no reader");
  assert.match(d.at, /^\d{4}-\d\d-\d\dT\d\d:\d\d/);

  // The listing is where a `<ref>` comes from, so it carries the disposition it just recorded.
  const after = await cli("finding", "0101");
  assert.equal(after.out[2], `  [1] minor phase-0101-1.txt:2 - minor finding 2 - closed ${d.at.slice(0, 10)} (wontfix): assertion message, no reader`);
  assert.equal(after.out[1], "  [0] minor phase-0101-1.txt:1 - minor finding 1", "the open one is unchanged");

  // The same finding by its file:line, and the page regenerates on the way out.
  const byWhere = await cli("finding", "0101", "phase-0101-1.txt:1", "--filed", "backlog 0251");
  assert.equal(byWhere.code, 0, byWhere.err.join("\n"));
  assert.equal(loadState(p.stateDir).plans["0101"].verdicts.at(-1).findings[0].disposition.verb, "filed");
  assert.match(readFileSync(p.digest, "utf8"), /^# Conductor digest$/m);
});

test("a disposition is refused with no reason, with a ref matching nothing, and with one matching two findings", async () => {
  const { p, cli } = setup([{ number: "0101", phases: [dev("1")] }], { a: ["0101"] });
  seedPlan(p, "0181", {
    verdicts: verdictOf([
      { severity: "minor", file: "core/src/a.rs", line: 88, what: "one" },
      { severity: "nit", file: "docs/b.md", line: null, what: "two" },
      { severity: "nit", file: "docs/b.md", line: null, what: "three" },
    ]),
  });
  const untouched = readFileSync(statePaths(p.stateDir).file, "utf8");

  for (const argv of [["finding", "0181", "0", "--wontfix"], ["finding", "0181", "0", "--wontfix", "   "]]) {
    const r = await cli(...argv);
    assert.equal(r.code, 2, argv.join(" "));
    assert.match(r.err[0], /--wontfix needs a reason; a disposition with none is how a finding gets closed for being old \(ADR-0216\)/);
  }

  const past = await cli("finding", "0181", "9", "--done", "repaired");
  assert.equal(past.code, 1);
  assert.equal(past.err[0], "conductor: there is no finding 9; the verdict carries 0 core/src/a.rs:88, 1 docs/b.md, 2 docs/b.md");

  const nowhere = await cli("finding", "0181", "docs/z.md", "--done", "repaired");
  assert.equal(nowhere.code, 1);
  assert.equal(nowhere.err[0], "conductor: no finding is at docs/z.md; the verdict carries 0 core/src/a.rs:88, 1 docs/b.md, 2 docs/b.md");

  const both = await cli("finding", "0181", "docs/b.md", "--filed", "backlog 0250");
  assert.equal(both.code, 1);
  assert.equal(both.err[0], "conductor: docs/b.md names 2 findings, so it says nothing: 1 nit two; 2 nit three");

  assert.equal(readFileSync(statePaths(p.stateDir).file, "utf8"), untouched, "no refusal wrote state");
});

test("a second disposition overwrites the first, and the first survives in the finding's history", async () => {
  const { p, cli } = setup([{ number: "0101", phases: [dev("1")] }], { a: ["0101"] });
  seedPlan(p, "0181", { verdicts: verdictOf([{ severity: "nit", file: "core/src/a.rs", line: 3, what: "a latent regex" }]) });

  assert.equal((await cli("finding", "0181", "0", "--wontfix", "no reader today")).code, 0);
  const first = loadState(p.stateDir).plans["0181"].verdicts.at(-1).findings[0].disposition;

  const second = await cli("finding", "0181", "0", "--done", "repaired in a later plan");
  assert.equal(second.code, 0, second.err.join("\n"));
  assert.equal(second.out[1], `  it was wontfix on ${first.at.slice(0, 10)} (no reader today); that stays in the finding's history.`);

  const f = loadState(p.stateDir).plans["0181"].verdicts.at(-1).findings[0];
  assert.deepEqual(f.disposition.verb, "done");
  assert.equal(f.disposition.reason, "repaired in a later plan");
  assert.deepEqual(f.dispositionHistory, [first]);
});

test("finding on a plan with no closing verdict exits non-zero saying why, and a malformed call is a usage error", async () => {
  const { p, cli } = setup([{ number: "0101", phases: [dev("1")] }], { a: ["0101"] });
  // A plan parked at a fix round carries the round's verdict and no close: its findings are the
  // conductor's own work in flight, so neither listing nor disposing them is allowed.
  seedPlan(p, "0182", {
    status: "parked",
    closed: null,
    fixRounds: 2,
    verdicts: verdictOf([{ severity: "blocker", file: "core/src/a.rs", line: 12, what: "the ring drops a frame" }]),
  });
  seedPlan(p, "0183", { verdicts: verdictOf([]) });
  const untouched = readFileSync(statePaths(p.stateDir).file, "utf8");

  const never = await cli("finding", "0199");
  assert.equal(never.code, 1);
  assert.equal(never.err[0], "conductor: plan 0199 has no closing verdict, so it has no findings (it never started)");

  const parked = await cli("finding", "0182", "0", "--done", "x");
  assert.equal(parked.code, 1);
  assert.equal(parked.err[0], "conductor: plan 0182 has no closing verdict, so it has no findings (it is parked, 2 fix rounds in)");

  const listed = await cli("finding", "0182");
  assert.equal(listed.code, 1);
  assert.equal(listed.err[0], parked.err[0]);
  assert.deepEqual(listed.out, []);
  assert.equal(readFileSync(statePaths(p.stateDir).file, "utf8"), untouched, "no refusal wrote state");

  const none = await cli("finding", "0183");
  assert.equal(none.code, 0);
  assert.deepEqual(none.out, ["conductor: plan 0183 closed with no findings (verdict round 2)."]);

  // A refusal is on stderr, so an operator reading it never sees a non-zero exit and nothing said.
  const nothingToClose = await cli("finding", "0183", "0", "--done", "repaired");
  assert.equal(nothingToClose.code, 1);
  assert.equal(nothingToClose.err[0], "conductor: plan 0183 closed with no findings (verdict round 2), so there is nothing to close");
  assert.deepEqual(nothingToClose.out, []);

  for (const argv of [
    ["finding"],
    ["finding", "181"],
    ["finding", "0181", "0"],
    ["finding", "0181", "0", "--nope", "x"],
    ["finding", "0181", "0", "done", "x"],
    ["finding", "0181", "--done", "x"],
  ]) {
    const r = await cli(...argv);
    assert.equal(r.code, 2, argv.join(" "));
    assert.match(r.err[0], /^usage: conductor\.mjs finding NNNN \[<ref> --done\|--wontfix\|--filed <reason>\]$/);
  }
});

test("resume refuses a claude_dir park until the plan's log marks that phase done", async () => {
  const claudePhase = { id: "1", owner: "dev", files: "`.claude/skills/dev/SKILL.md`" };
  const { p, cli } = setup([{ number: "0101", phases: [claudePhase, dev("2")] }], { a: ["0101"] });
  await cli("run", "--until-idle");
  const rec = loadState(p.stateDir).plans["0101"];
  assert.equal(rec.park.reason, "claude_dir");
  assert.match(rec.park.detail, /\.claude\/skills\/dev\/SKILL\.md/);

  const refused = await cli("resume", "0101");
  assert.equal(refused.code, 1);
  assert.match(refused.err[0], /its park reason \(claude_dir\) still holds: Phase 1 is still not marked done/);
  assert.equal(loadState(p.stateDir).plans["0101"].status, "parked");

  // The owner makes the edit and commits the row in the lane, exactly as for a human phase.
  const planPath = join(rec.worktree, "docs", "plans", "0101-fixture.md");
  writeFileSync(planPath, readFileSync(planPath, "utf8").replace(/^\| 1 — Step 1 \| dev \| not started \|/m, "| 1 — Step 1 | dev | done |"));
  sh(["commit", "-q", "-am", "docs(plans): phase 1 done by the owner"], rec.worktree);

  const accepted = await cli("resume", "0101");
  assert.equal(accepted.code, 0, accepted.err.join("\n"));
  assert.equal(loadState(p.stateDir).plans["0101"].status, "queued");
});

// ADR-0250: `run` is resident, and `--until-idle` is the run that ends once no lane can move.
test("run --until-idle ends a run whose queue is empty, and a resident run stays up watching until stopped", async () => {
  const { cli } = setup([{ number: "0101", phases: [dev("1")] }], { a: [] }, { idlePollMs: 20, stopRequested: () => Date.now() > stopAt });
  let stopAt = Infinity;
  const idle = await cli("run", "--until-idle");
  assert.equal(idle.code, 0, idle.err.join("\n"));
  assert.match(idle.out.join("\n"), /run ended - 0 merged, 0 parked/);
  assert.doesNotMatch(idle.out.join("\n"), /watching the queue/);

  const t0 = Date.now();
  stopAt = t0 + 300;
  const resident = await cli("run");
  assert.equal(resident.code, 0, resident.err.join("\n"));
  assert.ok(Date.now() - t0 >= 300, "the resident run did not end on an empty queue");
  assert.ok(resident.out.includes("conductor: lane a is idle, watching the queue"), resident.out.join("\n"));

  const both = await cli("run", "--once", "--until-idle");
  assert.equal(both.code, 1);
  assert.match(both.err.join("\n"), /--once and --until-idle are two ways to end a run/);
});

test("check refuses a local.json with no run_budget_usd", async () => {
  const { p, cli } = setup([{ number: "0101", phases: [dev("1")] }], { a: ["0101"] });
  writeFileSync(p.local, JSON.stringify({ budget_usd: { implement: 5, fix: 3, review: 4, merge: 2 }, max_open_worktrees: 3 }));
  const r = await cli("check");
  assert.equal(r.code, 1);
  assert.ok(r.err.includes("conductor: local.json: run_budget_usd must be a positive number"), r.err.join("\n"));
});

test("resume while a run is live leaves an ask for it rather than writing the record under it", async () => {
  const { p, cli } = setup([{ number: "0101", phases: [dev("1")] }], { a: ["0101"] });
  await cli("park", "0101");
  // This test process stands in for the live conductor.
  writeFileSync(join(p.stateDir, "conductor.pid"), String(process.pid));
  const r = await cli("resume", "0101");
  rmSync(join(p.stateDir, "conductor.pid"));
  assert.equal(r.code, 0, r.err.join("\n"));
  assert.deepEqual(r.out, ["conductor: plan 0101: the live run takes the resume on its next look, within a minute"]);
  assert.equal(loadState(p.stateDir).plans["0101"].status, "parked", "the record is the run's to change");
  assert.deepEqual(
    readFileSync(statePaths(p.stateDir).resumeAsks, "utf8").trim().split("\n").map((l) => JSON.parse(l).plan),
    ["0101"],
  );
});
