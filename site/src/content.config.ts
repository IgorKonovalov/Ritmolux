import { readFileSync } from 'node:fs';
import { defineCollection } from 'astro:content';
import { glob } from 'astro/loaders';
import type { Loader } from 'astro/loaders';
import { docsSchema } from '@astrojs/starlight/schema';

/**
 * The published set (Plan 0143), keyed source-path -> site route.
 *
 * Paths are relative to the repository root, which is `site/`'s parent. The
 * loader reads them IN PLACE: there is no staged copy anywhere under `site/`,
 * so `docs/` and `presets/` stay the single source (ADR-0154). A file not
 * listed here does not join the site by existing.
 */
const PUBLISHED: Record<string, string> = {
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
