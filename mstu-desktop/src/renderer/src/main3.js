const { app, BrowserWindow, protocol } = require('electron')
const { readFile } = require('fs/promises')
const { join } = require('path')
protocol.registerSchemesAsPrivileged([{ scheme: 'mstu-plugin', privileges: { standard: true, secure: true } }])
app.whenReady().then(() => {
  protocol.handle('mstu-plugin', async () => new Response(await readFile(join(__dirname, 'probe.html')), { headers: { 'content-type': 'text/html' } }))
  const win = new BrowserWindow({ width: 420, height: 360 })
  win.webContents.on('console-message', (_e, _l, msg) => msg.startsWith('[probe]') && console.log(msg))
  win.loadFile(join(__dirname, 'zoom.html'))
  setTimeout(async () => {
    const img = await win.webContents.capturePage(); require('fs').writeFileSync(join(__dirname, 'zoom.png'), img.toPNG())
    // Click halfway along each bar (bar spans 6..126 css px inside the page, x2.5 on screen, plus 10px offset).
    for (const y of [10 + 2.5 * 25, 180 + 2.5 * 25]) {
      const x = Math.round(10 + 2.5 * (6 + 60))
      win.webContents.sendInputEvent({ type: 'mouseDown', x, y, button: 'left', clickCount: 1 })
      win.webContents.sendInputEvent({ type: 'mouseUp', x, y, button: 'left', clickCount: 1 })
    }
    setTimeout(() => app.quit(), 800)
  }, 2500)
})
