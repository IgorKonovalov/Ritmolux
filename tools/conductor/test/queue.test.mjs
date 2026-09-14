// queue.json and local.json validation, and the conductor's preflight refusals.

import assert from "node:assert/strict";
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";

import { paths, preflight } from "../conductor.mjs";
import { loadLocal, validateQueue } from "../lib/queue.mjs";
import { FAKE, tmp, writePlan } from "./helpers.mjs";

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

test("a closed plan, a missing plan, a duplicate and a malformed key are each rejected", () => {
  const q = validateQueue(
    { lanes: { a: ["0090", "0101", "0999"], b: ["0101"] }, plans: { "0101": { add_dirs: "x", later: 1 } } },
    scratchRepo(),
  );
  const text = q.errors.join("\n");
  assert.match(text, /plan 0090: already closed/);
  assert.match(text, /plan 0999: no docs\/plans\/0999-\*\.md/);
  assert.match(text, /plan 0101: listed twice \(lanes a and b\)/);
  assert.match(text, /plan 0101: "add_dirs" must be a list of paths/);
  assert.match(text, /plan 0101: unknown key "later"/);
});

test("a closed plan the conductor itself merged stays accepted in the queue", () => {
  const repo = scratchRepo();
  const queue = { lanes: { a: ["0090", "0101"] } };
  assert.match(validateQueue(queue, repo, new Set(["0090"])).errors.join("\n"), /plan 0090: already closed/);
  assert.deepEqual(validateQueue(queue, repo, new Set(["0090"]), new Set(["0090"])).errors, []);
});

test("local.json is required, and every step budget must be set by the owner", () => {
  const dir = tmp();
  assert.match(loadLocal(join(dir, "local.json")).errors[0], /local\.json not found/);

  writeFileSync(join(dir, "local.json"), JSON.stringify({ budget_usd: { implement: 5, review: 2 }, max_open_worktrees: 2 }));
  assert.deepEqual(loadLocal(join(dir, "local.json")).errors, ["local.json: budget_usd.fix must be a positive number"]);

  // The committed example carries zeros on purpose, so copying it without editing is refused.
  writeFileSync(join(dir, "local.json"), JSON.stringify({ budget_usd: { implement: 0, fix: 0, review: 0 }, max_open_worktrees: 3 }));
  assert.equal(loadLocal(join(dir, "local.json")).errors.length, 3);

  writeFileSync(join(dir, "local.json"), JSON.stringify({ budget_usd: { implement: 5, fix: 3, review: 4 }, max_open_worktrees: 3 }));
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

const LOCAL = { budget_usd: { implement: 5, fix: 3, review: 4 }, max_open_worktrees: 3 };

test("preflight refuses to start without local.json", () => {
  const p = scratchTool({ version: "2.1.270 (Claude Code)" });
  const r = preflight(p, { claude: FAKE });
  assert.equal(r.errors.length, 1);
  assert.match(r.errors[0], /local\.json not found/);
});

test("preflight refuses a claude --version it has not been verified on", () => {
  const p = scratchTool({ local: LOCAL, version: "2.1.999 (Claude Code)" });
  const r = preflight(p, { claude: FAKE });
  assert.equal(r.errors.length, 1);
  assert.match(r.errors[0], /claude 2\.1\.999 is not a verified CLI version \(verified: 2\.1\.270\)/);
});

test("preflight passes on the verified version with local.json and a valid queue", () => {
  const p = scratchTool({ local: LOCAL, version: "2.1.270 (Claude Code)" });
  assert.deepEqual(preflight(p, { claude: FAKE }).errors, []);
  delete process.env.FAKE_CLAUDE_VERSION;
});
