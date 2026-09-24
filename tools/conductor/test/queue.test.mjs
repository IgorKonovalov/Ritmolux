// queue.json and local.json validation, and the conductor's preflight refusals.

import assert from "node:assert/strict";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";

import { VERIFIED_CLI, paths, preflight } from "../conductor.mjs";
import { loadLocal, pruneQueue, validateQueue } from "../lib/queue.mjs";
import { FAKE, TOOL_DIR, tmp, writePlan } from "./helpers.mjs";

const dev = (id) => ({ id, owner: "dev" });

function scratchRepo() {
  const repo = tmp();
  writePlan(repo, { number: "0101", phases: [dev("1")] });
  writePlan(repo, { number: "0102", phases: [dev("1")] });
  writePlan(repo, { number: "0103", phases: [dev("1")], status: "draft" });
  writePlan(repo, { number: "0104", phases: [dev("1")], status: "in-progress" });
  writePlan(repo, { number: "0090", phases: [dev("1")], status: "done — closed" }, { done: true });
  return repo;
}

test("a valid queue passes, with dependencies inside the queue and in done/", () => {
  const repo = scratchRepo();
  const q = validateQueue(
    { lanes: { a: ["0101"], b: ["0102"] }, plans: { "0102": { after: ["0101", "0090"], add_dirs: ["../corpus"] } } },
    repo,
  );
  assert.deepEqual(q.errors, []);
  assert.deepEqual(q.lanes, { a: ["0101"], b: ["0102"] });
  assert.deepEqual(q.plans["0102"].after, ["0101", "0090"]);
  assert.deepEqual(q.plans["0102"].add_dirs, ["../corpus"]);
});

test("a plan that is not approved is rejected with its name", () => {
  const q = validateQueue({ lanes: { a: ["0101", "0103"] } }, scratchRepo());
  assert.equal(q.errors.length, 1);
  assert.match(q.errors[0], /^plan 0103: Status is "draft", not approved$/);
});

test("an in-progress plan is accepted only when the conductor already started it", () => {
  const repo = scratchRepo();
  assert.match(validateQueue({ lanes: { a: ["0104"] } }, repo).errors[0], /plan 0104: Status is "in-progress"/);
  assert.deepEqual(validateQueue({ lanes: { a: ["0104"] } }, repo, new Set(["0104"])).errors, []);
});

test("a dependency on a plan neither queued nor done is rejected with both names", () => {
  const q = validateQueue({ lanes: { a: ["0101"] }, plans: { "0101": { after: ["0177"] } } }, scratchRepo());
  assert.equal(q.errors.length, 1);
  assert.match(q.errors[0], /^plan 0101: depends on plan 0177, which is neither in the queue nor in docs\/plans\/done\/$/);
});

test("a missing plan, a duplicate and a malformed key are each rejected", () => {
  const q = validateQueue(
    { lanes: { a: ["0101", "0999"], b: ["0101"] }, plans: { "0101": { add_dirs: "x", later: 1 } } },
    scratchRepo(),
  );
  const text = q.errors.join("\n");
  assert.match(text, /plan 0999: no docs\/plans\/0999-\*\.md/);
  assert.match(text, /plan 0101: listed twice \(lanes a and b\)/);
  assert.match(text, /plan 0101: "add_dirs" must be a list of paths/);
  assert.match(text, /plan 0101: unknown key "later"/);
});

// ADR-0220, reversing backlog 0240's demonstration: state/ is gitignored, so a clone or a wiped
// state has empty sets, and every merged plan still listed used to be a fatal preflight error.
test("a queued plan under done/ is a notice rather than a fatal error, whatever the state says", () => {
  const repo = scratchRepo();
  const queue = { lanes: { a: ["0090", "0101"] } };
  for (const started of [new Set(), new Set(["0090"])]) {
    const q = validateQueue(queue, repo, started);
    assert.deepEqual(q.errors, []);
    assert.equal(q.notices.length, 1);
    assert.match(q.notices[0], /^plan 0090: already merged \(0090-fixture\.md is under docs\/plans\/done\/\); `prune` drops it/);
  }
  // A queued number with no plan file at all is a different thing, and still fatal.
  const missing = validateQueue({ lanes: { a: ["0999"] } }, repo);
  assert.match(missing.errors.join("\n"), /plan 0999: no docs\/plans\/0999-\*\.md/);
  assert.deepEqual(missing.notices, []);
});

test("prune drops every merged plan from its lane and touches nothing else", () => {
  const repo = scratchRepo();
  const queue = { lanes: { a: ["0090", "0101"], b: ["0102"], c: "not a list" }, plans: { "0101": { after: ["0090"] } } };
  const { queue: pruned, dropped } = pruneQueue(queue, repo);
  assert.deepEqual(dropped, [{ plan: "0090", lane: "a", file: "0090-fixture.md" }]);
  assert.deepEqual(pruned, { lanes: { a: ["0101"], b: ["0102"], c: "not a list" }, plans: { "0101": { after: ["0090"] } } });

  // Nothing merged: the same object back, and nothing dropped.
  const tidy = pruneQueue(pruned, repo);
  assert.deepEqual(tidy.dropped, []);
  assert.deepEqual(tidy.queue, pruned);
});

test("local.json is required, and every step budget must be set by the owner", () => {
  const dir = tmp();
  assert.match(loadLocal(join(dir, "local.json")).errors[0], /local\.json not found/);

  writeFileSync(join(dir, "local.json"), JSON.stringify({ budget_usd: { implement: 5, review: 2, merge: 2 }, run_budget_usd: 60, max_open_worktrees: 2 }));
  assert.deepEqual(loadLocal(join(dir, "local.json")).errors, ["local.json: budget_usd.fix must be a positive number"]);

  // The committed example carries zeros on purpose, so copying it without editing is refused.
  writeFileSync(join(dir, "local.json"), readFileSync(join(TOOL_DIR, "local.example.json"), "utf8"));
  assert.equal(loadLocal(join(dir, "local.json")).errors.length, 5);

  // A resident run spends while nobody is looking, so its ceiling is required too (ADR-0250).
  writeFileSync(join(dir, "local.json"), JSON.stringify({ budget_usd: { implement: 5, fix: 3, review: 4, merge: 2 }, max_open_worktrees: 3 }));
  assert.deepEqual(loadLocal(join(dir, "local.json")).errors, ["local.json: run_budget_usd must be a positive number"]);

  writeFileSync(join(dir, "local.json"), JSON.stringify({ budget_usd: { implement: 5, fix: 3, review: 4, merge: 2 }, run_budget_usd: 60, max_open_worktrees: 3 }));
  assert.deepEqual(loadLocal(join(dir, "local.json")).errors, []);
});

function scratchTool({ local, version }) {
  const repo = scratchRepo();
  const toolDir = join(repo, "tools", "conductor");
  mkdirSync(toolDir, { recursive: true });
  writeFileSync(join(toolDir, "queue.json"), JSON.stringify({ lanes: { a: ["0101"] } }));
  if (local) writeFileSync(join(toolDir, "local.json"), JSON.stringify(local));
  process.env.FAKE_CLAUDE_VERSION = version;
  return paths({ repo, toolDir });
}

const LOCAL = { budget_usd: { implement: 5, fix: 3, review: 4, merge: 2 }, run_budget_usd: 60, max_open_worktrees: 3 };

test("preflight refuses to start without local.json", () => {
  const p = scratchTool({ version: "2.1.270 (Claude Code)" });
  const r = preflight(p, { claude: FAKE });
  assert.equal(r.errors.length, 1);
  assert.match(r.errors[0], /local\.json not found/);
});

test("preflight refuses a claude --version it has not been verified on", () => {
  // A minor above every verified version: never a patch update. Built from the live VERIFIED_CLI,
  // so a legitimately verified version may be added without editing this test.
  const [major, minor] = VERIFIED_CLI.at(-1).split(".").map(Number);
  const unverified = `${major}.${minor + 1}.0`;
  const p = scratchTool({ local: LOCAL, version: `${unverified} (Claude Code)` });
  const r = preflight(p, { claude: FAKE });
  assert.equal(r.errors.length, 1);
  assert.ok(r.errors[0].startsWith(`claude ${unverified} is not a verified CLI version (verified: ${VERIFIED_CLI.join(", ")})`), r.errors[0]);
  assert.deepEqual(r.warnings, []);
});

test("preflight passes on the verified version with local.json and a valid queue", () => {
  const p = scratchTool({ local: LOCAL, version: "2.1.270 (Claude Code)" });
  assert.deepEqual(preflight(p, { claude: FAKE }).errors, []);
  delete process.env.FAKE_CLAUDE_VERSION;
});
