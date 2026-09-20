import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react'

import { byTarget, leaves, nested, pathKey, typeLabel } from '../fields'
import type { Connector, Field, Library, Subscription } from '../types'
import Picker from './Picker'

/// One side of the editor: whose message it is and what it carries.
type Payload = {
  plugin: string
  message: string
  fields: Field[]
}

type Props = {
  connector: Connector
  source: Library
  target: Library

  onSetMapping: (to: number[], from: number[] | null) => void
  onSetSubscriptionMapping: (subscriptionId: string, to: number[], from: number[] | null) => void
  onAutoMap: (subscriptionId?: string) => void

  onRemoveConnector: () => void

  onAddSubscription: () => string | null
  onRemoveSubscription: (subscriptionId: string) => void
  onChangeSubscription: (subscriptionId: string, patch: Partial<Subscription>) => void

  onClose: () => void
}

type Tab = 'message' | 'events'

type Point = { x: number; y: number }
type Side = 'source' | 'target'
type Wire = { key: string; from: Point; to: Point; type: string }

/// A wire takes the colour of what it carries, the same colours the chips use.
const TYPE_COLOUR: Record<string, string> = {
  bytes: 'var(--accent)',
  string: 'var(--live)',
  enum: 'var(--live)',
  uint: 'var(--sink)',
  int: 'var(--sink)',
  float: 'var(--sink)',
  bool: 'var(--transform)'
}

const colourOf = (type: string): string => TYPE_COLOUR[type] ?? 'var(--text-dim)'

function curve(from: Point, to: Point): string {
  const bend = Math.max(40, Math.abs(to.x - from.x) / 2)

  return `M ${from.x} ${from.y} C ${from.x + bend} ${from.y}, ${to.x - bend} ${to.y}, ${to.x} ${to.y}`
}

/// Both messages as JSON, wired field to field by dragging. A wire copies the
/// value across as it is; transforms are not here yet.
function MappingEditor(props: Props): JSX.Element {
  const { connector, source, target } = props

  const surface = useRef<HTMLDivElement>(null)
  const sockets = useRef(new Map<string, HTMLElement>())

  const [tab, setTab] = useState<Tab>('message')
  const [openSubscription, setOpenSubscription] = useState<string | null>(null)
  const [wires, setWires] = useState<Wire[]>([])
  const [drag, setDrag] = useState<{
    side: Side
    path: number[]
    at: Point
  } | null>(null)
  const [picked, setPicked] = useState<string | null>(null)
  const [problem, setProblem] = useState<string | null>(null)

  /// The right-click menu, on the mapping it was opened over.
  const [menu, setMenu] = useState<{ key: string; x: number; y: number } | null>(null)
  const menuBox = useRef<HTMLDivElement>(null)

  /// The subscription the events tab is wiring, defaulting to the first one.
  const subscription =
    tab === 'events'
      ? (connector.subscriptions.find((item) => item.id === openSubscription) ??
        connector.subscriptions[0])
      : undefined

  const event = subscription ? source.events[subscription.event] : undefined
  const command = subscription ? target.commands[subscription.command] : undefined

  const sourcePayload: Payload = subscription
    ? {
        plugin: source.name,
        message: 'event payload',
        fields: event?.schema?.fields ?? []
      }
    : {
        plugin: source.name,
        message: 'output message',
        fields: source.output?.fields ?? []
      }

  const targetPayload: Payload = subscription
    ? {
        plugin: target.name,
        message: 'command payload',
        fields: command?.schema?.fields ?? []
      }
    : {
        plugin: target.name,
        message: 'input message',
        fields: target.input?.fields ?? []
      }

  const filled = useMemo(
    () => byTarget(subscription ? subscription.mappings : connector.mappings),
    [subscription, connector.mappings]
  )

  const onSet = (to: number[], from: number[] | null): void =>
    subscription
      ? props.onSetSubscriptionMapping(subscription.id, to, from)
      : props.onSetMapping(to, from)

  // Held still between renders: a fresh array each time would restart the
  // effect that measures the wires, over and over.
  const sourceLeaves = useMemo(() => leaves(sourcePayload.fields), [sourcePayload.fields])
  const targetLeaves = useMemo(() => leaves(targetPayload.fields), [targetPayload.fields])

  const id = (side: Side, path: number[]): string => `${side}:${pathKey(path)}`

  /// Where a row's socket sits, in the surface's own coordinates.
  const socketAt = useCallback((side: Side, path: number[]): Point | null => {
    const element = sockets.current.get(`${side}:${pathKey(path)}`)
    const box = surface.current?.getBoundingClientRect()

    if (!element || !box) return null

    const rect = element.getBoundingClientRect()

    return {
      x: rect.left + rect.width / 2 - box.left,
      y: rect.top + rect.height / 2 - box.top
    }
  }, [])

  const redraw = useCallback(() => {
    const drawn: Wire[] = []

    filled.forEach((from, key) => {
      const a = socketAt('source', from)
      const b = socketAt('target', key.split('.').map(Number))
      const leaf = targetLeaves.find((item) => pathKey(item.path) === key)

      if (a && b) drawn.push({ key, from: a, to: b, type: leaf?.field.type ?? '' })
    })

    // Only a real change is worth a render; the measurements repeat otherwise.
    setWires((current) => (JSON.stringify(current) === JSON.stringify(drawn) ? current : drawn))
  }, [filled, socketAt, targetLeaves])

  useLayoutEffect(redraw, [redraw])

  useEffect(() => {
    setPicked(null)
    setProblem(null)
    setMenu(null)
  }, [tab, subscription?.id])

  // A menu belongs to where it was opened, so anything moving closes it.
  useEffect(() => {
    if (!menu) return

    const away = (event: PointerEvent): void => {
      const at = event.target

      if (!(at instanceof Node) || !menuBox.current?.contains(at)) setMenu(null)
    }

    const shut = (): void => setMenu(null)

    window.addEventListener('pointerdown', away)
    window.addEventListener('resize', shut)

    return () => {
      window.removeEventListener('pointerdown', away)
      window.removeEventListener('resize', shut)
    }
  }, [menu])

  // Rows move when either side scrolls or the window changes shape.
  useEffect(() => {
    const observer = new ResizeObserver(redraw)
    if (surface.current) observer.observe(surface.current)

    window.addEventListener('resize', redraw)

    return () => {
      observer.disconnect()
      window.removeEventListener('resize', redraw)
    }
  }, [redraw])

  useEffect(() => {
    const onKey = (event: KeyboardEvent): void => {
      // An open menu or dropdown takes the first Escape for itself.
      if (event.key === 'Escape') {
        if (menu) setMenu(null)
        else if (!document.querySelector('.picker-list')) props.onClose()
      }

      if ((event.key === 'Delete' || event.key === 'Backspace') && picked) {
        onSet(picked.split('.').map(Number), null)
        setPicked(null)
      }
    }

    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [props, onSet, picked, menu])

  /// Finishes a drag on whatever row it was released over.
  function drop(event: React.PointerEvent): void {
    if (!drag) return

    setDrag(null)

    const row = (
      document.elementFromPoint(event.clientX, event.clientY) as HTMLElement | null
    )?.closest('[data-side]') as HTMLElement | null

    const side = row?.dataset.side as Side | undefined
    const path = row?.dataset.path?.split('.').map(Number)

    if (!side || !path || side === drag.side) return

    const from = drag.side === 'source' ? drag.path : path
    const to = drag.side === 'source' ? path : drag.path

    const fromField = sourceLeaves.find((leaf) => pathKey(leaf.path) === pathKey(from))?.field
    const toField = targetLeaves.find((leaf) => pathKey(leaf.path) === pathKey(to))?.field

    if (!fromField || !toField) return

    if (fromField.type !== toField.type) {
      setProblem(
        `${fromField.name} is ${typeLabel(fromField)}, ${toField.name} is ${typeLabel(toField)}.`
      )

      return
    }

    setProblem(null)
    onSet(to, from)
    setPicked(pathKey(to))
  }

  /// Right-clicking a mapping selects it and opens its menu where the pointer is.
  function openMenu(event: React.MouseEvent, key: string): void {
    event.preventDefault()
    event.stopPropagation()

    setPicked(key)
    setMenu({
      key,
      x: Math.min(event.clientX, window.innerWidth - 200),
      y: Math.min(event.clientY, window.innerHeight - 80)
    })
  }

  const wired = (side: Side, path: number[]): boolean =>
    side === 'target'
      ? filled.has(pathKey(path))
      : [...filled.values()].some((item) => pathKey(item) === pathKey(path))

  /// One line of the payload, which is either a record opening or a field.
  function rows(side: Side, fields: Field[], path: number[] = [], depth = 1): JSX.Element[] {
    return fields.flatMap((field, index) => {
      const here = [...path, index]
      const indent = { paddingLeft: `${depth * 12}px` }

      if (nested(field)) {
        return [
          <div className="json-row branch" key={pathKey(here)} style={indent}>
            <span className="key">&quot;{field.name}&quot;</span>
            <span className="punct">: {'{'}</span>
          </div>,
          ...rows(side, field.fields ?? [], here, depth + 1),
          <div className="json-row branch" key={`${pathKey(here)}-end`} style={indent}>
            <span className="punct">{'},'}</span>
          </div>
        ]
      }

      const on = wired(side, here)

      return [
        <div
          className={`json-row ${on ? 'wired' : ''} ${
            side === 'target' && picked === pathKey(here) ? 'picked' : ''
          }`}
          key={pathKey(here)}
          // A wired field is boxed in the colour of what it carries.
          style={{ ...indent, ['--box' as string]: on ? colourOf(field.type) : undefined }}
          data-side={side}
          data-path={pathKey(here)}
          title={field.description || field.name}
          onClick={() => side === 'target' && setPicked(pathKey(here))}
          onContextMenu={(event) => side === 'target' && on && openMenu(event, pathKey(here))}
        >
          <span className="key">&quot;{field.name}&quot;</span>
          <span className="punct">:</span>
          <span className={`type-chip ${field.type}`}>{typeLabel(field)}</span>

          {field.variants && (
            <span className="variants">{field.variants.map((item) => item.name).join(' | ')}</span>
          )}

          <span
            className={`socket ${side}`}
            style={{ background: on ? colourOf(field.type) : undefined }}
            onPointerDown={(event) => {
              event.stopPropagation()
              event.preventDefault()
              surface.current?.setPointerCapture(event.pointerId)
              setDrag({
                side,
                path: here,
                at: { x: event.clientX, y: event.clientY }
              })
            }}
            ref={(element) => {
              if (element) sockets.current.set(id(side, here), element)
              else sockets.current.delete(id(side, here))
            }}
          />
        </div>
      ]
    })
  }

  const pane = (side: Side, payload: Payload, chooser?: JSX.Element | false): JSX.Element => (
    <div className={`map-pane ${side}`}>
      <div className="map-pane-head">
        <span className="who">{payload.plugin}</span>
        <span className="what">{payload.message}</span>
      </div>

      {chooser && <div className="map-pane-pick">{chooser}</div>}

      <div className="map-json" onScroll={redraw}>
        <div className="json-row branch">
          <span className="punct">{'{'}</span>
        </div>

        {rows(side, payload.fields)}

        <div className="json-row branch">
          <span className="punct">{'}'}</span>
        </div>

        {payload.fields.length === 0 && <div className="empty">No fields.</div>}
      </div>
    </div>
  )

  const local = (point: Point): Point => {
    const box = surface.current?.getBoundingClientRect()

    return box ? { x: point.x - box.left, y: point.y - box.top } : point
  }

  const missing = targetLeaves.filter((leaf) => !filled.has(pathKey(leaf.path))).length

  /// The message tab's own coverage, which the tab shows even while events are open.
  const inputLeaves = leaves(target.input?.fields ?? []).length
  const messageCoverage = `${new Set(connector.mappings.map((mapping) => pathKey(mapping.to))).size}/${inputLeaves}`
  const chosen = picked ? filled.get(picked) : undefined

  const pendingFrom = drag && socketAt(drag.side, drag.path)
  const pending =
    drag && pendingFrom
      ? drag.side === 'source'
        ? curve(pendingFrom, local(drag.at))
        : curve(local(drag.at), pendingFrom)
      : null

  const wiring = tab !== 'events' || subscription !== undefined

  const rail = (
    <aside className="subscriptions">
      <div className="rail-head">Subscriptions</div>

      <div className="rail-list">
        {connector.subscriptions.map((item) => (
          <div
            className={`rail-row ${item.id === subscription?.id ? 'active' : ''}`}
            key={item.id}
            onClick={() => setOpenSubscription(item.id)}
          >
            <span className="line">
              <span className="badge event">on</span>
              {source.events[item.event]?.name ?? '—'}
            </span>

            <span className="line">
              <span className="badge command">call</span>
              {target.commands[item.command]?.name ?? '—'}
            </span>

            <button
              className="remove"
              title="Remove this subscription"
              onClick={(click) => {
                click.stopPropagation()
                props.onRemoveSubscription(item.id)
              }}
            >
              ✕
            </button>
          </div>
        ))}

        {connector.subscriptions.length === 0 && (
          <p className="rail-empty">
            {source.events.length === 0
              ? `${source.name} publishes no events.`
              : 'Nothing subscribed yet.'}
          </p>
        )}
      </div>

      <button
        className="button subtle"
        onClick={() => setOpenSubscription(props.onAddSubscription())}
        disabled={source.events.length === 0 || target.commands.length === 0}
      >
        + Subscribe
      </button>
    </aside>
  )

  return (
    <div
      className="editor-scrim"
      onPointerDown={(event) => event.target === event.currentTarget && props.onClose()}
    >
      <div className="mapping-editor">
        <header className="editor-head">
          <div className="editor-title">
            <span className="what">Wiring</span>
            <span className="route">
              {source.name} <span className="arrow">▸</span> {target.name}
            </span>
          </div>

          {/* Nothing is being wired until the events tab has a subscription open. */}
          {wiring && (
            <>
              <span className={`mapped ${missing > 0 ? 'short' : 'ok'}`}>
                {targetLeaves.length - missing} / {targetLeaves.length} fields mapped
              </span>

              <button
                className="button"
                onClick={() => props.onAutoMap(subscription?.id)}
                title="Wire fields that match by name or type"
                disabled={targetLeaves.length === 0}
              >
                Auto wire
              </button>
            </>
          )}

          <button className="button primary" title="Close (Esc)" onClick={props.onClose}>
            Done
          </button>
        </header>

        <div className="editor-tabs">
          <button
            className={`tab ${tab === 'message' ? 'active' : ''}`}
            onClick={() => setTab('message')}
          >
            Message
            <span className="count">{messageCoverage}</span>
          </button>

          <button
            className={`tab ${tab === 'events' ? 'active' : ''}`}
            onClick={() => setTab('events')}
          >
            Event subscribe
            <span className="count">{connector.subscriptions.length}</span>
          </button>
        </div>

        <div
          className="editor-body"
          ref={surface}
          onPointerMove={(event) =>
            setDrag((current) =>
              current ? { ...current, at: { x: event.clientX, y: event.clientY } } : current
            )
          }
          onPointerUp={drop}
        >
          {tab === 'events' && rail}

          <div className="map-area">
            {!wiring ? (
              <div className="nothing">
                {source.events.length === 0
                  ? `${source.name} publishes no events.`
                  : 'Subscribe an event to a command to wire its payload.'}
              </div>
            ) : (
              <>
                {pane(
                  'source',
                  sourcePayload,
                  subscription && (
                    <Picker
                      kind="event"
                      value={subscription.event}
                      items={source.events}
                      onPick={(next) =>
                        props.onChangeSubscription(subscription.id, { event: next })
                      }
                    />
                  )
                )}

                {pane(
                  'target',
                  targetPayload,
                  subscription && (
                    <Picker
                      kind="command"
                      value={subscription.command}
                      items={target.commands}
                      onPick={(next) =>
                        props.onChangeSubscription(subscription.id, { command: next })
                      }
                    />
                  )
                )}
              </>
            )}
          </div>

          <svg className="wires">
            {wires.map((wire) => (
              <g key={wire.key} onContextMenu={(event) => openMenu(event, wire.key)}>
                <path
                  className="wire-hit"
                  d={curve(wire.from, wire.to)}
                  onClick={() => setPicked(wire.key)}
                />
                <path
                  className={`wire ${picked === wire.key ? 'picked' : ''}`}
                  style={{ stroke: colourOf(wire.type) }}
                  d={curve(wire.from, wire.to)}
                  onClick={() => setPicked(wire.key)}
                />
              </g>
            ))}

            {pending && <path className="wire pending" d={pending} />}
          </svg>
        </div>

        <footer className="editor-foot">
          {problem ? (
            <span className="problem">⚠ Both ends must be the same type. {problem}</span>
          ) : picked && chosen ? (
            <span className="detail">
              <span className="mono">
                {sourceLeaves.find((leaf) => pathKey(leaf.path) === pathKey(chosen))?.label}
              </span>
              <span className="arrow">→</span>
              <span className="mono">
                {targetLeaves.find((leaf) => pathKey(leaf.path) === picked)?.label}
              </span>
              <span className="dim">copied as it is — transforms come later</span>
              <button
                className="remove"
                title="Remove this wire (Delete)"
                onClick={() => onSet(picked.split('.').map(Number), null)}
              >
                ✕
              </button>
            </span>
          ) : wiring ? (
            <span className="dim">
              Drag between two fields of the same type to wire them. Right-click a wire for what
              can be done to it.
            </span>
          ) : (
            <span className="dim">
              A subscription calls a command on {target.name} whenever {source.name} raises an
              event.
            </span>
          )}

          {/* Down here, well away from Done: it takes the whole link. */}
          <button
            className="remove-link"
            title="Remove the link itself, with its mappings and subscriptions"
            onClick={props.onRemoveConnector}
          >
            ✕ Remove link
          </button>
        </footer>

        {menu && (
          <div className="context-menu" style={{ left: menu.x, top: menu.y }} ref={menuBox}>
            <button
              className="menu-item danger"
              onClick={() => {
                onSet(menu.key.split('.').map(Number), null)
                setPicked(null)
                setMenu(null)
              }}
            >
              Remove mapping
              <span className="key">Del</span>
            </button>
          </div>
        )}
      </div>
    </div>
  )
}

export default MappingEditor
