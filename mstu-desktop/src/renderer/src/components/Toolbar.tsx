type Props = {
  name: string
  nodes: number
  connectors: number
  running: boolean
  connected: boolean
  error: string | null
  onDismissError: () => void
  onToggleRun: () => void
}

function Toolbar(props: Props): JSX.Element {
  return (
    <header className="toolbar">
      <span className="title">{props.name}</span>
      <span className="meta">
        {props.nodes} nodes · {props.connectors} connectors
      </span>

      {props.error && (
        <button className="toolbar-error" onClick={props.onDismissError} title="Dismiss">
          {props.error} ✕
        </button>
      )}

      <span className="spacer" />

      {!props.connected && <span className="offline">engine offline</span>}

      {props.running ? (
        <button className="button danger" onClick={props.onToggleRun}>
          ■ Stop
        </button>
      ) : (
        <button className="button primary" onClick={props.onToggleRun} disabled={!props.connected}>
          ▶ Start
        </button>
      )}
    </header>
  )
}

export default Toolbar
