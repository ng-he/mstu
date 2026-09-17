import { useEffect, useLayoutEffect, useRef, useState } from 'react'

import Graph, {
  MAX_NODE_HEIGHT,
  MAX_NODE_WIDTH,
  MIN_NODE_HEIGHT,
  MIN_NODE_WIDTH,
  NODE_HEIGHT,
  NODE_WIDTH
} from './components/Graph'
import Inspector from './components/Inspector'
import PluginPopup from './components/PluginPopup'
import Sidebar from './components/Sidebar'
import Toolbar from './components/Toolbar'
import { engine } from './engine'
import { PluginHost, type HostHooks } from './pluginHost'
import type { Connector, Field, Library, Mapping, Node, Popup, Subscription } from './types'

let nextId = 1
const newId = (prefix: string): string => `${prefix}${nextId++}`

const clamp = (value: number, low: number, high: number): number =>
  Math.min(high, Math.max(low, value))

/// A new row defaults to the first unmapped target field and a source field
/// of the same type: 0 -> 0 is usually a type mismatch the engine will drop.
function defaultMapping(source: Field[], target: Field[], taken: Set<number>): Mapping {
  const to = Math.max(
    0,
    target.findIndex((_, index) => !taken.has(index))
  )

  const from = Math.max(
    0,
    source.findIndex((field) => field.type === target[to]?.type)
  )

  return { id: newId('m'), from, to }
}

/// The engine keys a subscription by event and command, so two rows sharing a
/// pair would be a single registration that either row could delete.
function pairTaken(
  subscriptions: Subscription[],
  event: number,
  command: number,
  except?: string
): boolean {
  return subscriptions.some(
    (item) => item.id !== except && item.event === event && item.command === command
  )
}

/// First event and command pair no other subscription has claimed.
function freePair(
  subscriptions: Subscription[],
  events: number,
  commands: number
): { event: number; command: number } | null {
  for (let event = 0; event < events; event++) {
    for (let command = 0; command < commands; command++) {
      if (!pairTaken(subscriptions, event, command)) return { event, command }
    }
  }

  return null
}

/// Fills a whole payload at once: every target field takes the source field
/// of the same name, or failing that the first one of the same type.
///
/// A command payload is all-or-nothing, so an unmapped field is a message the
/// engine drops.
function autoMappings(source: Field[], target: Field[]): Mapping[] {
  return target.flatMap((field, to) => {
    const named = source.findIndex((item) => item.name === field.name)
    const from = named >= 0 ? named : source.findIndex((item) => item.type === field.type)

    return from < 0 ? [] : [{ id: newId('m'), from, to }]
  })
}

function App(): JSX.Element {
  const [connected, setConnected] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [libraries, setLibraries] = useState<Library[]>([])
  const [pipeline, setPipeline] = useState<number | null>(null)
  const [nodes, setNodes] = useState<Node[]>([])
  const [connectors, setConnectors] = useState<Connector[]>([])
  const [connectorId, setConnectorId] = useState<string | null>(null)
  const [running, setRunning] = useState(false)
  const [popups, setPopups] = useState<Popup[]>([])

  const hadGraph = useRef(false)

  const report = (problem: unknown): void =>
    setError(problem instanceof Error ? problem.message : String(problem))

  /// Runs every plugin page and hands it the engine's traffic; the hooks below keep it current.
  const [host] = useState(() => new PluginHost({} as HostHooks))

  const closePopup = (plugin: string): void => {
    setPopups((current) => current.filter((item) => item.plugin !== plugin))
    host.setPopup(plugin, false)
  }

  useLayoutEffect(() => {
    host.hooks = {
      library: (plugin) =>
        libraryOf(nodes.find((item) => item.plugin === plugin)) ?? undefined,

      resize: (plugin, role, width, height) => {
        if (role === 'popup') {
          setPopups((current) =>
            current.map((item) =>
              item.plugin === plugin
                ? { ...item, width: width ?? item.width, height: height ?? item.height }
                : item
            )
          )

          return
        }

        setNodes((current) =>
          current.map((node) =>
            node.plugin === plugin
              ? {
                  ...node,
                  width: clamp(width ?? node.width, MIN_NODE_WIDTH, MAX_NODE_WIDTH),
                  height: clamp(height ?? node.height, MIN_NODE_HEIGHT, MAX_NODE_HEIGHT)
                }
              : node
          )
        )
      },

      openPopup: (plugin, request) => {
        const page = request.page.replace(/^\/+/, '')
        if (!page) return

        const popup: Popup = {
          plugin,
          page,
          title: request.title ?? nodes.find((item) => item.plugin === plugin)?.name ?? '',
          width: request.width ?? 480,
          height: request.height ?? 320
        }

        setPopups((current) => [...current.filter((item) => item.plugin !== plugin), popup])
        host.setPopup(plugin, true)
      },

      closePopup,
      report
    }
  })

  // ---------- engine connection ----------

  useEffect(() => {
    // The engine may already be connected before this mounted.
    window.mstu.connected().then(setConnected).catch(report)

    const offStatus = window.mstu.onStatus(setConnected)
    const offEvent = window.mstu.onEvent((notice) => host.event(notice.plugin, notice.event, notice.values))
    const offLive = window.mstu.onLive((live) => host.live(live.plugin, live.values))

    return () => {
      offStatus()
      offEvent()
      offLive()
    }
  }, [host])

  /// The engine holds every pipeline in memory, so a restart invalidates
  /// every node and plugin id the canvas is holding. Start over rather than
  /// sending ids the new process has never heard of.
  useEffect(() => {
    if (connected) return

    if (hadGraph.current) {
      setError('Engine disconnected — the canvas was cleared, rebuild it')
    }

    hadGraph.current = false

    setPipeline(null)
    setNodes([])
    setConnectors([])
    setConnectorId(null)
    setRunning(false)
    setPopups([])
    host.clear()
  }, [connected, host])

  useEffect(() => {
    if (nodes.length > 0) hadGraph.current = true
  }, [nodes])

  useEffect(() => {
    if (!connected || pipeline !== null) return

    engine
      .describe()
      .then((described) => {
        setLibraries(described.libraries)
        return engine.createPipeline('Untitled pipeline')
      })
      .then((created) => setPipeline(created.pipeline))
      .catch(report)
  }, [connected, pipeline])

  // ---------- graph editing ----------

  async function addNode(library: Library): Promise<void> {
    if (pipeline === null) return

    try {
      const created = await engine.createPlugin(library.key)
      const added = await engine.addNode(pipeline, created.plugin)

      setNodes((current) => [
        ...current,
        {
          id: added.node,
          plugin: created.plugin,
          library: library.key,
          name: library.name,
          ui: Boolean(created.ui),
          running: false,
          width: NODE_WIDTH,
          height: NODE_HEIGHT,
          x: 60 + current.length * 80,
          y: 80 + current.length * 60
        }
      ])
    } catch (problem) {
      report(problem)
    }
  }

  async function createLink(from: number, to: number): Promise<void> {
    if (pipeline === null) return
    if (connectors.some((item) => item.from === from && item.to === to)) return

    try {
      await engine.connect(pipeline, from, to, [])

      const connector: Connector = { id: newId('c'), from, to, mappings: [], subscriptions: [] }
      setConnectors((current) => [...current, connector])
      setConnectorId(connector.id)
    } catch (problem) {
      report(problem)
    }
  }

  const connector = connectors.find((item) => item.id === connectorId) ?? null
  const nodeOf = (id: number | undefined): Node | undefined => nodes.find((item) => item.id === id)
  const libraryOf = (node: Node | undefined): Library | null =>
    libraries.find((item) => item.key === node?.library) ?? null

  const source = nodeOf(connector?.from)
  const target = nodeOf(connector?.to)

  /// The next connector is derived here, not inside the state updater: React
  /// runs an updater whenever it likes, so reading its result back was a race
  /// that silently skipped the commit to the engine.
  function updateConnector(change: (current: Connector) => Connector): Connector | null {
    if (!connector) return null

    const updated = change(connector)

    setConnectors((current) => current.map((item) => (item.id === updated.id ? updated : item)))

    return updated
  }

  /// Mapping edits are pushed straight to the engine: connecting the same
  /// pair again replaces the mapping it already has.
  function commitMappings(next: Connector): void {
    if (pipeline === null) return
    engine.connect(pipeline, next.from, next.to, next.mappings).catch(report)
  }

  function commitSubscription(next: Subscription): void {
    if (!source || !target) return

    engine
      .subscribe(source.plugin, next.event, target.plugin, next.command, next.mappings)
      .catch(report)
  }

  /// The engine keys a subscription by event and command, so one that moves
  /// to another pair has to be dropped from the old one by hand.
  function dropSubscription(previous: Subscription): void {
    if (!source || !target) return

    engine
      .unsubscribe(source.plugin, previous.event, target.plugin, previous.command)
      .catch(report)
  }

  const editMappings = (change: (current: Connector) => Connector): void => {
    const next = updateConnector(change)
    if (next) commitMappings(next)
  }

  const editSubscription = (
    subscriptionId: string,
    change: (current: Subscription) => Subscription
  ): void => {
    const previous = connector?.subscriptions.find((item) => item.id === subscriptionId)

    const next = updateConnector((current) => ({
      ...current,
      subscriptions: current.subscriptions.map((item) =>
        item.id === subscriptionId ? change(item) : item
      )
    }))

    const subscription = next?.subscriptions.find((item) => item.id === subscriptionId)
    if (!subscription) return

    const moved =
      previous &&
      (previous.event !== subscription.event || previous.command !== subscription.command)

    if (moved) dropSubscription(previous)

    commitSubscription(subscription)
  }

  async function toggleRun(): Promise<void> {
    if (pipeline === null) return

    try {
      if (running) {
        await engine.stop(pipeline)
      } else {
        await engine.start(pipeline)
      }

      setRunning(!running)

      // Starting switches every node on. Stopping only switches the sources
      // off, so whatever is still in flight drains, which is what the engine
      // does too.
      setNodes((current) =>
        current.map((node) => ({
          ...node,
          running: running ? node.running && !libraryOf(node)?.source : true
        }))
      )

      host.setRunning(!running)
    } catch (problem) {
      report(problem)
    }
  }

  /// Removing a node releases its plugin, so every link and subscription that
  /// referenced it goes with it.
  async function removeNode(id: number): Promise<void> {
    const node = nodes.find((item) => item.id === id)
    if (!node) return

    try {
      await engine.removePlugin(node.plugin)
    } catch (problem) {
      report(problem)
      return
    }

    const orphaned = connectors.filter((item) => item.from === id || item.to === id)

    setNodes((current) => current.filter((item) => item.id !== id))
    setConnectors((current) => current.filter((item) => item.from !== id && item.to !== id))

    if (orphaned.some((item) => item.id === connectorId)) setConnectorId(null)

    setPopups((current) => current.filter((item) => item.plugin !== node.plugin))
    host.forget(node.plugin)
  }

  /// A node only has a worker to switch once the pipeline has started.
  async function toggleNode(id: number, next: boolean): Promise<void> {
    if (pipeline === null) return

    setNodes((current) =>
      current.map((node) => (node.id === id ? { ...node, running: next } : node))
    )

    try {
      await engine.setNodeRunning(pipeline, id, next)
    } catch (problem) {
      report(problem)

      // The engine kept its old state, so the switch has to go back.
      setNodes((current) =>
        current.map((node) => (node.id === id ? { ...node, running: !next } : node))
      )
    }
  }

  return (
    <div className="app">
      <Sidebar
        libraries={libraries}
        connected={connected}
        pipelineName="Untitled pipeline"
        onAdd={addNode}
      />

      <div className="workspace">
        <Toolbar
          name="Untitled pipeline"
          nodes={nodes.length}
          connectors={connectors.length}
          running={running}
          connected={connected}
          error={error}
          onDismissError={() => setError(null)}
          onToggleRun={toggleRun}
        />

        <div className="stage">
          <Graph
            nodes={nodes}
            connectors={connectors}
            libraries={libraries}
            selectedConnector={connectorId}
            started={running}
            onSelectConnector={setConnectorId}
            onToggleNode={toggleNode}
            onRemoveNode={removeNode}
            onLink={createLink}
            onMoveNode={(id, x, y) =>
              setNodes((current) =>
                current.map((node) => (node.id === id ? { ...node, x, y } : node))
              )
            }
            host={host}
          />

          <Inspector
            connector={connector}
            source={libraryOf(source)}
            target={libraryOf(target)}
            onChangeMapping={(id, patch) =>
              editMappings((current) => ({
                ...current,
                mappings: current.mappings.map((mapping) =>
                  mapping.id === id ? { ...mapping, ...patch } : mapping
                )
              }))
            }
            onRemoveMapping={(id) =>
              editMappings((current) => ({
                ...current,
                mappings: current.mappings.filter((mapping) => mapping.id !== id)
              }))
            }
            onAddMapping={() =>
              editMappings((current) => ({
                ...current,
                mappings: [
                  ...current.mappings,
                  defaultMapping(
                    libraryOf(source)?.output?.fields ?? [],
                    libraryOf(target)?.input?.fields ?? [],
                    new Set(current.mappings.map((mapping) => mapping.to))
                  )
                ]
              }))
            }
            onAddSubscription={() => {
              const events = libraryOf(source)?.events ?? []
              const commands = libraryOf(target)?.commands ?? []

              const pair = freePair(connector?.subscriptions ?? [], events.length, commands.length)

              if (!pair) {
                report(new Error('Every event and command pair is already subscribed'))
                return
              }

              const subscription: Subscription = {
                id: newId('s'),
                event: pair.event,
                command: pair.command,
                mappings: autoMappings(
                  events[pair.event]?.schema?.fields ?? [],
                  commands[pair.command]?.schema?.fields ?? []
                )
              }

              updateConnector((current) => ({
                ...current,
                subscriptions: [...current.subscriptions, subscription]
              }))

              commitSubscription(subscription)
            }}
            onRemoveSubscription={(subscriptionId) => {
              const previous = connector?.subscriptions.find((item) => item.id === subscriptionId)

              if (previous) dropSubscription(previous)

              updateConnector((current) => ({
                ...current,
                subscriptions: current.subscriptions.filter((item) => item.id !== subscriptionId)
              }))
            }}
            onChangeSubscription={(subscriptionId, patch) => {
              const events = libraryOf(source)?.events ?? []
              const commands = libraryOf(target)?.commands ?? []

              const current = connector?.subscriptions.find((item) => item.id === subscriptionId)
              if (!current) return

              const next = { ...current, ...patch }

              // Moving onto a pair another row holds would collapse the two
              // into one registration, and removing either would kill both.
              if (pairTaken(connector?.subscriptions ?? [], next.event, next.command, next.id)) {
                report(
                  new Error(
                    `${events[next.event]?.name} → ${commands[next.command]?.name} is already subscribed`
                  )
                )

                return
              }

              editSubscription(subscriptionId, () => ({
                ...next,

                // The old rows index into the schema that just changed, so
                // they would now point at the wrong fields.
                mappings: autoMappings(
                  events[next.event]?.schema?.fields ?? [],
                  commands[next.command]?.schema?.fields ?? []
                )
              }))
            }}
            onAddSubscriptionMapping={(subscriptionId) =>
              editSubscription(subscriptionId, (current) => ({
                ...current,
                mappings: [
                  ...current.mappings,
                  defaultMapping(
                    libraryOf(source)?.events[current.event]?.schema?.fields ?? [],
                    libraryOf(target)?.commands[current.command]?.schema?.fields ?? [],
                    new Set(current.mappings.map((mapping) => mapping.to))
                  )
                ]
              }))
            }
            onRemoveSubscriptionMapping={(subscriptionId, mappingId) =>
              editSubscription(subscriptionId, (current) => ({
                ...current,
                mappings: current.mappings.filter((mapping) => mapping.id !== mappingId)
              }))
            }
            onChangeSubscriptionMapping={(subscriptionId, mappingId, patch) =>
              editSubscription(subscriptionId, (current) => ({
                ...current,
                mappings: current.mappings.map((mapping) =>
                  mapping.id === mappingId ? { ...mapping, ...patch } : mapping
                )
              }))
            }
          />
        </div>
      </div>

      {popups.map((popup) => (
        <PluginPopup
          key={popup.plugin}
          host={host}
          popup={popup}
          onClose={() => closePopup(popup.plugin)}
        />
      ))}
    </div>
  )
}

export default App
