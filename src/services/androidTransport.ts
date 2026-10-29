import { addPluginListener, invoke, type PluginListener } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { startBleBridge, stopBleBridge, writeBleBridge } from './backend'

interface TransportData {
  data: string
}

interface BridgeData {
  data: string
}

export interface AndroidTransport {
  port: number
  close: () => Promise<void>
}

type TransportKind = 'usb' | 'spp'

/**
 * Bridges an Android USB CDC-ACM or Bluetooth SPP stream to the same loopback
 * TCP bridge the embedded client already uses for BLE.
 */
export async function openAndroidTransport(
  kind: TransportKind,
  path: string,
  onLost?: () => void,
): Promise<AndroidTransport> {
  const { port } = await startBleBridge()
  let dataListener: PluginListener | null = null
  let disconnectListener: PluginListener | null = null
  let bridgeListener: UnlistenFn | null = null
  let closed = false

  const cleanup = async () => {
    if (closed) return
    closed = true
    try {
      await invoke('plugin:transport|close')
    } catch {
      /* the native transport may already be gone */
    }
    dataListener?.unregister()
    dataListener = null
    disconnectListener?.unregister()
    disconnectListener = null
    bridgeListener?.()
    bridgeListener = null
    await stopBleBridge().catch(() => {})
  }

  try {
    dataListener = await addPluginListener<TransportData>('transport', 'data', (payload) => {
      if (payload?.data) void writeBleBridge(payload.data).catch(() => {})
    })
    disconnectListener = await addPluginListener('transport', 'disconnect', () => {
      if (!closed) void onLost?.()
    })
    bridgeListener = await listen<BridgeData>('bridge://data', (event) => {
      if (event.payload?.data) {
        void invoke('plugin:transport|write', { data: event.payload.data }).catch(() => {})
      }
    })

    if (kind === 'usb') {
      await invoke('plugin:transport|open_usb', { path, baudRate: 115200 })
    } else {
      await invoke('plugin:transport|open_spp', { address: path })
    }
  } catch (error) {
    await cleanup()
    throw error
  }

  return { port, close: cleanup }
}
