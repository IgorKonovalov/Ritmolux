// The operator surface: `run`, `status`, `resume`, `park` and `abort` against a scratch repository
// and the fake CLI, driven through the same `main` the command line calls.

import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";

import { VERIFIED_CLI, cliVerdict, main, paths } from "../conductor.mjs";
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

function setup(plans, lanes, { gate, maxOpenWorktrees = 3, spec = {} } = {}) {
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
  writeFileSync(join(toolDir, "local.json"), JSON.stringify({ budget_usd: { implement: 5, fix: 3, review: 4 }, max_open_worktrees: maxOpenWorktrees }));
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

test("resume refuses a park whose worktree is dirty, naming the paths, and accepts once it is clean", async () => {
  const { p, cli } = setup([{ number: "0101", phases: [dev("1")] }], { a: ["0101"] });
  writeFileSync(join(p.toolDir, "spec.json"), JSON.stringify({ plans: { "0101": { dirtyPark: { untracked: 0 } } } }));
  await cli("run");
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
  const r = await cli("run", "--lane", "a");
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

test("run on a checkout with no state/ writes the pid file for the run and removes it after", async () => {
  // The gate runs mid-run, inside the lane: it is green only while the pid file exists.
  const { p, cli } = setup([{ number: "0101", phases: [dev("1")] }], { a: ["0101"] }, {
    gate: (p) => [{ name: "pid-file-present", cmd: [process.execPath, "-e", `process.exit(require("fs").existsSync(${JSON.stringify(join(p.stateDir, "conductor.pid"))}) ? 0 : 1)`] }],
  });
  assert.equal(existsSync(p.stateDir), false);
  const r = await cli("run");
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
    const r = await cli("run");
    assert.equal(r.code, 0, r.err.join("\n"));
    const { warning } = cliVerdict(version);
    assert.deepEqual(r.err, [`conductor: warning: ${warning}`]);
    const state = loadState(p.stateDir);
    assert.deepEqual(state.runs.at(-1).cli, { version, warning });
    assert.equal(state.plans["0101"].status, "merged", JSON.stringify(state.plans["0101"].park));
    assert.ok(readFileSync(p.digest, "utf8").includes(`- **claude ${version} is not a verified CLI version** - the run went ahead with a warning (ADR-0208): ${warning}.`));

    // The next run on a verified version carries no warning in its own section.
    process.env.FAKE_CLAUDE_VERSION = `${VERIFIED_CLI[0]} (Claude Code)`;
    await cli("run");
    const newest = readFileSync(p.digest, "utf8").split(/^## Run /m)[1];
    assert.ok(!newest.includes("is not a verified CLI version"), newest);
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
  await cli("run");
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
  assert.equal((await cli("run")).code, 0);
  const merged = loadState(p.stateDir).plans["0101"];
  assert.equal(merged.status, "merged", JSON.stringify(merged.park));
  assert.equal(merged.steps.filter((s) => s.kind === "review").length, 1, "no second review session ran");
});

test("adopt-close on a lane with no close review changes nothing, and a bad plan number is a usage error", async () => {
  const { p, cli } = setup([{ number: "0101", phases: [dev("1"), human("2")] }], { a: ["0101"] });
  await cli("run");
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

test("resume refuses a claude_dir park until the plan's log marks that phase done", async () => {
  const claudePhase = { id: "1", owner: "dev", files: "`.claude/skills/dev/SKILL.md`" };
  const { p, cli } = setup([{ number: "0101", phases: [claudePhase, dev("2")] }], { a: ["0101"] });
  await cli("run");
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
