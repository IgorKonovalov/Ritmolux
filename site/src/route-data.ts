import { defineRouteMiddleware } from '@astrojs/starlight/route-data';
import { PUBLISHED } from './plugins/rewrite-links.mjs';
import { langOf, sourceOf, twinOf } from './plugins/twins.mjs';

/**
 * Tells a Russian page that it is Russian, and links each twinned pair (ADR-0213).
 *
 * The site has no i18n locales, so Starlight builds every route as the default
 * locale's `en`. On a route whose source is a translation or the Russian
 * entrance page this rewrites the three places that value reaches the page:
 * `lang` (`<html lang>`), `entryMeta.lang` (`<main lang>`) and the `og:locale`
 * head entry, which was computed from the old value before this runs. Every
 * other route is left exactly as Starlight built it.
 *
 * `<html lang>` is also what Pagefind splits its index on, so the Russian routes
 * are indexed apart from the English ones and a search only returns pages in
 * the language of the page it was typed on.
 *
 * The `hreflang` alternates go only on a page whose id IS its source's whole
 * route. A chunk of a split document is not equivalent to a whole translation,
 * so it gets none; nor does a page with no published twin.
 */
export const onRequest = defineRouteMiddleware((context) => {
  const route = context.locals.starlightRoute;
  const source = sourceOf(route.entry?.filePath);

  if (langOf(source) === 'ru') {
    route.lang = 'ru';
    route.entryMeta.lang = 'ru';
    for (const entry of route.head) {
      if (entry.tag === 'meta' && entry.attrs?.property === 'og:locale') entry.attrs.content = 'ru';
    }
  }

  const twin = twinOf(source);
  const own = source === undefined ? undefined : PUBLISHED[source as keyof typeof PUBLISHED];
  if (twin === undefined || own === undefined || route.entry.id !== own.route) return;

  const url = (path: string) => new URL(`${import.meta.env.BASE_URL}${path}/`, context.site).href;
  const pages = { [langOf(source)]: own.route, [langOf(twin.source)]: twin.route };
  for (const lang of ['en', 'ru']) {
    route.head.push({ tag: 'link', attrs: { rel: 'alternate', hreflang: lang, href: url(pages[lang]) } });
  }
});
