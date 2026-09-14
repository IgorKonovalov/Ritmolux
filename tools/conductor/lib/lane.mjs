// The lane state machine (ADR-0205). Per plan:
//
//   open the lane -> for each same-owner run not done: one implement session, then verify its claim
//   -> a `human` phase parks -> conductor gate -> take the close lock -> review session
//   -> blockers/majors: release the lock, fix session, verify, gate, re-review (two fix rounds max)
//   -> closed: verify the close -> fast-forward main (one automatic re-merge) -> release the lock
//   -> remove the lane.
//
// Every judgement the loop cannot make parks the plan: the plan keeps its worktree and branch, the
// inbox gains an entry, and the lane moves to the next queued plan whose `after` list has merged.
// The repository, not the session, is the evidence at every step (close.mjs).

import { existsSync } from "node:fs";
import { join, relative } from "node:path";

import { verifyClose, verifyFix, verifyImplement } from "./close.mjs";
import { removeLane, laneNames, openLane } from "./cleanup.mjs";
import { runGate } from "./gate.mjs";
import { head, resolveCommit } from "./git.mjs";
import { appendCleanupFailure, appendPark } from "./inbox.mjs";
import { CLOSE, take } from "./locks.mjs";
import { fastForwardMain } from "./merge.mjs";
import { findPlan, nextStep, rangeLabel, readPlanFile } from "./plan.mjs";
import { endStep, planRecord, saveState, startStep, statePaths } from "./state.mjs";
import { renderPromptFile, runStep } from "./step.mjs";

export const MAX_FIX_ROUNDS = 2;

const now = () => new Date().toISOString();
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

/*
 * ctx: { repo, worktreeRoot, stateDir, promptsDir, settingsFile, withLockPath, claude, local, queue,
 *        state, gate?, lockDir?, lockPollMs?, pollMs?, once?, lanes?, events?(name, data),
 *        onChange?(), beforeMerge?(plan) }
 */

function save(ctx) {
  saveState(ctx.stateDir, ctx.state);
  ctx.onChange?.();
}

function event(ctx, name, data = {}) {
  ctx.events?.(name, data);
}

function merged(ctx, plan) {
  return ctx.state.plans[plan]?.status === "merged" || findPlan(ctx.repo, plan)?.done === true;
}

/** True when `plan` cannot merge in this run: it, or something it waits on, is parked or cyclic. */
function blocked(ctx, plan, seen = new Set()) {
  if (merged(ctx, plan)) return false;
  if (seen.has(plan)) return true;
  seen.add(plan);
  const status = ctx.state.plans[plan]?.status ?? "queued";
  if (status === "parked") return true;
  if (!ctx.queue.plans[plan]) return true;
  return (ctx.queue.plans[plan].after ?? []).some((d) => blocked(ctx, d, seen));
}

export function openWorktreeCount(state) {
  return Object.values(state.plans).filter((r) => r.worktree && !r.laneRemoved).length;
}

/** The next plan a lane can run: { plan } | { wait: true } | {}. */
export function pickNext(ctx, lane) {
  let wait = false;
  for (const plan of ctx.queue.lanes[lane] ?? []) {
    const status = ctx.state.plans[plan]?.status ?? "queued";
    if (status === "running" && ctx.state.plans[plan].lane === lane) return { plan };
    if (status !== "queued") continue;
    const unmet = (ctx.queue.plans[plan]?.after ?? []).filter((d) => !merged(ctx, d));
    if (unmet.length === 0) return { plan };
    if (!unmet.some((d) => blocked(ctx, d))) wait = true;
  }
  return wait ? { wait: true } : {};
}

export async function runLanes(ctx) {
  const lanes = ctx.lanes ?? Object.keys(ctx.queue.lanes);
  ctx.held ??= new Map();
  const run = { started: now(), ended: null, lanes };
  ctx.state.runs.push(run);
  ctx.run = run;
  save(ctx);
  try {
    await Promise.all(lanes.map((lane) => laneLoop(ctx, lane)));
  } finally {
    run.ended = now();
    for (const lane of lanes) if (ctx.state.lanes[lane]) ctx.state.lanes[lane] = { plan: null, step: null };
    save(ctx);
  }
}

async function laneLoop(ctx, lane) {
  for (;;) {
    if (ctx.stopRequested?.()) return;
    const pick = pickNext(ctx, lane);
    if (pick.plan) {
      const rec = ctx.state.plans[pick.plan];
      const hasLane = rec?.worktree && !rec.laneRemoved;
      if (!hasLane && openWorktreeCount(ctx.state) >= ctx.local.max_open_worktrees) {
        event(ctx, "worktree-cap", { lane, plan: pick.plan });
        return;
      }
      await runPlan(ctx, lane, pick.plan);
      if (ctx.once) return;
      continue;
    }
    if (pick.wait) {
      await sleep(ctx.pollMs ?? 5000);
      continue;
    }
    return;
  }
}

function park(ctx, rec, { reason, detail, phase = null, read = null }) {
  rec.status = "parked";
  rec.park = { reason, detail, phase, read, worktree: rec.worktree, at: now() };
  rec.parks.push(rec.park);
  rec.ended = now();
  if (ctx.state.lanes[rec.lane]) ctx.state.lanes[rec.lane] = { plan: null, step: null };
  appendPark(statePaths(ctx.stateDir).inbox, { plan: rec.plan, reason, detail, read, worktree: rec.worktree });
  event(ctx, "park", { plan: rec.plan, reason });
  save(ctx);
  return rec;
}

function planFileIn(cwd, plan) {
  const found = findPlan(cwd, plan);
  return found ? { ...found, rel: relative(cwd, found.path).replace(/\\/g, "/") } : null;
}

async function session(ctx, rec, kind, { owner, prompt, vars, budget, addDirs = [], info = {} }) {
  const paths = statePaths(ctx.stateDir);
  const label = `${rec.plan}-${String(rec.steps.length + 1).padStart(2, "0")}-${kind}`;
  const appendPromptFile = renderPromptFile(join(ctx.promptsDir, `${kind}.md`), vars, join(paths.prompts, `${label}.md`));
  const entry = startStep(ctx.stateDir, ctx.state, rec.plan, { kind, owner, label, ...info });
  ctx.state.lanes[rec.lane] = { plan: rec.plan, step: label, stepStarted: entry.started };
  save(ctx);
  event(ctx, `${kind}-step`, { plan: rec.plan, label });
  const result = await runStep({
    claude: ctx.claude,
    cwd: rec.worktree,
    prompt,
    settingsFile: ctx.settingsFile,
    appendPromptFile,
    budgetUsd: budget,
    model: ctx.local.model?.[kind],
    addDirs: [...addDirs, ...(ctx.queue.plans[rec.plan]?.add_dirs ?? []).map((d) => join(ctx.repo, d))],
    transcriptPath: join(paths.transcripts, `${label}.jsonl`),
    env: { RLX_LOCK_LOG: paths.lockLog, ...(ctx.lockDir ? { RLX_LOCK_DIR: ctx.lockDir } : {}) },
    expectPlan: rec.plan,
  });
  endStep(ctx.stateDir, ctx.state, entry, result);
  ctx.state.lanes[rec.lane] = { plan: rec.plan, step: null };
  save(ctx);
  return result;
}

async function gate(ctx, rec, label) {
  const g = await runGate({
    cwd: rec.worktree,
    commands: ctx.gate,
    logDir: join(ctx.stateDir, "gates"),
    label: `${rec.plan}-${label}`,
    lockDir: ctx.lockDir,
    lockPollMs: ctx.lockPollMs,
    onLockWait: (name, ms) => {
      rec.lockWaits[`${name}_ms`] = (rec.lockWaits[`${name}_ms`] ?? 0) + ms;
    },
  });
  rec.gates ??= [];
  rec.gates.push({ label, ok: g.ok, ran: g.ran, failed: g.failed ?? null, at: now() });
  save(ctx);
  return g;
}

function gateDetail(g) {
  const tests = g.failed.tests.length ? ` - failing: ${g.failed.tests.join(", ")}` : "";
  return `${g.failed.name} exited ${g.failed.code}${tests}`;
}

function priorRounds(rec) {
  if (rec.verdicts.length === 0) return "- none: this is the first round.";
  return rec.verdicts
    .map((v) => {
      const fix = rec.fixes.find((f) => f.round === v.round);
      const fixText = fix
        ? ` Fix round ${fix.round}: commits ${fix.commits.map((c) => c.slice(0, 7)).join(", ")}; ` +
          (fix.resolved.map((r) => `finding ${r.finding} resolved in ${r.commit.slice(0, 7)}`).join(", ") || "no resolutions claimed")
        : "";
      return `- Round ${v.round}: review at ${v.review_path} - ${v.blockers} blockers, ${v.majors} majors, ${v.minors} minors.${fixText}`;
    })
    .join("\n");
}

function findingsText(findings) {
  return findings.map((f, i) => `${i}. [${f.severity}] ${f.file}${f.line ? `:${f.line}` : ""} - ${f.what}`).join("\n");
}

export async function runPlan(ctx, lane, plan) {
  const rec = planRecord(ctx.state, plan);
  rec.lane = lane;
  rec.status = "running";
  rec.park = null;
  rec.started ??= now();
  rec.ended = null;
  save(ctx);
  const budgets = ctx.local.budget_usd;
  const paths = statePaths(ctx.stateDir);

  if (!rec.worktree || !existsSync(rec.worktree)) {
    const found = findPlan(ctx.repo, plan);
    if (!found) return park(ctx, rec, { reason: "plan_wrong", detail: `plan ${plan} is not in the main checkout` });
    const names = laneNames(ctx.worktreeRoot, found.file, plan);
    const open = openLane({ repo: ctx.repo, ...names });
    rec.worktree = names.worktree;
    rec.branch = names.branch;
    if (!open.ok) return park(ctx, rec, { reason: "lane_open", detail: open.detail });
    rec.base ??= open.base;
    rec.laneRemoved = false;
    event(ctx, "lane-open", { plan, worktree: rec.worktree });
    save(ctx);
  }
  const wt = rec.worktree;
  const common = { plan, lane: wt, branch: rec.branch, with_lock: ctx.withLockPath };

  // Implementer runs, until the plan needs a human or a review.
  while (!rec.closed) {
    const file = planFileIn(wt, plan);
    if (!file) return park(ctx, rec, { reason: "disagreement", detail: `plan ${plan} vanished from the worktree` });
    const next = nextStep(readPlanFile(file.path));
    if (next.kind === "human") {
      return park(ctx, rec, {
        reason: "human_phase",
        phase: next.phases[0],
        detail: `Phase ${next.phases[0]} is owned by human`,
        read: `${file.rel} Phase ${next.phases[0]}`,
      });
    }
    if (next.kind === "review") break;

    const range = rangeLabel(next.phases);
    const before = head(wt);
    const r = await session(ctx, rec, "implement", {
      owner: next.owner,
      prompt: `/${next.owner} conductor implement plan ${plan} phases ${range}`,
      vars: { ...common, plan_file: file.rel, phases: range, last_run: next.lastRun ? "yes" : "no" },
      budget: budgets.implement,
      info: { phases: next.phases },
    });
    if (r.status === "parked") {
      return park(ctx, rec, { reason: r.reason, detail: r.detail, phase: r.outcome?.phase ?? null, read: r.transcript });
    }
    const problems = verifyImplement({ cwd: wt, plan, phases: next.phases, before, outcome: r.outcome });
    if (problems.length) {
      return park(ctx, rec, { reason: "disagreement", detail: `implement ${range}: ${problems.join("; ")}`, read: r.transcript });
    }
  }

  if (!rec.closed) {
    const g = await gate(ctx, rec, "pre-review");
    if (!g.ok) return park(ctx, rec, { reason: "gate_red", detail: gateDetail(g), read: g.failed.log });

    for (;;) {
      const lock = await take(CLOSE, {
        dir: ctx.lockDir,
        pollMs: ctx.lockPollMs,
        what: `review ${plan}`,
        onWaited: (ms) => (rec.lockWaits.close_ms += ms),
      });
      const round = rec.verdicts.length + 1;
      const file = planFileIn(wt, plan);
      const reviewPath = join(paths.reviews, `${plan}-round-${round}.md`);
      event(ctx, "review-start", { plan, round });
      const r = await session(ctx, rec, "review", {
        owner: "architect",
        prompt: `/architect conductor review plan ${plan} round ${round}`,
        vars: { ...common, plan_file: file.rel, round, review_path: reviewPath, prior_rounds: priorRounds(rec) },
        budget: budgets.review,
        addDirs: [paths.reviews],
        info: { round },
      });
      if (r.status === "parked") {
        lock.release();
        return park(ctx, rec, { reason: r.reason, detail: r.detail, read: r.transcript });
      }
      const o = r.outcome;
      if (o.kind === "verdict") {
        rec.verdicts.push({ round, blockers: o.blockers, majors: o.majors, minors: o.minors, review_path: o.review_path, findings: o.findings });
        lock.release();
        save(ctx);
        if (o.blockers === 0 && o.majors === 0) {
          return park(ctx, rec, { reason: "disagreement", detail: `round ${round} verdict has no blockers or majors but the plan did not close`, read: o.review_path });
        }
        if (rec.fixRounds >= MAX_FIX_ROUNDS) {
          return park(ctx, rec, {
            reason: "review_failed",
            detail: `round ${round} still carries ${o.blockers} blockers and ${o.majors} majors after ${MAX_FIX_ROUNDS} fix rounds`,
            read: o.review_path,
          });
        }
        const serious = o.findings.filter((f) => f.severity === "blocker" || f.severity === "major");
        const owner = serious.every((f) => f.file.replace(/\\/g, "/").startsWith("studio/")) ? "studio-builder" : "dev";
        const before = head(wt);
        const f = await session(ctx, rec, "fix", {
          owner,
          prompt: `/${owner} conductor fix plan ${plan} round ${round}`,
          vars: { ...common, plan_file: file.rel, round, review_path: o.review_path, findings: findingsText(o.findings) },
          budget: budgets.fix,
          addDirs: [paths.reviews],
          info: { round },
        });
        if (f.status === "parked") return park(ctx, rec, { reason: f.reason, detail: f.detail, read: f.transcript });
        const problems = verifyFix({ cwd: wt, before, outcome: f.outcome, findingCount: o.findings.length });
        if (problems.length) {
          return park(ctx, rec, { reason: "disagreement", detail: `fix round ${round}: ${problems.join("; ")}`, read: f.transcript });
        }
        rec.fixRounds += 1;
        rec.fixes.push({
          round,
          commits: f.outcome.commits.map((c) => resolveCommit(c, wt)),
          resolved: f.outcome.resolved.map((x) => ({ finding: x.finding, commit: resolveCommit(x.commit, wt) })),
        });
        save(ctx);
        const g2 = await gate(ctx, rec, `fix-${round}`);
        if (!g2.ok) return park(ctx, rec, { reason: "gate_red", detail: gateDetail(g2), read: g2.failed.log });
        continue;
      }
      if (o.kind === "closed") {
        const problems = verifyClose({ cwd: wt, plan, outcome: o });
        if (problems.length) {
          lock.release();
          return park(ctx, rec, { reason: "disagreement", detail: `close: ${problems.join("; ")}`, read: r.transcript });
        }
        rec.verdicts.push({ ...o.verdict, round });
        rec.closed = { version: o.version, tag: o.tag, head: head(wt), at: now() };
        ctx.held.set(plan, lock);
        event(ctx, "closed", { plan, tag: o.tag });
        save(ctx);
        break;
      }
      lock.release();
      return park(ctx, rec, { reason: "disagreement", detail: `review returned a ${o.kind} outcome`, read: r.transcript });
    }
  }

  const lock =
    ctx.held.get(plan) ??
    (await take(CLOSE, { dir: ctx.lockDir, pollMs: ctx.lockPollMs, what: `merge ${plan}`, onWaited: (ms) => (rec.lockWaits.close_ms += ms) }));
  try {
    await ctx.beforeMerge?.(plan);
    const m = await fastForwardMain({
      repo: ctx.repo,
      worktree: wt,
      branch: rec.branch,
      tag: rec.closed.tag,
      runGate: (label) => gate(ctx, rec, label),
    });
    if (!m.ok) return park(ctx, rec, { reason: m.reason, detail: m.detail, read: m.gate?.failed?.log ?? null });
    rec.merge = { head: m.head, remerged: m.remerged, at: now() };
    event(ctx, "ff", { plan, head: m.head });
  } finally {
    lock.release();
    ctx.held.delete(plan);
  }
  rec.status = "merged";
  rec.ended = now();
  if (ctx.state.lanes[lane]) ctx.state.lanes[lane] = { plan: null, step: null };
  save(ctx);

  const c = removeLane({ repo: ctx.repo, worktree: wt, branch: rec.branch });
  rec.cleanup = c;
  if (c.ok) rec.laneRemoved = true;
  else appendCleanupFailure(paths.inbox, { plan, worktree: wt, branch: rec.branch, detail: c.detail });
  save(ctx);
  return rec;
}
