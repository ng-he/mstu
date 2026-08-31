import { readFile } from 'fs/promises'
import { extname, join, resolve, sep } from 'path'
import { app, BrowserWindow, dialog, ipcMain, protocol } from 'electron'

import { Engine, socketPath } from './engine'

/// Plugin UI folders, keyed by lowercased plugin id (URL hosts are lowercase).
const uiFolders = new Map<string, string>()

let window: BrowserWindow | null = null

const send = (channel: string, payload: unknown): void => {
  window?.webContents.send(channel, payload)
}

const engine = new Engine(
  (message) => send('mstu:event', message),
  (connected) => {
    console.log(`engine ${connected ? 'connected' : 'disconnected'} (${socketPath()})`)
    send('mstu:status', connected)
  }
)

protocol.registerSchemesAsPrivileged([
  { scheme: 'mstu-plugin', privileges: { standard: true, secure: true } }
])

const CONTENT_TYPES: Record<string, string> = {
  '.html': 'text/html',
  '.js': 'text/javascript',
  '.css': 'text/css',
  '.json': 'application/json',
  '.svg': 'image/svg+xml',
  '.png': 'image/png'
}

/// mstu-plugin://<plugin-id>/index.html -> <ui folder>/index.html
function servePluginUi(request: Request): Promise<Response> | Response {
  const url = new URL(request.url)
  const folder = uiFolders.get(url.hostname)

  if (!folder) {
    return new Response('unknown plugin', { status: 404 })
  }

  const file = resolve(join(folder, url.pathname === '/' ? 'index.html' : url.pathname))

  // A plugin UI may not reach outside its own folder.
  if (file !== resolve(folder) && !file.startsWith(resolve(folder) + sep)) {
    return new Response('forbidden', { status: 403 })
  }

  return readFile(file)
    .then(
      (body) =>
        new Response(body, {
          headers: { 'content-type': CONTENT_TYPES[extname(file)] ?? 'application/octet-stream' }
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
