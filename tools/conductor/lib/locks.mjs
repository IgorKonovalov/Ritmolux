// The two machine-wide locks the conductor itself takes (ADR-0205). The mechanism is
// tools/conductor/with-lock.mjs; this module names the locks and times the waits.
//
// `close` is held from before a review session starts until `main` has fast-forwarded, and
// released early when a review returns blockers or majors. `suite` is taken around the gate's
// nextest run; sessions take it themselves through the wrapper, which a hook enforces.

import { acquire, holder, lockDir } from "../with-lock.mjs";

export const CLOSE = "close";
export const SUITE = "suite";

export { holder, lockDir };

/** Takes `name`, reporting the wait through `onWaited(ms)` once it is held. */
export async function take(name, { dir, pollMs, what, onWaiting, onWaited } = {}) {
  const lock = await acquire(name, { dir, pollMs, what, onWait: onWaiting });
  onWaited?.(lock.waitedMs);
  return lock;
}

/** Runs `fn` under `name` and always releases, whatever `fn` does. */
export async function withLock(name, opts, fn) {
  const lock = await take(name, opts);
  try {
    return await fn(lock);
  } finally {
    lock.release();
  }
}
