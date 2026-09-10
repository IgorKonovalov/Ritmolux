/**
 * How the studio starts its player, typed once for all three processes.
 *
 * Shared rather than main-side because all three ends read it: main spawns with
 * it, the preload carries it across, and the renderer offers the choice. It is
 * deliberately free of Node — `electron/settings.ts` is where the value meets a
 * file, and the renderer may not import that (ADR-0178).
 */

/**
 * The two modes (ADR-0186).
 *
 * `windowed` spawns `--preview stdout`: the player opens the show window and
 * the studio paints a copy of what it draws, so what is edited is by
 * construction what an audience sees. `windowless` spawns
 * `--stream --sink stdout`: the same show loop with no window, for a
 * single-screen machine where the show window is a window in the way — and the
 * by-construction guarantee is given up for that session.
 */
export const PLAYER_MODES = ['windowed', 'windowless'] as const
export type PlayerMode = (typeof PLAYER_MODES)[number]

/**
 * The mode a machine that has never chosen gets.
 *
 * The VJ with a projector is who the player is for; the laptop session is the
 * accommodation, not the centre (ADR-0186 Alternative B).
 */
export const DEFAULT_PLAYER_MODE: PlayerMode = 'windowed'

export function isPlayerMode(value: unknown): value is PlayerMode {
  return typeof value === 'string' && (PLAYER_MODES as readonly string[]).includes(value)
}
