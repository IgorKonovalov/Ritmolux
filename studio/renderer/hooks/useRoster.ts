/**
 * The library's view of the roster, including the click that has not landed
 * yet.
 *
 * A `ctl/preset` dissolves, so between the click and the picture there is a
 * crossfade during which the player still reports the outgoing preset. Showing
 * the click immediately would be a lie for that interval, and showing nothing
 * makes the list feel broken; so the clicked entry is marked **pending** and
 * the highlight moves only when the player says what is on screen.
 *
 * The pending mark clears on the player's next `preset` event whatever it says.
 * A name the roster does not hold changes nothing at the player (spec 0003),
 * and a pending mark that waited for a confirmation that will never come would
 * stay lit forever.
 */
import { useCallback, useEffect, useState } from 'react'

export interface RosterView {
  names: string[]
  /** What the player says is on screen. */
  active: string | undefined
  /** What was clicked and has not been confirmed, if anything. */
  pending: string | undefined
  select: (name: string) => void
}

export function useRoster(
  names: string[],
  active: string | undefined,
  send: (name: string) => void,
): RosterView {
  const [pending, setPending] = useState<string>()

  // Any report of what is on screen ends the wait, including one that names the
  // preset the click tried to leave.
  useEffect(() => setPending(undefined), [active])

  const select = useCallback(
    (name: string) => {
      if (name === active) return
      setPending(name)
      send(name)
    },
    [active, send],
  )

  return { names, active, pending, select }
}
