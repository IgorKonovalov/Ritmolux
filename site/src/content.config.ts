import { readFileSync } from 'node:fs';
import { defineCollection } from 'astro:content';
import { glob } from 'astro/loaders';
import type { Loader } from 'astro/loaders';
import { docsSchema } from '@astrojs/starlight/schema';
// The publish boundary lives beside the rewriter that reads it, so the loader and
// the link rewrite cannot disagree about what is published.
import { PUBLISHED } from './plugins/rewrite-links.mjs';

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
            data: { title: titleFromLeadingHeading(props.filePath!), ...props.data },
          }),
      }),
  };
}

export const collections = {
  docs: defineCollection({
    loader: withDerivedTitles(
      glob({
        base: '..',
        pattern: Object.keys(PUBLISHED),
        generateId: ({ entry }) => PUBLISHED[entry],
      }),
    ),
    schema: docsSchema(),
  }),
};
