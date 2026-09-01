#!/usr/bin/env node
// Reject the two mechanical rot classes in comments: a relative link, and
// plan-relative narration. Rust and C/C++ sources are both read.
//
// Rationale: a relative link in a `.rs` comment fails silently in three ways at
// once. It breaks when a plan moves to docs/plans/done/, which is a routine step
// of every close; check-doc-links.mjs cannot see it, because that script walks
// `.md` only; and it does not resolve in rendered rustdoc at all, where the href
// is emitted as written and resolves against the generated HTML tree. Eleven were
// broken on main when ADR-0127 was written. Plan-relative narration — `this
// plan`, `used to`, `no longer` — is written from inside a session and stops
// being legible at its close: there is no "this plan" left, there is only the
// code. See ADR-0127 for the decision and its four rejected alternatives.
//
// The vocabulary covers TWO shapes, and the second is the one that survives a
// rewrite of the first. `this plan` is easy to spot and easy to cut. `before
// Plan 0038 Phase 2 bound it` reads like a citation, passes a word list built
// out of `this plan`, and is narration all the same: it dates the code against
// an event, so a reader has to reconstruct a history to decode a sentence about
// the present. An elapsed-time preposition in front of a numbered citation —
// before / since / until / pre- / after — is therefore reported, while the bare
// citation it decorates is not.
//
// C and C++ sources are walked for the same two classes. The shim is compiled
// separately from the core and drifts the same way; a gate that reads only the
// Rust half convicts one lane of a two-lane codebase.
//
// Usage:  node scripts/check-comment-hygiene.mjs [root]
// Exit 0 = no finding. Exit 1 = they are listed as `file:line -> reason`, which
// is clickable in most terminals. The optional `root` scans some other directory
// — the way this is run against the committed fixture tree, following
// check-doc-links.mjs: `node scripts/check-comment-hygiene.mjs scripts/fixtures`
// expects exit 1, and scripts/fixtures/README.md carries the per-file roster of
// what it should report. CI and the pre-push hook pass nothing and get the repo.
//
// What is NOT a finding, and each silence is load-bearing:
//
//   - a rustdoc INTRA-DOC link, `[Self::render]` or `[x](crate::render)`. rustc
//     resolves those, so they cannot rot silently, and they are the linking
//     mechanism ADR-0127 deliberately keeps. Only a target starting `./` or `../`
//     is reported.
//   - a bare-number citation, `Plan 0045 Phase 3`. That is the form ADR-0127
//     replaces the links WITH, so a gate that fired on the word "plan" would
//     convict the fix.
//   - anything outside a comment. A `//` inside a string literal is not a
//     comment, which is why this reads Rust rather than grepping lines.
//
// COMMENT LENGTH IS NOT CHECKED, by any threshold, in any file. ADR-0127
// Alternative B: length correlates weakly with the defect, fires on headers that
// are mostly genuine mechanism, and is gamed by inserting a blank line — which
// teaches authors to fragment prose rather than cut it. That judgement belongs to
// the architect's Mode 4 review.
//
// The escape hatch: `hygiene-allow: <reason>` in a comment suppresses findings on
// its own line and on the line after it. The reason is required — an escape
// without one is itself reported, because an unexplained escape is how a gate
// stops meaning anything. The vocabulary list has real false positives ("the
// value used to compute the knee"), and the escape exists for the residue after
// rewriting, not instead of rewriting: if the count printed on success is more
// than a handful, the word list is wrong and should be narrowed.

import { readdirSync, readFileSync } from "node:fs";
import { join, dirname, resolve, relative, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { execFileSync } from "node:child_process";

const REPO_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const REPO = resolve(process.argv[2] ?? REPO_ROOT);

// Only the fallback walk below reaches these. Build output and VCS internals
// hold sources we do not own; the enumeration this script actually uses is git's,
// which excludes them without being told to.
const SKIP_DIRS = new Set(["target", "node_modules", ".git"]);

// The fixture tree carries this checker's own bite check and is skipped on a
// repo walk, exactly as check-doc-links.mjs skips it; it is scanned when it IS
// the root, which is the only way its seeded findings are reachable. Skipped BY
// PATH, not by directory name — the name form also swallowed
// core/tests/fixtures/ there.
//
// This skip is independent of where the file list comes from: these fixtures ARE
// tracked, so `git ls-files` reaches them and they still have to be excluded
// whenever they are not the root.
const SEEDED_TREES = new Set([resolve(REPO_ROOT, "scripts", "fixtures")]);

// Which lexer an extension gets. The two dialects differ in three places that
// matter to a comment scanner — Rust block comments nest and C's do not, the raw
// string syntaxes are unrelated, and Rust's `'` is a lifetime far more often
// than a char literal — so the extension picks the rules rather than one lexer
// guessing.
const LANGS = new Map([
  [".rs", "rust"],
  [".c", "c"],
  [".h", "c"],
  [".cc", "c"],
  [".cpp", "c"],
  [".hpp", "c"],
]);

/** The lexer for a path's extension, or undefined if this gate does not read it. */
function langOf(path) {
  const dot = path.lastIndexOf(".");
  return dot < 0 ? undefined : LANGS.get(path.slice(dot));
}

/**
 * Whether an absolute path sits inside a seeded tree that is not the scan root.
 *
 * The root itself is never skipped, which is the whole reason the `root`
 * argument exists — the seeded findings are reachable only by pointing this
 * script at them. The filesystem walk got that free by only ever testing a
 * CHILD directory; enumerating a flat list has to say it.
 */
function inSeededTree(abs) {
  for (const tree of SEEDED_TREES) {
    if (REPO === tree || REPO.startsWith(tree + sep)) continue; // the root is that tree
    if (abs === tree || abs.startsWith(tree + sep)) return true;
  }
  return false;
}

/**
 * Every source file the REPOSITORY holds under the root, as `[path, lang]` pairs.
 *
 * ENUMERATED FROM GIT, NOT FROM THE FILESYSTEM, which makes "code we own" and
 * "code this gate judges" the same set by construction and costs one call. A
 * filesystem walk cannot tell them apart: a gitignored tree is absent from CI's
 * fresh clone and present in every working tree, so the CI job is green by
 * construction and the local push is not. That is not hypothetical — this gate
 * went from green to 490 findings between two pushes twenty minutes apart with
 * no commit touching it, 419 of them in `.venv/`'s torch, numpy and markupsafe
 * headers and 71 in the unpacked foobar2000 SDK, none of it written here. The
 * natural escape is `--no-verify`, and that is what makes the class worth
 * closing rather than the instances: a gate that fires on vendor code teaches
 * its users to skip the gate that fires on theirs. Patching the two trees by
 * name fixed those two and left the next `pip install` to re-break it.
 *
 * The consequence to know: a source file that git has never seen is not judged.
 * At push time — the only time this runs — every file in the push is committed,
 * so the window is a file created and not yet added, which is also the window in
 * which no gate in this repository can see it.
 */
function sourceFiles() {
  try {
    const out = execFileSync("git", ["ls-files", "-z"], {
      cwd: REPO,
      encoding: "utf8",
      stdio: ["ignore", "pipe", "ignore"],
      maxBuffer: 64 * 1024 * 1024,
    });
    const found = [];
    for (const path of out.split("\0")) {
      if (!path) continue;
      const lang = langOf(path);
      if (!lang) continue;
      if (inSeededTree(resolve(REPO, path))) continue;
      found.push([path, lang]);
    }
    return { files: found, source: "git" };
  } catch {
    return { files: walk(), source: "filesystem" };
  }
}

/**
 * The pre-git enumeration, kept as the fallback for a tree git cannot answer for
 * — no git on PATH, or a source drop that is not a checkout. It is the shape
 * that produced the 490 findings, so a run that falls back says so rather than
 * letting a reader assume the tracked set was measured (ADR-0016).
 */
function walk(dir = REPO, found = []) {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const full = join(dir, entry.name);
    if (entry.isDirectory()) {
      if (SKIP_DIRS.has(entry.name)) continue;
      if (SEEDED_TREES.has(full)) continue;
      walk(full, found);
    } else {
      const lang = langOf(entry.name);
      if (lang) found.push([relative(REPO, full), lang]);
    }
  }
  return found;
}

/**
 * The comment text of each line, indexed by 0-based line number.
 *
 * Source is lexed rather than grepped because `"https://x"` holds a `//` that
 * is not a comment and `// a "quote` holds a `"` that opens nothing.
 *
 * `lang` selects between the two dialects at the three points they differ.
 * Block comments nest in Rust and do not in C, hence the depth counter running
 * to at most 1 for C. A Rust raw string's terminator is its own hash count
 * (`r#"…"#`), hence the hash group; C's carries a caller-chosen delimiter
 * (`R"tag(…)tag"`). And Rust's `'` is a lifetime far more often than a char
 * literal, so it only opens one when a closing `'` sits where a char literal's
 * would — in C every `'` opens one.
 *
 * The scanner emits character SPANS and the line number is derived afterwards
 * from an index. Counting newlines as the scanner walks is the same computation
 * in principle and wrong in practice: every branch that skips a region has to
 * remember to count what it skipped, and one that forgets — `\` before a newline
 * inside a string, consumed as an escape pair — misreports every line below it
 * in the file.
 */
function commentSpans(source, lang) {
  return lex(source, lang).comments;
}

/**
 * One walk, two answers: the comment spans and the **ordinary** string-literal
 * spans.
 *
 * The lexer already had to find string literals in order not to read a `//`
 * inside one as a comment; this returns them rather than discarding them, so
 * the literal check below cannot disagree with the comment check about where a
 * string starts. Duplicating this walk is the trap — its raw-string, escape and
 * Rust-lifetime cases are each a bug someone already hit.
 *
 * Raw strings are deliberately **not** collected. `r"..."` spanning lines with
 * aligned columns is a formatted block whose spacing is the author's intent,
 * and it cannot carry the defect the literal check looks for: the defect is a
 * missing `\` continuation, and a raw string has no escapes to be missing.
 */
function lex(source, lang) {
  const spans = [];
  const strings = [];
  let i = 0;
  const n = source.length;

  while (i < n) {
    const c = source[i];

    // Line comment: everything to the newline.
    if (c === "/" && source[i + 1] === "/") {
      let j = i;
      while (j < n && source[j] !== "\n") j++;
      spans.push([i, j]);
      i = j;
      continue;
    }

    // Block comment, possibly multi-line. Rust's nest and C's do not, so the
    // depth counter only ever climbs on the Rust path — in C the first `*/`
    // closes, which is what an inner `/*` in a C comment means.
    if (c === "/" && source[i + 1] === "*") {
      let depth = 1;
      let j = i + 2;
      const start = j;
      while (j < n && depth > 0) {
        if (lang === "rust" && source[j] === "/" && source[j + 1] === "*") {
          depth++;
          j += 2;
        } else if (source[j] === "*" && source[j + 1] === "/") {
          depth--;
          j += 2;
        } else {
          j++;
        }
      }
      spans.push([start, Math.max(start, j - 2)]);
      i = j;
      continue;
    }

    // Raw string. Rust: r"..." / r#"..."# / br#"..."#, terminated by its own
    // hash count. C++11: R"tag(...)tag", terminated by its own delimiter — the
    // delimiter is whatever sits between the `R"` and the `(`, and it exists so
    // a literal can hold `)"`.
    const rawPattern = lang === "rust" ? /^b?r(#*)"/ : /^(?:u8|u|U|L)?R"([^()\\ ]*)\(/;
    const raw = rawPattern.exec(source.slice(i, i + 40));
    if (raw && (i === 0 || !/[A-Za-z0-9_]/.test(source[i - 1]))) {
      const terminator = lang === "rust" ? '"' + raw[1] : ")" + raw[1] + '"';
      let j = i + raw[0].length;
      while (j < n && source.slice(j, j + terminator.length) !== terminator) j++;
      i = j + terminator.length;
      continue;
    }

    // Ordinary string, with backslash escapes.
    if (c === '"') {
      let j = i + 1;
      while (j < n) {
        if (source[j] === "\\") {
          j += 2;
          continue;
        }
        if (source[j] === '"') break;
        j++;
      }
      strings.push([i + 1, Math.min(j, n)]);
      i = j + 1;
      continue;
    }

    // A char literal, or — in Rust only — a lifetime. `'\n'` and `'a'` close;
    // Rust's `'a` does not, and C has no such form.
    if (c === "'") {
      const escaped = source[i + 1] === "\\";
      const close = escaped ? source.indexOf("'", i + 2) : i + 2;
      if (!escaped && source[close] === "'") {
        i = close + 1;
        continue;
      }
      if (escaped && close !== -1 && close - i <= 8) {
        i = close + 1;
        continue;
      }
      if (lang !== "rust") {
        // Not a lifetime here, so an unterminated `'` is a stray apostrophe in
        // code we do not own the shape of; step over it rather than swallowing
        // the rest of the file as a literal.
        i++;
        continue;
      }
      i++; // a lifetime
      continue;
    }

    i++;
  }

  return { comments: spans, strings };
}

/** The comment text of each line, indexed by 0-based line number. */
function commentLines(source, lang) {
  const out = new Map();
  const add = (line, text) => out.set(line, (out.get(line) ?? "") + text);

  // Line starts, so an index maps to a line by binary search.
  const starts = [0];
  for (let i = 0; i < source.length; i++) {
    if (source[i] === "\n") starts.push(i + 1);
  }
  const lineOf = (idx) => {
    let lo = 0;
    let hi = starts.length - 1;
    while (lo < hi) {
      const mid = (lo + hi + 1) >> 1;
      if (starts[mid] <= idx) lo = mid;
      else hi = mid - 1;
    }
    return lo;
  };

  for (const [start, end] of commentSpans(source, lang)) {
    let line = lineOf(start);
    let from = start;
    while (from < end) {
      const lineEnd = line + 1 < starts.length ? starts[line + 1] - 1 : source.length;
      add(line, source.slice(from, Math.min(end, lineEnd)));
      from = lineEnd + 1;
      line++;
    }
    if (start === end) add(lineOf(start), "");
  }

  return out;
}

/**
 * Each ordinary string literal's raw source text, keyed by the 0-based line it
 * **starts** on.
 *
 * The start line is the right anchor because that is where the author typed the
 * opening quote and where the missing `\` belongs. The text is the raw slice
 * between the quotes, so a literal broken across source lines still carries the
 * newline and the continuation indent that make it convictable - which is the
 * whole point, since after a later reflow the run of spaces is all that is left
 * of the evidence.
 */
function stringLiteralLines(source, lang) {
  const out = new Map();
  const starts = [0];
  for (let i = 0; i < source.length; i++) {
    if (source[i] === "\n") starts.push(i + 1);
  }
  const lineOf = (idx) => {
    let lo = 0;
    let hi = starts.length - 1;
    while (lo < hi) {
      const mid = (lo + hi + 1) >> 1;
      if (starts[mid] <= idx) lo = mid;
      else hi = mid - 1;
    }
    return lo;
  };

  for (const [start, end] of lex(source, lang).strings) {
    const line = lineOf(start);
    // One line can open several literals; the longest is the one worth showing.
    const text = source.slice(start, end);
    const held = out.get(line);
    if (held === undefined || text.length > held.length) out.set(line, text);
  }

  return out;
}

// A relative link target, in markdown's two forms. Only `./` and `../` — an
// intra-doc link resolves through rustc and is left alone.
const DEFINITION = /\[[^\]]*\]:\s*(\.\.?\/[^\s)]+)/g;
const INLINE = /\]\((\.\.?\/[^)\s]+)\)/g;

// Written from inside a plan session, and undecodable once that session closes.
//
// `the plan` is exempt in front of a number, because `the Plan 0045 Phase 4b
// defect` is a bare-number citation — the form ADR-0127 asks for — and a gate
// that convicted it would be sending authors back to the links.
const VOCABULARY = /\b(this plan|the plan(?![ \t]+\d)|used to|no longer|is new|previously|any more)\b/gi;

// The second shape, and the one a rewrite of the first tends to produce. A bare
// citation is the wanted form and stays silent; the same citation with an
// elapsed-time preposition in front of it is narration, because it dates the
// code against an event instead of describing it. `since Plan 0095 it is the
// counter this folds over` asks the reader to know what happened at 0095 to
// decode a claim about today; `it is the counter this folds over (Plan 0095)`
// does not.
//
// `pre-` is spelled without the trailing separator the others need, since it
// attaches directly: `pre-0070 behaviour`, `pre-Plan 0087`.
const ELAPSED = /\b(?:before|since|until|after)\s+(?:plan|adr|phase)\s+\d+|\bpre-(?:plan\s+|adr\s+|phase\s+)?\d+/gi;

// `hygiene-allow: <reason>` — the reason is what makes it an escape rather than
// a silencer, so it is required.
const ESCAPE = /hygiene-allow:\s*(\S.*)?$/;

// A run of twelve or more spaces inside a string literal.
//
// The defect: a `format!` string broken across source lines without a trailing
// `\` keeps the newline AND the continuation line's indentation, so the reader
// gets `(x/y/rad/ang), which` then twenty-two spaces then `reads 0`. It
// compiles, it matches a `contains` assertion on either half, and it is wrong
// only where it is read. Joining the lines afterwards leaves the run behind,
// which is why the check is on the run and not on the line break.
//
// **The width is the whole rule, and it is a deliberately partial one.** A lost
// continuation and a hand-aligned column are the SAME construct - Rust admits a
// raw newline inside a literal - so nothing mechanical separates them by intent.
// What does separate them in practice is width: a continuation indent is
// produced by rustfmt against the enclosing block and measured 14-23 here, while
// hand-typed alignment measured 4-11. Twelve sits in that gap.
//
// The cost is stated rather than hidden: this is silent on a narrow instance.
// The narrowest this tree has held measured 6, in `core/src/dsp/mod.rs`, and it
// took a hand repair - no run of this gate convicted it and none would. The
// alternative was a threshold of four and roughly thirty `hygiene-allow` markers
// through the report-formatting code, which buys the narrow cases by making the
// escape ordinary - and an escape that is ordinary is how a gate stops meaning
// anything. See ADR-0127's own note on the escape hatch.
const LITERAL_RUN = / {12,}/;

/**
 * Decode a literal's raw source text into what the reader actually gets.
 *
 * The one rule that matters here is Rust's **line continuation**: a backslash
 * immediately before a newline removes the newline *and* the next line's
 * leading whitespace. A literal wrapped that way is correct and must stay
 * silent - which is most of them, since rustfmt wraps long messages constantly.
 * Reading the raw slice instead convicts every correctly-continued literal in
 * the tree.
 *
 * `\` is consumed as a pair so an escaped backslash before a newline is not
 * mistaken for a continuation. Other escapes pass through unchanged; none of
 * them produces a space, so none can create or hide the defect.
 */
function decodeLiteral(raw) {
  let out = "";
  let i = 0;
  while (i < raw.length) {
    if (raw[i] === "\\") {
      const next = raw[i + 1];
      // An escaped backslash is consumed as a pair, so a newline after it is
      // not mistaken for a continuation.
      if (next === "\\") {
        out += "\\\\";
        i += 2;
        continue;
      }
      // The continuation: the newline and the next line's indent both vanish.
      if (next === "\n" || (next === "\r" && raw[i + 2] === "\n")) {
        i += next === "\r" ? 3 : 2;
        while (i < raw.length && (raw[i] === " " || raw[i] === "\t")) i++;
        continue;
      }
      out += raw[i] + (next ?? "");
      i += 2;
      continue;
    }
    out += raw[i];
    i++;
  }
  return out;
}

// The SECOND arm, and the one that catches the defect in the form an author
// actually types. A raw newline followed immediately by 12 or more spaces and
// then a non-space is precisely a continuation indent that lost its `\`: the
// newline survives, the next line's indent survives, and the reader gets a run
// of spaces mid-sentence. The arm below it catches the same defect only AFTER
// someone rejoins the lines, which is the form this tree has happened to hold -
// so the message it prints named a shape it structurally could not see.
//
// A formatted block is not this. A table's rows start AT a column and carry
// their runs between fields; what is rejected here is a leading run, on the
// line after a break, with text behind it.
const CONTINUATION_INDENT = /\r?\n {12,}\S/;

/** Report the run and an excerpt for a literal carrying the defect, else null. */
function brokenLiteral(raw) {
  const text = decodeLiteral(raw);

  // The unrejoined form first, because the newline test below rejects it.
  const wrapped = CONTINUATION_INDENT.exec(text);
  if (wrapped) {
    const run = /\n( +)/.exec(wrapped[0]);
    return { spaces: run[1].length, excerpt: excerptOf(text), wrapped: true };
  }

  // A literal that still holds a newline after decoding is a formatted BLOCK -
  // a TOML fixture, embedded WGSL, a multi-line report - whose column spacing is
  // layout the author typed. The defect is a wrapped *sentence*: one line of
  // prose that lost its continuation, and prose does not carry a newline in the
  // middle of itself. Without this the block's own indent reads as the defect.
  // Both spellings of a line break count: an `\n` escape, which survives
  // decoding as its own two characters, and a raw newline the author typed
  // inside the quotes.
  if (text.includes("\\n") || text.includes("\n")) return null;
  const hit = LITERAL_RUN.exec(text);
  if (!hit) return null;
  if (!/\S/.test(text)) return null;
  const at = hit.index;
  if (at === 0 || at + hit[0].length >= text.length) return null;
  return { spaces: hit[0].length, excerpt: excerptOf(text), wrapped: false };
}

/**
 * A one-line excerpt of a literal.
 *
 * A raw newline inside it would break the one-finding-per-line report format, so
 * it is shown as the escape it should have been.
 */
function excerptOf(text) {
  const one = text.split(/\r?\n/).join("\\n");
  return one.length > 90 ? `${one.slice(0, 87)}...` : one;
}

const findings = [];
let escapes = 0;

const { files: sources, source: enumeration } = sourceFiles();

for (const [file, lang] of sources) {
  const show = file.split(sep).join("/");
  const source = readFileSync(join(REPO, file), "utf8");
  const comments = commentLines(source, lang);
  const literals = stringLiteralLines(source, lang);

  // An escape covers its own line and the one after it, so a marker can sit
  // above the sentence it is excusing rather than inside it.
  const allowed = new Set();
  for (const [line, text] of comments) {
    const escape = ESCAPE.exec(text);
    if (!escape) continue;
    if (!escape[1]) {
      findings.push(`${show}:${line + 1} -> hygiene-allow with no reason given`);
      continue;
    }
    escapes++;
    allowed.add(line);
    allowed.add(line + 1);
  }

  for (const [line, text] of [...comments].sort((a, b) => a[0] - b[0])) {
    if (allowed.has(line)) continue;

    for (const re of [DEFINITION, INLINE]) {
      re.lastIndex = 0;
      for (const m of text.matchAll(re)) {
        findings.push(`${show}:${line + 1} -> relative link \`${m[1]}\` (cite the bare number)`);
      }
    }

    VOCABULARY.lastIndex = 0;
    for (const m of text.matchAll(VOCABULARY)) {
      findings.push(`${show}:${line + 1} -> plan-relative narration \`${m[1]}\``);
    }

    ELAPSED.lastIndex = 0;
    for (const m of text.matchAll(ELAPSED)) {
      findings.push(`${show}:${line + 1} -> narration against an event \`${m[0]}\` (cite it bare)`);
    }
  }

  for (const [line, text] of [...literals].sort((a, b) => a[0] - b[0])) {
    if (allowed.has(line)) continue;
    const broken = brokenLiteral(text);
    if (!broken) continue;
    // The backslash is doubled because this is a template literal: written once
    // it is consumed as an escape and the operator reads `with no trailing )`,
    // which names nothing. The message is about a missing `\`, so it has to
    // print one.
    findings.push(
      `${show}:${line + 1} -> string literal carries ${broken.spaces} spaces mid-sentence ` +
        `(a line break with no trailing \\${broken.wrapped ? ", still unrejoined" : ""}): ` +
        `"${broken.excerpt}"`,
    );
  }
}

if (enumeration !== "git") {
  console.log(
    "note: git could not list this tree, so the file set came from a filesystem\n" +
      "      walk. That set includes anything gitignored sitting in the working\n" +
      "      tree — vendored headers, a virtualenv — which this gate is not\n" +
      "      entitled to judge and which no checkout contains.",
  );
}

if (findings.length === 0) {
  console.log(
    `comment hygiene: OK (${sources.length} tracked sources, no relative links, ` +
      `no plan-relative narration; ${escapes} escapes in use)`,
  );
  process.exit(0);
}

console.error(`comment hygiene: ${findings.length} findings`);
for (const f of findings) console.error(`  ${f}`);
console.error(
  "\nA comment carries the mechanism, the invariant, the trap, and any formula or\n" +
    "constant a reader cannot re-derive. Why an approach beat the alternative, what\n" +
    "was measured, and what the code did before belong in docs/ and are cited by\n" +
    "BARE NUMBER — `ADR-0046`, `Plan 0045 Phase 3`. `grep -rn 0046 docs/adrs`\n" +
    "resolves that in one command and there is no path left to rot.\n" +
    "\n" +
    "Narration is restated as a property of the code: `used to be free-running\n" +
    "until Plan 0095` becomes `the phase is locked, not free-running` — same fact,\n" +
    "no expiry. Where a sentence is a genuine false positive, `hygiene-allow: <why>`\n" +
    "in a comment suppresses its own line and the next one. See ADR-0127.",
);
process.exit(1);
