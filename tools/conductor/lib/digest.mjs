// tools/conductor/digest.md: what happened, readable the morning after (ADR-0205).
//
// Generated from state/ and git only — never written by hand, never inside a worktree — so
// deleting it and regenerating from the same state yields the same bytes. It carries no generation
// timestamp for that reason. Newest run first; each run's section is Needs you, Still parked from
// an earlier run (the newest run only, and only when one is), Not started (only when a queued plan
// was not opened), Closed, Failed and parked, Totals. Time is always time within one run: a plan's
// active time is its steps' and gates' own durations, never a span across a park.
//
// A finding line copies the verdict outcome the reviewer emitted — severity, file:line, what — and
// nothing else; the digest never summarizes review prose. An event (a step, a park, a merge)
// belongs to the latest run that had started by the event's timestamp.

import { existsSync, readFileSync } from "node:fs";
import { join, relative } from "node:path";

import { tagObjectType } from "./git.mjs";
import { dirtyText, resumeCommand } from "./inbox.mjs";
import { laneOpen } from "./lane.mjs";
import { readLedger } from "./ledger.mjs";
import { usageReading } from "./live.mjs";
import { findPlan, readPlanFile } from "./plan.mjs";
import { statePaths, totalSpend, writeAtomic } from "./state.mjs";

const HUMAN_REASONS = new Set(["human_phase", "stop_condition", "question", "plan_wrong"]);
const API_REASONS = new Set(["api", "no_outcome", "bad_outcome"]);

const stamp = (iso) => (iso ? iso.slice(0, 16).replace("T", " ") : "?");
const usd = (n) => `$${(n ?? 0).toFixed(2)}`;
const short = (sha) => (sha ? sha.slice(0, 7) : "?");

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
  const where = f.line ? `${f.file}:${f.line}` : f.file;
  const fixed = resolvedIn ? ` - resolved in \`${short(resolvedIn)}\`` : f.fixed_in ? ` - repaired by the close in \`${short(f.fixed_in)}\`` : "";
  return `  - ${f.severity} \`${where}\` ${f.what}${fixed}`;
}

/** The minors and nits the closing verdict merged with that its close did not repair (ADR-0209). */
function openFindings(rec) {
  return (rec.verdicts.at(-1)?.findings ?? []).filter((f) => (f.severity === "minor" || f.severity === "nit") && !f.fixed_in);
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

export function renderDigest(state, { repo, stateDir }) {
  const runs = state.runs ?? [];
  const lockLog = readLockLog(stateDir);
  const ledger = readLedger(join(stateDir, "suite-ledger.jsonl"));
  const plans = Object.values(state.plans).sort((a, b) => a.plan.localeCompare(b.plan));
  const out = ["# Conductor digest", "", "Generated from `tools/conductor/state/` and git after every step. Newest run first.", ""];
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
          minorsMerged.push(`- **${rec.plan} merged with ${open.length} open finding${open.length === 1 ? "" : "s"}**:`);
          for (const f of open) minorsMerged.push(`  - ${f.severity} \`${f.line ? `${f.file}:${f.line}` : f.file}\` ${f.what}`);
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
        const holds = laneOpen(rec) ? `Holds \`${rec.worktree}\`.` : `Worktree removed; \`resume\` reopens it from branch \`${rec.branch ?? "?"}\`.`;
        standing.push(
          `- **${rec.plan}** (\`${p.reason}\`) parked ${stamp(p.at)}, ${span(p.at, run.started)} before this run. ${p.detail}. ${holds}`,
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
    let runMerged = 0;
    let runParked = 0;
    let runSpend = 0;
    let closeWait = 0;
    let suiteWait = 0;
    for (const lane of run.lanes) {
      const recs = plans.filter((r) => r.lane === lane);
      const merged = recs.filter((r) => r.status === "merged" && inRun(r.merge?.at)).length;
      const parked = recs.reduce((n, r) => n + r.parks.filter((p) => inRun(p.at)).length, 0);
      const spend = recs.reduce((s, r) => s + spendInRun(r, inRun), 0);
      runMerged += merged;
      runParked += parked;
      runSpend += spend;
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
    out.push(
      `- run: ${runMerged} merged, ${runParked} parked, ${run.ended ? span(run.started, run.ended) : "still running"}, ${usd(runSpend)}. ` +
        `Suite-lock wait ${duration(suiteWait)}; close-lock wait ${duration(closeWait)}.`,
    );

    const readings = plans
      .flatMap((r) => r.steps.filter((s) => inRun(s.started)))
      .sort((a, b) => a.started.localeCompare(b.started))
      .map(stepUsage);
    const firstUsage = readings.map((u) => u.first ?? u.last).find(Boolean);
    const lastUsage = [...readings].reverse().map((u) => u.last ?? u.first).find(Boolean);
    out.push(firstUsage ? `- usage at run start: ${usageLine(firstUsage)}. At run end: ${usageLine(lastUsage)}.` : "- usage: no reading in this run.");

    let suiteMs = 0;
    let otherMs = 0;
    let suiteRuns = 0;
    for (const rec of plans) {
      for (const g of (rec.gates ?? []).filter((x) => inRun(x.at))) {
        for (const c of g.commands ?? []) {
          if (!isSuite(c)) otherMs += c.ms ?? 0;
          else if (!c.skipped) {
            suiteMs += c.ms ?? 0;
            suiteRuns += 1;
          }
        }
      }
    }
    // Every skip, the gate's and a session's alike, is a line in the suite ledger.
    const suiteSkips = ledger.filter((e) => e.skip && inRun(e.at)).length;
    out.push(
      `- gate: ${duration(suiteMs + otherMs)}; full suite ${duration(suiteMs)} over ${suiteRuns} run${suiteRuns === 1 ? "" : "s"}, ` +
        `everything else ${duration(otherMs)}; ${suiteSkips} suite run${suiteSkips === 1 ? "" : "s"} skipped.`,
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
