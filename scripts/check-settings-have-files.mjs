#!/usr/bin/env node
// Assert that the two applications a Rust test cannot see keep every setting in
// a file (ADR-0240).
//
// A setting is defined by a key in a user-editable file, and the menu, the
// hotkeys and the panels are editors of that file. The standalone's half of that
// rule is carried by types: `SettingsRow::config_path` is an exhaustive match
// from a menu row to a `config.toml` key, and the tests beside it hold every
// declared path to the schema and to `docs/configuration.md`. The studio and the
// foobar2000 component have no such test - nothing in CI compiles the component
// at all - so their half is this gate, reading source text.
//
// Usage:  node scripts/check-settings-have-files.mjs [root]
// Exit 0 = both properties hold. Exit 1 = each instance is listed as
// `file:line  <matched text>`, clickable in most terminals. The optional `root`
// scans some other directory, following the checkers beside it:
// `node scripts/check-settings-have-files.mjs scripts/fixtures/settings-files`
// expects exit 1 and the break count that tree's README states. CI and the
// pre-push hook pass nothing.
//
// THE TWO PROPERTIES:
//
//   1. NO BROWSER-STORAGE PERSISTENCE UNDER `studio/`. `localStorage`,
//      `sessionStorage` and `indexedDB` are the three ways an Electron renderer
//      keeps a choice where no file can be edited and no other process can read
//      it, which is exactly the shape ADR-0240 refuses. The studio persists
//      through `studio/electron/settings.ts` into `settings.json`, and that is a
//      fact about the code as it stands until something holds it.
//
//   2. EVERY `cfg_*` DECLARATION UNDER `plugin-foobar/` IS NAMED IN
//      `docs/configuration.md`. foobar2000's `cfg_var` store is host-side and
//      invisible from any file a user can edit, so a declaration there is either
//      resume state - which ADR-0240's bound excludes - or an unclaimed setting,
//      and nothing but a document can tell the two apart. Naming it is the
//      claim; the gate holds the claim to the declaration rather than judging
//      it.
//
// THE ALLOWLIST IS INLINE, AND A REASON IS WHAT MAKES IT ONE. This gate greps
// rather than parses, so a comment or a string carrying one of the three names
// bites (ADR-0240 records the cost). `settings-allow: <reason>` on the line
// suppresses it - an ordinary comment in source - and a marker with no reason is
// itself a finding, so the escape cannot quietly become an off switch. Same
// shape as ADR-0127's `hygiene-allow:` and ADR-0202's `count-allow:`.
//
// THE GATE'S OWN HOLES, named here rather than left to be discovered:
//   1. It reads declarations spelled the way it greps for - `cfg_<type> <name>(`
//      on one line. A declaration wrapped across lines, or reached through a
//      macro or a typedef, escapes it. Nothing compiles that tree here, so there
//      is no stronger reading available.
//   2. Half 1 names three APIs. A renderer that persisted through the Cache API,
//      the File System Access API or a `cookie` would pass.
//   3. It says nothing about whether a studio key reaches `settings.json` - that
//      is the studio's own vitest test, which can read the type.

import { readFileSync, readdirSync } from "node:fs";
import { join, dirname, resolve, relative, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { execFileSync } from "node:child_process";

const REPO_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const REPO = resolve(process.argv[2] ?? REPO_ROOT);

// Only the fallback walk below reaches these; git's enumeration excludes them
// without being told to.
const SKIP_DIRS = new Set(["target", "node_modules", ".git", "dist", "out"]);

// The document that carries the plugin's claim, relative to the scan root.
const DOC = "docs/configuration.md";

// Half 1's scope: source the studio's two processes are built from. A tracked
// `.json` is data rather than code and `package-lock.json` is enormous, so the
// extensions are the ones a statement can appear in.
const STUDIO_PREFIX = "studio/";
const STUDIO_EXTENSIONS = new Set([".ts", ".tsx", ".js", ".jsx", ".mjs", ".cjs", ".html"]);

// Half 2's scope: the C++ shim.
const PLUGIN_PREFIX = "plugin-foobar/";
const PLUGIN_EXTENSIONS = new Set([".cpp", ".h", ".hpp", ".cc"]);

// The three browser-storage APIs, as whole words so `myLocalStorageShim` is not
// one of them and `window.localStorage` is.
const BROWSER_STORAGE = /\b(?:localStorage|sessionStorage|indexedDB)\b/g;

// A foobar2000 config-variable declaration: a `cfg_`-prefixed type, the variable
// it declares, and the constructor call. `\b` before `cfg_` is what keeps
// `g_cfg_preset.get()` out - the character before `cfg` there is `_`, which is a
// word character, so the boundary fails.
const CFG_DECL = /\bcfg_[A-Za-z0-9_]*\s+([A-Za-z_][A-Za-z0-9_]*)\s*\(/g;

// `settings-allow: <reason>` on the line suppresses it. The reason is reviewed
// rather than checked, and a marker without one is reported in its place.
const ESCAPE = /settings-allow:\s*(\S.*)?$/;

/** The extension of a slash-separated path, or `""`. */
function extensionOf(path) {
  const dot = path.lastIndexOf(".");
  return dot >= 0 ? path.slice(dot) : "";
}

/** Whether this gate reads a path at all, and which half it belongs to. */
function halfFor(path) {
  const slashed = path.split(sep).join("/");
  if (slashed.includes("node_modules/")) return null;
  if (slashed.startsWith(STUDIO_PREFIX) && STUDIO_EXTENSIONS.has(extensionOf(slashed))) {
    return "studio";
  }
  if (slashed.startsWith(PLUGIN_PREFIX) && PLUGIN_EXTENSIONS.has(extensionOf(slashed))) {
    return "plugin";
  }
  return null;
}

/**
 * Every file the REPOSITORY holds under the root that either half reads.
 *
 * Enumerated from git, not from the filesystem, for the reason the sibling gates
 * are: "code we own" and "code this gate judges" are then the same set by
 * construction, and a vendored or generated tree that is gitignored is absent
 * from CI's fresh clone and present in every working tree. The walk stays as the
 * fallback for a tree git cannot answer for.
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
      if (halfFor(path)) found.push(path);
    }
    return { files: found, source: "git" };
  } catch {
    return { files: walk(), source: "filesystem" };
  }
}

function walk(dir = REPO, found = []) {
  let entries;
  try {
    entries = readdirSync(dir, { withFileTypes: true });
  } catch {
    return found;
  }
  for (const entry of entries) {
    const full = join(dir, entry.name);
    if (entry.isDirectory()) {
      if (SKIP_DIRS.has(entry.name)) continue;
      walk(full, found);
    } else {
      const path = relative(REPO, full);
      if (halfFor(path)) found.push(path);
    }
  }
  return found;
}

/** Whether `name` appears in `doc` inside backticks. */
function documented(doc, name) {
  return doc.includes(`\`${name}\``);
}

const findings = [];
const { files, source } = sourceFiles();

let studioFiles = 0;
let pluginFiles = 0;
let declarations = 0;

let doc = null;
try {
  doc = readFileSync(join(REPO, DOC), "utf8");
} catch {
  doc = null;
}

for (const path of files) {
  const half = halfFor(path);
  let src;
  try {
    src = readFileSync(join(REPO, path), "utf8");
  } catch {
    continue;
  }
  const lines = src.split("\n");
  if (half === "studio") studioFiles += 1;
  else pluginFiles += 1;

  const pattern = half === "studio" ? BROWSER_STORAGE : CFG_DECL;
  pattern.lastIndex = 0;
  for (const m of src.matchAll(pattern)) {
    const line = src.slice(0, m.index).split("\n").length;
    const text = lines[line - 1] ?? "";

    if (half === "plugin") {
      declarations += 1;
      // A declaration the document names is the claim this gate exists to
      // require; only an unclaimed one is a finding.
      if (doc !== null && documented(doc, m[1])) continue;
    }

    const escape = text.match(ESCAPE);
    if (escape) {
      if (!escape[1]) {
        findings.push({
          path,
          line,
          text: "settings-allow with no reason given",
          full: text,
          half,
        });
      }
      continue;
    }
    findings.push({ path, line, text: m[0].trim(), full: text, half });
  }
}

console.log(
  `settings have files: ${studioFiles} studio source(s) and ${pluginFiles} plugin source(s) ` +
    `scanned, enumerated from the ${source}; ${declarations} cfg declaration(s)`,
);

if (doc === null) {
  console.error(
    `\nsettings have files: ${DOC} is missing, so no plugin declaration can be claimed`,
  );
  process.exit(1);
}

if (findings.length === 0) {
  console.log(
    "settings have files: OK (no browser-storage persistence under studio/, " +
      "every plugin cfg declaration is named in the document)",
  );
  process.exit(0);
}

const studioBreaks = findings.filter((f) => f.half === "studio").length;
const pluginBreaks = findings.length - studioBreaks;
console.error(
  `\nsettings have files: ${findings.length} finding(s) ` +
    `(${studioBreaks} under studio/, ${pluginBreaks} under plugin-foobar/)`,
);
for (const f of findings) {
  console.error(`  ${f.path}:${f.line}  ${f.text}`);
  const shown = f.full.trim();
  console.error(`    ${shown.length > 96 ? shown.slice(0, 93) + "…" : shown}`);
}
console.error(
  "\nEvery setting lives in a file, and the menu edits that file (ADR-0240):\n" +
    "\n" +
    "  studio/       persist through studio/electron/settings.ts into settings.json.\n" +
    "                Browser storage keeps a choice where no file can be edited and\n" +
    "                no other process can read it. Momentary view state - a panel\n" +
    "                width, which view is open - owes no key and needs no storage.\n" +
    "\n" +
    `  plugin-foobar/  name the declaration in ${DOC}, in backticks, and say\n` +
    "                  whether it is resume state or a setting. foobar's cfg_var\n" +
    "                  store is host-side and invisible from any file a user can\n" +
    "                  edit, so the document is the only thing that can tell the\n" +
    "                  two apart.\n" +
    "\n" +
    "Where the match is a comment or a string rather than a real one, put\n" +
    "`settings-allow: <why>` on the line. The reason is reviewed rather than\n" +
    "checked, so write the one you would defend.",
);
process.exit(1);
