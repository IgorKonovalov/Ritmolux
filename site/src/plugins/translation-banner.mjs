import { execFileSync } from 'node:child_process';
import path from 'node:path';

/**
 * Renders the staleness notice on a translated page, and removes the stamp that
 * decided it (ADR-0185).
 *
 * A page is translated when its source is `<name>.ru.md`, not when something
 * lists it: the pair is a sibling pair on disk, and the publish boundary in
 * `rewrite-links.mjs` decides only whether the page is served at all.
 *
 * THE STAMP IS REMOVED rather than left to render. It is an HTML comment, so it
 * would be invisible in the output - but it is also the document's FIRST node,
 * and `stripLeadingHeading` in `astro.config.mjs` drops the opening `# ` heading
 * only when that heading is the first node. Left in place, every Russian page
 * would show its title twice. This plugin therefore runs FIRST in the remark
 * chain, and inserts its notice after the leading heading so the strip still
 * finds it.
 *
 * WHAT THIS DELIBERATELY DOES NOT DO: fail a build because a translation has
 * drifted. Drift is an advisory - `scripts/check-translations.mjs` prints a row
 * and exits 0, and here the reader is told on the page itself. A missing or
 * malformed stamp is the other half and it THROWS: that is mechanical rather
 * than a judgement about prose, and an unstamped Russian page is one nothing can
 * ever say anything about. The throw reaches the reader as a failed build
 * through `failOnEmptyPages` in `astro.config.mjs` - Astro's content layer
 * stores an entry whose markdown chain threw WITHOUT its rendered html rather
 * than aborting, so without that integration this throw would serve a title over
 * an empty body.
 *
 * THE SHALLOW TRAP IS THE REASON THIS IS NOT THREE LINES. `git log -1 -- <path>`
 * on a shallow clone returns the tip commit for every path, so every translated
 * page would carry a notice saying it is out of date - published, to readers,
 * permanently. `actions/checkout@v4` defaults to `fetch-depth: 1`. Two guards,
 * because either alone fails open: the Pages `build` job checks out with
 * `fetch-depth: 0` so the notice is true, and this plugin renders NO notice at
 * all when it detects a shallow repository or cannot reach git, so a shallow
 * local build cannot publish a false one either.
 */

const SUFFIX = '.ru.md';

/** Line 1 of a translation. The same rule `scripts/check-translations.mjs` gates. */
const STAMP = /^<!--\s*translated-from:\s*([0-9a-f]{7,40})\s*-->\s*$/;

/** `{ sha, date }` of the commit that last touched `source`, or null. */
const lastCommit = memo((source) => {
  const out = git(path.dirname(source), ['log', '-1', '--format=%H%x09%cs', '--', source]);
  if (out === null || out === '') return null;
  const [sha, date] = out.split('\t');
  return sha ? { sha, date } : null;
});

/** Whether the repository holding `dir` is shallow, or unreadable by git. */
const cannotMeasure = memo((dir) => git(dir, ['rev-parse', '--is-shallow-repository']) !== 'false');

function memo(fn) {
  const cache = new Map();
  return (key) => {
    if (!cache.has(key)) cache.set(key, fn(key));
    return cache.get(key);
  };
}

function git(cwd, args) {
  try {
    return execFileSync('git', args, {
      cwd,
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'ignore'],
    }).trim();
  } catch {
    return null;
  }
}

/** The notice, as mdast. Russian, because its only reader is a Russian page. */
function notice(stamp, current) {
  return {
    type: 'blockquote',
    children: [
      {
        type: 'paragraph',
        children: [
          { type: 'strong', children: [{ type: 'text', value: 'Перевод устарел.' }] },
          { type: 'text', value: ' Английский оригинал изменился ' },
          { type: 'text', value: current.date },
          { type: 'text', value: ': перевод сделан с версии ' },
          { type: 'inlineCode', value: stamp },
          { type: 'text', value: ', сейчас актуальна ' },
          { type: 'inlineCode', value: current.sha.slice(0, 7) },
          {
            type: 'text',
            value:
              '. Расхождение не проверено построчно — если что-то важно, откройте английскую ' +
              'страницу.',
          },
        ],
      },
    ],
  };
}

/** A unified attacher: use it in `remarkPlugins` as `translationBanner`. */
export function translationBanner() {
  return (tree, file) => {
    if (!file.path || !file.path.endsWith(SUFFIX)) return;

    const first = tree.children[0];
    const match = first?.type === 'html' ? STAMP.exec(first.value.trim()) : null;
    if (match === null) {
      throw new Error(
        `translation-banner: ${file.path} does not open with a ` +
          '`<!-- translated-from: <sha> -->` stamp, so nothing can say which version of its ' +
          'source it describes (ADR-0185). Run: node scripts/check-translations.mjs',
      );
    }
    tree.children.shift();

    const source = path.resolve(file.path.slice(0, -SUFFIX.length) + '.md');
    if (cannotMeasure(path.dirname(source))) return;

    const current = lastCommit(source);
    if (current === null || current.sha.startsWith(match[1])) return;

    // After the leading heading, which `stripLeadingHeading` removes downstream:
    // inserting in front of it would leave that heading second and unstripped,
    // and the page would show its title twice.
    const at = tree.children[0]?.type === 'heading' && tree.children[0].depth === 1 ? 1 : 0;
    tree.children.splice(at, 0, notice(match[1], current));
  };
}
