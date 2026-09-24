// Merging main into a lane, and fast-forwarding main to a closed plan's branch from the main checkout.
//
// Refuses a main checkout that is dirty or not on main — the owner's work in progress is never
// touched. Three things can stand between a close and the fast-forward, and each is handled once:
//
//   - the branch moved since the conductor's gate last passed on it (`gatedHead`): always true after
//     a close, whose session merged main, bumped and tagged; true again for a plan resumed after a
//     merge park whose conflict the owner resolved in the lane. The gate runs on the new tip and the
//     annotated tag moves onto it before anything reaches main. A missing `gatedHead` gates too.
//   - main moved since the close (main is not an ancestor of the branch): one automatic re-merge in
//     the worktree, the gate again, the tag moved, the fast-forward retried once. A re-merge that
//     conflicts is aborted and handed to `resolveConflict`, which runs one merge session (ADR-0248);
//     without one it parks `merge_conflict`.
//   - the fast-forward is refused although main IS an ancestor (a held index.lock, a file in the
//     way): nothing a re-merge can fix, so it parks at once rather than paying for a gate.
//
// `onGated(sha)` reports each tip the gate passed on, so a resumed plan does not gate it twice.
// `runGate` may repair before it answers (ADR-0248): a result carrying `park` is the park to take, and
// a green one after a repair names a tip the tag then moves onto, as for any other moved tip.

import { currentBranch, git, head, isAncestor, isClean, resolveCommit, tagMessage, tagObjectType } from "./git.mjs";

/** The paths a merge in progress left conflicted in `cwd`, `/`-separated. */
export function conflictedPaths(cwd) {
  const r = git(["diff", "--name-only", "--diff-filter=U"], cwd);
  return r.code === 0 && r.stdout ? r.stdout.split("\n").map((p) => p.replace(/\\/g, "/")) : [];
}

/**
 * Merges `main` into the worktree's branch. Returns { ok: true, merged } — `merged` false when main
 * was already in the branch — or { ok: false, paths, detail } for a conflict, with the merge aborted
 * so the tree is clean again and the conflicted paths are what a merge session is handed.
 */
export function mergeMainInto(worktree) {
  if (isAncestor("main", "HEAD", worktree)) return { ok: true, merged: false };
  const r = git(["merge", "--no-edit", "main"], worktree);
  if (r.code === 0) return { ok: true, merged: true };
  const paths = conflictedPaths(worktree);
  git(["merge", "--abort"], worktree);
  return { ok: false, paths, detail: r.stdout.split("\n").slice(-3).join(" ") };
}

/** Moves annotated `tag` onto the worktree's tip, keeping its message. Returns a park or null. */
function moveTag(tag, worktree) {
  if (!tag || resolveCommit(tag, worktree) === head(worktree)) return null;
  if (tagObjectType(tag, worktree) !== "tag") {
    return { ok: false, reason: "disagreement", detail: `tag ${tag} is not annotated` };
  }
  const message = tagMessage(tag, worktree);
  const moved = git(["tag", "-a", "-f", tag, "-m", message], worktree);
  if (moved.code !== 0 || resolveCommit(tag, worktree) !== head(worktree)) {
    return { ok: false, reason: "merge_failed", detail: `could not move ${tag} onto the branch tip: ${moved.stderr}` };
  }
  return null;
}

export async function fastForwardMain({ repo, worktree, branch, tag, gatedHead, runGate, onGated, resolveConflict }) {
  if (currentBranch(repo) !== "main") {
    return { ok: false, reason: "main_dirty", detail: `the main checkout is on "${currentBranch(repo)}", not main` };
  }
  if (!isClean(repo)) {
    return { ok: false, reason: "main_dirty", detail: "the main checkout has uncommitted changes; the fast-forward will not touch them" };
  }

  let remerged = false;
  if (head(worktree) !== gatedHead) {
    if (!isClean(worktree)) {
      return { ok: false, reason: "disagreement", detail: `${branch} has uncommitted changes since the close` };
    }
    const gate = await runGate("post-close");
    if (!gate.ok) return gate.park ? { ok: false, ...gate.park, gate } : { ok: false, reason: "gate_red", detail: `gate red on the branch as it stands after the close: ${gate.failed.name}`, gate };
    onGated?.(head(worktree));
    const park = moveTag(tag, worktree);
    if (park) return park;
  }

  const first = git(["merge", "--ff-only", branch], repo);
  if (first.code === 0) return { ok: true, head: head(repo), remerged };

  if (isAncestor("main", branch, repo)) {
    return { ok: false, reason: "merge_failed", detail: `fast-forward refused although main is already in ${branch}: ${first.stderr}` };
  }

  const merge = mergeMainInto(worktree);
  if (!merge.ok) {
    if (!resolveConflict) return { ok: false, reason: "merge_conflict", detail: `main moved and does not merge into ${branch}: ${merge.detail}` };
    const park = await resolveConflict(merge.paths);
    if (park) return park;
  }
  remerged = true;
  const gate = await runGate("remerge");
  if (!gate.ok) {
    return gate.park ? { ok: false, ...gate.park, gate } : { ok: false, reason: "gate_red", detail: `gate red after re-merging main: ${gate.failed.name}`, gate };
  }
  onGated?.(head(worktree));
  const park = moveTag(tag, worktree);
  if (park) return park;
  const second = git(["merge", "--ff-only", branch], repo);
  if (second.code !== 0) {
    return { ok: false, reason: "merge_failed", detail: `fast-forward refused twice: ${second.stderr}` };
  }
  return { ok: true, head: head(repo), remerged };
}
