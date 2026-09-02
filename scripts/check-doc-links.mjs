#!/usr/bin/env node
// Verify every relative markdown link in the repo resolves to a file that exists.
//
// Rationale: the architect close ceremony `git mv`s a finished plan into
// docs/plans/done/, which breaks links in BOTH directions — every document that
// named the plan at its old path, and every `../adrs/...` link inside the plan
// itself, which now resolves one directory too high. Markdown link rot degrades
// silently and only in a browser, so nothing surfaced it: by Plan 0060's close
// it had accumulated to 74 broken links across 23 files, from six consecutive
// closes. See the `architect` skill, "Close-ceremony bookkeeping" step 1b.
//
// Usage:  node scripts/check-doc-links.mjs [root]
// Exit 0 = every relative link resolves. Exit 1 = the broken ones are listed as
// `file:line -> target`, which is clickable in most terminals. The optional
// `root` scans some other directory — used to run this against the committed
// fixture tree (Plan 0084 Phase 1 built the argument, Plan 0093 Phase 1 finally
// committed the tree): `node scripts/check-doc-links.mjs scripts/fixtures`
// expects exit 1 and exactly five breaks, across the four classes below. CI and
// the pre-push hook pass nothing and get the repo.
//
// Four break classes, and the first is the only one this script had until
// Plan 0084. Markdown has two link forms and checking one of them was a green
// light over 85 broken links of the other:
//
//   1. inline       [text](target)          — target must exist
//   2. definition   [label]: target         — target must exist
//   3. use          [label] / [text][label] — the file must define `label`,
//                                             or it renders as literal brackets
//   4. backlog      ](design-backlog.md#…)  — the fragment form is prohibited
//
// Class 3 is what a close ceremony breaks when it moves link-dense prose between
// files: the *uses* travel with the paragraph and the *definitions* stay behind
// (Plan 0061 Phase 7b did exactly this to 62 links). It is scoped per file
// because that is markdown's own scope — a definition in README.md does nothing
// for a use in README-archive.md.
//
// Still deliberately narrow: only that the *file* exists — not `#anchor`
// fragments, not external URLs (which would make this a network call and a
// flake).
//
// ONE FRAGMENT IS REJECTED OUTRIGHT, as a fourth break class (ADR-0149). A link
// into docs/design-backlog.md or its archive keeps resolving to a file that
// exists after the close ceremony moves an entry's body from the one to the
// other, so this script reported 87 such references clean — every one of them
// landing at the top of a 280 KB document instead of at the entry. The anchor is
// the part that rots and the number is not: an entry's heading text IS its
// anchor, headings get reworded, and the ledger row that replaces an archived
// body carries the number but not the old heading, so even a fragment checker
// would only convert silent rot into loud rot at every close. The form is
// therefore prohibited rather than validated — `[backlog 0072](../design-backlog.md)`,
// or in prose that already links the file, the bare number. Same answer ADR-0127
// reached one level down for `.rs` comments: a reference whose form cannot be
// checked is replaced by one that cannot break.
//
// This is NOT a general fragment checker, and ADR-0149 records that it made one
// less likely. Every other `#anchor` in the repository stays unchecked.
//
// Code is skipped — fenced blocks and inline spans alike — because a document
// that *describes* link syntax is not making a link. This file's own prose in
// the architect skill was the first false positive.

import { readdirSync, readFileSync, existsSync } from "node:fs";
import { join, dirname, resolve, relative, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { execFileSync } from "node:child_process";

const REPO_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const REPO = resolve(process.argv[2] ?? REPO_ROOT);

// Build output, vendored deps, and VCS internals hold markdown we do not own.
const SKIP_DIRS = new Set(["target", "node_modules", ".git"]);

// A seeded tree holds deliberately broken links as this checker's own bite check
// (Plan 0093 Phase 1, which committed the tree the `root` argument above was
// built for and never got). Skip it on a repo walk, where those seeds would red
// the gate for everyone; scan it when it IS the root, which is the only way they
// are reachable.
//
// Skipped BY PATH, and enumerated here rather than matched by directory name.
// The name form also swallowed core/tests/fixtures/ and its twelve relative
// links — nine markdown files silently out of the walk under a green gate, which
// is the shape this gate exists to catch (Plan 0094 Phase 1). A second seeded
// tree has to name itself on this list.
const SEEDED_TREES = new Set([resolve(REPO_ROOT, "scripts", "fixtures")]);

/** Whether an absolute path sits inside a seeded tree that is not the scan root. */
function inSeededTree(abs) {
  for (const tree of SEEDED_TREES) {
    if (REPO === tree || REPO.startsWith(tree + sep)) continue; // the root is that tree
    if (abs === tree || abs.startsWith(tree + sep)) return true;
  }
  return false;
}

/**
 * Every `.md` file the REPOSITORY holds under the root, as relative paths.
 *
 * ENUMERATED FROM GIT for the same reason check-comment-hygiene.mjs is: a
 * filesystem walk cannot tell code we own from code sitting in the working tree
 * that git ignores, so a gitignored vendored README is absent from CI's fresh
 * clone and present in every working tree — CI green by construction, the local
 * push blocked, with a diagnostic pointing at files nobody here wrote. This gate
 * was green only because neither vendored tree happened to carry a
 * relative-linked `.md`, which is luck rather than a property: seeding one
 * `.venv/pkg/README.md` with two relative links reports both and exits 1.
 *
 * The consequence is the same one: a markdown file git has never seen is not
 * checked, and at push time — the only time this runs — that window is a file
 * created and not yet added.
 *
 * Falls back to the filesystem walk when git cannot answer, and says so, rather
 * than reporting a set it did not measure (ADR-0016).
 */
function markdownFiles() {
  try {
    const out = execFileSync("git", ["ls-files", "-z"], {
      cwd: REPO,
      encoding: "utf8",
      stdio: ["ignore", "pipe", "ignore"],
      maxBuffer: 64 * 1024 * 1024,
    });
    const found = [];
    for (const path of out.split("\0")) {
      if (!path || !path.endsWith(".md")) continue;
      if (inSeededTree(resolve(REPO, path))) continue;
      found.push(path);
    }
    return { files: found, source: "git" };
  } catch {
    return { files: walk(), source: "filesystem" };
  }
}

/** The pre-git enumeration, kept as the fallback for a tree git cannot answer for. */
function walk(dir = REPO, found = []) {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const full = join(dir, entry.name);
    if (entry.isDirectory()) {
      if (SKIP_DIRS.has(entry.name)) continue;
      if (SEEDED_TREES.has(full)) continue;
      walk(full, found);
    } else if (entry.name.endsWith(".md")) {
      found.push(relative(REPO, full));
    }
  }
  return found;
}

// `](target)` where target is not a URL, a mailto:, or a bare `#anchor`.
// The target runs to the first `)`, `#`, or whitespace.
const LINK = /\]\((?!https?:|mailto:|#)([^)#\s]+)/g;

// The one prohibited fragment, in both link forms. Matched on the filename
// rather than on a resolved path, because the offending link is offending
// wherever it points from and the target need not exist for the form to be
// wrong. The `#` is required: dropping it is the whole repair.
const BACKLOG_FRAGMENT = /\]\(([^)\s]*design-backlog(?:-archive)?\.md)#([^)\s]*)\)/g;
const BACKLOG_FRAGMENT_DEFINITION = /^ {0,3}\[([^\]]+)\]:\s*([^\s]*design-backlog(?:-archive)?\.md)#(\S*)/;

// A link reference definition: up to three spaces, `[label]:`, then the target.
// An optional `"title"` may follow, which is why the target is one token.
const DEFINITION = /^ {0,3}\[([^\]]+)\]:\s*(\S+)/;

// A reference *use*, in markdown's three spellings:
//   [text][label]   full        — the label is the second bracket
//   [label][]       collapsed   — the label is the first
//   [label]         shortcut    — the label is the bracket itself
// A `[` not followed by `]` `(` or `:` is the shortcut form; the negative
// lookahead is what keeps inline links and definitions out of this class.
const USE = /\[([^\][]+)\](?:\[([^\][]*)\])?/g;

/** Markdown matches reference labels case-insensitively, on collapsed space. */
const normalize = (label) => label.trim().toLowerCase().replace(/\s+/g, " ");

/** A relative target that must resolve to a file on disk. */
const isRelativeTarget = (t) => !/^(https?:|mailto:|#)/.test(t);

/**
 * Not every bracket in prose is a link. A shortcut use is only reported when
 * *some* file in the tree defines that label — which is the difference between
 * a paragraph that lost its definitions in a move and a table cell that happens
 * to hold brackets. A label nothing anywhere defines was never a link.
 */
function collect() {
  const { files, source: enumeration } = markdownFiles();
  const parsed = new Map(); // file -> { definitions: Map, uses: [], inline: [] }
  const known = new Set(); // every label defined anywhere in the tree

  for (const file of files) {
    const base = dirname(join(REPO, file));
    const lines = readFileSync(join(REPO, file), "utf8").split(/\r?\n/);
    const definitions = new Map(); // label -> { line, target }
    const uses = [];
    const inline = [];
    const fragments = [];
    let inFence = false;

    lines.forEach((raw, i) => {
      if (/^\s*(```|~~~)/.test(raw)) {
        inFence = !inFence;
        return;
      }
      if (inFence) return;
      // Blank out inline code spans so prose describing link syntax is not a link.
      const line = raw.replace(/`[^`]*`/g, (s) => " ".repeat(s.length));

      for (const m of line.matchAll(LINK)) {
        // Decode %20 and friends; a malformed escape is the author's, not ours.
        let target = m[1];
        try {
          target = decodeURIComponent(target);
        } catch {
          /* leave it as written */
        }
        inline.push({ line: i + 1, target, written: m[1], base });
      }

      BACKLOG_FRAGMENT.lastIndex = 0;
      for (const m of line.matchAll(BACKLOG_FRAGMENT)) {
        fragments.push({ line: i + 1, target: m[1], fragment: m[2] });
      }
      const fragDef = line.match(BACKLOG_FRAGMENT_DEFINITION);
      if (fragDef) {
        fragments.push({ line: i + 1, target: fragDef[2], fragment: fragDef[3] });
      }

      const def = line.match(DEFINITION);
      if (def) {
        const label = normalize(def[1]);
        known.add(label);
        if (!definitions.has(label)) {
          definitions.set(label, { line: i + 1, target: def[2], base });
        }
        return; // a definition line is not also a use of its own label
      }

      for (const m of line.matchAll(USE)) {
        // `[text](target)` is the inline class, already collected above.
        if (line[m.index + m[0].length] === "(") continue;
        // A footnote or a task-list checkbox is not a reference link.
        if (m[1].startsWith("^")) continue;
        if (/^[ xX]$/.test(m[1])) continue;
        const label = normalize(m[2] ? m[2] : m[1]);
        if (label) uses.push({ line: i + 1, label, written: m[2] || m[1] });
      }
    });

    parsed.set(file, { definitions, uses, inline, fragments });
  }

  return { parsed, known, enumeration };
}

const { parsed, known, enumeration } = collect();
const broken = [];

for (const [file, { definitions, uses, inline, fragments }] of parsed) {
  const show = file.split(sep).join("/");

  for (const { line, target, written, base } of inline) {
    if (!existsSync(resolve(base, target))) {
      broken.push(`${show}:${line} -> ${written}`);
    }
  }

  for (const [label, { line, target, base }] of definitions) {
    if (!isRelativeTarget(target)) continue;
    // A definition's target runs to whitespace, so it carries any `#fragment`
    // with it; the inline form's regex stops at the `#`. Strip it here so the
    // two forms answer the same question — this class is "does the FILE exist",
    // and without the strip an anchored definition is reported as a missing
    // path, which names the wrong defect and reports one line twice.
    let decoded = target.split("#")[0];
    try {
      decoded = decodeURIComponent(decoded);
    } catch {
      /* leave it as written */
    }
    if (decoded !== "" && !existsSync(resolve(base, decoded))) {
      broken.push(`${show}:${line} -> [${label}]: ${target}`);
    }
  }

  for (const { line, target, fragment } of fragments) {
    broken.push(`${show}:${line} -> ${target}#${fragment} (a backlog reference carries no fragment)`);
  }

  const reported = new Set();
  for (const { line, label, written } of uses) {
    if (definitions.has(label)) continue;
    if (!known.has(label)) continue; // ordinary brackets, not a lost link
    const key = `${line}:${label}`;
    if (reported.has(key)) continue;
    reported.add(key);
    broken.push(`${show}:${line} -> [${written}] (no definition in this file)`);
  }
}

if (enumeration !== "git") {
  console.log(
    "note: git could not list this tree, so the file set came from a filesystem\n" +
      "      walk. That set includes anything gitignored sitting in the working\n" +
      "      tree — a vendored README, a virtualenv — whose links are not ours to\n" +
      "      judge and which no checkout contains.",
  );
}

if (broken.length === 0) {
  console.log(`doc links: OK (every relative markdown link in ${parsed.size} tracked files resolves)`);
  process.exit(0);
}

console.error(`doc links: ${broken.length} broken`);
for (const b of broken) console.error(`  ${b}`);
console.error(
  "\nA plan that moved to docs/plans/done/ breaks links in both directions:\n" +
    "  inbound   ../plans/NNNN-... -> ../plans/done/NNNN-...   (and (NNNN-...) -> (done/NNNN-...))\n" +
    "  outbound  inside the moved plan, ../adrs/... -> ../../adrs/...\n" +
    "            and its links to still-active plans, (NNNN-...) -> (../NNNN-...)\n" +
    "A bare NNNN-*.md link inside docs/adrs/ is identified by its NUMBER, not its\n" +
    "slug — unless the prose says \"Plan NNNN\", where the missing piece is the\n" +
    "../plans/ prefix rather than the filename.\n" +
    "\n" +
    "`[label] (no definition in this file)` is the third class: the use travelled\n" +
    "to another file and its `[label]: target` definition stayed behind. Copy the\n" +
    "definition into this file — markdown scopes them per document.\n" +
    "\n" +
    "`(a backlog reference carries no fragment)` is the fourth class, and it is a\n" +
    "form rule rather than a resolution failure (ADR-0149): the target exists,\n" +
    "which is why 87 of these read clean. Drop the `#...` and keep the number:\n" +
    "  [backlog 0072](../design-backlog.md)\n" +
    "or, where the file is already linked in the same breath, just `backlog 0072`.\n" +
    "An entry's heading IS its anchor, and the close ceremony moves the body to\n" +
    "design-backlog-archive.md leaving a ledger row that carries the number and\n" +
    "not the heading — so the anchor is the half that cannot survive. Every other\n" +
    "`#anchor` in this repository stays unchecked; this is one prohibited form,\n" +
    "not a fragment checker.",
);
process.exit(1);
