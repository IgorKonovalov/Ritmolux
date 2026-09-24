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

/**
 * The park for a session that started a command in the background and reached its result with that
 * command still unfinished. Nothing re-invokes a `claude -p` session, so the process exits, the task
 * is killed and the work is lost — after whatever the session already committed has landed. No
 * session may claim this reason in its own outcome.
 */
export const LOST_BACKGROUND = "lost_background";

/**
 * The park for a phase whose declared files include a path under `.claude/`. The CLI refuses a
 * headless session an `Edit` or `Write` there whatever the allowlist says, so the phase is the
 * owner's (ADR-0210); the lane parks in front of it with the edit as the detail rather than running
 * it and failing a done-when it was never able to satisfy. No session may claim this reason.
 */
export const CLAUDE_DIR = "claude_dir";

/**
 * The park for a lane whose plan declares files under `studio/` and whose dependency install failed.
 * `studio/node_modules` is gitignored and `git worktree add` never creates one, so without the
 * install the three studio checks skip and the lane's gate is silent about exactly the work the plan
 * is doing (ADR-0218). Proceeding with checks that cannot run is what the park replaces, and it
 * happens before any session starts. The trigger is the ABSENCE of `studio/node_modules`, asked
 * before every run rather than only at open, so the worktree this park leaves behind is installed
 * into on the next one. No session may claim this reason.
 */
export const STUDIO_INSTALL = "studio_install";

const SHELL_TOOLS = new Set(["Bash", "PowerShell"]);

/** One line, at most 80 characters, for naming a command in a park detail. */
const commandHead = (text) => {
  const one = String(text ?? "").replace(/\s+/g, " ").trim();
  return one.length > 80 ? `${one.slice(0, 77)}...` : one;
};

function resultText(content) {
  if (typeof content === "string") return content;
  if (Array.isArray(content)) return content.map((c) => (typeof c?.text === "string" ? c.text : "")).join("\n");
  return "";
}

/**
 * The two shapes the CLI reports a started background command in. `live.mjs` recognises the same two
 * — they are text the CLI owns, not a typed field, so a reworded message stops both seeing a start,
 * which is why the hook and the prompts are the other two layers.
 */
const BACKGROUND_STARTED = [/^Command running in background with ID: /, /moved to the background \(ID: /];

/**
 * Parses a stream-json transcript into the facts the conductor keeps: the two the CLI contract check
 * reads — `init` ({ skills } from system/init, or null when there was none) and `shellCalls`, the
 * count of Bash and PowerShell tool uses — and `backgroundOutstanding`, the background commands that
 * were started and never finished.
 *
 * A start is a shell `tool_use` carrying `run_in_background`, or a `tool_result` in either shape
 * above. It is cancelled by a task notification, a permission denial (the hook refusing it) or an
 * error result for the same `tool_use_id`: none of those left a command running.
 */
export function readResult(transcript) {
  let result = null;
  let sessionId = null;
  let rateLimit = null;
  let init = null;
  let shellCalls = 0;
  const commands = new Map();
  const background = new Map();
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
    if ((e.type === "assistant" || e.type === "user") && Array.isArray(e.message?.content)) {
      for (const c of e.message.content) {
        if (c?.type === "tool_use" && typeof c.id === "string") {
          if (!SHELL_TOOLS.has(c.name)) continue;
          shellCalls += 1;
          commands.set(c.id, commandHead(c.input?.command));
          if (c.input?.run_in_background === true) background.set(c.id, commands.get(c.id));
        } else if (c?.type === "tool_result" && typeof c.tool_use_id === "string") {
          if (c.is_error) background.delete(c.tool_use_id);
          else if (BACKGROUND_STARTED.some((re) => re.test(resultText(c.content)))) {
            background.set(c.tool_use_id, commands.get(c.tool_use_id) ?? "");
          }
        }
      }
    }
    if (e.type === "system" && (e.subtype === "task_notification" || e.subtype === "permission_denied")) {
      background.delete(e.tool_use_id);
    }
    if (e.type === "rate_limit_event") rateLimit = e.rate_limit_info ?? null;
    if (e.type === "result") result = e;
  }
  const backgroundOutstanding = [...background].map(([id, command]) => ({ id, command }));
  if (!result) return { present: false, sessionId, rateLimit, init, shellCalls, backgroundOutstanding };
  return {
    init,
    shellCalls,
    backgroundOutstanding,
    present: true,
    sessionId: result.session_id ?? sessionId,
    subtype: result.subtype ?? null,
    isError: result.is_error === true,
    apiErrorStatus: typeof result.api_error_status === "number" ? result.api_error_status : null,
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
