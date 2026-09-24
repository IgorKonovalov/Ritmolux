#!/usr/bin/env node
// The conductor (ADR-0205): takes approved plans off tools/conductor/queue.json and runs them in
// worktree lanes, one fresh headless `claude -p` session per same-owner run of phases, a conductor
// gate, a fresh review-and-close session, a fast-forward of main and the lane's removal. It never
// pushes. Every judgement it cannot make parks the plan.
//
//   node tools/conductor/conductor.mjs run [--lane a|b] [--once]
//   node tools/conductor/conductor.mjs status
//   node tools/conductor/conductor.mjs digest [--history]
//   node tools/conductor/conductor.mjs resume NNNN
//   node tools/conductor/conductor.mjs park NNNN
//   node tools/conductor/conductor.mjs finding NNNN [<ref> --done|--wontfix|--filed <reason>]
//   node tools/conductor/conductor.mjs adopt-close NNNN
//   node tools/conductor/conductor.mjs pause [--off]
//   node tools/conductor/conductor.mjs abort
//   node tools/conductor/conductor.mjs prune
//   node tools/conductor/conductor.mjs check
//
// Runs from the main checkout. Runtime output lives under tools/conductor/state/, in
// tools/conductor/digest.md and — only once `digest --history` has been asked for — in
// tools/conductor/digest-history.md, all gitignored. tools/conductor/README.md is the operator guide.

import { spawnSync } from "node:child_process";
import { appendFileSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { pidAlive } from "./with-lock.mjs";
import { adoptedClose, verifyClose } from "./lib/close.mjs";
import { settledPark, writeDigest, writeHistory } from "./lib/digest.mjs";
import { currentBranch, head, isClean } from "./lib/git.mjs";
import { appendPark, dirtyText, dirtyWorktree } from "./lib/inbox.mjs";
import { runLanes } from "./lib/lane.mjs";
import { ascii } from "./lib/live.mjs";
import { CLAUDE_DIR } from "./lib/outcome.mjs";
import { donePhases, findPlan, readPlanFile } from "./lib/plan.mjs";
import { loadLocal, loadQueue, pruneQueue, readQueue, startedPlans } from "./lib/queue.mjs";
import { FINDING_VERBS, askPause, clearPause, disposeFinding, findingRef, findingWhere, loadState, pauseAsk, planRecord, recoverInterrupted, saveState, statePaths, totalSpend } from "./lib/state.mjs";
import { activeChildren, killTree } from "./lib/step.mjs";

export const TOOL_DIR = dirname(fileURLToPath(import.meta.url));
export const REPO = resolve(TOOL_DIR, "..", "..");

// The CLI versions tools/conductor/spike/README.md's evidence table was produced on. Re-running the
// spike probe is how one is added. An unlisted version at a higher patch of a listed major.minor runs
// with a warning; any other unlisted version is refused (ADR-0208).
export const VERIFIED_CLI = ["2.1.270", "2.1.272", "2.1.273", "2.1.278"];

/**
 * What preflight makes of `claude --version`: {} for a listed version, { warning } for one sharing
 * major and minor with a listed version at a lower patch, { error } for anything else.
 */
export function cliVerdict(version, verified = VERIFIED_CLI) {
  if (verified.includes(version)) return {};
  const unlisted = `claude ${version} is not a verified CLI version (verified: ${verified.join(", ")})`;
  const clear = "re-run tools/conductor/spike/probe.mjs and record the evidence before adding it";
  const [major, minor, patch] = version.split(".").map(Number);
  const lowerPatch = verified.some((v) => {
    const [a, b, c] = v.split(".").map(Number);
    return a === major && b === minor && c < patch;
  });
  if (lowerPatch) {
    return { warning: `${unlisted}; a patch update of a verified version runs with this warning (ADR-0208) - ${clear}` };
  }
  return { error: `${unlisted}; ${clear}` };
}

export function paths({ repo = REPO, toolDir = TOOL_DIR } = {}) {
  return {
    repo,
    toolDir,
    queue: join(toolDir, "queue.json"),
    local: join(toolDir, "local.json"),
    stateDir: join(toolDir, "state"),
    digest: join(toolDir, "digest.md"),
    digestHistory: join(toolDir, "digest-history.md"),
    settings: join(toolDir, "settings.conductor.json"),
    prompts: join(toolDir, "prompts"),
    withLock: join(toolDir, "with-lock.mjs"),
    worktreeRoot: dirname(repo),
  };
}

export function claudeVersion(claude) {
  const [bin, ...pre] = claude;
  const r = spawnSync(bin, [...pre, "--version"], { encoding: "utf8" });
  if (r.status !== 0) return { error: `could not run ${claude.join(" ")} --version: ${r.error?.message ?? r.stderr}` };
  const m = (r.stdout ?? "").match(/(\d+\.\d+\.\d+)/);
  return m ? { version: m[1], raw: r.stdout.trim() } : { error: `unrecognised --version output: ${r.stdout}` };
}

/**
 * Everything that must hold before a run starts. Returns
 * { errors, warnings, notices, local, queue, state, claude, cli: { version, warning } | null }.
 * `claude` overrides local.json's command vector (tests pass the fake).
 */
export function preflight(p = paths(), { claude } = {}) {
  const errors = [];
  const warnings = [];
  const { errors: localErrors, local } = loadLocal(p.local);
  errors.push(...localErrors);
  const command = claude ?? local?.claude ?? ["claude"];
  const v = claudeVersion(command);
  let cli = null;
  if (v.error) errors.push(v.error);
  else {
    const verdict = cliVerdict(v.version);
    if (verdict.error) errors.push(verdict.error);
    if (verdict.warning) warnings.push(verdict.warning);
    cli = { version: v.version, warning: verdict.warning ?? null };
  }
  const state = loadState(p.stateDir);
  const queue = loadQueue(p.queue, p.repo, startedPlans(state));
  errors.push(...queue.errors);
  return { errors, warnings, notices: queue.notices ?? [], local, queue, state, claude: command, cli };
}

const pidFile = (p) => join(p.stateDir, "conductor.pid");

function runningPid(p) {
  if (!existsSync(pidFile(p))) return null;
  const pid = Number(readFileSync(pidFile(p), "utf8").trim());
  return pidAlive(pid) ? pid : null;
}

function regenerate(p, state) {
  writeDigest(p.digest, state, { repo: p.repo, stateDir: p.stateDir });
}

/** The line `run` prints for a lane event as it happens, or null for one it does not print. */
export function eventLine(name, d) {
  switch (name) {
    case "worktree-cap":
      return `conductor: lane ${d.lane} stopped at the worktree cap (max_open_worktrees ${d.max}, held by ${d.holding.join(", ")}); ${d.plan} not started`;
    case "lane-open":
      return `conductor: ${d.plan} opened its lane at ${d.worktree}`;
    case "park":
      return `conductor: ${d.plan} parked (${d.reason})`;
    case "closed":
      return `conductor: ${d.plan} closed${d.tag ? `, tag ${d.tag}` : ""}`;
    case "ff":
      return `conductor: ${d.plan} fast-forwarded main to ${d.head.slice(0, 7)}`;
    default:
      return null;
  }
}

const minutes = (iso) => (iso ? `${Math.max(0, Math.round((Date.now() - Date.parse(iso)) / 60000))} min` : "?");
const isPlan = (s) => /^\d{4}$/.test(s ?? "");

async function cmdRun(args, o) {
  const p = o.p;
  const laneIdx = args.indexOf("--lane");
  const lane = laneIdx >= 0 ? args[laneIdx + 1] : null;
  const pf = preflight(p, { claude: o.claude });
  const errors = [...pf.errors];
  if (currentBranch(p.repo) !== "main") errors.push(`the main checkout ${p.repo} is not on main`);
  if (lane && !pf.queue.lanes?.[lane]) errors.push(`queue.json has no lane "${lane}"`);
  const other = runningPid(p);
  if (other) errors.push(`a conductor is already running (pid ${other}); \`status\` shows it, \`abort\` stops it`);
  if (errors.length) {
    for (const e of errors) o.err(`conductor: ${e}`);
    return 1;
  }
  for (const n of pf.notices) o.log(`conductor: notice: ${n}`);
  for (const w of pf.warnings) o.err(`conductor: warning: ${w}`);

  const state = pf.state;
  const recovered = recoverInterrupted(p.stateDir, state);
  if (recovered) o.log(`conductor: ${recovered} step(s) were in flight when the last run stopped; they will run again`);
  // A clean checkout has no state/ yet: nothing before this line writes into it.
  mkdirSync(p.stateDir, { recursive: true });
  // A pause does not outlive the run it was asked of (ADR-0219), so an ask sitting here belongs to a
  // conductor that is gone. Clearing it is what stops a dead run's ask from making this one a process
  // that starts, does nothing and exits.
  if (clearPause(p.stateDir)) {
    o.log("conductor: a pause left behind by an earlier run was cleared; a pause does not outlive its run");
  }
  writeFileSync(pidFile(p), String(process.pid));

  // Every line `run` prints while lanes run also goes to state/live.log, under one header per run.
  const liveLog = join(p.stateDir, "live.log");
  const appendLive = (text) => {
    try {
      appendFileSync(liveLog, text);
    } catch {}
  };
  appendLive(`\n== run ${new Date().toISOString().slice(0, 16).replace("T", " ")} UTC, lanes ${(lane ? [lane] : Object.keys(pf.queue.lanes)).join(", ")} ==\n`);
  const emit = (line) => {
    const text = ascii(line);
    o.log(text);
    appendLive(`${text}\n`);
  };

  const ctx = {
    repo: p.repo,
    worktreeRoot: o.worktreeRoot ?? p.worktreeRoot,
    stateDir: p.stateDir,
    promptsDir: p.prompts,
    settingsFile: p.settings,
    withLockPath: p.withLock,
    claude: pf.claude,
    cli: pf.cli,
    local: pf.local,
    queue: pf.queue,
    state,
    gate: o.gate,
    lockDir: o.lockDir,
    lockPollMs: o.lockPollMs,
    pollMs: o.pollMs,
    once: args.includes("--once"),
    paused: () => Boolean(pauseAsk(p.stateDir)),
    lanes: lane ? [lane] : undefined,
    commitPollMs: o.commitPollMs,
    onChange: () => regenerate(p, state),
    live: emit,
    events: (name, data) => {
      const line = eventLine(name, data);
      if (line) emit(line);
    },
  };

  const interrupt = () => {
    for (const child of activeChildren) killTree(child);
    recoverInterrupted(p.stateDir, state);
    regenerate(p, state);
    clearPause(p.stateDir);
    rmSync(pidFile(p), { force: true });
    o.err("conductor: interrupted; in-flight steps will run again on the next `run`");
    process.exit(130);
  };
  if (o.signals !== false) {
    process.once("SIGINT", interrupt);
    process.once("SIGTERM", interrupt);
  }
  try {
    await runLanes(ctx);
  } finally {
    if (o.signals !== false) {
      process.removeListener("SIGINT", interrupt);
      process.removeListener("SIGTERM", interrupt);
    }
    regenerate(p, state);
    clearPause(p.stateDir);
    rmSync(pidFile(p), { force: true });
  }
  const recs = Object.values(state.plans);
  if (ctx.run?.paused) {
    o.log("conductor: paused - the plan in flight finished and no further plan was started; the ask is cleared.");
  }
  o.log(
    `conductor: run ended - ${recs.filter((r) => r.status === "merged").length} merged, ` +
      `${recs.filter((r) => r.status === "parked").length} parked. Nothing was pushed.`,
  );
  o.log(`digest: ${p.digest}`);
  return 0;
}

function cmdStatus(args, o) {
  const p = o.p;
  const state = loadState(p.stateDir);
  const pid = runningPid(p);
  o.log(pid ? `conductor: running (pid ${pid})` : "conductor: not running");
  const { lanes } = loadQueue(p.queue, p.repo, startedPlans(state));
  const laneNames = [...new Set([...Object.keys(lanes ?? {}), ...Object.keys(state.lanes)])].sort();
  for (const lane of laneNames) {
    const l = pid ? state.lanes[lane] : null;
    if (!l?.plan) {
      o.log(`lane ${lane}: idle`);
      continue;
    }
    const rec = state.plans[l.plan];
    const step = l.step ? `step ${l.step} for ${minutes(l.stepStarted)}` : "between steps";
    const waiting = l.waitingUntil ? `, waiting out the usage limit until ${l.waitingUntil}` : "";
    o.log(`lane ${lane}: plan ${l.plan}, ${step}${waiting}, spend so far $${totalSpend(rec).toFixed(2)}`);
  }
  const parked = Object.values(state.plans).filter((r) => r.status === "parked");
  if (parked.length === 0) o.log("parked: none");
  else {
    o.log("parked:");
    // The same verdict the digest renders, so a record the repository has already settled is never
    // printed here as work while the page calls it stale (ADR-0214).
    for (const r of parked) {
      const settled = settledPark(r, p.repo);
      o.log(`- ${r.plan} (${r.park.reason}): ${r.park.detail}${settled ? ` - already settled: ${settled}; \`resume ${r.plan}\` clears the record` : ""}`);
    }
  }
  regenerate(p, state);
  o.log(`digest: ${p.digest}`);
  return 0;
}

/**
 * `digest` rewrites the current-state page; `digest --history` writes the per-run account beside it
 * (ADR-0214). The history is written only when asked for: a second page always on disk, read
 * approximately never, is where a stale reading would hide.
 */
function cmdDigest(args, o) {
  const p = o.p;
  const state = loadState(p.stateDir);
  if (args.includes("--history")) {
    writeHistory(p.digestHistory, state, { repo: p.repo, stateDir: p.stateDir });
    o.log(`history: ${p.digestHistory}`);
    return 0;
  }
  regenerate(p, state);
  o.log(`digest: ${p.digest}`);
  return 0;
}

/** Why a park still holds, or null when the owner has acted on it. */
function parkStillTrue(p, rec) {
  const { reason, phase } = rec.park;
  // Whatever the reason, no new session starts on a tree the last one left dirty.
  const dirty = dirtyWorktree(rec.worktree);
  if (dirty) return `the worktree ${rec.worktree} has uncommitted changes: ${dirtyText(dirty)}; commit them, or \`git restore\` them there, first`;
  // Both of these park on a phase only the owner can do — one the plan tagged `human`, one whose
  // files the CLI will not let a session touch (ADR-0210). Either way the lane moves on when the
  // plan's own log says the phase is done, which is the same evidence for both.
  if (reason === "human_phase" || reason === CLAUDE_DIR) {
    const where = rec.worktree && existsSync(rec.worktree) ? rec.worktree : p.repo;
    const found = findPlan(where, rec.plan);
    if (!found) return `plan ${rec.plan} is not in ${where}`;
    if (!donePhases(readPlanFile(found.path)).has(phase)) {
      const rel = relative(where, found.path).replace(/\\/g, "/");
      return `Phase ${phase} is still not marked done in the ## Implementation log of ${rel} in ${where}; commit the row there first`;
    }
  }
  if (reason === "main_dirty" && (currentBranch(p.repo) !== "main" || !isClean(p.repo))) {
    return `the main checkout is still dirty or not on main`;
  }
  return null;
}

function cmdResume(args, o) {
  const p = o.p;
  const [plan] = args;
  if (!isPlan(plan)) {
    o.err("usage: conductor.mjs resume NNNN");
    return 2;
  }
  if (runningPid(p)) {
    o.err("conductor: a run is in progress; resume after it ends, or `abort` it first");
    return 1;
  }
  const state = loadState(p.stateDir);
  const rec = state.plans[plan];
  if (!rec || rec.status !== "parked") {
    o.err(`conductor: plan ${plan} is not parked (${rec?.status ?? "never started"})`);
    return 1;
  }
  const still = parkStillTrue(p, rec);
  if (still) {
    o.err(`conductor: refusing to resume ${plan} - its park reason (${rec.park.reason}) still holds: ${still}`);
    return 1;
  }
  const reason = rec.park.reason;
  rec.status = "queued";
  rec.park = null;
  if (reason === "review_failed") rec.fixRounds = 0;
  saveState(p.stateDir, state);
  regenerate(p, state);
  o.log(`conductor: plan ${plan} is queued again; \`run\` picks it up in lane ${rec.lane}`);
  return 0;
}

function cmdPark(args, o) {
  const p = o.p;
  const [plan] = args;
  if (!isPlan(plan)) {
    o.err("usage: conductor.mjs park NNNN");
    return 2;
  }
  if (runningPid(p)) {
    o.err("conductor: a run is in progress; park after it ends, or `abort` it first");
    return 1;
  }
  const state = loadState(p.stateDir);
  const rec = planRecord(state, plan);
  if (rec.status === "merged" || rec.status === "parked") {
    o.err(`conductor: plan ${plan} is already ${rec.status}`);
    return 1;
  }
  rec.status = "parked";
  rec.park = { reason: "owner", detail: "parked by the owner", phase: null, read: null, worktree: rec.worktree, at: new Date().toISOString() };
  const dirty = dirtyWorktree(rec.worktree);
  if (dirty) rec.park.dirty = dirty;
  rec.parks.push(rec.park);
  appendPark(statePaths(p.stateDir).inbox, { plan, reason: "owner", detail: "parked by the owner", worktree: rec.worktree, dirty });
  saveState(p.stateDir, state);
  regenerate(p, state);
  o.log(`conductor: plan ${plan} parked; \`resume ${plan}\` queues it again`);
  return 0;
}

const FINDING_USAGE = `usage: conductor.mjs finding NNNN [<ref> --${FINDING_VERBS.join("|--")} <reason>]`;

/** How a finding reads on the command line: what the verdict carried, then what has become of it. */
function findingText(f, index) {
  const d = f.disposition;
  return (
    `  [${index}] ${f.severity} ${findingWhere(f)} - ${f.what}` +
    (f.fixed_in ? ` - repaired by the close in ${f.fixed_in.slice(0, 7)}` : "") +
    (d ? ` - closed ${d.at.slice(0, 10)} (${d.verb}): ${d.reason}` : "")
  );
}

/** Where a plan stands when it has no close, said in the refusal so the reader knows why. */
function planStanding(rec) {
  if (!rec) return "it never started";
  return `it is ${rec.status}` + (rec.fixRounds ? `, ${rec.fixRounds} fix round${rec.fixRounds === 1 ? "" : "s"} in` : "");
}

/**
 * `finding NNNN` lists a plan's closing verdict; `finding NNNN <ref> --done|--wontfix|--filed
 * <reason>` records the owner's disposition against one of them (ADR-0216). The reason is required
 * and nothing verifies any of it: only the owner writes a disposition, so this command is the whole
 * record of the judgement. `<ref>` is the index the listing prints, or the `file:line` exactly one
 * finding carries.
 */
function cmdFinding(args, o) {
  const p = o.p;
  const [plan, ref, flag, ...reasonWords] = args;
  // The verb is the entry that matched the flag in full, dashes included: taking it from the typed
  // word instead lets `done` through as `ne`, and nothing downstream verifies a disposition.
  const verb = args.length > 2 ? (FINDING_VERBS.find((v) => flag === `--${v}`) ?? null) : null;
  if (!isPlan(plan) || args.length === 2 || (args.length > 2 && verb === null)) {
    o.err(FINDING_USAGE);
    return 2;
  }
  const reason = reasonWords.join(" ").trim();
  if (verb && !reason) {
    o.err(`conductor: --${verb} needs a reason; a disposition with none is how a finding gets closed for being old (ADR-0216)`);
    return 2;
  }
  if (verb && runningPid(p)) {
    o.err("conductor: a run is in progress and would overwrite the record; close the finding after it ends, or `abort` it first");
    return 1;
  }

  const state = loadState(p.stateDir);
  const rec = state.plans[plan];
  // The close, not the array, is what makes the last verdict a closing one: a `verdict` outcome
  // pushes its findings before any fix round, so a plan still in or parked at a round carries
  // verdicts and no close, and its blockers are the conductor's own work in flight.
  const verdict = rec?.closed ? rec.verdicts?.at(-1) : null;
  if (!verdict) {
    o.err(`conductor: plan ${plan} has no closing verdict, so it has no findings (${planStanding(rec)})`);
    return 1;
  }
  const findings = verdict.findings ?? [];
  if (findings.length === 0) {
    const where = `conductor: plan ${plan} closed with no findings (verdict round ${verdict.round})`;
    // A refusal says its one sentence on stderr, like every other one here; the listing is output.
    if (verb) {
      o.err(`${where}, so there is nothing to close`);
      return 1;
    }
    o.log(`${where}.`);
    return 0;
  }
  if (!verb) {
    o.log(`conductor: plan ${plan}, closing verdict round ${verdict.round}, ${findings.length} finding${findings.length === 1 ? "" : "s"}:`);
    for (const [i, f] of findings.entries()) o.log(findingText(f, i));
    return 0;
  }

  const found = findingRef(findings, ref);
  if (found.error) {
    o.err(`conductor: ${found.error}`);
    return 1;
  }
  const f = findings[found.index];
  const previous = disposeFinding(f, verb, reason);
  saveState(p.stateDir, state);
  regenerate(p, state);
  o.log(`conductor: plan ${plan} finding ${found.index} (${f.severity} ${findingWhere(f)}) is closed ${verb}: ${reason}`);
  if (previous) o.log(`  it was ${previous.verb} on ${previous.at.slice(0, 10)} (${previous.reason}); that stays in the finding's history.`);
  return 0;
}

/**
 * `adopt-close NNNN`: record the close a session already committed in the lane, so the repair for a
 * park that landed after its close is a command rather than a hand edit to state/conductor.json
 * (backlog 0229). It runs the same check the lane does — `verifyClose` against the branch as it
 * stands — and changes nothing unless that passes. The gate on the close tip and the fast-forward
 * stay the conductor's: `resume` then `run` does them.
 */
function cmdAdoptClose(args, o) {
  const p = o.p;
  const [plan] = args;
  if (!isPlan(plan)) {
    o.err("usage: conductor.mjs adopt-close NNNN");
    return 2;
  }
  if (runningPid(p)) {
    o.err("conductor: a run is in progress; adopt-close after it ends, or `abort` it first");
    return 1;
  }
  const state = loadState(p.stateDir);
  const rec = state.plans[plan];
  if (!rec?.worktree || !existsSync(rec.worktree)) {
    o.err(`conductor: plan ${plan} has no lane on disk (${rec?.worktree ?? "no worktree recorded"})`);
    return 1;
  }
  if (rec.closed) {
    o.err(`conductor: plan ${plan} is already recorded closed (${rec.closed.tag ?? "no tag"}); nothing to adopt`);
    return 1;
  }
  const adopted = adoptedClose({ cwd: rec.worktree, plan, round: rec.verdicts.length + 1 });
  if (!adopted) {
    o.err(`conductor: no close to adopt in ${rec.worktree} - plan ${plan} is not under docs/plans/done/ with Status done and a ## Close review`);
    return 1;
  }
  const problems = verifyClose({ cwd: rec.worktree, plan, outcome: adopted });
  if (problems.length) {
    o.err(`conductor: the close in ${rec.worktree} does not verify, so nothing was recorded:`);
    for (const problem of problems) o.err(`  - ${problem}`);
    return 1;
  }
  rec.verdicts.push({ ...adopted.verdict });
  rec.closed = { version: adopted.version, tag: adopted.tag, head: head(rec.worktree), at: new Date().toISOString(), adopted: true };
  saveState(p.stateDir, state);
  regenerate(p, state);
  o.log(`conductor: plan ${plan} recorded closed${adopted.tag ? `, tag ${adopted.tag}` : " with no version"} from its ## Close review.`);
  o.log(rec.status === "parked" ? `Run \`resume ${plan}\`, then \`run\`: the gate on the close tip and the fast-forward still owe.` : "The gate on the close tip and the fast-forward still owe; `run` does them.");
  return 0;
}

/**
 * `pause` asks a live run to finish the plan in flight and start no further one; `pause --off`
 * cancels the ask while the run is still live (ADR-0219). It prints what it is now waiting for,
 * because the gap between asking and stopping is a suite's twelve minutes and an operator who cannot
 * see it reaches for `abort` instead. Setting one needs a running conductor: the ask does not outlive
 * a run, so recorded against no run it would be an instruction nothing ever reads.
 */
function cmdPause(args, o) {
  const p = o.p;
  if (args.includes("--off")) {
    const cleared = clearPause(p.stateDir);
    o.log(cleared ? "conductor: the pause is off; a lane starts its next queued plan again" : "conductor: no pause was asked for");
    return 0;
  }
  const pid = runningPid(p);
  if (!pid) {
    o.err("conductor: no conductor is running, and a pause does not outlive a run; start the run you want with `run --once`");
    return 1;
  }
  const already = pauseAsk(p.stateDir);
  askPause(p.stateDir, already ?? { at: new Date().toISOString(), pid });
  o.log(
    already
      ? `conductor: already paused (asked at ${already.at ?? "an unreadable time"}). Waiting for:`
      : "conductor: paused - each lane finishes the plan in flight and starts no other. `pause --off` cancels. Waiting for:",
  );
  const state = loadState(p.stateDir);
  const inFlight = Object.entries(state.lanes)
    .filter(([, l]) => l?.plan)
    .sort(([a], [b]) => a.localeCompare(b));
  if (inFlight.length === 0) o.log("- no lane has a plan in flight; the run ends as soon as each lane looks again");
  for (const [lane, l] of inFlight) {
    o.log(`- lane ${lane}: plan ${l.plan}, ${l.step ? `step ${l.step} for ${minutes(l.stepStarted)}` : "between steps"}`);
  }
  return 0;
}

function cmdAbort(args, o) {
  const p = o.p;
  const pid = runningPid(p);
  if (pid) {
    // On Windows a signal cannot run the conductor's handler, so the whole tree goes: the
    // conductor and every claude session under it.
    if (process.platform === "win32") spawnSync("taskkill", ["/PID", String(pid), "/T", "/F"], { stdio: "ignore" });
    else process.kill(pid, "SIGTERM");
    const deadline = Date.now() + 15_000;
    while (pidAlive(pid) && Date.now() < deadline) spawnSync(process.execPath, ["-e", "setTimeout(()=>{},200)"]);
    if (pidAlive(pid)) {
      o.err(`conductor: pid ${pid} did not stop`);
      return 1;
    }
  }
  const state = loadState(p.stateDir);
  const n = recoverInterrupted(p.stateDir, state);
  rmSync(pidFile(p), { force: true });
  regenerate(p, state);
  o.log(pid ? `conductor: stopped pid ${pid}; ${n} in-flight step(s) will run again on the next \`run\`` : "conductor: not running");
  return 0;
}

function cmdCheck(args, o) {
  const r = preflight(o.p, { claude: o.claude });
  if (r.errors.length) {
    for (const e of r.errors) o.err(`conductor: ${e}`);
    return 1;
  }
  for (const n of r.notices) o.log(`conductor: notice: ${n}`);
  for (const w of r.warnings) o.err(`conductor: warning: ${w}`);
  o.log(r.warnings.length ? "conductor: preflight OK, with a warning" : "conductor: preflight OK");
  return 0;
}

/**
 * `prune` drops every merged plan from queue.json's lane lists (ADR-0220). The committed queue is
 * accumulate-only and nothing else removes from it; this is the carrier for a discipline whose only
 * previous enforcement was someone remembering. It edits a committed file, so it prints every plan
 * it dropped and the file to commit, and it rewrites nothing when there is nothing to drop.
 */
function cmdPrune(args, o) {
  const p = o.p;
  if (runningPid(p)) {
    o.err("conductor: a run is in progress and reads the queue it started with; prune after it ends, or `abort` it first");
    return 1;
  }
  const r = readQueue(p.queue);
  if (r.error) {
    o.err(`conductor: ${r.error}`);
    return 1;
  }
  const { queue, dropped } = pruneQueue(r.value, p.repo);
  if (dropped.length === 0) {
    o.log("conductor: the queue lists no merged plan; nothing to prune");
    return 0;
  }
  writeFileSync(p.queue, JSON.stringify(queue, null, 2) + "\n");
  for (const d of dropped) o.log(`conductor: dropped plan ${d.plan} from lane ${d.lane} (${d.file} is under docs/plans/done/)`);
  o.log(`conductor: ${p.queue} rewritten; commit it.`);
  return 0;
}

const COMMANDS = { run: cmdRun, status: cmdStatus, digest: cmdDigest, resume: cmdResume, park: cmdPark, finding: cmdFinding, "adopt-close": cmdAdoptClose, pause: cmdPause, abort: cmdAbort, prune: cmdPrune, check: cmdCheck };

/** `overrides` exists for tests: { p, claude, gate, worktreeRoot, lockDir, lockPollMs, pollMs, commitPollMs, log, err, signals }. */
export async function main(argv, overrides = {}) {
  const o = {
    p: paths(),
    log: (s) => console.log(s),
    err: (s) => console.error(s),
    ...overrides,
  };
  const [command, ...args] = argv;
  const fn = COMMANDS[command];
  if (!fn) {
    o.err(
      "usage: node tools/conductor/conductor.mjs run [--lane a|b] [--once] | status | digest [--history] | " +
        `resume NNNN | park NNNN | finding NNNN [<ref> --${FINDING_VERBS.join("|--")} <reason>] | adopt-close NNNN | pause [--off] | abort | prune | check`,
    );
    return 2;
  }
  return fn(args, o);
}

const norm = (p) => (process.platform === "win32" ? resolve(p).toLowerCase() : resolve(p));
if (process.argv[1] && norm(fileURLToPath(import.meta.url)) === norm(process.argv[1])) {
  main(process.argv.slice(2)).then((code) => process.exit(code));
}
