import { readFileSync } from 'node:fs';
import Slugger from 'github-slugger';
import { PUBLISHED } from './rewrite-links.mjs';
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
        subStarts.forEach((subStart, j) => {
          const subEnd = j + 1 < subStarts.length ? subStarts[j + 1] : end;
          const subSource = lines[subStart].slice(4).trim();
          const subTitle = plainHeading(stripProvenanceText(subSource));
          section.children.push({
            kind: 'subsection',
            route: `${route}/${subSlugger.slug(subTitle)}`,
            title: subTitle,
            sourceHeading: subSource,
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
      body: lines.slice(0, starts[0]).join('\n'),
      children: sections,
    },
    sections,
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
 */
export function sidebarGroup(source, label) {
  const fileURL = new URL(`../../../${source}`, import.meta.url);
  const text = readFileSync(fileURL, 'utf8');
  const route = PUBLISHED[source];
  const split = splitDocument(text, route, label);
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
