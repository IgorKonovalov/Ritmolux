// The allowlist every conductor session runs under.
//
// A session must be able to put a file back with `git restore`, to make and remove a scratch
// directory inside its own lane, and to read a file — and must never reach a `git checkout` that can
// move the lane's branch, `git stash`, whose stack every worktree shares, or a deletion whose path
// leaves the worktree.
//
// THE TWO HALVES REST ON DIFFERENT THINGS (ADR-0233). A case that says a command is REFUSED and
// names a `recorded` shape is asserted against what the real CLI did with that shape, read out of the
// table in `tools/conductor/spike/README.md` that `spike/matcher-probe.mjs` produced; being wrong
// about a refusal costs a directory, so it is not left to a model. It also goes through `decide`, so a
// deny rule deleted after the probe turns it red. Every other case — every allowed
// one, and a refusal the probe has not asked — goes through `decide` below, which models the CLI's
// documented rule matching: a rule's text is matched against the whole command with `*` standing for
// any run of characters, spaces included; a rule whose only wildcard is a trailing ` *` also matches
// the bare command; a compound command is split at `&&`, `||`, `;`, `|`, `&` and newlines and every
// part must be allowed on its own; and deny is consulted before allow. That half is a model of the
// CLI, not the CLI: the probe has already caught it wrong — it refuses a `| sed` the CLI runs, and
// before the `$` and backtick rules it allowed the shell expansions the CLI refuses — and a CLI that
// changed its matcher would not turn it red. Being wrong about an allowed case costs a session one turn.

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";

import { TOOL_DIR } from "./helpers.mjs";

const settings = JSON.parse(readFileSync(join(TOOL_DIR, "settings.conductor.json"), "utf8"));

// The rustdoc form the review prompt prints, read out of the prompt, so a prompt and an allowlist that
// drift apart turn a case below red rather than send a session into a denial.
const promptedRustdoc = readFileSync(join(TOOL_DIR, "prompts", "review.md"), "utf8").match(/`(RUSTDOCFLAGS=[^`]+)`/)?.[1];
// The recorded half: the probe's verdict table, keyed by the shape exactly as the table spells it.
// A re-run adds a `Verdict ...` column beside the earlier ones rather than overwriting them, so each
// row's verdict is the rightmost one it carries, and `refusedBy` is the column beside that verdict.
const recorded = readRecorded(readFileSync(join(TOOL_DIR, "spike", "README.md"), "utf8"));

/** { cli, verdicts: Map<shape, verdict>, refusedBy: Map<shape, text> } from spike/README.md. */
function readRecorded(md) {
  const heading = md.match(/^## What does the allowlist refuse\? Observed on (\S+)$/m);
  assert.ok(heading, "spike/README.md carries no allowlist verdict table");
  const section = md.slice(heading.index).split(/\n## /)[0];
  const lines = section.split("\n").filter((l) => l.startsWith("|"));
  const cells = (l) => l.slice(1, -1).split(/(?<!\\)\|/).map((c) => c.trim());
  const verdictCols = cells(lines[0]).flatMap((h, i) => (h.startsWith("Verdict") ? [i] : []));
  assert.ok(verdictCols.length, "the verdict table has no Verdict column");
  const verdicts = new Map();
  const refusedBy = new Map();
  for (const l of lines.slice(2)) {
    const row = cells(l);
    const shape = row[0].match(/^(`+)\s?(.*?)\s?\1/)?.[2]?.replace(/\\\|/g, "|");
    for (const col of [...verdictCols].reverse()) {
      const verdict = row[col]?.match(/\*\*(.+?)\*\*/)?.[1];
      if (shape && verdict) {
        verdicts.set(shape, verdict);
        refusedBy.set(shape, row[col + 1] ?? "");
        break;
      }
    }
  }
  return { cli: heading[1], verdicts, refusedBy };
}

const allow = settings.permissions.allow;
const deny = settings.permissions.deny;

// ---------------------------------------------------------------------------------------------
// The matcher

const escape = (s) => s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

/** One rule's pattern as a regular expression over the whole command. */
function ruleRegex(pattern) {
  const body = pattern.split("*").map(escape).join("[\\s\\S]*");
  // A rule whose only wildcard is a trailing ` *` also covers the command with no arguments at all,
  // so `Bash(git status *)` matches a bare `git status`.
  if ((pattern.match(/\*/g) ?? []).length === 1 && pattern.endsWith(" *")) {
    return new RegExp(`^(?:${body}|${escape(pattern.slice(0, -2))})$`);
  }
  return new RegExp(`^${body}$`);
}

const parsed = new Map();
for (const rule of [...allow, ...deny]) {
  const m = rule.match(/^(\w+)\((.*)\)$/s);
  parsed.set(rule, m ? { tool: m[1], regex: ruleRegex(m[2]) } : { tool: rule, regex: null });
}

const parts = (command) =>
  command
    .split(/&&|\|\||;|\||&|\n/)
    .map((s) => s.trim())
    .filter(Boolean);

/**
 * Every rule of `rules` that covers `part`. The decision needs only the first, but the roster test
 * below asks which rules a case covers, and a rule the CLI would reach second is covered all the
 * same: `Bash(git status)` says what `Bash(git status *)` says, deliberately, in case the trailing
 * wildcard does not reach a bare command.
 */
function matching(rules, tool, part) {
  return rules.filter((rule) => {
    const p = parsed.get(rule);
    return p.tool === tool && (p.regex === null || p.regex.test(part));
  });
}

/** { allowed, rules } for one tool call: the rules it relied on, or the deny rules that stopped it. */
function decide(tool, command) {
  const rules = [];
  // A tool with no command of its own — Read, Monitor, WebFetch — is decided by the bare tool name.
  for (const part of parts(command).length ? parts(command) : [""]) {
    const denied = matching(deny, tool, part);
    if (denied.length) return { allowed: false, rules: denied };
    const allowed = matching(allow, tool, part);
    if (!allowed.length) return { allowed: false, rules: [] };
    rules.push(...allowed);
  }
  return { allowed: true, rules };
}

// ---------------------------------------------------------------------------------------------
// The cases
//
// Every rule in the file must appear in `rules` of some case below, which is what makes a rule added
// without a case a red test rather than a permission nobody looked at.

/** { tool, command, allowed, why? } */
const CASES = [
  // The tool-level grants and denials.
  { tool: "Read", command: "", allowed: true },
  { tool: "Glob", command: "", allowed: true },
  { tool: "Grep", command: "", allowed: true },
  { tool: "Edit", command: "", allowed: true },
  { tool: "Write", command: "", allowed: true },
  { tool: "Skill", command: "", allowed: true },
  { tool: "Monitor", command: "", allowed: false, why: "nothing re-invokes a headless session" },
  { tool: "WebFetch", command: "", allowed: false },
  { tool: "WebSearch", command: "", allowed: false },

  // Building, testing and reading the repository.
  { tool: "Bash", command: "cargo fmt --all --check", allowed: true },
  { tool: "Bash", command: "node scripts/toc.mjs --check", allowed: true },
  { tool: "Bash", command: "npm --prefix studio run typecheck", allowed: true },
  { tool: "Bash", command: "npx vitest run", allowed: true },
  { tool: "Bash", command: "python3 tools/sd-filter/bench.py", allowed: true },
  { tool: "Bash", command: "RUSTDOCFLAGS=-D warnings cargo doc --no-deps", allowed: true },

  // The two regenerations this project documents, each in both spellings a session writes: the bare
  // one docs/developing.md and presets/README.md give, and the lock-wrapped one a conductor session
  // must use for anything that runs tests. Every other variable in the same shape stays refused —
  // the rule is a list of named variables, not the shape `VAR=value <allowed command>`, which would
  // admit the ones that change what a build does.
  { tool: "Bash", command: "RLX_UPDATE_PRESET_SCHEMA=1 cargo nextest run -p rlx-core --test suite preset_schema::", allowed: true },
  {
    tool: "Bash",
    command: "RLX_UPDATE_PRESET_SCHEMA=1 node tools/conductor/with-lock.mjs suite -- cargo nextest run -p rlx-core --test suite preset_schema::",
    allowed: true,
    why: "the suite-lock hook denies the bare form in a conductor session",
  },
  { tool: "Bash", command: "RLX_UPDATE_PARAM_REFERENCE=1 cargo test -p rlx-core --test suite the_parameter_reference_block_is_current", allowed: true },
  {
    tool: "Bash",
    command: "RLX_UPDATE_PARAM_REFERENCE=1 node tools/conductor/with-lock.mjs suite -- cargo test -p rlx-core --test suite the_parameter_reference_block_is_current",
    allowed: true,
  },
  { tool: "Bash", command: "RLX_ANYTHING_ELSE=1 cargo nextest run --workspace", allowed: false, why: "the rule names variables, it is not a shape" },
  { tool: "Bash", command: "CARGO_TARGET_DIR=target/p9 cargo build", allowed: false, why: "same: a variable that changes what a build does" },
  { tool: "Bash", command: "RLX_UPDATE_PRESET_SCHEMA=0 cargo nextest run --workspace", allowed: false, why: "the documented value is part of the rule" },
  { tool: "PowerShell", command: "$env:RLX_UPDATE_PRESET_SCHEMA = '1'; cargo nextest run", allowed: false, why: "an assignment is its own command in that shell" },
  { tool: "PowerShell", command: "cargo clippy --workspace --all-targets -- -D warnings", allowed: true },
  { tool: "PowerShell", command: "node tools/conductor/with-lock.mjs suite -- cargo nextest run --workspace", allowed: true },
  { tool: "PowerShell", command: "npm --prefix studio run build", allowed: true },
  { tool: "PowerShell", command: "npx electron-builder --dir", allowed: true },

  // Reading and writing history in the lane.
  { tool: "Bash", command: "git status --short", allowed: true },
  { tool: "Bash", command: "git status", allowed: true },
  { tool: "Bash", command: "git add core/src/lib.rs", allowed: true },
  { tool: "Bash", command: 'git commit -m "feat(core): a thing"', allowed: true },
  { tool: "Bash", command: "git log --oneline -5", allowed: true },
  { tool: "Bash", command: "git diff --stat", allowed: true },
  { tool: "Bash", command: "git diff", allowed: true },
  { tool: "Bash", command: "git show HEAD --stat", allowed: true },
  { tool: "Bash", command: "git mv docs/plans/0190-x.md docs/plans/done/0190-x.md", allowed: true },
  { tool: "Bash", command: "git rm core/tests/dead.rs", allowed: true },
  { tool: "Bash", command: "git merge main", allowed: true },
  { tool: "Bash", command: "git tag -a v0.1.0 -m 'chore: Release v0.1.0'", allowed: true },
  { tool: "Bash", command: "git cat-file -t v0.1.0", allowed: true },
  { tool: "Bash", command: "git rev-parse HEAD", allowed: true },
  { tool: "Bash", command: "git ls-files presets/", allowed: true },
  { tool: "Bash", command: "git grep -n TODO", allowed: true },
  { tool: "Bash", command: "git branch --show-current", allowed: true },
  { tool: "Bash", command: "git worktree list", allowed: true },
  { tool: "PowerShell", command: "git status --short", allowed: true },
  { tool: "PowerShell", command: "git status", allowed: true },
  { tool: "PowerShell", command: "git add docs/plans/0190-x.md", allowed: true },
  { tool: "PowerShell", command: "git commit -m 'docs(plans): 0190 Phase 2'", allowed: true },
  { tool: "PowerShell", command: "git log --oneline -3", allowed: true },
  { tool: "PowerShell", command: "git diff --cached", allowed: true },
  { tool: "PowerShell", command: "git show HEAD", allowed: true },
  { tool: "PowerShell", command: "git mv a.md b.md", allowed: true },
  { tool: "PowerShell", command: "git merge main", allowed: true },
  { tool: "PowerShell", command: "git tag -a v0.1.0 -m 'chore: Release v0.1.0'", allowed: true },
  { tool: "PowerShell", command: "git cat-file -t v0.1.0", allowed: true },
  { tool: "PowerShell", command: "git rev-parse HEAD", allowed: true },

  // Every command backlog 0231 recorded as refused, one case each. First the ones the widened
  // settings now run.
  { tool: "PowerShell", command: "New-Item -ItemType Directory -Force target/p8", allowed: true },
  { tool: "Bash", command: "mkdir -p target/p8", allowed: true },
  { tool: "Bash", command: "mkdir -p target/p8 && node scripts/docs-shots.mjs", allowed: true },
  { tool: "PowerShell", command: "Remove-Item studio/shared/seed-target.ts", allowed: true },
  { tool: "Bash", command: "rm target/p8/out.png", allowed: true },
  { tool: "Bash", command: "rm -rf target/p8", allowed: true },
  { tool: "Bash", command: "git clean -f -- studio/shared/seed-target.ts", allowed: true },
  { tool: "PowerShell", command: "git clean -f -- studio/shared/seed-target.ts", allowed: true },
  { tool: "Bash", command: "git checkout -- core/tests/goldens/attractor.png", allowed: true },
  { tool: "PowerShell", command: "git checkout -- core/tests/goldens/attractor.png", allowed: true },
  { tool: "Bash", command: "git restore core/tests/goldens/attractor.png", allowed: true },
  { tool: "PowerShell", command: "git restore core/tests/goldens/attractor.png", allowed: true },
  { tool: "Bash", command: "cat tools/conductor/state/conductor.json", allowed: true },
  { tool: "PowerShell", command: "Get-Content tools/conductor/state/conductor.json", allowed: true },

  // Then the ones refused for their shape rather than their verb. The prompts' shape rules are what
  // avoids these; widening for them would mean allowing `cd` and a bare assignment.
  { tool: "PowerShell", command: "cd studio; npm run typecheck", allowed: false, why: "`cd` is not a command a session may run" },
  { tool: "Bash", command: "cd studio && npm run typecheck", allowed: false, why: "same, in the other shell", recorded: "cd tools && git status --short" },
  { tool: "PowerShell", command: "$env:ELECTRON_SKIP_BINARY_DOWNLOAD = '1'; npm --prefix studio ci", allowed: false, why: "an assignment is its own command" },
  { tool: "PowerShell", command: "$env:CARGO_TARGET_DIR = 'target/p9'; cargo nextest run", allowed: false, why: "same" },

  // The four commands Plan 0221's run recorded under `permission_denials`. The prompts now spell the
  // allowed form of each; the refused forms stay refused, since `env *`, `awk *` and
  // `git -C * add *` would each reach any program, or any checkout on the machine.
  { tool: "Bash", command: promptedRustdoc ?? "review.md prints no RUSTDOCFLAGS form", allowed: true, why: "the literal the review prompt prints" },
  { tool: "Bash", command: 'env RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps', allowed: false, why: "`env` runs any program" },
  { tool: "Bash", command: "git -C /elsewhere add x", allowed: false, why: "`-C` reaches a checkout outside the lane" },
  { tool: "Bash", command: "git -C /elsewhere commit -m x", allowed: false },
  { tool: "Bash", command: "awk '/a/,/b/' docs/developing.md", allowed: false, why: "`awk` writes files and runs commands" },
  { tool: "Bash", command: "awk '/^## Disk/,/^## /' docs/developing.md | grep -c config.toml", allowed: false, why: "the done-when pipe: its `awk` part is refused" },

  // The eight commands the 2026-09-24 sessions were refused. A command is admitted when the session
  // already holds the capability through Read, Glob, Grep or Write; the rest stay refused.
  { tool: "Bash", command: "ls", allowed: true },
  { tool: "Bash", command: "ls -la tools/conductor/lib", allowed: true },
  { tool: "Bash", command: "printenv RLX_CONDUCTOR", allowed: true },
  { tool: "Bash", command: "grep -n settledPark tools/conductor/lib/digest.mjs", allowed: true },
  { tool: "Bash", command: "sed -n 180,200p tools/conductor/lib/digest.mjs", allowed: true },
  { tool: "Bash", command: "sed -i s/a/b/ core/src/lib.rs", allowed: false, why: "only `sed -n` is admitted; an in-place edit is Edit's" },
  { tool: "Bash", command: "sed s/a/b/ core/src/lib.rs", allowed: false },
  { tool: "Bash", command: "cp core/src/lib.rs core/src/lib2.rs", allowed: false, why: "Write is the reviewed path for creating a file" },
  { tool: "Bash", command: "mv core/src/lib.rs core/src/lib2.rs", allowed: false, why: "same" },
  { tool: "Bash", command: "gh run list", allowed: false, why: "reaches the network and authenticates as the owner" },
  { tool: "Bash", command: "target/debug/shot --preset rose_star", allowed: false, why: "a bare binary path runs any program" },
  { tool: "Bash", command: 'git grep -n -E "settledPark|parkStillTrue" -- tools/conductor', allowed: false, why: "the alternation's `|` splits the command" },
  { tool: "Bash", command: "git grep -n -e settledPark -e parkStillTrue -- tools/conductor", allowed: true, why: "the same search, spelled without a `|`" },

  // The lane is the bound. A deletion whose path leaves it is refused however it is spelled.
  { tool: "Bash", command: "rm -rf ../rlx-plan-0175", allowed: false, why: "escapes the worktree", recorded: "rm -rf ../canary-dotdot" },
  { tool: "Bash", command: "rm ../../secrets.txt", allowed: false, recorded: "rm -rf ../canary-dotdot" },
  { tool: "Bash", command: "rm -rf ~/.cargo", allowed: false, recorded: "rm -rf ~/canary-tilde" },
  { tool: "Bash", command: "rm -rf /etc/hosts", allowed: false, recorded: "rm -rf <absolute path>" },
  { tool: "Bash", command: "rm /tmp/x", allowed: false, recorded: "rm -rf <absolute path>" },
  { tool: "Bash", command: "rm -rf C:/Users/Someone/WORK", allowed: false, recorded: "rm -rf C:/canary-drive" },
  { tool: "Bash", command: "rm target/x /home/someone/.ssh", allowed: false, why: "an absolute path after the first argument; not probed" },
  { tool: "Bash", command: "rm -rf target/x /etc", allowed: false },
  { tool: "Bash", command: 'rm -rf "/etc"', allowed: false, why: "a quoted absolute path; not probed" },
  { tool: "Bash", command: "rm -rf '/home/someone/.ssh'", allowed: false, why: "a quoted absolute path; not probed" },
  { tool: "Bash", command: 'rm -rf "target/debug"', allowed: true, why: "a quoted relative path still runs" },
  // Paths the shell produces rather than a session writes. `decide` refuses every one of these
  // through the `$` and backtick rules, and the transcript is what shows the CLI agrees.
  { tool: "Bash", command: "rm -rf $HOME/.cargo", allowed: false, recorded: "rm -rf $HOME/.cargo" },
  { tool: "Bash", command: "rm -rf ${HOME}/.cargo", allowed: false, recorded: "rm -rf ${HOME}/canary-brace" },
  { tool: "Bash", command: 'rm -rf "$(git rev-parse --show-toplevel)/../rlx-plan-0180"', allowed: false, recorded: 'rm -rf "$(git rev-parse --show-toplevel)/.."' },
  { tool: "Bash", command: 'rm -rf "$(dirname "$PWD")"/rlx-plan-0180', allowed: false, recorded: 'rm -rf "$(dirname "$PWD")"/canary-subst' },
  { tool: "Bash", command: "rm -rf `dirname $PWD`/rlx-plan-0180", allowed: false, recorded: "rm -rf `dirname $PWD`/canary-tick" },
  { tool: "Bash", command: 'rm -rf "$SCRATCH"', allowed: false, why: "the bound's price: any expansion in a deletion is refused, a legitimate one too" },
  { tool: "PowerShell", command: "Remove-Item -Recurse $env:USERPROFILE\\WORK", allowed: false, why: "not probed: the PowerShell tool exists only on Windows" },
  { tool: "Bash", command: "rm -rf C:\\Users\\Someone\\WORK", allowed: false },
  { tool: "PowerShell", command: "Remove-Item -Recurse ../rlx-plan-0175", allowed: false },
  { tool: "PowerShell", command: "Remove-Item ~/.cargo/config.toml", allowed: false },
  { tool: "PowerShell", command: "Remove-Item /etc/hosts", allowed: false },
  { tool: "PowerShell", command: "Remove-Item -Recurse /var/log", allowed: false },
  { tool: "PowerShell", command: "Remove-Item target/x /home/someone/.ssh", allowed: false, why: "an absolute path after the first argument" },
  { tool: "PowerShell", command: 'Remove-Item "/etc"', allowed: false, why: "a quoted absolute path; not probed" },
  { tool: "PowerShell", command: "Remove-Item -Recurse '/var/log'", allowed: false, why: "a quoted absolute path; not probed" },
  { tool: "PowerShell", command: "Remove-Item C:/Users/Someone/WORK", allowed: false },
  { tool: "PowerShell", command: "Remove-Item -Recurse C:\\Users\\Someone\\WORK", allowed: false },

  // A `git clean` with no path would delete everything untracked in the lane, the scratch a phase
  // is working in included. The rule asks for the path after `--`, so a bare one matches nothing.
  { tool: "Bash", command: "git clean -fdx", allowed: false, why: "no path" },
  { tool: "Bash", command: "git clean", allowed: false },
  { tool: "PowerShell", command: "git clean -fdx", allowed: false },
  { tool: "Bash", command: "git clean -f studio/shared/seed-target.ts", allowed: false, why: "the path is not after a `--`" },

  // `git checkout` reaches the lane's branch unless `--` forces the argument to be a path.
  { tool: "Bash", command: "git checkout main", allowed: false },
  { tool: "Bash", command: "git checkout -b plan-0190", allowed: false },
  { tool: "PowerShell", command: "git checkout main", allowed: false },

  // Unchanged since before this plan: the stash stack is shared across worktrees, and the owner pushes.
  { tool: "Bash", command: "git stash push -m wip", allowed: false },
  { tool: "Bash", command: "git stash pop", allowed: false },
  { tool: "Bash", command: "git push origin main", allowed: false },
  { tool: "Bash", command: "git push", allowed: false },
  { tool: "PowerShell", command: "git push origin main", allowed: false },
  { tool: "PowerShell", command: "git push", allowed: false },
  { tool: "Bash", command: "cargo build && git push", allowed: false, why: "one part of a compound command is enough" },
];

for (const c of CASES) {
  const label = c.command ? `${c.tool}: ${c.command}` : c.tool;
  const basis = c.recorded ? ` [recorded on ${recorded.cli}]` : "";
  test(`${c.allowed ? "allowed" : "refused"} - ${label}${c.why ? ` (${c.why})` : ""}${basis}`, () => {
    if (c.recorded) {
      // A refusal the probe asked is what the CLI did, not what the model says: a shape the table
      // records as RAN turns this red however the rules read.
      assert.equal(c.allowed, false, "a recorded shape backs a refusal only");
      const verdict = recorded.verdicts.get(c.recorded);
      assert.ok(verdict, `spike/README.md records no verdict for \`${c.recorded}\``);
      assert.equal(verdict, "DENIED", `the CLI ${verdict} \`${c.recorded}\` on ${recorded.cli}`);
      // The table is frozen at probe time, so it cannot see a rule deleted since. The model reads the
      // file as it is now: the transcript says the CLI refused the shape, and this says the file still
      // carries a rule that refuses it.
      assert.equal(decide(c.tool, c.command).allowed, false, "settings.conductor.json no longer refuses a shape the CLI was recorded refusing");
    } else {
      assert.equal(decide(c.tool, c.command).allowed, c.allowed);
    }
  });
}

test("a deletion through a shell expansion is refused by the file, not only by the CLI", () => {
  // Without a rule of its own, `${HOME}`, `$(...)` and backticks were refused only because dontAsk
  // does not auto-allow them - behaviour of one CLI version, not a claim this file makes.
  for (const shape of ["rm -rf $HOME/.cargo", "rm -rf ${HOME}/canary-brace", 'rm -rf "$(dirname "$PWD")"/canary-subst', "rm -rf `dirname $PWD`/canary-tick"]) {
    assert.equal(recorded.verdicts.get(shape), "DENIED", shape);
    assert.match(recorded.refusedBy.get(shape), /deny rule/, `${shape} is refused by ${recorded.refusedBy.get(shape)}`);
  }
});

test("a literal in-lane deletion still runs, so the bound did not buy safety by refusing work", () => {
  assert.equal(recorded.verdicts.get("rm -rf target/debug"), "RAN");
});

test("the verdict table's literal-escape controls are refused, or the run proved nothing", () => {
  // The four shapes the deny rules were written for. If any of them ran, the settings file was not
  // in force during the probe and no other verdict in the table means anything.
  for (const shape of ["rm -rf ../canary-dotdot", "rm -rf ~/canary-tilde", "rm -rf <absolute path>", "rm -rf C:/canary-drive"]) {
    assert.equal(recorded.verdicts.get(shape), "DENIED", shape);
  }
});

test("every rule in settings.conductor.json is exercised by a case above", () => {
  const exercised = new Set();
  for (const c of CASES) for (const r of decide(c.tool, c.command).rules) exercised.add(r);
  // A refused case names the deny rule that stopped it; an allowed one names every allow rule it
  // leaned on. What is left is a rule nobody wrote a case for.
  const unexercised = [...allow, ...deny].filter((r) => !exercised.has(r));
  assert.deepEqual(unexercised, [], "add a case for each of these, or drop the rule");
});

test("no allow entry reaches a branch-moving git checkout, or git stash at all", () => {
  // `git checkout -- <path>` cannot move HEAD: `--` forces every argument after it to be a pathspec.
  // Any other checkout form can, and the stash stack is shared by every worktree on the machine.
  const reaching = allow.filter(
    (e) => /\bgit\s+stash\b/.test(e) || (/\bgit\s+checkout\b/.test(e) && !/\bgit\s+checkout\s+--\s/.test(e)) || /^(Bash|PowerShell)$/.test(e) || /\((git|git \*)\)$/.test(e),
  );
  assert.deepEqual(reaching, []);
});

test("sessions may not arm a Monitor", () => {
  // Nothing re-invokes a headless session, so a Monitor armed on a backgrounded command never fires
  // and the session ends having lost that work.
  assert.ok(deny.includes("Monitor"));
  assert.ok(!allow.includes("Monitor"));
});
