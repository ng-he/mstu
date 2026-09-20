import { engine } from './engine'
import type { Descriptor, Library, Schema } from './types'

type Values = Record<string, unknown>
type Listener<T> = (value: T) => void

/// Where a page is shown: in its node on the canvas, or in the plugin's popup.
export type PageRole = 'node' | 'popup'

export type PopupRequest = { page: string; title?: string; width?: number; height?: number }

/// What the host owes the pages, read fresh on every call.
export type HostHooks = {
  library: (plugin: string) => Library | undefined
  resize: (plugin: string, role: PageRole, width?: number, height?: number) => void
  openPopup: (plugin: string, request: PopupRequest) => void
  closePopup: (plugin: string) => void
  report: (problem: unknown) => void
}

/// The `plugin` object a page's script receives.
export type PluginApi = ReturnType<PluginHost['createApi']>

type Page = {
  plugin: string
  live: Listener<Values>[]
  state: Listener<{ running: boolean }>[]
  settings: Listener<Values>[]
  popup: Listener<{ open: boolean }>[]
  events: Map<string, Listener<Values>[]>
}

const fieldsOf = (schema: Schema | undefined): { name: string }[] => schema?.fields ?? []

const indexOf = (list: { name: string }[], name: string): number =>
  list.findIndex((item) => item.name === name)

/// Positional values, or the index-keyed settings object, keyed by field name.
function named(schema: Schema | undefined, values: ArrayLike<unknown> | Record<number, unknown>): Values {
  const result: Values = {}

  fieldsOf(schema).forEach((field, index) => {
    result[field.name] = (values as Record<number, unknown>)?.[index]
  })

  return result
}

const warn = (message: string): void => console.warn(`[mstu] ${message}`)

/// Runs plugin pages in the host document and feeds them the engine's traffic.
export class PluginHost {
  hooks: HostHooks

  private pages = new Set<Page>()
  private settings = new Map<string, Record<number, unknown>>()
  private popups = new Set<string>()
  private running = false

  /// Last live snapshot per plugin, and when it last differed from the one before.
  private snapshots = new Map<string, string>()
  private moved = new Map<string, number>()

  constructor(hooks: HostHooks) {
    this.hooks = hooks
  }

  /// Registers a page; the returned dispose drops its listeners.
  ///
  /// `box` measures what the mounted page needs, including the node's own chrome.
  attach(
    plugin: string,
    role: PageRole,
    root: ShadowRoot,
    box: () => number = () => 0
  ): { api: PluginApi; dispose: () => void } {
    const page: Page = { plugin, live: [], state: [], settings: [], popup: [], events: new Map() }

    this.pages.add(page)

    return { api: this.createApi(page, role, root, box), dispose: () => this.pages.delete(page) }
  }

  live(plugin: string, values: unknown[]): void {
    const library = this.hooks.library(plugin)
    const snapshot = JSON.stringify(values)

    // A plugin whose readings changed since the last push is doing something.
    if (this.snapshots.get(plugin) !== snapshot) {
      this.snapshots.set(plugin, snapshot)
      this.moved.set(plugin, Date.now())
    }

    this.each(plugin, (page) => page.live.forEach((listener) => listener(named(library?.live, values))))
  }

  /// Plugins whose readings moved within `window` milliseconds.
  active(window = 800): string[] {
    const since = Date.now() - window

    return [...this.moved.entries()].filter(([, at]) => at >= since).map(([plugin]) => plugin)
  }

  event(plugin: string, index: number, values: unknown[]): void {
    const descriptor: Descriptor | undefined = this.hooks.library(plugin)?.events[index]
    if (!descriptor) return

    this.each(plugin, (page) =>
      (page.events.get(descriptor.name) ?? []).forEach((listener) =>
        listener(named(descriptor.schema, values))
      )
    )
  }

  setRunning(running: boolean): void {
    this.running = running
    this.pages.forEach((page) => page.state.forEach((listener) => listener({ running })))
  }

  setPopup(plugin: string, open: boolean): void {
    if (open) this.popups.add(plugin)
    else this.popups.delete(plugin)

    this.each(plugin, (page) => page.popup.forEach((listener) => listener({ open })))
  }

  /// A removed plugin's settings go with it.
  forget(plugin: string): void {
    this.settings.delete(plugin)
    this.popups.delete(plugin)
    this.snapshots.delete(plugin)
    this.moved.delete(plugin)
  }

  clear(): void {
    this.settings.clear()
    this.popups.clear()
    this.snapshots.clear()
    this.moved.clear()
    this.running = false
  }

  private each(plugin: string, visit: (page: Page) => void): void {
    this.pages.forEach((page) => page.plugin === plugin && visit(page))
  }

  private namedSettings(plugin: string): Values {
    return named(this.hooks.library(plugin)?.settings, this.settings.get(plugin) ?? {})
  }

  private store(plugin: string, field: number, value: unknown): void {
    this.settings.set(plugin, { ...this.settings.get(plugin), [field]: value })
  }

  /// Tells every page of the plugin but `except` that its settings changed.
  private broadcastSettings(plugin: string, except?: Page): void {
    const values = this.namedSettings(plugin)

    this.each(plugin, (page) => page !== except && page.settings.forEach((listener) => listener(values)))
  }

  private createApi(page: Page, role: PageRole, root: ShadowRoot, box: () => number) {
    // The getters below are not arrows, so they cannot use `this`.
    const host = this
    const { plugin } = page
    const library = (): Library | undefined => this.hooks.library(plugin)
    const settingIndex = (name: string): number => indexOf(fieldsOf(library()?.settings), name)

    const api = {
      /// This page's shadow root, for anything `el` does not cover.
      root,

      /// Current settings, keyed by name.
      get settings(): Values {
        return host.namedSettings(plugin)
      },

      /// The plugin's schemas as the engine describes them.
      get schemas() {
        const described = library()
        return {
          settings: described?.settings ?? null,
          live: described?.live ?? null,
          events: described?.events ?? [],
          commands: described?.commands ?? []
        }
      },

      /// Writes one setting through to the plugin.
      set: (name: string, value: unknown): void => {
        const field = settingIndex(name)
        if (field < 0) return warn(`no setting named '${name}'`)

        // Only what the plugin accepted becomes a setting.
        engine
          .setParameter(plugin, field, value)
          .then(() => {
            this.store(plugin, field, value)
            this.broadcastSettings(plugin, page)
          })
          .catch(this.hooks.report)
      },

      /// Opens the file or folder chooser and stores what comes back.
      pick: (name: string, { kind = 'file' }: { kind?: 'file' | 'directory' } = {}): void => {
        const field = settingIndex(name)
        if (field < 0) return warn(`no setting named '${name}'`)

        window.mstu
          .pick(kind)
          .then(async (picked) => {
            if (!picked) return

            await engine.setParameter(plugin, field, picked)
            this.store(plugin, field, picked)
            this.broadcastSettings(plugin)
          })
          .catch(this.hooks.report)
      },

      /// Invokes one of the plugin's commands by name.
      invoke: (name: string, payload: unknown[] = []): void => {
        const command = indexOf(library()?.commands ?? [], name)
        if (command < 0) return warn(`no command named '${name}'`)

        engine.invoke(plugin, command, payload).catch(this.hooks.report)
      },

      /// Live values, pushed by the engine on a timer.
      onLive: (listener: Listener<Values>) => (page.live.push(listener), api),

      /// One of the plugin's events, by name.
      onEvent: (name: string, listener: Listener<Values>) => {
        page.events.set(name, [...(page.events.get(name) ?? []), listener])
        return api
      },

      /// Whether the pipeline is running; called right away with the current state.
      onState: (listener: Listener<{ running: boolean }>) => {
        page.state.push(listener)
        listener({ running: this.running })
        return api
      },

      /// Settings changed elsewhere; called right away with the current ones.
      onSettings: (listener: Listener<Values>) => {
        page.settings.push(listener)
        listener(this.namedSettings(plugin))
        return api
      },

      /// Whether this plugin's popup is open; called right away with the current state.
      onPopup: (listener: Listener<{ open: boolean }>) => {
        page.popup.push(listener)
        listener({ open: this.popups.has(plugin) })
        return api
      },

      /// Asks for a different box: the node's on the canvas, or the popup's.
      resize: (width?: number, height?: number): void =>
        this.hooks.resize(plugin, role, width, height),

      /// Asks for the height this page's own content needs, at the given width.
      fit: (width?: number): void => this.hooks.resize(plugin, role, width, box()),

      /// Opens another page of this UI in a window floating over the canvas.
      popup: (pagePath: string, options: Omit<PopupRequest, 'page'> = {}): void =>
        this.hooks.openPopup(plugin, { page: pagePath, ...options }),

      /// Closes the page opened with popup().
      closePopup: (): void => this.hooks.closePopup(plugin),

      /// A local file served to this UI, since the host page cannot load file:// itself.
      mediaUrl: (path: string): string =>
        `${pluginOrigin(plugin)}/_media/?path=${encodeURIComponent(path ?? '')}`,

      el: (id: string): HTMLElement | null => root.getElementById(id)
    }

    return api
  }
}

export const pluginOrigin = (plugin: string): string => `mstu-plugin://${plugin.toLowerCase()}`
