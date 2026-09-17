/// Room for the popup's 16:9 video plus its note line.
const PREVIEW = { title: 'Preview', width: 640, height: 384 }

/// Microseconds as m:ss.
const clock = (us) => {
  const total = Math.floor((us ?? 0) / 1_000_000)
  return `${Math.floor(total / 60)}:${String(total % 60).padStart(2, '0')}`
}

export default function mount(plugin) {
  const el = plugin.el

  let duration = 0
  let ended = false
  let open = false

  plugin.resize(320, 300)

  const showState = (text, lit) => {
    el('state').textContent = text
    el('dot').classList.toggle('on', lit)
  }

  const showProgress = (position) => {
    const ratio = duration > 0 ? Math.min(1, position / duration) : 0

    el('played').style.width = `${ratio * 100}%`
    el('head').style.left = `${ratio * 100}%`
    el('at').textContent = clock(position)
  }

  el('path').addEventListener('change', (event) => plugin.set('path', event.target.value))
  el('browse').addEventListener('click', () => plugin.pick('path', { kind: 'file' }))
  el('rate').addEventListener('change', (event) => plugin.set('rate', Number(event.target.value)))
  el('loop').addEventListener('change', (event) => plugin.set('loop', event.target.checked))

  /// The video lives in a popup over the canvas; the host says when it opens or closes.
  el('watch').addEventListener('click', () => {
    if (open) plugin.closePopup()
    else plugin.popup('preview.html', PREVIEW)
  })

  plugin.onPopup((state) => {
    open = state.open
    el('watch').classList.toggle('on', open)
  })

  /// Seeking needs a length to scrub against.
  el('scrub').addEventListener('pointerdown', (event) => {
    if (duration <= 0) return

    const box = el('scrub').getBoundingClientRect()
    const ratio = Math.min(1, Math.max(0, (event.clientX - box.left) / box.width))
    const position = Math.round(ratio * duration)

    showProgress(position)
    plugin.set('position', position)
  })

  plugin.onSettings((settings) => {
    el('path').value = settings.path ?? ''
    el('rate').value = String(settings.rate ?? 1)
    el('loop').checked = Boolean(settings.loop)
    el('watch').disabled = !settings.path
  })

  plugin.onEvent('file.changed', ({ file_name, codec }) => {
    el('file').textContent = file_name.split('/').pop()
    el('codec').textContent = codec
    el('watch').disabled = !file_name

    ended = false
  })

  plugin.onEvent('file.ended', () => showState('end of file', false))

  plugin.onLive((live) => {
    duration = live.duration ?? 0

    el('samples').textContent = (live.samples ?? 0).toLocaleString()
    el('total').textContent = duration > 0 ? clock(duration) : '--:--'
    el('scrub').setAttribute('aria-disabled', String(duration <= 0))

    showProgress(live.position ?? 0)

    ended = Boolean(live.ended)
    if (ended) showState('end of file', false)
  })

  plugin.onState(({ running }) => {
    if (!ended) showState(running ? 'playing' : 'idle', running)
  })
}
