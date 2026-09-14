// Thin synchronous git helpers. Every call names its cwd; nothing here reads process.cwd(), because
// the conductor runs in the main checkout and acts on worktrees beside it.

import { spawnSync } from "node:child_process";

export function git(args, cwd) {
  const r = spawnSync("git", args, { cwd, encoding: "utf8", maxBuffer: 64 * 1024 * 1024 });
  return {
    code: r.status ?? 1,
    stdout: (r.stdout ?? "").trim(),
    stderr: (r.stderr ?? "").trim() || (r.error ? r.error.message : ""),
  };
}

export function gitOk(args, cwd) {
  const r = git(args, cwd);
  if (r.code !== 0) throw new Error(`git ${args.join(" ")} failed in ${cwd}: ${r.stderr}`);
  return r.stdout;
}

/** The full SHA a commit-ish resolves to, or null. */
export function resolveCommit(rev, cwd) {
  const r = git(["rev-parse", "--verify", "--quiet", `${rev}^{commit}`], cwd);
  return r.code === 0 ? r.stdout : null;
}

export function isAncestor(ancestor, descendant, cwd) {
  return git(["merge-base", "--is-ancestor", ancestor, descendant], cwd).code === 0;
}

/** True when `git status --porcelain` prints nothing — no staged, unstaged or untracked change. */
export function isClean(cwd) {
  const r = git(["status", "--porcelain"], cwd);
  return r.code === 0 && r.stdout === "";
}

export function currentBranch(cwd) {
  return git(["branch", "--show-current"], cwd).stdout;
}

export function head(cwd) {
  return resolveCommit("HEAD", cwd);
}

/** `tag` for an annotated tag, `commit` for a lightweight one, null when absent. */
export function tagObjectType(tag, cwd) {
  const r = git(["cat-file", "-t", `refs/tags/${tag}`], cwd);
  return r.code === 0 ? r.stdout : null;
}

export function tagMessage(tag, cwd) {
  return git(["tag", "-l", "--format=%(contents)", tag], cwd).stdout;
}

/** Commits in `base..tip`, oldest first, as full SHAs. */
export function commitsBetween(base, tip, cwd) {
  const r = git(["rev-list", "--reverse", `${base}..${tip}`], cwd);
  return r.code === 0 && r.stdout ? r.stdout.split("\n") : [];
}

export function branchExists(branch, cwd) {
  return git(["show-ref", "--verify", "--quiet", `refs/heads/${branch}`], cwd).code === 0;
}
