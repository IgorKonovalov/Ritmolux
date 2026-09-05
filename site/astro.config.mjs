import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';
import { rewriteLinks } from './src/plugins/rewrite-links.mjs';
import { stripProvenance } from './src/plugins/strip-provenance.mjs';
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
            { label: 'Preset guide', slug: 'guide/preset-guide' },
            { label: 'Expression language', slug: 'guide/expression-language' },
            { label: 'Colour and palettes', slug: 'guide/palettes' },
            { label: 'Tuning walkthrough', slug: 'guide/tuning-walkthrough' },
            sidebarGroup('presets/README.md', 'Parameter roster'),
            { label: 'Gallery', slug: 'gallery' },
          ],
        },
        {
          label: 'Understand and build it',
          items: [
            { label: 'Non-functional requirements', slug: 'engine/nfr' },
            { label: 'Headless capture and video', slug: 'engine/capturing' },
            { label: 'Technique catalogue', slug: 'engine/techniques' },
            { label: 'Diffusion filter', slug: 'engine/diffusion-filter' },
            { label: 'On-device validation', slug: 'engine/on-device-validation' },
            { label: 'Releasing', slug: 'engine/releasing' },
            { label: 'C ABI contract', slug: 'engine/spec-c-abi' },
            { label: 'Ring determinism', slug: 'engine/spec-ring-determinism' },
          ],
        },
      ],
    }),
  ],
});
