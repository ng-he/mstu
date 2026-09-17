/// Live values arrive at 4Hz, so only correct drift bigger than that.
const DRIFT = 0.5

export default function mount(plugin) {
  const video = plugin.el('video')
  const note = plugin.el('note')

  let path = ''
  let rate = 1
  let last = -1

  const say = (text, error = false) => {
    note.textContent = text
    note.classList.toggle('error', error)
  }

  const load = () => {
    if (!path) {
      video.removeAttribute('src')
      say('no file selected')
      return
    }

    const next = plugin.mediaUrl(path)

    if (video.getAttribute('src') !== next) {
      video.setAttribute('src', next)
      say(`${path.split('/').pop()} · follows the node's position`)
    }
  }

  /// Not a player of its own: it is nudged to wherever the pipeline says it is.
  const follow = (position, advancing) => {
    if (!video.src || video.readyState < 1) return

    const at = position / 1_000_000

    if (Math.abs(video.currentTime - at) > DRIFT) {
      video.currentTime = at
    }

    // No decoder keeps up above 4x or flat out, so hold the frame instead.
    const playable = advancing && rate > 0 && rate <= 4

    if (playable) {
      video.playbackRate = rate
      if (video.paused) video.play().catch(() => undefined)
    } else if (!video.paused) {
      video.pause()
    }
  }

  video.addEventListener('error', () => say('this container cannot be previewed here', true))

  plugin.onSettings((settings) => {
    path = settings.path ?? ''
    rate = settings.rate ?? 1
    load()
  })

  plugin.onEvent('file.changed', ({ file_name }) => {
    path = file_name
    load()
  })

  plugin.onLive((live) => {
    const position = live.position ?? 0

    follow(position, position !== last)
    last = position
  })

  plugin.onState(({ running }) => {
    if (!running) video.pause()
  })

  // Let go of the file stream when the popup closes.
  return () => {
    video.pause()
    video.removeAttribute('src')
    video.load()
  }
}
