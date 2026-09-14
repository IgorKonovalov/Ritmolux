// state/inbox.md: one entry per park or per cleanup failure, appended, never rewritten. Each entry
// names the plan, the reason, the file to read, the worktree it holds and the command that resumes
// it — the whole of what an owner needs to act without opening state/conductor.json.

import { appendFileSync, existsSync, mkdirSync, writeFileSync } from "node:fs";
import { dirname } from "node:path";

export function resumeCommand(plan) {
  return `node tools/conductor/conductor.mjs resume ${plan}`;
}

function header(path) {
  if (existsSync(path)) return;
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, "# Conductor inbox\n\nOne entry per park or cleanup failure, newest last.\n");
}

export function appendPark(path, { plan, reason, detail, read, worktree, at = new Date() }) {
  header(path);
  const lines = [
    "",
    `## ${at.toISOString().slice(0, 16).replace("T", " ")} — plan ${plan} parked: ${reason}`,
    "",
    `- **Why:** ${detail}`,
    `- **Read:** ${read ?? "the plan"}`,
    `- **Holds:** ${worktree ?? "no worktree"}`,
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
