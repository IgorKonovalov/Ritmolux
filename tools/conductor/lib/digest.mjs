// tools/conductor/digest.md: what is happening, and where you are needed (ADR-0214).
//
// Two renderers over one state reader. `renderDigest` is the current-state page, rewritten after
// every step: **Needs you** — the whole worklist — then **Now**, what each lane is doing this minute.
// `renderHistory` is the per-run account `conductor.mjs digest --history` writes beside it in
// digest-history.md: newest run first, each run's Needs you, Still parked from an earlier run (the
// newest run only), Not started, Closed, Failed and parked, Totals. Both build from
// `readDigestState` and neither reaches past it: a second traversal of state/conductor.json is where
// the two pages would drift.
//
// Generated from state/ and git only — never written by hand, never inside a worktree — so neither
// page carries a generation timestamp. The current page's park ages and a live lane's elapsed time
// are the one reading of the clock, taken from the reader's `now`; with no run live and nothing
// parked, either page regenerated from the same state yields the same bytes.
//
// Time in the history is always time within one run: a plan's active time is its steps' and gates'
// own durations, never a span across a park. A finding line copies the verdict outcome the reviewer
// emitted — severity, file:line, what — plus whatever the record says has become of it, and nothing
// else; neither page summarizes review prose. An event (a step, a park, a merge) belongs to the
// latest run that had started by the event's timestamp.

import { existsSync, readdirSync, readFileSync } from "node:fs";
import { join, relative } from "node:path";

import { isAncestor, resolveCommit, tagObjectType } from "./git.mjs";
import { dirtyText, resumeCommand } from "./inbox.mjs";
import { laneOpen } from "./lane.mjs";
import { readLedger } from "./ledger.mjs";
import { usageReading } from "./live.mjs";
import { CLAUDE_DIR } from "./outcome.mjs";
import { findPlan, parsePlan, readPlanFile, rowIsOwed, settledPhase } from "./plan.mjs";
import { findingWhere, statePaths, totalSpend, writeAtomic } from "./state.mjs";

const HUMAN_REASONS = new Set(["human_phase", "stop_condition", "question", "plan_wrong"]);
const API_REASONS = new Set(["api", "no_outcome", "bad_outcome"]);

const stamp = (iso) => (iso ? iso.slice(0, 16).replace("T", " ") : "?");
const usd = (n) => `$${(n ?? 0).toFixed(2)}`;
const short = (sha) => (sha ? sha.slice(0, 7) : "?");
const plural = (n, word) => `${n} ${word}${n === 1 ? "" : "s"}`;

export function duration(ms) {
  if (!(ms >= 0)) return "?";
  const min = Math.round(ms / 60000);
  if (min < 1) return "< 1 min";
  const h = Math.floor(min / 60);
  return h ? `${h} h ${min % 60} min` : `${min} min`;
}

const span = (a, b) => (a && b ? duration(Date.parse(b) - Date.parse(a)) : "?");

/** A step's two usage readings: the ones endStep kept, or the raw event an older record carried. */
function stepUsage(s) {
  return {
    first: s.usage?.first ?? usageReading(s.result?.rateLimitFirst ?? null),
    last: s.usage?.last ?? usageReading(s.result?.rateLimit ?? null),
  };
}

const resetStamp = (epochSeconds) => (typeof epochSeconds === "number" ? stamp(new Date(epochSeconds * 1000).toISOString()).slice(5) : "?");

/** `5h 0.27 (resets 09-15 14:30); 7d 0.02 (resets 09-22 16:00)`, in UTC like every digest stamp. */
function usageLine(r) {
  const parts = [];
  if (r.five) parts.push(`5h ${r.five.utilization.toFixed(2)} (resets ${resetStamp(r.five.resetsAt)})`);
  if (r.seven) parts.push(`7d ${r.seven.utilization.toFixed(2)} (resets ${resetStamp(r.seven.resetsAt)})`);
  if (r.status && r.status !== "allowed") parts.push(r.status);
  return parts.join("; ");
}

const gateMs = (g) => (g.commands ?? []).reduce((t, c) => t + (c.ms ?? 0), 0);
const isSuite = (c) => c.suite === true || c.name === "cargo nextest";

/**
 * The step a park came out of: the last step that ended by the park, started in the same run, with
 * no gate between its end and the park. Null for a park no session produced (a gate, a merge, a lane).
 */
function parkSession(rec, p, sameRun) {
  const step = [...rec.steps].reverse().find((s) => s.ended && s.ended <= p.at);
  if (!step || !sameRun(step.started)) return null;
  if ((rec.gates ?? []).some((g) => g.at > step.ended && g.at <= p.at)) return null;
  return step;
}

function runOf(runs, iso) {
  if (!iso) return -1;
  let idx = -1;
  for (const [i, r] of runs.entries()) if (r.started <= iso) idx = i;
  return idx;
}

function readLockLog(stateDir) {
  const file = statePaths(stateDir).lockLog;
  if (!existsSync(file)) return [];
  return readFileSync(file, "utf8")
    .split("\n")
    .filter(Boolean)
    .map((l) => {
      try {
        return JSON.parse(l);
      } catch {
        return null;
      }
    })
    .filter(Boolean);
}

function planTitle(repo, plan) {
  const found = findPlan(repo, plan);
  if (!found) return { title: `Plan ${plan}`, rel: null };
  return { title: readPlanFile(found.path).title ?? `Plan ${plan}`, rel: relative(repo, found.path).replace(/\\/g, "/") };
}

function findingLine(f, resolvedIn) {
  const fixed = resolvedIn ? ` - resolved in \`${short(resolvedIn)}\`` : f.fixed_in ? ` - repaired by the close in \`${short(f.fixed_in)}\`` : "";
  const d = f.disposition;
  const closed = d ? ` - closed ${d.at.slice(0, 10)} (${d.verb}): ${d.reason}` : "";
  return `  - ${f.severity} \`${findingWhere(f)}\` ${f.what}${fixed}${closed}`;
}

/**
 * The minors and nits the closing verdict merged with that its close did not repair (ADR-0209) and
 * the owner has not disposed of (ADR-0216). `fixed_in` is evidence checked against the branch; a
 * disposition is a judgement checked against nothing, and both take a finding off the worklist.
 */
function openFindings(rec) {
  return (rec.verdicts.at(-1)?.findings ?? []).filter((f) => (f.severity === "minor" || f.severity === "nit") && !f.fixed_in && !f.disposition);
}

/** How many of a plan's closing findings the owner has closed with a verb and a reason. */
function closedCount(rec) {
  return (rec.verdicts.at(-1)?.findings ?? []).filter((f) => f.disposition).length;
}

/**
 * What a plan's steps cost inside one run. `totalSpend` sums every step the plan ever ran, across
 * every run; a bullet that carries a run-scoped time beside a lifetime `$` reads as neither, so both
 * figures are named. Totals sums this over every plan, so the two agree by construction.
 */
const spendInRun = (rec, inRun) => rec.steps.filter((s) => inRun(s.started)).reduce((t, s) => t + (s.result?.spendUsd ?? 0), 0);

/**
 * A plan's time in one run. `active` is the sum of its steps' and gates' own durations there;
 * `wall` runs from its first step or gate in the run to its merge, so a night spent parked before
 * the run is never counted.
 */
function timeInRun(rec, run, inRun) {
  let active = 0;
  const starts = [];
  for (const s of rec.steps.filter((x) => inRun(x.started) && x.ended)) {
    active += Date.parse(s.ended) - Date.parse(s.started);
    starts.push(Date.parse(s.started));
  }
  for (const g of (rec.gates ?? []).filter((x) => inRun(x.at))) {
    const ms = gateMs(g);
    active += ms;
    starts.push(Date.parse(g.at) - ms);
  }
  const end = Date.parse(rec.merge?.at ?? rec.ended ?? run.ended);
  const start = starts.length ? Math.min(...starts) : Date.parse(run.started);
  return { active, wall: end - start };
}

function closedFindings(rec) {
  const lines = [];
  for (const v of rec.verdicts) {
    const fix = rec.fixes.find((x) => x.round === v.round);
    for (const [i, f] of (v.findings ?? []).entries()) {
      lines.push(findingLine(f, fix?.resolved.find((r) => r.finding === i)?.commit));
    }
  }
  return lines;
}

/**
 * Why the repository has already settled a park, as a phrase, or null while the park still holds.
 * Two conditions, both read from the tree and both narrow (ADR-0214):
 *
 * - the plan is under `docs/plans/done/` **in the main checkout** with `Status: done` — a close that
 *   landed outside the conductor, which never touches state/conductor.json;
 * - a park on a phase only the owner can do (`human_phase`, `claude_dir`) sits on a phase the plan's
 *   own `## Implementation log` now marks done, or owed on a phase marked `Blocks merge: no`
 *   (`settledPhase`, the same reader `parkStillTrue` asks). That row is read in the lane when the
 *   worktree is still there and in the main checkout otherwise, so a lane removed by hand is not a
 *   missing plan.
 *
 * A branch's own `done/` copy is deliberately not enough: a close committed in a lane that has not
 * merged is unfinished work, and a wrong "already settled" tells the owner the opposite. Nothing else
 * is a signal here — never an age, never a branch's commits, never a tag.
 */
export function settledPark(rec, repo) {
  if (!rec.park) return null;
  const closed = findPlan(repo, rec.plan);
  if (closed?.done && readPlanFile(closed.path).statusWord === "done") {
    return "the plan is under `docs/plans/done/` with `Status: done`";
  }
  const { reason, phase } = rec.park;
  if ((reason !== "human_phase" && reason !== CLAUDE_DIR) || !phase) return null;
  const where = rec.worktree && existsSync(rec.worktree) ? rec.worktree : repo;
  const found = findPlan(where, rec.plan);
  const how = found ? settledPhase(readPlanFile(found.path), phase) : null;
  if (!how) return null;
  return `Phase ${phase} now reads \`${how}\` in the plan's \`## Implementation log\``;
}

/**
 * Every phase a closed plan still owes (ADR-0249): the `owed` rows of each plan under
 * `docs/plans/done/` in `repo`, the main checkout. Read from the tree and never from `state/`, so a
 * wiped state directory and a close that happened outside the conductor both still show it, and
 * the owner's commit marking the row done is the whole of settling it.
 */
export function owedPhasesInDone(repo) {
  const dir = join(repo, "docs", "plans", "done");
  if (!existsSync(dir)) return [];
  const out = [];
  for (const file of readdirSync(dir).filter((f) => /^\d{4}-.*\.md$/.test(f)).sort()) {
    const text = readFileSync(join(dir, file), "utf8");
    if (!/\|\s*owed\b/i.test(text)) continue;
    const doc = parsePlan(text);
    for (const row of doc.log.rows.filter(rowIsOwed)) {
      out.push({ plan: doc.number ?? file.slice(0, 4), phase: row.id, title: row.title, rel: `docs/plans/done/${file}` });
    }
  }
  return out;
}

/**
 * The repair commits a merged plan's closed tip carries that no review read (ADR-0248): a repair at
 * `post-close` or `remerge`. Each stays on the page until `origin/main` holds it, since the owner's
 * reading before the push is the whole of what checks it; with no `origin/main` it stays.
 */
function unreviewedRepairs(rec, repo) {
  const pushed = repo && resolveCommit("refs/remotes/origin/main", repo);
  return (rec.repairs ?? [])
    .filter((r) => r.unreviewed)
    .flatMap((r) => r.commits.map((sha) => ({ sha, stage: r.stage })))
    .filter(({ sha }) => !(pushed && isAncestor(sha, "refs/remotes/origin/main", repo)));
}

/**
 * The newest reading of `origin/main`'s CI any close took (ADR-0251), across every plan's record, that
 * actually read something. An unread reading is skipped rather than returned: it says nothing about
 * main, so a red reading stays on the page until a later close reads green, and never leaves because
 * a machine without `gh` closed after it. Null when no close ever read one.
 */
export function latestUpstream(plans) {
  let best = null;
  for (const rec of plans) {
    for (const u of rec.upstream ?? []) {
      if (u.state !== "green" && u.state !== "red") continue;
      if (!best || u.at > best.at) best = { ...u, plan: rec.plan };
    }
  }
  return best;
}

/**
 * The one traversal of `state`, `state/` and git both pages render from. `now` is an option so a
 * test can pin the clock the current page's ages read.
 */
export function readDigestState(state, { repo, stateDir, now = Date.now() }) {
  const runs = state.runs ?? [];
  const plans = Object.values(state.plans ?? {}).sort((a, b) => a.plan.localeCompare(b.plan));
  const latest = runs.at(-1) ?? null;
  const parked = plans.filter((r) => r.status === "parked" && r.park).map((rec) => ({ rec, settled: settledPark(rec, repo) }));
  return {
    repo,
    stateDir,
    now,
    runs,
    plans,
    latest,
    running: Boolean(latest && !latest.ended),
    laneStates: state.lanes ?? {},
    lockLog: readLockLog(stateDir),
    ledger: readLedger(join(stateDir, "suite-ledger.jsonl")),
    parked: parked.filter((p) => !p.settled).map((p) => p.rec),
    settled: parked.filter((p) => p.settled),
    merged: plans.filter((r) => r.status === "merged"),
    stops: latest?.stops ?? [],
    cli: latest?.cli ?? null,
    owed: repo ? owedPhasesInDone(repo) : [],
    upstream: latestUpstream(plans),
  };
}

/** `1 merged, 0 parked, 6 h 28 min, $12.30` — one run's own figures, for both pages. */
function runFigures(view, i) {
  const run = view.runs[i];
  const inRun = (iso) => runOf(view.runs, iso) === i;
  const merged = view.plans.filter((r) => r.status === "merged" && inRun(r.merge?.at)).length;
  const parked = view.plans.reduce((n, r) => n + r.parks.filter((p) => inRun(p.at)).length, 0);
  const spend = view.plans.reduce((s, r) => s + spendInRun(r, inRun), 0);
  return `${merged} merged, ${parked} parked, ${run.ended ? span(run.started, run.ended) : "still running"}, ${usd(spend)}`;
}

/** Where a parked plan's work sits: the worktree it still holds, or the branch `resume` reopens. */
function parkHolds(rec) {
  return laneOpen(rec) ? `Holds \`${rec.worktree}\`.` : `Worktree removed; \`resume\` reopens it from branch \`${rec.branch ?? "?"}\`.`;
}

/** A standing park as the worklist states it: one bullet, then the command that clears it. */
function standingParkLines(view, rec) {
  const p = rec.park;
  const i = runOf(view.runs, p.at);
  const step = parkSession(rec, p, (iso) => runOf(view.runs, iso) === i);
  const usage = step ? stepUsage(step).last : null;
  return [
    `- **${rec.plan}** (\`${p.reason}\`)${p.phase ? ` at Phase ${p.phase}` : ""} parked ${stamp(p.at)}, ${duration(view.now - Date.parse(p.at))} ago. ` +
      `${p.detail}. Read: ${p.read ?? "the plan"}. ${parkHolds(rec)}` +
      (p.dirty ? ` Left dirty: ${dirtyText(p.dirty)}.` : "") +
      (usage ? ` Usage at park: ${usageLine(usage)}.` : ""),
    `  Resume: \`${resumeCommand(rec.plan)}\``,
  ];
}

/** The current page's first section: everything waiting on the owner, and nothing else. */
function needsYou(view) {
  const lines = [];
  const counts = { parked: 0, stops: 0, findings: 0, lanes: 0, owed: 0, repairs: 0, upstream: 0 };
  // The run carries its own CLI reading, so the line stops appearing on the first run whose version is listed.
  if (view.cli?.warning) {
    lines.push(`- **claude ${view.cli.version} is not a verified CLI version** - the last run went ahead with a warning (ADR-0208): ${view.cli.warning}.`);
  }
  // A red origin/main never stopped the close that read it (ADR-0251); this line is where it costs
  // something, and it leaves only when a later close reads green.
  const u = view.upstream;
  if (u?.state === "red") {
    counts.upstream = 1;
    const jobs = u.jobs?.length ? u.jobs.join(", ") : `run ${u.run}`;
    lines.push(
      `- **origin/main's CI is red**: run ${u.run}${u.sha ? ` at \`${short(u.sha)}\`` : ""}, failing ${jobs}, read by ${u.plan}'s close ${stamp(u.at)}. ` +
        `Repair main and push${u.url ? `; ${u.url}` : ""}. The line leaves when a later close reads it green.`,
    );
  }
  for (const rec of view.parked) {
    counts.parked += 1;
    lines.push(...standingParkLines(view, rec));
  }
  for (const o of view.owed) {
    counts.owed += 1;
    lines.push(
      `- **${o.plan} owes Phase ${o.phase}** (${o.title}): merged without it (\`Blocks merge: no\`). ` +
        `Do it, then mark its row \`done\` in \`${o.rel}\` on main and commit; the line leaves with the commit.`,
    );
  }
  for (const s of view.stops) {
    counts.stops += 1;
    lines.push(
      `- **Lane ${s.lane} stopped at the worktree cap** (\`max_open_worktrees\` ${s.max}): ${s.plan} was not opened. ` +
        `Worktrees held by ${s.holding.join(", ")}.`,
    );
  }
  let closed = 0;
  for (const rec of view.merged) {
    // A cleanup failure is settled the moment the directory is gone, whatever the record still says.
    if (rec.cleanup && !rec.cleanup.ok && laneOpen(rec)) {
      counts.lanes += 1;
      lines.push(`- **${rec.plan} merged, lane not removed**: ${rec.cleanup.detail}. Holds \`${rec.worktree}\`.`);
    }
    for (const r of unreviewedRepairs(rec, view.repo)) {
      counts.repairs += 1;
      lines.push(`- **${rec.plan} reached main with an unreviewed repair**: \`${short(r.sha)}\` at ${r.stage}. Read it before you push.`);
    }
    closed += closedCount(rec);
    const open = openFindings(rec);
    if (open.length === 0) continue;
    counts.findings += 1;
    lines.push(`- **${rec.plan} merged with ${plural(open.length, "open finding")}**:`);
    for (const f of open) lines.push(`  - ${f.severity} \`${findingWhere(f)}\` ${f.what}`);
  }
  // One line for every finding the owner has closed, never a per-plan breakdown: that is the
  // accumulation this page was rid of, one indent further in (ADR-0216). It is left out entirely at
  // zero, so a page with nothing on it stays one line.
  if (closed) {
    lines.push(
      `- ${plural(closed, "finding")} closed, each with a verb and a reason: ` +
        "`node tools/conductor/conductor.mjs finding NNNN` lists one plan's, `digest --history` every one.",
    );
  }
  // A park the repository has already settled is a record to clear, never work: it goes under its own
  // heading and is counted apart, so one real park is not read as five (ADR-0214). The digest writes
  // nothing back — clearing it stays an explicit `resume`.
  const stale = [];
  for (const { rec, settled } of view.settled) {
    stale.push(
      `- **${rec.plan}** (\`${rec.park.reason}\`)${rec.park.phase ? ` at Phase ${rec.park.phase}` : ""} parked ${stamp(rec.park.at)}: ` +
        `${settled}. ${parkHolds(rec)}`,
      `  Clear the record: \`${resumeCommand(rec.plan)}\``,
    );
  }
  const parts = [];
  if (counts.upstream) parts.push("origin/main red");
  if (counts.parked) parts.push(plural(counts.parked, "park"));
  if (view.settled.length) parts.push(`${view.settled.length} already settled`);
  if (counts.owed) parts.push(`${plural(counts.owed, "owed phase")}`);
  if (counts.repairs) parts.push(`${plural(counts.repairs, "unreviewed repair")}`);
  if (counts.stops) parts.push(`${plural(counts.stops, "lane")} stopped at the worktree cap`);
  if (counts.findings) parts.push(`${plural(counts.findings, "merge")} with open findings`);
  if (counts.lanes) parts.push(`${plural(counts.lanes, "lane")} still on disk after a merge`);
  const summary = parts.length ? `${parts.join(", ")}.` : "Nothing: no park, no lane stopped at the worktree cap, no open finding.";
  return [
    "## Needs you",
    "",
    summary,
    "",
    ...(lines.length ? [...lines, ""] : []),
    ...(stale.length ? ["### Already settled, clear the record", "", ...stale, ""] : []),
  ];
}

/** The current page's second section: what each lane is doing, or what the last run left. */
function nowSection(view) {
  const out = ["## Now", ""];
  if (!view.latest) return [...out, "No run has started yet.", ""];
  if (!view.running) {
    return [...out, `- No run is live. The last ended ${stamp(view.latest.ended)}: ${runFigures(view, view.runs.length - 1)}.`, ""];
  }
  const lanes = view.latest.lanes?.length ? view.latest.lanes : Object.keys(view.laneStates).sort();
  for (const lane of lanes) {
    const l = view.laneStates[lane];
    if (l?.cap) {
      out.push(`- lane ${lane}: waiting at the worktree cap (\`max_open_worktrees\` ${l.cap.max}) to start ${l.cap.plan}; the slots are held by ${l.cap.holding.join(", ")}.`);
      continue;
    }
    if (!l?.plan) {
      out.push(l?.watching ? `- lane ${lane}: idle, watching the queue.` : `- lane ${lane}: idle.`);
      continue;
    }
    const rec = view.plans.find((r) => r.plan === l.plan);
    const where = l.step ? `step \`${l.step}\` for ${duration(view.now - Date.parse(l.stepStarted))}` : "between steps";
    const waiting = l.waitingUntil ? `, waiting out the usage limit until ${stamp(l.waitingUntil)}` : "";
    out.push(`- lane ${lane}: ${l.plan}, ${where}${waiting}, ${usd(rec ? totalSpend(rec) : 0)} spent so far.`);
  }
  out.push(`- run started ${stamp(view.latest.started)}, ${duration(view.now - Date.parse(view.latest.started))} ago.`, "");
  return out;
}

/** The page written after every step: the worklist, then what is running. */
export function renderDigest(state, opts) {
  const view = readDigestState(state, opts);
  return [
    "# Conductor digest",
    "",
    "The current state, rewritten from `tools/conductor/state/` and git after every step. " +
      "Per-run history: `node tools/conductor/conductor.mjs digest --history`.",
    "",
    ...needsYou(view),
    ...nowSection(view),
  ].join("\n");
}

/** The per-run account `digest --history` writes to digest-history.md. */
export function renderHistory(state, opts) {
  const view = readDigestState(state, opts);
  const { repo, runs, plans, lockLog, ledger } = view;
  const out = [
    "# Conductor history",
    "",
    "Every run, newest first, regenerated from `tools/conductor/state/` and git by `conductor.mjs digest --history`.",
    "",
  ];
  if (runs.length === 0) out.push("No run has started yet.", "");

  for (let i = runs.length - 1; i >= 0; i--) {
    const run = runs[i];
    const inRun = (iso) => runOf(runs, iso) === i;
    out.push(`## Run ${stamp(run.started)} -> ${run.ended ? stamp(run.ended) : "running"} (lanes ${run.lanes.join(", ")})`, "");

    // Needs you
    const needs = [];
    const minorsMerged = [];
    // A run carries its own CLI reading, so the line stops appearing on the first run whose version is listed.
    if (run.cli?.warning) {
      needs.push(`- **claude ${run.cli.version} is not a verified CLI version** - the run went ahead with a warning (ADR-0208): ${run.cli.warning}.`);
    }
    for (const rec of plans) {
      for (const p of rec.parks.filter((x) => inRun(x.at))) {
        const current = rec.status === "parked" && rec.park?.at === p.at;
        const where = p.phase ? ` at Phase ${p.phase}` : "";
        const usage = parkSession(rec, p, inRun) ? stepUsage(parkSession(rec, p, inRun)).last : null;
        needs.push(
          `- **${rec.plan} parked**${where} (\`${p.reason}\`)${current ? "" : " - since resumed"}. ${p.detail}. ` +
            `Read: ${p.read ?? "the plan"}. Holds \`${p.worktree ?? "no worktree"}\`.` +
            (p.dirty ? ` Left dirty: ${dirtyText(p.dirty)}.` : "") +
            (usage ? ` Usage at park: ${usageLine(usage)}.` : ""),
        );
        if (current) needs.push(`  Resume: \`${resumeCommand(rec.plan)}\``);
      }
      if (rec.status === "merged" && inRun(rec.merge?.at)) {
        if (rec.cleanup && !rec.cleanup.ok) {
          needs.push(`- **${rec.plan} merged, lane not removed**: ${rec.cleanup.detail}. Holds \`${rec.worktree}\`.`);
        }
        const open = openFindings(rec);
        if (open.length > 0) {
          minorsMerged.push(`- **${rec.plan} merged with ${plural(open.length, "open finding")}**:`);
          for (const f of open) minorsMerged.push(`  - ${f.severity} \`${findingWhere(f)}\` ${f.what}`);
        }
      }
    }
    for (const s of run.stops ?? []) {
      needs.push(
        `- **Lane ${s.lane} stopped at the worktree cap** (\`max_open_worktrees\` ${s.max}): ${s.plan} was not opened. ` +
          `Worktrees held by ${s.holding.join(", ")}.`,
      );
    }
    // Only the newest run lists what an earlier run left parked: an older section is history.
    const standing = [];
    if (i === runs.length - 1) {
      for (const rec of plans) {
        if (rec.status !== "parked" || !rec.park?.at || runOf(runs, rec.park.at) >= i) continue;
        const p = rec.park;
        standing.push(
          `- **${rec.plan}** (\`${p.reason}\`) parked ${stamp(p.at)}, ${span(p.at, run.started)} before this run. ${p.detail}. ${parkHolds(rec)}`,
          `  Resume: \`${resumeCommand(rec.plan)}\``,
        );
      }
    }
    out.push("### Needs you", "");
    if (needs.length + minorsMerged.length + standing.length === 0) out.push("- nothing: no park, and every merge was clean.");
    else out.push(...needs, ...minorsMerged);
    out.push("");
    if (standing.length) out.push("#### Still parked from an earlier run", "", ...standing, "");

    // Not started: left out when the run opened every queued plan it could.
    if (run.notStarted?.length) {
      out.push("### Not started", "");
      for (const n of run.notStarted) out.push(`- **${n.plan}** (lane ${n.lane}): ${n.reason}`);
      out.push("");
    }

    // Closed
    out.push("### Closed", "");
    const closed = plans.filter((r) => r.status === "merged" && inRun(r.merge?.at));
    if (closed.length === 0) out.push("- none");
    for (const rec of closed) {
      const { title, rel } = planTitle(repo, rec.plan);
      const tag = rec.closed?.tag;
      const tagText = tag ? `${rec.closed.version}, tag \`${tag}\` ${tagObjectType(tag, repo) === "tag" ? "annotated" : "NOT annotated"}` : "no version, tag none";
      const { active, wall } = timeInRun(rec, run, inRun);
      out.push(
        `- **${rec.plan} - ${title}** - ${tagText}, merge \`${short(rec.merge.head)}\`${rec.merge.remerged ? " (after one re-merge)" : ""}, ` +
          `${rec.fixRounds} fix round${rec.fixRounds === 1 ? "" : "s"}, active ${duration(active)}, wall ${duration(wall)} in this run, ` +
          `${usd(spendInRun(rec, inRun))} this run, ${usd(totalSpend(rec))} total. Review: \`${rel ?? rec.plan}\` \`## Close review\`.`,
      );
      out.push(...closedFindings(rec));
    }
    out.push("");

    // Failed and parked
    out.push("### Failed and parked", "");
    const failed = [];
    for (const rec of plans) {
      for (const g of (rec.gates ?? []).filter((x) => !x.ok && inRun(x.at))) {
        const tests = g.failed.tests?.length ? ` - failing: ${g.failed.tests.join(", ")}` : "";
        failed.push(`- **${rec.plan}** gate red at \`${g.label}\`: ${g.failed.name} exited ${g.failed.code}${tests}. Log: \`${g.failed.log}\``);
      }
      for (const p of rec.parks.filter((x) => inRun(x.at) && !HUMAN_REASONS.has(x.reason) && x.reason !== "gate_red")) {
        if (p.reason === "budget") {
          const step = [...rec.steps].reverse().find((s) => s.result?.reason === "budget");
          failed.push(`- **${rec.plan}** spend cap hit in \`${step?.label ?? "a step"}\`: spent ${usd(step?.result?.spendUsd)}.`);
        } else if (p.reason === "disagreement") {
          failed.push(`- **${rec.plan}** disagreement - the session's claim and git differ: ${p.detail}`);
        } else if (API_REASONS.has(p.reason)) {
          failed.push(`- **${rec.plan}** session error (\`${p.reason}\`): ${p.detail}`);
        } else {
          failed.push(`- **${rec.plan}** \`${p.reason}\`: ${p.detail}`);
        }
      }
    }
    out.push(...(failed.length ? failed : ["- none"]), "");

    // Totals
    out.push("### Totals", "");
    const laneLines = [];
    let closeWait = 0;
    let suiteWait = 0;
    for (const lane of run.lanes) {
      const recs = plans.filter((r) => r.lane === lane);
      const merged = recs.filter((r) => r.status === "merged" && inRun(r.merge?.at)).length;
      const parked = recs.reduce((n, r) => n + r.parks.filter((p) => inRun(p.at)).length, 0);
      const spend = recs.reduce((s, r) => s + spendInRun(r, inRun), 0);
      laneLines.push(`lane ${lane}: ${merged} merged, ${parked} parked, ${usd(spend)}`);
      for (const r of recs) {
        for (const w of Array.isArray(r.lockWaits) ? r.lockWaits : []) {
          if (!inRun(w.at)) continue;
          if (w.lock === "close") closeWait += w.ms;
          else if (w.lock === "suite") suiteWait += w.ms;
        }
      }
    }
    suiteWait += lockLog.filter((e) => e.lock === "suite" && inRun(e.at)).reduce((s, e) => s + (e.waited_ms ?? 0), 0);
    out.push(`- ${laneLines.join("; ")}.`);
    out.push(`- run: ${runFigures(view, i)}. Suite-lock wait ${duration(suiteWait)}; close-lock wait ${duration(closeWait)}.`);

    const readings = plans
      .flatMap((r) => r.steps.filter((s) => inRun(s.started)))
      .sort((a, b) => a.started.localeCompare(b.started))
      .map(stepUsage);
    const firstUsage = readings.map((u) => u.first ?? u.last).find(Boolean);
    const lastUsage = [...readings].reverse().map((u) => u.last ?? u.first).find(Boolean);
    out.push(firstUsage ? `- usage at run start: ${usageLine(firstUsage)}. At run end: ${usageLine(lastUsage)}.` : "- usage: no reading in this run.");

    // The suite's three tiers are counted apart, so what ADR-0211 saved is a number here rather
    // than something inferred from the wall clock: `served` ran `-P fast` in the full suite's place.
    let suiteMs = 0;
    let otherMs = 0;
    let suiteRuns = 0;
    let servedMs = 0;
    let servedRuns = 0;
    for (const rec of plans) {
      for (const g of (rec.gates ?? []).filter((x) => inRun(x.at))) {
        for (const c of g.commands ?? []) {
          if (!isSuite(c)) otherMs += c.ms ?? 0;
          else if (c.skipped) continue;
          else if (c.served) {
            servedMs += c.ms ?? 0;
            servedRuns += 1;
          } else {
            suiteMs += c.ms ?? 0;
            suiteRuns += 1;
          }
        }
      }
    }
    // Every skip, the gate's and a session's alike, is a line in the suite ledger.
    const suiteSkips = ledger.filter((e) => e.skip && inRun(e.at)).length;
    const times = (n) => plural(n, "run");
    out.push(
      `- gate: ${duration(suiteMs + otherMs + servedMs)}; full suite ${duration(suiteMs)} over ${times(suiteRuns)}, ` +
        `served -P fast ${servedRuns ? `${duration(servedMs)} over ${times(servedRuns)}` : "none"}, ` +
        `everything else ${duration(otherMs)}; ${suiteSkips} suite ${suiteSkips === 1 ? "run" : "runs"} skipped.`,
    );
    out.push("");
  }
  return out.join("\n");
}

export function writeDigest(path, state, opts) {
  const text = renderDigest(state, opts);
  writeAtomic(path, text);
  return text;
}

export function writeHistory(path, state, opts) {
  const text = renderHistory(state, opts);
  writeAtomic(path, text);
  return text;
}
