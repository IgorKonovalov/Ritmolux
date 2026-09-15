// The digest is a pure function of state and git: regenerating it gives the same bytes, and a
// finding line is the verdict's own fields and nothing from the review's prose.

import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { readFileSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";

import { duration, renderDigest, writeDigest } from "../lib/digest.mjs";
import { tmp, writePlan } from "./helpers.mjs";

function repoWithTag() {
  const repo = tmp("rlx-digest-repo-");
  const sh = (...a) => {
    const r = spawnSync("git", a, { cwd: repo, encoding: "utf8" });
    assert.equal(r.status, 0, r.stderr);
    return r.stdout.trim();
  };
  sh("init", "-q", "-b", "main");
  sh("config", "user.email", "t@example.invalid");
  sh("config", "user.name", "T");
  writePlan(repo, { number: "0175", title: "An eased value arrives", status: "done", phases: [{ id: "1", owner: "dev" }] }, { done: true });
  sh("add", "docs");
  sh("commit", "-q", "-m", "init");
  sh("tag", "-a", "v0.124.0", "-m", "chore: Release v0.124.0");
  return { repo, head: sh("rev-parse", "HEAD") };
}

const REVIEW_PROSE = "SENTINEL-PROSE the reviewer wrote at length about easing";

function sampleState(repo, head, stateDir) {
  const reviewPath = join(stateDir, "reviews", "0175-round-1.md");
  return {
    version: 1,
    runs: [{ started: "2026-09-15T01:12:00.000Z", ended: "2026-09-15T07:40:00.000Z", lanes: ["a", "b"] }],
    lanes: {},
    plans: {
      "0175": {
        plan: "0175",
        status: "merged",
        lane: "a",
        worktree: "C:/WORK/rlx-plan-0175",
        branch: "plan-0175-an-eased-value-arrives",
        steps: [
          { kind: "implement", label: "0175-01-implement", started: "2026-09-15T01:13:00.000Z", ended: "2026-09-15T02:00:00.000Z", result: { status: "ok", spendUsd: 6.1 } },
          { kind: "review", label: "0175-02-review", started: "2026-09-15T02:10:00.000Z", ended: "2026-09-15T02:30:00.000Z", result: { status: "ok", spendUsd: 3.2 } },
          { kind: "fix", label: "0175-03-fix", started: "2026-09-15T02:31:00.000Z", ended: "2026-09-15T02:50:00.000Z", result: { status: "ok", spendUsd: 1.0 } },
          { kind: "review", label: "0175-04-review", started: "2026-09-15T02:55:00.000Z", ended: "2026-09-15T03:04:00.000Z", result: { status: "ok", spendUsd: 2.0 } },
        ],
        park: null,
        parks: [],
        fixRounds: 1,
        verdicts: [
          {
            round: 1,
            blockers: 0,
            majors: 1,
            minors: 0,
            review_path: reviewPath,
            findings: [{ severity: "major", file: "core/src/preset/schema/easing.rs", line: 88, what: "snap test missing the alpha-near-1 case" }],
          },
          {
            round: 2,
            blockers: 0,
            majors: 0,
            minors: 1,
            review_path: reviewPath.replace("round-1", "round-2"),
            findings: [{ severity: "minor", file: "docs/presets.md", line: 612, what: 'occlude row still says "with a stage"' }],
          },
        ],
        fixes: [{ round: 1, commits: ["e4f5a6b7c8d9e0f1a2b3c4d5e6f7a8b9c0d1e2f3"], resolved: [{ finding: 0, commit: "e4f5a6b7c8d9e0f1a2b3c4d5e6f7a8b9c0d1e2f3" }] }],
        closed: { version: "0.124.0", tag: "v0.124.0", head },
        merge: { head, remerged: false, at: "2026-09-15T03:05:00.000Z" },
        lockWaits: [
          { lock: "suite", ms: 30 * 60000, at: "2026-09-15T02:05:00.000Z" },
          { lock: "close", ms: 6 * 60000, at: "2026-09-15T02:10:00.000Z" },
          { lock: "suite", ms: 8 * 60000, at: "2026-09-15T02:54:00.000Z" },
        ],
        gates: [],
        started: "2026-09-15T01:13:00.000Z",
        ended: "2026-09-15T03:05:00.000Z",
      },
    },
  };
}

test("regenerating the digest from the same state and git gives identical bytes", () => {
  const { repo, head } = repoWithTag();
  const stateDir = tmp("rlx-digest-state-");
  const state = sampleState(repo, head, stateDir);
  const path = join(tmp("rlx-digest-out-"), "digest.md");
  const first = writeDigest(path, state, { repo, stateDir });
  rmSync(path);
  writeDigest(path, state, { repo, stateDir });
  assert.equal(readFileSync(path, "utf8"), first);
  assert.equal(renderDigest(state, { repo, stateDir }), first);
});

test("a finding line carries only what the verdict carried", () => {
  const { repo, head } = repoWithTag();
  const stateDir = tmp("rlx-digest-state-");
  const state = sampleState(repo, head, stateDir);
  // The review file holds prose the digest must never quote.
  const reviews = join(stateDir, "reviews");
  spawnSync(process.execPath, ["-e", `require('fs').mkdirSync(${JSON.stringify(reviews)},{recursive:true})`]);
  writeFileSync(join(reviews, "0175-round-1.md"), `# Review\n\n${REVIEW_PROSE}\n`);
  const text = renderDigest(state, { repo, stateDir });

  assert.ok(!text.includes("SENTINEL-PROSE"), "no review prose reaches the digest");
  const lines = text.split("\n");
  assert.ok(lines.includes("  - major `core/src/preset/schema/easing.rs:88` snap test missing the alpha-near-1 case - resolved in `e4f5a6b`"));
  assert.ok(lines.includes('  - minor `docs/presets.md:612` occlude row still says "with a stage"'));
  assert.ok(
    lines.includes(
      "- **0175 - An eased value arrives** - 0.124.0, tag `v0.124.0` annotated, merge `" +
        head.slice(0, 7) +
        "`, 1 fix round, active 1 h 35 min, wall 1 h 52 min in this run, $12.30. Review: `docs/plans/done/0175-fixture.md` `## Close review`.",
    ),
    text,
  );
  const needs = lines.indexOf("### Needs you");
  assert.deepEqual(lines.slice(needs + 2, needs + 4), ["- **0175 merged with 1 open finding**:", '  - minor `docs/presets.md:612` occlude row still says "with a stage"']);
  assert.ok(lines.includes("## Run 2026-09-15 01:12 -> 2026-09-15 07:40 (lanes a, b)"));
  assert.ok(lines.includes("- run: 1 merged, 0 parked, 6 h 28 min, $12.30. Suite-lock wait 38 min; close-lock wait 6 min."));
});

test("a plan spanning two runs counts each lock wait in the run it happened in, once", () => {
  const { repo, head } = repoWithTag();
  const stateDir = tmp("rlx-digest-state-");
  const state = sampleState(repo, head, stateDir);
  // The plan parked in an earlier run and resumed in this one: its first implement step and a
  // 20-minute suite wait belong to the earlier run.
  state.runs.unshift({ started: "2026-09-14T20:00:00.000Z", ended: "2026-09-14T22:00:00.000Z", lanes: ["a"] });
  const rec = state.plans["0175"];
  rec.steps.unshift({ kind: "implement", label: "0175-00-implement", started: "2026-09-14T20:05:00.000Z", ended: "2026-09-14T21:00:00.000Z", result: { status: "ok", spendUsd: 1 } });
  rec.lockWaits.unshift({ lock: "suite", ms: 20 * 60000, at: "2026-09-14T20:30:00.000Z" });
  const lines = renderDigest(state, { repo, stateDir }).split("\n");
  const runLines = lines.filter((l) => l.startsWith("- run: "));
  assert.equal(runLines.length, 2);
  assert.match(runLines[0], /Suite-lock wait 38 min; close-lock wait 6 min\.$/, "the newer run keeps only its own waits");
  assert.match(runLines[1], /Suite-lock wait 20 min; close-lock wait < 1 min\.$/, "the earlier run keeps only its own");
});

test("a run's cap stop and its not-started plans render from state alone, byte for byte on regeneration", () => {
  const { repo, head } = repoWithTag();
  const stateDir = tmp("rlx-digest-state-");
  const state = sampleState(repo, head, stateDir);
  const run = state.runs[0];
  run.stops = [{ lane: "a", reason: "worktree_cap", plan: "0181", holding: ["0175", "0180", "0185"], max: 3, at: "2026-09-15T03:06:00.000Z" }];
  run.notStarted = [
    { plan: "0181", lane: "a", reason: "worktree cap" },
    { plan: "0182", lane: "a", reason: "after 0180 (parked)" },
  ];
  const path = join(tmp("rlx-digest-out-"), "digest.md");
  const first = writeDigest(path, state, { repo, stateDir });
  rmSync(path);
  assert.equal(writeDigest(path, state, { repo, stateDir }), first);

  const lines = first.split("\n");
  assert.ok(lines.includes("- **Lane a stopped at the worktree cap** (`max_open_worktrees` 3): 0181 was not opened. Worktrees held by 0175, 0180, 0185."), first);
  const start = lines.indexOf("### Not started");
  assert.ok(start > lines.indexOf("### Needs you") && start < lines.indexOf("### Closed"), first);
  assert.deepEqual(lines.slice(start + 1, start + 5), ["", "- **0181** (lane a): worktree cap", "- **0182** (lane a): after 0180 (parked)", ""]);
  assert.match(first, /### Needs you\n\n- \*\*Lane a stopped/);

  // The same run with nothing left unopened has no Not started section.
  run.stops = [];
  run.notStarted = [];
  assert.ok(!renderDigest(state, { repo, stateDir }).includes("### Not started"));
});

const W = (five, seven, status = "allowed") => ({ status, five: { utilization: five, resetsAt: 1789481400 }, seven: { utilization: seven, resetsAt: 1790056800 } });

/**
 * The pilot's shape: 0185 starts in the evening run, parks on the budget, sits parked overnight and
 * merges in the morning run; 0175 and 0180 park in the evening and are still parked, 0175's worktree
 * since removed by hand and 0180's still on disk.
 */
function pilotState(repo, head, stateDir) {
  const kept = tmp("rlx-plan-0180-");
  const removed = join(tmp("rlx-gone-"), "rlx-plan-0175");
  const step = (label, started, ended, extra = {}) => ({ kind: label.split("-")[2], label, started, ended, result: { status: "ok", spendUsd: 1 }, ...extra });
  const parkAt = (plan, reason, at, worktree) => ({ reason, detail: `${plan} ${reason}`, phase: null, read: null, worktree, at });
  const parked = (plan, reason, at, worktree, branch, steps) => {
    const park = parkAt(plan, reason, at, worktree);
    return { plan, status: "parked", lane: "a", worktree, branch, steps, park, parks: [park], fixRounds: 0, verdicts: [], fixes: [], gates: [], lockWaits: [], started: steps[0].started, ended: at };
  };
  const eveningPark = parkAt("0185", "budget", "2026-09-14T19:02:00.000Z", "C:/WORK/rlx-plan-0185");
  return {
    version: 1,
    runs: [
      { started: "2026-09-14T17:00:00.000Z", ended: "2026-09-14T21:00:00.000Z", lanes: ["a"] },
      { started: "2026-09-15T08:30:00.000Z", ended: "2026-09-15T11:00:00.000Z", lanes: ["a"] },
    ],
    lanes: {},
    plans: {
      "0175": parked("0175", "plan_wrong", "2026-09-14T17:40:00.000Z", removed, "plan-0175-an-eased-value-arrives", [
        step("0175-01-implement", "2026-09-14T17:01:00.000Z", "2026-09-14T17:40:00.000Z"),
      ]),
      "0180": parked("0180", "plan_wrong", "2026-09-14T18:20:00.000Z", kept, "plan-0180-the-converted-picture", [
        step("0180-01-implement", "2026-09-14T17:41:00.000Z", "2026-09-14T18:20:00.000Z", { usage: { first: W(0.2, 0.84), last: W(0.31, 0.85, "allowed_warning") } }),
      ]),
      "0185": {
        plan: "0185",
        status: "merged",
        lane: "a",
        worktree: "C:/WORK/rlx-plan-0185",
        branch: "plan-0185-occlude",
        steps: [
          step("0185-01-implement", "2026-09-14T18:21:00.000Z", "2026-09-14T19:02:00.000Z", {
            result: { status: "parked", reason: "budget", spendUsd: 8.1 },
            usage: { first: W(0.31, 0.85), last: W(0.4, 0.86) },
          }),
          step("0185-02-implement", "2026-09-15T08:31:00.000Z", "2026-09-15T09:01:00.000Z", { usage: { first: W(0.02, 0.0), last: W(0.05, 0.01) } }),
          step("0185-03-review", "2026-09-15T09:14:00.000Z", "2026-09-15T09:40:00.000Z", { usage: { first: W(0.05, 0.01), last: W(0.08, 0.01) } }),
        ],
        park: null,
        parks: [eveningPark],
        fixRounds: 0,
        verdicts: [{ round: 1, blockers: 0, majors: 0, minors: 1, review_path: "r.md", findings: [{ severity: "minor", file: "a.rs", line: 3, what: "stale comment" }, { severity: "nit", file: "b.md", line: null, what: "typo" }] }],
        fixes: [],
        gates: [
          {
            label: "pre-review",
            ok: true,
            at: "2026-09-15T09:13:00.000Z",
            commands: [
              { name: "check-doc-links.mjs", code: 0, ms: 60000 },
              { name: "cargo clippy", code: 0, ms: 60000 },
              { name: "cargo nextest", code: 0, ms: 10 * 60000 },
            ],
          },
          { label: "post-close", ok: true, at: "2026-09-15T09:53:00.000Z", commands: [{ name: "cargo nextest", code: 0, ms: 11 * 60000 }] },
        ],
        closed: { version: "0.124.0", tag: "v0.124.0", head },
        merge: { head, remerged: false, at: "2026-09-15T09:54:00.000Z" },
        lockWaits: [],
        started: "2026-09-14T18:21:00.000Z",
        ended: "2026-09-15T09:54:00.000Z",
      },
    },
  };
}

test("a plan parked overnight reports its active time in the run it merged in, never the span from its first start", () => {
  const { repo, head } = repoWithTag();
  const stateDir = tmp("rlx-digest-state-");
  const state = pilotState(repo, head, stateDir);
  const text = renderDigest(state, { repo, stateDir });
  const line = text.split("\n").find((l) => l.startsWith("- **0185 - "));
  assert.ok(line, text);
  // Steps in the morning run: 30 + 26 min; gates: 12 + 11 min. 79 min in all.
  const m = line.match(/, active (?:(\d+) h )?(\d+) min, wall (?:(\d+) h )?(\d+) min in this run, /);
  assert.ok(m, line);
  const activeMin = Number(m[1] ?? 0) * 60 + Number(m[2]);
  assert.ok(Math.abs(activeMin - 79) <= 1, `active ${activeMin} min, expected 79: ${line}`);
  // Wall time runs from the first step in the morning run to the merge: 08:31 to 09:54.
  assert.equal(Number(m[3] ?? 0) * 60 + Number(m[4]), 83);
  assert.ok(!/15 h|16 h/.test(line), "no span across the night");
});

test("the newest run lists the plans an earlier run left parked, and the earlier run does not", () => {
  const { repo, head } = repoWithTag();
  const stateDir = tmp("rlx-digest-state-");
  const state = pilotState(repo, head, stateDir);
  const text = renderDigest(state, { repo, stateDir });
  const [newest, earlier] = text.split(/^## Run /m).slice(1);
  const section = newest.slice(newest.indexOf("#### Still parked from an earlier run"), newest.indexOf("### Closed"));
  const items = section.split("\n").filter((l) => l.startsWith("- **"));
  assert.deepEqual(
    items.map((l) => l.slice(0, 8)),
    ["- **0175", "- **0180"],
  );
  assert.ok(items[0].includes("Worktree removed; `resume` reopens it from branch `plan-0175-an-eased-value-arrives`."), items[0]);
  assert.ok(!items[0].includes("rlx-plan-0175"), "a removed worktree is named by its branch");
  assert.ok(items[0].includes("parked 2026-09-14 17:40, 14 h 50 min before this run."), items[0]);
  assert.match(items[1], /Holds `[^`]*rlx-plan-0180-[^`]*`\.$/);
  assert.match(section, /^ {2}Resume: `node tools\/conductor\/conductor\.mjs resume 0180`$/m);
  assert.ok(!newest.includes("- nothing:"), "a standing park is not nothing");
  assert.ok(!earlier.includes("Still parked from an earlier run"));

  // The earlier run's park lines carry the usage reading their session ended on.
  assert.match(earlier, /- \*\*0180 parked\*\* \(`plan_wrong`\)\..* Usage at park: 5h 0\.31 \(resets 09-15 \d\d:\d\d\); 7d 0\.85 \(resets 09-22 \d\d:\d\d\); allowed_warning\.$/m);
  assert.match(earlier, /- \*\*0185 parked\*\* \(`budget`\) - since resumed\..* Usage at park: 5h 0\.40 /m);

  // The open findings are listed with file:line, minors and nits alike.
  assert.match(newest, /- \*\*0185 merged with 2 open findings\*\*:\n {2}- minor `a\.rs:3` stale comment\n {2}- nit `b\.md` typo\n/);

  // Totals: the run's first and last usage reading, and gate minutes split by the suite.
  assert.match(newest, /^- usage at run start: 5h 0\.02 \(resets [^)]+\); 7d 0\.00 \(resets [^)]+\)\. At run end: 5h 0\.08 \(resets [^)]+\); 7d 0\.01 \(resets [^)]+\)\.$/m);
  assert.match(newest, /^- gate: 23 min; full suite 21 min over 2 runs, everything else 2 min; 0 suite runs skipped\.$/m);

  // And the whole digest regenerates byte for byte.
  assert.equal(renderDigest(state, { repo, stateDir }), text);
});

test("newest run first, and a run with nothing in it says so", () => {
  const state = {
    version: 1,
    runs: [
      { started: "2026-09-14T10:00:00.000Z", ended: "2026-09-14T11:00:00.000Z", lanes: ["a"] },
      { started: "2026-09-15T10:00:00.000Z", ended: null, lanes: ["a"] },
    ],
    lanes: {},
    plans: {},
  };
  const text = renderDigest(state, { repo: tmp(), stateDir: tmp() });
  assert.ok(text.indexOf("## Run 2026-09-15 10:00 -> running") < text.indexOf("## Run 2026-09-14 10:00"));
  assert.match(text, /### Closed\n\n- none\n/);
  assert.match(text, /still running/);
});

test("durations", () => {
  assert.equal(duration(20_000), "< 1 min");
  assert.equal(duration(6 * 60000), "6 min");
  assert.equal(duration(112 * 60000), "1 h 52 min");
});
