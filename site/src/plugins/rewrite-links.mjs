import { statSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { visit } from 'unist-util-visit';

/** The repository root - `site/`'s parent, three levels above this file. */
export const REPO_ROOT = fileURLToPath(new URL('../../../', import.meta.url));

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

    visit(tree, ['link', 'definition'], (node) => {
      const url = node.url;
      if (!url || url.startsWith('#') || isExternal(url)) return;

      const [target, fragment] = splitFragment(url);
      if (target === '') return;

      const abs = path.resolve(fromDir, target);
      const rel = path.relative(REPO_ROOT, abs).split(path.sep).join('/');

      const route = PUBLISHED[rel];
      if (route) {
        node.url = `${siteBase}${route}/${fragment}`;
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
