#!/usr/bin/env node
// Re-run the machine-runnable probes that docs/design-backlog.md's live entries
// carry beside their claims about this repository.
//
// Rationale: four backlog entries have been falsified and all four failed the
// same way — each asserted something about *the repo* (what it contains, what
// it documents, what is built) and each assertion was wrong when written or
// shortly after. 0052, 0078, 0081, 0082. Three carried no verification stamp at
// all; the fourth carried one that was dated, recent and *true*, and verified
// the half of the entry that survived rather than the half in its own title.
// That last case is why a prose stamp is not enough: it records that somebody
// looked, not what they looked at, and it cannot be re-run when the subject
// moves. See ADR-0108 and Plan 0093.
//
// Usage:  node scripts/check-backlog-claims.mjs [root]
//         node scripts/check-backlog-claims.mjs --self-test
//
// Exit 0 = every stated reduction still holds. Exit 1 = the breaks are listed
// as `file:line -> entry`, which is clickable in most terminals. The optional
// `root` scans some other tree — used to run this against the committed fixture
// under scripts/fixtures/; CI and the pre-push hook pass nothing and get the
// repo. That is the same argument check-doc-links.mjs takes, deliberately.
//
// GREEN MEANS "THE STATED REDUCTION STILL HOLDS", NEVER "THIS ENTRY IS TRUE."
// The probe verifies the reduction its author chose, and entry 0081 is the
// worked example of a verification that was true and off-target. The defence is
// that the probe is printed beside the claim and a reviewer can read whether it
// covers the claim; there is no mechanical defence and this script does not
// pretend to one.
//
// The grammar, in full — three forms, written as inline-code spans inside a
// `- **Verified <ISO date>**` bullet:
//
//   `absent: <regex> in: <path>`     no line under <path> matches <regex>
//   `present: <regex> in: <path>`    some line under <path> matches <regex>
//   `unprobeable: <why>`             this claim cannot be reduced; say why
//
// <path> is a file or a directory, resolved from the root. <regex> is JS regex
// source, so `.` needs escaping when a literal dot is meant: `G = C / 0\.85`
// matches the rule and `G = C / 0.85` also matches `0-85`.
//
// WHY THIS PARSES A GRAMMAR INSTEAD OF RUNNING A SHELL: the natural spelling of
// a probe is a backticked `grep -rn ... core/src/` and re-running it would be
// one execSync — which would make every markdown file in the repo a script CI
// executes on push. The restricted grammar buys the same expressiveness for the
// checks that actually occur, with a parser instead of an interpreter.
// (ADR-0108, Notes.)

import { readdirSync, readFileSync, existsSync, statSync } from "node:fs";
import { join, resolve, relative, sep, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const SCRIPT = fileURLToPath(import.meta.url);
const REPO_DEFAULT = resolve(dirname(SCRIPT), "..");

const argv = process.argv.slice(2);
const SELF_TEST = argv.includes("--self-test");
const ROOT_ARG = argv.find((a) => !a.startsWith("--"));
const REPO = resolve(ROOT_ARG ?? REPO_DEFAULT);

const BACKLOG = "docs/design-backlog.md";

// Build output, vendored deps, and VCS internals hold files we do not own.
const SKIP_DIRS = new Set(["target", "node_modules", ".git"]);

// An entry heading: `## 0082 — ...`. Deliberately NOT `## Entry 0032 — ...` or
// `## Entries 0068-0069 — ...`, which are section preambles, not entries.
const ENTRY = /^##\s+(\d{4})\b/;

// Everything above this heading is the closed-entry ledger, which is history.
// An archived entry is a closed record whose value is the correction it
// carries; re-probing it would be checking history against the present.
const LIVE_FROM = /^##\s+Open entries\b/i;

// The dated verification bullet this gate reads. The older undated
// `- **Verified against code:**` stamps keep their shape and are ignored here —
// what changed is that the mechanical part moves out of the sentence and into
// something re-runnable, not that the prose convention is replaced.
const BULLET = /^\s*[-*+]\s+\*\*Verified\s+(\d{4}-\d{2}-\d{2})\*\*/;

const VERB = /^(absent|present|unprobeable):\s*([\s\S]+)$/;

// `<pattern> in: <path>`, resolved against the LAST ` in: ` so a pattern may
// contain the word itself.
const SPLIT_IN = /^([\s\S]*?)\s+in:\s*(\S+)$/;

const show = (abs, root = REPO) => relative(root, abs).split(sep).join("/");

/**
 * Every file under `abs`, which may itself be a file. Binary files are skipped
 * by the caller rather than here, because that needs the bytes.
 */
function* filesUnder(abs) {
  const st = statSync(abs);
  if (!st.isDirectory()) {
    yield abs;
    return;
  }
  for (const entry of readdirSync(abs, { withFileTypes: true })) {
    const full = join(abs, entry.name);
    if (entry.isDirectory()) {
      if (!SKIP_DIRS.has(entry.name)) yield* filesUnder(full);
    } else {
      yield full;
    }
  }
}

/**
 * The first `file:line` under `pathAbs` whose line matches `re`, or null.
 *
 * The backlog itself is excluded: it quotes every probe verbatim, so a probe
 * scoped at `docs/` would otherwise be falsified by its own text.
 */
function firstMatch(pathAbs, re, root) {
  const selfQuoting = join(root, ...BACKLOG.split("/"));
  for (const file of filesUnder(pathAbs)) {
    if (file === selfQuoting) continue;
    const buf = readFileSync(file);
    // A NUL byte in the head is the cheap, encoding-free binary test. Decoding
    // a PNG as UTF-8 produces replacement characters that can match anything.
    if (buf.subarray(0, 8000).includes(0)) continue;
    const lines = buf.toString("utf8").split(/\r?\n/);
    for (let i = 0; i < lines.length; i++) {
      if (re.test(lines[i])) return `${show(file, root)}:${i + 1}`;
    }
  }
  return null;
}

/**
 * Resolve one parsed probe against a tree.
 * Returns `{ ok }` or `{ ok: false, detail }` — never throws for author error.
 */
function runProbe(probe, root) {
  if (probe.verb === "unprobeable") return { ok: true };

  let re;
  try {
    re = new RegExp(probe.pattern);
  } catch (err) {
    return { ok: false, detail: `malformed regex: ${err.message}` };
  }

  const pathAbs = resolve(root, probe.path);
  if (!existsSync(pathAbs)) {
    return { ok: false, detail: `probe path does not exist: ${probe.path}` };
  }

  const hit = firstMatch(pathAbs, re, root);
  if (probe.verb === "absent") {
    return hit ? { ok: false, detail: `matched at ${hit}` } : { ok: true };
  }
  return hit ? { ok: true } : { ok: false, detail: `no match under ${probe.path}` };
}

/** The verification bullets of every live entry, with their code spans. */
function readBullets(root) {
  const backlogAbs = join(root, ...BACKLOG.split("/"));
  if (!existsSync(backlogAbs)) return { fatal: `${BACKLOG} not found under ${root}` };

  const lines = readFileSync(backlogAbs, "utf8").split(/\r?\n/);
  const hasLiveMarker = lines.some((l) => LIVE_FROM.test(l));
  const bullets = [];
  const entries = new Set();
  let live = !hasLiveMarker;
  let entry = null;

  for (let i = 0; i < lines.length; i++) {
    if (LIVE_FROM.test(lines[i])) {
      live = true;
      continue;
    }
    const heading = lines[i].match(ENTRY);
    if (heading) {
      entry = live ? heading[1] : null;
      continue;
    }
    if (!live) continue;
    const stamp = lines[i].match(BULLET);
    if (!stamp) continue;

    // A wrapped bullet continues on indented, non-empty lines that do not start
    // a new list item — which is how an inline-code span can span two lines.
    const block = [lines[i]];
    for (let j = i + 1; j < lines.length; j++) {
      const next = lines[j];
      if (!next.trim() || !/^\s/.test(next)) break;
      if (/^\s*[-*+]\s/.test(next) || /^\s*#/.test(next)) break;
      block.push(next);
    }

    const spans = [...block.join("\n").matchAll(/`([^`]+)`/g)]
      .map((m) => m[1].replace(/\s+/g, " ").trim())
      .filter((s) => VERB.test(s));

    if (entry) entries.add(entry);
    bullets.push({ entry: entry ?? "(outside any entry)", line: i + 1, stamped: stamp[1], spans });
  }

  return { bullets, entries };
}

/**
 * Parse and resolve every probe in a tree.
 * Returns `{ breaks, probes, unprobeable, entries }`.
 */
function check(root) {
  const parsed = readBullets(root);
  if (parsed.fatal) return { fatal: parsed.fatal };

  const breaks = [];
  const probes = [];
  const unprobeable = [];

  for (const bullet of parsed.bullets) {
    const at = `${BACKLOG}:${bullet.line} -> ${bullet.entry}`;

    if (bullet.spans.length === 0) {
      breaks.push(`${at} — verification bullet carries no probe and no \`unprobeable:\` opt-out`);
      continue;
    }

    for (const span of bullet.spans) {
      const [, verb, rest] = span.match(VERB);

      if (verb === "unprobeable") {
        unprobeable.push({ entry: bullet.entry, line: bullet.line, why: rest, stamped: bullet.stamped });
        continue;
      }

      const split = rest.match(SPLIT_IN);
      if (!split) {
        breaks.push(`${at} \`${span}\` — malformed probe (expected \`${verb}: <regex> in: <path>\`)`);
        continue;
      }

      const probe = {
        entry: bullet.entry,
        line: bullet.line,
        verb,
        pattern: split[1],
        path: split[2],
        stamped: bullet.stamped,
        source: span,
      };
      const result = runProbe(probe, root);
      if (result.ok) probes.push(probe);
      else breaks.push(`${at} \`${span}\` — ${result.detail}`);
    }
  }

  return { breaks, probes, unprobeable, entries: parsed.entries };
}

// --- self-test ---------------------------------------------------------------
//
// The fixture half proves the checker reports the seeded breaks and nothing
// else. The non-vacuity half is the one that matters and it is permanent: it
// asserts that the probe reconstructed from backlog 0082's own claim FAILS
// against today's tree — the instrument proving, without time travel, that this
// gate would have caught the historical case on the day the governor landed. If
// `sustained_miss` is ever renamed this fails loudly, which is correct: it is
// pinned to the worked example, and the example is the point.

function selfTest() {
  const fixture = resolve(REPO_DEFAULT, "scripts/fixtures/backlog-claims");
  const results = [];
  const record = (label, ok, detail) => results.push({ label, ok, detail });

  const f = check(fixture);
  if (f.fatal) {
    console.error(`self-test: cannot read the fixture — ${f.fatal}`);
    process.exit(1);
  }

  const hits = (entry) => f.breaks.filter((b) => b.includes(`-> ${entry}`));
  record("fixture: violated absent:", hits("0001").length === 1, hits("0001")[0]);
  record("fixture: violated present:", hits("0002").length === 1, hits("0002")[0]);
  record(
    "fixture: malformed regex",
    hits("0003").length === 1 && /malformed regex/.test(hits("0003")[0] ?? ""),
    hits("0003")[0],
  );
  record("fixture: bullet with no probe", hits("0004").length === 1, hits("0004")[0]);
  record(
    "fixture: valid unprobeable:",
    hits("0005").length === 0 && f.unprobeable.some((u) => u.entry === "0005"),
    "must be rostered, never reported",
  );
  record(
    "fixture: nothing else reported",
    f.breaks.length === 4 && f.probes.length === 2,
    `${f.breaks.length} breaks, ${f.probes.length} holding probes`,
  );

  const zeroEightyTwo = { verb: "absent", pattern: "sustained_miss", path: "core/src" };
  const vacuity = runProbe(zeroEightyTwo, REPO_DEFAULT);
  record(
    "non-vacuity: `absent: sustained_miss in: core/src`",
    !vacuity.ok,
    vacuity.ok ? "HOLDS against HEAD — the worked example has stopped biting" : vacuity.detail,
  );

  const width = Math.max(...results.map((r) => r.label.length));
  for (const r of results) {
    const pad = r.label.length > 40 ? `\n${" ".repeat(width + 2)}` : " ".repeat(width - r.label.length + 1);
    console.log(`${r.label}${pad} ${r.ok ? "OK" : "FAILED"} (${r.detail ?? "no detail"})`);
  }
  const passed = results.filter((r) => r.ok).length;
  console.log(`self-test: ${passed}/${results.length}`);
  process.exit(passed === results.length ? 0 : 1);
}

// --- main --------------------------------------------------------------------

if (SELF_TEST) selfTest();

const { fatal, breaks, probes, unprobeable, entries } = check(REPO);

if (fatal) {
  console.error(`backlog claims: ${fatal}`);
  process.exit(1);
}

if (breaks.length === 0) {
  console.log(
    `backlog claims: OK — ${probes.length} stated reductions still hold across ` +
      `${entries.size} entries (${unprobeable.length} unprobeable)`,
  );
  console.log(
    "                green means the reductions still match the tree, not that the entries are true",
  );
  process.exit(0);
}

console.error(`backlog claims: ${breaks.length} broken`);
for (const b of breaks) console.error(`  ${b}`);
console.error(
  "\nA broken probe means the reduction an entry's author committed to no longer\n" +
    "matches the tree. That is not automatically the entry's fault — a rename\n" +
    "breaks a `present:` probe without touching the claim — but it does mean the\n" +
    "entry needs re-reading before anyone acts on it.\n" +
    "\n" +
    "Repairing a falsified entry is an `architect` call, not a `dev` one: whether\n" +
    "it is corrected in place, closed, or split is a judgement, and a wrong live\n" +
    "entry is more dangerous than a closed one.\n" +
    "\n" +
    "Grammar:  `absent: <regex> in: <path>`   `present: <regex> in: <path>`\n" +
    "          `unprobeable: <why>`           (printed in the summary, never a break)\n" +
    "Regex source, so a literal dot needs escaping: `0\\.85`, not `0.85`.",
);
process.exit(1);
