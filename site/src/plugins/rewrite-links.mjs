import { statSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { visit } from 'unist-util-visit';
import { fragmentsOf } from './split-document.mjs';

/** The repository root - `site/`'s parent, three levels above this file. */
const REPO_ROOT_URL = new URL('../../../', import.meta.url);
export const REPO_ROOT = fileURLToPath(REPO_ROOT_URL);

/**
 * The published set (Plan 0143), keyed repo-relative source path -> site route.
 *
 * This map is the publish boundary itself, which is why it lives beside the
 * rewriter rather than in `content.config.ts`: a target inside it becomes a
 * site route, and a target outside it becomes a GitHub URL. Both halves have to
 * read the same list or the site links to pages it also publishes.
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
  // Entrance A - use it / author presets
  'docs/preset-guide.md': 'guide/preset-guide',
  'docs/presets.md': 'guide/expression-language',
  'docs/preset-palettes.md': 'guide/palettes',
  'docs/preset-tuning-walkthrough.md': 'guide/tuning-walkthrough',
  'presets/README.md': 'guide/parameter-roster',
  // Entrance B - understand and build it
  'docs/nfr.md': 'engine/nfr',
  'docs/capturing.md': 'engine/capturing',
  'docs/generative-techniques-catalogue.md': 'engine/techniques',
  'docs/diffusion-filter.md': 'engine/diffusion-filter',
  'docs/on-device-validation.md': 'engine/on-device-validation',
  'docs/releasing.md': 'engine/releasing',
  'docs/specs/0001-c-abi.md': 'engine/spec-c-abi',
  'docs/specs/0002-ring-determinism.md': 'engine/spec-ring-determinism',
  // Get it - the tester's own files, published as they ship (ADR-0167)
  'packaging/windows/READ-ME-FIRST.md': 'install/windows',
  'packaging/macos/READ-ME-FIRST.md': 'install/macos',
  'packaging/foobar/READ-ME-FIRST.md': 'install/foobar',
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
 * becomes an absolute GitHub URL at `GITHUB_REF`. Nothing in `docs/` or
 * `presets/` is edited to achieve this - the source keeps the relative form
 * that `scripts/check-doc-links.mjs` gates and that an editor and GitHub both
 * navigate (ADR-0154).
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
        const landed = resolveFragment(ownSource, PUBLISHED[ownSource], url, file.path);
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

      const route = PUBLISHED[rel];
      if (route) {
        const landed = fragment === '' ? null : resolveFragment(rel, route, fragment, file.path);
        node.url =
          landed === null
            ? `${siteBase}${route}/${fragment}`
            : `${siteBase}${landed.route}/${landed.hash}`;
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
