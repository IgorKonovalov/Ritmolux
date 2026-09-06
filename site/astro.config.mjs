import { execFileSync } from 'node:child_process';
import { readFileSync, readdirSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';
import rehypeMermaid from 'rehype-mermaid';
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

/**
 * How a ```mermaid fence becomes a picture (ADR-0171).
 *
 * `img-svg` renders each fence to an SVG at BUILD time with the headless
 * Chromium `playwright` installs, and emits a `<picture>` carrying a light and a
 * dark rendering. Nothing about mermaid reaches the reader: the alternative is
 * its runtime, over a megabyte on every page for four diagrams, rendered after
 * the page paints and told the theme by hand on every toggle.
 *
 * The two themes are named rather than styled, and `themeVariables` sets the
 * accent from the site's own violet - Starlight's typography does not reach
 * inside mermaid's SVG, which carries its own inline styles, so this object is
 * the one place a diagram's colour is decided.
 *
 * A fence that does not parse FAILS THE BUILD. That is the decision, not an
 * accident of configuration: a broken diagram is a broken build, in the same
 * place a broken link is.
 */
const MERMAID = {
  strategy: 'img-svg',
  dark: { theme: 'dark', themeVariables: { primaryColor: '#241d54', lineColor: '#7d6cf0' } },
  mermaidConfig: {
    theme: 'default',
    themeVariables: { primaryColor: '#d8d3f7', lineColor: '#5b4bd6' },
    fontFamily: 'var(--sl-font, system-ui, sans-serif)',
  },
};

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
const doc = (source) => sidebarGroup(source, PUBLISHED[source]);

/**
 * Fails the build when a page rendered to nothing.
 *
 * THIS IS THE ONLY THING THAT MAKES A CONTENT ERROR VISIBLE. Astro's content
 * layer renders each entry inside a try, and an entry whose remark or rehype
 * chain throws is stored WITHOUT its rendered html rather than aborting the
 * sync: the page then builds, gets indexed, gets a menu entry, and serves a
 * title over an empty body. Two of this project's own build-time rules throw
 * exactly that way and would otherwise be silent - `rewrite-links.mjs` throws on
 * a relative target that does not resolve, and `rehype-mermaid` throws on a
 * fence that does not parse. Both are supposed to fail the build (ADR-0154,
 * ADR-0171), and before this hook neither did.
 *
 * The test is emptiness rather than a diff, because emptiness is what the
 * failure mode produces and it needs no baseline. A page with no
 * `sl-markdown-content` at all is a template that does not have one, not a
 * defect, so it is skipped.
 */
function failOnEmptyPages() {
  const MARKER = '<div class="sl-markdown-content">';
  return {
    name: 'ritmolux-fail-on-empty-pages',
    hooks: {
      'astro:build:done': ({ dir, logger }) => {
        const root = fileURLToPath(dir);
        const empty = [];
        const walk = (directory) => {
          for (const entry of readdirSync(directory, { withFileTypes: true })) {
            const full = path.join(directory, entry.name);
            if (entry.isDirectory()) {
              walk(full);
            } else if (entry.name.endsWith('.html')) {
              // Starlight's 404 is a title and a sentence in its own template,
              // with a genuinely empty markdown body. It is the one page whose
              // emptiness means nothing.
              if (path.relative(root, full) === '404.html') continue;
              const html = readFileSync(full, 'utf8');
              if (!html.includes(MARKER)) continue;
              // The container closing on the marker IS the empty case, exactly:
              // Astro emits no whitespace between them, and a page with one
              // character of content does not match.
              if (html.includes(`${MARKER}</div>`)) {
                empty.push(path.relative(root, full).split(path.sep).join('/'));
              }
            }
          }
        };
        walk(root);
        if (empty.length === 0) {
          logger.info('every page has a body');
          return;
        }
        throw new Error(
          `${empty.length} page(s) rendered to an empty body:\n  ${empty.join('\n  ')}\n\n` +
            `A page builds with a title and nothing under it when its markdown chain THREW and ` +
            `Astro stored the entry unrendered. The two throws that reach here are a relative ` +
            `link that resolves to nothing (rewrite-links.mjs) and a mermaid fence that does ` +
            `not parse (rehype-mermaid); neither prints anything of its own. Check the source ` +
            `document's links and fences.`,
        );
      },
    },
  };
}

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
    rehypePlugins: [scrollWideTables, [rehypeMermaid, MERMAID]],
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
    failOnEmptyPages(),
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
      // Six groups, each named for what the reader is doing rather than for
      // where the file came from (ADR-0169). Every group's first entry is a page
      // a stranger can start from; `scripts/check-site-routes.mjs` holds every
      // route to being reachable from here rather than only by search.
      sidebar: [
        {
          label: 'Get it',
          items: [
            { label: 'Start here', slug: 'start-here' },
            doc('packaging/windows/READ-ME-FIRST.md'),
            doc('packaging/macos/READ-ME-FIRST.md'),
            doc('packaging/foobar/READ-ME-FIRST.md'),
          ],
        },
        {
          label: 'Use it',
          items: [
            doc('docs/running.md'),
            doc('docs/configuration.md'),
            { label: 'Gallery', slug: 'gallery' },
          ],
        },
        {
          label: 'Author presets',
          items: [
            doc('docs/preset-guide.md'),
            doc('docs/presets.md'),
            doc('docs/preset-palettes.md'),
            doc('docs/preset-tuning-walkthrough.md'),
            doc('presets/README.md'),
            doc('docs/capturing.md'),
          ],
        },
        {
          label: 'Embed it',
          items: [
            doc('docs/embedding.md'),
            doc('core-cabi/include/rlx_core.h'),
            doc('docs/specs/0001-c-abi.md'),
            doc('docs/specs/0002-ring-determinism.md'),
          ],
        },
        {
          label: 'How it works',
          items: [
            doc('docs/how-it-works.md'),
            doc('docs/nfr.md'),
            doc('docs/generative-techniques-catalogue.md'),
          ],
        },
        {
          label: 'Contribute',
          items: [
            doc('docs/developing.md'),
            doc('docs/testing.md'),
            doc('docs/milkdrop-conversion.md'),
            doc('docs/releasing.md'),
            doc('docs/on-device-validation.md'),
            doc('docs/diffusion-filter.md'),
          ],
        },
      ],
    }),
  ],
});
