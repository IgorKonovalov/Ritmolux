// The run terminal: the stream reader and the gate reader turn events into lines, never throw on
// what they do not know, print ASCII only, and — end to end through `run` — show a session's commit
// while that session is still running.

import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";

import { main, paths } from "../conductor.mjs";
import {
  ascii,
  cargoCall,
  gateReader,
  liveLine,
  lockTimes,
  phaseClock,
  standingParkBody,
  stepEndBody,
  streamReader,
  testCounts,
  usageReading,
  usageText,
} from "../lib/live.mjs";
import { servedNotice } from "../lib/ledger.mjs";
import { lineReader } from "../lib/step.mjs";
import { FAKE, TEST_DIR, TOOL_DIR, tmp, writePlan } from "./helpers.mjs";

const RESETS_5H = 1789481400;
const RESETS_7D = 1790056800;

const toolUse = (id, name, command) => ({ type: "assistant", message: { content: [{ type: "tool_use", id, name, input: { command } }] } });
const toolResult = (id, content, isError = false) => ({ type: "user", message: { content: [{ type: "tool_result", tool_use_id: id, content, is_error: isError }] } });
const usage272 = (five, seven) => ({
  type: "rate_limit_event",
  rate_limit_info: { status: "allowed", resetsAt: RESETS_5H, rateLimitType: "five_hour", unifiedWindows: { five_hour: { utilization: five, resetsAt: RESETS_5H }, seven_day: { utilization: seven, resetsAt: RESETS_7D } } },
});

test("the usage reading comes out of both recorded shapes", () => {
  const unified = usageReading(usage272(0.27, 0.02).rate_limit_info);
  assert.deepEqual(unified, { status: "allowed", five: { utilization: 0.27, resetsAt: RESETS_5H }, seven: { utilization: 0.02, resetsAt: RESETS_7D } });
  assert.match(usageText(unified), /^5h 0\.27 \(resets \d\d:\d\d\); 7d 0\.02 \(resets \d\d-\d\d \d\d:\d\d\)$/);

  const topLevel = usageReading({ status: "allowed_warning", resetsAt: RESETS_7D, rateLimitType: "seven_day", utilization: 0.84 });
  assert.deepEqual(topLevel, { status: "allowed_warning", five: null, seven: { utilization: 0.84, resetsAt: RESETS_7D } });
  assert.match(usageText(topLevel), /^7d 0\.84 \(resets \d\d-\d\d \d\d:\d\d\); allowed_warning$/);

  assert.equal(usageReading({ status: "allowed" }), null);
  assert.equal(usageReading(null), null);
});

test("a usage line prints when a reading or the status changes, and not again when neither does", () => {
  const shared = {};
  const r = streamReader({ shared });
  assert.equal(r.lines(usage272(0.27, 0.02)).length, 1);
  assert.deepEqual(r.lines(usage272(0.27, 0.02)), []);
  // A second session sharing the run's state does not reprint an unchanged reading.
  assert.deepEqual(streamReader({ shared }).lines(usage272(0.27, 0.02)), []);
  assert.equal(r.lines(usage272(0.28, 0.02)).length, 1);
  const warned = usage272(0.28, 0.02);
  warned.rate_limit_info.status = "allowed_warning";
  assert.match(r.lines(warned)[0], /allowed_warning$/);
});

test("a foreground nextest call prints its start and its counts, lock wait and run time", () => {
  let t = 0;
  const r = streamReader({ now: () => t });
  const cmd = 'node "C:\\Users\\Some One\\WORK\\Ritmolux\\tools\\conductor\\with-lock.mjs" suite -- cargo nextest run -p standalone --lib';
  assert.deepEqual(r.lines(toolUse("t1", "Bash", cmd)), ["  tests  nextest run -p standalone --lib started"]);
  t = 200_000;
  const out = 'with-lock: "suite" waited 130.0s, held 62.0s\n   Compiling standalone\n\u2500\u2500\u2500\u2500\n     Summary [  60.000s] 212 tests run: 212 passed, 6 skipped\n';
  assert.deepEqual(r.lines(toolResult("t1", out)), ["  tests  nextest run -p standalone --lib: 212 passed, 0 failed, 6 skipped; lock wait 2m10s, ran 1m02s"]);
});

test("a failing nextest call names its failing tests, and a check call its exit code", () => {
  let t = 0;
  const r = streamReader({ now: () => t });
  r.lines(toolUse("t1", "PowerShell", "node tools/conductor/with-lock.mjs suite -- cargo nextest run -p standalone --test shot_cli"));
  t = 37_000;
  const out =
    "Exit code 100\n        FAIL [   7.761s] ( 6/11) standalone::shot_cli the_count_column\n     Summary [  36.656s] 6/11 tests run: 5 passed, 1 failed, 17 skipped\n        FAIL [   7.761s] ( 6/11) standalone::shot_cli the_count_column\n";
  assert.deepEqual(r.lines(toolResult("t1", out, true)), [
    "  tests  nextest run -p standalone --test shot_cli: 5 passed, 1 failed, 17 skipped - failing: standalone::shot_cli the_count_column; ran 37s",
  ]);
  r.lines(toolUse("t2", "Bash", "cargo clippy --workspace --all-targets -- -D warnings"));
  assert.deepEqual(r.lines(toolResult("t2", [{ type: "text", text: "Exit code 101\nerror: could not compile" }], true)), [
    "  check  clippy --workspace --all-targets -- failed (exit 101); ran 0s",
  ]);
  // A shell call that runs no cargo test or check prints nothing.
  assert.deepEqual(r.lines(toolUse("t3", "Bash", "git status")), []);
  assert.deepEqual(r.lines(toolResult("t3", "clean")), []);
});

test("a backgrounded run reports its end when its task notification arrives, from its output file", () => {
  let t = 0;
  const outputs = { "C:/tmp/b1.output": "     Summary [ 652.000s] 1940 tests run: 1940 passed, 6 skipped\n" };
  const r = streamReader({ now: () => t, readOutput: (p) => outputs[p] ?? null });
  r.lines(toolUse("t1", "Bash", "node tools/conductor/with-lock.mjs suite -- cargo nextest run --workspace"));
  assert.deepEqual(r.lines(toolResult("t1", "Command running in background with ID: b1. Output is being written to: C:/tmp/b1.output.")), []);
  t = 652_000;
  const notification = {
    type: "system",
    subtype: "task_notification",
    tool_use_id: "t1",
    status: "completed",
    output_file: "C:/tmp/b1.output",
    summary: 'Background command "Run the full suite" completed (exit code 0)',
  };
  assert.deepEqual(r.lines(notification), ["  tests  nextest run --workspace: 1940 passed, 0 failed, 6 skipped; ran 10m52s"]);
  assert.deepEqual(r.lines(notification), [], "a second notification for the same call prints nothing");
});

test("a foreground call's own task notification, which arrives before its result, prints nothing", () => {
  const r = streamReader();
  r.lines(toolUse("t1", "Bash", "cargo doc --workspace --no-deps"));
  assert.deepEqual(r.lines({ type: "system", subtype: "task_notification", tool_use_id: "t1", status: "completed", output_file: "", summary: "Build docs" }), []);
  assert.equal(r.lines(toolResult("t1", "Finished")).length, 1);
});

test("a permission denial names the tool and the head of its command", () => {
  const r = streamReader();
  r.lines(toolUse("t9", "PowerShell", "cd studio; npx vitest run --reporter verbose --coverage --run the whole studio suite now"));
  assert.deepEqual(r.lines({ type: "system", subtype: "permission_denied", tool_name: "PowerShell", tool_use_id: "t9" }), [
    "  denied PowerShell: cd studio; npx vitest run --reporter verbose --coverage -...",
  ]);
  assert.deepEqual(r.lines({ type: "system", subtype: "permission_denied", tool_name: "WebFetch", tool_use_id: "unknown" }), ["  denied WebFetch"]);
});

test("a malformed or unknown stream line prints nothing and does not throw", () => {
  const r = streamReader();
  for (const e of [
    null,
    42,
    "text",
    {},
    { type: "system", subtype: "thinking_tokens" },
    { type: "assistant" },
    { type: "assistant", message: { content: "not a list" } },
    { type: "assistant", message: { content: [null, { type: "tool_use" }] } },
    { type: "user", message: { content: [{ type: "tool_result", tool_use_id: "never-used" }] } },
    { type: "rate_limit_event", rate_limit_info: "garbage" },
    { type: "system", subtype: "task_notification" },
    { type: "tool_progress", tool_use_id: "x" },
  ]) {
    assert.deepEqual(r.lines(e), [], JSON.stringify(e));
  }
  // The byte-level reader drops a line that is not JSON and one split across chunks arrives whole.
  const seen = [];
  const lines = lineReader((e) => {
    seen.push(e);
    throw new Error("a display that throws");
  });
  lines.push(Buffer.from('not json\n{"type":"rate_'));
  lines.push(Buffer.from('limit_event"}\n[1,'));
  lines.end();
  assert.deepEqual(seen, [{ type: "rate_limit_event" }, ]);
});

test("every line is ASCII", () => {
  assert.equal(ascii("feat(core): the \u2014 dash \u2500\u2500 rule, \u201cquotes\u201d and \u00e9\u4e2d\ttab"), 'feat(core): the - dash -- rule, "quotes" and ?? tab');
  assert.match(ascii("\u2026"), /^[\x20-\x7e]*$/);
  // Not `ascii()` in isolation but the call on the way out: every line a plan prints goes through
  // `liveLine`, and the run terminal is a Windows console.
  assert.equal(
    liveLine("0182", "  commit 3f2a1bc feat: an \u201ceased\u201d value \u2014 arrives", new Date(2026, 8, 16, 10, 2)),
    '10:02 0182   commit 3f2a1bc feat: an "eased" value - arrives',
  );
});

// Backlog 0233: `phase N done` carried no elapsed time, so a 28-minute phase read like a 2-minute
// one. This is where the clock's semantics are pinned; the end-to-end test below only proves the
// wiring, because wall-clock timings there cannot separate "measured the gap" from "never reset".
test("a phase line measures from the previous phase line, and only a phase line moves the mark", () => {
  let t = 1000;
  const clock = phaseClock({ now: () => t });

  t = 1000 + 8 * 60000;
  assert.equal(clock.commit("3f2a1bcdeadbeef", "feat(shot): the report hears the musical clock"), "  commit 3f2a1bc 8m00s feat(shot): the report hears the musical clock");
  // The commit and the phase row it carries are found by the same poll: the phase line reads the
  // same span, and the commit did not reset the mark.
  assert.equal(clock.phase("1"), "  phase  1 done, 8m00s");

  // The next phase is measured from that line, not from the step's start: 28 minutes, not 36.
  t = 1000 + 36 * 60000;
  assert.equal(clock.phase("2"), "  phase  2 done, 28m00s");
  t = 1000 + 37 * 60000;
  assert.equal(clock.phase("3"), "  phase  3 done, 1m00s");
});

test("the step end line carries the outcome, duration, spend and turns", () => {
  assert.equal(
    stepEndBody({ label: "0182-01-implement", result: { status: "ok", outcome: { kind: "phases_done" }, spendUsd: 5.83, numTurns: 64 }, ms: 38 * 60000 }),
    "implement-01 end    phases_done, 38 min, $5.83, 64 turns",
  );
  assert.equal(stepEndBody({ label: "0175-02-review", result: { status: "parked", reason: "budget", spendUsd: 7.5 }, ms: 5000 }), "review-02 end    parked budget, < 1 min, $7.50");
});

test("a standing park names its worktree, or its branch when the worktree is gone", () => {
  const rec = { plan: "0175", branch: "plan-0175-an-eased-value-arrives", worktree: "C:/WORK/rlx-plan-0175", park: { reason: "plan_wrong", at: "2026-09-14T10:00:00.000Z" } };
  const nowMs = Date.parse("2026-09-15T10:05:00.000Z");
  assert.equal(standingParkBody(rec, { open: true, nowMs }), "still parked (plan_wrong) for 24 h 5 min; holds C:/WORK/rlx-plan-0175");
  const gone = standingParkBody(rec, { open: false, nowMs });
  assert.equal(gone, "still parked (plan_wrong) for 24 h 5 min; worktree gone; resume reopens it from branch plan-0175-an-eased-value-arrives");
  assert.ok(!gone.includes("rlx-plan-0175;") && !gone.includes("C:/WORK"), "no worktree path");
});

test("the gate prints node checks as one line and each cargo command's start and end", () => {
  const g = gateReader({ stage: "pre-review" });
  const node = { name: "check-doc-links.mjs", cmd: ["node", "scripts/check-doc-links.mjs"] };
  const clippy = { name: "cargo clippy", cmd: ["cargo", "clippy"] };
  const nextest = { name: "cargo nextest", cmd: ["cargo", "nextest", "run", "--workspace"] };
  const lines = [
    ...g.start(node),
    ...g.end(node, { code: 0, ms: 4000, output: "" }),
    ...g.end(node, { code: 0, ms: 5000, output: "" }),
    ...g.start(clippy),
    ...g.end(clippy, { code: 0, ms: 161_000, output: "" }),
    ...g.start(nextest),
    ...g.end(nextest, { code: 0, ms: 651_000, output: "     Summary [ 651.000s] 1940 tests run: 1940 passed, 6 skipped\n" }),
    ...g.finish({ ok: true }, 840_000),
  ];
  assert.deepEqual(lines, [
    "gate pre-review  checks ok (2, 9s)",
    "  gate   cargo clippy running",
    "  gate   cargo clippy ok 2m41s",
    "  gate   cargo nextest running",
    "  gate   cargo nextest ok 10m51s (1940 passed, 0 failed, 6 skipped)",
    "gate pre-review  green, 14m00s",
  ]);
  const red = gateReader({ stage: "fix-1" });
  assert.deepEqual(red.end(node, { code: 1, ms: 2000, output: "" }), ["  gate   check-doc-links.mjs FAILED (exit 1) 2s"]);
  assert.deepEqual(red.finish({ ok: false, failed: { name: "check-doc-links.mjs" } }, 2000), ["gate fix-1  red at check-doc-links.mjs, 2s"]);
});

// ADR-0211's three states are three different lines: a skip names the record and runs nothing, a
// served step names the tier and the tree it leaned on and then runs, a full run says neither.
test("the gate's suite line says which tier ran and which record it leaned on", () => {
  const g = gateReader({ stage: "post-close" });
  const nextest = { name: "cargo nextest", cmd: ["cargo", "nextest", "run", "--workspace"] };
  const notice = servedNotice({
    record: { tree: "1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b", by: "gate 0191-pre-review", at: "2026-09-16T01:00:00.000Z" },
    paths: ["docs/plans/done/0191-a.md", "docs/plans/README.md"],
  });
  assert.deepEqual(g.served(nextest, notice), [
    "  gate   cargo nextest served -P fast: tree 1a2b3c4 green by gate 0191-pre-review at 2026-09-16T01:00:00.000Z, 2 served paths",
  ]);
  assert.deepEqual(g.start(nextest), ["  gate   cargo nextest running"], "a served step still runs");
  assert.deepEqual(g.end(nextest, { code: 0, ms: 200_000, output: "     Summary [ 200.000s] 1200 tests run: 1200 passed\n" }), [
    "  gate   cargo nextest ok 3m20s (1200 passed, 0 failed)",
  ]);
  assert.deepEqual(g.skipped(nextest, "tree 1a2b3c4 green by gate 0191-pre-review at 2026-09-16T01:00:00.000Z"), [
    "  gate   cargo nextest skipped: tree 1a2b3c4 green by gate 0191-pre-review at 2026-09-16T01:00:00.000Z",
  ]);
  for (const l of [...g.served(nextest, notice), ...g.skipped(nextest, notice)]) assert.match(l, /^[\x20-\x7e]*$/, `ASCII: ${l}`);
});

test("the parsers read nextest, cargo test and the wrapper's exit line", () => {
  assert.deepEqual(testCounts("test a ... ok\ntest result: ok. 3 passed; 0 failed; 1 ignored; 0 measured\ntest b ... FAILED\ntest result: FAILED. 1 passed; 1 failed; 0 ignored"), {
    passed: 4,
    failed: 1,
    skipped: 1,
    failing: ["b"],
    summary: null,
  });
  assert.equal(testCounts("Finished"), null);
  assert.deepEqual(lockTimes('with-lock: "suite" waited 0.0s, held 12.5s'), { waitedMs: 0, heldMs: 12500 });
  assert.deepEqual(cargoCall("cd core && RUSTDOCFLAGS='-D warnings' cargo doc --no-deps"), { kind: "check", what: "doc --no-deps" });
  assert.equal(cargoCall("echo nothing"), null);
});

function sh(args, cwd) {
  const r = spawnSync("git", args, { cwd, encoding: "utf8" });
  assert.equal(r.status, 0, `git ${args.join(" ")}: ${r.stderr}`);
  return r.stdout.trim();
}

test("run prints a session's milestones in order, the commit before the session ends, and live.log holds the same lines", async () => {
  const repo = tmp("rlx-live-repo-");
  sh(["init", "-q", "-b", "main"], repo);
  for (const [k, v] of [["user.email", "t@example.invalid"], ["user.name", "T"], ["commit.gpgsign", "false"], ["tag.gpgSign", "false"], ["core.autocrlf", "false"]]) sh(["config", k, v], repo);
  writeFileSync(join(repo, "VERSION"), "0.1.0\n");
  writePlan(repo, { number: "0101", phases: [{ id: "1", owner: "dev" }, { id: "2", owner: "dev" }] });
  sh(["add", "VERSION", "docs"], repo);
  sh(["commit", "-q", "-m", "init"], repo);

  const toolDir = tmp("rlx-live-tool-");
  writeFileSync(join(toolDir, "queue.json"), JSON.stringify({ lanes: { a: ["0101"] } }));
  writeFileSync(join(toolDir, "local.json"), JSON.stringify({ budget_usd: { implement: 5, fix: 3, review: 4 }, run_budget_usd: 60, max_open_worktrees: 3 }));
  const p = { ...paths({ repo, toolDir }), settings: join(TOOL_DIR, "settings.conductor.json"), prompts: join(TOOL_DIR, "prompts"), withLock: join(TOOL_DIR, "with-lock.mjs") };
  const liveCopy = join(toolDir, "printed.log");
  writeFileSync(liveCopy, "");

  const stream = [
    toolUse("toolu_t1", "Bash", `node "${p.withLock}" suite -- cargo nextest run -p rlx-core`),
    toolResult("toolu_t1", 'with-lock: "suite" waited 1.0s, held 3.0s\n     Summary [   3.000s] 12 tests run: 12 passed, 3 skipped\n'),
    usage272(0.27, 0.02),
    "{ this line is not JSON",
    // A denied command carrying what a cargo error frame actually contains.
    toolUse("toolu_t2", "PowerShell", "cd studio; npx vitest run ── “watch”"),
    { type: "system", subtype: "permission_denied", tool_name: "PowerShell", tool_use_id: "toolu_t2" },
  ];
  const PHASE_DELAY_MS = 3000;
  writeFileSync(join(toolDir, "spec.json"), JSON.stringify({ plans: { "0101": { awaitLive: true, stream, implementCost: 5.83, numTurns: 64, phaseDelayMs: PHASE_DELAY_MS } } }));
  process.env.FAKE_CLAUDE_SCENARIO = join(TEST_DIR, "lane-scenario.mjs");
  process.env.FAKE_LANE_SPEC = join(toolDir, "spec.json");
  process.env.FAKE_EVENTS = join(toolDir, "events.jsonl");
  process.env.FAKE_CLAUDE_VERSION = "2.1.270 (Claude Code)";
  process.env.FAKE_LIVE_FILE = liveCopy;

  const out = [];
  const code = await main(["run", "--once"], {
    p,
    claude: FAKE,
    gate: [{ name: "noop", cmd: [process.execPath, "-e", "0"] }],
    // A lane path with a non-ASCII component. `conductor.mjs`'s own `emit` is the only `ascii()` a
    // line that never passes through `liveLine` meets — a lane opening prints this path — so this is
    // what arms the end-to-end assertion against that call being removed.
    worktreeRoot: tmp("rlx-live-lanes-é—"),
    lockDir: tmp("rlx-live-locks-"),
    lockPollMs: 20,
    pollMs: 50,
    commitPollMs: 40,
    signals: false,
    log: (s) => {
      out.push(s);
      writeFileSync(liveCopy, `${s}\n`, { flag: "a" });
    },
    err: () => {},
  });
  delete process.env.FAKE_LIVE_FILE;
  assert.equal(code, 0);

  const sha = sh(["log", "--format=%H", "-1", "--grep", "phase 1", "main"], repo).slice(0, 7);
  const find = (re) => out.findIndex((l) => re.test(l));
  const order = [
    find(/^\d\d:\d\d 0101 implement-01 start  phases 1-2 \(dev\)$/),
    // The subject's em dash, curly quotes and box-drawing character reach the line as ASCII: the
    // assertion below is what would go red if either `ascii()` call were removed. The sha is still
    // matched exactly, so the ordering this list pins is unweakened.
    find(new RegExp(`^\\d\\d:\\d\\d 0101   commit ${sha} \\d+[ms]\\S* feat: plan 0101 phase 1 - an "eased" value - arrives$`)),
    find(/^\d\d:\d\d 0101   phase  1 done, \d+[ms]\S*$/),
    find(/^\d\d:\d\d 0101   tests  nextest run -p rlx-core: 12 passed, 0 failed, 3 skipped; lock wait 1s, ran 3s$/),
    find(/^\d\d:\d\d 0101   usage  5h 0\.27 \(resets \d\d:\d\d\); 7d 0\.02 \(resets \d\d-\d\d \d\d:\d\d\)$/),
    find(/^\d\d:\d\d 0101   denied PowerShell: cd studio; npx vitest run -- "watch"$/),
    find(/^\d\d:\d\d 0101 implement-01 end    phases_done, (< 1|\d+) min, \$5\.83, 64 turns$/),
  ];
  assert.ok(order.every((i) => i >= 0), `every milestone printed:\n${out.join("\n")}`);
  assert.deepEqual([...order].sort((a, b) => a - b), order, `in order:\n${out.join("\n")}`);
  assert.equal(new Set(out.filter((l) => /\bcommit\b/.test(l) && l.includes(sha))).size, 1, "the commit is printed once");

  // The phase lines carry real durations, and the phase the fake slept through says so. What the
  // clock measures is pinned above; this is the wiring.
  const seconds = (id) => {
    const line = out.find((l) => new RegExp(`  phase  ${id} done, `).test(l));
    assert.ok(line, `phase ${id} printed:\n${out.join("\n")}`);
    const m = line.match(/done, (?:(\d+)m)?(\d+)s$/);
    assert.ok(m, `a duration on: ${line}`);
    return Number(m[1] ?? 0) * 60 + Number(m[2]);
  };
  assert.ok(seconds("2") >= PHASE_DELAY_MS / 1000, `phase 2 took at least the sleep, got ${seconds("2")}s`);
  assert.ok(seconds("1") < seconds("2"), `phase 1 (${seconds("1")}s) is the shorter of the two`);
  for (const l of out) assert.match(l, /^[\x20-\x7e]*$/, `ASCII: ${l}`);

  // live.log holds exactly the lines run printed while the lanes ran, under a run header.
  const log = readFileSync(join(p.stateDir, "live.log"), "utf8").split("\n").filter(Boolean);
  assert.match(log[0], /^== run \d{4}-\d\d-\d\d \d\d:\d\d UTC, lanes a ==$/);
  const printed = out.filter((l) => !l.startsWith("conductor: run ended") && !l.startsWith("digest: "));
  assert.deepEqual(log.slice(1), printed);
});
