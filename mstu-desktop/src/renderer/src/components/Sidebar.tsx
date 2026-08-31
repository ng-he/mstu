import type { Library } from '../types'

type Props = {
  libraries: Library[]
  connected: boolean
  pipelineName: string
  onAdd: (library: Library) => void
}

function Sidebar({ libraries, connected, pipelineName, onAdd }: Props): JSX.Element {
  return (
    <aside className="sidebar">
      <div className="sidebar-section">
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

      <div className="sidebar-section">
        <div className="sidebar-head">
          <span>Plugins</span>
          <span className={`kind-tag ${connected ? 'source' : ''}`}>
            {connected ? 'live' : 'offline'}
          </span>
        </div>

        <div className="sidebar-list">
          {libraries.map((library) => (
            <button
              key={library.key}
              className="plugin-row"
              title="Add to the graph"
              onClick={() => onAdd(library)}
            >
              <span className="name">{library.name}</span>
              <span className={`kind-tag ${library.source ? 'source' : 'sink'}`}>
                {library.source ? 'source' : 'sink'}
              </span>
            </button>
          ))}

          {libraries.length === 0 && (
            <div className="empty">
              {connected ? 'No plugins found.' : 'Waiting for the engine…'}
            </div>
          )}
        </div>
      </div>
    </aside>
  )
}

export default Sidebar
