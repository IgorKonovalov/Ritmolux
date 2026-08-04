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
// Usage:  node scripts/check-doc-links.mjs
// Exit 0 = every relative link resolves. Exit 1 = the broken ones are listed as
// `file:line -> target`, which is clickable in most terminals.
//
// Deliberately narrow. It checks inline links `[text](target)` only, and only
// that the *file* exists — not `#anchor` fragments, not reference-style links,
// not external URLs (which would make this a network call and a flake).
//
// Code is skipped — fenced blocks and inline spans alike — because a document
// that *describes* link syntax is not making a link. This file's own prose in
// the architect skill was the first false positive.

import { readdirSync, readFileSync, statSync, existsSync } from "node:fs";
import { join, dirname, resolve, relative, sep } from "node:path";
import { fileURLToPath } from "node:url";

const REPO = resolve(dirname(fileURLToPath(import.meta.url)), "..");

// Build output, vendored deps, and VCS internals hold markdown we do not own.
const SKIP_DIRS = new Set(["target", "node_modules", ".git"]);

/** Every `.md` file in the repo, as paths relative to the repo root. */
function markdownFiles(dir = REPO, found = []) {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const full = join(dir, entry.name);
    if (entry.isDirectory()) {
      if (!SKIP_DIRS.has(entry.name)) markdownFiles(full, found);
    } else if (entry.name.endsWith(".md")) {
      found.push(relative(REPO, full));
    }
  }
  return found;
}

// `](target)` where target is not a URL, a mailto:, or a bare `#anchor`.
// The target runs to the first `)`, `#`, or whitespace.
const LINK = /\]\((?!https?:|mailto:|#)([^)#\s]+)/g;

const broken = [];
for (const file of markdownFiles()) {
  const base = dirname(join(REPO, file));
  const lines = readFileSync(join(REPO, file), "utf8").split(/\r?\n/);
  let inFence = false;
  lines.forEach((line, i) => {
    if (/^\s*(```|~~~)/.test(line)) {
      inFence = !inFence;
      return;
    }
    if (inFence) return;
    // Blank out inline code spans so prose describing link syntax is not a link.
    line = line.replace(/`[^`]*`/g, (s) => " ".repeat(s.length));
    for (const m of line.matchAll(LINK)) {
      // Decode %20 and friends; a malformed escape is the author's, not ours.
      let target = m[1];
      try {
        target = decodeURIComponent(target);
      } catch {
        /* leave it as written */
      }
      if (!existsSync(resolve(base, target))) {
        broken.push(`${file.split(sep).join("/")}:${i + 1} -> ${m[1]}`);
      }
    }
  });
}

if (broken.length === 0) {
  console.log("doc links: OK (every relative markdown link resolves)");
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
    "../plans/ prefix rather than the filename.",
);
process.exit(1);
