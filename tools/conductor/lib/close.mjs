// Checking a session's claim against the repository. Each verifier returns a list of problems; an
// empty list is the only thing that lets the lane move on, and any problem parks the plan as a
// disagreement with the problems as its detail. The session's word is never the evidence.

import { commitsBetween, git, head, isAncestor, isClean, resolveCommit, tagObjectType } from "./git.mjs";
import { donePhases, findPlan, readPlanFile } from "./plan.mjs";

/**
 * The close a session already committed on this branch, or null: the plan under `done/` with
 * `Status: done` and a `## Close review` section. A review session that committed its close and then
 * lost its outcome — its turn ended on a backgrounded command, or its result was malformed — leaves
 * exactly this. Reviewing such a branch again would write a second close, a second version and a
 * second tag, so the lane verifies it instead (backlog 0229).
 */
export function closeOnBranch(cwd, plan) {
  const found = findPlan(cwd, plan);
  if (!found || !found.done) return null;
  const doc = readPlanFile(found.path);
  if (doc.statusWord !== "done" || !doc.hasCloseReview) return null;
  return { path: found.path, doc };
}

/** The annotated `vX.Y.Z` tag on the tip, or null. A docs-only close leaves none, which is valid. */
function tagOnTip(cwd) {
  const r = git(["tag", "--points-at", "HEAD", "--list", "v*"], cwd);
  if (r.code !== 0 || !r.stdout) return null;
  return r.stdout.split("\n").map((t) => t.trim()).find((t) => /^v\d+\.\d+\.\d+$/.test(t) && tagObjectType(t, cwd) === "tag") ?? null;
}

/**
 * The `closed` outcome a landed close would have printed, read back off the branch: the tag on the
 * tip and the version it names, and a verdict that carries the plan's own `## Close review` as its
 * review path. The findings list is empty because the prose is not machine-readable — what is
 * adopted is that the close happened, never a claim about what it found.
 *
 * Returns null when there is no close on the branch to adopt.
 */
export function adoptedClose({ cwd, plan, round = 1 }) {
  const found = closeOnBranch(cwd, plan);
  if (!found) return null;
  const tag = tagOnTip(cwd);
  return {
    kind: "closed",
    plan,
    version: tag ? tag.slice(1) : null,
    tag,
    verdict: { round, blockers: 0, majors: 0, minors: 0, review_path: found.path, findings: [] },
  };
}

function claimedCommits(claims, made, cwd, problems) {
  const matched = new Set();
  for (const c of claims) {
    const full = resolveCommit(c, cwd);
    if (!full) problems.push(`claimed commit ${c} does not exist`);
    else if (!made.includes(full)) problems.push(`claimed commit ${c} was not made by this step`);
    else matched.add(full);
  }
  const unclaimed = made.filter((m) => !matched.has(m));
  if (unclaimed.length) problems.push(`commits made but not claimed: ${unclaimed.map((s) => s.slice(0, 7)).join(", ")}`);
}

/** An implement step: its commits, the log rows for its range, and a clean tree. */
export function verifyImplement({ cwd, plan, phases, before, outcome }) {
  const problems = [];
  if (outcome.kind !== "phases_done") return [`expected a phases_done outcome, got ${outcome.kind}`];
  const made = commitsBetween(before, head(cwd), cwd);
  if (made.length === 0) problems.push("the step made no commit");
  claimedCommits(outcome.commits, made, cwd, problems);
  // Phase ids are strings ("4b"); a session may print a numeric `through`, which validation accepts.
  if (String(outcome.through) !== phases.at(-1)) {
    problems.push(`outcome says through Phase ${outcome.through}; the step was Phases ${phases[0]}-${phases.at(-1)}`);
  }
  const found = findPlan(cwd, plan);
  if (!found) problems.push(`plan ${plan} is missing from the worktree`);
  else {
    const doc = readPlanFile(found.path);
    const done = donePhases(doc);
    for (const id of phases) if (!done.has(id)) problems.push(`the log does not mark Phase ${id} done`);
    for (const row of doc.log.rows) {
      if (!phases.includes(row.id) || !row.commit) continue;
      const full = resolveCommit(row.commit, cwd);
      if (!full || !made.includes(full)) problems.push(`log row for Phase ${row.id} names ${row.commit}, not a commit this step made`);
    }
  }
  if (!isClean(cwd)) problems.push("the worktree is not clean");
  return problems;
}

/** A fix step: its commits exist and were made here, each resolution names one of them, clean tree. */
export function verifyFix({ cwd, before, outcome, findingCount }) {
  const problems = [];
  if (outcome.kind !== "fixed") return [`expected a fixed outcome, got ${outcome.kind}`];
  const made = commitsBetween(before, head(cwd), cwd);
  if (made.length === 0) problems.push("the fix step made no commit");
  claimedCommits(outcome.commits, made, cwd, problems);
  for (const r of outcome.resolved) {
    if (r.finding >= findingCount) problems.push(`resolved finding ${r.finding} does not exist`);
    const full = resolveCommit(r.commit, cwd);
    if (!full || !made.includes(full)) problems.push(`finding ${r.finding} is resolved in ${r.commit}, not a commit this step made`);
  }
  if (!isClean(cwd)) problems.push("the worktree is not clean");
  return problems;
}

/** The paths a commit changes, `/`-separated; a merge commit is compared against its first parent. */
function changedPaths(sha, cwd) {
  const r = git(["diff-tree", "-r", "-m", "--first-parent", "--no-commit-id", "--name-only", "--root", sha], cwd);
  return r.code === 0 ? r.stdout.split("\n").filter(Boolean) : [];
}

/**
 * Each finding a close marked repaired (ADR-0209): its `fixed_in` commit must exist, be on the
 * branch, and change that finding's file.
 */
function repairProblems(outcome, cwd) {
  const problems = [];
  for (const [i, f] of (outcome.verdict?.findings ?? []).entries()) {
    if (!f.fixed_in) continue;
    const full = resolveCommit(f.fixed_in, cwd);
    if (!full) problems.push(`finding ${i} is fixed_in ${f.fixed_in}, which does not exist`);
    else if (!isAncestor(full, "HEAD", cwd)) problems.push(`finding ${i} is fixed_in ${f.fixed_in}, which is not on the branch`);
    else if (!changedPaths(full, cwd).includes(f.file.replace(/\\/g, "/"))) problems.push(`finding ${i} is fixed_in ${f.fixed_in}, which does not change ${f.file}`);
  }
  return problems;
}

/**
 * A close: the plan moved to done/ with Status done and a ## Close review section, a clean tree,
 * every repaired finding's commit on the branch and touching its file, and — when a version
 * moved — an annotated tag on the branch tip.
 */
export function verifyClose({ cwd, plan, outcome }) {
  const problems = [];
  if (outcome.kind !== "closed") return [`expected a closed outcome, got ${outcome.kind}`];
  const found = findPlan(cwd, plan);
  if (!found || !found.done) problems.push(`plan ${plan} is not under docs/plans/done/`);
  else {
    const doc = readPlanFile(found.path);
    if (doc.statusWord !== "done") problems.push(`plan ${plan} Status is "${doc.status}", not done`);
    if (!doc.hasCloseReview) problems.push(`plan ${plan} has no ## Close review section`);
  }
  if (!isClean(cwd)) problems.push("the worktree is not clean");
  problems.push(...repairProblems(outcome, cwd));
  if (outcome.tag) {
    const type = tagObjectType(outcome.tag, cwd);
    if (!type) problems.push(`tag ${outcome.tag} does not exist`);
    else if (type !== "tag") problems.push(`tag ${outcome.tag} is lightweight, not annotated`);
    else if (resolveCommit(outcome.tag, cwd) !== head(cwd)) problems.push(`tag ${outcome.tag} is not on the branch tip`);
  }
  return problems;
}
