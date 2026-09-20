/// Helpers for plugin UIs.
///
/// A plugin page is an HTML file whose styles and markup the host mounts in a
/// shadow root on the canvas. Its script is a module the host imports:
///
///   <link rel="stylesheet" href="./_sdk/mstu.css" />
///   <div id="state">idle</div>
///   <script type="module" src="./index.js"></script>
///
///   // index.js
///   export default function mount(plugin) {
///     plugin.onState(({ running }) => (plugin.el('state').textContent = running ? 'on' : 'idle'))
///     return () => {} // optional cleanup when the page goes away
///   }
///
/// `plugin` addresses settings, commands and events by the names in the
/// plugin's schemas: settings, schemas, set, pick, invoke, onLive, onEvent,
/// onState, onSettings, onPopup, fit, resize, popup, closePopup, mediaUrl, el, root.
/// onState, onSettings and onPopup are also called right away with the current value.
///
/// fit(width) asks for the height this page's content needs; resize(width, height) names both.
/// Styles come from _sdk/mstu.css, whose own classes are prefixed mstu-.
///
/// Keep state inside mount(): it runs again each time the page is shown.

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
