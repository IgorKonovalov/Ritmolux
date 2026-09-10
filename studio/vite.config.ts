import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import { fileURLToPath, URL } from 'node:url'

// `base: './'` matters: the built renderer is loaded with `loadFile`, so every
// asset URL has to be relative or it resolves against the filesystem root.
export default defineConfig({
  root: 'renderer',
  base: './',
  plugins: [react()],
  server: { port: 5273, strictPort: true },
  build: {
    outDir: '../dist/renderer',
    emptyOutDir: true,
    sourcemap: true,
  },
  resolve: {
    alias: {
      '@': fileURLToPath(new URL('./renderer', import.meta.url)),
      '@shared': fileURLToPath(new URL('./shared', import.meta.url)),
    },
  },
})
