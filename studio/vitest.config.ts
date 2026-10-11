import { defineConfig } from 'vitest/config'
import react from '@vitejs/plugin-react'
import { fileURLToPath, URL } from 'node:url'

export default defineConfig({
  plugins: [react()],
  test: {
    globals: true,
    // The default environment is node, because most of what has logic here is
    // main-process code. The few renderer tests opt into jsdom per file with
    // `@vitest-environment jsdom`.
    environment: 'node',
    include: ['**/*.test.ts', '**/*.test.tsx'],
    exclude: ['node_modules/**', 'dist/**', 'release/**'],
    // Vitest writes its transformed modules to `<os.tmpdir()>/<random id>/ssr/`
    // and never removes that directory; the teardown does (the module says why).
    globalSetup: ['./test/global-teardown.ts'],
  },
  resolve: {
    alias: {
      '@': fileURLToPath(new URL('./renderer', import.meta.url)),
      '@shared': fileURLToPath(new URL('./shared', import.meta.url)),
    },
  },
})
