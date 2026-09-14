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
        "`, 1 fix round, 1 h 52 min, $12.30. Review: `docs/plans/done/0175-fixture.md` `## Close review`.",
    ),
    text,
  );
  assert.ok(lines.includes("- **0175 merged with 1 minor** - see Closed."));
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
