// Shared scaffolding for the conductor tests: temp directories, plan documents in the shape
// the architect's template produces, and the path to the fake CLI.

import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export const TEST_DIR = dirname(fileURLToPath(import.meta.url));
export const TOOL_DIR = resolve(TEST_DIR, "..");
export const REPO = resolve(TOOL_DIR, "..", "..");
export const FAKE_CLAUDE = join(TEST_DIR, "fake-claude.mjs");
export const FAKE = [process.execPath, FAKE_CLAUDE];

export function tmp(prefix = "rlx-conductor-test-") {
  return mkdtempSync(join(tmpdir(), prefix));
}

/**
 * spec: { number, title?, status?, phases: [{ id, owner, title?, stop? }],
 *         rows?: { [id]: { state, commit? } }, closeReview?: string, lane? }
 */
export function planText(spec) {
  const title = spec.title ?? `Plan ${spec.number} fixture`;
  const owners = [...new Set(spec.phases.map((p) => p.owner))].map((o) => `\`${o}\``).join(", ");
  const lines = [
    `# ${spec.number} — ${title}`,
    "",
    `> **Status:** ${spec.status ?? "approved (2026-09-14)"}`,
    "> **Created:** 2026-09-14",
    `> **Owner skill(s):** ${owners}`,
    "> **Closes:** none",
    "",
    "## TL;DR",
    "",
    "A fixture.",
    "",
    "## Implementation phases",
    "",
  ];
  for (const p of spec.phases) {
    lines.push(`### Phase ${p.id} — ${p.title ?? `Step ${p.id}`}`);
    lines.push(`- **Owner skill:** ${p.owner}`);
    lines.push(`- **What:** phase ${p.id}.`);
    lines.push(`- **Files touched:** \`phase-${p.id}.txt\``);
    lines.push(`- **Done when:** the file exists.`);
    if (p.stop) lines.push(`- **Stop condition:** ${p.stop}`);
    lines.push("");
  }
  lines.push("## Implementation log", "", `**Lane:** ${spec.lane ?? "_(unset)_"}`, "");
  lines.push("| phase | owner | state | commit |", "|---|---|---|---|");
  for (const p of spec.phases) {
    const row = spec.rows?.[p.id] ?? { state: "not started" };
    lines.push(`| ${p.id} — ${p.title ?? `Step ${p.id}`} | ${p.owner} | ${row.state} | ${row.commit ? `\`${row.commit}\`` : ""} |`);
  }
  lines.push("", "### Notes", "", "### Close triggers", "");
  if (spec.closeReview) lines.push("## Close review", "", spec.closeReview, "");
  lines.push("## Followups (after this lands)", "");
  return lines.join("\n");
}

export function slugFor(spec) {
  return `${spec.number}-fixture`;
}

export function writePlan(repo, spec, { done = false } = {}) {
  const dir = done ? join(repo, "docs", "plans", "done") : join(repo, "docs", "plans");
  mkdirSync(dir, { recursive: true });
  const path = join(dir, `${slugFor(spec)}.md`);
  writeFileSync(path, planText(spec));
  return path;
}

export function outcomeBlock(obj) {
  return "Done.\n\n```rlx-outcome\n" + JSON.stringify(obj) + "\n```\n";
}
