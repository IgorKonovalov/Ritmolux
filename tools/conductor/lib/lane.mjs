// The lane state machine (ADR-0205). Per plan:
//
//   open the lane (installing the studio's dependencies when the plan declares files under studio/,
//   since the gate's three studio checks are guarded on a directory no worktree is born with)
//   -> for each same-owner run not done: one implement session, then verify its claim
//   -> a `human` phase parks -> conductor gate -> take the close lock -> review session
//   -> blockers/majors: release the lock, fix session, verify, gate, re-review (two fix rounds max)
//   -> closed: verify the close -> gate the close tip -> fast-forward main (one automatic re-merge)
//   -> release the lock
//   -> remove the lane.
//
// Every judgement the loop cannot make parks the plan: the plan keeps its worktree and branch, the
// inbox gains an entry, and the lane moves to the next queued plan whose `after` list has merged.
// The repository, not the session, is the evidence at every step (close.mjs).
//
// A resident run (ADR-0250) never ends on an empty lane: the lane looks again every IDLE_POLL_MS,
// re-reading the queue, and on every look clears the parks of a closed list whose condition the tree
// now shows settled. The worktree cap is a wait. `pause`, a spent `run_budget_usd` or a refused CLI
// version ends it the way ADR-0219's pause does: the plan in flight finishes and no other starts.

import { spawnSync } from "node:child_process";
import { closeSync, existsSync, fstatSync, mkdirSync, openSync, readSync } from "node:fs";
import { join, relative } from "node:path";

import { adoptedClose, verifyClose, verifyFix, verifyImplement } from "./close.mjs";
import { removeLane, laneNames, openLane } from "./cleanup.mjs";
import { defaultGate, gateForStage, runGate } from "./gate.mjs";
import { currentBranch, git, head, isClean, resolveCommit } from "./git.mjs";
import { appendCleanupFailure, appendPark, appendSelfResume, dirtyText, dirtyWorktree } from "./inbox.mjs";
import { servedNotice } from "./ledger.mjs";
import {
  gateReader,
  liveLine,
  phaseClock,
  standingParkBody,
  stepEndBody,
  stepStartBody,
  streamReader,
} from "./live.mjs";
import { CLOSE, take } from "./locks.mjs";
import { fastForwardMain } from "./merge.mjs";
import { CLAUDE_DIR, CLI_CONTRACT, STUDIO_INSTALL } from "./outcome.mjs";
import { donePhases, findPlan, nextStep, rangeLabel, readPlanFile } from "./plan.mjs";
import { clearPark, endStep, planRecord, saveState, spendSince, startStep, statePaths, takeResumeAsks } from "./state.mjs";
import { USAGE_LIMIT, renderPromptFile, runStep } from "./step.mjs";

export const MAX_FIX_ROUNDS = 2;

// A session the usage limit ends is continued once the window reopens, rather than parked. A reset
// further off than MAX_USAGE_WAIT_MS (the seven-day window's) parks `usage_limit` instead, as does a
// reset the CLI did not report or a step that has already been continued MAX_USAGE_RESUMES times.
// The margin is there because the window reopens on the server's clock, not this machine's.
export const MAX_USAGE_WAIT_MS = 6 * 60 * 60 * 1000;
export const MAX_USAGE_RESUMES = 3;
const USAGE_MARGIN_MS = 2 * 60 * 1000;
const RESUME_PROMPT =
  "The usage limit that ended this session has reset. Carry on exactly where you stopped, with the same " +
  "scope and the same rules, and finish by printing the rlx-outcome block.";

/** How often a resident run's idle or capped lane looks again (ADR-0250). */
export const IDLE_POLL_MS = 60 * 1000;

/**
 * The parks a run clears by itself once the tree shows them settled (ADR-0250). Every other reason
 * is the owner's: none of them can be read as settled from the tree.
 */
export const SELF_RESUME_REASONS = new Set(["human_phase", CLAUDE_DIR, USAGE_LIMIT, "main_dirty", STUDIO_INSTALL]);

/** A failed studio install is retried this long after its park, at most STUDIO_RETRIES times per plan. */
export const STUDIO_RETRY_MS = 60 * 60 * 1000;
export const STUDIO_RETRIES = 3;

const now = () => new Date().toISOString();
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

/*
 * ctx: { repo, worktreeRoot, stateDir, promptsDir, settingsFile, withLockPath, claude, local, queue,
 *        state, gate?, lockDir?, lockPollMs?, pollMs?, commitPollMs?, once?, lanes?, events?(name, data),
 *        live?(line), onChange?(), beforeMerge?(plan) }
 */

/**
 * A lane is open when its record names a worktree and that directory exists. The worktree cap, the
 * cap's list of holders, a plan's reopen test and the standing-park line all ask this, so a lane the
 * owner removed with `git worktree remove` stops counting at once. The record's removal flag is
 * history only: nothing that counts lanes reads it.
 */
export function laneOpen(rec) {
  return Boolean(rec?.worktree) && existsSync(rec.worktree);
}

/**
 * Why a park still holds, or null when the tree shows it settled. `resume` asks this before it
 * clears a park, and a self-resume asks it first, so the two never disagree on the conditions they
 * share. Only `human_phase`, `claude_dir` and `main_dirty` have a condition here; every other reason
 * is the owner's to judge, and `resume` takes their word for it.
 */
export function parkStillTrue(rec, repo) {
  const { reason, phase } = rec.park;
  // Whatever the reason, no new session starts on a tree the last one left dirty.
  const dirty = dirtyWorktree(rec.worktree);
  if (dirty) return `the worktree ${rec.worktree} has uncommitted changes: ${dirtyText(dirty)}; commit them, or \`git restore\` them there, first`;
  // Both of these park on a phase only the owner can do — one the plan tagged `human`, one whose
  // files the CLI will not let a session touch (ADR-0210). Either way the lane moves on when the
  // plan's own log says the phase is done, which is the same evidence for both.
  if (reason === "human_phase" || reason === CLAUDE_DIR) {
    const where = laneOpen(rec) ? rec.worktree : repo;
    const found = findPlan(where, rec.plan);
    if (!found) return `plan ${rec.plan} is not in ${where}`;
    if (!donePhases(readPlanFile(found.path)).has(phase)) {
      const rel = relative(where, found.path).replace(/\\/g, "/");
      return `Phase ${phase} is still not marked done in the ## Implementation log of ${rel} in ${where}; commit the row there first`;
    }
  }
  if (reason === "main_dirty" && (currentBranch(repo) !== "main" || !isClean(repo))) {
    return `the main checkout is still dirty or not on main`;
  }
  return null;
}

/**
 * The condition that has settled `rec`'s park, as a phrase, or null while it holds or is not the
 * run's to clear. Only SELF_RESUME_REASONS qualify, and never over a dirty worktree (parkStillTrue).
 * A `usage_limit` park qualifies once the reset it recorded has passed, and not at all when the CLI
 * reported none; a `studio_install` park is retried STUDIO_RETRY_MS after it parked, and only
 * STUDIO_RETRIES times, which is what keeps an install that always fails from looping.
 */
export function selfResumeWhy(rec, repo, nowMs = Date.now()) {
  const reason = rec.park?.reason;
  if (!SELF_RESUME_REASONS.has(reason)) return null;
  if (parkStillTrue(rec, repo)) return null;
  switch (reason) {
    case "human_phase":
    case CLAUDE_DIR:
      return `Phase ${rec.park.phase} reads done in the plan's ## Implementation log`;
    case "main_dirty":
      return "the main checkout is on main and clean";
    case USAGE_LIMIT: {
      if (typeof rec.park.resetsAt !== "number") return null;
      const open = rec.park.resetsAt * 1000 + USAGE_MARGIN_MS;
      return nowMs >= open ? `the usage window reopened at ${new Date(rec.park.resetsAt * 1000).toISOString().slice(0, 16).replace("T", " ")} UTC` : null;
    }
    case STUDIO_INSTALL: {
      const tries = (rec.selfResumes ?? []).filter((r) => r.reason === STUDIO_INSTALL).length;
      if (tries >= STUDIO_RETRIES) return null;
      if (nowMs - Date.parse(rec.park.at) < STUDIO_RETRY_MS) return null;
      return `the install is retried an hour after it failed (retry ${tries + 1} of ${STUDIO_RETRIES})`;
    }
    default:
      return null;
  }
}

/** The suite ledger (ADR-0207): the gate reads and writes it, and every session's wrapper is handed it. */
const suiteLedger = (ctx) => join(ctx.stateDir, "suite-ledger.jsonl");

/**
 * What a lane runs when its plan touches `studio/` and the dependencies are absent (ADR-0218).
 * `--prefix` rather than a `cd`, and relative to the worktree, which is the command's cwd. `ci`
 * rather than `install`: it installs the committed lockfile exactly and fails when it and
 * `package.json` disagree — and it DELETES an existing `node_modules` first, which is why the
 * caller's absence check is what keeps a resume from redoing a good install.
 */
const STUDIO_INSTALL_CMD = ["npm", "--prefix", "studio", "ci"];

/**
 * True when any phase's `Files touched` declares a path under `studio/`. The trigger is what the plan
 * DECLARES, so a phase that edits the studio without naming it gets a lane with no install — and now
 * an announced skip rather than a silent one.
 */
function touchesStudio(plan) {
  return plan.phases.some((p) => /(^|[^\w/-])studio\//.test(String(p.filesText ?? "")));
}

/**
 * Runs `cmd` to completion in `cwd` as one process. `npm` is a `.cmd` shim on Windows and is not
 * spawnable without a shell there, which is the same retry `gate.mjs` makes for the same reason.
 */
function runInstall(cmd, cwd) {
  const [bin, ...args] = cmd;
  let r = spawnSync(bin, args, { cwd, encoding: "utf8" });
  if (r.error?.code === "ENOENT" && process.platform === "win32") {
    const quote = (a) => (/[\s"]/.test(a) ? `"${a.replace(/"/g, '\\"')}"` : a);
    r = spawnSync(cmd.map(quote).join(" "), { cwd, encoding: "utf8", shell: true });
  }
  const output = `${r.stdout ?? ""}${r.stderr ?? ""}${r.error ? `\n${r.error.message}` : ""}`;
  return { code: r.status ?? 1, output };
}

/**
 * Makes the lane's plan's precondition true: with `studio/` in the plan's declared files, install the
 * studio's dependencies so the gate's three studio checks are real for this lane. Returns a park
 * detail when the install failed, and null when it succeeded or was not needed.
 *
 * ASKED ON EVERY RUN, NOT ONLY AT OPEN, and the absence of `studio/node_modules` is the trigger. A
 * failed install parks a lane whose worktree already exists, so an open-lane-only call would leave
 * that park unclearable: the resume would find the lane open, skip the install, and run the plan to
 * a merge with the three studio checks skipped — the state ADR-0218 refuses.
 */
function installStudioDeps(ctx, rec) {
  if (existsSync(join(rec.worktree, "studio", "node_modules"))) return null;
  const found = findPlan(rec.worktree, rec.plan);
  if (!found || !touchesStudio(readPlanFile(found.path))) return null;
  const cmd = ctx.studioInstall ?? STUDIO_INSTALL_CMD;
  const t0 = Date.now();
  const r = runInstall(cmd, rec.worktree);
  const took = `${Math.round((Date.now() - t0) / 1000)}s`;
  live(ctx, rec.plan, `  lane   ${cmd.join(" ")} exited ${r.code} after ${took}`);
  if (r.code === 0) return null;
  const tail = r.output.trim().split("\n").slice(-15).join("\n");
  return `${cmd.join(" ")} exited ${r.code} in ${rec.worktree}; the plan touches studio/ and its three gate checks cannot run without it:\n${tail}`;
}

/** Emits one run-terminal line; a display that throws never stops a lane. */
function live(ctx, plan, body) {
  if (!ctx.live) return;
  try {
    ctx.live(liveLine(plan, body));
  } catch {}
}

/** The last 64 KiB of a backgrounded command's output file, or null. */
function readTail(path) {
  const fd = openSync(path, "r");
  try {
    const size = fstatSync(fd).size;
    const len = Math.min(size, 64 * 1024);
    const buf = Buffer.alloc(len);
    readSync(fd, buf, 0, len, size - len);
    return buf.toString("utf8");
  } finally {
    closeSync(fd);
  }
}

/**
 * Polls the worktree while a session runs and emits each commit that lands, then each phase whose
 * `## Implementation log` row reads done after it. `stop()` takes one last look, so a commit made
 * just before the session ended is still printed before its end line.
 */
function watchCommits(ctx, rec, plan) {
  const wt = rec.worktree;
  const base = head(wt);
  const seen = new Set();
  const donePrinted = new Set();
  const clock = phaseClock();
  const doneNow = () => {
    const found = findPlan(wt, plan);
    return found ? donePhases(readPlanFile(found.path)) : new Set();
  };
  try {
    for (const id of doneNow()) donePrinted.add(id);
  } catch {}
  const poll = () => {
    if (!base || !existsSync(wt)) return;
    const r = git(["log", "--reverse", "--format=%H%x09%s", `${base}..HEAD`], wt);
    if (r.code !== 0 || !r.stdout) return;
    let fresh = false;
    for (const line of r.stdout.split("\n")) {
      const [sha, ...subject] = line.split("\t");
      if (!sha || seen.has(sha)) continue;
      seen.add(sha);
      fresh = true;
      live(ctx, plan, clock.commit(sha, subject.join("\t")));
    }
    if (!fresh) return;
    try {
      for (const id of doneNow()) {
        if (donePrinted.has(id)) continue;
        donePrinted.add(id);
        live(ctx, plan, clock.phase(id));
      }
    } catch {}
  };
  const timer = ctx.live ? setInterval(poll, ctx.commitPollMs ?? 3000) : null;
  return {
    stop() {
      if (timer) {
        clearInterval(timer);
        poll();
      }
    },
  };
}

function recordWait(rec, lock, ms) {
  if (!Array.isArray(rec.lockWaits)) rec.lockWaits = [];
  rec.lockWaits.push({ lock, ms, at: now() });
}

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
  return Object.values(state.plans).filter(laneOpen).length;
}

/** The next plan a lane can run: { plan } | { wait: true } | {}. */
export function pickNext(ctx, lane) {
  let wait = false;
  for (const plan of ctx.queue.lanes[lane] ?? []) {
    // A merged plan is skipped here, not only by its state record: with no record beside the queue -
    // a clone, a second machine, a wiped state/ - the status below reads `queued` while the plan is
    // already under docs/plans/done/. This is the same one condition validateQueue reports as a
    // notice rather than an error (ADR-0220), which is what makes the two agree.
    if (merged(ctx, plan)) continue;
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
  // `cli` is preflight's reading of `claude --version`, carrying its warning for an unverified patch.
  const run = { started: now(), ended: null, lanes, ...(ctx.resident && !ctx.once ? { resident: true } : {}), ...(ctx.cli ? { cli: ctx.cli } : {}) };
  ctx.state.runs.push(run);
  ctx.run = run;
  save(ctx);
  const nowMs = Date.now();
  for (const rec of Object.values(ctx.state.plans).sort((a, b) => a.plan.localeCompare(b.plan))) {
    if (rec.status === "parked") live(ctx, rec.plan, standingParkBody(rec, { open: laneOpen(rec), nowMs }));
  }
  try {
    await Promise.all(lanes.map((lane) => laneLoop(ctx, lane)));
  } finally {
    run.ended = now();
    for (const lane of lanes) if (ctx.state.lanes[lane]) ctx.state.lanes[lane] = { plan: null, step: null };
    save(ctx);
  }
}

/**
 * Records in the run every plan of `lane` still queued when the lane stops, with why it did not
 * start: the first unmerged plan it waits on and that plan's status, or else `stopped` — the lane's
 * own reason for stopping (`worktree cap`, `--once`, `paused`, `stopped`).
 */
function recordNotStarted(ctx, lane, stopped) {
  ctx.run.notStarted ??= [];
  for (const plan of ctx.queue.lanes[lane] ?? []) {
    if (merged(ctx, plan) || (ctx.state.plans[plan]?.status ?? "queued") !== "queued") continue;
    const dep = (ctx.queue.plans[plan]?.after ?? []).find((d) => !merged(ctx, d));
    const reason = dep ? `after ${dep} (${ctx.state.plans[dep]?.status ?? "queued"})` : stopped;
    ctx.run.notStarted.push({ plan, lane, reason });
  }
}

/**
 * Records that `lane` stopped because the run was paused (ADR-0219), so the run's own record tells a
 * pause apart from `--once` and from a queue that simply ran out. `reason` is what paused it: `asked`
 * for the owner's `pause`, `run_budget` for a spent `run_budget_usd`, `cli_version` for a CLI the run
 * refused between sessions (ADR-0250).
 */
function recordPaused(ctx, lane, reason = "asked") {
  ctx.run.paused ??= { at: now(), lanes: [], reason };
  ctx.run.paused.lanes.push(lane);
  recordNotStarted(ctx, lane, "paused");
  save(ctx);
}

/** What this run has spent, against `run_budget_usd`; a missing budget never pauses. */
function runBudgetSpent(ctx) {
  const cap = ctx.local.run_budget_usd;
  if (typeof cap !== "number") return false;
  return spendSince(ctx.state, ctx.run.started) >= cap;
}

/** Sets a lane's record between plans, saving only when it changed. Returns true when it did. */
function setLane(ctx, lane, extra) {
  const next = { plan: null, step: null, ...extra };
  if (JSON.stringify(ctx.state.lanes[lane] ?? null) === JSON.stringify(next)) return false;
  ctx.state.lanes[lane] = next;
  save(ctx);
  return true;
}

/**
 * Re-reads queue.json from the main checkout. A queue that no longer validates is not taken: the
 * lane keeps the one it had and says so once per distinct error, since a half-edited queue is the
 * owner mid-change rather than a new instruction.
 */
function refreshQueue(ctx) {
  if (!ctx.reloadQueue) return;
  const q = ctx.reloadQueue();
  if (q.errors?.length) {
    const text = q.errors.join("; ");
    if (ctx.queueError !== text) live(ctx, "queue", `  lane   queue.json does not validate, the run keeps the queue it had: ${text}`);
    ctx.queueError = text;
    return;
  }
  ctx.queueError = null;
  ctx.queue = q;
}

/**
 * Takes every `resume` the owner asked for while the run is live and clears the ones whose park no
 * longer holds, re-checked here since the tree may have moved since the command checked it.
 */
function takeAsks(ctx) {
  for (const ask of takeResumeAsks(ctx.stateDir)) {
    const rec = ctx.state.plans[ask.plan];
    if (rec?.status !== "parked" || !rec.park) continue;
    const still = parkStillTrue(rec, ctx.repo);
    if (still) {
      live(ctx, rec.plan, `  lane   resume asked, refused: ${still}`);
      continue;
    }
    const reason = clearPark(rec);
    live(ctx, rec.plan, `  lane   resumed by the owner from ${reason}`);
    event(ctx, "resumed", { plan: rec.plan, reason });
    save(ctx);
  }
}

/** Clears every park in `lane`'s queue whose condition the tree now shows settled (ADR-0250). */
function selfResume(ctx, lane) {
  const nowMs = Date.now();
  for (const plan of ctx.queue.lanes[lane] ?? []) {
    const rec = ctx.state.plans[plan];
    if (rec?.status !== "parked" || !rec.park) continue;
    const why = selfResumeWhy(rec, ctx.repo, nowMs);
    if (!why) continue;
    const reason = clearPark(rec);
    (rec.selfResumes ??= []).push({ reason, why, at: now() });
    appendSelfResume(statePaths(ctx.stateDir).inbox, { plan, reason, why });
    live(ctx, plan, `  lane   resumed itself from ${reason}: ${why}`);
    event(ctx, "self-resume", { plan, reason, why });
    save(ctx);
  }
}

async function laneLoop(ctx, lane) {
  // `--once` runs one plan and ends, resident or not: a lane with nothing to start ends it at once.
  const resident = Boolean(ctx.resident) && !ctx.once;
  const idlePoll = ctx.idlePollMs ?? IDLE_POLL_MS;
  for (;;) {
    if (ctx.stopRequested?.()) return recordNotStarted(ctx, lane, "stopped");
    // The pause ask is read here, beside the stop request, and nowhere else: the plan in flight has
    // already finished by the time the loop is back at the top, which is what makes the granularity
    // the plan rather than the step. A spent run budget and a refused CLI pause the same way.
    if (ctx.paused?.()) return recordPaused(ctx, lane, "asked");
    if (ctx.cliRefused) return recordPaused(ctx, lane, "cli_version");
    if (runBudgetSpent(ctx)) return recordPaused(ctx, lane, "run_budget");
    takeAsks(ctx);
    selfResume(ctx, lane);
    const pick = pickNext(ctx, lane);
    if (pick.plan) {
      const rec = ctx.state.plans[pick.plan];
      if (!laneOpen(rec) && openWorktreeCount(ctx.state) >= ctx.local.max_open_worktrees) {
        const holding = Object.values(ctx.state.plans)
          .filter(laneOpen)
          .map((r) => r.plan)
          .sort();
        // The cap is the disk bound (ADR-0205). A slot can free up while a holder is in flight in
        // this run, so the lane waits for it; a resident run waits whatever holds the slots, since a
        // parked holder may resume itself or be removed by hand. Otherwise the lane stops, and the
        // stop is a fact in the run record, for the digest and the run's output.
        const inFlight = holding.some((plan) => Object.values(ctx.state.lanes).some((l) => l?.plan === plan));
        if (resident || inFlight) {
          const cap = { plan: pick.plan, holding, max: ctx.local.max_open_worktrees };
          if (setLane(ctx, lane, { cap })) event(ctx, "worktree-wait", { lane, ...cap });
          await sleep(inFlight ? (ctx.pollMs ?? 5000) : idlePoll);
          refreshQueue(ctx);
          continue;
        }
        const stop = { lane, reason: "worktree_cap", plan: pick.plan, holding, max: ctx.local.max_open_worktrees, at: now() };
        ctx.run.stops ??= [];
        ctx.run.stops.push(stop);
        recordNotStarted(ctx, lane, "worktree cap");
        save(ctx);
        event(ctx, "worktree-cap", stop);
        return;
      }
      await runPlan(ctx, lane, pick.plan);
      if (ctx.once) return recordNotStarted(ctx, lane, "--once");
      continue;
    }
    if (pick.wait) {
      await sleep(ctx.pollMs ?? 5000);
      continue;
    }
    if (resident) {
      if (setLane(ctx, lane, { watching: true })) event(ctx, "idle", { lane });
      ctx.onIdleLook?.(lane);
      await sleep(idlePoll);
      refreshQueue(ctx);
      continue;
    }
    return recordNotStarted(ctx, lane, "stopped");
  }
}

function park(ctx, rec, { reason, detail, phase = null, read = null, resetsAt = null }) {
  rec.status = "parked";
  rec.park = { reason, detail, phase, read, worktree: rec.worktree, at: now() };
  // The reset a usage limit reported, which is what lets the park clear itself once it passes.
  if (reason === USAGE_LIMIT && typeof resetsAt === "number") rec.park.resetsAt = resetsAt;
  // No session is trusted to have left the tree clean. The paths are recorded and never reverted:
  // they may be the evidence the owner needs.
  const dirty = dirtyWorktree(rec.worktree);
  if (dirty) rec.park.dirty = dirty;
  rec.parks.push(rec.park);
  rec.ended = now();
  if (ctx.state.lanes[rec.lane]) ctx.state.lanes[rec.lane] = { plan: null, step: null };
  appendPark(statePaths(ctx.stateDir).inbox, { plan: rec.plan, reason, detail, read, worktree: rec.worktree, dirty });
  event(ctx, "park", { plan: rec.plan, reason });
  save(ctx);
  return rec;
}

function planFileIn(cwd, plan) {
  const found = findPlan(cwd, plan);
  return found ? { ...found, rel: relative(cwd, found.path).replace(/\\/g, "/") } : null;
}

/**
 * Reads `claude --version` again before a session (ADR-0250): an update installed while a resident
 * run is up would otherwise run sessions on a version the run never judged. Returns a park result for
 * a refused version, and records it so the run pauses; a patch above a verified one runs with its
 * warning, printed once per version.
 */
function cliRefusal(ctx, rec) {
  const verdict = ctx.checkCli?.();
  if (!verdict) return null;
  if (verdict.error) {
    ctx.cliRefused = verdict.error;
    return { status: "parked", reason: CLI_CONTRACT, detail: `no session started, and the run pauses: ${verdict.error}` };
  }
  if (verdict.version && ctx.run.cli?.version !== verdict.version) {
    ctx.run.cli = { version: verdict.version, warning: verdict.warning ?? null };
    if (verdict.warning) live(ctx, rec.plan, `  lane   warning: ${verdict.warning}`);
    save(ctx);
  }
  return null;
}

async function session(ctx, rec, kind, { owner, prompt, vars, budget, addDirs = [], info = {} }) {
  const refused = cliRefusal(ctx, rec);
  if (refused) return refused;
  const paths = statePaths(ctx.stateDir);
  const label = `${rec.plan}-${String(rec.steps.length + 1).padStart(2, "0")}-${kind}`;
  const appendPromptFile = renderPromptFile(join(ctx.promptsDir, `${kind}.md`), vars, join(paths.prompts, `${label}.md`));
  const entry = startStep(ctx.stateDir, ctx.state, rec.plan, { kind, owner, label, ...info });
  ctx.state.lanes[rec.lane] = { plan: rec.plan, step: label, stepStarted: entry.started };
  save(ctx);
  event(ctx, `${kind}-step`, { plan: rec.plan, label });
  live(ctx, rec.plan, stepStartBody({ label, kind, owner, phases: info.phases, round: info.round }));
  // The hook appends to a file in this directory; the file itself must not exist until a hook ran.
  mkdirSync(join(ctx.stateDir, "hooks"), { recursive: true });
  const reader = streamReader({ readOutput: ctx.readOutput ?? readTail, shared: (ctx.liveShared ??= {}) });
  const watch = watchCommits(ctx, rec, rec.plan);
  const t0 = Date.now();
  const segment = (n, resume) =>
    runStep({
      onStreamEvent: ctx.live ? (e) => reader.lines(e).forEach((body) => live(ctx, rec.plan, body)) : undefined,
      claude: ctx.claude,
      cwd: rec.worktree,
      prompt: resume ? RESUME_PROMPT : prompt,
      resume,
      settingsFile: ctx.settingsFile,
      appendPromptFile,
      budgetUsd: budget,
      model: ctx.local.model?.[kind],
      addDirs: [...addDirs, ...(ctx.queue.plans[rec.plan]?.add_dirs ?? []).map((d) => join(ctx.repo, d))],
      transcriptPath: join(paths.transcripts, n === 0 ? `${label}.jsonl` : `${label}-resume-${n}.jsonl`),
      skill: owner,
      hookLog: join(ctx.stateDir, "hooks", `${label}.log`),
      env: {
        RLX_LOCK_LOG: paths.lockLog,
        RLX_SUITE_LEDGER: suiteLedger(ctx),
        RLX_SUITE_LEDGER_BY: label,
        ...(ctx.lockDir ? { RLX_LOCK_DIR: ctx.lockDir } : {}),
      },
      expectPlan: rec.plan,
    });
  const first = await segment(0);
  let result = first;
  const waits = [];
  let turns = first.numTurns ?? 0;
  while (result.status === "parked" && result.reason === USAGE_LIMIT) {
    const why = usageWaitRefusal(result, waits.length, Date.now());
    if (why) {
      result = { ...result, detail: `${result.detail} (${why})` };
      break;
    }
    const waitMs = Math.max(0, result.resetsAt * 1000 - Date.now()) + (ctx.usageMarginMs ?? USAGE_MARGIN_MS);
    const until = new Date(Date.now() + waitMs);
    waits.push({ transcript: result.transcript, resetsAt: result.resetsAt, waitedMs: waitMs, at: now() });
    ctx.state.lanes[rec.lane] = { ...ctx.state.lanes[rec.lane], waitingUntil: until.toISOString() };
    save(ctx);
    live(ctx, rec.plan, `  usage  limit reached; waiting ${Math.round(waitMs / 60000)} min, until ${until.toISOString().slice(11, 16)} UTC, then continuing the session`);
    event(ctx, "usage-wait", { plan: rec.plan, label, until: until.toISOString() });
    await (ctx.sleep ?? sleep)(waitMs);
    ctx.state.lanes[rec.lane] = { plan: rec.plan, step: label, stepStarted: entry.started };
    save(ctx);
    result = await segment(waits.length, result.sessionId);
    turns += result.numTurns ?? 0;
  }
  if (waits.length) result = { ...result, numTurns: turns, rateLimitFirst: first.rateLimitFirst, usageWaits: waits };
  watch.stop();
  live(ctx, rec.plan, stepEndBody({ label, result, ms: Date.now() - t0 }));
  endStep(ctx.stateDir, ctx.state, entry, result);
  ctx.state.lanes[rec.lane] = { plan: rec.plan, step: null };
  save(ctx);
  return result;
}

/**
 * Why a usage-limited step is parked rather than waited out, or null when it can wait: the CLI
 * reported no reset, the reset is further off than MAX_USAGE_WAIT_MS, the session has no id to
 * continue, or the step has been continued MAX_USAGE_RESUMES times already.
 */
export function usageWaitRefusal(result, resumes, nowMs) {
  if (!result.sessionId) return "no session id to continue";
  if (typeof result.resetsAt !== "number") return "the CLI reported no reset time";
  if (resumes >= MAX_USAGE_RESUMES) return `already continued ${resumes} times`;
  if (result.resetsAt * 1000 - nowMs > MAX_USAGE_WAIT_MS) return `the reset is more than ${MAX_USAGE_WAIT_MS / 3600000} h away`;
  return null;
}

async function gate(ctx, rec, label) {
  const lines = gateReader({ stage: label });
  const show = (bodies) => bodies.forEach((b) => live(ctx, rec.plan, b));
  const t0 = Date.now();
  const g = await runGate({
    cwd: rec.worktree,
    commands: gateForStage(label, ctx.gate ?? defaultGate()),
    logDir: join(ctx.stateDir, "gates"),
    label: `${rec.plan}-${label}`,
    lockDir: ctx.lockDir,
    lockPollMs: ctx.lockPollMs,
    onLockWait: (name, ms) => recordWait(rec, name, ms),
    onCommandStart: (c) => show(lines.start(c)),
    onCommandEnd: (c, r) => show(lines.end(c, r)),
    ledger: suiteLedger(ctx),
    onCommandSkipped: (c, record) => show(lines.skipped(c, `tree ${record.tree.slice(0, 7)} green by ${record.by} at ${record.at}`)),
    onCommandServed: (c, serving) => show(lines.served(c, servedNotice(serving))),
    onCommandUnmet: (c, why) => show(lines.skipped(c, why)),
  });
  show(lines.finish(g, Date.now() - t0));
  rec.gates ??= [];
  rec.gates.push({ label, ok: g.ok, ran: g.ran, commands: g.commands, failed: g.failed ?? null, at: now() });
  // The tip the conductor itself last saw green. The fast-forward compares against this, never
  // against the close head: the close session merges main, bumps and tags, and its own gate run is
  // a claim like any other.
  if (g.ok) rec.gatedHead = head(rec.worktree);
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
  ctx.state.lanes[lane] = { plan, step: null };
  save(ctx);
  const budgets = ctx.local.budget_usd;
  const paths = statePaths(ctx.stateDir);

  if (!laneOpen(rec)) {
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
  // Before any session, open lane or not: a lane that cannot run its plan's checks parks here, where
  // the cost is one park, rather than after the phases that needed them (ADR-0218). A resume after a
  // failed install reaches this again — the worktree it left behind is exactly the case an
  // open-only call could never repair.
  const install = installStudioDeps(ctx, rec);
  if (install) return park(ctx, rec, { reason: STUDIO_INSTALL, detail: install });
  const wt = rec.worktree;
  const common = { plan, lane: wt, branch: rec.branch, with_lock: ctx.withLockPath };

  // What the plan still needs is read from the branch, not from the record alone (backlog 0229). A
  // review session that committed its close and then lost its outcome leaves `rec.closed` null over a
  // branch whose plan is already under `done/`; starting round 1 there would write a second close, a
  // second version and a second tag. That close is verified and adopted instead.
  if (!rec.closed) {
    const adopted = adoptedClose({ cwd: wt, plan, round: rec.verdicts.length + 1 });
    if (adopted) {
      const problems = verifyClose({ cwd: wt, plan, outcome: adopted });
      if (problems.length) {
        return park(ctx, rec, { reason: "disagreement", detail: `close found on the branch: ${problems.join("; ")}`, read: adopted.verdict.review_path });
      }
      rec.verdicts.push({ ...adopted.verdict });
      rec.closed = { version: adopted.version, tag: adopted.tag, head: head(wt), at: now(), adopted: true };
      event(ctx, "closed", { plan, tag: adopted.tag });
      save(ctx);
    }
  }

  if (!rec.closed) {
    // Implementer runs, until the plan needs a human or a review. Nothing in this loop closes the
    // plan: a plan that arrives closed skipped the whole block.
    for (;;) {
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
      if (next.kind === "claude_dir") {
        return park(ctx, rec, {
          reason: CLAUDE_DIR,
          phase: next.phases[0],
          detail:
            `Phase ${next.phases[0]} edits ${next.paths.join(", ")}, and the CLI refuses a headless session ` +
            `an edit under .claude/ whatever the allowlist says (ADR-0210). The phase is yours; nothing was run.`,
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
        return park(ctx, rec, { reason: r.reason, detail: r.detail, phase: r.outcome?.phase ?? null, read: r.transcript, resetsAt: r.resetsAt });
      }
      const problems = verifyImplement({ cwd: wt, plan, phases: next.phases, before, outcome: r.outcome });
      if (problems.length) {
        return park(ctx, rec, { reason: "disagreement", detail: `implement ${range}: ${problems.join("; ")}`, read: r.transcript });
      }
    }

    const g = await gate(ctx, rec, "pre-review");
    if (!g.ok) return park(ctx, rec, { reason: "gate_red", detail: gateDetail(g), read: g.failed.log });

    for (;;) {
      const lock = await take(CLOSE, {
        dir: ctx.lockDir,
        pollMs: ctx.lockPollMs,
        what: `review ${plan}`,
        onWaited: (ms) => recordWait(rec, CLOSE, ms),
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
        return park(ctx, rec, { reason: r.reason, detail: r.detail, read: r.transcript, resetsAt: r.resetsAt });
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
        if (f.status === "parked") return park(ctx, rec, { reason: f.reason, detail: f.detail, read: f.transcript, resetsAt: f.resetsAt });
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
    (await take(CLOSE, { dir: ctx.lockDir, pollMs: ctx.lockPollMs, what: `merge ${plan}`, onWaited: (ms) => recordWait(rec, CLOSE, ms) }));
  try {
    await ctx.beforeMerge?.(plan);
    const m = await fastForwardMain({
      repo: ctx.repo,
      worktree: wt,
      branch: rec.branch,
      tag: rec.closed.tag,
      gatedHead: rec.gatedHead ?? null,
      runGate: (label) => gate(ctx, rec, label),
      onGated: (sha) => {
        rec.gatedHead = sha;
        save(ctx);
      },
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
