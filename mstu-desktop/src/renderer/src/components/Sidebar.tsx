import { useMemo, useState } from 'react'

import { LIBRARY_DRAG } from './Graph'
import { ROLES, ROLE_NAMES, role } from '../roles'
import type { Library } from '../types'

type Props = {
  libraries: Library[]
  connected: boolean
  pipelineName: string
  onAdd: (library: Library) => void
}

/// A list worth searching, rather than one worth scrolling.
const SEARCHABLE = 7

function Sidebar({ libraries, connected, pipelineName, onAdd }: Props): JSX.Element {
  const [query, setQuery] = useState('')

  const found = useMemo(() => {
    const needle = query.trim().toLowerCase()

    return libraries.filter(
      (library) => !needle || `${library.name} ${library.key}`.toLowerCase().includes(needle)
    )
  }, [libraries, query])

  const groups = ROLES.map((kind) => ({
    kind,
    members: found.filter((library) => role(library) === kind)
  })).filter((group) => group.members.length > 0)

  return (
    <aside className="sidebar">
      <div className="sidebar-section pipelines">
        <div className="sidebar-head">
          <span>Pipelines</span>
        </div>

        <div className="sidebar-list">
          <button className="pipeline-row selected">
            <span className="status-dot" />
            <span>{pipelineName}</span>
          </button>
        </div>
      </div>

      <div className="sidebar-section plugins">
        <div className="sidebar-head">
          <span>Plugins</span>
          <span className="tally">{libraries.length}</span>
          <span className={`kind-tag ${connected ? 'live' : ''}`}>
            {connected ? 'live' : 'offline'}
          </span>
        </div>

        {libraries.length >= SEARCHABLE && (
          <div className="sidebar-search">
            <input
              value={query}
              placeholder="Filter plugins"
              aria-label="Filter plugins"
              onChange={(event) => setQuery(event.target.value)}
            />
          </div>
        )}

        <div className="sidebar-list">
          {groups.map((group) => (
            <div key={group.kind}>
              <div className={`plugin-group ${group.kind}`}>{ROLE_NAMES[group.kind]}</div>

              {group.members.map((library) => (
                <button
                  key={library.key}
                  className={`plugin-row ${group.kind}`}
                  title="Click to add, or drag onto the canvas"
                  draggable
                  onDragStart={(event) => {
                    event.dataTransfer.setData(LIBRARY_DRAG, library.key)
                    event.dataTransfer.effectAllowed = 'copy'
                  }}
                  onClick={() => onAdd(library)}
                >
                  <span className="name">{library.name}</span>
                  <span className="version">{library.version}</span>
                </button>
              ))}
            </div>
          ))}

          {libraries.length === 0 && (
            <div className="empty">
              {connected ? 'No plugins found.' : 'Waiting for the engine…'}
            </div>
          )}

          {libraries.length > 0 && found.length === 0 && (
            <div className="empty">Nothing matches “{query.trim()}”.</div>
          )}
        </div>
      </div>
    </aside>
  )
}

export default Sidebar
