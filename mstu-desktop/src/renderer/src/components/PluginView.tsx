import { useEffect, useRef, type CSSProperties } from 'react'

import { pluginOrigin, type PageRole, type PluginHost } from '../pluginHost'

type Props = {
  host: PluginHost
  plugin: string
  page: string
  role: PageRole
  className?: string
  style?: CSSProperties
}

/// Attributes that point at a file, resolved against the plugin page, not the host.
const LINKS = ['src', 'href', 'poster']

/// Parses a plugin page into its styles, its markup and the scripts to run.
async function loadPage(url: string): Promise<{ styles: Node[]; body: Node[]; scripts: string[] }> {
  const response = await fetch(url)
  if (!response.ok) throw new Error(`${url}: ${response.status}`)

  const doc = new DOMParser().parseFromString(await response.text(), 'text/html')

  for (const element of doc.querySelectorAll('*')) {
    for (const name of LINKS) {
      const value = element.getAttribute(name)
      if (value !== null && !value.startsWith('#')) element.setAttribute(name, new URL(value, url).href)
    }
  }

  const scripts = [...doc.querySelectorAll('script')].flatMap((script) => {
    script.remove()

    if (script.type === 'module' && script.src) return [script.src]

    console.warn(`[mstu] ${url}: only <script type="module" src> is run`)
    return []
  })

  const styles = [...doc.querySelectorAll('link[rel="stylesheet"], style')]
  styles.forEach((style) => style.remove())

  return {
    styles: styles.map((node) => document.importNode(node, true)),
    body: [...doc.body.childNodes].map((node) => document.importNode(node, true)),
    scripts
  }
}

/// Waits for the stylesheets, so the page never shows unstyled.
const loaded = (nodes: Node[]): Promise<unknown> =>
  Promise.all(
    nodes
      .filter((node): node is HTMLLinkElement => node instanceof HTMLLinkElement)
      .map(
        (link) =>
          new Promise((done) => {
            link.addEventListener('load', done)
            link.addEventListener('error', done)
          })
      )
  )

/// Height the page's content wants, measured with the box let go of for a moment.
///
/// offsetHeight is layout, so the canvas zoom does not skew it.
function contentHeight(element: HTMLElement): number {
  const { flex, height } = element.style

  element.style.flex = 'none'
  element.style.height = 'auto'

  const measured = element.offsetHeight

  element.style.flex = flex
  element.style.height = height

  return measured
}

/// A plugin page in a shadow root: the host's styles stay out, and it zooms like the rest of the canvas.
function PluginView({ host, plugin, page, role, className, style }: Props): JSX.Element {
  const box = useRef<HTMLDivElement>(null)

  useEffect(() => {
    const element = box.current
    if (!element) return

    const root = element.shadowRoot ?? element.attachShadow({ mode: 'open' })

    // A node is asked for as a whole, chrome included; a popup's size is the page's.
    const wanted = (): number => {
      const node = role === 'node' ? element.closest('.node') : null
      const chrome = node instanceof HTMLElement ? node.offsetHeight - element.offsetHeight : 0

      return contentHeight(element) + Math.max(0, chrome)
    }

    const { api, dispose } = host.attach(plugin, role, root, wanted)
    const cleanups: (() => void)[] = []
    let gone = false

    // The canvas pans and zooms on these; inside a plugin they belong to the plugin.
    const keep = (event: Event): void => event.stopPropagation()
    element.addEventListener('pointerdown', keep)
    element.addEventListener('wheel', keep)

    const url = `${pluginOrigin(plugin)}/${page}`

    loadPage(url)
      .then(async ({ styles, body, scripts }) => {
        if (gone) return

        root.replaceChildren(...styles)
        await loaded(styles)
        if (gone) return

        root.append(...body)

        for (const script of scripts) {
          const module = await import(/* @vite-ignore */ script)
          if (gone) return

          // mount(plugin) may be async and may hand back its own cleanup.
          const cleanup = await module.default?.(api)
          if (typeof cleanup !== 'function') continue
          if (gone) return cleanup()

          cleanups.push(cleanup)
        }
      })
      .catch(host.hooks.report)

    return () => {
      gone = true
      cleanups.forEach((cleanup) => cleanup())
      dispose()
      root.replaceChildren()
      element.removeEventListener('pointerdown', keep)
      element.removeEventListener('wheel', keep)
    }
  }, [host, plugin, page, role])

  return <div ref={box} className={className} style={style} />
}

export default PluginView
