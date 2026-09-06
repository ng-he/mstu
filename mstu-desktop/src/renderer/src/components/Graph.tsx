import { useEffect, useRef, useState } from 'react'

import type { Connector, Library, Node } from '../types'

/// Used until a plugin UI declares a size of its own.
export const NODE_WIDTH = 280
export const NODE_HEIGHT = 210

/// What the host will grant a plugin that asks for a size.
export const MIN_NODE_WIDTH = 240
export const MIN_NODE_HEIGHT = 160
export const MAX_NODE_WIDTH = 1080
export const MAX_NODE_HEIGHT = 720

/// How far the canvas may be zoomed, and the dot spacing at 1:1.
const MIN_ZOOM = 0.25
const MAX_ZOOM = 2.5
const GRID = 22

const clamp = (value: number, low: number, high: number): number =>
  Math.min(high, Math.max(low, value))

/// Pan offset in screen pixels plus a scale, applied to one wrapper so nodes
/// and edges stay in the same coordinate space.
type View = { x: number; y: number; zoom: number }

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

/// Screen point to graph point, undoing the current pan and zoom.
function toGraph(view: View, box: DOMRect, clientX: number, clientY: number): Point {
  return {
    x: (clientX - box.left - view.x) / view.zoom,
    y: (clientY - box.top - view.y) / view.zoom,
  }
}

/// A link in progress: where it started and where the pointer is now.
type Link = { from: number; at: Point }

const outputPort = (node: Node): Point => ({
  x: node.x + node.width,
  y: node.y + node.height / 2,
})

const inputPort = (node: Node): Point => ({
  x: node.x,
  y: node.y + node.height / 2,
})

function curve(from: Point, to: Point): string {
  const bend = Math.max(50, Math.abs(to.x - from.x) / 2)

  return `M ${from.x} ${from.y} C ${from.x + bend} ${from.y}, ${to.x - bend} ${to.y}, ${to.x} ${to.y}`
}

function Graph(props: Props): JSX.Element {
  const { nodes, connectors, libraries, selectedConnector } = props

  const surface = useRef<HTMLDivElement>(null)
  const [dragging, setDragging] = useState<number | null>(null)
  const [link, setLink] = useState<Link | null>(null)
  const [view, setView] = useState<View>({ x: 0, y: 0, zoom: 1 })
  const [panning, setPanning] = useState(false)
  const grab = useRef({ x: 0, y: 0 })
  const panFrom = useRef({ x: 0, y: 0 })

  /// Wheel has to be a native non-passive listener: React's is passive, and
  /// without preventDefault a pinch zooms the whole window instead.
  useEffect(() => {
    const element = surface.current
    if (!element) return

    const onWheel = (event: WheelEvent): void => {
      event.preventDefault()

      const box = element.getBoundingClientRect()
      const px = event.clientX - box.left
      const py = event.clientY - box.top

      setView((current) => {
        const zoom = clamp(current.zoom * Math.exp(-event.deltaY * 0.0015), MIN_ZOOM, MAX_ZOOM)

        // Keep whatever is under the cursor pinned there.
        return {
          zoom,
          x: px - ((px - current.x) * zoom) / current.zoom,
          y: py - ((py - current.y) * zoom) / current.zoom,
        }
      })
    }

    element.addEventListener('wheel', onWheel, { passive: false })

    return () => element.removeEventListener('wheel', onWheel)
  }, [])

  /// Steps the zoom about the middle of the canvas, for the buttons.
  function zoomBy(factor: number): void {
    const box = surface.current?.getBoundingClientRect()
    if (!box) return

    const px = box.width / 2
    const py = box.height / 2

    setView((current) => {
      const zoom = clamp(current.zoom * factor, MIN_ZOOM, MAX_ZOOM)

      return {
        zoom,
        x: px - ((px - current.x) * zoom) / current.zoom,
        y: py - ((py - current.y) * zoom) / current.zoom,
      }
    })
  }

  const library = (key: string): Library | undefined => libraries.find((item) => item.key === key)
  const node = (id: number): Node | undefined => nodes.find((item) => item.id === id)

  const linked = (id: number, side: 'in' | 'out'): boolean =>
    connectors.some((connector) => (side === 'in' ? connector.to : connector.from) === id)

  const localPoint = (event: React.PointerEvent): Point | null => {
    const box = surface.current?.getBoundingClientRect()
    if (!box) return null

    return toGraph(view, box, event.clientX, event.clientY)
  }

  /// Empty canvas drags the view, and the middle button does it anywhere.
  function startPan(event: React.PointerEvent): void {
    const target = event.target as HTMLElement
    const background = target === surface.current || target.classList.contains('viewport')

    if (event.button !== 1 && !(event.button === 0 && background)) return

    // Otherwise the drag starts selecting text across the canvas.
    event.preventDefault()

    panFrom.current = { x: event.clientX - view.x, y: event.clientY - view.y }
    setPanning(true)
    surface.current?.setPointerCapture(event.pointerId)
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
    if (panning) {
      setView((current) => ({
        ...current,
        x: event.clientX - panFrom.current.x,
        y: event.clientY - panFrom.current.y,
      }))

      return
    }

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
        Math.max(0, point.y - grab.current.y),
      )
    }
  }

  /// Dropping anywhere on a node counts, not just on its port.
  function onPointerUp(event: React.PointerEvent): void {
    setDragging(null)
    setPanning(false)

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

  // Doubling keeps the dots aligned to the same origin while stopping them
  // from turning into moire when zoomed out.
  let grid = GRID * view.zoom
  while (grid < 12) grid *= 2

  return (
    <div
      className={`graph ${link ? 'linking' : ''} ${panning ? 'panning' : ''}`}
      ref={surface}
      onPointerDown={startPan}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
      style={{
        // The dots belong to the canvas, so they pan and zoom with it.
        backgroundSize: `${grid}px ${grid}px`,
        backgroundPosition: `${view.x}px ${view.y}px`,
      }}
    >
      <div
        className="viewport"
        style={{
          transform: `translate(${view.x}px, ${view.y}px) scale(${view.zoom})`,
        }}
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
              style={{
                left: item.x,
                top: item.y,
                width: item.width,
                height: item.height,
              }}
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

      <div className="zoom-controls">
        <button className="icon-button" title="Zoom out" onClick={() => zoomBy(1 / 1.2)}>
          −
        </button>
        <button
          className="level"
          title="Reset the view"
          onClick={() => setView({ x: 0, y: 0, zoom: 1 })}
        >
          {Math.round(view.zoom * 100)}%
        </button>
        <button className="icon-button" title="Zoom in" onClick={() => zoomBy(1.2)}>
          +
        </button>
      </div>
    </div>
  )
}

export default Graph
