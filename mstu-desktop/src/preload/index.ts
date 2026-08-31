import { contextBridge, ipcRenderer } from 'electron'

export type EngineEvent = {
  type: 'event'
  plugin: string
  event: number
  values: unknown[]
}

/// Everything the renderer may call. Nothing else crosses the bridge.
const api = {
  call: <T = unknown,>(method: string, params: unknown = {}): Promise<T> =>
    ipcRenderer.invoke('mstu:call', method, params) as Promise<T>,

  connected: (): Promise<boolean> => ipcRenderer.invoke('mstu:connected'),

  socket: (): Promise<string> => ipcRenderer.invoke('mstu:socket'),

  pick: (kind: 'file' | 'directory'): Promise<string | null> =>
    ipcRenderer.invoke('mstu:pick', kind),

  onEvent: (listener: (event: EngineEvent) => void): (() => void) => {
    const handler = (_: unknown, payload: EngineEvent): void => listener(payload)
    ipcRenderer.on('mstu:event', handler)

    return () => ipcRenderer.removeListener('mstu:event', handler)
  },

  onStatus: (listener: (connected: boolean) => void): (() => void) => {
    const handler = (_: unknown, connected: boolean): void => listener(connected)
    ipcRenderer.on('mstu:status', handler)

    return () => ipcRenderer.removeListener('mstu:status', handler)
  }
}

export type Api = typeof api

contextBridge.exposeInMainWorld('mstu', api)
