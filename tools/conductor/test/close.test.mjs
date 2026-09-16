// Reading a close off the branch, rather than out of a session's outcome (backlog 0229). These build
// the branch state by hand — the plan file, the tag, the tree — so each condition `closeOnBranch` and
// `adoptedClose` insist on has a case that isolates it.

import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, readFileSync, renameSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";

import { adoptedClose, closeOnBranch, verifyClose } from "../lib/close.mjs";
import { findPlan } from "../lib/plan.mjs";
import { tmp, writePlan } from "./helpers.mjs";

function sh(args, cwd) {
  const r = spawnSync("git", args, { cwd, encoding: "utf8" });
  assert.equal(r.status, 0, `git ${args.join(" ")}: ${r.stderr}`);
  return r.stdout.trim();
}

const SPEC = { number: "0101", phases: [{ id: "1", owner: "dev" }] };

/**
 * A lane whose plan is as a finished close leaves it: under `done/`, `Status: done`, a
 * `## Close review`, an annotated `v0.1.1` on the tip, clean. Each option undoes one of those.
 */
function lane({ done = true, status = "done", closeReview = true, tag = "annotated", dirty = false } = {}) {
  const repo = tmp("rlx-close-repo-");
  sh(["init", "-q", "-b", "main"], repo);
  sh(["config", "user.email", "conductor-test@example.invalid"], repo);
  sh(["config", "user.name", "Conductor Test"], repo);
  sh(["config", "commit.gpgsign", "false"], repo);
  sh(["config", "tag.gpgSign", "false"], repo);
  sh(["config", "core.autocrlf", "false"], repo);
  writeFileSync(join(repo, "README.md"), "scratch\n");
  writePlan(repo, { ...SPEC, status: `${status} - closed by the conductor`, closeReview: closeReview ? "Round 1: no blockers, no majors." : undefined });
  if (done) {
    mkdirSync(join(repo, "docs", "plans", "done"), { recursive: true });
    const from = findPlan(repo, "0101").path;
    renameSync(from, join(repo, "docs", "plans", "done", "0101-fixture.md"));
  }
  sh(["add", "README.md", "docs"], repo);
  sh(["commit", "-q", "-m", "chore: Release 0.1.1"], repo);
  if (tag === "annotated") sh(["tag", "-a", "v0.1.1", "-m", "chore: Release v0.1.1"], repo);
  if (tag === "lightweight") sh(["tag", "v0.1.1"], repo);
  if (dirty) writeFileSync(join(repo, "suite-output.log"), "still compiling\n");
  return repo;
}

test("a finished close on the branch is found, and read back as the outcome it would have printed", () => {
  const repo = lane();
  const found = closeOnBranch(repo, "0101");
  assert.ok(found, "the close is on the branch");
  assert.match(readFileSync(found.path, "utf8"), /## Close review/);

  const o = adoptedClose({ cwd: repo, plan: "0101", round: 2 });
  assert.equal(o.kind, "closed");
  assert.equal(o.plan, "0101");
  assert.equal(o.version, "0.1.1");
  assert.equal(o.tag, "v0.1.1");
  assert.equal(o.verdict.round, 2);
  // What is adopted is that a close happened, never a claim about what it found: the prose of the
  // `## Close review` is not machine-readable, so the verdict carries the path and no findings.
  assert.deepEqual(o.verdict.findings, []);
  assert.equal(o.verdict.review_path, found.path);
  assert.deepEqual(verifyClose({ cwd: repo, plan: "0101", outcome: o }), []);
});

test("each thing a close must leave behind is separately required", () => {
  for (const [why, opts] of [
    ["the plan is still under docs/plans/", { done: false }],
    ["its Status is not done", { status: "in-progress" }],
    ["it has no ## Close review section", { closeReview: false }],
  ]) {
    const repo = lane(opts);
    assert.equal(closeOnBranch(repo, "0101"), null, why);
    assert.equal(adoptedClose({ cwd: repo, plan: "0101" }), null, why);
  }
  assert.equal(closeOnBranch(lane(), "0102"), null, "a plan that is not in the checkout at all");
});

test("a close whose tree is dirty is found but does not verify, so it is never adopted silently", () => {
  const repo = lane({ dirty: true });
  const o = adoptedClose({ cwd: repo, plan: "0101" });
  assert.ok(o, "the close is on the branch");
  assert.deepEqual(verifyClose({ cwd: repo, plan: "0101", outcome: o }), ["the worktree is not clean"]);
});

test("a docs-only close carries no tag and no version, and still verifies", () => {
  const repo = lane({ tag: "none" });
  const o = adoptedClose({ cwd: repo, plan: "0101" });
  assert.equal(o.tag, null);
  assert.equal(o.version, null);
  assert.deepEqual(verifyClose({ cwd: repo, plan: "0101", outcome: o }), []);
});

test("a lightweight tag on the tip is not read as the close's tag", () => {
  // `verifyClose` rejects a lightweight tag, so adopting one would park the plan on a tag the close
  // never claimed. A version-carrying close leaves an annotated tag or it is not one.
  const repo = lane({ tag: "lightweight" });
  const o = adoptedClose({ cwd: repo, plan: "0101" });
  assert.equal(o.tag, null);
  assert.deepEqual(verifyClose({ cwd: repo, plan: "0101", outcome: o }), []);
});

test("a tag that is not on the tip is not read as the close's tag", () => {
  const repo = lane();
  writeFileSync(join(repo, "README.md"), "a commit after the tag\n");
  sh(["add", "README.md"], repo);
  sh(["commit", "-q", "-m", "docs: after the tag"], repo);
  assert.equal(adoptedClose({ cwd: repo, plan: "0101" }).tag, null);
});
