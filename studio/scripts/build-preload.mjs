// Bundles the preload script to `dist/preload/index.cjs` (ADR-0178).
//
// CJS with `electron` external: a sandboxed preload resolves
// `require('electron')` from the runtime, and bundling it would replace the
// live module object with a copy that has no `contextBridge`.
import { build, context } from 'esbuild'
import { fileURLToPath } from 'node:url'
import { dirname, resolve } from 'node:path'

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const watch = process.argv.includes('--watch')

const config = {
  entryPoints: [resolve(root, 'electron/preload/index.ts')],
  outfile: resolve(root, 'dist/preload/index.cjs'),
  bundle: true,
  platform: 'node',
  format: 'cjs',
  target: 'node20',
  external: ['electron'],
  // Named explicitly: esbuild reads `paths` from the config it is given, and
  // the root `tsconfig.json` is a references-only file that declares none, so
  // without this every `@shared/*` import fails to resolve.
  tsconfig: resolve(root, 'tsconfig.preload.json'),
  sourcemap: true,
  logLevel: 'info',
}

if (watch) {
  const ctx = await context(config)
  await ctx.watch()
} else {
  await build(config)
}
