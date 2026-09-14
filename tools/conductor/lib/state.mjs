// The conductor's runtime record, tools/conductor/state/conductor.json (gitignored).
//
// Every write goes to a temp file beside the target and is renamed over it, so a conductor killed
// mid-write leaves the previous complete file, never a truncated one. A step is recorded twice —
// when it starts and when it ends — so a restart can tell a completed step (kept) from one that
// was in flight when the process died (re-run: its session's commits, if any, are re-derived from
// the plan log and git, not from this file).

import { existsSync, mkdirSync, readFileSync, renameSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";

export function emptyState() {
  return { version: 1, runs: [], lanes: {}, plans: {} };
}

export function statePaths(stateDir) {
  return {
    dir: stateDir,
    file: join(stateDir, "conductor.json"),
    transcripts: join(stateDir, "transcripts"),
    reviews: join(stateDir, "reviews"),
    prompts: join(stateDir, "prompts"),
    inbox: join(stateDir, "inbox.md"),
    lockLog: join(stateDir, "locks.jsonl"),
  };
}

export function writeAtomic(path, text) {
  mkdirSync(dirname(path), { recursive: true });
  const tmp = `${path}.${process.pid}.tmp`;
  writeFileSync(tmp, text);
  renameSync(tmp, path);
}

export function loadState(stateDir) {
  const { file } = statePaths(stateDir);
  if (!existsSync(file)) return emptyState();
  return JSON.parse(readFileSync(file, "utf8"));
}

export function saveState(stateDir, state) {
  writeAtomic(statePaths(stateDir).file, JSON.stringify(state, null, 2) + "\n");
}

export function planRecord(state, plan) {
  state.plans[plan] ??= {
    plan,
    status: "queued",
    lane: null,
    worktree: null,
    branch: null,
    base: null,
    steps: [],
    park: null,
    parks: [],
    fixRounds: 0,
    verdicts: [],
    fixes: [],
    closed: null,
    merge: null,
    // One { lock, ms, at } per wait, so the digest can put each wait in the run it happened in.
    lockWaits: [],
    started: null,
    ended: null,
  };
  return state.plans[plan];
}

/** Records a step's start and persists it before the session is spawned. */
export function startStep(stateDir, state, plan, step) {
  const rec = planRecord(state, plan);
  const entry = { ...step, started: new Date().toISOString(), ended: null, result: null };
  rec.steps.push(entry);
  saveState(stateDir, state);
  return entry;
}

export function endStep(stateDir, state, entry, result) {
  entry.ended = new Date().toISOString();
  entry.result = result;
  saveState(stateDir, state);
}

/** Steps that started and never ended: the ones a killed conductor was running. */
export function interruptedSteps(state) {
  const out = [];
  for (const rec of Object.values(state.plans)) {
    for (const s of rec.steps) if (s.started && !s.ended) out.push({ plan: rec.plan, step: s });
  }
  return out;
}

/**
 * Marks every in-flight step interrupted, so the lane loop re-derives and re-runs it. Returns how
 * many there were. Completed steps, verdicts, fix rounds and parks are untouched.
 */
export function recoverInterrupted(stateDir, state) {
  const found = interruptedSteps(state);
  for (const { step } of found) {
    step.ended = new Date().toISOString();
    step.result = { status: "interrupted" };
  }
  for (const lane of Object.values(state.lanes)) {
    if (lane) lane.step = null;
  }
  if (found.length) saveState(stateDir, state);
  return found.length;
}

export function completedSteps(rec) {
  return rec.steps.filter((s) => s.ended && s.result?.status !== "interrupted");
}

export function totalSpend(rec) {
  return rec.steps.reduce((sum, s) => sum + (s.result?.spendUsd ?? 0), 0);
}
