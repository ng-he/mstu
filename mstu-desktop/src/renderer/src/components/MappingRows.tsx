import type { Field, Mapping } from '../types'

type Props = {
  source: Field[]
  target: Field[]
  mappings: Mapping[]
  onChange: (id: string, patch: Partial<Mapping>) => void
  onRemove: (id: string) => void
  onAdd: () => void
  addLabel: string
}

function FieldSelect({
  fields,
  value,
  onChange
}: {
  fields: Field[]
  value: number
  onChange: (index: number) => void
}): JSX.Element {
  const field = fields[value]

  return (
    <div className="field-select">
      <select value={value} onChange={(event) => onChange(Number(event.target.value))}>
        {fields.map((item, index) => (
          <option key={item.name} value={index}>
            [{index}] {item.name}
          </option>
        ))}
      </select>
      {field && <span className={`type-chip ${field.type}`}>{field.type}</span>}
    </div>
  )
}

/// Shared by the mapping tab and every event subscription: both are
/// "copy these source fields into those target fields".
function MappingRows({
  source,
  target,
  mappings,
  onChange,
  onRemove,
  onAdd,
  addLabel
}: Props): JSX.Element {
  const mapped = new Set(mappings.map((mapping) => mapping.to))
  const unmapped = target.filter((_, index) => !mapped.has(index))

  const mismatched = mappings.filter(
    (mapping) => source[mapping.from]?.type !== target[mapping.to]?.type
  )

  return (
    <div>
      {mappings.map((mapping) => (
        <div
          className={`map-row ${
            source[mapping.from]?.type !== target[mapping.to]?.type ? 'mismatch' : ''
          }`}
          key={mapping.id}
        >
          <FieldSelect
            fields={source}
            value={mapping.from}
            onChange={(from) => onChange(mapping.id, { from })}
          />
          <span className="arrow">→</span>
          <FieldSelect
            fields={target}
            value={mapping.to}
            onChange={(to) => onChange(mapping.id, { to })}
          />
          <button className="remove" title="Remove" onClick={() => onRemove(mapping.id)}>
            ✕
          </button>
        </div>
      ))}

      {mappings.length === 0 && (
        <div className="empty">Nothing mapped yet — the target message stays empty.</div>
      )}

      <button className="add-row" onClick={onAdd} disabled={source.length === 0}>
        + {addLabel}
      </button>

      {mismatched.length > 0 && (
        <div className="warning error">
          <span>⚠</span>
          <span>
            Type mismatch on{' '}
            {mismatched.map((mapping) => target[mapping.to]?.name ?? '?').join(', ')}. The engine
            drops a message that does not match the target schema.
          </span>
        </div>
      )}

      {unmapped.length > 0 && (
        <div className="warning">
          <span>⚠</span>
          <span>
            {unmapped.map((field) => field.name).join(', ')}{' '}
            {unmapped.length === 1 ? 'is' : 'are'} never written. The engine drops a message it
            cannot fill.
          </span>
        </div>
      )}
    </div>
  )
}

export default MappingRows
