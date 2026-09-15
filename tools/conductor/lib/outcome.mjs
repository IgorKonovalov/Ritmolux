// Reading what a headless session ended on: the stream-json `result` event (tools/conductor/spike/
// README.md is the evidence for its shape) and the `rlx-outcome` block the session prints last.
//
// Both are claims. This module only decides whether they are well-formed; checking a claim
// against git is lane.mjs's job. A missing or malformed outcome is reported as such, and the
// caller turns it into a park — never into a pass.

const PHASE_ID = /^[0-9]+[a-z]?$/;
const PLAN = /^[0-9]{4}$/;
const SEVERITIES = new Set(["blocker", "major", "minor", "nit"]);

export const IMPLEMENTER_PARK_REASONS = new Set([
  "human_phase",
  "stop_condition",
  "plan_wrong",
  "question",
  "check_red",
]);
export const REVIEW_PARK_REASONS = new Set(["merge_conflict", "check_red", "plan_wrong"]);

/**
 * The park the conductor itself gives a session whose CLI broke the headless contract (ADR-0208): it
 * made a shell call and the project hooks left no log line, or its system/init did not list the skill
 * its prompt invoked. No session may claim it in its own outcome.
 */
export const CLI_CONTRACT = "cli_contract";

const SHELL_TOOLS = new Set(["Bash", "PowerShell"]);

/**
 * Parses a stream-json transcript into the facts the conductor keeps, including the two the CLI
 * contract check reads: `init` ({ skills } from system/init, or null when there was none) and
 * `shellCalls`, the count of Bash and PowerShell tool uses.
 */
export function readResult(transcript) {
  let result = null;
  let sessionId = null;
  let rateLimit = null;
  let init = null;
  let shellCalls = 0;
  for (const line of transcript.split("\n")) {
    if (!line.trim()) continue;
    let e;
    try {
      e = JSON.parse(line);
    } catch {
      continue;
    }
    if (e.session_id && !sessionId) sessionId = e.session_id;
    if (e.type === "system" && e.subtype === "init" && !init) init = { skills: Array.isArray(e.skills) ? e.skills : null };
    if (e.type === "assistant" && Array.isArray(e.message?.content)) {
      shellCalls += e.message.content.filter((c) => c?.type === "tool_use" && SHELL_TOOLS.has(c.name)).length;
    }
    if (e.type === "rate_limit_event") rateLimit = e.rate_limit_info ?? null;
    if (e.type === "result") result = e;
  }
  if (!result) return { present: false, sessionId, rateLimit, init, shellCalls };
  return {
    init,
    shellCalls,
    present: true,
    sessionId: result.session_id ?? sessionId,
    subtype: result.subtype ?? null,
    isError: result.is_error === true,
    terminalReason: result.terminal_reason ?? null,
    spendUsd: typeof result.total_cost_usd === "number" ? result.total_cost_usd : null,
    numTurns: result.num_turns ?? null,
    text: typeof result.result === "string" ? result.result : "",
    errors: Array.isArray(result.errors) ? result.errors : [],
    permissionDenials: Array.isArray(result.permission_denials) ? result.permission_denials.length : 0,
    rateLimit,
  };
}

/** The last fenced ```rlx-outcome block in `text`, parsed and validated. */
export function parseOutcome(text) {
  const blocks = [...(text ?? "").matchAll(/```rlx-outcome[^\n]*\n([\s\S]*?)\n?```/g)];
  if (blocks.length === 0) return { ok: false, error: "no rlx-outcome block" };
  let value;
  try {
    value = JSON.parse(blocks.at(-1)[1]);
  } catch (e) {
    return { ok: false, error: `rlx-outcome is not JSON: ${e.message}` };
  }
  const error = validate(value);
  return error ? { ok: false, error } : { ok: true, outcome: value };
}

function isShaList(v) {
  return Array.isArray(v) && v.every((s) => typeof s === "string" && /^[0-9a-f]{7,40}$/.test(s));
}

/**
 * A verdict's counts and findings. Only a closed verdict's findings may carry `fixed_in`: the commit
 * in which the close repaired a minor or nit whose repair cannot change what any program does
 * (ADR-0209). Whether that commit is on the branch and touches the finding's file is close.mjs's check.
 */
function validateVerdict(v, where, { fixedIn = false } = {}) {
  if (!v || typeof v !== "object") return `${where} is not an object`;
  for (const k of ["blockers", "majors", "minors"]) {
    if (!Number.isInteger(v[k]) || v[k] < 0) return `${where}.${k} is not a count`;
  }
  if (typeof v.review_path !== "string" || !v.review_path) return `${where}.review_path missing`;
  if (!Array.isArray(v.findings)) return `${where}.findings is not a list`;
  for (const [i, f] of v.findings.entries()) {
    if (!f || !SEVERITIES.has(f.severity)) return `${where}.findings[${i}].severity invalid`;
    if (typeof f.file !== "string") return `${where}.findings[${i}].file missing`;
    if (!(f.line === null || Number.isInteger(f.line))) return `${where}.findings[${i}].line invalid`;
    if (typeof f.what !== "string" || !f.what) return `${where}.findings[${i}].what missing`;
    if (f.fixed_in !== undefined) {
      if (!fixedIn) return `${where}.findings[${i}].fixed_in is only allowed on a closed verdict`;
      if (!isShaList([f.fixed_in])) return `${where}.findings[${i}].fixed_in is not a SHA`;
      if (f.severity !== "minor" && f.severity !== "nit") return `${where}.findings[${i}].fixed_in is on a ${f.severity}`;
    }
  }
  const count = (s) => v.findings.filter((f) => f.severity === s).length;
  if (count("blocker") !== v.blockers || count("major") !== v.majors || count("minor") !== v.minors) {
    return `${where} counts disagree with its findings list`;
  }
  return null;
}

function validate(o) {
  if (!o || typeof o !== "object") return "outcome is not an object";
  if (!PLAN.test(String(o.plan ?? ""))) return "outcome.plan is not a four-digit plan number";
  switch (o.kind) {
    case "phases_done":
      if (!PHASE_ID.test(String(o.through ?? ""))) return "phases_done.through is not a phase id";
      if (!isShaList(o.commits) || o.commits.length === 0) return "phases_done.commits is not a list of SHAs";
      return null;
    case "fixed":
      if (!Number.isInteger(o.round) || o.round < 1) return "fixed.round invalid";
      if (!isShaList(o.commits) || o.commits.length === 0) return "fixed.commits is not a list of SHAs";
      if (!Array.isArray(o.resolved)) return "fixed.resolved is not a list";
      for (const [i, r] of o.resolved.entries()) {
        if (!Number.isInteger(r?.finding) || !isShaList([r?.commit])) return `fixed.resolved[${i}] invalid`;
      }
      return null;
    case "parked":
      if (!IMPLEMENTER_PARK_REASONS.has(o.reason) && !REVIEW_PARK_REASONS.has(o.reason)) {
        return `parked.reason "${o.reason}" is not a known reason`;
      }
      if (typeof o.detail !== "string" || !o.detail) return "parked.detail missing";
      return null;
    case "verdict":
      return validateVerdict(o, "verdict");
    case "closed":
      if (!(o.version === null || /^\d+\.\d+\.\d+$/.test(String(o.version)))) return "closed.version invalid";
      if (!(o.tag === null || /^v\d+\.\d+\.\d+$/.test(String(o.tag)))) return "closed.tag invalid";
      if ((o.version === null) !== (o.tag === null)) return "closed.version and closed.tag disagree";
      if (o.verdict?.blockers > 0 || o.verdict?.majors > 0) return "closed carries blockers or majors";
      return validateVerdict(o.verdict, "closed.verdict", { fixedIn: true });
    default:
      return `unknown outcome kind "${o.kind}"`;
  }
}
