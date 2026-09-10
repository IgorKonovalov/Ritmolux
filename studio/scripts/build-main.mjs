// Bundles the main process to `dist/main/index.cjs` (ADR-0178).
//
// CJS with `electron` external: a sandboxed Electron main bundle resolves
// `require('electron')` from the runtime, and bundling it would replace the
// live module object with a copy that has no `app`.
import { build, context } from 'esbuild'
import { fileURLToPath } from 'node:url'
import { dirname, resolve } from 'node:path'

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const watch = process.argv.includes('--watch')

const config = {
  entryPoints: [resolve(root, 'electron/main.ts')],
  outfile: resolve(root, 'dist/main/index.cjs'),
  bundle: true,
  platform: 'node',
  format: 'cjs',
  target: 'node20',
  external: ['electron'],
  // Named explicitly: esbuild reads `paths` from the config it is given, and
  // the root `tsconfig.json` is a references-only file that declares none, so
  // without this every `@shared/*` import fails to resolve.
  tsconfig: resolve(root, 'tsconfig.main.json'),
  sourcemap: true,
  logLevel: 'info',
}

if (watch) {
  const ctx = await context(config)
  await ctx.watch()
} else {
  await build(config)
}
