#!/usr/bin/env node
// Verify that the BUILT documentation site links correctly across the publish
// boundary.
//
// Rationale: `scripts/check-doc-links.mjs` gates the *source*, and it passes
// whatever this script would catch. The published set keeps its relative
// markdown links exactly as written (ADR-0154 -- editing them would forfeit that
// gate and break navigation in an editor and on GitHub), and `site/` rewrites
// them at build time: a target inside the published set becomes a site route, a
// target outside it becomes an absolute GitHub URL. When that rewrite is wrong
// nothing in the tree shows it. The source was never the thing that broke, so
// the built output is the only place the failure is visible.
//
// Usage:  node scripts/check-site-links.mjs [dist] [--require-api]
// Exit 0 = every property below holds. Exit 1 = the violations are listed as
// `page -> href`, which is clickable in most terminals. The optional `dist`
// argument points at some other build directory; it is what makes the
// "no build happened" behaviour below testable.
//
// Four properties, all exact -- there is no threshold to tune:
//
//   1. No site-relative href ends in `.md`. A markdown link that escaped the
//      rewrite serves a 404, and it is the single most likely rewrite defect.
//      Off-site GitHub hrefs DO end in `.md` and must: the rewrite's whole job
//      is to turn a relative `.md` link into a blob URL for the same `.md`
//      file. The property is about links the site itself has to serve.
//   2. Every site-relative href resolves to a file in the build output. That
//      includes an href into `/api/`, so a renamed crate breaks this gate rather
//      than the reader; the rustdoc's OWN pages are not scanned, because they
//      are another tool's generated output.
//   3. Every off-site href is an absolute `https` URL. Nothing else is a legal
//      way out of this site -- no `http`, no protocol-relative `//host`, no
//      other scheme.
// THE `/api/` TREE IS THE ONE CONDITIONAL PART. The Rust API reference is built
// by a separate Pages job and unpacked into `dist/api/` before this runs, so in
// CI an `/api/` href resolves like any other and a renamed crate breaks the gate
// rather than the reader. A laptop that has not run `cargo doc` has no such
// tree, and failing there would make the local gate permanently red - so an
// absent tree SKIPS `/api/` hrefs and says loudly that it did. `--require-api`
// turns the absence itself into a failure, and the workflow passes it: a
// silently-skipped download would publish a menu entry pointing at nothing.
//
//   4. No site-relative link's TEXT ends in `.md`. Property 1 is about where a
//      link goes; this one is about what it says. A source writes
//      ``[`docs/presets.md`](presets.md)`` because a path is the honest name of
//      a file on GitHub, and the rewrite renames it to the target's declared
//      title (ADR-0169). A survivor works, so nothing else would ever report
//      it, and it reads as a leaked file path. Off-site links are exempt and
//      keep their path text, for the same reason the source wrote it.
//
// A build that has not happened fails LOUDLY rather than passing vacuously:
// an empty or missing directory, or one holding no HTML at all, is reported as
// a missing build and exits 1. A gate that goes green because it found nothing
// to check is worse than no gate.

import { readFileSync, readdirSync, existsSync, statSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const REPO_ROOT = path.resolve(fileURLToPath(new URL("..", import.meta.url)));
const CONFIG = path.join(REPO_ROOT, "site", "astro.config.mjs");
const args = process.argv.slice(2);
const REQUIRE_API = args.includes("--require-api");
const DIST = path.resolve(args.find((a) => !a.startsWith("--")) ?? path.join(REPO_ROOT, "site", "dist"));

// The base is read from the site config rather than restated, so this gate
// cannot drift from the subpath the site is actually built for. `SITE_BASE`
// overrides it exactly as it overrides the build's, and the two MUST be set
// together: a check run at a different base than the build reports every
// internal link as broken.
const configSource = existsSync(CONFIG) ? readFileSync(CONFIG, "utf8") : "";
const baseMatch = /^const BASE = .*?["'](\/[^"']*)["']/m.exec(configSource);
if (!baseMatch) {
  console.error(
    `check-site-links: could not read \`const BASE = '...'\` from ${path.relative(REPO_ROOT, CONFIG)}.\n` +
      "That declaration is this gate's only source for the site's subpath. If it was renamed or\n" +
      "inlined into `defineConfig`, restore it or teach this script the new shape.",
  );
  process.exit(1);
}
const configured = process.env.SITE_BASE ?? baseMatch[1];
const BASE = configured.endsWith("/") ? configured : `${configured}/`;

/**
 * Every page this SITE emits, which is deliberately not every page under `dist`.
 *
 * `dist/api/` is rustdoc's output, unpacked in by the Pages workflow. It is
 * generated by another tool with its own conventions - script templates written
 * as `href="./static.files/${f}"`, and a thousand cross-links this gate has no
 * business grading - so it is a DESTINATION here and never a source. What is
 * checked is that the site's own `/api/` hrefs land on a file inside it.
 */
function htmlFilesUnder(dir) {
  const out = [];
  const apiRoot = path.join(dir, "api");
  const walk = (d) => {
    if (path.resolve(d) === path.resolve(apiRoot)) return;
    for (const entry of readdirSync(d, { withFileTypes: true })) {
      const full = path.join(d, entry.name);
      if (entry.isDirectory()) walk(full);
      else if (entry.name.endsWith(".html")) out.push(full);
    }
  };
  if (existsSync(dir) && statSync(dir).isDirectory()) walk(dir);
  return out;
}

const pages = htmlFilesUnder(DIST);
if (pages.length === 0) {
  console.error(
    `check-site-links: no HTML found under ${path.relative(REPO_ROOT, DIST) || DIST}.\n` +
      "The site has not been built. Run it first, then re-run this gate:\n" +
      "  cd site && npm install && npm run build",
  );
  process.exit(1);
}

/** Where a site-relative href lands in the build output, or null if it escapes it. */
function resolveInBuild(href, pageFile) {
  const clean = decodeURIComponent(href.split("#")[0].split("?")[0]);
  if (clean === "") return null;
  let rel;
  if (clean.startsWith("/")) {
    if (!clean.startsWith(BASE)) return null;
    rel = clean.slice(BASE.length);
  } else {
    const fromDir = path.relative(DIST, path.dirname(pageFile));
    rel = path.posix.normalize(path.posix.join(fromDir.split(path.sep).join("/"), clean));
    if (rel.startsWith("..")) return null;
  }
  const target = path.join(DIST, rel);
  // A trailing slash, or a final segment with no extension, addresses a
  // directory -- Astro writes those as `<dir>/index.html`.
  if (clean.endsWith("/") || rel === "" || !path.extname(rel)) {
    return path.join(target, "index.html");
  }
  return target;
}

/**
 * An anchor's visible text: its markup with tags and entities removed.
 *
 * Property 4 reads a link the rewrite was supposed to rename, and the rename
 * replaces a code span with plain text -- so the text is read from the RENDERED
 * anchor rather than from the source, which is the only place a survivor shows.
 * Anchors do not nest, so a non-greedy match needs no parser.
 */
function linkText(markup) {
  return markup
    .replace(/<[^>]*>/g, "")
    .replace(/&[a-z]+;|&#\d+;/gi, " ")
    .trim();
}

// The rustdoc tree, present in CI and usually absent on a laptop.
const API_DIR = path.join(DIST, "api");
const HAS_API = existsSync(API_DIR) && statSync(API_DIR).isDirectory();
if (!HAS_API && REQUIRE_API) {
  console.error(
    `check-site-links: --require-api was passed and ${path.relative(REPO_ROOT, API_DIR) || API_DIR}\n` +
      "does not exist. The Pages workflow builds the rustdoc in its own job and unpacks it here\n" +
      "before this gate runs; an absent tree means that download did not happen, and the site\n" +
      "would publish a `Rust API` menu entry pointing at nothing.",
  );
  process.exit(1);
}

/** Whether an href addresses the rustdoc tree rather than a built page. */
function isApi(href) {
  const clean = href.split("#")[0].split("?")[0];
  return clean.startsWith(`${BASE}api/`) || clean === `${BASE}api` || clean.startsWith("api/");
}

const unrewritten = [];
const unresolved = [];
const notHttps = [];
const outsideBase = [];
const pathText = [];

for (const pageFile of pages) {
  const page = path.relative(DIST, pageFile).split(path.sep).join("/");
  const html = readFileSync(pageFile, "utf8");
  for (const [, href, inner] of html.matchAll(/<a\s[^>]*href="([^"]*)"[^>]*>([\s\S]*?)<\/a>/g)) {
    if (href === "" || href.startsWith("#")) continue;
    if (href.startsWith("https://") || /^([a-z][a-z0-9+.-]*:|\/\/)/i.test(href)) continue;
    const text = linkText(inner);
    if (text.endsWith(".md")) pathText.push([page, `"${text}" -> ${href}`]);
  }
  for (const [, href] of html.matchAll(/href="([^"]*)"/g)) {
    if (href === "" || href.startsWith("#")) continue;

    if (href.startsWith("https://")) continue;
    if (href.startsWith("//") || /^[a-z][a-z0-9+.-]*:/i.test(href)) {
      notHttps.push([page, href]);
      continue;
    }

    if (href.split("#")[0].split("?")[0].endsWith(".md")) {
      unrewritten.push([page, href]);
      continue;
    }

    if (!HAS_API && isApi(href)) continue;

    const target = resolveInBuild(href, pageFile);
    if (target === null) outsideBase.push([page, href]);
    else if (!existsSync(target)) unresolved.push([page, href]);
  }
}

const failures =
  unrewritten.length + unresolved.length + notHttps.length + outsideBase.length + pathText.length;
if (failures === 0) {
  console.log(
    `site links: OK (${pages.length} built pages, every site-relative href resolves, ` +
      `every off-site href is absolute https, no on-site link reads as a file path)`,
  );
  if (!HAS_API) {
    console.log(
      "site links: NOTE - no dist/api/ tree, so /api/ hrefs were NOT checked. Run\n" +
        "  cargo doc --workspace --no-deps  and copy target/doc to site/dist/api  to check them,\n" +
        "or ignore this locally: the Pages workflow passes --require-api and cannot skip.",
    );
  }
  process.exit(0);
}

const report = (rows, heading) => {
  if (rows.length === 0) return;
  console.error(`\n${heading} (${rows.length}):`);
  for (const [page, href] of rows) console.error(`  ${page} -> ${href}`);
};

report(unrewritten, "A markdown link escaped the rewrite and the site would serve a 404");
report(unresolved, "A site-relative href resolves to nothing in the build output");
report(notHttps, "An off-site href is not an absolute https URL");
report(outsideBase, `A site-relative href leaves the site base ${BASE}`);
report(pathText, "A link into the site reads as a file path rather than as a page title");

console.error(
  "\nThe rewrite lives in site/src/plugins/rewrite-links.mjs. A link that escaped it is\n" +
    "usually a node type the plugin does not visit (raw HTML `<a href>` in a source\n" +
    "document is the likely one) rather than a wrong target -- the plugin throws on a\n" +
    "target it cannot resolve, so a silent survivor never reached it at all.\n" +
    "\n" +
    "An href that resolves to nothing is a page the site links to and does not build.\n" +
    "Check PUBLISHED in site/src/plugins/rewrite-links.mjs against the sidebar in\n" +
    "site/astro.config.mjs: a route in one and not the other fails exactly this way.\n" +
    "\n" +
    "A link that reads as a file path is a source link whose whole text was that path and\n" +
    "whose target is published. The rewriter renames those to the target's declared title;\n" +
    "one that survived was written in a shape `pathTextLeaf` does not recognise - text split\n" +
    "across nodes, or raw HTML. Give the link a name in the source, or teach the rewriter.",
);
process.exit(1);
