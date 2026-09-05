import { visit } from 'unist-util-visit';

/**
 * Removes the trailing provenance parenthetical from headings and from block
 * ends, at build time (ADR-0168's mechanical half).
 *
 * WHY THIS RUNS AS A REMARK PLUGIN AND NOT AS AN EDIT TO `docs/`. Two reasons,
 * and both are hard. Slugs - and therefore route names and heading anchors -
 * are computed downstream in rehype from the heading text this pass leaves
 * behind, so a heading like `## Engine-wide controls (Plan 0018)` would
 * otherwise address itself as `engine-wide-controls-plan-0018` permanently
 * (ADR-0166). And the source keeps the citation, because `docs/` is the single
 * source for the editor and GitHub as well as for the site (ADR-0154); nothing
 * under `docs/` or `presets/` is edited to make the site render.
 *
 * The source headings are unchanged, so `scripts/toc.mjs --check` is unaffected.
 */

/** `ADR-0090`, `Plan 0076`, `Plan 0091 Phase 5`. */
const CITE = String.raw`(?:ADR-\d{4}|Plan \d{4}(?:\s+Phase \d+)?)`;

/** What may join two citations inside one parenthetical. */
const SEP = String.raw`\s*[,;/&+]\s*`;

/**
 * A trailing `(...)` that mentions a plan or an ADR anywhere inside it.
 *
 * Deliberately narrower than "any trailing parenthetical": a heading ending in
 * parentheses that happen to hold a four-digit number is not provenance, and
 * this corpus has headings ending in figures and version numbers.
 */
const TRAILING_CITED = new RegExp(String.raw`\s*\([^()]*(?:ADR-\d{4}|Plan \d{4})[^()]*\)\s*$`);

/**
 * A trailing `(...)` holding citations and nothing else.
 *
 * Body prose gets this stricter test, because a parenthetical woven into a
 * sentence is a judgement call about the sentence and belongs to the prose
 * rewrite, not to a regex. `... (forward-extensible by size). (ADR-0008)` loses
 * its second parenthetical and keeps its first.
 */
const TRAILING_PURE = new RegExp(String.raw`\s*\(${CITE}(?:${SEP}${CITE})*\)\s*$`);

/**
 * The string form, for a heading that becomes a page title rather than a node.
 *
 * The splitter lifts a section's heading out of the body and hands it to
 * Starlight as the entry's `title`, which never passes through the mdast
 * transform above. A backtick inside the matched parenthetical stands in for
 * the `inlineCode` check the node form makes.
 */
export function stripProvenanceText(text) {
  const match = TRAILING_CITED.exec(text);
  if (!match || match[0].includes('`')) return text;
  return text.slice(0, match.index).replace(/\s+$/, '');
}

/** The plain text a node contributes, for offset arithmetic over its siblings. */
function nodeText(node) {
  if (node.type === 'text' || node.type === 'inlineCode') return node.value;
  if (Array.isArray(node.children)) return node.children.map(nodeText).join('');
  return '';
}

/**
 * Truncates `parent.children` at the trailing parenthetical `pattern` matches.
 *
 * Returns whether anything changed. The match is found against the flattened
 * plain text of every child, so a citation split across a text node and a link
 * node - `(added 2026-07-21; retargeted 2026-07-22 per [ADR-0010](...))` - is
 * one match rather than three unmatchable fragments.
 */
function stripTrailing(parent) {
  const children = parent.children;
  if (!children || children.length === 0) return false;

  const spans = [];
  let text = '';
  for (const child of children) {
    const value = nodeText(child);
    spans.push({ child, start: text.length, end: text.length + value.length });
    text += value;
  }

  const cited = parent.type === 'heading' || parent.type === 'link';
  const match = (cited ? TRAILING_CITED : TRAILING_PURE).exec(text);
  if (!match) return false;
  const cut = match.index;

  // A code span inside the parenthetical means it carries subject matter rather
  // than record, and the whole parenthetical stays. `## Idiom D - full-screen
  // fragment (have it: `fragment_field.rs`)` is that shape carrying no citation
  // at all, which is what makes its `(have it: `lines/`, Plan 0010 closed)`
  // siblings content and not provenance.
  if (spans.some((s) => s.child.type === 'inlineCode' && s.end > cut)) return false;

  const host = spans.findIndex((s) => cut >= s.start && cut < s.end);
  // The `(` landing anywhere but in plain text means the parenthesis itself is
  // inside a link or a code span, and truncating there would corrupt the node.
  if (host === -1 || spans[host].child.type !== 'text') return false;

  spans[host].child.value = spans[host].child.value.slice(0, cut - spans[host].start);
  children.length = host + 1;
  while (children.length > 0) {
    const last = children[children.length - 1];
    if (last.type !== 'text') break;
    last.value = last.value.replace(/\s+$/, '');
    if (last.value !== '') break;
    children.pop();
  }
  return true;
}

/**
 * A unified attacher: use it in `remarkPlugins` as `stripProvenance`.
 *
 * `tableCell` is visited alongside `paragraph` because a cell is a block end
 * too; `listItem` is not, because its content is a `paragraph` already.
 *
 * A `link` is visited ONLY when it addresses this same page, and that narrow
 * case is the generated contents block (ADR-0163): its rows are links whose
 * text is the heading verbatim, so a row would otherwise keep displaying a
 * citation the heading above it no longer shows. The row's `#anchor` still
 * names the pre-strip slug and is repaired by the fragment map, not here - a
 * link into another document is left entirely alone, because its text is the
 * author's sentence rather than a copy of a heading.
 */
export function stripProvenance() {
  return (tree) => {
    visit(tree, ['heading', 'paragraph', 'tableCell', 'link'], (node) => {
      if (node.type === 'link' && !node.url?.startsWith('#')) return;
      stripTrailing(node);
    });
  };
}
