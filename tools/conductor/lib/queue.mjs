// The committed queue (tools/conductor/queue.json) and the machine-local limits
// (tools/conductor/local.json). Both are validated whole before anything runs, and every rejection
// names the plan or the key it is about.
//
// queue.json:
//   { "lanes": { "a": ["0175", "0185"], "b": [] },
//     "plans": { "0181": { "after": ["0185"] }, "0180": { "add_dirs": ["../milkdrop-corpus"] } } }
// local.json (never committed, no defaults):
//   { "budget_usd": { "implement": 5, "fix": 3, "review": 4 }, "max_open_worktrees": 3 }
//   optional: "model": { "implement": "opus", ... }, "claude": ["claude"]

import { existsSync, readFileSync } from "node:fs";

import { findPlan, readPlanFile } from "./plan.mjs";

const PLAN = /^[0-9]{4}$/;
const STEP_KINDS = ["implement", "fix", "review"];

function readJson(path, what) {
  if (!existsSync(path)) return { error: `${what} not found at ${path}` };
  try {
    return { value: JSON.parse(readFileSync(path, "utf8")) };
  } catch (e) {
    return { error: `${what} is not valid JSON: ${e.message}` };
  }
}

/**
 * Validates the queue against the repository. `startedPlans` are plans the conductor's state
 * already owns, which may legitimately read `in-progress` rather than `approved`. `mergedPlans`
 * are plans the conductor's state records as merged: they sit under done/ and stay listed in the
 * committed queue, so they are accepted there rather than rejected as closed — otherwise every run
 * after the first merge would refuse to start.
 * Returns { errors, lanes: {name: [plan]}, plans: {plan: {lane, after, add_dirs, path}} }.
 */
export function validateQueue(queue, repo, startedPlans = new Set(), mergedPlans = new Set()) {
  const errors = [];
  const out = { errors, lanes: {}, plans: {} };
  if (!queue || typeof queue !== "object" || !queue.lanes || typeof queue.lanes !== "object") {
    errors.push('queue: "lanes" must be an object of lane name -> ordered plan list');
    return out;
  }
  const extra = queue.plans ?? {};
  if (typeof extra !== "object") errors.push('queue: "plans" must be an object');

  for (const [lane, list] of Object.entries(queue.lanes)) {
    if (!/^[a-z]$/.test(lane)) errors.push(`queue: lane name "${lane}" must be one lowercase letter`);
    if (!Array.isArray(list)) {
      errors.push(`queue: lane ${lane} must be a list of plan numbers`);
      continue;
    }
    out.lanes[lane] = [];
    for (const plan of list) {
      if (!PLAN.test(String(plan))) {
        errors.push(`queue: lane ${lane} lists "${plan}", not a four-digit plan number`);
        continue;
      }
      if (out.plans[plan]) {
        errors.push(`plan ${plan}: listed twice (lanes ${out.plans[plan].lane} and ${lane})`);
        continue;
      }
      out.lanes[lane].push(plan);
      out.plans[plan] = { lane, after: [], add_dirs: [], path: null };
    }
  }

  for (const [plan, entry] of Object.entries(out.plans)) {
    const opts = extra[plan] ?? {};
    for (const key of Object.keys(opts)) {
      if (key !== "after" && key !== "add_dirs") errors.push(`plan ${plan}: unknown key "${key}"`);
    }
    if (opts.after !== undefined) {
      if (!Array.isArray(opts.after) || !opts.after.every((p) => PLAN.test(String(p)))) {
        errors.push(`plan ${plan}: "after" must be a list of plan numbers`);
      } else entry.after = opts.after.map(String);
    }
    if (opts.add_dirs !== undefined) {
      if (!Array.isArray(opts.add_dirs) || !opts.add_dirs.every((d) => typeof d === "string" && d)) {
        errors.push(`plan ${plan}: "add_dirs" must be a list of paths`);
      } else entry.add_dirs = opts.add_dirs;
    }

    const found = findPlan(repo, plan);
    if (!found) {
      errors.push(`plan ${plan}: no docs/plans/${plan}-*.md`);
      continue;
    }
    if (found.done && mergedPlans.has(plan)) {
      entry.path = found.path;
      continue;
    }
    if (found.done) {
      errors.push(`plan ${plan}: already closed (${found.file} is under docs/plans/done/)`);
      continue;
    }
    entry.path = found.path;
    const parsed = readPlanFile(found.path);
    const ok =
      parsed.statusWord === "approved" || (startedPlans.has(plan) && parsed.statusWord === "in-progress");
    if (!ok) errors.push(`plan ${plan}: Status is "${parsed.status}", not approved`);
    for (const e of parsed.errors) errors.push(`plan ${plan}: ${e}`);
  }

  for (const [plan, entry] of Object.entries(out.plans)) {
    for (const dep of entry.after) {
      if (dep === plan) errors.push(`plan ${plan}: depends on itself`);
      else if (!out.plans[dep] && !findPlan(repo, dep)?.done) {
        errors.push(`plan ${plan}: depends on plan ${dep}, which is neither in the queue nor in docs/plans/done/`);
      }
    }
  }
  return out;
}

export function loadQueue(path, repo, startedPlans, mergedPlans) {
  const r = readJson(path, "queue.json");
  if (r.error) return { errors: [r.error], lanes: {}, plans: {} };
  return validateQueue(r.value, repo, startedPlans, mergedPlans);
}

/** The started and merged plan sets validateQueue takes, read from the conductor's state. */
export function stateSets(state) {
  const recs = Object.values(state.plans);
  return [new Set(recs.map((r) => r.plan)), new Set(recs.filter((r) => r.status === "merged").map((r) => r.plan))];
}

/** Validates local.json. Every budget is required: the conductor carries no default spend. */
export function loadLocal(path) {
  const r = readJson(path, "local.json");
  if (r.error) {
    return { errors: [`${r.error} - copy tools/conductor/local.example.json and set your own figures`] };
  }
  const local = r.value;
  const errors = [];
  for (const kind of STEP_KINDS) {
    const v = local?.budget_usd?.[kind];
    if (typeof v !== "number" || !(v > 0)) errors.push(`local.json: budget_usd.${kind} must be a positive number`);
  }
  if (!Number.isInteger(local?.max_open_worktrees) || local.max_open_worktrees < 1) {
    errors.push("local.json: max_open_worktrees must be a positive integer");
  }
  if (local?.claude !== undefined && !(Array.isArray(local.claude) && local.claude.length > 0)) {
    errors.push('local.json: "claude" must be a non-empty command list');
  }
  return { errors, local };
}
