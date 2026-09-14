// Fast-forwarding main to a closed plan's branch, from the main checkout.
//
// Refuses a main checkout that is dirty or not on main — the owner's work in progress is never
// touched. Three things can stand between a close and the fast-forward, and each is handled once:
//
//   - the branch moved since the conductor's gate last passed on it (`gatedHead`): always true after
//     a close, whose session merged main, bumped and tagged; true again for a plan resumed after a
//     merge park whose conflict the owner resolved in the lane. The gate runs on the new tip and the
//     annotated tag moves onto it before anything reaches main. A missing `gatedHead` gates too.
//   - main moved since the close (main is not an ancestor of the branch): one automatic re-merge in
//     the worktree, the gate again, the tag moved, the fast-forward retried once.
//   - the fast-forward is refused although main IS an ancestor (a held index.lock, a file in the
//     way): nothing a re-merge can fix, so it parks at once rather than paying for a gate.
//
// `onGated(sha)` reports each tip the gate passed on, so a resumed plan does not gate it twice.

import { currentBranch, git, head, isAncestor, isClean, resolveCommit, tagMessage, tagObjectType } from "./git.mjs";

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

export async function fastForwardMain({ repo, worktree, branch, tag, gatedHead, runGate, onGated }) {
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
    if (!gate.ok) return { ok: false, reason: "gate_red", detail: `gate red on the branch as it stands after the close: ${gate.failed.name}`, gate };
    onGated?.(head(worktree));
    const park = moveTag(tag, worktree);
    if (park) return park;
  }

  const first = git(["merge", "--ff-only", branch], repo);
  if (first.code === 0) return { ok: true, head: head(repo), remerged };

  if (isAncestor("main", branch, repo)) {
    return { ok: false, reason: "merge_failed", detail: `fast-forward refused although main is already in ${branch}: ${first.stderr}` };
  }

  const merge = git(["merge", "--no-edit", "main"], worktree);
  if (merge.code !== 0) {
    git(["merge", "--abort"], worktree);
    return { ok: false, reason: "merge_conflict", detail: `main moved and does not merge into ${branch}: ${merge.stdout.split("\n").slice(-3).join(" ")}` };
  }
  remerged = true;
  const gate = await runGate("remerge");
  if (!gate.ok) {
    return { ok: false, reason: "gate_red", detail: `gate red after re-merging main: ${gate.failed.name}`, gate };
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
