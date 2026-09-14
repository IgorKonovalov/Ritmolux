// Opening and removing a plan lane. A lane is WORK/rlx-plan-NNNN on plan-NNNN-<slug>, cut from
// main in the main checkout (ADR-0053).
//
// Removal runs from the main checkout after the fast-forward: `git worktree remove`, `git worktree
// prune`, `git branch -d`. `-d` refuses an unmerged branch, which is the safety property — a refusal
// means the fast-forward did not land. On Windows a removal fails while any process holds the
// directory; that is an inbox entry, not a park, because the plan has already merged.

import { existsSync } from "node:fs";
import { basename, join } from "node:path";

import { branchExists, git, head } from "./git.mjs";

export function laneNames(worktreeRoot, planFile, plan) {
  const slug = basename(planFile, ".md").replace(/^\d{4}-/, "");
  return { worktree: join(worktreeRoot, `rlx-plan-${plan}`), branch: `plan-${plan}-${slug}` };
}

/** Opens (or re-opens after a restart) the lane. Returns { ok, worktree, branch, base, detail }. */
export function openLane({ repo, worktree, branch }) {
  if (existsSync(worktree)) {
    const current = git(["branch", "--show-current"], worktree).stdout;
    if (current !== branch) return { ok: false, detail: `${worktree} exists and is on "${current}", not ${branch}` };
    return { ok: true, worktree, branch, base: null, reopened: true };
  }
  const args = branchExists(branch, repo)
    ? ["worktree", "add", worktree, branch]
    : ["worktree", "add", "-b", branch, worktree, "main"];
  const r = git(args, repo);
  if (r.code !== 0) return { ok: false, detail: `git ${args.join(" ")}: ${r.stderr}` };
  return { ok: true, worktree, branch, base: head(worktree), reopened: false };
}

export function removeLane({ repo, worktree, branch }) {
  const problems = [];
  const remove = git(["worktree", "remove", worktree], repo);
  if (remove.code !== 0) problems.push(`git worktree remove: ${remove.stderr}`);
  git(["worktree", "prune"], repo);
  const del = git(["branch", "-d", branch], repo);
  if (del.code !== 0) problems.push(`git branch -d: ${del.stderr}`);
  return { ok: problems.length === 0, detail: problems.join("; ") };
}
