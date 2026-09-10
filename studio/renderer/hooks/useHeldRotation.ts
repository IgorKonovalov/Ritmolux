/**
 * Rotation is held for as long as the studio is attached (ADR-0189).
 *
 * Rotation is on by default with a dwell of tens of seconds, so without this
 * the preset under the editor changes by itself while the author works and an
 * edit lands in whatever the dwell most recently brought in. Holding it is what
 * makes "the preset on screen is the one being edited" true rather than
 * probable.
 *
 * `hold` is sent on every `hello`, which is once per player run: an attach is
 * the hello, and a player that greets again is a new child to hold. Repeating
 * it is safe by contract — `auto` and `hold` are **positions, not presses**
 * (spec 0003), so a surface that restates its position cannot thereby toggle
 * rotation back on. This is the first consumer to depend on that for
 * correctness rather than for ergonomics.
 *
 * **The attach is the event's identity**, not its contents: two runs of the
 * same build greet with the same version and schema, and only the object tells
 * them apart. The caller hands over what the event reducer kept, which is one
 * object per greeting; a caller that built a fresh one each render would send
 * `hold` again after every resume.
 */
import { useCallback, useEffect, useState } from 'react'

import type { HelloEvent, TransportVerb } from '@shared/protocol'

export interface RotationHold {
  /** Whether the studio is holding rotation, as the surface says it. */
  held: boolean
  /** Give it back to the player: `auto`, the opposite position. */
  resume: () => void
}

export function useHeldRotation(
  hello: HelloEvent | undefined,
  transport: (verb: TransportVerb) => void,
): RotationHold {
  const [held, setHeld] = useState(false)

  useEffect(() => {
    if (hello === undefined) return
    transport('hold')
    setHeld(true)
  }, [hello, transport])

  const resume = useCallback(() => {
    transport('auto')
    setHeld(false)
  }, [transport])

  return { held, resume }
}
