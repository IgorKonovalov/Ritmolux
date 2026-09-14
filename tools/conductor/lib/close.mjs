// Checking a session's claim against the repository. Each verifier returns a list of problems; an
// empty list is the only thing that lets the lane move on, and any problem parks the plan as a
// disagreement with the problems as its detail. The session's word is never the evidence.

import { commitsBetween, head, isClean, resolveCommit, tagObjectType } from "./git.mjs";
import { donePhases, findPlan, readPlanFile } from "./plan.mjs";

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

/**
 * A close: the plan moved to done/ with Status done and a ## Close review section, a clean tree,
 * and — when a version moved — an annotated tag on the branch tip.
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
  if (outcome.tag) {
    const type = tagObjectType(outcome.tag, cwd);
    if (!type) problems.push(`tag ${outcome.tag} does not exist`);
    else if (type !== "tag") problems.push(`tag ${outcome.tag} is lightweight, not annotated`);
    else if (resolveCommit(outcome.tag, cwd) !== head(cwd)) problems.push(`tag ${outcome.tag} is not on the branch tip`);
  }
  return problems;
}
