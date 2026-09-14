// tools/conductor/digest.md: what happened, readable the morning after (ADR-0205).
//
// Generated from state/ and git only — never written by hand, never inside a worktree — so
// deleting it and regenerating from the same state yields the same bytes. It carries no generation
// timestamp for that reason. Newest run first; each run's section is Needs you, Closed, Failed and
// parked, Totals.
//
// A finding line copies the verdict outcome the reviewer emitted — severity, file:line, what — and
// nothing else; the digest never summarizes review prose. An event (a step, a park, a merge)
// belongs to the latest run that had started by the event's timestamp.

import { existsSync, readFileSync } from "node:fs";
import { relative } from "node:path";

import { tagObjectType } from "./git.mjs";
import { resumeCommand } from "./inbox.mjs";
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
  return `  - ${f.severity} \`${where}\` ${f.what}${resolvedIn ? ` - resolved in \`${short(resolvedIn)}\`` : ""}`;
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
    for (const rec of plans) {
      for (const p of rec.parks.filter((x) => inRun(x.at))) {
        const current = rec.status === "parked" && rec.park?.at === p.at;
        const where = p.phase ? ` at Phase ${p.phase}` : "";
        needs.push(
          `- **${rec.plan} parked**${where} (\`${p.reason}\`)${current ? "" : " - since resumed"}. ${p.detail}. ` +
            `Read: ${p.read ?? "the plan"}. Holds \`${p.worktree ?? "no worktree"}\`.`,
        );
        if (current) needs.push(`  Resume: \`${resumeCommand(rec.plan)}\``);
      }
      if (rec.status === "merged" && inRun(rec.merge?.at)) {
        if (rec.cleanup && !rec.cleanup.ok) {
          needs.push(`- **${rec.plan} merged, lane not removed**: ${rec.cleanup.detail}. Holds \`${rec.worktree}\`.`);
        }
        const minors = rec.verdicts.at(-1)?.minors ?? 0;
        if (minors > 0) minorsMerged.push(`- **${rec.plan} merged with ${minors} minor${minors === 1 ? "" : "s"}** - see Closed.`);
      }
    }
    out.push("### Needs you", "");
    if (needs.length + minorsMerged.length === 0) out.push("- nothing: no park, and every merge was clean.");
    else out.push(...needs, ...minorsMerged);
    out.push("");

    // Closed
    out.push("### Closed", "");
    const closed = plans.filter((r) => r.status === "merged" && inRun(r.merge?.at));
    if (closed.length === 0) out.push("- none");
    for (const rec of closed) {
      const { title, rel } = planTitle(repo, rec.plan);
      const tag = rec.closed?.tag;
      const tagText = tag ? `${rec.closed.version}, tag \`${tag}\` ${tagObjectType(tag, repo) === "tag" ? "annotated" : "NOT annotated"}` : "no version, tag none";
      out.push(
        `- **${rec.plan} - ${title}** - ${tagText}, merge \`${short(rec.merge.head)}\`${rec.merge.remerged ? " (after one re-merge)" : ""}, ` +
          `${rec.fixRounds} fix round${rec.fixRounds === 1 ? "" : "s"}, ${span(rec.started, rec.ended)}, ${usd(totalSpend(rec))}. ` +
          `Review: \`${rel ?? rec.plan}\` \`## Close review\`.`,
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
      const spend = recs.reduce((s, r) => s + r.steps.filter((x) => inRun(x.started)).reduce((t, x) => t + (x.result?.spendUsd ?? 0), 0), 0);
      runMerged += merged;
      runParked += parked;
      runSpend += spend;
      laneLines.push(`lane ${lane}: ${merged} merged, ${parked} parked, ${usd(spend)}`);
      for (const r of recs) {
        if (inRun(r.started) || r.steps.some((x) => inRun(x.started))) {
          closeWait += r.lockWaits?.close_ms ?? 0;
          suiteWait += r.lockWaits?.suite_ms ?? 0;
        }
      }
    }
    suiteWait += lockLog.filter((e) => e.lock === "suite" && inRun(e.at)).reduce((s, e) => s + (e.waited_ms ?? 0), 0);
    out.push(`- ${laneLines.join("; ")}.`);
    out.push(
      `- run: ${runMerged} merged, ${runParked} parked, ${run.ended ? span(run.started, run.ended) : "still running"}, ${usd(runSpend)}. ` +
        `Suite-lock wait ${duration(suiteWait)}; close-lock wait ${duration(closeWait)}.`,
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
