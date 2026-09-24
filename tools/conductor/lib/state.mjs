// The conductor's runtime record, tools/conductor/state/conductor.json (gitignored).
//
// Every write goes to a temp file beside the target and is renamed over it, so a conductor killed
// mid-write leaves the previous complete file, never a truncated one. A step is recorded twice —
// when it starts and when it ends — so a restart can tell a completed step (kept) from one that
// was in flight when the process died (re-run: its session's commits, if any, are re-derived from
// the plan log and git, not from this file).

import { appendFileSync, existsSync, mkdirSync, readFileSync, renameSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";

import { usageReading } from "./live.mjs";

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
    pause: join(stateDir, "pause.json"),
    resumeAsks: join(stateDir, "resume-asks.jsonl"),
  };
}

/**
 * The pause ask (ADR-0219): the file `pause` writes and the lane loop reads between plans. It is its
 * own file rather than a field of conductor.json because the two are written by different processes —
 * the running conductor rewrites that record whole, and would overwrite an ask written under it.
 *
 * Returns what the ask says, or null when there is none. An unreadable ask still counts as one: the
 * operator asked, and refusing to read their own file is not a reason to keep starting plans.
 */
export function pauseAsk(stateDir) {
  const { pause } = statePaths(stateDir);
  if (!existsSync(pause)) return null;
  try {
    return JSON.parse(readFileSync(pause, "utf8"));
  } catch {
    return { at: null };
  }
}

export function askPause(stateDir, ask) {
  writeAtomic(statePaths(stateDir).pause, JSON.stringify(ask, null, 2) + "\n");
  return ask;
}

/** Removes the ask, returning what it said, or null when there was none. */
export function clearPause(stateDir) {
  const ask = pauseAsk(stateDir);
  if (ask) rmSync(statePaths(stateDir).pause, { force: true });
  return ask;
}

/**
 * The resume asks (ADR-0250): what `resume NNNN` leaves for a run that is live. The running conductor
 * owns conductor.json and rewrites it whole after every step, so a second process writing a cleared
 * park into it would be overwritten; the ask is appended here instead, and the lane loop takes every
 * ask on its next look and clears the park itself, after checking the condition again.
 */
export function askResume(stateDir, plan, at = new Date().toISOString()) {
  const { resumeAsks } = statePaths(stateDir);
  mkdirSync(dirname(resumeAsks), { recursive: true });
  appendFileSync(resumeAsks, JSON.stringify({ plan, at }) + "\n");
}

/**
 * Every pending resume ask, oldest first, and the file removed. The rename first makes an ask
 * appended while this reads land in a fresh file for the next look rather than be deleted unread.
 */
export function takeResumeAsks(stateDir) {
  const { resumeAsks } = statePaths(stateDir);
  if (!existsSync(resumeAsks)) return [];
  const taken = `${resumeAsks}.${process.pid}.taking`;
  try {
    renameSync(resumeAsks, taken);
  } catch {
    return [];
  }
  const text = readFileSync(taken, "utf8");
  rmSync(taken, { force: true });
  return text
    .split("\n")
    .filter(Boolean)
    .map((l) => {
      try {
        return JSON.parse(l);
      } catch {
        return null;
      }
    })
    .filter((a) => a && /^\d{4}$/.test(String(a.plan)));
}

/**
 * Clears a park, as `resume` and a self-resume both do: the plan is queued again, and a
 * `review_failed` park gets its fix rounds back. The park stays in `parks` as history.
 */
export function clearPark(rec) {
  const reason = rec.park?.reason ?? null;
  rec.status = "queued";
  rec.park = null;
  if (reason === "review_failed") rec.fixRounds = 0;
  return reason;
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

/**
 * Records a step's end. `entry.usage` keeps the session's first and last usage reading, each
 * { status, five, seven } in the one shape usageReading reads out of either CLI shape, or null.
 */
export function endStep(stateDir, state, entry, result) {
  entry.ended = new Date().toISOString();
  entry.result = result;
  entry.usage = { first: usageReading(result?.rateLimitFirst ?? null), last: usageReading(result?.rateLimit ?? null) };
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

/** The three futures an open finding has: repaired, judged not worth repairing, filed (ADR-0216). */
export const FINDING_VERBS = ["done", "wontfix", "filed"];

/** How a finding is named on both digest pages and on the `finding` command line. */
export function findingWhere(f) {
  return f.line ? `${f.file}:${f.line}` : f.file;
}

/**
 * The finding a `<ref>` names within `findings`: `{ index }`, or `{ error }` naming what it saw.
 * A ref is either the index or the `file:line` exactly one finding carries — never a prefix and
 * never a nearest match, because closing the wrong finding leaves no trace that it was wrong.
 */
export function findingRef(findings, ref) {
  const roster = findings.map((f, i) => `${i} ${findingWhere(f)}`).join(", ");
  if (/^\d+$/.test(ref)) {
    const index = Number(ref);
    if (index >= findings.length) return { error: `there is no finding ${index}; the verdict carries ${roster}` };
    return { index };
  }
  const hits = findings.map((f, i) => ({ f, i })).filter(({ f }) => findingWhere(f) === ref);
  if (hits.length === 0) return { error: `no finding is at ${ref}; the verdict carries ${roster}` };
  if (hits.length > 1) {
    return { error: `${ref} names ${hits.length} findings, so it says nothing: ${hits.map(({ f, i }) => `${i} ${f.severity} ${f.what}`).join("; ")}` };
  }
  return { index: hits[0].i };
}

/**
 * Records the owner's disposition on a finding, keeping any previous one in `dispositionHistory`:
 * a `wontfix` someone later repairs should read as repaired, and that it was first declined is
 * worth more than a tidy record. Returns the disposition it replaced, or null.
 */
export function disposeFinding(finding, verb, reason, at = new Date().toISOString()) {
  const previous = finding.disposition ?? null;
  if (previous) (finding.dispositionHistory ??= []).push(previous);
  finding.disposition = { verb, reason, at };
  return previous;
}

/**
 * Records a close found on the branch (`adoptedClose`) as the plan's close. A clean verdict the
 * review already recorded stays the plan's verdict, since it carries the findings the adopted one,
 * read from prose, cannot; with none, the adopted verdict is recorded.
 */
export function adoptClose(rec, adopted, headSha, at = new Date().toISOString()) {
  const last = rec.verdicts.at(-1);
  if (!(last && last.blockers === 0 && last.majors === 0)) rec.verdicts.push({ ...adopted.verdict });
  rec.closed = { version: adopted.version, tag: adopted.tag, head: headSha, at, adopted: true };
}

export function completedSteps(rec) {
  return rec.steps.filter((s) => s.ended && s.result?.status !== "interrupted");
}

/** What every plan's steps started at or after `since` spent: one run's spend, for `run_budget_usd`. */
export function spendSince(state, since) {
  let sum = 0;
  for (const rec of Object.values(state.plans)) {
    for (const s of rec.steps) if (s.started >= since) sum += s.result?.spendUsd ?? 0;
  }
  return sum;
}

export function totalSpend(rec) {
  return rec.steps.reduce((sum, s) => sum + (s.result?.spendUsd ?? 0), 0);
}
