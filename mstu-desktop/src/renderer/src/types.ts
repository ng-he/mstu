export type FieldType =
  | 'bool'
  | 'int'
  | 'uint'
  | 'float'
  | 'string'
  | 'bytes'
  | 'enum'
  | 'record'
  | 'list'

export type Field = {
  name: string
  type: FieldType
  description?: string
}

export type Schema = {
  fields: Field[]
} | null

export type Descriptor = {
  name: string
  schema: Schema
}

/// A plugin library as the engine describes it.
export type Library = {
  key: string
  name: string
  version: string
  source: boolean
  input: Schema
  output: Schema
  settings: Schema
  events: Descriptor[]
  commands: Descriptor[]
}

export type Node = {
  /// Node id inside the pipeline.
  id: number
  /// Plugin instance id, the address for settings, commands and events.
  plugin: string
  library: string
  name: string
  ui: boolean

  /// Whether the node is processing right now. Nothing runs before the
  /// pipeline starts, since a node has no worker until then.
  running: boolean

  x: number
  y: number
}

/// One field of the source message copied into one field of the target.
export type Mapping = {
  id: string
  from: number
  to: number
}

export type Subscription = {
  id: string
  event: number
  command: number
  mappings: Mapping[]
}

export type Connector = {
  id: string
  from: number
  to: number
  mappings: Mapping[]
  subscriptions: Subscription[]
}
