/**
 * The one question a fork asks: what to call the copy.
 *
 * It stands in the editor's status strip rather than over the window. The
 * gesture it answers for is a slider release or a stop drag, so the author is
 * looking at the picture at the instant it appears; a dialog centred on the
 * preview would cover the thing they just moved (ADR-0189 names the placement
 * as this lane's to make small).
 *
 * Nothing is written until Save. Escape and Cancel drop the gesture with the
 * edits it was carrying, which is what cancelling an unconsented write means.
 */
import { useState } from 'react'

import type { ForkRequest } from '../hooks/useActivePreset'

import styles from './ForkPrompt.module.css'

export interface ForkPromptProps {
  request: ForkRequest
  onSave: (name: string) => void
  onCancel: () => void
}

export function ForkPrompt({ request, onSave, onCancel }: ForkPromptProps): JSX.Element {
  const [name, setName] = useState(request.suggestion)
  const empty = name.trim() === ''

  return (
    <form
      className={styles.prompt}
      onSubmit={(event) => {
        event.preventDefault()
        if (!empty) onSave(name.trim())
      }}
      onKeyDown={(event) => {
        if (event.key === 'Escape') onCancel()
      }}
    >
      <label className={styles.field}>
        <span className={styles.label}>save a copy as</span>
        <input
          className={styles.name}
          value={name}
          autoFocus
          aria-label="save a copy as"
          onChange={(event) => setName(event.currentTarget.value)}
        />
      </label>
      <button type="submit" className={styles.save} disabled={empty}>
        save
      </button>
      <button type="button" className={styles.cancel} onClick={onCancel}>
        cancel
      </button>
      <p className={styles.why}>
        Editing <span className={styles.source}>{request.source}</span> would change a preset the
        studio did not create, so this edit and every one after it go to the copy.
      </p>
    </form>
  )
}
