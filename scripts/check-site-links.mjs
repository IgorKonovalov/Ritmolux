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
// Usage:  node scripts/check-site-links.mjs [dist]
// Exit 0 = every property below holds. Exit 1 = the violations are listed as
// `page -> href`, which is clickable in most terminals. The optional `dist`
// argument points at some other build directory; it is what makes the
// "no build happened" behaviour below testable.
//
// Three properties, all exact -- there is no threshold to tune:
//
//   1. No site-relative href ends in `.md`. A markdown link that escaped the
//      rewrite serves a 404, and it is the single most likely rewrite defect.
//      Off-site GitHub hrefs DO end in `.md` and must: the rewrite's whole job
//      is to turn a relative `.md` link into a blob URL for the same `.md`
//      file. The property is about links the site itself has to serve.
//   2. Every site-relative href resolves to a file in the build output.
//   3. Every off-site href is an absolute `https` URL. Nothing else is a legal
//      way out of this site -- no `http`, no protocol-relative `//host`, no
//      other scheme.
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
const DIST = path.resolve(process.argv[2] ?? path.join(REPO_ROOT, "site", "dist"));

// The base is read from the site config rather than restated, so this gate
// cannot drift from the subpath the site is actually built for.
const configSource = existsSync(CONFIG) ? readFileSync(CONFIG, "utf8") : "";
const baseMatch = /^const BASE = ["'](.+?)["'];/m.exec(configSource);
if (!baseMatch) {
  console.error(
    `check-site-links: could not read \`const BASE = '...'\` from ${path.relative(REPO_ROOT, CONFIG)}.\n` +
      "That declaration is this gate's only source for the site's subpath. If it was renamed or\n" +
      "inlined into `defineConfig`, restore it or teach this script the new shape.",
  );
  process.exit(1);
}
const BASE = baseMatch[1].endsWith("/") ? baseMatch[1] : `${baseMatch[1]}/`;

function htmlFilesUnder(dir) {
  const out = [];
  const walk = (d) => {
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

const unrewritten = [];
const unresolved = [];
const notHttps = [];
const outsideBase = [];

for (const pageFile of pages) {
  const page = path.relative(DIST, pageFile).split(path.sep).join("/");
  const html = readFileSync(pageFile, "utf8");
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

    const target = resolveInBuild(href, pageFile);
    if (target === null) outsideBase.push([page, href]);
    else if (!existsSync(target)) unresolved.push([page, href]);
  }
}

const failures = unrewritten.length + unresolved.length + notHttps.length + outsideBase.length;
if (failures === 0) {
  console.log(
    `site links: OK (${pages.length} built pages, every site-relative href resolves, ` +
      `every off-site href is absolute https)`,
  );
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

console.error(
  "\nThe rewrite lives in site/src/plugins/rewrite-links.mjs. A link that escaped it is\n" +
    "usually a node type the plugin does not visit (raw HTML `<a href>` in a source\n" +
    "document is the likely one) rather than a wrong target -- the plugin throws on a\n" +
    "target it cannot resolve, so a silent survivor never reached it at all.\n" +
    "\n" +
    "An href that resolves to nothing is a page the site links to and does not build.\n" +
    "Check PUBLISHED in site/src/plugins/rewrite-links.mjs against the sidebar in\n" +
    "site/astro.config.mjs: a route in one and not the other fails exactly this way.",
);
process.exit(1);
