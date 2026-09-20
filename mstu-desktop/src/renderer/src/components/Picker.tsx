import { useEffect, useRef, useState } from 'react'

import type { Descriptor } from '../types'

type Props = {
  kind: 'event' | 'command'
  value: number
  items: Descriptor[]
  onPick: (index: number) => void
}

const count = (item: Descriptor): string => {
  const fields = item.schema?.fields.length ?? 0

  return fields === 1 ? '1 field' : `${fields} fields`
}

/// The app's own dropdown: a native select cannot show what a payload carries,
/// and renders as the platform's widget rather than as part of the editor.
function Picker({ kind, value, items, onPick }: Props): JSX.Element {
  const [open, setOpen] = useState(false)
  const box = useRef<HTMLDivElement>(null)

  useEffect(() => {
    if (!open) return

    const outside = (event: PointerEvent): void => {
      if (!box.current?.contains(event.target as Node)) setOpen(false)
    }

    const key = (event: KeyboardEvent): void => {
      if (event.key === 'Escape') setOpen(false)
    }

    window.addEventListener('pointerdown', outside)
    window.addEventListener('keydown', key)

    return () => {
      window.removeEventListener('pointerdown', outside)
      window.removeEventListener('keydown', key)
    }
  }, [open])

  const chosen = items[value]

  return (
    <div className={`picker ${open ? 'open' : ''}`} ref={box}>
      <button
        className="picker-value"
        aria-haspopup="listbox"
        aria-expanded={open}
        onClick={() => setOpen((current) => !current)}
      >
        <span className={`badge ${kind}`}>{kind === 'event' ? 'on' : 'call'}</span>
        <span className="picked">{chosen?.name ?? '—'}</span>
        <span className="hint">{chosen ? count(chosen) : ''}</span>
        <span className="caret" />
      </button>

      {open && (
        <div className="picker-list" role="listbox">
          {items.map((item, index) => (
            <button
              className={`picker-option ${index === value ? 'on' : ''}`}
              key={item.name}
              role="option"
              aria-selected={index === value}
              onClick={() => {
                setOpen(false)
                if (index !== value) onPick(index)
              }}
            >
              <span className="name">{item.name}</span>
              <span className="hint">{count(item)}</span>
            </button>
          ))}
        </div>
      )}
    </div>
  )
}

export default Picker
