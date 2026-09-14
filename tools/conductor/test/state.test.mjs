// State persistence: writes are atomic, and a conductor killed mid-step restarts from the last
// completed step.

import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { pathToFileURL } from "node:url";

import {
  completedSteps,
  endStep,
  loadState,
  recoverInterrupted,
  saveState,
  startStep,
  statePaths,
  writeAtomic,
} from "../lib/state.mjs";
import { TOOL_DIR, tmp } from "./helpers.mjs";

test("a write that dies before its rename leaves the previous state intact", () => {
  const dir = tmp();
  const state = loadState(dir);
  startStep(dir, state, "0101", { kind: "implement", phases: ["1"] });
  const before = readFileSync(statePaths(dir).file, "utf8");
  // What a kill between writeFileSync(tmp) and renameSync leaves behind: a torn temp file.
  writeFileSync(`${statePaths(dir).file}.99999.tmp`, '{"version": 1, "plans": {"01');
  assert.equal(readFileSync(statePaths(dir).file, "utf8"), before);
  assert.equal(loadState(dir).plans["0101"].steps.length, 1);
});

test("writeAtomic replaces an existing file whole", () => {
  const dir = tmp();
  const file = join(dir, "x.json");
  writeAtomic(file, "first");
  writeAtomic(file, "second");
  assert.equal(readFileSync(file, "utf8"), "second");
  assert.equal(existsSync(`${file}.${process.pid}.tmp`), false);
});

test("a killed conductor restarts from the last completed step", async () => {
  const dir = tmp();
  const stateUrl = pathToFileURL(join(TOOL_DIR, "lib", "state.mjs")).href;
  // A child that completes step 1, starts step 2, then hangs inside it the way a long session does.
  const script = join(dir, "conductor-stand-in.mjs");
  writeFileSync(
    script,
    `import { loadState, startStep, endStep } from ${JSON.stringify(stateUrl)};
     const dir = ${JSON.stringify(dir)};
     const state = loadState(dir);
     const one = startStep(dir, state, "0101", { kind: "implement", phases: ["1", "2"] });
     endStep(dir, state, one, { status: "ok", spendUsd: 2 });
     startStep(dir, state, "0101", { kind: "implement", phases: ["3"] });
     process.stdout.write("in-step\\n");
     setInterval(() => {}, 1000);`,
  );
  const child = spawn(process.execPath, [script], { stdio: ["ignore", "pipe", "inherit"] });
  await new Promise((r) => child.stdout.once("data", r));
  child.kill("SIGKILL");
  await new Promise((r) => child.on("exit", r));

  const state = loadState(dir);
  const rec = state.plans["0101"];
  assert.equal(rec.steps.length, 2);
  assert.equal(rec.steps[1].ended, null, "the kill left step 2 in flight");

  assert.equal(recoverInterrupted(dir, state), 1);
  const reloaded = loadState(dir);
  const done = completedSteps(reloaded.plans["0101"]);
  assert.deepEqual(done.map((s) => s.phases), [["1", "2"]]);
  assert.equal(reloaded.plans["0101"].steps[1].result.status, "interrupted");
  // Recovery is idempotent: nothing is in flight any more.
  assert.equal(recoverInterrupted(dir, reloaded), 0);
  saveState(dir, reloaded);
});

test("endStep records the result and the end time", () => {
  const dir = tmp();
  const state = loadState(dir);
  const s = startStep(dir, state, "0102", { kind: "review", round: 1 });
  endStep(dir, state, s, { status: "parked", reason: "budget", spendUsd: 4.2 });
  const again = loadState(dir).plans["0102"].steps[0];
  assert.ok(again.ended);
  assert.equal(again.result.reason, "budget");
});
