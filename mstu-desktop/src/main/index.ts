import { createReadStream } from 'fs'
import { readFile, stat } from 'fs/promises'
import { extname, isAbsolute, join, resolve, sep } from 'path'
import { app, BrowserWindow, dialog, ipcMain, protocol } from 'electron'

import { Engine, socketPath } from './engine'

/// Plugin UI folders, keyed by lowercased plugin id (URL hosts are lowercase).
const uiFolders = new Map<string, string>()

let window: BrowserWindow | null = null

/// Reaching into a window destroyed mid-push throws out of the main process.
const send = (channel: string, payload: unknown): void => {
  if (!window || window.isDestroyed()) return

  window.webContents.send(channel, payload)
}

const engine = new Engine(
  (message) => send('mstu:event', message),
  (message) => send('mstu:live', message),
  (connected) => {
    console.log(`engine ${connected ? 'connected' : 'disconnected'} (${socketPath()})`)
    send('mstu:status', connected)
  }
)

protocol.registerSchemesAsPrivileged([
  // The host page fetches plugin pages and imports their scripts, both CORS requests.
  {
    scheme: 'mstu-plugin',
    privileges: { standard: true, secure: true, supportFetchAPI: true, corsEnabled: true }
  }
])

const CONTENT_TYPES: Record<string, string> = {
  '.html': 'text/html',
  '.js': 'text/javascript',
  '.css': 'text/css',
  '.json': 'application/json',
  '.svg': 'image/svg+xml',
  '.png': 'image/png'
}

/// The UI SDK's helpers and base styles, served under every plugin's own origin.
const SDK_PREFIX = '/_sdk/'
const SDK_FOLDER = join(__dirname, '../../resources/sdk')

/// A plugin UI cannot load file:// from its own scheme, so media it knows the
/// path of comes back through mstu-plugin://<id>/_media/?path=...
const MEDIA_PREFIX = '/_media/'

const MEDIA_TYPES: Record<string, string> = {
  '.mp4': 'video/mp4',
  '.m4v': 'video/mp4',
  '.mov': 'video/quicktime',
  '.webm': 'video/webm'
}

async function serveMedia(url: URL, request: Request): Promise<Response> {
  const path = url.searchParams.get('path')

  if (!path || !isAbsolute(path)) {
    return new Response('bad path', { status: 400 })
  }

  const info = await stat(path).catch(() => null)

  if (!info?.isFile()) {
    return new Response('not found', { status: 404 })
  }

  const type = MEDIA_TYPES[extname(path).toLowerCase()] ?? 'application/octet-stream'
  const stream = (from: number, to: number): ReadableStream =>
    createReadStream(path, { start: from, end: to }) as unknown as ReadableStream

  // Seeking a <video> is range requests: without 206 the element cannot jump
  // to a position, which is the whole point of following the progress bar.
  const range = /^bytes=(\d*)-(\d*)$/.exec(request.headers.get('range') ?? '')

  if (!range) {
    return new Response(stream(0, info.size - 1), {
      headers: {
        'content-type': type,
        'content-length': String(info.size),
        'accept-ranges': 'bytes'
      }
    })
  }

  const start = range[1] ? Number(range[1]) : 0
  const end = range[2] ? Math.min(Number(range[2]), info.size - 1) : info.size - 1

  if (start >= info.size || start > end) {
    return new Response('range not satisfiable', {
      status: 416,
      headers: { 'content-range': `bytes */${info.size}` }
    })
  }

  return new Response(stream(start, end), {
    status: 206,
    headers: {
      'content-type': type,
      'content-length': String(end - start + 1),
      'content-range': `bytes ${start}-${end}/${info.size}`,
      'accept-ranges': 'bytes'
    }
  })
}

/// mstu-plugin://<plugin-id>/index.html -> <ui folder>/index.html
function servePluginUi(request: Request): Promise<Response> | Response {
  const url = new URL(request.url)

  if (url.pathname.startsWith(MEDIA_PREFIX)) {
    return serveMedia(url, request)
  }

  const sdk = url.pathname.startsWith(SDK_PREFIX)
  const folder = sdk ? SDK_FOLDER : uiFolders.get(url.hostname)

  if (!folder) {
    return new Response('unknown plugin', { status: 404 })
  }

  const path = sdk ? url.pathname.slice(SDK_PREFIX.length) : url.pathname
  const file = resolve(join(folder, path === '' || path === '/' ? 'index.html' : path))

  // A plugin UI may not reach outside its own folder.
  if (file !== resolve(folder) && !file.startsWith(resolve(folder) + sep)) {
    return new Response('forbidden', { status: 403 })
  }

  return readFile(file)
    .then(
      (body) =>
        new Response(body, {
          headers: {
            'content-type': CONTENT_TYPES[extname(file)] ?? 'application/octet-stream',
            'access-control-allow-origin': '*'
          }
        })
    )
    .catch(() => new Response('not found', { status: 404 }))
}

function createWindow(): void {
  window = new BrowserWindow({
    width: 1440,
    height: 900,
    show: false,
    webPreferences: {
      preload: join(__dirname, '../preload/index.js'),
      contextIsolation: true,
      nodeIntegration: false
    }
  })

  window.on('ready-to-show', () => window?.show())
  window.on('closed', () => {
    window = null
  })

  const rendererUrl = process.env['ELECTRON_RENDERER_URL']

  if (rendererUrl) {
    window.loadURL(rendererUrl)
  } else {
    window.loadFile(join(__dirname, '../renderer/index.html'))
  }
}

ipcMain.handle('mstu:call', async (_event, method: string, params: unknown) => {
  const result = (await engine.call(method, params)) as { plugin?: string; ui?: string }

  // Remember where a new plugin's UI lives so mstu-plugin:// can serve it.
  if (method === 'create_plugin' && result?.plugin && result.ui) {
    uiFolders.set(result.plugin.toLowerCase(), result.ui)
  }

  return result
})

ipcMain.handle('mstu:connected', () => engine.connected)

ipcMain.handle('mstu:socket', () => socketPath())

/// Backs the Browse buttons in plugin UIs.
ipcMain.handle('mstu:pick', async (_event, kind: 'file' | 'directory') => {
  if (!window) return null

  const picked = await dialog.showOpenDialog(window, {
    properties: [kind === 'directory' ? 'openDirectory' : 'openFile']
  })

  return picked.canceled ? null : picked.filePaths[0]
})

app.whenReady().then(() => {
  protocol.handle('mstu-plugin', servePluginUi)

  createWindow()
  engine.connect()

  app.on('activate', () => {
    if (BrowserWindow.getAllWindows().length === 0) {
      createWindow()
    }
  })
})

app.on('window-all-closed', () => {
  if (process.platform !== 'darwin') {
    app.quit()
  }
})
