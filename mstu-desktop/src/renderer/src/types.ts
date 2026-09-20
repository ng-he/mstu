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

/// What one enum admits.
export type Variant = {
  name: string
  value: string | number | boolean | null
}

/// A type as the engine describes it, down to what it contains.
export type TypeInfo = {
  type: FieldType

  /// Width of a number, and whether it is signed.
  bits?: number
  signed?: boolean

  /// An enum's variants, a record's fields, a list's element type.
  variants?: Variant[]
  fields?: Field[]
  element?: TypeInfo
}

export type Field = TypeInfo & {
  name: string
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
  live: Schema
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

  /// Size of the node box, from the plugin UI's `ready` message once it loads.
  width: number
  height: number

  x: number
  y: number
}

/// A page of a plugin's UI floating over the workspace, at most one per plugin.
export type Popup = {
  plugin: string
  page: string
  title: string
  /// Size of the page itself, not counting the title bar.
  width: number
  height: number
}

/// One field of the source message copied into one field of the target.
///
/// Each side is a path of field indexes: [4] is a top-level field, [4, 1] is
/// field 1 of the record in field 4.
export type Mapping = {
  id: string
  from: number[]
  to: number[]
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
