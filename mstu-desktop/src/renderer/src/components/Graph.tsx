import { useRef, useState } from 'react'

import type { Connector, Library, Node } from '../types'

export const NODE_WIDTH = 280
export const NODE_HEIGHT = 210

type Props = {
  nodes: Node[]
  connectors: Connector[]
  libraries: Library[]
  selectedConnector: string | null

  /// Whether the pipeline has started: a node has no worker to switch before
  /// that, so the power switches stay disabled.
  started: boolean

  onSelectConnector: (id: string) => void
  onToggleNode: (id: number, running: boolean) => void
  onRemoveNode: (id: number) => void
  onMoveNode: (id: number, x: number, y: number) => void
  onLink: (from: number, to: number) => void
  registerFrame: (plugin: string, frame: HTMLIFrameElement | null) => void
  onFrameReady: (plugin: string) => void
}

type Point = { x: number; y: number }

/// A link in progress: where it started and where the pointer is now.
type Link = { from: number; at: Point }

const outputPort = (node: Node): Point => ({
  x: node.x + NODE_WIDTH,
  y: node.y + NODE_HEIGHT / 2
})

const inputPort = (node: Node): Point => ({ x: node.x, y: node.y + NODE_HEIGHT / 2 })

function curve(from: Point, to: Point): string {
  const bend = Math.max(50, Math.abs(to.x - from.x) / 2)

  return `M ${from.x} ${from.y} C ${from.x + bend} ${from.y}, ${to.x - bend} ${to.y}, ${to.x} ${to.y}`
}

function Graph(props: Props): JSX.Element {
  const { nodes, connectors, libraries, selectedConnector } = props

  const surface = useRef<HTMLDivElement>(null)
  const [dragging, setDragging] = useState<number | null>(null)
  const [link, setLink] = useState<Link | null>(null)
  const grab = useRef({ x: 0, y: 0 })

  const library = (key: string): Library | undefined => libraries.find((item) => item.key === key)
  const node = (id: number): Node | undefined => nodes.find((item) => item.id === id)

  const linked = (id: number, side: 'in' | 'out'): boolean =>
    connectors.some((connector) => (side === 'in' ? connector.to : connector.from) === id)

  const localPoint = (event: React.PointerEvent): Point | null => {
    const box = surface.current?.getBoundingClientRect()
    if (!box) return null

    return { x: event.clientX - box.left, y: event.clientY - box.top }
  }

  function startDrag(event: React.PointerEvent, item: Node): void {
    const point = localPoint(event)
    if (!point) return

    grab.current = { x: point.x - item.x, y: point.y - item.y }
    setDragging(item.id)
    event.currentTarget.setPointerCapture(event.pointerId)
  }

  function startLink(event: React.PointerEvent, item: Node): void {
    const point = localPoint(event)
    if (!point) return

    event.stopPropagation()
    event.preventDefault()

    setLink({ from: item.id, at: point })
    surface.current?.setPointerCapture(event.pointerId)
  }

  function onPointerMove(event: React.PointerEvent): void {
    const point = localPoint(event)
    if (!point) return

    if (link) {
      setLink({ ...link, at: point })
      return
    }

    if (dragging !== null) {
      props.onMoveNode(
        dragging,
        Math.max(0, point.x - grab.current.x),
        Math.max(0, point.y - grab.current.y)
      )
    }
  }

  /// Dropping anywhere on a node counts, not just on its port.
  function onPointerUp(event: React.PointerEvent): void {
    setDragging(null)

    if (!link) return

    const dropped = document
      .elementFromPoint(event.clientX, event.clientY)
      ?.closest('[data-node]')
      ?.getAttribute('data-node')

    setLink(null)

    if (dropped === null || dropped === undefined) return

    const to = Number(dropped)
    const target = node(to)

    if (to !== link.from && library(target?.library ?? '')?.input) {
      props.onLink(link.from, to)
    }
  }

  const from = link ? node(link.from) : undefined

  return (
    <div
      className={`graph ${link ? 'linking' : ''}`}
      ref={surface}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
    >
      <svg className="edges">
        {connectors.map((connector) => {
          const source = node(connector.from)
          const target = node(connector.to)
          if (!source || !target) return null

          const path = curve(outputPort(source), inputPort(target))

          return (
            <g key={connector.id}>
              <path
                className="edge-hit"
                d={path}
                onClick={() => props.onSelectConnector(connector.id)}
              />
              <path
                className={`edge ${connector.id === selectedConnector ? 'selected' : ''}`}
                d={path}
                onClick={() => props.onSelectConnector(connector.id)}
              />
            </g>
          )
        })}

      </svg>

      {/* Its own layer: the link being dragged stays above the nodes. */}
      {link && from && (
        <svg className="edges overlay">
          <path className="edge pending" d={curve(outputPort(from), link.at)} />
        </svg>
      )}

      {nodes.map((item) => {
        const descriptor = library(item.library)

        return (
          <div
            key={item.id}
            data-node={item.id}
            className={`node ${props.started && !item.running ? 'off' : ''}`}
            style={{ left: item.x, top: item.y, width: NODE_WIDTH, height: NODE_HEIGHT }}
          >
            {descriptor?.source && <span className="stripe" />}

            <div className="node-head" onPointerDown={(event) => startDrag(event, item)}>
              <span className="name">{item.name}</span>
              <span className="id">({item.plugin})</span>

              <button
                className={`power ${item.running ? 'on' : ''}`}
                role="switch"
                aria-checked={item.running}
                aria-label={`${item.name} power`}
                disabled={!props.started}
                title={
                  props.started
                    ? item.running
                      ? 'Switch this node off'
                      : 'Switch this node on'
                    : 'Start the pipeline to switch nodes'
                }
                /* The header drags the node, this must not. */
                onPointerDown={(event) => event.stopPropagation()}
                onClick={() => props.onToggleNode(item.id, !item.running)}
              >
                <span className="knob" />
              </button>

              <button
                className="remove"
                title="Remove this plugin"
                aria-label={`Remove ${item.name}`}
                onPointerDown={(event) => event.stopPropagation()}
                onClick={() => props.onRemoveNode(item.id)}
              >
                ✕
              </button>
            </div>

            <div className="node-body">
              {item.ui ? (
                <iframe
                  className="plugin-ui"
                  title={item.name}
                  src={`mstu-plugin://${item.plugin.toLowerCase()}/index.html`}
                  ref={(frame) => props.registerFrame(item.plugin, frame)}
                  onLoad={() => props.onFrameReady(item.plugin)}
                />
              ) : (
                <div className="node-ui-slot">no plugin UI</div>
              )}
            </div>

            {descriptor?.input && (
              <span className={`port in ${linked(item.id, 'in') ? 'linked' : ''}`} />
            )}

            {descriptor?.output && (
              <span
                className={`port out ${linked(item.id, 'out') ? 'linked' : ''} ${
                  link?.from === item.id ? 'active' : ''
                }`}
                title="Drag onto another plugin to link"
                onPointerDown={(event) => startLink(event, item)}
              />
            )}
          </div>
        )
      })}
    </div>
  )
}

export default Graph
