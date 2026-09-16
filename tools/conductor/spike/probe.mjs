#!/usr/bin/env node
// Plan 0187 Phase 1 probe: runs real headless `claude -p` sessions against a disposable
// worktree and records what the installed CLI does, so the conductor is built on observed
// behaviour. Spends model usage: four short sessions on the model named by --model.
//
//   node tools/conductor/spike/probe.mjs [--model haiku] [--sessions a,b,c,d]
//   node tools/conductor/spike/probe.mjs --analyze <out-dir>     (re-read a run, no spend)
//
// Session A is the clean run: `/dev implement plan 9999` with an appended system prompt that
// steers the session through a fixed list of tool calls (a hook-denied one, an allowed one, a
// permission-denied one, Write + Edit). Session B repeats the prompt under a budget far below one
// turn's cost, to observe how a budget stop ends. Session C asks whether a headless session may
// Read, Edit and Write under the project's `.claude/`, under the conductor's own settings file, with
// a control write outside it in the same turn; Session D asks the same under settings that name
// `.claude/` paths explicitly, which is what tells a CLI restriction from a missing grant. After all
// four exit, the worktree is removed from the parent to observe whether the children left a handle
// on it.
//
// The script is also its own PreToolUse hook (`--hook-env <file>`): the settings file it passes
// registers `node probe.mjs --hook-env ...`, which appends the environment the hook saw to a file.
// Output lands under target/conductor-spike/<stamp>/ and is never committed.

import { spawn, spawnSync } from "node:child_process";
import { randomBytes } from "node:crypto";
import {
  appendFileSync,
  copyFileSync,
  existsSync,
  mkdirSync,
  readFileSync,
  writeFileSync,
} from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const HERE = dirname(fileURLToPath(import.meta.url));
const REPO = resolve(HERE, "..", "..", "..");
const MARKER = "kestrel-4417";

const argv = process.argv.slice(2);
const flag = (name, fallback) => {
  const i = argv.indexOf(name);
  return i >= 0 ? argv[i + 1] : fallback;
};

if (argv[0] === "--hook-env") {
  // Hook mode: record what the hook process sees, then pass the tool call through untouched.
  const input = JSON.parse(readFileSync(0, "utf8") || "{}");
  appendFileSync(
    argv[1],
    JSON.stringify({
      RLX_CONDUCTOR: process.env.RLX_CONDUCTOR ?? null,
      RLX_PROBE_TOKEN: process.env.RLX_PROBE_TOKEN ?? null,
      tool_name: input.tool_name ?? null,
      command: input.tool_input?.command ?? null,
    }) + "\n",
  );
  process.stdout.write("{}");
  process.exit(0);
}

if (argv[0] === "--analyze") {
  const dir = resolve(argv[1]);
  const summary = analyzeRun(dir);
  writeFileSync(join(dir, "summary.json"), JSON.stringify(summary, null, 2));
  console.log(JSON.stringify(summary, null, 2));
  process.exit(0);
}

const model = flag("--model", "haiku");
const sessions = flag("--sessions", "a,b,c,d").split(",");
const stamp = new Date().toISOString().replace(/[:.]/g, "-");
const out = join(REPO, "target", "conductor-spike", stamp);
mkdirSync(out, { recursive: true });

const worktree = resolve(REPO, "..", "rlx-probe-0187");
const branch = `probe-0187-${stamp.slice(0, 19).toLowerCase()}`;
const token = randomBytes(6).toString("hex");

const git = (args, cwd = REPO) => {
  const r = spawnSync("git", args, { cwd, encoding: "utf8" });
  return { code: r.status, stdout: r.stdout.trim(), stderr: r.stderr.trim() };
};

if (existsSync(worktree)) {
  console.error(`refusing: ${worktree} already exists (a previous probe left it; remove it first)`);
  process.exit(2);
}
const add = git(["worktree", "add", "-b", branch, worktree, "HEAD"]);
writeFileSync(join(out, "worktree-add.json"), JSON.stringify(add, null, 2));
if (add.code !== 0) {
  console.error(add.stderr);
  process.exit(2);
}
copyFileSync(join(HERE, "fixture-plan.md"), join(worktree, "docs", "plans", "9999-probe-fixture.md"));

const hookLog = join(out, "hook-env.jsonl");
const settingsPath = join(out, "settings.json");
writeFileSync(
  settingsPath,
  JSON.stringify(
    {
      permissions: {
        allow: [
          "Read",
          "Glob",
          "Grep",
          "Edit",
          "Write",
          "Skill",
          "Bash(cargo --version)",
          "Bash(git add *)",
          "Bash(git status *)",
        ],
      },
      hooks: {
        PreToolUse: [
          {
            matcher: "Bash|PowerShell",
            hooks: [
              {
                type: "command",
                command: "node",
                args: [fileURLToPath(import.meta.url), "--hook-env", hookLog],
              },
            ],
          },
        ],
      },
    },
    null,
    2,
  ),
);

const appendPath = join(out, "append.md");
writeFileSync(
  appendPath,
  `RLX-PROBE-MARKER: ${MARKER}

You are running inside a harness probe (Plan 0187 Phase 1). The skill you were invoked with is
loaded only so the probe can observe that it loaded. Do NOT follow its workflow: do not restate a
plan and do not wait for a go. Perform exactly these steps in order, using the Bash tool (never
PowerShell) for every command, and report each result verbatim:

1. Quote the first markdown heading line of the skill instructions you received.
2. Quote the one-line description CLAUDE.md in your context gives for tools/sd-filter/.
3. Give the value of the RLX-PROBE-MARKER line in your system prompt.
4. Bash: git add -A    (report the exact tool error or output)
5. Bash: cargo --version    (report the output)
6. Bash: node -e "console.log(42)"    (report the exact tool error or output)
7. Write a file probe-out.txt in the current directory containing exactly: alpha
   Then use the Edit tool to change alpha to beta. Report whether each call succeeded.
8. Bash: git status --short    (report the output)

End your reply with a fenced code block tagged rlx-outcome containing
{"kind": "probe", "steps": 8}
`,
);

// Session C's subject: a scratch file under `.claude/skills/` in the worktree, with known content, so
// an Edit that lands is visible from the parent afterwards and one that is refused leaves it as it
// was. Backlog 0230 saw the CLI refuse `Edit` under `.claude/` although settings.conductor.json
// allows the tool, and ADR-0209 grants a close every file under `.claude/skills/`. Whether that rule
// is reachable is a question about the CLI, so it is asked here rather than assumed either way.
const CLAUDE_SCRATCH = join(worktree, ".claude", "skills", "probe-scratch");
mkdirSync(CLAUDE_SCRATCH, { recursive: true });
writeFileSync(join(CLAUDE_SCRATCH, "NOTES.md"), "# probe scratch\n\nThe word here is alpha.\n");

// The steps go in session C's own `-p`, with no slash command: invoking `/dev` made the session obey
// that skill's restate-and-wait instead, and the question here is about the CLI's file access, which
// no skill is party to.
// The project paths are given absolute. A relative `.claude/...` is resolved against the user's home
// configuration directory rather than the working directory, which is a finding of its own (step 5
// reproduces it) but not the question: the question is the project's `.claude/`, in this worktree.
const CLAUDE_DIR_PROMPT = `Do exactly these five steps in order, using only Read, Edit and Write - never Bash or PowerShell.
Report each one verbatim, naming the tool, the path, and the exact text of any error. If a step is
refused, say so and go on to the next.

1. Read ${CLAUDE_SCRATCH}\\NOTES.md
2. Edit that same file, changing the word alpha to beta.
3. Write ${CLAUDE_SCRATCH}\\NEW.md containing exactly: gamma
4. Write ${worktree}\\probe-control.txt containing exactly: delta
   (outside .claude/, the control: it shows whether this session could write at all)
5. Read the relative path .claude/skills/probe-scratch/NOTES.md and report which absolute path the
   tool says it looked at.

Then end your reply with a fenced code block tagged rlx-outcome containing {"kind": "probe", "steps": 5}`;

// Session D asks the same question under settings that name `.claude/` explicitly, which is what
// separates a restriction the CLI applies to its own configuration directory from one the conductor's
// allowlist merely fails to grant. Several spellings of the path-scoped rule are offered at once: the
// question is whether any of them reaches, not which.
const openSettingsPath = join(out, "settings-claude-open.json");
writeFileSync(
  openSettingsPath,
  JSON.stringify(
    {
      permissions: {
        allow: ["Read", "Glob", "Grep", "Edit", "Write", "Edit(.claude/**)", "Write(.claude/**)", "Edit(//.claude/**)", "Write(//.claude/**)", `Edit(${worktree}/.claude/**)`, `Write(${worktree}/.claude/**)`],
      },
    },
    null,
    2,
  ),
);

const CLAUDE_OPEN_PROMPT = `Do exactly these three steps in order, using only Edit and Write - never Bash or PowerShell.
Report each one verbatim, naming the tool, the path, and the exact text of any error. If a step is
refused, say so and go on to the next.

1. Edit ${CLAUDE_SCRATCH}\\NOTES.md, changing the word alpha to beta.
2. Write ${CLAUDE_SCRATCH}\\NEW.md containing exactly: gamma
3. Write ${worktree}\\probe-control.txt containing exactly: delta

Then end your reply with a fenced code block tagged rlx-outcome containing {"kind": "probe", "steps": 3}`;

const env = { ...process.env, RLX_CONDUCTOR: "1", RLX_PROBE_TOKEN: token };

const common = [
  "--model",
  model,
  "--output-format",
  "stream-json",
  "--verbose",
  "--include-hook-events",
  "--permission-mode",
  "dontAsk",
  "--settings",
  settingsPath,
];

const runs = {
  a: [
    "-p",
    "/dev implement plan 9999",
    ...common,
    "--append-system-prompt-file",
    appendPath,
    "--max-budget-usd",
    "0.50",
  ],
  b: ["-p", "/dev implement plan 9999", ...common, "--max-budget-usd", "0.0001"],
  // Session C runs under the conductor's real settings file, not the probe's, because the question is
  // what a conductor session may do. It therefore has no probe hook and no `Bash(cargo --version)`.
  c: [
    "-p",
    CLAUDE_DIR_PROMPT,
    "--model",
    model,
    "--output-format",
    "stream-json",
    "--verbose",
    "--include-hook-events",
    "--permission-mode",
    "dontAsk",
    "--settings",
    join(HERE, "..", "settings.conductor.json"),
    "--max-budget-usd",
    "0.50",
  ],
  d: [
    "-p",
    CLAUDE_OPEN_PROMPT,
    "--model",
    model,
    "--output-format",
    "stream-json",
    "--verbose",
    "--include-hook-events",
    "--permission-mode",
    "dontAsk",
    "--settings",
    openSettingsPath,
    "--max-budget-usd",
    "0.50",
  ],
};

const meta = {
  stamp,
  model,
  cli_version: spawnSync("claude", ["--version"], { encoding: "utf8" }).stdout.trim(),
  worktree,
  branch,
  token,
  sessions: {},
};

for (const name of sessions) {
  meta.sessions[name] = await runSession(name, runs[name]);
  writeFileSync(join(out, "meta.json"), JSON.stringify(meta, null, 2));
}

// Observed from the parent after every child has exited: is the worktree still held?
const probeFile = join(worktree, "probe-out.txt");
meta.probe_out_txt = existsSync(probeFile) ? readFileSync(probeFile, "utf8") : null;
// What session C actually left on disk, which is the evidence; what it reported is only its account.
const readBack = (p) => (existsSync(p) ? readFileSync(p, "utf8") : null);
meta.claude_dir = {
  scratch: CLAUDE_SCRATCH,
  settings: { c: join(HERE, "..", "settings.conductor.json"), d: openSettingsPath },
  edited_notes_md: readBack(join(CLAUDE_SCRATCH, "NOTES.md")),
  wrote_new_md: readBack(join(CLAUDE_SCRATCH, "NEW.md")),
  control_outside_claude: readBack(join(worktree, "probe-control.txt")),
};
meta.worktree_remove = git(["worktree", "remove", "--force", worktree]);
meta.worktree_exists_after_remove = existsSync(worktree);
meta.branch_delete = git(["branch", "-D", branch]);
writeFileSync(join(out, "meta.json"), JSON.stringify(meta, null, 2));

const summary = analyzeRun(out);
writeFileSync(join(out, "summary.json"), JSON.stringify(summary, null, 2));
console.log(JSON.stringify(summary, null, 2));
console.log(`\nraw output: ${out}`);

function runSession(name, args) {
  return new Promise((resolveRun) => {
    const started = Date.now();
    const transcript = join(out, `session-${name}.jsonl`);
    const stderrPath = join(out, `session-${name}.stderr.txt`);
    writeFileSync(transcript, "");
    writeFileSync(stderrPath, "");
    const child = spawn("claude", args, { cwd: worktree, env, stdio: ["ignore", "pipe", "pipe"] });
    child.stdout.on("data", (d) => appendFileSync(transcript, d));
    child.stderr.on("data", (d) => appendFileSync(stderrPath, d));
    let timedOut = false;
    const timer = setTimeout(
      () => {
        timedOut = true;
        child.kill();
      },
      10 * 60 * 1000,
    );
    child.on("close", (code, signal) => {
      clearTimeout(timer);
      resolveRun({ args, exit_code: code, signal, timed_out: timedOut, ms: Date.now() - started });
    });
  });
}

function analyzeRun(dir) {
  const metaIn = JSON.parse(readFileSync(join(dir, "meta.json"), "utf8"));
  const summary = {
    cli_version: metaIn.cli_version,
    model: metaIn.model,
    token: metaIn.token,
    probe_out_txt: metaIn.probe_out_txt,
    claude_dir: metaIn.claude_dir ?? null,
    worktree_remove: metaIn.worktree_remove,
    worktree_exists_after_remove: metaIn.worktree_exists_after_remove,
    hook_env: existsSync(join(dir, "hook-env.jsonl"))
      ? readFileSync(join(dir, "hook-env.jsonl"), "utf8")
          .split("\n")
          .filter(Boolean)
          .map((l) => JSON.parse(l))
      : [],
    sessions: {},
  };
  for (const [name, run] of Object.entries(metaIn.sessions)) {
    const lines = readFileSync(join(dir, `session-${name}.jsonl`), "utf8")
      .split("\n")
      .filter(Boolean);
    const events = [];
    const unparsed = [];
    for (const l of lines) {
      try {
        events.push(JSON.parse(l));
      } catch {
        unparsed.push(l.slice(0, 200));
      }
    }
    const kinds = [...new Set(events.map((e) => [e.type, e.subtype].filter(Boolean).join("/")))];
    const init = events.find((e) => e.type === "system" && e.subtype === "init");
    const result = events.findLast((e) => e.type === "result");
    const toolUses = [];
    const toolResults = [];
    const texts = [];
    const skillText = [];
    for (const e of events) {
      const content = e.message?.content;
      if (!Array.isArray(content)) continue;
      for (const c of content) {
        if (e.type === "assistant" && c.type === "tool_use") {
          toolUses.push({ id: c.id, name: c.name, input: c.input });
        }
        if (e.type === "assistant" && c.type === "text") texts.push(c.text);
        if (e.type === "user" && c.type === "tool_result") {
          const body = Array.isArray(c.content)
            ? c.content.map((x) => x.text ?? "").join("")
            : String(c.content ?? "");
          toolResults.push({ id: c.tool_use_id, is_error: c.is_error ?? false, content: body.slice(0, 600) });
        }
        if (e.type === "user" && c.type === "text" && /# dev . Ritmolux/.test(c.text)) {
          skillText.push(c.text.split("\n").find((x) => x.startsWith("# ")));
        }
      }
    }
    summary.sessions[name] = {
      ...run,
      event_kinds: kinds,
      unparsed_lines: unparsed,
      init: init && {
        keys: Object.keys(init),
        session_id: init.session_id,
        model: init.model,
        permissionMode: init.permissionMode,
        cwd: init.cwd,
        skills: init.skills,
        slash_commands_has_dev: Array.isArray(init.slash_commands)
          ? init.slash_commands.includes("dev")
          : null,
      },
      result: result && {
        keys: Object.keys(result),
        subtype: result.subtype,
        is_error: result.is_error,
        session_id: result.session_id,
        total_cost_usd: result.total_cost_usd,
        num_turns: result.num_turns,
        result_text_tail: typeof result.result === "string" ? result.result.slice(-1500) : result.result,
        errors: result.errors,
      },
      skill_heading_in_user_message: skillText,
      hook_events: events
        .filter((e) => /hook/i.test(`${e.type}/${e.subtype}`))
        .map((e) => JSON.stringify(e).slice(0, 400)),
      tool_uses: toolUses,
      tool_results: toolResults,
      assistant_text_tail: texts.join("\n").slice(-2500),
    };
  }
  return summary;
}
