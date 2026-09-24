// Checking a session's claim against the repository. Each verifier returns a list of problems; an
// empty list is the only thing that lets the lane move on, and any problem parks the plan as a
// disagreement with the problems as its detail. The session's word is never the evidence.

import { commitsBetween, git, head, isAncestor, isClean, resolveCommit, tagObjectType } from "./git.mjs";
import { donePhases, findPlan, nonBlocking, readPlanFile, rowIsOwed } from "./plan.mjs";

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

/** The lines of `paths` in `cwd`'s working tree that still carry a conflict marker, as `path:line`. */
function markerLines(paths, cwd) {
  if (paths.length === 0) return [];
  const r = git(["grep", "-n", "-I", "-E", "^(<{7}|>{7})( |$)", "--", ...paths], cwd);
  return r.code === 0 && r.stdout ? r.stdout.split("\n").map((l) => l.split(":").slice(0, 2).join(":")) : [];
}

/**
 * A merge session (ADR-0248): its commit exists, was made by this step, is on the branch tip's
 * history, is a merge whose second parent is `main` as the conductor saw it when it handed the
 * conflict over (or a later main tip, when main moved again meanwhile), the tree is clean, and no
 * path it was handed still carries a conflict marker. `git grep` reads the working tree, which the
 * clean-tree check makes the committed one.
 */
export function verifyMerge({ cwd, before, mainTip, paths, outcome }) {
  if (outcome.kind !== "merged") return [`expected a merged outcome, got ${outcome.kind}`];
  const problems = [];
  const made = commitsBetween(before, head(cwd), cwd);
  const full = resolveCommit(outcome.commit, cwd);
  if (!full) problems.push(`claimed merge commit ${outcome.commit} does not exist`);
  else if (!made.includes(full)) problems.push(`claimed merge commit ${outcome.commit} was not made by this step`);
  else {
    const parents = git(["rev-list", "--parents", "-n", "1", full], cwd).stdout.split(" ").slice(1);
    const second = parents[1];
    if (parents.length !== 2) problems.push(`${outcome.commit} is not a two-parent merge commit`);
    else if (second !== mainTip && !(isAncestor(mainTip, second, cwd) && isAncestor(second, "main", cwd))) {
      problems.push(`${outcome.commit}'s second parent ${second.slice(0, 7)} is not main's tip ${mainTip.slice(0, 7)}`);
    }
  }
  if (!isClean(cwd)) problems.push("the worktree is not clean");
  const markers = markerLines(paths, cwd);
  if (markers.length) problems.push(`conflict markers left in ${markers.join(", ")}`);
  return problems;
}

/** The paths a commit changes, `/`-separated; a merge commit is compared against its first parent. */
function changedPaths(sha, cwd) {
  const r = git(["diff-tree", "-r", "-m", "--first-parent", "--no-commit-id", "--name-only", "--root", sha], cwd);
  return r.code === 0 ? r.stdout.split("\n").filter(Boolean) : [];
}

/**
 * The paths `file` had at commit `sha`, other than `file` itself. Two sources: every rename git
 * pairs between the `sha` tree and the `HEAD` tree, at git's default similarity - a tree-to-tree
 * diff, so a merge in between neither hides a rename nor needs walking - and the plan's own move,
 * which is known by construction: the plan's `done/` path came from the same basename directly under
 * `docs/plans/`, however much the close grew the file past git's similarity line.
 */
function earlierPaths(file, sha, cwd, plan) {
  const paths = new Set();
  const r = git(["diff", "--name-status", "-z", "-M", sha, "HEAD"], cwd);
  if (r.code === 0) {
    // -z: `R<score>\0<old>\0<new>\0` for a rename, `<status>\0<path>\0` for everything else.
    const t = r.stdout.split("\0");
    for (let k = 0; k < t.length; ) {
      if (/^[RC]\d*$/.test(t[k])) {
        if (t[k][0] === "R" && t[k + 2] === file) paths.add(t[k + 1]);
        k += 3;
      } else k += 2;
    }
  }
  const found = findPlan(cwd, plan);
  if (found?.done && file === `docs/plans/done/${found.file}`) paths.add(`docs/plans/${found.file}`);
  paths.delete(file);
  return [...paths];
}

/**
 * Each finding a close marked repaired (ADR-0209): its `fixed_in` commit must exist, be on the
 * branch, and change that finding's file - under the path the finding names, or under a path the
 * file had at that commit, since a close moves the plan file after repairing it.
 */
function repairProblems(outcome, cwd, plan) {
  const problems = [];
  for (const [i, f] of (outcome.verdict?.findings ?? []).entries()) {
    if (!f.fixed_in) continue;
    const full = resolveCommit(f.fixed_in, cwd);
    if (!full) problems.push(`finding ${i} is fixed_in ${f.fixed_in}, which does not exist`);
    else if (!isAncestor(full, "HEAD", cwd)) problems.push(`finding ${i} is fixed_in ${f.fixed_in}, which is not on the branch`);
    else {
      const file = f.file.replace(/\\/g, "/");
      const changed = changedPaths(full, cwd);
      const earlier = earlierPaths(file, full, cwd, plan);
      if (![file, ...earlier].some((p) => changed.includes(p))) {
        const also = earlier.length ? ` (nor ${earlier.join(", ")}, its path at ${f.fixed_in})` : "";
        problems.push(`finding ${i} is fixed_in ${f.fixed_in}, which does not change ${f.file}${also}`);
      }
    }
  }
  return problems;
}

/**
 * An `owed` row is a close's to leave only on a human phase the plan marks `Blocks merge: no`
 * (ADR-0249); on any other phase it is a phase the plan closed without.
 */
function owedProblems(doc, plan) {
  const byId = new Map(doc.phases.map((p) => [p.id, p]));
  return doc.log.rows
    .filter((row) => rowIsOwed(row) && byId.has(row.id) && !nonBlocking(byId.get(row.id)))
    .map((row) => `plan ${plan} Phase ${row.id} reads owed, but only a human phase marked Blocks merge: no may be owed`);
}

/**
 * A close: the plan moved to done/ with Status done and a ## Close review section, no row owed that
 * may not be, a clean tree, every repaired finding's commit on the branch and
 * touching its file, and — when a version moved — an annotated tag on the branch tip.
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
    problems.push(...owedProblems(doc, plan));
  }
  if (!isClean(cwd)) problems.push("the worktree is not clean");
  problems.push(...repairProblems(outcome, cwd, plan));
  if (outcome.tag) {
    const type = tagObjectType(outcome.tag, cwd);
    if (!type) problems.push(`tag ${outcome.tag} does not exist`);
    else if (type !== "tag") problems.push(`tag ${outcome.tag} is lightweight, not annotated`);
    else if (resolveCommit(outcome.tag, cwd) !== head(cwd)) problems.push(`tag ${outcome.tag} is not on the branch tip`);
  }
  return problems;
}
