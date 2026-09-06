import { statSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { visit } from 'unist-util-visit';
import { fragmentsOf } from './split-document.mjs';

/** The repository root - `site/`'s parent, three levels above this file. */
const REPO_ROOT_URL = new URL('../../../', import.meta.url);
export const REPO_ROOT = fileURLToPath(REPO_ROOT_URL);

/**
 * The published set (Plan 0143), keyed repo-relative source path ->
 * `{ route, title }`.
 *
 * This map is the publish boundary itself, which is why it lives beside the
 * rewriter rather than in `content.config.ts`: a target inside it becomes a
 * site route, and a target outside it becomes a GitHub URL. Both halves have to
 * read the same list or the site links to pages it also publishes.
 *
 * `title` is the page's `<h1>`, its menu label, and the text a link into it
 * gets when the source wrote a path instead of a name - one string, declared
 * once, so the three cannot disagree (ADR-0169). A document's own opening
 * heading is still stripped from the body and no longer names the page: a
 * heading is written for a file and a title is written for a menu entry, and in
 * this corpus the two said different things.
 *
 * A file not listed here does not join the site by existing. Adding one means
 * this map AND the sidebar in `astro.config.mjs`.
 *
 * The set is `docs/` plus `presets/README.md` plus the three `READ-ME-FIRST.md`
 * under `packaging/`.
 * The packaging files are here so the site's installation pages ARE the file a
 * tester finds inside the release zip: a drift between the two is not merely
 * unlikely, it is unrepresentable (ADR-0167).
 */
export const PUBLISHED = {
  // Use it
  'docs/running.md': { route: 'use/running', title: 'Running the app' },
  'docs/configuration.md': { route: 'use/configuration', title: 'Configuration' },
  // Author presets
  'docs/preset-guide.md': { route: 'guide/preset-guide', title: 'Preset guide' },
  'docs/presets.md': { route: 'guide/expression-language', title: 'Expression language' },
  'docs/preset-palettes.md': { route: 'guide/palettes', title: 'Colour and palettes' },
  'docs/preset-tuning-walkthrough.md': {
    route: 'guide/tuning-walkthrough',
    title: 'Tuning walkthrough',
  },
  'presets/README.md': { route: 'guide/parameter-roster', title: 'Parameter roster' },
  'docs/capturing.md': { route: 'engine/capturing', title: 'Headless capture and video' },
  // How it works
  'docs/how-it-works.md': { route: 'engine/how-it-works', title: 'How it works' },
  'docs/nfr.md': { route: 'engine/nfr', title: 'Non-functional requirements' },
  'docs/generative-techniques-catalogue.md': {
    route: 'engine/techniques',
    title: 'Technique catalogue',
  },
  // Embed it
  'docs/specs/0001-c-abi.md': { route: 'engine/spec-c-abi', title: 'C ABI contract' },
  'docs/specs/0002-ring-determinism.md': {
    route: 'engine/spec-ring-determinism',
    title: 'Ring determinism',
  },
  // Contribute
  'docs/developing.md': { route: 'contribute/developing', title: 'Developing' },
  'docs/diffusion-filter.md': { route: 'engine/diffusion-filter', title: 'Diffusion filter' },
  'docs/on-device-validation.md': {
    route: 'engine/on-device-validation',
    title: 'On-device validation',
  },
  'docs/releasing.md': { route: 'engine/releasing', title: 'Releasing' },
  // Get it - the tester's own files, published as they ship (ADR-0167)
  'packaging/windows/READ-ME-FIRST.md': { route: 'install/windows', title: 'Install on Windows' },
  'packaging/macos/READ-ME-FIRST.md': { route: 'install/macos', title: 'Install on macOS' },
  'packaging/foobar/READ-ME-FIRST.md': {
    route: 'install/foobar',
    title: 'Install the foobar2000 component',
  },
};

/**
 * The ref every off-site URL is pinned to.
 *
 * `main` is chosen over a release tag, and the tradeoff is not symmetric. About
 * 87 % of the relative links in the published set point OUTSIDE it - into
 * `docs/adrs/`, `docs/plans/` and the design backlog - and those documents move:
 * a plan is `git mv`d into `plans/done/` at every close. `scripts/check-doc-links.mjs`
 * forces the source link to be corrected in the same commit as the move, so a
 * URL built from `main` is right at every commit where the source is right.
 * Pinned to a tag, the same link would be correct only until the next close and
 * would then rot with nothing to detect it.
 *
 * FAILURE MODE, and it is real: these URLs describe the tip of `main`, not the
 * commit the site was built from. Between a deploy and a later move, a link
 * 404s until the site is rebuilt. That is the same staleness the site itself
 * carries - the site is a current-version site by design, with no per-release
 * versioning - so pinning to `main` keeps one staleness surface instead of two.
 */
const GITHUB_REF = 'main';
const GITHUB_BASE = `https://github.com/IgorKonovalov/Ritmolux`;

/** A relative markdown path standing alone, with nothing else around it. */
const PATH_ONLY = /^[\w./-]+\.md$/;

/**
 * Whether a link's whole visible text is the path it points at.
 *
 * The corpus writes both `[the expression language](presets.md)` and
 * ``[`docs/presets.md`](presets.md)``, and only the second is a path standing in
 * for a name. The test is deliberately narrow - ONE leaf, text or code, whose
 * value is nothing but a relative markdown path - so
 * `[the full section in the roster](...)` keeps its sentence.
 *
 * `strong` and `emphasis` are unwrapped on the way down, because the corpus also
 * writes ``[**`docs/presets.md`**](presets.md)``: the emphasis is a decoration
 * on the path, not a second thing the link says.
 */
function pathTextLeaf(node) {
  if (node.children?.length !== 1) return null;
  const only = node.children[0];
  if (only.type === 'strong' || only.type === 'emphasis') return pathTextLeaf(only);
  if (only.type !== 'text' && only.type !== 'inlineCode') return null;
  return PATH_ONLY.test(only.value.trim()) ? only : null;
}

function splitFragment(url) {
  const hash = url.indexOf('#');
  return hash === -1 ? [url, ''] : [url.slice(0, hash), url.slice(hash)];
}

function isExternal(url) {
  return /^[a-z][a-z0-9+.-]*:/i.test(url) || url.startsWith('//');
}

/** The repo-relative published source a vfile came from, or null. */
function sourceOf(filePath) {
  const rel = path.relative(REPO_ROOT, path.resolve(filePath)).split(path.sep).join('/');
  return rel in PUBLISHED ? rel : null;
}

/**
 * Where `#slug`, written against a document as one page, lands after the split.
 *
 * Throws rather than guessing. An unmatched fragment is the failure this map
 * exists to make visible: before the split every anchor into a 273 KB page
 * landed somewhere on the right page whether or not it was correct, so being
 * wrong was invisible, and after the split it would land on the wrong page
 * instead (ADR-0166).
 */
function resolveFragment(source, route, fragment, from) {
  const map = fragmentsOf(source, route, REPO_ROOT_URL);
  if (map === null) return null;

  const slug = decodeURIComponent(fragment.slice(1));
  const target = map.get(slug);
  if (target === undefined) {
    throw new Error(
      `rewrite-links: ${from} links to ${source}#${slug}, and no heading in that document ` +
        `has that slug. ${source} is split into routes by size, so every fragment into it is ` +
        `resolved through its heading map; a fragment that matches nothing is a dead link that ` +
        `used to land on the right page by accident. Fix the link, or the heading it names.`,
    );
  }
  return { route: target.route, hash: target.anchor === null ? '' : `#${target.anchor}` };
}

/**
 * Rewrites every relative markdown link at build time.
 *
 * A target inside the published set becomes a site route; a target outside it
 * becomes an absolute GitHub URL at `GITHUB_REF`. A link into the published set
 * whose whole text is that path is also renamed to the target's declared title.
 * Nothing in `docs/` or `presets/` is edited to achieve any of this - the source
 * keeps the relative form that `scripts/check-doc-links.mjs` gates and that an
 * editor and GitHub both navigate (ADR-0154).
 *
 * `image` nodes are deliberately NOT visited. Astro resolves relative image
 * references against the source file's own location and optimizes them, which
 * is exactly the wanted behaviour; rewriting them to GitHub URLs would forfeit
 * it and serve full-size PNGs from another origin.
 *
 * A unified attacher: use it in `remarkPlugins` as `[rewriteLinks, { base }]`.
 * Passing `rewriteLinks({ base })` instead hands unified the transformer where
 * it expects the attacher, and it is then called with no arguments at all.
 *
 * @param {{ base: string }} options - the Astro `base`, e.g. `/ritmolux/`.
 */
export function rewriteLinks({ base }) {
  const siteBase = base.endsWith('/') ? base : `${base}/`;

  return (tree, file) => {
    // Without a source path there is no way to resolve a relative target, and
    // silently leaving `.md` hrefs in the output is the failure this plugin
    // exists to prevent. Fail loudly instead.
    if (!file.path) {
      throw new Error('rewrite-links: markdown reached the rewriter with no source path');
    }
    const fromDir = path.dirname(path.resolve(file.path));
    const ownSource = sourceOf(file.path);

    visit(tree, ['link', 'definition'], (node) => {
      const url = node.url;
      if (!url || isExternal(url)) return;

      // A same-page anchor addresses this document, and after a split "this
      // document" is many routes - the generated contents block (ADR-0163) is
      // a whole page of them. In a document that does not split, the anchor is
      // already right and nothing needs doing.
      if (url.startsWith('#')) {
        if (ownSource === null) return;
        const landed = resolveFragment(ownSource, PUBLISHED[ownSource].route, url, file.path);
        if (landed !== null) node.url = `${siteBase}${landed.route}/${landed.hash}`;
        return;
      }

      // A root-relative URL is already a site route - the site's own pages
      // write them. Only a genuinely relative target is resolvable against a
      // source file, and only those cross the publish boundary.
      if (url.startsWith('/')) return;

      const [target, fragment] = splitFragment(url);
      if (target === '') return;

      const abs = path.resolve(fromDir, target);
      const rel = path.relative(REPO_ROOT, abs).split(path.sep).join('/');

      const published = PUBLISHED[rel];
      if (published) {
        const { route, title } = published;
        const landed = fragment === '' ? null : resolveFragment(rel, route, fragment, file.path);
        node.url =
          landed === null
            ? `${siteBase}${route}/${fragment}`
            : `${siteBase}${landed.route}/${landed.hash}`;
        // A path is the honest name of a file on GitHub and the wrong register
        // on a site, so a link that reads as a path takes the target's declared
        // title instead (ADR-0169). Only links INTO the published set are
        // renamed: an off-site link still addresses a file at a path, and its
        // path is what the reader is about to see.
        // The leaf is retitled in place rather than the link's children being
        // replaced, so an emphasis around the path survives as an emphasis
        // around the title, and a code span becomes plain text - a title in
        // backticks would read as a literal again.
        const leaf = node.type === 'link' ? pathTextLeaf(node) : null;
        if (leaf !== null) {
          leaf.type = 'text';
          leaf.value = title;
        }
        return;
      }

      // Outside the published set: an absolute GitHub URL. `blob` addresses a
      // file and `tree` a directory; GitHub does redirect one to the other, but
      // emitting the right one keeps the built output honest for the gate.
      let kind = 'blob';
      try {
        if (statSync(abs).isDirectory()) kind = 'tree';
      } catch {
        // check-doc-links.mjs already asserts every relative target resolves on
        // disk, so a miss here means this rewriter resolved from the wrong
        // directory - a defect, not a broken document.
        throw new Error(
          `rewrite-links: ${file.path} links to ${target}, which does not exist at ${abs}`,
        );
      }
      node.url = `${GITHUB_BASE}/${kind}/${GITHUB_REF}/${rel}${fragment}`;
    });
  };
}
