/**
 * The player's picture.
 *
 * Every pixel here came off the child's pipe; the studio draws none of its own
 * (ADR-0175). The canvas is sized to the geometry the `stream` event declared
 * and scaled by CSS, so the element's box never changes the pixels.
 *
 * Each painted frame is acknowledged, which is what lets the main process drop
 * rather than queue: an unacknowledged frame means this component is behind,
 * and the newest frame is the only one worth showing.
 */
import { useEffect, useRef, useState } from 'react'

import type { FrameMessage } from '@shared/frames'
import type { StreamEvent } from '@shared/protocol'

import { onFramePort } from '../framePort'

import styles from './Preview.module.css'

export interface PreviewStats {
  dropped: number
  delivered: number
}

interface Props {
  stream: StreamEvent | undefined
  onStats: (stats: PreviewStats) => void
}

export function Preview({ stream, onStats }: Props): JSX.Element {
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const [painted, setPainted] = useState(false)

  useEffect(() => {
    const canvas = canvasRef.current
    if (canvas === null || stream === undefined) return
    const context = canvas.getContext('2d')
    if (context === null) return

    let port: MessagePort | undefined
    const release = onFramePort((p) => {
      port = p
      p.onmessage = (event: MessageEvent<FrameMessage>) => {
        const { frame, dropped, delivered } = event.data
        // The transferred buffer is viewed, not copied; ImageData takes
        // ownership of the view for the life of the putImageData call.
        const pixels = new Uint8ClampedArray(frame)
        if (pixels.length === stream.width * stream.height * 4) {
          context.putImageData(new ImageData(pixels, stream.width, stream.height), 0, 0)
          setPainted(true)
        }
        onStats({ dropped, delivered })
        p.postMessage({ ack: true })
      }
      p.start()
    })

    return () => {
      release()
      if (port !== undefined) port.onmessage = null
    }
  }, [stream, onStats])

  return (
    <div className={styles.frame}>
      {stream === undefined ? (
        <p className={styles.waiting}>waiting for the player&rsquo;s stream&hellip;</p>
      ) : (
        <canvas
          ref={canvasRef}
          className={styles.canvas}
          width={stream.width}
          height={stream.height}
          aria-label="The player's preview picture"
          data-painted={painted}
        />
      )}
    </div>
  )
}
