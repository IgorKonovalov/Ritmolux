import path from 'node:path';
import { PUBLISHED } from './rewrite-links.mjs';

/**
 * The one rule pairing an English page with its Russian translation (ADR-0213).
 *
 * A translation is the `<name>.ru.md` sibling of `<name>.md` on disk, and a pair
 * is a twin only while BOTH halves are in `PUBLISHED`. The header control, the
 * in-page cross link, the route middleware and the Russian entrance page all read
 * the pairing from here, so none of them can derive a twin the others do not.
 *
 * Nothing here touches the file system or `import.meta.url`: the module is
 * imported by components that are bundled into the build, where a path computed
 * from the module's own location names the bundle rather than the source.
 */

const SUFFIX = '.ru.md';

/**
 * The page the site owns as the Russian slice's front door. It is not a
 * translation and has no English twin; it lists the translations instead.
 */
export const RU_ENTRANCE = { source: 'site/src/content/docs/ru.mdx', route: 'ru' };

/**
 * The repo-relative source of a content entry, from its `filePath`.
 *
 * A store entry's `filePath` is posix-relative to the Astro project root,
 * `site/` - `../docs/running.md` for a published document,
 * `src/content/docs/ru.mdx` for a page the site owns - so prefixing `site/` and
 * normalizing yields the `PUBLISHED` key. A chunk of a split document carries its
 * source's `filePath`, so every chunk maps to the whole source. An entry with no
 * `filePath` (Starlight's generated 404) maps to `undefined`.
 */
export function sourceOf(filePath) {
  if (!filePath) return undefined;
  return path.posix.normalize(`site/${filePath.replaceAll('\\', '/')}`);
}

/** `ru` for a translation and for the entrance page, `en` for everything else. */
export function langOf(source) {
  return source !== undefined && (source.endsWith(SUFFIX) || source === RU_ENTRANCE.source)
    ? 'ru'
    : 'en';
}

/**
 * The published twin of a source, as `{ source, route, title }`, or `undefined`.
 *
 * Only a markdown source can have one: without the `.md` test a non-markdown
 * source such as the C ABI header would map to itself and be its own twin.
 */
export function twinOf(source) {
  if (source === undefined || !source.endsWith('.md')) return undefined;
  const twin = source.endsWith(SUFFIX)
    ? source.slice(0, -'.ru.md'.length) + '.md'
    : source.slice(0, -'.md'.length) + SUFFIX;
  return PUBLISHED[twin] === undefined ? undefined : { source: twin, ...PUBLISHED[twin] };
}
