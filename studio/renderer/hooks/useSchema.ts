/**
 * The engine's parameter schema, fetched once for the window's lifetime.
 *
 * Main runs `ritmolux --schema` and caches the document, so this hook is one
 * IPC round trip however many components ask. It is not re-fetched on a preset
 * change: the schema is a property of the binary, and the binary does not
 * change under a running studio.
 */
import { useEffect, useState } from 'react'

import type { SchemaDocument } from '@shared/schema'

export type SchemaState =
  | { status: 'loading' }
  | { status: 'ready'; document: SchemaDocument }
  | { status: 'failed'; reason: string }

export function useSchema(): SchemaState {
  const [state, setState] = useState<SchemaState>({ status: 'loading' })

  useEffect(() => {
    let live = true
    void window.api.app.getSchema().then((result) => {
      if (!live) return
      setState(
        result.ok
          ? { status: 'ready', document: result.document }
          : { status: 'failed', reason: result.reason },
      )
    })
    // The window can close while the child is still printing; without this the
    // resolve lands on an unmounted tree.
    return () => {
      live = false
    }
  }, [])

  return state
}
