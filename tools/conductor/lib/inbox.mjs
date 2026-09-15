// state/inbox.md: one entry per park or per cleanup failure, appended, never rewritten. Each entry
// names the plan, the reason, the file to read, the worktree it holds and the command that resumes
// it — the whole of what an owner needs to act without opening state/conductor.json. A park that left
// its worktree dirty also names the dirty paths, capped at DIRTY_PATHS_SHOWN.

import { spawnSync } from "node:child_process";
import { appendFileSync, existsSync, mkdirSync, writeFileSync } from "node:fs";
import { dirname } from "node:path";

/** How many dirty paths a park record, inbox entry or digest line names before counting the rest. */
export const DIRTY_PATHS_SHOWN = 10;

/** Every path `git status --porcelain -z` reports in `cwd`, in its order: tracked changes, then untracked. */
export function dirtyPaths(cwd) {
  const r = spawnSync("git", ["status", "--porcelain", "-z"], { cwd, encoding: "utf8", maxBuffer: 64 * 1024 * 1024 });
  if (r.status !== 0) return [];
  const tokens = r.stdout.split("\0").filter(Boolean);
  const out = [];
  for (let i = 0; i < tokens.length; i++) {
    out.push(tokens[i].slice(3));
    // A rename or copy entry is followed by its source path, which is not a separate change.
    if (/[RC]/.test(tokens[i].slice(0, 2))) i++;
  }
  return out;
}

/** A park record's `dirty` field — the first DIRTY_PATHS_SHOWN paths and a count of the rest — or null. */
export function capDirty(paths) {
  if (paths.length === 0) return null;
  return { paths: paths.slice(0, DIRTY_PATHS_SHOWN), more: Math.max(0, paths.length - DIRTY_PATHS_SHOWN) };
}

/** The dirty paths of `dir`, capped, or null when it is clean or absent. */
export function dirtyWorktree(dir) {
  return dir && existsSync(dir) ? capDirty(dirtyPaths(dir)) : null;
}

export function dirtyText(dirty) {
  const names = dirty.paths.map((p) => "`" + p + "`").join(", ");
  return dirty.more ? `${names} and ${dirty.more} more` : names;
}

export function resumeCommand(plan) {
  return `node tools/conductor/conductor.mjs resume ${plan}`;
}

function header(path) {
  if (existsSync(path)) return;
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, "# Conductor inbox\n\nOne entry per park or cleanup failure, newest last.\n");
}

export function appendPark(path, { plan, reason, detail, read, worktree, dirty = null, at = new Date() }) {
  header(path);
  const lines = [
    "",
    `## ${at.toISOString().slice(0, 16).replace("T", " ")} — plan ${plan} parked: ${reason}`,
    "",
    `- **Why:** ${detail}`,
    `- **Read:** ${read ?? "the plan"}`,
    `- **Holds:** ${worktree ?? "no worktree"}`,
    ...(dirty ? [`- **Left dirty:** ${dirtyText(dirty)}. \`resume\` refuses until the worktree is clean.`] : []),
    `- **Resume:** \`${resumeCommand(plan)}\``,
    "",
  ];
  appendFileSync(path, lines.join("\n"));
}

export function appendCleanupFailure(path, { plan, worktree, branch, detail, at = new Date() }) {
  header(path);
  appendFileSync(
    path,
    [
      "",
      `## ${at.toISOString().slice(0, 16).replace("T", " ")} — plan ${plan} merged, lane not removed`,
      "",
      `- **Why:** ${detail}`,
      `- **Holds:** ${worktree} on \`${branch}\``,
      `- **Clean up:** close every shell inside it, then \`git worktree remove ${worktree}\`, \`git worktree prune\`, \`git branch -d ${branch}\``,
      "",
    ].join("\n"),
  );
}
