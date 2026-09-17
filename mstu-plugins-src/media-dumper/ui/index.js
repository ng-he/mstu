import { humanSize } from './_sdk/mstu.js'

export default function mount(plugin) {
  const el = plugin.el

  /// The dot lights while the byte count is still moving.
  let written = 0

  plugin.resize(300, 220)

  el('dir').addEventListener('change', (event) => plugin.set('output_path', event.target.value))
  el('browse').addEventListener('click', () => plugin.pick('output_path', { kind: 'directory' }))
  el('finish').addEventListener('click', () => plugin.invoke('write.finish'))

  plugin.onSettings((settings) => {
    el('dir').value = settings.output_path ?? ''
  })

  plugin.onLive(({ file, bytes }) => {
    const open = Boolean(file)

    el('file').textContent = open ? file : 'no file open'
    el('size').textContent = humanSize(bytes)
    el('dot').classList.toggle('on', open && bytes !== written)
    el('finish').disabled = !open

    written = bytes ?? 0
  })

  plugin.onState(({ running }) => {
    if (!running) el('dot').classList.remove('on')
  })
}
