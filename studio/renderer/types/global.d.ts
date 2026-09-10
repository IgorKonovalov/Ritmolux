import type { ElectronAPI } from '../../electron/preload'

declare global {
  interface Window {
    /** The only bridge. Assembled in `electron/preload/index.ts`. */
    api: ElectronAPI
  }
}

export {}
