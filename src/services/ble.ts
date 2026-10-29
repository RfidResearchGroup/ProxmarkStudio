import {
  checkPermissions,
  connect as bleConnect,
  disconnect as bleDisconnect,
  getAdapterState,
  getMtu,
  startScan,
  stopScan,
  type BleCharacteristic,
  type BleConnection,
  type BleDevice,
  type BleService,
  type WriteType,
} from '@mnlphlp/plugin-blec'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { base64ToBytes, bytesToBase64 } from '@/utils/format'
import { startBleBridge, stopBleBridge, writeBleBridge } from './backend'
import {
  DEFAULT_BLE_RX,
  DEFAULT_BLE_SERVICE,
  DEFAULT_BLE_TX,
  NUS_BLE_RX,
  NUS_BLE_SERVICE,
  NUS_BLE_TX,
  PM5_BLE_CHAR,
  PM5_BLE_SERVICE,
} from '@/stores/settings'

export interface BleDeviceInfo {
  address: string
  name: string
  rssi: number
  isConnected: boolean
  isBonded: boolean
}

export interface BleUartConfig {
  serviceUuid?: string
  writeUuid?: string
  notifyUuid?: string
}

export interface BleBridge {
  port: number
  device: BleDeviceInfo
  serviceUuid: string
  writeUuid: string
  notifyUuid: string
  close: () => Promise<void>
}

interface BridgeDataEvent {
  data: string
}

interface BridgeStateEvent {
  connected: boolean
}

const NOTIFY = 0x10
const INDICATE = 0x20
const WRITE = 0x08
const WRITE_WITHOUT_RESPONSE = 0x04

function normalizeDevice(device: BleDevice): BleDeviceInfo {
  return {
    address: device.address,
    name: device.name?.trim() || device.address,
    rssi: Number.isFinite(device.rssi) ? device.rssi : -127,
    isConnected: device.isConnected,
    isBonded: device.isBonded,
  }
}

export async function ensureBluetoothPermissions(): Promise<boolean> {
  try {
    return await checkPermissions(true)
  } catch {
    return true
  }
}

export async function getBluetoothState(): Promise<string> {
  try {
    return await getAdapterState()
  } catch {
    return 'Unknown'
  }
}

export async function scanForDevices(
  onUpdate: (devices: BleDeviceInfo[]) => void,
  timeout = 12000,
): Promise<void> {
  const granted = await ensureBluetoothPermissions()
  if (!granted) throw new Error('BLE_PERMISSION_DENIED')
  await startScan((devices) => onUpdate(devices.map(normalizeDevice)), timeout)
}

export async function stopScanning(): Promise<void> {
  try {
    await stopScan()
  } catch {
    /* scanning may have already timed out */
  }
}

function sameUuid(left: string, right: string): boolean {
  const normalize = (value: string) => value.trim().replace(/-/g, '').toUpperCase()
  const a = normalize(left)
  const b = normalize(right)
  if (a === b) return true
  // Some backends expose 16-bit UUIDs as `AE88` while others expand them.
  return (a.length === 4 && b.endsWith(`0000${a}00001000800000805F9B34FB`)) ||
    (b.length === 4 && a.endsWith(`0000${b}00001000800000805F9B34FB`))
}

function canNotify(characteristic: BleCharacteristic) {
  return Boolean(characteristic.properties & (NOTIFY | INDICATE))
}

function canWrite(characteristic: BleCharacteristic) {
  return Boolean(characteristic.properties & (WRITE | WRITE_WITHOUT_RESPONSE))
}

function writeTypeFor(characteristic: BleCharacteristic): WriteType {
  return characteristic.properties & WRITE ? 'withResponse' : 'withoutResponse'
}

function chooseUart(
  services: BleService[],
  config: BleUartConfig,
): { service: string; notify: BleCharacteristic; write: BleCharacteristic; writeType: WriteType } | null {
  const configuredService = config.serviceUuid?.trim()
  const configuredNotify = config.notifyUuid?.trim()
  const configuredWrite = config.writeUuid?.trim()

  if (configuredService) {
    const service = services.find((item) => sameUuid(item.uuid, configuredService))
    if (!service) return null
    const notify = service.characteristics.find((item) =>
      sameUuid(item.uuid, configuredNotify ?? DEFAULT_BLE_TX),
    )
    const write = service.characteristics.find((item) =>
      sameUuid(item.uuid, configuredWrite ?? DEFAULT_BLE_RX),
    )
    if (!notify || !write) return null
    return { service: service.uuid, notify, write, writeType: writeTypeFor(write) }
  }

  // Proxmark5 BWM (ESP32-C2): service AE86 with one bidirectional AE88
  // characteristic. DEV.md documents read/write/notify on the same UUID.
  const pm5 = services.find((service) => sameUuid(service.uuid, PM5_BLE_SERVICE))
  if (pm5) {
    const characteristic =
      pm5.characteristics.find((item) => sameUuid(item.uuid, PM5_BLE_CHAR)) ??
      pm5.characteristics.find((item) => canNotify(item) && canWrite(item))
    if (characteristic) {
      return {
        service: pm5.uuid,
        notify: characteristic,
        write: characteristic,
        writeType: writeTypeFor(characteristic),
      }
    }
  }

  // Nordic UART Service bridge (common on third-party BLE add-ons).
  const nus = services.find((service) => sameUuid(service.uuid, NUS_BLE_SERVICE))
  if (nus) {
    const notify = nus.characteristics.find((item) => sameUuid(item.uuid, NUS_BLE_TX)) ??
      nus.characteristics.find(canNotify)
    const write = nus.characteristics.find((item) => sameUuid(item.uuid, NUS_BLE_RX)) ??
      nus.characteristics.find(canWrite)
    if (notify && write) {
      return { service: nus.uuid, notify, write, writeType: writeTypeFor(write) }
    }
  }

  // Fall back to any UART-shaped service (HM-10, custom bridges, ...).
  for (const service of services) {
    const notify = service.characteristics.find(canNotify)
    const write = service.characteristics.find(canWrite)
    if (notify && write) {
      return { service: service.uuid, notify, write, writeType: writeTypeFor(write) }
    }
  }

  return null
}

class BleUart {
  private chain: Promise<void> = Promise.resolve()
  private chunkSize = 20
  private closed = false

  constructor(
    private readonly connection: BleConnection,
    readonly serviceUuid: string,
    readonly writeUuid: string,
    readonly notifyUuid: string,
    private readonly writeType: WriteType,
    private readonly onNotification: (bytes: Uint8Array) => void,
  ) {}

  async open() {
    try {
      const mtu = await getMtu()
      if (Number.isFinite(mtu) && mtu > 23) this.chunkSize = Math.max(20, Math.floor(mtu) - 3)
    } catch {
      this.chunkSize = 20
    }
    await this.connection.subscribe(this.notifyUuid, this.serviceUuid, (data) => {
      if (!this.closed) this.onNotification(new Uint8Array(data))
    })
  }

  send(bytes: Uint8Array) {
    if (this.closed) return Promise.resolve()
    const copy = new Uint8Array(bytes)
    this.chain = this.chain.then(async () => {
      for (let offset = 0; offset < copy.length; offset += this.chunkSize) {
        if (this.closed) return
        const chunk = copy.slice(offset, offset + this.chunkSize)
        await this.connection.send(this.writeUuid, [...chunk], this.writeType, this.serviceUuid)
      }
    })
    return this.chain
  }

  async close() {
    this.closed = true
    try {
      await this.connection.unsubscribe(this.notifyUuid, this.serviceUuid)
    } catch {
      /* the link may already be gone */
    }
  }
}

export interface OpenBleBridgeOptions {
  config?: BleUartConfig
  onDisconnect?: () => void
}

/**
 * Connects to a BLE UART bridge and exposes it as a loopback TCP transport for
 * the embedded Proxmark3 client. The client cannot speak BLE itself, so frames
 * are forwarded in both directions without modification.
 */
export async function openBleBridge(
  device: BleDeviceInfo,
  options: OpenBleBridgeOptions = {},
): Promise<BleBridge> {
  const granted = await ensureBluetoothPermissions()
  if (!granted) throw new Error('BLE_PERMISSION_DENIED')

  let intentionalClose = false
  let unlistenData: UnlistenFn | null = null
  let unlistenState: UnlistenFn | null = null
  let uart: BleUart | null = null
  let connection: BleConnection | null = null
  let bridgeStarted = false

  try {
    connection = await bleConnect(device.address, () => {
      if (!intentionalClose) options.onDisconnect?.()
    })
    const services = await connection.listServices()
    const uartConfig = chooseUart(services, options.config ?? {})
    if (!uartConfig) {
      throw new Error(
        'No UART service found. Configure the service and characteristic UUIDs in Settings → BLE UART defaults.',
      )
    }

    const { port } = await startBleBridge()
    bridgeStarted = true

    uart = new BleUart(
      connection,
      uartConfig.service,
      uartConfig.write.uuid,
      uartConfig.notify.uuid,
      uartConfig.writeType,
      (bytes) => {
        void writeBleBridge(bytesToBase64(bytes)).catch(() => {
          /* socket may already be closed */
        })
      },
    )
    await uart.open()

    unlistenData = await listen<BridgeDataEvent>('bridge://data', (event) => {
      try {
        void uart?.send(base64ToBytes(event.payload.data))
      } catch {
        /* ignore malformed frames */
      }
    })
    unlistenState = await listen<BridgeStateEvent>('bridge://state', (event) => {
      if (!event.payload.connected && !intentionalClose) {
        options.onDisconnect?.()
      }
    })

    const activeUart = uart
    const activeConnection = connection
    const close = async () => {
      if (intentionalClose) return
      intentionalClose = true
      unlistenData?.()
      unlistenState?.()
      unlistenData = null
      unlistenState = null
      await activeUart.close()
      try {
        await bleDisconnect(activeConnection.address)
      } catch {
        /* already disconnected */
      }
      if (bridgeStarted) {
        bridgeStarted = false
        try {
          await stopBleBridge()
        } catch {
          /* already stopped */
        }
      }
      uart = null
      connection = null
    }

    return {
      port,
      device,
      serviceUuid: uartConfig.service,
      writeUuid: uartConfig.write.uuid,
      notifyUuid: uartConfig.notify.uuid,
      close,
    }
  } catch (error) {
    unlistenData?.()
    unlistenState?.()
    if (uart) await uart.close().catch(() => {})
    if (connection) await bleDisconnect(connection.address).catch(() => {})
    if (bridgeStarted) await stopBleBridge().catch(() => {})
    throw error
  }
}

export function defaultBleConfig(): Required<BleUartConfig> {
  return {
    serviceUuid: DEFAULT_BLE_SERVICE,
    writeUuid: DEFAULT_BLE_RX,
    notifyUuid: DEFAULT_BLE_TX,
  }
}

export function nusBleConfig(): Required<BleUartConfig> {
  return {
    serviceUuid: NUS_BLE_SERVICE,
    writeUuid: NUS_BLE_RX,
    notifyUuid: NUS_BLE_TX,
  }
}
