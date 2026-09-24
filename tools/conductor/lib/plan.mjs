// Reading a plan document: its header, its phases with their owner tags, and what its
// `## Implementation log` table says is done. The log is the implementers' record; the conductor
// reads it to decide the next step and then checks the rows it relies on against git.

import { existsSync, readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";

export const OWNERS = new Set(["dev", "studio-builder", "human"]);
export const IMPLEMENTERS = new Set(["dev", "studio-builder"]);

/** Locates plan NNNN under docs/plans/ (active) or docs/plans/done/. */
export function findPlan(repo, number) {
  for (const [dir, done] of [
    [join(repo, "docs", "plans"), false],
    [join(repo, "docs", "plans", "done"), true],
  ]) {
    if (!existsSync(dir)) continue;
    const name = readdirSync(dir).find((f) => f.startsWith(`${number}-`) && f.endsWith(".md"));
    if (name) return { path: join(dir, name), file: name, done };
  }
  return null;
}

function section(text, heading) {
  const lines = text.split("\n");
  const start = lines.findIndex((l) => l.trim() === heading);
  if (start < 0) return null;
  let end = lines.findIndex((l, i) => i > start && /^## /.test(l));
  if (end < 0) end = lines.length;
  return { lines: lines.slice(start + 1, end), offset: start + 1 };
}

export function parsePlan(raw) {
  // A checkout with core.autocrlf=true hands back CRLF; every pattern below is written for LF.
  const text = raw.replace(/\r\n/g, "\n");
  const title = text.match(/^# (\d{4}) [—–-] (.+)$/m);
  const status = text.match(/^> \*\*Status:\*\*\s*(.+)$/m);
  const plan = {
    number: title ? title[1] : null,
    title: title ? title[2].trim() : null,
    status: status ? status[1].trim() : null,
    // The leading word only, letters and hyphens: a status is prose after its word, and the word
    // can be followed by any punctuation (`done. Phases ...`, `approved (2026-09-14)`, `done —`).
    statusWord: status ? (status[1].trim().toLowerCase().match(/^[a-z][a-z-]*/)?.[0] ?? null) : null,
    phases: [],
    log: { lane: null, rows: [] },
    hasCloseReview: /^## Close review\s*$/m.test(text),
    errors: [],
  };

  const phases = section(text, "## Implementation phases");
  if (!phases) plan.errors.push("no ## Implementation phases section");
  let current = null;
  // `Files touched` wraps across lines; every continuation until the next bullet belongs to it.
  let collecting = false;
  for (const line of phases?.lines ?? []) {
    const h = line.match(/^### Phase (\d+[a-z]?) [—–-] (.+)$/);
    if (h) {
      current = { id: h[1], title: h[2].trim(), owner: null, stopCondition: null, blocksMerge: null, filesText: "" };
      plan.phases.push(current);
      collecting = false;
      continue;
    }
    if (!current) continue;
    const files = line.match(/^- \*\*Files touched:\*\*\s*(.*)$/);
    if (files) {
      current.filesText = files[1];
      collecting = true;
      continue;
    }
    if (collecting) {
      if (/^\s*- \*\*/.test(line) || !line.trim()) collecting = false;
      else {
        current.filesText += ` ${line.trim()}`;
        continue;
      }
    }
    const owner = line.match(/^- \*\*Owner skill:\*\*\s*`?([\w-]+)`?\s*$/);
    if (owner) {
      if (current.owner) plan.errors.push(`Phase ${current.id} carries two owner tags`);
      current.owner = owner[1];
    }
    const stop = line.match(/^- \*\*Stop condition:\*\*\s*(.+)$/);
    if (stop) current.stopCondition = stop[1].trim();
    const blocks = line.match(/^- \*\*Blocks merge:\*\*\s*`?([\w-]+)`?\s*$/);
    if (blocks) current.blocksMerge = blocks[1].toLowerCase();
  }
  for (const p of plan.phases) {
    if (!OWNERS.has(p.owner)) plan.errors.push(`Phase ${p.id} has no valid owner tag (${p.owner})`);
    // ADR-0249: only a human phase can be owed after the merge; an implementer phase is the merge.
    if (p.blocksMerge !== null && p.owner !== "human") {
      plan.errors.push(`Phase ${p.id} carries Blocks merge, which only a human phase may (it is ${p.owner})`);
    } else if (p.blocksMerge !== null && p.blocksMerge !== "no" && p.blocksMerge !== "yes") {
      plan.errors.push(`Phase ${p.id} carries Blocks merge "${p.blocksMerge}"; it is no or yes`);
    }
  }

  const log = section(text, "## Implementation log");
  for (const line of log?.lines ?? []) {
    const lane = line.match(/^\*\*Lane:\*\*\s*(.+)$/);
    if (lane) plan.log.lane = lane[1].trim();
    const row = line.match(/^\|\s*(\d+[a-z]?) [—–-] ([^|]*)\|\s*([^|]*)\|\s*([^|]*)\|\s*([^|]*)\|\s*$/);
    if (row) {
      const commit = row[5].trim().replace(/`/g, "");
      plan.log.rows.push({
        id: row[1],
        title: row[2].trim(),
        owner: row[3].trim(),
        state: row[4].trim(),
        commit: /^[0-9a-f]{7,40}$/.test(commit) ? commit : null,
      });
    }
  }
  return plan;
}

export function readPlanFile(path) {
  return parsePlan(readFileSync(path, "utf8"));
}

/** A log row counts as done when it reads `done`, or is the row a phase commit carried. */
export function rowIsDone(row) {
  return /^done\b/i.test(row.state) || /^committed with this row$/i.test(row.state);
}

export function donePhases(plan) {
  return new Set(plan.log.rows.filter(rowIsDone).map((r) => r.id));
}

/**
 * A log row reads `owed` when the conductor merged its plan without that phase: a human phase marked
 * `Blocks merge: no` (ADR-0249). It stays owed until the owner marks it done on `main`.
 */
export function rowIsOwed(row) {
  return /^owed\b/i.test(row.state);
}

export function owedPhases(plan) {
  return new Set(plan.log.rows.filter(rowIsOwed).map((r) => r.id));
}

/** True for a human phase whose plan lets it be owed after the merge rather than waited for. */
export function nonBlocking(phase) {
  return phase?.owner === "human" && phase.blocksMerge === "no";
}

/**
 * How the log settles owner phase `id` of `plan`: `done`, `owed` when its row reads owed and the
 * phase is a human one marked `Blocks merge: no` (ADR-0249), or null while it is neither.
 *
 * The one answer to "has the owner's phase settled?" — a park guard, a self-resume and the digest all
 * ask it here. The marker is read from the phase itself: a bare `owed` row on a blocking phase settles
 * nothing, or one word in the log would skip a phase the plan says the merge waits for.
 */
export function settledPhase(plan, id) {
  if (donePhases(plan).has(id)) return "done";
  if (owedPhases(plan).has(id) && nonBlocking(plan.phases.find((p) => p.id === id))) return "owed";
  return null;
}

/** Contiguous runs of same-owner phases, in plan order. */
export function runs(plan) {
  const out = [];
  for (const p of plan.phases) {
    const last = out.at(-1);
    if (last && last.owner === p.owner) last.phases.push(p.id);
    else out.push({ owner: p.owner, phases: [p.id] });
  }
  return out;
}

/**
 * The `.claude/` paths a phase's `Files touched` declares, deduplicated. The CLI refuses a headless
 * session an `Edit` or `Write` under a project's `.claude/` whatever the allowlist says (ADR-0210,
 * measured in tools/conductor/spike/README.md), so a phase that names one is the owner's and the lane
 * stops in front of it.
 */
export function claudePaths(phase) {
  return [...new Set([...String(phase?.filesText ?? "").matchAll(/\.claude\/[A-Za-z0-9_.*/-]+/g)].map((m) => m[0].replace(/[.,;]$/, "")))];
}

/**
 * The next thing the plan needs, from the first run holding a phase the log marks neither done nor
 * owed: `implement` over that run's pending phases, `human` to park at, `owed` for the leading
 * pending phases of a human run marked `Blocks merge: no` (ADR-0249), `claude_dir` for a phase no
 * headless session can do, or `review` once every phase is done or owed. `lastRun` is true when no
 * implementer run follows it.
 *
 * A `.claude/` phase stops the run **in front of** itself: the pending phases before it are still a
 * step, and the phase after them is where the lane parks. Running the whole range and failing inside
 * it is what ADR-0210 replaces — the park carries the edit, not a red done-when.
 */
export function nextStep(plan) {
  const done = donePhases(plan);
  const owed = owedPhases(plan);
  const all = runs(plan);
  const byId = new Map(plan.phases.map((p) => [p.id, p]));
  for (const [i, run] of all.entries()) {
    const pending = run.phases.filter((id) => !done.has(id) && !owed.has(id));
    if (pending.length === 0) continue;
    if (run.owner === "human") {
      const blockingAt = pending.findIndex((id) => !nonBlocking(byId.get(id)));
      if (blockingAt !== 0) return { kind: "owed", owner: "human", phases: blockingAt < 0 ? pending : pending.slice(0, blockingAt) };
      return { kind: "human", owner: "human", phases: pending };
    }
    const lastRun = !all.slice(i + 1).some((r) => IMPLEMENTERS.has(r.owner));
    const blockedAt = pending.findIndex((id) => claudePaths(byId.get(id)).length > 0);
    if (blockedAt === 0) {
      return { kind: "claude_dir", owner: run.owner, phases: [pending[0]], paths: claudePaths(byId.get(pending[0])) };
    }
    // Truncated: the `.claude/` phase still follows, so this is not the plan's last implementer run
    // however the runs after it look.
    if (blockedAt > 0) return { kind: "implement", owner: run.owner, phases: pending.slice(0, blockedAt), lastRun: false };
    return { kind: "implement", owner: run.owner, phases: pending, lastRun };
  }
  return { kind: "review" };
}

/** `1-3`, `4b`, `4-4b` — the range string a prompt carries. */
export function rangeLabel(ids) {
  return ids.length === 1 ? ids[0] : `${ids[0]}-${ids.at(-1)}`;
}
