/// Bridge between a plugin UI and the mstu host.
///
/// The wire protocol addresses settings, commands and events by index. This
/// maps them onto the names in the plugin's own schemas, so a UI never has to
/// hardcode a position that a schema change would silently break.

const send = (message) => parent.postMessage(message, '*')

const warn = (message) => console.warn(`[mstu] ${message}`)

const fieldsOf = (schema) => schema?.fields ?? []

const indexOf = (list, name) => (list ?? []).findIndex((item) => item.name === name)

/// Positional values to an object keyed by the schema's field names.
///
/// `values` may be an array or the host's index-keyed settings object.
function named(fields, values) {
  const result = {}

  fields.forEach((field, index) => {
    result[field.name] = values?.[index]
  })

  return result
}

/// Connects to the host and resolves once the plugin's schemas have arrived.
///
/// `size` is the box this UI would like on the canvas; the host clamps it.
export function connect(size = {}) {
  const onLive = []
  const onState = []
  const onSettings = []
  const onEvent = new Map()

  let schemas = { settings: null, live: null, events: [], commands: [] }
  let settings = {}
  let started = null

  const api = {
    /// Current settings, keyed by name.
    get settings() {
      return settings
    },

    /// The plugin's schemas as the engine describes them.
    get schemas() {
      return schemas
    },

    /// Writes one setting through to the plugin.
    set(name, value) {
      const field = indexOf(fieldsOf(schemas.settings), name)

      if (field < 0) return warn(`no setting named '${name}'`)

      send({ type: 'set_parameter', field, value })
      settings = { ...settings, [name]: value }
    },

    /// Opens the host's file or folder chooser and stores what comes back.
    pick(name, { kind = 'file' } = {}) {
      const field = indexOf(fieldsOf(schemas.settings), name)

      if (field < 0) return warn(`no setting named '${name}'`)

      send({ type: 'pick', kind, field })
    },

    /// Invokes one of the plugin's commands by name.
    invoke(name, payload = []) {
      const command = indexOf(schemas.commands, name)

      if (command < 0) return warn(`no command named '${name}'`)

      send({ type: 'invoke', command, payload })
    },

    /// The plugin's live values, pushed by the engine on a timer.
    onLive(listener) {
      onLive.push(listener)
      return api
    },

    /// One of the plugin's events, by name.
    onEvent(name, listener) {
      if (!onEvent.has(name)) onEvent.set(name, [])
      onEvent.get(name).push(listener)
      return api
    },

    /// Whether the pipeline is running.
    onState(listener) {
      onState.push(listener)
      return api
    },

    /// Settings changed elsewhere, such as by the file chooser.
    onSettings(listener) {
      onSettings.push(listener)
      return api
    },

    el: (id) => document.getElementById(id)
  }

  const ready = new Promise((resolve) => {
    started = resolve
  })

  window.addEventListener('message', ({ data }) => {
    if (!data || typeof data !== 'object') return

    if (data.type === 'init') {
      if (data.schemas) schemas = data.schemas

      settings = named(fieldsOf(schemas.settings), data.settings)
      onSettings.forEach((listener) => listener(settings))
      started(api)

      return
    }

    if (data.type === 'live') {
      const values = named(fieldsOf(schemas.live), data.payload)
      onLive.forEach((listener) => listener(values))

      return
    }

    if (data.type === 'event') {
      const descriptor = schemas.events?.[data.event]

      if (!descriptor) return

      const values = named(fieldsOf(descriptor.schema), data.payload)
      ;(onEvent.get(descriptor.name) ?? []).forEach((listener) => listener(values))

      return
    }

    if (data.type === 'state') {
      onState.forEach((listener) => listener(data))
    }
  })

  send({ type: 'ready', ...size })

  return ready
}

/// Bytes as a short human string, for the size readouts plugins tend to show.
export function humanSize(bytes) {
  const units = ['B', 'KB', 'MB', 'GB', 'TB']

  let size = bytes ?? 0
  let unit = 0

  while (size >= 1024 && unit < units.length - 1) {
    size /= 1024
    unit++
  }

  return `${unit === 0 ? size : size.toFixed(1)} ${units[unit]}`
}
