// The operator surface: `run`, `status`, `resume`, `park` and `abort` against a scratch repository
// and the fake CLI, driven through the same `main` the command line calls.

import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";

import { main, paths } from "../conductor.mjs";
import { resumeCommand } from "../lib/inbox.mjs";
import { loadState, statePaths } from "../lib/state.mjs";
import { FAKE, TEST_DIR, TOOL_DIR, tmp, writePlan } from "./helpers.mjs";

function sh(args, cwd) {
  const r = spawnSync("git", args, { cwd, encoding: "utf8" });
  assert.equal(r.status, 0, `git ${args.join(" ")}: ${r.stderr}`);
  return r.stdout.trim();
}

const dev = (id) => ({ id, owner: "dev" });
const human = (id) => ({ id, owner: "human" });

function setup(plans, lanes) {
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
  writeFileSync(join(toolDir, "local.json"), JSON.stringify({ budget_usd: { implement: 5, fix: 3, review: 4 }, max_open_worktrees: 3 }));
  const p = { ...paths({ repo, toolDir }), settings: join(TOOL_DIR, "settings.conductor.json"), prompts: join(TOOL_DIR, "prompts"), withLock: join(TOOL_DIR, "with-lock.mjs") };
  mkdirSync(p.stateDir, { recursive: true });

  const specFile = join(toolDir, "spec.json");
  writeFileSync(specFile, JSON.stringify({ plans: {} }));
  process.env.FAKE_CLAUDE_SCENARIO = join(TEST_DIR, "lane-scenario.mjs");
  process.env.FAKE_LANE_SPEC = specFile;
  process.env.FAKE_EVENTS = join(toolDir, "events.jsonl");
  process.env.FAKE_CLAUDE_VERSION = "2.1.270 (Claude Code)";

  const out = [];
  const err = [];
  const o = {
    p,
    claude: FAKE,
    gate: [{ name: "noop", cmd: [process.execPath, "-e", "0"] }],
    worktreeRoot: tmp("rlx-cli-lanes-"),
    lockDir: tmp("rlx-cli-locks-"),
    lockPollMs: 20,
    pollMs: 50,
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

  const second = await cli("run", "--lane", "a");
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
  await cli("run");
  const before = readFileSync(p.digest, "utf8");
  assert.match(before, /### Closed\n\n- \*\*0101 - Plan 0101 fixture\*\*/);
  rmSync(p.digest);
  await cli("status");
  assert.equal(readFileSync(p.digest, "utf8"), before);
});

test("resume refuses a human-phase park the log does not mark done, and accepts the digest's command once it does", async () => {
  const { p, cli } = setup([{ number: "0101", phases: [dev("1"), human("2"), dev("3")] }], { a: ["0101"] });
  await cli("run");
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

  const finish = await cli("run");
  assert.equal(finish.code, 0);
  const done = loadState(p.stateDir).plans["0101"];
  assert.equal(done.status, "merged", JSON.stringify(done.park));
  assert.deepEqual(done.steps.map((s) => s.kind), ["implement", "implement", "review"]);
});

test("run starts again after a merge, and a resumed sibling of the merged plan runs to its merge", async () => {
  const { p, cli } = setup(
    [
      { number: "0101", phases: [dev("1")] },
      { number: "0102", phases: [dev("1"), human("2")] },
    ],
    { a: ["0101", "0102"] },
  );
  const first = await cli("run");
  assert.equal(first.code, 0, first.err.join("\n"));
  let state = loadState(p.stateDir);
  assert.equal(state.plans["0101"].status, "merged");
  assert.equal(state.plans["0102"].status, "parked");

  // The merged plan stays listed in queue.json and now sits under done/; that must not refuse a run.
  const again = await cli("run");
  assert.equal(again.code, 0, again.err.join("\n"));
  assert.equal((await cli("check")).code, 0);

  const rec = loadState(p.stateDir).plans["0102"];
  const planPath = join(rec.worktree, "docs", "plans", "0102-fixture.md");
  writeFileSync(planPath, readFileSync(planPath, "utf8").replace(/^\| 2 — Step 2 \| human \| not started \|/m, "| 2 — Step 2 | human | done |"));
  sh(["commit", "-q", "-am", "docs(plans): phase 2 done by the owner"], rec.worktree);
  assert.equal((await cli("resume", "0102")).code, 0);

  const finish = await cli("run");
  assert.equal(finish.code, 0, finish.err.join("\n"));
  state = loadState(p.stateDir);
  assert.equal(state.plans["0102"].status, "merged", JSON.stringify(state.plans["0102"].park));
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
  writeFileSync(join(p.stateDir, "conductor.pid"), String(process.pid));
  const refused = await cli("run");
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

test("unknown commands and malformed plan numbers are usage errors", async () => {
  const { cli } = setup([{ number: "0101", phases: [dev("1")] }], { a: ["0101"] });
  assert.equal((await cli("launch")).code, 2);
  assert.equal((await cli("resume", "175")).code, 2);
});
