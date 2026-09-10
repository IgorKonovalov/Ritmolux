/**
 * What the studio did to the show's rotation, said out loud.
 *
 * A VJ who opens the studio mid-set loses rotation, which is a real change to a
 * running show; it is announced here and given back by the control beside it
 * (ADR-0189).
 */
import styles from './Rotation.module.css'

export interface RotationProps {
  held: boolean
  onResume: () => void
}

export function Rotation({ held, onResume }: RotationProps): JSX.Element {
  return (
    <span className={styles.rotation} data-held={held}>
      {held ? (
        <>
          <span className={styles.state}>rotation held</span>
          <button type="button" className={styles.resume} onClick={onResume}>
            resume
          </button>
        </>
      ) : (
        <span className={styles.state}>rotation running</span>
      )}
    </span>
  )
}
