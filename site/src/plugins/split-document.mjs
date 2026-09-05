import { readFileSync } from 'node:fs';
import Slugger from 'github-slugger';
import { stripProvenanceText } from './strip-provenance.mjs';

/**
 * Splits a long published document into one route per section, at build time
 * (ADR-0166).
 *
 * The thresholds are the whole decision, and they are measured rather than
 * chosen: 40 KB selects the documents a reader cannot navigate, 20 KB selects
 * the sections that stay unnavigable after a flat split, and the split stops at
 * `###` because a third level shatters coherent small sections into pages with
 * nothing on them. `ROUTE_SOURCE_CEILING` is not a lever - it is the assertion
 * that the two above did their job, and a route over it means the arithmetic in
 * ADR-0166 needs redoing, not that the constant needs raising.
 *
 * Nothing under `docs/` or `presets/` is edited to make this work: the split
 * reads the source text and emits chunks of it (ADR-0154).
 */
export const DOCUMENT_SPLIT_BYTES = 40_000;
export const SECTION_SPLIT_BYTES = 20_000;
export const ROUTE_SOURCE_CEILING = 30_000;

/**
 * The documents the splitter owns.
 *
 * Scoped to one document while the mechanism is a walking skeleton; the size
 * rule in `DOCUMENT_SPLIT_BYTES` is what decides this set once it generalises.
 */
export const SPLIT_SOURCES = new Set(['presets/README.md']);

const FENCE = /^\s{0,3}(```+|~~~+)/;

/**
 * Marks every line inside a fenced code block, the fence lines included.
 *
 * A `## ` at the start of a line inside a fence is shell output or a markdown
 * example, not a section, and this corpus is full of both.
 */
function fencedLines(lines) {
  const fenced = new Array(lines.length).fill(false);
  let open = null;
  lines.forEach((line, i) => {
    const fence = FENCE.exec(line);
    if (open === null) {
      if (fence) {
        open = fence[1][0];
        fenced[i] = true;
      }
      return;
    }
    fenced[i] = true;
    if (fence && fence[1][0] === open) open = null;
  });
  return fenced;
}

/** Indices of the ATX headings at exactly `depth`, within `[from, to)`. */
function headingStarts(lines, fenced, depth, from, to) {
  const marker = `${'#'.repeat(depth)} `;
  const starts = [];
  for (let i = from; i < to; i++) {
    if (!fenced[i] && lines[i].startsWith(marker)) starts.push(i);
  }
  return starts;
}

/**
 * A heading's markdown reduced to the plain text a title and a menu label need.
 *
 * Starlight takes `title` as a string and prints it verbatim, so a code span
 * left in it shows its backticks - `Bloom — \`bloom_amount\`` on the page and
 * again in the sidebar. Removing the markers does not move the route: a slug
 * drops every one of these characters anyway, so a chunk addresses itself
 * identically before and after.
 */
export function plainHeading(text) {
  return text
    .replace(/!?\[([^\]]*)\]\([^)]*\)/g, '$1')
    .replace(/`/g, '')
    .replace(/\*\*([^*]+)\*\*/g, '$1')
    .replace(/\*([^*]+)\*/g, '$1')
    .trim();
}

const bytes = (text) => Buffer.byteLength(text, 'utf8');

/**
 * The routes one document contributes, or `null` when it is small enough to
 * stay a single page.
 *
 * The first chunk is the document's index: the prose before its first `##`,
 * which is where a document's opening explanation lives, plus a generated list
 * of what it now links to. Every other chunk carries a section's body WITHOUT
 * its own heading line - Starlight renders the entry's `title` as the page
 * `<h1>`, so leaving the heading in would print it twice. That is the same
 * reason `astro.config.mjs` drops a document's leading `# `.
 *
 * `sourceHeading` on each chunk is the heading text AS WRITTEN, before the
 * provenance strip. The fragment map is keyed on the slug of that, because a
 * link in the source was written against the source.
 */
export function splitDocument(source, baseRoute, title) {
  if (bytes(source) <= DOCUMENT_SPLIT_BYTES) return null;

  const lines = source.split('\n');
  const fenced = fencedLines(lines);
  const starts = headingStarts(lines, fenced, 2, 0, lines.length);
  if (starts.length === 0) return null;

  const slugger = new Slugger();
  const sections = [];

  starts.forEach((start, k) => {
    const end = k + 1 < starts.length ? starts[k + 1] : lines.length;
    const sourceHeading = lines[start].slice(3).trim();
    const sectionTitle = plainHeading(stripProvenanceText(sourceHeading));
    const route = `${baseRoute}/${slugger.slug(sectionTitle)}`;
    const section = {
      kind: 'section',
      route,
      title: sectionTitle,
      sourceHeading,
      headingLine: start,
      from: start,
      to: end,
      body: lines.slice(start + 1, end).join('\n'),
      children: [],
    };

    // The section is measured WITH its heading line, which is how ADR-0166's
    // distribution was summed; a chunk's own heading becomes its page title
    // rather than page content, so it is not in `body`.
    if (bytes(lines.slice(start, end).join('\n')) > SECTION_SPLIT_BYTES) {
      const subStarts = headingStarts(lines, fenced, 3, start + 1, end);
      if (subStarts.length > 0) {
        const subSlugger = new Slugger();
        section.body = lines.slice(start + 1, subStarts[0]).join('\n');
        section.to = subStarts[0];
        subStarts.forEach((subStart, j) => {
          const subEnd = j + 1 < subStarts.length ? subStarts[j + 1] : end;
          const subSource = lines[subStart].slice(4).trim();
          const subTitle = plainHeading(stripProvenanceText(subSource));
          section.children.push({
            kind: 'subsection',
            route: `${route}/${subSlugger.slug(subTitle)}`,
            title: subTitle,
            sourceHeading: subSource,
            headingLine: subStart,
            from: subStart,
            to: subEnd,
            body: lines.slice(subStart + 1, subEnd).join('\n'),
            children: [],
          });
        });
      }
    }
    sections.push(section);
  });

  return {
    index: {
      kind: 'index',
      route: baseRoute,
      title,
      sourceHeading: null,
      headingLine: null,
      from: 0,
      to: starts[0],
      body: lines.slice(0, starts[0]).join('\n'),
      children: sections,
    },
    sections,
    lines,
    fenced,
  };
}

/** Every chunk of a split document, index first, in reading order. */
export function chunksOf(split) {
  const out = [split.index];
  for (const section of split.sections) {
    out.push(section);
    out.push(...section.children);
  }
  return out;
}

/**
 * The markdown list a parent route carries so it is a way in rather than a
 * stub, appended to the parent's own prose.
 *
 * The hrefs are root-relative and already carry the site base, which is why
 * `rewrite-links.mjs` returns early on a leading `/`: they are site routes
 * already and there is no source file to resolve them against.
 */
export function contentsList(chunk, base, heading) {
  if (chunk.children.length === 0) return '';
  const rows = chunk.children.map((child) => `- [${child.title}](${base}${child.route}/)`);
  return `\n\n## ${heading}\n\n${rows.join('\n')}\n`;
}

/**
 * The one collapsed sidebar group a split document contributes (ADR-0166).
 *
 * Generated rather than hand-listed: 46 entries written out by hand is the
 * shape of roster that has rotted repeatedly in this repository, and a heading
 * rename would silently orphan a route. A section that split again nests its
 * subsections under itself, so the menu is never a flat list of 45 siblings.
 *
 * @param source repo-relative path, as spelled in `PUBLISHED`
 * @param route  the site route that path publishes as
 */
export function sidebarGroup(source, route, label) {
  const fileURL = new URL(`../../../${source}`, import.meta.url);
  const split = splitDocument(readFileSync(fileURL, 'utf8'), route, label);
  if (!split) return { label, slug: route };

  const leaf = (chunk) => ({ label: chunk.title, slug: chunk.route });
  return {
    label,
    collapsed: true,
    items: [
      { label: 'Overview', slug: route },
      ...split.sections.map((section) =>
        section.children.length === 0
          ? leaf(section)
          : {
              label: section.title,
              collapsed: true,
              items: [{ label: 'Overview', slug: section.route }, ...section.children.map(leaf)],
            },
      ),
    ],
  };
}

/**
 * Every heading in a split document, keyed by the slug it had as one page.
 *
 * This is the contract that makes the split safe (ADR-0166). A link in the
 * source was written against the source: `presets/README.md#attractor-detail-sharpness-plan-0027`
 * names a slug computed from the whole document, from the heading text AS
 * WRITTEN, with the provenance still in it. After the split that heading lives
 * on its own route and, if it is not that route's own title, under an anchor
 * computed from the STRIPPED text by a slugger that saw only that route's
 * headings. Neither end of that is guessable, so it is recorded rather than
 * derived at the link site.
 *
 * The value is `{ route, anchor }`; `anchor` is null when the heading became
 * the route's own title and the route alone addresses it.
 */
function fragmentMap(split) {
  const { lines, fenced } = split;
  const map = new Map();
  const sourceSlugger = new Slugger();

  // The slug an author would have written: one slugger over the whole document
  // in reading order, over the heading exactly as it appears.
  const sourceSlugs = new Map();
  for (let i = 0; i < lines.length; i++) {
    if (fenced[i]) continue;
    const heading = /^(#{1,6})\s+(.+?)\s*$/.exec(lines[i]);
    if (!heading) continue;
    sourceSlugs.set(i, sourceSlugger.slug(plainHeading(heading[2])));
  }

  const record = (line, route, anchor) => {
    const slug = sourceSlugs.get(line);
    // First writer wins: two headings that slugged the same in the source
    // already had one unreachable anchor before the split, and inventing a
    // second target here would silently pick the wrong one.
    if (slug !== undefined && !map.has(slug)) map.set(slug, { route, anchor });
  };

  for (const chunk of chunksOf(split)) {
    // A fresh slugger per chunk, because each chunk renders as its own page and
    // `rehype-collect-headings` starts a new one for every page it sees.
    const chunkSlugger = new Slugger();
    for (let i = chunk.from; i < chunk.to; i++) {
      if (fenced[i]) continue;

      // An author-placed `<a id="...">` is a target a heading slug cannot
      // express - it names a passage rather than a section. The raw HTML rides
      // into whichever chunk holds its line, so the id is still the id and only
      // the route around it moves.
      for (const explicit of lines[i].matchAll(/<a\s+(?:id|name)=["']([^"']+)["']/g)) {
        if (!map.has(explicit[1])) map.set(explicit[1], { route: chunk.route, anchor: explicit[1] });
      }

      const heading = /^(#{1,6})\s+(.+?)\s*$/.exec(lines[i]);
      if (!heading) continue;
      if (i === chunk.headingLine) {
        record(i, chunk.route, null);
        continue;
      }
      // The document's own `# ` title is dropped from the body it renders, so
      // it has no anchor either; the index route is what addresses it.
      if (heading[1].length === 1 && chunk.kind === 'index') {
        record(i, chunk.route, null);
        continue;
      }
      record(i, chunk.route, chunkSlugger.slug(plainHeading(stripProvenanceText(heading[2]))));
    }
  }
  return map;
}

/** `source` -> its fragment map, built once per build. */
const FRAGMENTS = new Map();

/**
 * The fragment map for a published source, or null when it does not split.
 *
 * @param source repo-relative path, as spelled in `PUBLISHED`
 * @param route  the site route that path publishes as
 * @param repoRoot the repository root, as a `file:` URL
 */
export function fragmentsOf(source, route, repoRoot) {
  if (!SPLIT_SOURCES.has(source)) return null;
  let map = FRAGMENTS.get(source);
  if (map === undefined) {
    const split = splitDocument(readFileSync(new URL(source, repoRoot), 'utf8'), route, source);
    map = split === null ? null : fragmentMap(split);
    FRAGMENTS.set(source, map);
  }
  return map;
}
