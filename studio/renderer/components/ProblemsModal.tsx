/**
 * Every problem the player has reported this run, not only the newest.
 *
 * One gesture routinely produces several: changing a preset's system keeps the
 * outgoing system's `[params]` bindings and structural tables, and each one the
 * incoming system does not declare is its own warning. A surface showing the
 * head of the list tells the author one of them and hides the rest.
 *
 * **The list is the reducer's list.** Its length, its order and its bound all
 * come from `usePlayerEvents`; nothing here re-declares how many are kept or
 * re-sorts them. A warning carries no line or column (spec 0003), so it cannot
 * be marked in the file tab the way an error is — which is what makes this list
 * the only place a warning is legible at all.
 */
import { useEffect } from 'react'

import type { Problem } from '../hooks/usePlayerEvents'

import styles from './ProblemsModal.module.css'

export interface ProblemsModalProps {
  /** Newest first, as the reducer keeps them. */
  problems: Problem[]
  onClose: () => void
}

export function ProblemsModal({ problems, onClose }: ProblemsModalProps): JSX.Element {
  useEffect(() => {
    const onKey = (event: KeyboardEvent): void => {
      if (event.key === 'Escape') onClose()
    }
    window.addEventListener('keydown', onKey)
    // A listener on `window` outlives the component unless it is taken back.
    return () => window.removeEventListener('keydown', onKey)
  }, [onClose])

  return (
    <div className={styles.scrim} role="dialog" aria-modal="true" aria-label="problems">
      <div className={styles.sheet}>
        <div className={styles.head}>
          <h2 className={styles.heading}>
            {problems.length} {problems.length === 1 ? 'problem' : 'problems'}
          </h2>
          <button type="button" className={styles.close} onClick={onClose}>
            close
          </button>
        </div>

        <ol className={styles.list}>
          {problems.map((problem, index) => (
            // The reducer keeps no identity for a problem and two identical
            // lines are a real thing to report twice, so the index is the key.
            <li key={index} className={styles.item} data-kind={problem.kind}>
              <span className={styles.kind}>{problem.kind}</span>
              <span className={styles.file}>
                {problem.file}
                {problem.line !== null && `:${problem.line}`}
              </span>
              <span className={styles.message}>{problem.message}</span>
            </li>
          ))}
        </ol>

        {problems.length === 0 && <p className={styles.empty}>nothing has been reported</p>}
      </div>
    </div>
  )
}
