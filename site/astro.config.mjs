import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';
import { SKIP, visit } from 'unist-util-visit';
import { rewriteLinks } from './src/plugins/rewrite-links.mjs';
import { stripProvenance } from './src/plugins/strip-provenance.mjs';
import { PUBLISHED } from './src/plugins/rewrite-links.mjs';
import { sidebarGroup } from './src/plugins/split-document.mjs';

/**
 * The Pages subpath, read by the link rewriter and by
 * `scripts/check-site-links.mjs`.
 *
 * The two hosting stages do not share it, and the difference is case. A GitHub
 * project site is served under the repository's name AS SPELLED, so the
 * permanent home is `/Ritmolux/`; the demo is a directory someone creates by
 * hand inside another repository's `public/`, and it is `/ritmolux/`. The
 * default here is the demo, because that is what a local `npm run build`
 * and a local preview serve. `.github/workflows/pages.yml` sets `SITE_BASE`
 * for the deployment, and must set it for the gate too - a build and a check
 * that disagree about the base report every internal link as broken.
 */
const BASE = process.env.SITE_BASE ?? '/ritmolux/';

/**
 * Drops the document's opening `# ` heading.
 *
 * `src/content.config.ts` derives each page's `title` from that heading, and
 * Starlight renders `title` as the page `<h1>`. Without this the document shows
 * its heading twice. Only a heading that is the FIRST node is removed, so an
 * `# ` further down (there is none today) survives untouched.
 */
/**
 * The workspace version, from the one line that owns it (ADR-0005).
 *
 * Anchored to `[workspace.package]` rather than to the first `version = ` in the
 * file, because a manifest gains sections and the first match is not a contract.
 */
const VERSION = /\[workspace\.package\][\s\S]*?^version = "([^"]+)"/m.exec(
  readFileSync(new URL('../Cargo.toml', import.meta.url), 'utf8'),
)[1];

/**
 * The commit this build was made from, for the footer stamp.
 *
 * `GITHUB_SHA` first, because a CI checkout has it and asking `git` for it there
 * is a second source for one fact. A local build falls back to `git`, and a
 * build from a tarball with no git directory says so rather than failing - a
 * missing stamp is not a reason to have no site.
 */
const COMMIT = (() => {
  if (process.env.GITHUB_SHA) return process.env.GITHUB_SHA.slice(0, 7);
  try {
    return execFileSync('git', ['rev-parse', '--short', 'HEAD'], {
      cwd: new URL('../', import.meta.url),
      encoding: 'utf8',
    }).trim();
  } catch {
    return 'unknown';
  }
})();

/**
 * Fills in the `@VERSION@` placeholder the packaging files carry.
 *
 * `packaging/*\/READ-ME-FIRST.md` ships inside a release zip with that token
 * substituted at packaging time, and the site publishes those same files
 * (ADR-0167). Without this the reader would meet the raw token, which is the
 * one way the published copy could look unlike the shipped one.
 */
function substituteVersion() {
  return (tree) => {
    visit(tree, ['text', 'inlineCode', 'code'], (node) => {
      if (node.value.includes('@VERSION@')) node.value = node.value.replaceAll('@VERSION@', VERSION);
    });
  };
}

/**
 * Wraps every table in a scroll container.
 *
 * A rehype plugin rather than a remark one because the wrapper is a `div` in the
 * output tree, which markdown has no node for. The container is focusable and
 * labelled: a box that scrolls but cannot be reached from the keyboard is
 * unreadable to anyone not using a pointer.
 *
 * `SKIP` is load-bearing. The replacement puts the table inside a new node at
 * the same index, so without it the walk descends into the wrapper, finds the
 * table again, and wraps forever.
 */
function scrollWideTables() {
  return (tree) => {
    visit(tree, 'element', (node, index, parent) => {
      if (node.tagName !== 'table' || parent === undefined || index === undefined) return;
      parent.children[index] = {
        type: 'element',
        tagName: 'div',
        properties: { className: ['table-scroll'], tabIndex: 0, role: 'region' },
        children: [node],
      };
      return [SKIP, index + 1];
    });
  };
}

function stripLeadingHeading() {
  return (tree) => {
    const first = tree.children[0];
    if (first && first.type === 'heading' && first.depth === 1) tree.children.shift();
  };
}

/**
 * One sidebar entry per published document: a plain link while the document is
 * small, and a collapsed group of its routes once it is large enough to split.
 *
 * Every entry goes through here rather than only the ones that split today, so
 * a document crossing the threshold joins the menu as a group on the next build
 * with no edit to this file.
 */
const doc = (source, label) => sidebarGroup(source, PUBLISHED[source], label);

export default defineConfig({
  site: 'https://igorkonovalov.github.io',
  base: BASE,
  // `stripProvenance` runs first because everything downstream reads the
  // headings it rewrites: rehype computes a slug from each heading, and a slug
  // is a route name and an anchor (ADR-0166).
  markdown: {
    remarkPlugins: [
      stripLeadingHeading,
      substituteVersion,
      stripProvenance,
      [rewriteLinks, { base: BASE }],
    ],
    rehypePlugins: [scrollWideTables],
  },
  // The published set is read in place from the repository root, one level
  // above this project. Vite refuses to serve files outside its root in dev
  // unless the ancestor is allowed explicitly.
  vite: {
    server: { fs: { allow: ['..'] } },
    // Read by src/components/Footer.astro. `define` rather than a module the
    // component imports, so the two facts are resolved once, here, beside the
    // manifest and the git call that produce them.
    define: {
      'import.meta.env.BUILD_COMMIT': JSON.stringify(COMMIT),
      'import.meta.env.BUILD_VERSION': JSON.stringify(VERSION),
    },
  },
  integrations: [
    starlight({
      title: 'Ritmolux',
      customCss: ['./src/styles/site.css'],
      components: { Footer: './src/components/Footer.astro' },
      description:
        'Reader-facing documentation for Ritmolux: preset authoring, the expression language, ' +
        'the parameter roster, and the engine contracts.',
      social: [
        {
          icon: 'github',
          label: 'GitHub',
          href: 'https://github.com/IgorKonovalov/Ritmolux',
        },
      ],
      sidebar: [
        {
          label: 'Get it',
          items: [
            { label: 'Start here', slug: 'start-here' },
            doc('packaging/windows/READ-ME-FIRST.md', 'Windows'),
            doc('packaging/macos/READ-ME-FIRST.md', 'macOS'),
            doc('packaging/foobar/READ-ME-FIRST.md', 'foobar2000 component'),
          ],
        },
        {
          label: 'Use it / author presets',
          items: [
            doc('docs/preset-guide.md', 'Preset guide'),
            doc('docs/presets.md', 'Expression language'),
            doc('docs/preset-palettes.md', 'Colour and palettes'),
            doc('docs/preset-tuning-walkthrough.md', 'Tuning walkthrough'),
            doc('presets/README.md', 'Parameter roster'),
            { label: 'Gallery', slug: 'gallery' },
          ],
        },
        {
          label: 'Understand and build it',
          items: [
            doc('docs/nfr.md', 'Non-functional requirements'),
            doc('docs/capturing.md', 'Headless capture and video'),
            doc('docs/generative-techniques-catalogue.md', 'Technique catalogue'),
            doc('docs/diffusion-filter.md', 'Diffusion filter'),
            doc('docs/on-device-validation.md', 'On-device validation'),
            doc('docs/releasing.md', 'Releasing'),
            doc('docs/specs/0001-c-abi.md', 'C ABI contract'),
            doc('docs/specs/0002-ring-determinism.md', 'Ring determinism'),
          ],
        },
      ],
    }),
  ],
});
