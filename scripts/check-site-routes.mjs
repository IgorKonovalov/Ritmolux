#!/usr/bin/env node
// Verify that every route the BUILT site serves is reachable from its menu, and
// that no route grew past the size the split exists to hold it under.
//
// Rationale: three defects live here and none of them is visible in the tree.
//
//   Adding a page needs an edit to `PUBLISHED` in
//   site/src/plugins/rewrite-links.mjs AND an edit to the sidebar in
//   site/astro.config.mjs, and nothing made the two agree. A file added to the
//   first and not the second becomes an orphan: it builds, Pagefind indexes it,
//   and the only way to it is search.
//
//   The reverse is a sidebar entry naming a route that does not build.
//   `check-site-links.mjs` catches that one, because the sidebar renders into
//   every page and its href resolves to nothing -- this script names it as what
//   it is instead of as a broken link on 137 pages.
//
//   And a split route is only worth having while it stays small. ADR-0166 picks
//   40 KB and 20 KB from a measured distribution and asserts a worst case of
//   about 27 KB; 30,000 bytes is the ceiling that says the arithmetic still
//   holds. A route over it means the distribution moved and ADR-0166 needs
//   redoing -- NOT that this constant needs raising.
//
// Usage:  node scripts/check-site-routes.mjs [dist]
// Exit 0 = every property below holds. Exit 1 = the violations are listed.
//
// Three properties, all exact:
//
//   1. Every route a published source contributes appears in the sidebar.
//   2. Every route the build serves appears in the sidebar, except the landing
//      page, which the site title links to instead.
//   3. No route's markdown source exceeds ROUTE_SOURCE_CEILING bytes.
//
// A build that has not happened fails LOUDLY rather than passing vacuously,
// on the same reasoning as check-site-links.mjs.

import { readFileSync, readdirSync, existsSync, statSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { PUBLISHED } from "../site/src/plugins/rewrite-links.mjs";
import {
  ROUTE_SOURCE_CEILING,
  chunksOf,
  splitDocument,
} from "../site/src/plugins/split-document.mjs";

const REPO_ROOT = path.resolve(fileURLToPath(new URL("..", import.meta.url)));
const REPO_ROOT_URL = new URL("../", import.meta.url);
const CONFIG = path.join(REPO_ROOT, "site", "astro.config.mjs");
const DIST = path.resolve(process.argv[2] ?? path.join(REPO_ROOT, "site", "dist"));

// The base is read from the site config rather than restated, so this gate
// cannot drift from the subpath the site is actually built for. `SITE_BASE`
// overrides it exactly as it overrides the build's, and the two MUST be set
// together.
const configSource = existsSync(CONFIG) ? readFileSync(CONFIG, "utf8") : "";
const baseMatch = /^const BASE = .*?["'](\/[^"']*)["']/m.exec(configSource);
if (!baseMatch) {
  console.error(
    `check-site-routes: could not read \`const BASE = '...'\` from ${path.relative(REPO_ROOT, CONFIG)}.`,
  );
  process.exit(1);
}
const configured = process.env.SITE_BASE ?? baseMatch[1];
const BASE = configured.endsWith("/") ? configured : `${configured}/`;

/** Routes with an `index.html` under `dist`, as `a/b` with no leading slash. */
function builtRoutes(dir) {
  const out = [];
  const walk = (d, prefix) => {
    for (const entry of readdirSync(d, { withFileTypes: true })) {
      if (!entry.isDirectory()) continue;
      // Astro's asset output and the Pagefind index are not routes.
      if (entry.name === "_astro" || entry.name === "pagefind") continue;
      const route = prefix === "" ? entry.name : `${prefix}/${entry.name}`;
      if (existsSync(path.join(d, entry.name, "index.html"))) out.push(route);
      walk(path.join(d, entry.name), route);
    }
  };
  if (existsSync(dir) && statSync(dir).isDirectory()) walk(dir, "");
  return out;
}

/** Every route named by a sidebar link, from one built page. */
function sidebarRoutes(pageFile) {
  const html = readFileSync(pageFile, "utf8");
  const nav = /<nav[^>]*class="[^"]*\bsidebar\b[^"]*"[^>]*>([\s\S]*?)<\/nav>/.exec(html);
  if (!nav) return null;
  const routes = new Set();
  for (const [, href] of nav[1].matchAll(/href="([^"]*)"/g)) {
    if (!href.startsWith(BASE)) continue;
    routes.add(href.slice(BASE.length).replace(/\/$/, "").split("#")[0]);
  }
  return routes;
}

/**
 * Every route a published source contributes, and the size of each SPLIT route.
 *
 * A document that does not split is already bounded by the split threshold
 * itself and is deliberately not measured here: at 40,000 bytes it stays one
 * route by decision, so holding it to the 30,000-byte ceiling would make the
 * two constants contradict each other. The ceiling is an assertion about the
 * splitter's output -- that cutting at `##` and then at `###` actually produced
 * pages a reader can hold -- not a size limit on documents.
 */
function publishedRoutes() {
  const routes = new Set();
  const splitSizes = new Map();
  for (const [source, route] of Object.entries(PUBLISHED)) {
    const text = readFileSync(new URL(source, REPO_ROOT_URL), "utf8");
    const split = splitDocument(text, route, source);
    if (split === null) {
      routes.add(route);
      continue;
    }
    for (const chunk of chunksOf(split)) {
      routes.add(chunk.route);
      splitSizes.set(chunk.route, Buffer.byteLength(chunk.body, "utf8"));
    }
  }
  return { routes, splitSizes };
}

const pages = builtRoutes(DIST);
if (pages.length === 0 || !existsSync(path.join(DIST, "index.html"))) {
  console.error(
    `check-site-routes: no built site under ${path.relative(REPO_ROOT, DIST) || DIST}.\n` +
      "Run it first, then re-run this gate:\n  cd site && npm install && npm run build",
  );
  process.exit(1);
}

// The landing page is a splash template and carries no sidebar, so the menu is
// read from the first built route that has one.
let menu = null;
for (const route of pages) {
  menu = sidebarRoutes(path.join(DIST, route, "index.html"));
  if (menu !== null && menu.size > 0) break;
}
if (menu === null || menu.size === 0) {
  console.error(
    "check-site-routes: no sidebar found in the built output. Starlight renders the menu into\n" +
      "every page as `<nav class=\"sidebar\">`; if that markup changed, teach this script the\n" +
      "new shape rather than dropping the check.",
  );
  process.exit(1);
}

// The landing page is reached by the site title, not by a menu entry.
const NOT_IN_MENU = new Set([""]);

const { routes, splitSizes } = publishedRoutes();
const missingFromMenu = [...routes].filter((route) => !menu.has(route));
const orphans = pages.filter((route) => !menu.has(route) && !NOT_IN_MENU.has(route));
const oversized = [...splitSizes.entries()].filter(([, size]) => size > ROUTE_SOURCE_CEILING);

const failures = missingFromMenu.length + orphans.length + oversized.length;
if (failures === 0) {
  console.log(
    `site routes: OK (${pages.length} built routes, ${routes.size} from the published set, ` +
      `every one in the menu; largest split route ${Math.max(...splitSizes.values())} B, ` +
      `under ${ROUTE_SOURCE_CEILING})`,
  );
  process.exit(0);
}

if (missingFromMenu.length > 0) {
  console.error(`\nA published route is not in the sidebar (${missingFromMenu.length}):`);
  for (const route of missingFromMenu) console.error(`  ${route}`);
  console.error(
    "\nPUBLISHED in site/src/plugins/rewrite-links.mjs and the sidebar in site/astro.config.mjs\n" +
      "have to name the same set. A route in the first and not the second builds, gets indexed,\n" +
      "and is reachable only by search.",
  );
}
if (orphans.length > 0) {
  console.error(`\nA built route is reachable only by search (${orphans.length}):`);
  for (const route of orphans) console.error(`  ${route}`);
}
if (oversized.length > 0) {
  console.error(`\nA route's source exceeds ${ROUTE_SOURCE_CEILING} bytes (${oversized.length}):`);
  for (const [route, size] of oversized) console.error(`  ${size} B  ${route}`);
  console.error(
    "\nThe repair is new arithmetic in ADR-0166, not a raised constant: the thresholds were\n" +
      "picked from a measured distribution, and a route over the ceiling means the distribution\n" +
      "moved.",
  );
}
process.exit(1);
