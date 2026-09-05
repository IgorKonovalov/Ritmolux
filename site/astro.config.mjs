import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';
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
    remarkPlugins: [stripLeadingHeading, stripProvenance, [rewriteLinks, { base: BASE }]],
  },
  // The published set is read in place from the repository root, one level
  // above this project. Vite refuses to serve files outside its root in dev
  // unless the ancestor is allowed explicitly.
  vite: { server: { fs: { allow: ['..'] } } },
  integrations: [
    starlight({
      title: 'Ritmolux',
      customCss: ['./src/styles/gallery.css'],
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
