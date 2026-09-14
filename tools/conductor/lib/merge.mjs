// Fast-forwarding main to a closed plan's branch, from the main checkout.
//
// Refuses a main checkout that is dirty or not on main — the owner's work in progress is never
// touched. When main moved since the close, the branch takes one automatic re-merge in its
// worktree, the gate runs again, the annotated tag moves onto the new tip with its message kept,
// and the fast-forward is retried once. Anything else is a park.

import { currentBranch, git, head, isClean, resolveCommit, tagMessage, tagObjectType } from "./git.mjs";

export async function fastForwardMain({ repo, worktree, branch, tag, runGate }) {
  if (currentBranch(repo) !== "main") {
    return { ok: false, reason: "main_dirty", detail: `the main checkout is on "${currentBranch(repo)}", not main` };
  }
  if (!isClean(repo)) {
    return { ok: false, reason: "main_dirty", detail: "the main checkout has uncommitted changes; the fast-forward will not touch them" };
  }
  const first = git(["merge", "--ff-only", branch], repo);
  if (first.code === 0) return { ok: true, head: head(repo), remerged: false };

  const merge = git(["merge", "--no-edit", "main"], worktree);
  if (merge.code !== 0) {
    git(["merge", "--abort"], worktree);
    return { ok: false, reason: "merge_conflict", detail: `main moved and does not merge into ${branch}: ${merge.stdout.split("\n").slice(-3).join(" ")}` };
  }
  const gate = await runGate("remerge");
  if (!gate.ok) {
    return { ok: false, reason: "gate_red", detail: `gate red after re-merging main: ${gate.failed.name}`, gate };
  }
  if (tag) {
    if (tagObjectType(tag, worktree) !== "tag") {
      return { ok: false, reason: "disagreement", detail: `tag ${tag} is not annotated` };
    }
    const message = tagMessage(tag, worktree);
    const moved = git(["tag", "-a", "-f", tag, "-m", message], worktree);
    if (moved.code !== 0 || resolveCommit(tag, worktree) !== head(worktree)) {
      return { ok: false, reason: "merge_failed", detail: `could not move ${tag} onto the re-merged tip: ${moved.stderr}` };
    }
  }
  const second = git(["merge", "--ff-only", branch], repo);
  if (second.code !== 0) {
    return { ok: false, reason: "merge_failed", detail: `fast-forward refused twice: ${second.stderr}` };
  }
  return { ok: true, head: head(repo), remerged: true };
}
