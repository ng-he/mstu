import { createConnection, Socket } from 'net'
import { createInterface } from 'readline'
import { join } from 'path'

export type Json = unknown

type Pending = {
  resolve: (result: Json) => void
  reject: (error: Error) => void
}

export const socketPath = (): string =>
  join(process.env.XDG_RUNTIME_DIR ?? '/tmp', 'mstu.sock')

/// Line delimited JSON over the engine's unix socket.
///
/// The engine runs on its own, so this reconnects instead of failing.
export class Engine {
  private socket: Socket | null = null
  private pending = new Map<number, Pending>()
  private nextId = 1
  private retry: NodeJS.Timeout | null = null

  /// The renderer mounts after the first connect, so it has to be able to ask.
  connected = false

  constructor(
    private onEvent: (message: Json) => void,
    private onStatus: (connected: boolean) => void
  ) {}

  connect(): void {
    if (this.socket || this.retry) return

    const socket = createConnection(socketPath())
    this.socket = socket

    socket.on('connect', () => {
      this.connected = true
      this.onStatus(true)
    })

    createInterface({ input: socket }).on('line', (line) => this.receive(line))

    socket.on('error', () => undefined)
    socket.on('close', () => {
      this.socket = null
      this.connected = false
      this.onStatus(false)

      for (const { reject } of this.pending.values()) {
        reject(new Error('engine disconnected'))
      }
      this.pending.clear()

      this.retry = setTimeout(() => {
        this.retry = null
        this.connect()
      }, 1000)
    })
  }

  private receive(line: string): void {
    let message: { id?: number; ok?: boolean; result?: Json; error?: string; type?: string }

    try {
      message = JSON.parse(line)
    } catch {
      return
    }

    if (message.type === 'event') {
      this.onEvent(message)
      return
    }

    const pending = message.id !== undefined ? this.pending.get(message.id) : undefined
    if (!pending) return

    this.pending.delete(message.id as number)

    if (message.ok) {
      pending.resolve(message.result ?? {})
    } else {
      pending.reject(new Error(message.error ?? 'engine call failed'))
    }
  }

  call(method: string, params: Json = {}): Promise<Json> {
    const socket = this.socket

    if (!socket || socket.destroyed) {
      return Promise.reject(new Error('engine not connected'))
    }

    const id = this.nextId++

    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject })
      socket.write(`${JSON.stringify({ id, method, params })}\n`)
    })
  }
}
