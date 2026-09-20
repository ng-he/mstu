import type { Field, Mapping, TypeInfo } from './types'

/// A field a mapping can address, and where it sits in the message.
export type Leaf = {
  /// Field indexes from the root of the message.
  path: number[]

  /// Dotted name, as it reads in the payload: "codec.name".
  label: string

  field: Field
}

/// Paths are compared and keyed by their text, since arrays never match by identity.
export const pathKey = (path: number[]): string => path.join('.')

/// A number says how wide it is, which a payload view should show: uint64, float32.
export function typeLabel(type: TypeInfo): string {
  if ((type.type === 'uint' || type.type === 'int' || type.type === 'float') && type.bits) {
    return `${type.type}${type.bits}`
  }

  if (type.type === 'list' && type.element) return `list<${typeLabel(type.element)}>`

  return type.type
}

/// Whether a record's fields are worth walking into.
export const nested = (field: Field): boolean =>
  field.type === 'record' && (field.fields?.length ?? 0) > 0

/// Every field a mapping can carry a value into or out of. A record is a
/// container, so its leaves are addressable and the record itself is not.
export function leaves(fields: Field[], path: number[] = [], prefix = ''): Leaf[] {
  return fields.flatMap((field, index) => {
    const here = [...path, index]
    const label = prefix ? `${prefix}.${field.name}` : field.name

    return nested(field) ? leaves(field.fields ?? [], here, label) : [{ path: here, label, field }]
  })
}

/// The field a path points at, walking into records on the way.
export function fieldAt(fields: Field[], path: number[]): Field | undefined {
  const [index, ...rest] = path
  const field = fields[index]

  if (!field || rest.length === 0) return field

  return fieldAt(field.fields ?? [], rest)
}

/// Source path filling each target path, keyed by the target's path text.
export const byTarget = (mappings: Mapping[]): Map<string, number[]> =>
  new Map(mappings.map((mapping) => [pathKey(mapping.to), mapping.from]))
