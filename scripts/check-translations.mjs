#!/usr/bin/env node
// Every `.ru.md` translation carries a `translated-from: <sha>` stamp, and the
// stamp is compared against the commit that last touched its source. ADR-0185.
//
// A translation is the one second copy in this repository that NO MACHINE CAN
// CHECK: a link either resolves or does not, a roster row either fits 320 bytes
// or does not, and no gate here can tell whether a Russian paragraph still says
// what the English one now says. So the split is deliberate and it is the whole
// design:
//
//   MISSING OR MALFORMED STAMP -> exit 1. Mechanical, not a judgement.
//   THE SOURCE HAS MOVED       -> an advisory row, and exit 0.
//
// Hard-failing on drift was rejected: with one translator, it makes the Russian
// slice a hostage of every hotkey edit to docs/running.md, and a gate that
// blocks work for a reason nobody can fix in the same commit gets bypassed.
//
// Usage:  node scripts/check-translations.mjs [root]
//         node scripts/check-translations.mjs --self-test
//
// The optional `root` scans some other tree - the seeded fixtures under
// scripts/fixtures/translations/, which the ordinary repository walk skips by
// path. That is the same argument check-doc-links.mjs takes, deliberately.
//
// A TRANSLATION IS A SIBLING, NOT A LISTED FILE: `<name>.ru.md` beside
// `<name>.md`, so the pair is adjacent to any sweep that opens either one and
// there is no manifest to keep in step. A `.ru.md` whose source is not beside it
// is a break rather than a skip - that is what a rename looks like from here.
//
// THE STALENESS HALF NEEDS HISTORY AND SAYS SO WHEN IT HAS NONE. On a shallow
// clone `git log -1 -- <path>` returns the tip commit for every path, because
// the tip is grafted parentless and every file reads as added by it - so every
// translation would report as stale, from the first run after the stamps were
// written, and an advisory that is wrong every time is an advisory nobody reads.
// The reading is withheld instead, which is the shape ADR-0108's probe gate
// already uses for the same trap. The site build carries the same trap and
// guards it twice (site/src/plugins/translation-banner.mjs, and the
// `fetch-depth: 0` on the Pages `build` job).

import { execFileSync, spawnSync } from "node:child_process";
import { cpSync, existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve, sep } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const SCRIPT = fileURLToPath(import.meta.url);
const REPO_DEFAULT = resolve(dirname(SCRIPT), "..");

const argv = process.argv.slice(2);
const SELF_TEST = argv.includes("--self-test");
const ROOT_ARG = argv.find((a) => !a.startsWith("--"));
const REPO = resolve(ROOT_ARG ?? REPO_DEFAULT);

const SUFFIX = ".ru.md";

/**
 * The stamp, on line 1 and nowhere else.
 *
 * Seven to forty lowercase hex: `git log -1 --format=%h` writes the short form
 * and `%H` the full one, and both are written by hand often enough that
 * demanding one of the two would be a rule people work around. The comparison
 * downstream is a prefix test, so a short stamp is checked at its own width.
 *
 * Line 1 is part of the contract rather than a convenience: the packaging step
 * strips this line on the `.md` -> `.txt` copy, and a stamp that has drifted
 * into the middle of a file would ship to a tester as raw HTML.
 */
const STAMP = /^<!--\s*translated-from:\s*([0-9a-f]{7,40})\s*-->\s*$/;

/** Any line that is trying to be the stamp, so a malformed one is named. */
const STAMP_ISH = /translated-from/;

// The seeded trees, which are deliberately broken and must not be walked unless
// they ARE the root. Named by path rather than by directory name, so a second
// fixture tree elsewhere has to opt in rather than inherit the skip.
const SEEDED_TREES = new Set([resolve(REPO_DEFAULT, "scripts", "fixtures")]);

function inSeededTree(abs) {
  for (const tree of SEEDED_TREES) {
    if (REPO === tree || REPO.startsWith(tree + sep)) continue; // the root is that tree
    if (abs === tree || abs.startsWith(tree + sep)) return true;
  }
  return false;
}

/** `git` in `cwd`, stdout trimmed, or null when git cannot answer. */
function git(cwd, args) {
  try {
    return execFileSync("git", args, {
      cwd,
      encoding: "utf8",
      stdio: ["ignore", "pipe", "ignore"],
      maxBuffer: 64 * 1024 * 1024,
    }).trim();
  } catch {
    return null;
  }
}

/**
 * Every tracked `.ru.md` under the root, as relative paths.
 *
 * ENUMERATED FROM GIT, for the reason check-doc-links.mjs and
 * check-comment-hygiene.mjs are: a filesystem walk cannot tell what this
 * repository owns from what happens to be sitting in a working tree, so a
 * vendored tree would fail the local push and be absent from CI's clone. The
 * cost is the same one: a translation created and not yet `git add`ed is not
 * checked, and push time is the only time this runs.
 *
 * Returns null when git cannot answer at all, rather than a set it did not
 * measure (ADR-0016).
 */
function translationFiles(root) {
  const out = git(root, ["ls-files", "-z"]);
  if (out === null) return null;
  const found = [];
  for (const path of out.split("\0")) {
    if (!path || !path.endsWith(SUFFIX)) continue;
    if (inSeededTree(resolve(root, path))) continue;
    found.push(path);
  }
  return found;
}

/**
 * Read one translation's stamp.
 * Returns `{ stamp }`, or `{ break: <detail>, line }`.
 */
function readStamp(text) {
  const lines = text.replace(/^﻿/, "").split(/\r?\n/);
  const at = lines.findIndex((l) => STAMP_ISH.test(l));

  if (at === -1) {
    return {
      line: 1,
      break:
        "carries no `<!-- translated-from: <sha> -->` stamp. Line 1 of a translation is the " +
        "commit its source was translated from; without it nothing can say whether this page " +
        "still describes the English one (ADR-0185)",
    };
  }
  const match = STAMP.exec(lines[at]);
  if (!match) {
    return {
      line: at + 1,
      break:
        `malformed stamp: \`${lines[at].trim()}\`. Expected exactly ` +
        "`<!-- translated-from: <sha> -->`, the sha 7 to 40 lowercase hex",
    };
  }
  if (at !== 0) {
    return {
      line: at + 1,
      break:
        "the stamp is not on line 1. The packaging step strips line 1 on the `.md` -> `.txt` " +
        "copy, so a stamp below it ships to a tester as raw HTML",
    };
  }
  return { stamp: match[1] };
}

/**
 * Parse every translation under `root`.
 * Returns `{ breaks, stamped }`, or `{ fatal }`.
 */
function check(root) {
  const files = translationFiles(root);
  if (files === null) {
    return { fatal: `git cannot enumerate ${root}, so the translation set was never measured` };
  }

  const breaks = [];
  const stamped = [];

  for (const file of files.sort()) {
    const source = `${file.slice(0, -SUFFIX.length)}.md`;
    if (!existsSync(join(root, source))) {
      breaks.push(
        `${file}:1 — no source beside it: ${source} does not exist. A translation is a sibling ` +
          "of the document it translates; this is what a rename looks like from here",
      );
      continue;
    }

    const read = readStamp(readFileSync(join(root, file), "utf8"));
    if (read.break) {
      breaks.push(`${file}:${read.line} — ${read.break}`);
      continue;
    }
    stamped.push({ file, source, stamp: read.stamp });
  }

  return { breaks, stamped };
}

// --- the advisory ------------------------------------------------------------
//
// Staleness is reported and never part of the exit code (ADR-0185). The row
// names the source, the stamped sha and the current one, so acting on it is a
// `git diff <stamped>..<current> -- <source>` and not an archaeology session.

/** Whether `root` is a shallow clone; false when git cannot answer. */
function isShallow(root) {
  return git(root, ["rev-parse", "--is-shallow-repository"]) === "true";
}

/** `{ sha, date }` of the commit that last touched `path`, or null. */
function lastCommit(path, root) {
  const out = git(root, ["log", "-1", "--format=%H%x09%cs", "--", path]);
  if (!out) return null;
  const [sha, date] = out.split("\t");
  return sha ? { sha, date } : null;
}

function advisory(stamped, root) {
  if (isShallow(root)) return { shallow: true, moved: [], unmeasured: [] };

  const moved = [];
  const unmeasured = [];
  for (const entry of stamped) {
    const current = lastCommit(entry.source, root);
    if (current === null) {
      unmeasured.push(entry);
      continue;
    }
    // A prefix test, because the stamp may be a short sha and the reading is
    // always full. `startsWith` on the full sha is what makes both forms legal.
    if (current.sha.startsWith(entry.stamp)) continue;
    moved.push({ ...entry, current: current.sha.slice(0, 10), date: current.date });
  }
  return { shallow: false, moved, unmeasured };
}

function printAdvisory({ shallow, moved, unmeasured }) {
  console.log("");
  console.log("advisory — reported, and never part of the exit code:");

  if (shallow) {
    console.log(
      "  staleness not measured: this is a shallow clone, where `git log -1` returns the\n" +
        "  tip commit for every path. Run it on a full checkout — the pre-push hook does.",
    );
  } else if (moved.length === 0) {
    console.log("  no translated source has moved since its translation was stamped");
  } else {
    console.log(`  ${moved.length} translation(s) whose source has moved since the stamp:`);
    for (const m of moved) {
      console.log(`    ${m.file}  stamped ${m.stamp}, ${m.source} is at ${m.current} (${m.date})`);
    }
    console.log(
      "  Re-read the diff, correct the Russian, and move the stamp in the same commit:\n" +
        "    git diff <stamped>..<current> -- <source>",
    );
  }

  for (const u of unmeasured) {
    console.log(`  ${u.file}  ${u.source} has no commit yet, so there is nothing to compare`);
  }
}

// --- self-test ---------------------------------------------------------------
//
// Two halves, because the two states this gate reports need different trees.
//
// The HARD half is textual and needs no history, so it runs against the seeded
// red tree in place and asserts the exact break count: a bare "exits non-zero"
// is also what a crash looks like.
//
// The ADVISORY half needs a repository whose history is known, which the seeded
// green tree cannot carry - the sha of the commit that will hold it is not
// knowable until it exists. So the green tree is copied into a throwaway
// repository, where its stamps are stale by construction, and then re-stamped
// from the shas that repository actually produced. Both directions are asserted:
// stale -> two rows, current -> none. The shallow reading is a real `--depth 1`
// clone of that repository rather than a simulated one.
//
// The banner plugin is exercised here too, against the same repository, and
// that is deliberate: it reads the same stamp with its own copy of the rule, in
// a project this script cannot import from (site/ has its own dependency tree).
// Pinning the two together with assertions is what stops the copies drifting.

/** Every GIT_* variable dropped: a hook runs with GIT_DIR set at the real repo. */
const CLEAN_ENV = Object.fromEntries(
  Object.entries(process.env).filter(([k]) => !k.startsWith("GIT_")),
);

function gitIn(dir, ...args) {
  return execFileSync("git", args, {
    cwd: dir,
    env: CLEAN_ENV,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  }).trim();
}

function seedRepo(dir, fixture) {
  cpSync(fixture, dir, { recursive: true });
  gitIn(dir, "init", "-q", "-b", "main");
  gitIn(dir, "config", "user.name", "self-test");
  gitIn(dir, "config", "user.email", "self-test@example.invalid");
  gitIn(dir, "config", "commit.gpgsign", "false");
  gitIn(dir, "add", "-A");
  gitIn(dir, "commit", "-q", "-m", "the sources and their translations");
}

/** This script, as a child, against `root`: `{ status, out }`. */
function runGate(root) {
  const r = spawnSync(process.execPath, [SCRIPT, root], { env: CLEAN_ENV, encoding: "utf8" });
  return { status: r.status, out: `${r.stdout}${r.stderr}` };
}

async function selfTest() {
  const results = [];
  const record = (name, ok, detail, out) => results.push({ name, ok, detail, out });

  // --- the hard half, in place -----------------------------------------------
  const red = resolve(REPO_DEFAULT, "scripts/fixtures/translations/red");
  const redRun = runGate(red);
  const redBreaks = (redRun.out.match(/^ {2}\S+\.ru\.md:\d+ —/gm) ?? []).length;
  record(
    "red fixture: four broken stamps, exit 1",
    redRun.status === 1 && redBreaks === 4,
    `exit ${redRun.status}, ${redBreaks} break(s) reported`,
    redRun.out,
  );
  for (const [what, phrase] of [
    ["no stamp at all", "carries no `<!-- translated-from: <sha> -->` stamp"],
    ["a stamp that is not a sha", "malformed stamp"],
    ["a stamp below line 1", "the stamp is not on line 1"],
    ["a translation with no source", "no source beside it"],
  ]) {
    record(`red fixture: ${what} is named`, redRun.out.includes(phrase), phrase, redRun.out);
  }

  // --- the advisory half, in a throwaway repository ---------------------------
  const dir = mkdtempSync(join(tmpdir(), "rlx-translations-"));
  const shallowDir = mkdtempSync(join(tmpdir(), "rlx-translations-shallow-"));
  try {
    seedRepo(dir, resolve(REPO_DEFAULT, "scripts/fixtures/translations/green"));

    const staleRun = runGate(dir);
    record(
      "green fixture: a stamp naming another commit is stale, exit 0",
      staleRun.status === 0 && staleRun.out.includes("2 translation(s) whose source has moved"),
      `exit ${staleRun.status}`,
      staleRun.out,
    );

    // Re-stamped from the shas this repository actually produced - the full form
    // on one pair and the short form on the other, so both widths are proven to
    // compare equal rather than only the one that happens to be written today.
    const full = gitIn(dir, "log", "-1", "--format=%H", "--", "docs/handbook.md");
    const short = gitIn(dir, "log", "-1", "--format=%h", "--", "packaging/box/READ-ME-FIRST.md");
    restamp(join(dir, "docs/handbook.ru.md"), full);
    restamp(join(dir, "packaging/box/READ-ME-FIRST.ru.md"), short);
    gitIn(dir, "commit", "-q", "-a", "-m", "stamp both translations at their sources");

    const currentRun = runGate(dir);
    record(
      "green fixture: a full sha and a short sha both read as current",
      currentRun.status === 0 &&
        currentRun.out.includes("no translated source has moved") &&
        currentRun.out.includes("2 stamped translation(s)"),
      `exit ${currentRun.status}`,
      currentRun.out,
    );

    // The stamps are current, so go back to a stale one before measuring the
    // shallow reading: a shallow clone that reports nothing because there IS
    // nothing to report proves only that the run finished.
    restamp(join(dir, "docs/handbook.ru.md"), "1".repeat(40));
    gitIn(dir, "commit", "-q", "-a", "-m", "stale again, for the shallow reading");
    const staleAgain = runGate(dir);

    gitIn(shallowDir, "clone", "-q", "--depth", "1", pathToFileURL(dir).href, "clone");
    const shallowRun = runGate(join(shallowDir, "clone"));
    record(
      "shallow clone: the staleness half is withheld, not reported wrong",
      staleAgain.out.includes("1 translation(s) whose source has moved") &&
        shallowRun.status === 0 &&
        shallowRun.out.includes("staleness not measured: this is a shallow clone") &&
        !shallowRun.out.includes("whose source has moved"),
      `full clone reports the row, shallow clone exits ${shallowRun.status}`,
      shallowRun.out,
    );

    // --- the banner, against the same repository -------------------------------
    const { translationBanner } = await import(
      pathToFileURL(resolve(REPO_DEFAULT, "site/src/plugins/translation-banner.mjs")).href
    );
    const banner = translationBanner();
    const page = (stamp) => ({
      type: "root",
      children: [
        { type: "html", value: `<!-- translated-from: ${stamp} -->` },
        { type: "heading", depth: 1, children: [{ type: "text", value: "Заголовок" }] },
        { type: "paragraph", children: [{ type: "text", value: "Текст." }] },
      ],
    });
    const file = { path: join(dir, "docs/handbook.ru.md") };
    const text = (node) =>
      node.type === "text" || node.type === "inlineCode"
        ? node.value
        : (node.children ?? []).map(text).join("");

    const currentSha = gitIn(dir, "log", "-1", "--format=%H", "--", "docs/handbook.md");
    const currentDate = gitIn(dir, "log", "-1", "--format=%cs", "--", "docs/handbook.md");

    const stale = page("1".repeat(40));
    banner(stale, file);
    const notice = stale.children[1];
    record(
      "banner: a stale page gets a dated notice under its heading, and loses the stamp",
      stale.children[0].type === "heading" &&
        notice?.type === "blockquote" &&
        text(notice).includes(currentDate) &&
        text(notice).includes(currentSha.slice(0, 7)),
      notice === undefined ? "nothing inserted" : `${notice.type}: ${text(notice)}`,
    );

    const current = page(currentSha);
    banner(current, file);
    record(
      "banner: a current page gets no notice, and loses the stamp",
      current.children.length === 2 &&
        current.children[0].type === "heading" &&
        current.children[1].type === "paragraph",
      current.children.map((c) => c.type).join(", "),
    );

    let threw = null;
    try {
      banner(
        { type: "root", children: [{ type: "paragraph", children: [] }] },
        { path: join(dir, "docs/handbook.ru.md") },
      );
    } catch (e) {
      threw = e.message;
    }
    record(
      "banner: an unstamped translation THROWS rather than rendering",
      threw !== null && threw.includes("translated-from"),
      threw ?? "returned normally, so the site would publish an unstamped page",
    );

    const shallowStale = page("1".repeat(40));
    banner(shallowStale, { path: join(shallowDir, "clone/docs/handbook.ru.md") });
    record(
      "banner: a shallow build publishes no notice at all",
      shallowStale.children.length === 2 && shallowStale.children[1].type === "paragraph",
      shallowStale.children.map((c) => c.type).join(", "),
    );
  } finally {
    rmSync(dir, { recursive: true, force: true });
    rmSync(shallowDir, { recursive: true, force: true });
  }

  for (const r of results) {
    console.log(`  ${r.ok ? "ok  " : "FAIL"} ${r.name}`);
    if (!r.ok) {
      console.log(`       ${r.detail}`);
      for (const l of (r.out ?? "").trim().split(/\r?\n/)) if (l) console.log(`       | ${l}`);
    }
  }
  const passed = results.filter((r) => r.ok).length;
  console.log(`translations self-test: ${passed} of ${results.length}`);
  // The count is asserted, not merely printed: a case deleted along with the
  // rule it pinned would otherwise leave this run green and shorter.
  process.exit(passed === results.length && results.length === 12 ? 0 : 1);
}

/** Rewrite line 1's stamp in place, leaving the rest of the file alone. */
function restamp(path, sha) {
  const lines = readFileSync(path, "utf8").split(/\r?\n/);
  lines[0] = `<!-- translated-from: ${sha} -->`;
  writeFileSync(path, lines.join("\n"));
}

// --- main --------------------------------------------------------------------

if (SELF_TEST) await selfTest();

const { fatal, breaks, stamped } = check(REPO);

if (fatal) {
  console.error(`translations: ${fatal}`);
  process.exit(1);
}

const summary = advisory(stamped, REPO);

if (breaks.length === 0) {
  console.log(`translations: OK — ${stamped.length} stamped translation(s)`);
  console.log(
    "              green means every translation says which commit it was made from,\n" +
      "              never that it still says what that commit says",
  );
  printAdvisory(summary);
  process.exit(0);
}

console.error(`translations: ${breaks.length} broken`);
for (const b of breaks) console.error(`  ${b}`);
printAdvisory(summary);
console.error(
  "\nA stamp is the only thing standing behind a translated page (ADR-0185). Drift is\n" +
    "an advisory above and never a failure; a MISSING or MALFORMED stamp is this exit\n" +
    "code, because it is mechanical rather than a judgement about prose.\n" +
    "\n" +
    "Line 1 of `<name>.ru.md`, and the sha is the source's last-touching commit:\n" +
    "    <!-- translated-from: $(git log -1 --format=%h -- <name>.md) -->",
);
process.exit(1);
