import { readFileSync } from 'node:fs';
import { defineCollection } from 'astro:content';
import { glob } from 'astro/loaders';
import type { Loader } from 'astro/loaders';
import { docsSchema } from '@astrojs/starlight/schema';
// The publish boundary lives beside the rewriter that reads it, so the loader and
// the link rewrite cannot disagree about what is published.
import { PUBLISHED } from './plugins/rewrite-links.mjs';

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

const ROUTES: Record<string, string> = { ...PUBLISHED, ...SITE_PAGES };

/**
 * Starlight's `docsSchema()` requires a `title`; not one file in the published
 * set carries frontmatter, and none may gain any. The title is therefore taken
 * from the document's own opening `# ` heading, injected into the frontmatter
 * object on its way to schema validation.
 *
 * The matching half of this lives in `astro.config.mjs`: a remark plugin drops
 * that same leading heading from the body, because Starlight renders `title`
 * as the page `<h1>` and the document would otherwise show two.
 */
function titleFromLeadingHeading(filePath: string): string {
  for (const line of readFileSync(filePath, 'utf8').split(/\r?\n/)) {
    if (line.trim() === '') continue;
    const heading = /^#\s+(.+?)\s*$/.exec(line);
    if (heading) return heading[1];
    break;
  }
  throw new Error(
    `${filePath} is in the published set but does not open with an "# " heading, ` +
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
            // demanding an `# ` heading of a file that has no body heading.
            data: props.data?.title
              ? props.data
              : { title: titleFromLeadingHeading(props.filePath!), ...props.data },
          }),
      }),
  };
}

export const collections = {
  docs: defineCollection({
    loader: withDerivedTitles(
      glob({
        base: '..',
        pattern: Object.keys(ROUTES),
        generateId: ({ entry }) => ROUTES[entry],
      }),
    ),
    schema: docsSchema(),
  }),
};
