// The plan reader, first against real plans in this repository — the shapes the architect's
// template actually produces — then against fixtures for the states a real plan rarely holds.

import assert from "node:assert/strict";
import { test } from "node:test";

import { donePhases, findPlan, nextStep, parsePlan, rangeLabel, readPlanFile, runs } from "../lib/plan.mjs";
import { REPO, planText } from "./helpers.mjs";

test("Plan 0187 reads as six dev phases, 4b included, then a human pilot", () => {
  const found = findPlan(REPO, "0187");
  assert.ok(found, "plan 0187 is in docs/plans/ or docs/plans/done/");
  const plan = readPlanFile(found.path);
  assert.equal(plan.number, "0187");
  assert.equal(plan.title, "The conductor runs the lanes");
  assert.deepEqual(plan.errors, []);
  assert.deepEqual(
    plan.phases.map((p) => [p.id, p.owner]),
    [["1", "dev"], ["2", "dev"], ["3", "dev"], ["4", "dev"], ["4b", "dev"], ["5", "dev"], ["6", "human"]],
  );
  assert.match(plan.phases[0].stopCondition, /^if project skills or hooks do not load under `-p`, stop/);
  assert.deepEqual(runs(plan), [
    { owner: "dev", phases: ["1", "2", "3", "4", "4b", "5"] },
    { owner: "human", phases: ["6"] },
  ]);
  assert.deepEqual(plan.log.rows.map((r) => r.id), ["1", "2", "3", "4", "4b", "5", "6"]);
});

test("closed Plan 0172 reads as a dev run then a studio-builder run, every row done", () => {
  const plan = readPlanFile(findPlan(REPO, "0172").path);
  assert.equal(plan.statusWord, "done");
  assert.deepEqual(runs(plan), [
    { owner: "dev", phases: ["1", "2", "3"] },
    { owner: "studio-builder", phases: ["4"] },
  ]);
  assert.deepEqual([...donePhases(plan)], ["1", "2", "3", "4"]);
  assert.deepEqual(plan.log.rows.map((r) => r.commit), ["c303c1f", "6ce0594", "db0df8e", "fc4c9ee"]);
  assert.deepEqual(nextStep(plan), { kind: "review" });
});

const MIXED = {
  number: "0101",
  phases: [
    { id: "1", owner: "dev" },
    { id: "2", owner: "dev" },
    { id: "3", owner: "studio-builder" },
    { id: "4", owner: "human" },
    { id: "5", owner: "dev" },
  ],
};

test("the next step is the first run with a phase the log does not mark done", () => {
  const fresh = parsePlan(planText(MIXED));
  assert.deepEqual(nextStep(fresh), { kind: "implement", owner: "dev", phases: ["1", "2"], lastRun: false });

  const midRun = parsePlan(planText({ ...MIXED, rows: { 1: { state: "done", commit: "abc1234" }, 2: { state: "committed with this row" } } }));
  assert.deepEqual([...donePhases(midRun)], ["1", "2"]);
  assert.deepEqual(nextStep(midRun), { kind: "implement", owner: "studio-builder", phases: ["3"], lastRun: false });

  const atHuman = parsePlan(planText({ ...MIXED, rows: { 1: { state: "done" }, 2: { state: "done" }, 3: { state: "done" } } }));
  assert.deepEqual(nextStep(atHuman), { kind: "human", owner: "human", phases: ["4"] });

  const afterHuman = parsePlan(planText({ ...MIXED, rows: { 1: { state: "done" }, 2: { state: "done" }, 3: { state: "done" }, 4: { state: "done" } } }));
  assert.deepEqual(nextStep(afterHuman), { kind: "implement", owner: "dev", phases: ["5"], lastRun: true });
});

test("a missing or foreign owner tag is a plan error", () => {
  const text = planText({ number: "0101", phases: [{ id: "1", owner: "dev" }] }).replace("- **Owner skill:** dev", "- **Owner skill:** architect");
  assert.deepEqual(parsePlan(text).errors, ["Phase 1 has no valid owner tag (architect)"]);
});

test("range labels", () => {
  assert.equal(rangeLabel(["4b"]), "4b");
  assert.equal(rangeLabel(["1", "2", "3"]), "1-3");
});
