import { useCallback, useEffect, useRef, useState } from 'react'

import Graph from './components/Graph'
import Inspector from './components/Inspector'
import Sidebar from './components/Sidebar'
import Toolbar from './components/Toolbar'
import { engine } from './engine'
import type { Connector, Field, Library, Mapping, Node, Subscription } from './types'

let nextId = 1
const newId = (prefix: string): string => `${prefix}${nextId++}`

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

  /// Plugin UI frames, and the settings we have pushed to each plugin.
  const frames = useRef(new Map<string, HTMLIFrameElement>())
  const settings = useRef(new Map<string, Record<number, unknown>>())
  const hadGraph = useRef(false)

  const report = (problem: unknown): void =>
    setError(problem instanceof Error ? problem.message : String(problem))

  const post = useCallback((plugin: string, message: unknown): void => {
    frames.current.get(plugin)?.contentWindow?.postMessage(message, '*')
  }, [])

  // ---------- engine connection ----------

  useEffect(() => {
    // The engine may already be connected before this mounted.
    window.mstu.connected().then(setConnected).catch(report)

    const offStatus = window.mstu.onStatus(setConnected)

    const offEvent = window.mstu.onEvent((notice) => {
      post(notice.plugin, { type: 'event', event: notice.event, payload: notice.values })
    })

    return () => {
      offStatus()
      offEvent()
    }
  }, [post])

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
    frames.current.clear()
    settings.current.clear()
  }, [connected])

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

  // ---------- plugin UI bridge ----------

  useEffect(() => {
    const onMessage = async (message: MessageEvent): Promise<void> => {
      const entry = [...frames.current.entries()].find(
        ([, frame]) => frame.contentWindow === message.source
      )

      if (!entry) return

      const [plugin] = entry
      const data = message.data ?? {}

      try {
        if (data.type === 'ready') {
          post(plugin, { type: 'init', settings: settings.current.get(plugin) ?? {} })
          return
        }

        if (data.type === 'set_parameter') {
          await engine.setParameter(plugin, data.field, data.value)
          const current = settings.current.get(plugin) ?? {}
          settings.current.set(plugin, { ...current, [data.field]: data.value })
          return
        }

        if (data.type === 'invoke') {
          await engine.invoke(plugin, data.command, data.payload ?? [])
          return
        }

        if (data.type === 'pick') {
          const picked = await window.mstu.pick(data.kind === 'directory' ? 'directory' : 'file')
          if (!picked) return

          await engine.setParameter(plugin, data.field, picked)
          const current = settings.current.get(plugin) ?? {}
          settings.current.set(plugin, { ...current, [data.field]: picked })
          post(plugin, { type: 'init', settings: settings.current.get(plugin) })
        }
      } catch (problem) {
        report(problem)
      }
    }

    window.addEventListener('message', onMessage)
    return () => window.removeEventListener('message', onMessage)
  }, [post])

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

  const editMappings = (change: (current: Connector) => Connector): void => {
    const next = updateConnector(change)
    if (next) commitMappings(next)
  }

  const editSubscription = (
    subscriptionId: string,
    change: (current: Subscription) => Subscription
  ): void => {
    const next = updateConnector((current) => ({
      ...current,
      subscriptions: current.subscriptions.map((item) =>
        item.id === subscriptionId ? change(item) : item
      )
    }))

    const subscription = next?.subscriptions.find((item) => item.id === subscriptionId)
    if (subscription) commitSubscription(subscription)
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

      for (const node of nodes) {
        post(node.plugin, { type: 'state', running: !running })
      }
    } catch (problem) {
      report(problem)
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
            onSelectConnector={setConnectorId}
            onLink={createLink}
            onMoveNode={(id, x, y) =>
              setNodes((current) =>
                current.map((node) => (node.id === id ? { ...node, x, y } : node))
              )
            }
            registerFrame={(plugin, frame) => {
              if (frame) frames.current.set(plugin, frame)
              else frames.current.delete(plugin)
            }}
            onFrameReady={(plugin) =>
              post(plugin, { type: 'init', settings: settings.current.get(plugin) ?? {} })
            }
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
              const subscription: Subscription = {
                id: newId('s'),
                event: 0,
                command: 0,
                mappings: autoMappings(
                  libraryOf(source)?.events[0]?.schema?.fields ?? [],
                  libraryOf(target)?.commands[0]?.schema?.fields ?? []
                )
              }

              updateConnector((current) => ({
                ...current,
                subscriptions: [...current.subscriptions, subscription]
              }))

              commitSubscription(subscription)
            }}
            onRemoveSubscription={(subscriptionId) =>
              updateConnector((current) => ({
                ...current,
                subscriptions: current.subscriptions.filter((item) => item.id !== subscriptionId)
              }))
            }
            onChangeSubscription={(subscriptionId, patch) =>
              editSubscription(subscriptionId, (current) => {
                const next = { ...current, ...patch }

                // The old rows index into the schema that just changed, so
                // they would now point at the wrong fields.
                return {
                  ...next,
                  mappings: autoMappings(
                    libraryOf(source)?.events[next.event]?.schema?.fields ?? [],
                    libraryOf(target)?.commands[next.command]?.schema?.fields ?? []
                  )
                }
              })
            }
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
    </div>
  )
}

export default App
