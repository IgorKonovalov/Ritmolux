#!/usr/bin/env node
// Assert that the reader documents cite by link, never by bare number.
//
// The repository's citation convention is deliberate: ADR-0127 requires a code
// comment to name the record that earned a claim — `ADR-0046`, `Plan 0045 Phase
// 3` — so a threshold can be traced to the measurement behind it. It works, and
// it stays. What it is not is a register for the five documents a preset author
// reads: 235 bare citations across three of them addressed a session
// reconstructing why a number exists, and were read by someone who wanted to
// know what a parameter does.
//
// So those five documents take the opposite rule, and this gate is it:
//
//     Keep the fact. Demote the provenance to the link.
//
// A citation inside a markdown link is fine and always was — a link is inert
// until clicked, and it is the escape hatch for a reader who does want the
// reasoning. What is rejected is the BARE number, because that is the form that
// interrupts a sentence. See ADR-0168.
//
// Usage:  node scripts/check-reader-prose.mjs [root]
// Exit 0 = every citation in the five documents is inside a link. Exit 1 = the
// bare ones are listed as `file:line  <citation>`, clickable in most terminals.
// The optional `root` scans some other directory, following the checkers beside
// it: `node scripts/check-reader-prose.mjs scripts/fixtures/reader-prose`
// expects exit 1 and six breaks. CI and the pre-push hook pass nothing.
//
// WHY A GATE AND NOT A CONVENTION. The convention was tried: `CLAUDE.md` has
// asked for bare-number citation in documentation for as long as it has existed,
// and that is precisely how these five accumulated 235 of them. The same
// substitution has already been made twice in this repository for the same
// reason — close-ceremony archiving and index-row length were both conventions
// first, and both needed a gate (ADR-0116).
//
// SCOPE IS ENTRANCE A, and the boundary is the list below rather than a rule a
// script could infer. Entrance B — `docs/capturing.md`, `docs/nfr.md`,
// `docs/on-device-validation.md`, `docs/releasing.md`, the specs and the
// technique catalogue — KEEPS its bare citations, because its readers are
// contributors for whom the working record is the point rather than the noise.
// Two rules now apply in two places and the seam is this array; that cost is
// named in ADR-0168's Consequences rather than discovered here.
//
// THE PLURAL IS MATCHED, and it is not a flourish. `### Ink on paper — ...
// (Plans 0027, 0078)` sat in the roster carrying two plan numbers into its own
// route name, invisible to every instrument in the repository: this gate's
// singular form would have missed it, and so does the build-time strip in
// `site/src/plugins/strip-provenance.mjs`, whose pattern is the singular. One
// heading is enough of a demonstration that the form occurs.
//
// THE GATE'S OWN HOLES, named here rather than left to be discovered:
//   1. Fenced code is not scanned. A path like `docs/plans/0155-…` inside a
//      shell block is a command, not a claim, and the reader documents are full
//      of runnable commands.
//   2. A citation spelled in words ("the plan that measured it") is not matched,
//      and cannot be. The gate removes a mechanical form; whether a rewrite is
//      GOOD is a Mode 4 review's judgement, exactly as comment length is under
//      ADR-0127.
//   3. `design-backlog NNNN` is not matched. It is a different corpus with a
//      different form, and ADR-0149 already governs how it may be written.

import { readFileSync, existsSync } from "node:fs";
import { join, dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const REPO_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const REPO = resolve(process.argv[2] ?? REPO_ROOT);

/**
 * The Entrance A set: the documents a preset author reads.
 *
 * Held as a literal list on purpose. "Reader-facing" is not a property a script
 * can infer from a path, and a heuristic that tried would either drag in
 * Entrance B or quietly drop a file the day it was renamed.
 */
const READER_DOCS = [
  "presets/README.md",
  "docs/presets.md",
  "docs/preset-palettes.md",
  "docs/preset-guide.md",
  "docs/preset-tuning-walkthrough.md",
];

/**
 * A citation: `ADR-0046`, `Plan 0045`, `Plan 0045 Phase 3`, and both plurals.
 *
 * The separator class after `ADR` carries the three unicode dashes as well as
 * the hyphen, because a document that has been through an editor's smart-dash
 * pass writes `ADR‑0046` with U+2011 and reads identically.
 */
const CITATION = /\b(?:Plans?[‑–— -]?\d{4}|ADRs?[‑–— -]\d{4})\b/g;

/**
 * Blank out every markdown link construct, preserving offsets.
 *
 * Offsets are preserved rather than the text rebuilt, so a match's index still
 * points at its real column and the reported line stays the line the author is
 * looking at. The four forms, in the order they must be masked:
 *
 *   1. `[text](target)` — inline. Masked whole: a citation is allowed in either
 *      half, and `[Plan 0154](done/0154-….md)` is the commonest shape here.
 *   2. `[text][label]`  — full reference.
 *   3. `[label]: target` — the definition line, which is not prose at all.
 *   4. `[ADR-0098]`     — collapsed/shortcut reference, whose target is a
 *      definition elsewhere in the file. This one is easy to forget and the
 *      roster uses it: the citation IS the link text, so a gate that only knew
 *      the first three forms would convict a correct link.
 *
 * Form 4 is masked only when the file actually defines that label, so a bare
 * `[Plan 0154]` with no definition — which renders as literal brackets — is
 * still caught.
 */
function maskLinks(src) {
  const blank = (m) => " ".repeat(m.length);

  // The `>` in the prefix is load-bearing: the roster's one collapsed reference
  // has its definition inside a blockquote, and a definition the scanner cannot
  // see makes every USE of that label look bare.
  const DEFINITION = /^[ \t]*(?:>[ \t]*)*\[([^\]\n]+)\]:/gm;

  const labels = new Set();
  for (const m of src.matchAll(DEFINITION)) {
    labels.add(m[1].trim().toLowerCase());
  }

  return src
    .replace(/\[[^\]\n]*\]\([^)\n]*\)/g, blank)
    .replace(/\[[^\]\n]*\]\[[^\]\n]*\]/g, blank)
    .replace(/^[ \t]*(?:>[ \t]*)*\[[^\]\n]+\]:.*$/gm, blank)
    .replace(/\[([^\]\n]+)\]/g, (m, label) =>
      labels.has(label.trim().toLowerCase()) ? blank(m) : m,
    );
}

/** Blank out fenced code, preserving offsets. A command is not a claim. */
function maskFences(src) {
  const lines = src.split("\n");
  let inFence = false;
  return lines
    .map((line) => {
      if (/^\s*(```|~~~)/.test(line)) {
        inFence = !inFence;
        return " ".repeat(line.length);
      }
      return inFence ? " ".repeat(line.length) : line;
    })
    .join("\n");
}

const missing = [];
const findings = [];
const scanned = [];

for (const doc of READER_DOCS) {
  const path = join(REPO, doc);
  if (!existsSync(path)) {
    missing.push(doc);
    continue;
  }

  const src = readFileSync(path, "utf8");
  const masked = maskLinks(maskFences(src));
  const lines = src.split("\n");

  // One pass over the masked text; the line number comes from counting newlines
  // before the match, which is exact because masking preserved every offset.
  let bare = 0;
  for (const m of masked.matchAll(CITATION)) {
    const line = masked.slice(0, m.index).split("\n").length;
    findings.push({ doc, line, citation: m[0], text: lines[line - 1].trim() });
    bare += 1;
  }

  const total = (maskFences(src).match(CITATION) ?? []).length;
  scanned.push(`  ${doc}: ${total} citation(s), ${bare} bare`);
}

console.log("reader prose: the Entrance A documents");
for (const s of scanned) console.log(s);

if (missing.length > 0) {
  console.error(`\nreader prose: ${missing.length} document(s) in the list do not exist`);
  for (const d of missing) console.error(`  ${d}`);
  console.error(
    "\nThe list in this script IS the scope boundary (ADR-0168). A renamed or\n" +
      "retired reader document must be renamed here in the same commit, or the\n" +
      "gate silently stops covering it.",
  );
  process.exit(1);
}

if (findings.length === 0) {
  console.log(
    `reader prose: OK (every citation in ${READER_DOCS.length} documents is inside a link)`,
  );
  process.exit(0);
}

console.error(`\nreader prose: ${findings.length} bare citation(s)`);
for (const f of findings) {
  console.error(`  ${f.doc}:${f.line}  ${f.citation}`);
  console.error(`    ${f.text.length > 96 ? f.text.slice(0, 93) + "…" : f.text}`);
}
console.error(
  "\nThese five documents cite by link, never by bare number (ADR-0168):\n" +
    "\n" +
    "    Keep the fact. Demote the provenance to the link.\n" +
    "\n" +
    "A sentence states what the software does and what the number is. The plan or\n" +
    "ADR that earned it becomes a link on that sentence, or is dropped where the\n" +
    "sentence stands on its own. What goes is the narration of the decision —\n" +
    "which plan measured it, in which phase, what it superseded — never the\n" +
    "measurement.\n" +
    "\n" +
    "  before:  Measured at Plan 0063 Phase 5; `depth_fade` above 0.8 flattens the\n" +
    "           figure because the far end stops separating.\n" +
    "  after:   `depth_fade` above 0.8 flattens the figure: the far end of the\n" +
    "           attractor stops separating from the near end.\n" +
    "\n" +
    "This rule is scoped to these five. Every code comment and every Entrance B\n" +
    "document keeps bare-number citation, which is what ADR-0127 and CLAUDE.md ask\n" +
    "for everywhere else.",
);
process.exit(1);
