import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { defineCollection } from 'astro:content';
import { glob } from 'astro/loaders';
import type { Loader, LoaderContext } from 'astro/loaders';
import { docsSchema } from '@astrojs/starlight/schema';
// The publish boundary lives beside the rewriter that reads it, so the loader and
// the link rewrite cannot disagree about what is published.
import { PUBLISHED } from './plugins/rewrite-links.mjs';
import { SPLIT_SOURCES, chunksOf, contentsList, splitDocument } from './plugins/split-document.mjs';

/**
 * Pages that belong to the site rather than to the documentation corpus.
 *
 * These are the only markdown files `site/` owns. They are kept out of
 * `PUBLISHED` on purpose: that map is the publish boundary the link rewriter
 * reads, and nothing in `docs/` can hold a relative link to a page that exists
 * only here. Paths are repo-relative, like `PUBLISHED`, because the loader is
 * rooted at the repository.
 */
const SITE_PAGES: Record<string, string> = {
  'site/src/content/docs/index.mdx': 'index',
  'site/src/content/docs/gallery.mdx': 'gallery',
};

/** Repo-relative sources the glob loader still serves one route each. */
const WHOLE: Record<string, string> = Object.fromEntries(
  Object.entries({ ...PUBLISHED, ...SITE_PAGES }).filter(([source]) => !SPLIT_SOURCES.has(source)),
);

const SITE_ROOT = new URL('../', import.meta.url);
const REPO_ROOT = new URL('../../', import.meta.url);

/**
 * Starlight's `docsSchema()` requires a `title`; not one file in the published
 * set carries frontmatter, and none may gain any. The title is therefore taken
 * from the document's own opening `# ` heading, injected into the frontmatter
 * object on its way to schema validation.
 *
 * The matching half of this lives in `astro.config.mjs`: a remark plugin drops
 * that same leading heading from the body, because Starlight renders `title`
 * as the page `<h1>` and the document would otherwise show two.
 *
 * A setext heading counts. `packaging/*\/READ-ME-FIRST.md` writes its title over
 * a rule of `=` rather than behind a `# `, and those files are published as they
 * ship inside the release zip rather than edited to suit a renderer.
 */
export function titleFromLeadingHeading(filePath: string, source?: string): string {
  const lines = (source ?? readFileSync(filePath, 'utf8')).split(/\r?\n/);
  for (let i = 0; i < lines.length; i++) {
    if (lines[i].trim() === '') continue;
    const atx = /^#\s+(.+?)\s*$/.exec(lines[i]);
    if (atx) return atx[1];
    if (/^=+\s*$/.test(lines[i + 1] ?? '')) return lines[i].trim();
    break;
  }
  throw new Error(
    `${filePath} is in the published set but does not open with a heading, ` +
      `so no page title can be derived for it.`,
  );
}

/** Wraps a loader so every entry gains a derived `title` before it is validated. */
function withDerivedTitles(loader: Loader): Loader {
  return {
    ...loader,
    load: (context) =>
      loader.load({
        ...context,
        parseData: (props) =>
          context.parseData({
            ...props,
            // A page that carries its own frontmatter title - the site's own
            // landing page does - is left alone; deriving one would mean
            // demanding a heading of a file that has no body heading.
            data: props.data?.title
              ? props.data
              : { title: titleFromLeadingHeading(props.filePath!), ...props.data },
          }),
      }),
  };
}

/**
 * Emits one entry per route for each document the splitter owns (ADR-0166).
 *
 * The chunks are rendered rather than globbed because they are never files:
 * `docs/` and `presets/` are read in place and there is no staged copy of a
 * document cut into pieces (ADR-0154). `renderMarkdown` runs the site's own
 * remark chain over each chunk, so the provenance strip and the link rewrite
 * apply exactly as they do to a whole document, and `fileURL` is what lets the
 * rewriter resolve a chunk's relative links against the document they came
 * from rather than against nothing.
 */
function splitLoader(sources: Iterable<string>): Loader {
  return {
    name: 'ritmolux-split-documents',
    load: async (context: LoaderContext) => {
      const base = context.config.base.endsWith('/')
        ? context.config.base
        : `${context.config.base}/`;

      for (const source of sources) {
        const fileURL = new URL(source, REPO_ROOT);
        const filePath = fileURLToPath(fileURL);
        const text = readFileSync(filePath, 'utf8');
        const split = splitDocument(text, PUBLISHED[source], titleFromLeadingHeading(filePath, text));
        if (!split) {
          throw new Error(
            `${source} is listed as a split document but is under the size the split needs, ` +
              `so it would silently serve one route while the sidebar names many.`,
          );
        }

        // `filePath` on a store entry is posix-relative to the Astro project
        // root, which is `site/`; the sources sit above it.
        const relative = path
          .relative(fileURLToPath(SITE_ROOT), filePath)
          .split(path.sep)
          .join('/');

        for (const chunk of chunksOf(split)) {
          const body =
            chunk.body +
            contentsList(chunk, base, chunk.kind === 'index' ? 'Sections' : 'In this section');
          const rendered = await context.renderMarkdown(body, { fileURL });
          context.store.set({
            id: chunk.route,
            data: await context.parseData({
              id: chunk.route,
              data: { title: chunk.title },
              filePath: relative,
            }),
            body,
            filePath: relative,
            digest: context.generateDigest(body),
            rendered,
            assetImports: rendered.metadata?.imagePaths,
          });
        }
      }
    },
  };
}

/** Runs the whole-document loader and the splitter into one collection. */
function bothLoaders(whole: Loader, split: Loader): Loader {
  return {
    name: 'ritmolux-published-set',
    load: async (context) => {
      await whole.load(context);
      await split.load(context);
    },
  };
}

export const collections = {
  docs: defineCollection({
    loader: bothLoaders(
      withDerivedTitles(
        glob({ base: '..', pattern: Object.keys(WHOLE), generateId: ({ entry }) => WHOLE[entry] }),
      ),
      splitLoader(SPLIT_SOURCES),
    ),
    schema: docsSchema(),
  }),
};
