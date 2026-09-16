// The run terminal: one line per milestone as it happens, the same lines appended to state/live.log.
//
// Everything here is pure: it turns a stream-json event, a gate command or a plan record into the
// text of a line, and does no I/O of its own. A line is `HH:MM NNNN <body>`, where a body starting
// with two spaces is a milestone inside a step or a gate. Every line is ASCII, because the run
// terminal is a Windows console.
//
// The stream is the CLI's, not the conductor's: the reader knows the event kinds observed on the
// verified CLI (tool_use in `assistant`, tool_result in `user`, system/task_notification,
// system/permission_denied, rate_limit_event) and prints nothing for anything else. A renamed event
// drops a line and never throws, which is why this display is never evidence for anything.

const pad = (n) => String(n).padStart(2, "0");

/** `HH:MM` in local time. */
export function clock(date) {
  return `${pad(date.getHours())}:${pad(date.getMinutes())}`;
}

/** Any character a Windows console may not render becomes ASCII: dashes and rules to `-`, the rest to `?`. */
export function ascii(text) {
  return String(text)
    .replace(/\t/g, " ")
    .replace(/[‐-―−─-╿]/g, "-")
    .replace(/[‘’]/g, "'")
    .replace(/[“”]/g, '"')
    .replace(/…/g, "...")
    .replace(/[^\x20-\x7e]/g, "?");
}

export function liveLine(plan, body, date = new Date()) {
  return ascii(`${clock(date)} ${plan} ${body}`);
}

/** `9s`, `2m41s`, `1h02m`: a command's duration. */
export function shortDuration(ms) {
  if (!(ms >= 0)) return "?";
  const s = Math.round(ms / 1000);
  if (s < 60) return `${s}s`;
  if (s < 3600) return `${Math.floor(s / 60)}m${pad(s % 60)}s`;
  return `${Math.floor(s / 3600)}h${pad(Math.floor(s / 60) % 60)}m`;
}

/** `< 1 min`, `38 min`, `1 h 52 min`: a step's duration. */
export function minutes(ms) {
  if (!(ms >= 0)) return "?";
  const min = Math.round(ms / 60000);
  if (min < 1) return "< 1 min";
  const h = Math.floor(min / 60);
  return h ? `${h} h ${min % 60} min` : `${min} min`;
}

const head = (text, max = 60) => {
  const one = String(text ?? "").replace(/\s+/g, " ").trim();
  return one.length > max ? `${one.slice(0, max - 3)}...` : one;
};

// ---------------------------------------------------------------------------------------------
// Usage windows

/**
 * The 5-hour and 7-day windows of a rate_limit_event's `rate_limit_info`, in either recorded shape:
 * `unifiedWindows.{five_hour,seven_day}` (2.1.272), or the one window a top-level `rateLimitType` +
 * `utilization` names (2.1.270). `resetsAt` is epoch seconds. Null when neither shape is present.
 */
export function usageReading(info) {
  if (!info || typeof info !== "object") return null;
  const win = (w) => (w && typeof w.utilization === "number" ? { utilization: w.utilization, resetsAt: w.resetsAt ?? null } : null);
  let five = win(info.unifiedWindows?.five_hour);
  let seven = win(info.unifiedWindows?.seven_day);
  if (!five && !seven && typeof info.utilization === "number") {
    const one = { utilization: info.utilization, resetsAt: info.resetsAt ?? null };
    if (info.rateLimitType === "five_hour") five = one;
    else if (info.rateLimitType === "seven_day") seven = one;
  }
  if (!five && !seven) return null;
  return { status: typeof info.status === "string" ? info.status : null, five, seven };
}

function resets(epochSeconds, withDate) {
  if (typeof epochSeconds !== "number") return "?";
  const d = new Date(epochSeconds * 1000);
  return withDate ? `${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${clock(d)}` : clock(d);
}

/** `5h 0.27 (resets 14:30); 7d 0.02 (resets 09-22 16:00)`, plus the status when it is not `allowed`. */
export function usageText(reading) {
  const parts = [];
  if (reading.five) parts.push(`5h ${reading.five.utilization.toFixed(2)} (resets ${resets(reading.five.resetsAt, false)})`);
  if (reading.seven) parts.push(`7d ${reading.seven.utilization.toFixed(2)} (resets ${resets(reading.seven.resetsAt, true)})`);
  if (reading.status && reading.status !== "allowed") parts.push(reading.status);
  return parts.join("; ");
}

const usageKey = (r) => `${r.status}|${r.five?.utilization}|${r.seven?.utilization}`;

// ---------------------------------------------------------------------------------------------
// Test and check output

/**
 * nextest's `Summary` line, or the sum of cargo test's `test result:` lines:
 * { passed, failed, skipped, failing: [names] } or null when the output carries neither.
 */
export function testCounts(output) {
  const text = String(output ?? "");
  const failing = new Set();
  for (const m of text.matchAll(/^\s*(?:FAIL|TIMEOUT|SIGSEGV|SIGABRT) \[[^\]]*\]\s+(?:\(\s*\d+\/\d+\)\s+)?(.+?)\s*$/gm)) failing.add(m[1]);
  for (const m of text.matchAll(/^test (\S+) \.\.\. FAILED\s*$/gm)) failing.add(m[1]);
  const summary = [...text.matchAll(/^\s*Summary \[[^\]]*\]\s+(.+?)\s*$/gm)].at(-1);
  if (summary) {
    const counts = { passed: 0, failed: 0, skipped: 0, failing: [...failing], summary: summary[1] };
    const tail = summary[1].split("run:")[1] ?? "";
    for (const m of tail.matchAll(/(\d+) (passed|failed|skipped|timed out)/g)) {
      const key = m[2] === "timed out" ? "failed" : m[2];
      counts[key] += Number(m[1]);
    }
    return counts;
  }
  const results = [...text.matchAll(/^test result: \w+\. (\d+) passed; (\d+) failed; (\d+) ignored/gm)];
  if (results.length === 0) return null;
  const counts = { passed: 0, failed: 0, skipped: 0, failing: [...failing], summary: null };
  for (const m of results) {
    counts.passed += Number(m[1]);
    counts.failed += Number(m[2]);
    counts.skipped += Number(m[3]);
  }
  return counts;
}

/** The wrapper's exit line, `with-lock: "suite" waited 1.5s, held 3.0s`, as { waitedMs, heldMs }. */
export function lockTimes(output) {
  const m = [...String(output ?? "").matchAll(/with-lock: "[^"]+" waited ([\d.]+)s, held ([\d.]+)s/g)].at(-1);
  return m ? { waitedMs: Number(m[1]) * 1000, heldMs: Number(m[2]) * 1000 } : null;
}

/** The cargo test or check a shell command runs: { kind: "tests"|"check", what } or null. */
export function cargoCall(command) {
  if (typeof command !== "string") return null;
  for (const seg of command.split(/&&|\|\||;|\||\n/)) {
    const m = seg.match(/\bcargo(?:\.exe)?(?:\s+\+\S+)?(?:\s+llvm-cov)?\s+(nextest|test|clippy|doc)\b(.*)$/);
    if (!m) continue;
    const what = head(`${m[1]}${m[2]}`.replace(/\s+-D\s+warnings\b/, ""), 50);
    return { kind: m[1] === "nextest" || m[1] === "test" ? "tests" : "check", what };
  }
  return null;
}

function resultText(content) {
  if (typeof content === "string") return content;
  if (Array.isArray(content)) return content.map((c) => (typeof c?.text === "string" ? c.text : "")).join("\n");
  return "";
}

function countsText(c) {
  const failing = c.failing.length ? ` - failing: ${c.failing.slice(0, 3).join(", ")}${c.failing.length > 3 ? ` and ${c.failing.length - 3} more` : ""}` : "";
  return `${c.passed} passed, ${c.failed} failed${c.skipped ? `, ${c.skipped} skipped` : ""}${failing}`;
}

/** The body of a finished test or check call. */
function callEndBody(call, { ok, code, output, elapsedMs }) {
  const skip = String(output ?? "").match(/with-lock: skipped [^:]+: (tree \S+) is green in the suite ledger, run by (.+?) at (\S+):/);
  if (skip) return `  tests  ${call.what} skipped: ${skip[1]} green by ${skip[2]} at ${skip[3]}`;
  const lock = lockTimes(output);
  const ran = shortDuration(lock ? lock.heldMs : elapsedMs);
  const wait = lock ? `lock wait ${shortDuration(lock.waitedMs)}, ` : "";
  const counts = call.kind === "tests" ? testCounts(output) : null;
  const tag = call.kind === "tests" ? "tests " : "check ";
  if (counts) return `  ${tag} ${call.what}: ${countsText(counts)}; ${wait}ran ${ran}`;
  const result = ok ? "ok" : `failed${code != null ? ` (exit ${code})` : ""}`;
  return `  ${tag} ${call.what} ${result}; ${wait}ran ${ran}`;
}

// ---------------------------------------------------------------------------------------------
// The stream reader

/**
 * A reader for one session's stream. `lines(event)` returns the bodies that event produces, possibly
 * none; it never throws. `now` is the clock the elapsed times use; `readOutput(path)` returns a
 * backgrounded command's output file (or null), the one read this module leaves to its caller;
 * `shared` carries the last printed usage across sessions, so an unchanged reading is not reprinted.
 */
export function streamReader({ now = () => Date.now(), readOutput = () => null, shared = {} } = {}) {
  const tools = new Map();

  function onToolUse(c) {
    const command = c.input?.command;
    const call = cargoCall(command);
    tools.set(c.id, { name: c.name, input: c.input ?? {}, call, started: now(), background: false, ended: false });
    return call ? [`  ${call.kind === "tests" ? "tests " : "check "} ${call.what} started`] : [];
  }

  function onToolResult(c) {
    const t = tools.get(c.tool_use_id);
    if (!t?.call || t.ended) return [];
    const text = resultText(c.content);
    if (/^Command running in background with ID: /.test(text) || /moved to the background \(ID: /.test(text)) {
      t.background = true;
      return [];
    }
    t.ended = true;
    const code = c.is_error ? Number(text.match(/^Exit code (\d+)/)?.[1] ?? NaN) : 0;
    return [callEndBody(t.call, { ok: !c.is_error, code: Number.isNaN(code) ? null : code, output: text, elapsedMs: now() - t.started })];
  }

  function onTaskNotification(e) {
    const t = tools.get(e.tool_use_id);
    // A foreground call's notification arrives before its tool_result, which is what ends it.
    if (!t?.call || !t.background || t.ended) return [];
    t.ended = true;
    const code = e.summary?.match(/\(exit code (\d+)\)/)?.[1];
    let output = "";
    if (e.output_file) {
      try {
        output = readOutput(e.output_file) ?? "";
      } catch {
        output = "";
      }
    }
    return [
      callEndBody(t.call, {
        ok: e.status === "completed" && (code === undefined || code === "0"),
        code: code === undefined ? null : Number(code),
        output,
        elapsedMs: now() - t.started,
      }),
    ];
  }

  function onUsage(info) {
    const reading = usageReading(info);
    if (!reading) return [];
    const key = usageKey(reading);
    if (shared.usageKey === key) return [];
    shared.usageKey = key;
    return [`  usage  ${usageText(reading)}`];
  }

  function onDenied(e) {
    const t = tools.get(e.tool_use_id);
    const input = t?.input ?? {};
    const what = head(input.command ?? input.file_path ?? input.pattern ?? "");
    return [`  denied ${e.tool_name ?? t?.name ?? "a tool"}${what ? `: ${what}` : ""}`];
  }

  function read(e) {
    if (!e || typeof e !== "object") return [];
    if (e.type === "assistant" || e.type === "user") {
      const content = e.message?.content;
      if (!Array.isArray(content)) return [];
      const out = [];
      for (const c of content) {
        if (c?.type === "tool_use" && typeof c.id === "string") out.push(...onToolUse(c));
        else if (c?.type === "tool_result") out.push(...onToolResult(c));
      }
      return out;
    }
    if (e.type === "rate_limit_event") return onUsage(e.rate_limit_info);
    if (e.type === "system" && e.subtype === "task_notification") return onTaskNotification(e);
    if (e.type === "system" && e.subtype === "permission_denied") return onDenied(e);
    return [];
  }

  return {
    lines(event) {
      try {
        return read(event);
      } catch {
        return [];
      }
    },
  };
}

// ---------------------------------------------------------------------------------------------
// Steps, commits, phases, parks

/** `implement-01`: a step label `NNNN-01-implement` as the run terminal names it. */
export function stepName(label) {
  const m = String(label).match(/^\d{4}-(\d+)-(\w+)$/);
  return m ? `${m[2]}-${m[1]}` : String(label);
}

export function stepStartBody({ label, kind, owner, phases, round }) {
  const scope = kind === "implement" && phases?.length ? `phases ${phases.length === 1 ? phases[0] : `${phases[0]}-${phases.at(-1)}`}` : round ? `round ${round}` : kind;
  return `${stepName(label)} start  ${scope} (${owner})`;
}

export function stepEndBody({ label, result, ms }) {
  const kind = result.status === "ok" ? result.outcome?.kind ?? "ok" : `parked ${result.reason}`;
  const turns = result.numTurns != null ? `, ${result.numTurns} turn${result.numTurns === 1 ? "" : "s"}` : "";
  return `${stepName(label)} end    ${kind}, ${minutes(ms)}, $${(result.spendUsd ?? 0).toFixed(2)}${turns}`;
}

// A commit line carries its duration after the sha rather than at the end, because the subject is
// variable-length and truncated: a trailing figure would read as part of the message.
export function commitBody(sha, subject, ms) {
  return `  commit ${sha.slice(0, 7)} ${shortDuration(ms)} ${head(subject, 90)}`;
}

export function phaseBody(id, ms) {
  return `  phase  ${id} done, ${shortDuration(ms)}`;
}

/**
 * The clock the commit and phase lines of one step read: how long since the previous phase line, or
 * since the step started before the first. Without it a 28-minute phase and a 2-minute one print the
 * same line, and the only way to tell them apart is subtracting the timestamps by hand.
 *
 * Only a phase line moves the mark. A commit and the phase row it carries are found by the same poll,
 * so resetting on the commit would make every phase line read as no time at all.
 */
export function phaseClock({ now = () => Date.now() } = {}) {
  let since = now();
  return {
    commit: (sha, subject) => commitBody(sha, subject, now() - since),
    phase(id) {
      const body = phaseBody(id, now() - since);
      since = now();
      return body;
    },
  };
}

/** A plan still parked when a run starts: its age, and the worktree it holds or the branch `resume` reopens. */
export function standingParkBody(rec, { open, nowMs }) {
  const age = rec.park?.at ? minutes(nowMs - Date.parse(rec.park.at)) : "?";
  const where = open ? `holds ${rec.worktree}` : `worktree gone; resume reopens it from branch ${rec.branch ?? "?"}`;
  return `still parked (${rec.park?.reason ?? "?"}) for ${age}; ${where}`;
}

// ---------------------------------------------------------------------------------------------
// The gate

const isCargo = (c) => c.cmd?.[0] === "cargo";

/**
 * A reader for one gate run. The cargo commands get a line as each starts and ends; the node,
 * studio and sd-filter checks before them print one aggregate line, or one line per failure.
 */
export function gateReader({ stage }) {
  let checks = 0;
  let checksMs = 0;
  let checksFailed = false;
  const flush = () => {
    if (checks === 0 || checksFailed) return [];
    const body = `gate ${stage}  checks ok (${checks}, ${shortDuration(checksMs)})`;
    checks = 0;
    checksMs = 0;
    return [body];
  };
  return {
    start(c) {
      if (!isCargo(c)) return [];
      return [...flush(), `  gate   ${c.name} running`];
    },
    skipped(c, why) {
      return [...(isCargo(c) ? flush() : []), `  gate   ${c.name} skipped: ${why}`];
    },
    // A served step does run, so this line precedes its own `running` line rather than replacing it:
    // it says which tier is about to run and which tree's record bought the narrowing (ADR-0211).
    served(c, why) {
      return [...(isCargo(c) ? flush() : []), `  gate   ${c.name} served -P fast: ${why}`];
    },
    end(c, { code, ms, output }) {
      if (!isCargo(c)) {
        checks += 1;
        checksMs += ms;
        if (code === 0) return [];
        checksFailed = true;
        return [`  gate   ${c.name} FAILED (exit ${code}) ${shortDuration(ms)}`];
      }
      const counts = c.name === "cargo nextest" ? testCounts(output) : null;
      const detail = counts ? ` (${countsText(counts)})` : "";
      return [`  gate   ${c.name} ${code === 0 ? "ok" : `FAILED (exit ${code})`} ${shortDuration(ms)}${detail}`];
    },
    finish(g, ms) {
      return [...flush(), `gate ${stage}  ${g.ok ? "green" : `red at ${g.failed.name}`}, ${shortDuration(ms)}`];
    },
  };
}
