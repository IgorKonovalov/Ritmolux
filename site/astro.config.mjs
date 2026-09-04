import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';
import { rewriteLinks } from './src/plugins/rewrite-links.mjs';

/** The Pages subpath, shared by both hosting stages and by the link rewriter. */
const BASE = '/ritmolux/';

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
  // Plan 0143 Phase 4 publishes into `public/ritmolux/` of the personal Pages
  // site, and Phase 7 moves to the project repository at the same subpath, so
  // `base` is the same string for both stages and no published URL moves.
  base: BASE,
  markdown: { remarkPlugins: [stripLeadingHeading, [rewriteLinks, { base: BASE }]] },
  // The published set is read in place from the repository root, one level
  // above this project. Vite refuses to serve files outside its root in dev
  // unless the ancestor is allowed explicitly.
  vite: { server: { fs: { allow: ['..'] } } },
  integrations: [
    starlight({
      title: 'Ritmolux',
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
            { label: 'Parameter roster', slug: 'guide/parameter-roster' },
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
