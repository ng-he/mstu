import type { Library, Mapping } from './types'

const call = <T,>(method: string, params: unknown = {}): Promise<T> =>
  window.mstu.call<T>(method, params)

/// Engine mappings are `[from, to]` pairs of paths into the two messages.
const pairs = (mappings: Mapping[]): number[][][] =>
  mappings.map((mapping) => [mapping.from, mapping.to])

export const engine = {
  describe: () => call<{ libraries: Library[] }>('describe'),

  createPipeline: (name: string) => call<{ pipeline: number }>('create_pipeline', { name }),

  createPlugin: (library: string) =>
    call<{ plugin: string; ui: string | null }>('create_plugin', { library }),

  addNode: (pipeline: number, plugin: string) =>
    call<{ node: number }>('add_node', { pipeline, plugin }),

  /// Takes the plugin out of every pipeline and releases it.
  removePlugin: (plugin: string) => call('remove_plugin', { plugin }),

  connect: (pipeline: number, from: number, to: number, mappings: Mapping[]) =>
    call('connect', { pipeline, from, to, mappings: pairs(mappings) }),

  subscribe: (from: string, event: number, to: string, command: number, mappings: Mapping[]) =>
    call('subscribe', { from, event, to, command, mappings: pairs(mappings) }),

  unsubscribe: (from: string, event: number, to: string, command: number) =>
    call('unsubscribe', { from, event, to, command }),

  setParameter: (plugin: string, field: number, value: unknown) =>
    call('set_parameter', { plugin, field, value }),

  invoke: (plugin: string, command: number, payload: unknown[] = []) =>
    call('invoke', { plugin, command, payload }),

  setNodeRunning: (pipeline: number, node: number, running: boolean) =>
    call('set_node_running', { pipeline, node, running }),

  start: (pipeline: number) => call('start', { pipeline }),

  stop: (pipeline: number) => call('stop', { pipeline })
}
