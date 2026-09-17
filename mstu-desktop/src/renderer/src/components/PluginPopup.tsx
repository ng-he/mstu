import { useRef, useState } from 'react'

import type { PluginHost } from '../pluginHost'
import type { Popup } from '../types'
import PluginView from './PluginView'

type Props = {
  host: PluginHost
  popup: Popup
  onClose: () => void
}

/// A plugin page over the workspace, not modal, so the canvas stays usable.
function PluginPopup({ host, popup, onClose }: Props): JSX.Element {
  const [at, setAt] = useState(() => ({
    x: Math.max(16, (window.innerWidth - popup.width) / 2),
    y: 80
  }))

  const grab = useRef<{ x: number; y: number } | null>(null)

  return (
    <div className="popup" style={{ left: at.x, top: at.y, width: popup.width }}>
      <div
        className="popup-head"
        onPointerDown={(event) => {
          grab.current = { x: event.clientX - at.x, y: event.clientY - at.y }
          event.currentTarget.setPointerCapture(event.pointerId)
        }}
        onPointerMove={(event) => {
          if (!grab.current) return

          // Keep the title bar on screen so the popup can always be grabbed back.
          setAt({
            x: event.clientX - grab.current.x,
            y: Math.max(0, event.clientY - grab.current.y)
          })
        }}
        onPointerUp={() => {
          grab.current = null
        }}
      >
        <span className="name">{popup.title}</span>
        <span className="id">({popup.plugin})</span>

        <button
          className="remove"
          title="Close"
          aria-label={`Close ${popup.title}`}
          onPointerDown={(event) => event.stopPropagation()}
          onClick={onClose}
        >
          ✕
        </button>
      </div>

      <PluginView
        host={host}
        plugin={popup.plugin}
        page={popup.page}
        role="popup"
        className="plugin-ui"
        style={{ height: popup.height }}
      />
    </div>
  )
}

export default PluginPopup
