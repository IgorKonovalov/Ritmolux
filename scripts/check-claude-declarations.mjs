#!/usr/bin/env node
// Assert that a plan phase which names something under `.claude/` declares a `.claude/` path in its
// `**Files touched:**` bullet.
//
// The CLI refuses a headless session an `Edit` or `Write` under a project's `.claude/` whatever the
// allowlist says (ADR-0210), so the conductor parks a plan in front of a phase whose `Files touched`
// carries such a path. `claudePaths()` in tools/conductor/lib/plan.mjs reads that bullet for a
// literal path and nothing else: a phase that edits three skill files while calling them "the three
// conductor-mode sections" is a true sentence with no path in it, and it reaches a session, hits the
// denial, and parks `check_red` late. This gate moves that failure to drafting time. It cannot make
// the scan find a path that is not written; it refuses the phase until one is.
//
// Usage:  node scripts/check-claude-declarations.mjs [root]
//         node scripts/check-claude-declarations.mjs --self-test
//
// Exit 0 = no active plan names a `.claude/` artefact without declaring its path. Exit 1 = each
// phase that does, as `file:line  Phase N names <what> (<matched text>)`. The optional `root` reads
// some other tree's `docs/plans/`, following the checkers beside it; the seeded roots live under
// `scripts/fixtures/claude-declarations/`. CI and the pre-push hook pass nothing.
//
// A PLAN UNDER `docs/plans/done/` IS AN ADVISORY AND NEVER AN EXIT CODE. The conductor never runs a
// closed plan, so an undeclared edit in one is a record of how it was drafted rather than a park
// waiting to happen, and a plan is append-only once closed — the gate would be red on a file nobody
// may edit. Those phases are listed after the verdict so the record stays visible.
//
// WHAT COUNTS AS NAMING, and it is prose, so it is a heuristic with false positives in both
// directions. A phase body is read from its `### Phase` heading to the next heading, the `Owner
// skill` line excluded, with newlines folded so a phrase wrapped across two lines still matches:
//   - a path under `.claude/`, or `SKILL.md`, anywhere in the body;
//   - a skill, a conductor-mode section or `settings.json` behind a DEFINITE determiner — "the three
//     conductor-mode sections", "the `dev` skill", "the project's `settings.json`". "A skill" or "a
//     conductor-mode section" describes a class and names nothing, which is how a plan writes about
//     this gate without being convicted by it;
//   - a hook by its file name under `.claude/hooks/`, or a `PreToolUse` / `PostToolUse` hook. A bare
//     "hook" is not read, because `.githooks/pre-push` is one too.
// Text inside double quotes is a quotation and is not read: a plan quoting the shape it forbids is
// mentioning it. A `settings.json` qualified by the studio is the studio's own file (ADR-0240) and is
// not read either.
//
// `claude-allow: <reason>` anywhere in a phase body excuses that phase; a marker with no reason is
// itself a finding, the shape of `hygiene-allow:` and `count-allow:`.

import { existsSync, readdirSync, readFileSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const SCRIPT = fileURLToPath(import.meta.url);
const REPO_DEFAULT = resolve(dirname(SCRIPT), "..");

const argv = process.argv.slice(2);
const SELF_TEST = argv.includes("--self-test");
const ROOT_ARG = argv.find((a) => !a.startsWith("--"));
const REPO = resolve(ROOT_ARG ?? REPO_DEFAULT);

/**
 * The hook file names, read from this repository's `.claude/hooks/` whatever root is scanned: the
 * vocabulary is the harness's, and a seeded root carries no `.claude/` of its own.
 */
function hookNames() {
  const dir = join(REPO_DEFAULT, ".claude", "hooks");
  if (!existsSync(dir)) return [];
  return readdirSync(dir)
    .filter((f) => f.endsWith(".js"))
    .map((f) => f.slice(0, -3));
}

const escapeRe = (s) => s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

// A definite determiner, then at most two words, then the noun. Two words carry "the three
// conductor-mode sections" and "the implementer skills' conductor-mode sections"; the words are
// joined by whitespace only, so a comma or a full stop ends the reach.
const DEFINITE = String.raw`\b(?:the|its|their|each|every|both|all|these|those)`;
const WORDS = String.raw`((?:\s+[\w'’\`-]+){0,2}?)`;

const HOOKS = hookNames();

/** Each rule: what it names, its pattern, and an optional veto on the words the match reached over. */
const RULES = [
  // At least one character past the slash: a bare `.claude/` is the directory as a subject — "a
  // session may not edit `.claude/`" — and names no artefact in it.
  { what: "a .claude/ path", re: /\.claude\/[\w*][\w.*/-]*/g },
  { what: "a skill file", re: /\bSKILL\.md\b/g },
  {
    what: "a conductor-mode section",
    re: new RegExp(String.raw`${DEFINITE}${WORDS}\s+\`?conductor[- ]mode\`?\s+sections?\b`, "gi"),
  },
  {
    what: "a skill",
    re: new RegExp(String.raw`${DEFINITE}${WORDS}\s+skills?(?!\s+(?:lane|lanes|tag|tags)\b)\b`, "gi"),
  },
  {
    what: "settings.json",
    re: new RegExp(String.raw`${DEFINITE}${WORDS}\s+\`?settings(?:\.local)?\.json\b`, "gi"),
    veto: (words) => /studio/i.test(words),
  },
  { what: "settings.local.json", re: /\bsettings\.local\.json\b/g },
  { what: "a hook", re: /\b(?:PreToolUse|PostToolUse)\b/g },
  ...(HOOKS.length
    ? [{ what: "a hook", re: new RegExp(String.raw`\b(?:${HOOKS.map(escapeRe).join("|")})(?:\.js)?\b`, "g") }]
    : []),
];

// The reason stops at the end of the line or at an HTML comment's `-->`, so `<!-- claude-allow: -->`
// reads as the bare marker it is rather than as a reason of `-->`.
const ESCAPE = /claude-allow:[ \t]*([^\n]*?)[ \t]*(?:-->|$)/m;

/** Blank out every double-quoted span, keeping offsets, so a quotation is not read as a use. */
function maskQuotes(text) {
  return text.replace(/["“](?:[^"“”\n]|\n(?![ \t]*\n)){0,400}?["”]/g, (m) => m.replace(/[^\n]/g, " "));
}

/**
 * The phases of one plan: `{ id, line, body, declared }`. `line` is the heading's 1-based line,
 * `body` the text from the heading to the next heading with the `Owner skill` line blanked, and
 * `declared` whether `Files touched` names a `.claude/` path.
 */
export function phasesOf(raw) {
  const lines = raw.replace(/\r\n/g, "\n").split("\n");
  const start = lines.findIndex((l) => l.trim() === "## Implementation phases");
  if (start < 0) return [];
  const phases = [];
  let current = null;
  let collecting = false;
  for (let i = start + 1; i < lines.length; i++) {
    const line = lines[i];
    if (/^## /.test(line)) break;
    const h = line.match(/^### Phase (\d+[a-z]?)\b/);
    if (h) {
      current = { id: h[1], line: i + 1, lines: [], files: "" };
      phases.push(current);
      collecting = false;
      continue;
    }
    if (/^#{1,3} /.test(line)) {
      current = null;
      continue;
    }
    if (!current) continue;
    const files = line.match(/^- \*\*Files touched:\*\*\s*(.*)$/);
    if (files) {
      current.files = files[1];
      collecting = true;
    } else if (collecting) {
      if (/^\s*- \*\*/.test(line) || !line.trim()) collecting = false;
      else current.files += ` ${line.trim()}`;
    }
    current.lines.push(/^- \*\*Owner skill:\*\*/.test(line) ? "" : line);
  }
  return phases.map((p) => ({
    id: p.id,
    line: p.line,
    body: p.lines.join("\n"),
    // The path rule's pattern, so a bare `.claude/` declares nothing here either: claudePaths() finds
    // no path in it, and the conductor would not park in front of the phase.
    declared: /\.claude\/[\w*][\w.*/-]*/.test(p.files),
  }));
}

/** What one phase names without declaring it: `[{ line, what, text }]`, empty when it is clean. */
export function convict(phase) {
  if (phase.declared) return [];
  const escape = phase.body.match(ESCAPE);
  if (escape) {
    if (escape[1]) return [];
    const line = phase.line + 1 + phase.body.slice(0, escape.index).split("\n").length - 1;
    return [{ line, what: "claude-allow with no reason given", text: escape[0].trim() }];
  }
  const scanned = maskQuotes(phase.body);
  const found = [];
  for (const rule of RULES) {
    for (const m of scanned.matchAll(rule.re)) {
      if (rule.veto && rule.veto(m[1] ?? "")) continue;
      const line = phase.line + 1 + scanned.slice(0, m.index).split("\n").length - 1;
      found.push({ line, what: rule.what, text: m[0].replace(/\s+/g, " ").trim() });
    }
  }
  // One finding per phase is enough to name it; the first by position is the one a reader looks at.
  found.sort((a, b) => a.line - b.line);
  return found.slice(0, 1);
}

/** Every plan under `root`: `{ findings, advisory, plans }`, or `{ fatal }`. */
export function check(root) {
  const plansDir = join(root, "docs", "plans");
  if (!existsSync(plansDir)) return { fatal: `${plansDir} does not exist, so no plan was read` };
  const findings = [];
  const advisory = [];
  let plans = 0;
  for (const [dir, rel, into] of [
    [plansDir, "docs/plans", findings],
    [join(plansDir, "done"), "docs/plans/done", advisory],
  ]) {
    if (!existsSync(dir)) continue;
    for (const file of readdirSync(dir).sort()) {
      if (!/^\d{4}-.+\.md$/.test(file)) continue;
      plans++;
      const raw = readFileSync(join(dir, file), "utf8");
      for (const phase of phasesOf(raw)) {
        for (const f of convict(phase)) into.push({ path: `${rel}/${file}`, phase: phase.id, ...f });
      }
    }
  }
  return { findings, advisory, plans };
}

// --- self-test ---------------------------------------------------------------
//
// Two roots, one phase shape. `undeclared/` carries Plan 0190 Phase 9 as it was drafted - "the three
// conductor-mode sections" and no path - beside the silences the heuristic must keep; `declared/` is
// the same tree with that phase's paths written into `Files touched`. The counts are asserted: a
// bare "exits non-zero" is also what a crash looks like, and a detector that stopped matching exits
// 0 from both roots.

function runGate(root) {
  const r = spawnSync(process.execPath, [SCRIPT, root], { encoding: "utf8" });
  return { status: r.status, out: `${r.stdout}${r.stderr}` };
}

function selfTest() {
  const results = [];
  const record = (name, ok, detail, out) => results.push({ name, ok, detail, out });
  const fixtures = resolve(REPO_DEFAULT, "scripts/fixtures/claude-declarations");

  const red = runGate(join(fixtures, "undeclared"));
  const redRows = (red.out.match(/^ {2}docs\/plans\/\d{4}-\S+:\d+ {2}Phase /gm) ?? []).length;
  record(
    "undeclared: three convicted phases, exit 1",
    red.status === 1 && redRows === 3,
    `exit ${red.status}, ${redRows} finding(s)`,
    red.out,
  );
  record(
    "undeclared: Plan 0190 Phase 9's shape is convicted by its conductor-mode sections",
    /0001-the-routing-lands\.md:\d+ {2}Phase 9 names a conductor-mode section \(the (?:three )?conductor-mode sections\)/i.test(
      red.out,
    ),
    "expected Phase 9 named by its conductor-mode sections",
    red.out,
  );
  record(
    "undeclared: a claude-allow with no reason is itself a finding",
    /Phase 3 names claude-allow with no reason given/.test(red.out),
    "expected Phase 3's bare marker",
    red.out,
  );
  record(
    "undeclared: a Files touched of a bare .claude/ declares nothing",
    /0001-the-routing-lands\.md:\d+ {2}Phase 10 names a skill/.test(red.out),
    "expected Phase 10 convicted despite its bare .claude/",
    red.out,
  );
  record(
    "undeclared: the same shape in a closed plan is an advisory row",
    /advisory[^\n]*\n {2}docs\/plans\/done\/0002-a-closed-record\.md:\d+ {2}Phase 1 /.test(red.out),
    "expected the done/ plan under the advisory heading",
    red.out,
  );

  const green = runGate(join(fixtures, "declared"));
  record(
    "declared: the same phase with its paths written is clean, exit 0",
    green.status === 0 && /claude declarations: OK/.test(green.out),
    `exit ${green.status}`,
    green.out,
  );

  // The silences, one phase each in `undeclared/`, asserted by id rather than by count so a
  // regression names the shape it broke.
  for (const [id, shape] of [
    ["1", "a quotation of the forbidden shape"],
    ["2", "a class named with an indefinite article"],
    ["4", "a claude-allow with a reason"],
    ["5", "the pre-push hook and the studio's settings.json"],
    ["6", "a phase that declares its .claude/ path"],
    ["7", "the skill lane and an Owner skill line"],
  ]) {
    record(
      `undeclared: Phase ${id}, ${shape}, is silent`,
      !new RegExp(String.raw`0001-the-routing-lands\.md:\d+ {2}Phase ${id} `).test(red.out),
      `Phase ${id} was reported`,
      red.out,
    );
  }

  for (const r of results) {
    console.log(`  ${r.ok ? "ok  " : "FAIL"} ${r.name}`);
    if (!r.ok) {
      console.log(`       ${r.detail}`);
      for (const l of (r.out ?? "").trim().split(/\r?\n/)) if (l) console.log(`       | ${l}`);
    }
  }
  const passed = results.filter((r) => r.ok).length;
  console.log(`claude declarations self-test: ${passed} of ${results.length}`);
  // The count is asserted, not merely printed: a case deleted along with the rule it pinned would
  // otherwise leave this run green and shorter.
  process.exit(passed === results.length && results.length === 12 ? 0 : 1);
}

// --- main --------------------------------------------------------------------

if (SELF_TEST) selfTest();

const { fatal, findings, advisory, plans } = check(REPO);
if (fatal) {
  console.error(`claude declarations: ${fatal}`);
  process.exit(1);
}

function printAdvisory() {
  if (advisory.length === 0) return;
  console.log(`\nadvisory — closed plans, never part of the exit code (${advisory.length}):`);
  for (const f of advisory) console.log(`  ${f.path}:${f.line}  Phase ${f.phase} names ${f.what} (${f.text})`);
}

console.log(`claude declarations: ${plans} plan(s) read`);
if (findings.length === 0) {
  console.log("claude declarations: OK (every active phase naming a .claude/ artefact declares its path)");
  printAdvisory();
  process.exit(0);
}

console.error(`\nclaude declarations: ${findings.length} phase(s) name a .claude/ artefact with no path declared`);
for (const f of findings) console.error(`  ${f.path}:${f.line}  Phase ${f.phase} names ${f.what} (${f.text})`);
printAdvisory();
console.error(
  "\nA headless session cannot edit under `.claude/` (ADR-0210), and the conductor stops in front of\n" +
    "a phase only when its `**Files touched:**` bullet carries the literal path. Write the paths:\n" +
    "\n" +
    "  before:  - **Files touched:** `prompts/*.md` and the three conductor-mode sections.\n" +
    "  after:   - **Files touched:** `prompts/*.md`, `.claude/skills/dev/SKILL.md`, ...\n" +
    "\n" +
    "Where the phase mentions the artefact without editing it, put `claude-allow: <why>` in the\n" +
    "phase body, as an HTML comment. The reason is reviewed rather than checked.",
);
process.exit(1);
