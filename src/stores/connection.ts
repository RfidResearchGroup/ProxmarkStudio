import { computed, ref } from 'vue'
import { defineStore } from 'pinia'
import { useSettingsStore } from './settings'
import { useToastStore } from './toast'
import * as pm3 from '@/services/pm3'
import type { SessionInfo } from '@/services/pm3'
import {
  listSerialPorts,
  isAndroid,
  type SerialPortInfo,
} from '@/services/backend'
import { openAndroidTransport, type AndroidTransport } from '@/services/androidTransport'
import {
  openBleBridge,
  type BleBridge,
  type BleDeviceInfo,
  type BleUartConfig,
} from '@/services/ble'
import { parseDeviceInfo, hasDeviceDetails as deviceHasDetails, type DeviceInfo } from '@/services/deviceInfo'
import { errorMessage } from '@/utils/format'
import type { TranslationKey } from '@/i18n/en'

export type ConnectionStatus = 'idle' | 'connecting' | 'connected' | 'error'
export type Transport = 'usb' | 'ble' | 'spp' | 'tcp'

function withTimeout<T>(promise: Promise<T>, ms: number, message: string): Promise<T> {
  return new Promise<T>((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error(message)), ms)
    promise.then(
      (value) => {
        clearTimeout(timer)
        resolve(value)
      },
      (error) => {
        clearTimeout(timer)
        reject(error)
      },
    )
  })
}

export const useConnectionStore = defineStore('connection', () => {
  const settings = useSettingsStore()
  const toast = useToastStore()

  const status = ref<ConnectionStatus>('idle')
  const transport = ref<Transport | null>(null)
  const portLabel = ref('')
  const session = ref<SessionInfo | null>(null)
  const device = ref<DeviceInfo | null>(null)
  const clientVersion = ref('')
  const connectedAt = ref<number | null>(null)
  const lastError = ref('')
  const probing = ref(false)
  const serialPorts = ref<SerialPortInfo[]>([])
  const loadingPorts = ref(false)
  const bridgePort = ref<number | null>(null)
  const lastBleDevice = ref<BleDeviceInfo | null>(null)
  const lastSerial = ref('')
  const initialised = ref(false)

  let bridge: BleBridge | null = null
  let androidTransport: AndroidTransport | null = null
  let bleDisconnectInFlight = false
  let deviceWatchTimer: number | null = null
  let deviceCheckBusy = false

  const DEVICE_WATCH_INTERVAL = 2500

  function portKey(value: string): string {
    // Windows exposes the same port as COM3 and \\.\COM3 depending on the API.
    return value
      .trim()
      .replace(/^\\\\\.\\/, '')
      .toLowerCase()
  }

  function stopDeviceWatch() {
    if (deviceWatchTimer !== null) {
      window.clearInterval(deviceWatchTimer)
      deviceWatchTimer = null
    }
  }

  async function checkDevicePresence() {
    if (!isConnected.value || (transport.value !== 'usb' && transport.value !== 'spp')) {
      stopDeviceWatch()
      return
    }
    if (deviceCheckBusy) return
    deviceCheckBusy = true
    try {
      const ports = await listSerialPorts()
      serialPorts.value = ports
      if (!isConnected.value) return

      const expected = [session.value?.port, lastSerial.value, portLabel.value]
        .filter((value): value is string => Boolean(value))
        .map(portKey)
      if (!expected.length) return

      const present = ports.some((port) => expected.includes(portKey(port.path)))
      if (!present) await handleDeviceLost()
    } catch {
      // A transient enumeration failure is not proof that the device is gone.
    } finally {
      deviceCheckBusy = false
    }
  }

  function startDeviceWatch() {
    stopDeviceWatch()
    if (transport.value !== 'usb' && transport.value !== 'spp') return
    deviceWatchTimer = window.setInterval(() => {
      void checkDevicePresence()
    }, DEVICE_WATCH_INTERVAL)
  }

  async function handleDeviceLost() {
    if (transport.value !== 'usb' && transport.value !== 'spp') return
    if (!session.value?.open) return

    stopDeviceWatch()
    lastError.value = settings.t('home.deviceLost')
    status.value = 'error'
    session.value = null
    device.value = null
    connectedAt.value = null
    toast.error(settings.t('home.deviceLostToast'), settings.t('home.deviceLostBody'))

    const nativeTransport = androidTransport
    androidTransport = null
    if (nativeTransport) await nativeTransport.close().catch(() => {})

    // The port is gone, so closing the client session may block on the dead
    // handle.  Do not make the UI wait for it.
    try {
      await withTimeout(pm3.disconnect(), 3000, 'disconnect timed out')
    } catch {
      /* the device is already gone */
    }
  }

  const isConnected = computed(
    () => status.value === 'connected' && Boolean(session.value?.open) && !session.value?.offline,
  )
  const isBusy = computed(() => status.value === 'connecting' || probing.value)
  const hasDeviceDetails = computed(() => deviceHasDetails(device.value))
  const usbPorts = computed(() => serialPorts.value.filter((port) => port.kind !== 'bluetooth'))
  const sppPorts = computed(() => serialPorts.value.filter((port) => port.kind === 'bluetooth'))
  const transportKey = computed<TranslationKey>(() => {
    switch (transport.value) {
      case 'usb':
        return 'home.transportUsb'
      case 'ble':
        return 'home.transportBle'
      case 'spp':
        return 'home.transportSpp'
      case 'tcp':
        return 'home.transportTcp'
      default:
        return 'home.transportOffline'
    }
  })

  function applySession(info: SessionInfo, chosenTransport: Transport, label: string) {
    session.value = info
    transport.value = chosenTransport
    portLabel.value = label || info.port
    status.value = 'connected'
    connectedAt.value = Date.now()
    lastError.value = ''
    startDeviceWatch()
  }

  function resetSession() {
    stopDeviceWatch()
    session.value = null
    device.value = null
    status.value = 'idle'
    transport.value = null
    portLabel.value = ''
    connectedAt.value = null
    bridgePort.value = null
  }

  async function init() {
    if (initialised.value) return
    initialised.value = true
    try {
      clientVersion.value = await pm3.version()
    } catch {
      /* the plugin may not be available in a plain browser preview */
    }
    try {
      const info = await pm3.status()
      if (info.open && !info.offline) {
        applySession(info, info.port.startsWith('tcp:127.0.0.1') ? 'ble' : 'usb', info.port)
        void probeDevice()
      }
    } catch {
      /* no session yet */
    }
  }

  async function refreshSession() {
    try {
      const info = await pm3.status()
      if (info.open && !info.offline) {
        if (status.value === 'connected') session.value = info
      } else if (status.value === 'connected') {
        // The client closed the session underneath us (for example after the
        // cable was pulled while a command was running).
        if (transport.value === 'ble') await handleBleDisconnect()
        else await handleDeviceLost()
      }
      return info
    } catch {
      return null
    }
  }

  async function refreshPorts() {
    loadingPorts.value = true
    try {
      serialPorts.value = await listSerialPorts()
    } catch (error) {
      serialPorts.value = []
      lastError.value = errorMessage(error)
    } finally {
      loadingPorts.value = false
    }
  }

  async function probeDevice() {
    if (!isConnected.value) return
    probing.value = true
    try {
      const [versionResult, statusResult] = await Promise.allSettled([
        withTimeout(pm3.execute('hw version'), 20000, 'hw version timed out'),
        withTimeout(pm3.execute('hw status'), 20000, 'hw status timed out'),
      ])
      const versionOut = versionResult.status === 'fulfilled' ? versionResult.value.output : ''
      const statusOut = statusResult.status === 'fulfilled' ? statusResult.value.output : ''
      // The port may have disappeared while the two commands were running.
      if (!isConnected.value) return
      device.value = parseDeviceInfo(versionOut, statusOut)
      if (!versionOut && !statusOut) {
        const reason =
          versionResult.status === 'rejected' ? errorMessage(versionResult.reason) : 'No identification output'
        lastError.value = reason
      }
    } finally {
      probing.value = false
    }
  }

  async function openSession(port: string, chosenTransport: Transport, label?: string) {
    if (isConnected.value) await disconnect()
    status.value = 'connecting'
    lastError.value = ''
    try {
      const info = await withTimeout(pm3.connect(port), 45000, 'Connection timed out')
      if (!info.open || info.offline) {
        throw new Error('The client opened an offline session instead of connecting to the device.')
      }
      applySession(info, chosenTransport, label ?? port)
      void probeDevice()
      return info
    } catch (error) {
      status.value = 'error'
      lastError.value = errorMessage(error)
      session.value = null
      transport.value = chosenTransport
      portLabel.value = label ?? port
      try {
        await pm3.disconnect()
      } catch {
        /* ignore cleanup failures */
      }
      throw error
    }
  }

  async function connectUsb(port: string) {
    lastSerial.value = port
    lastBleDevice.value = null
    try {
      if (isAndroid) {
        if (isConnected.value) await disconnect()
        androidTransport = await openAndroidTransport('usb', port, () => { void handleDeviceLost() })
        const info = await openSession(`tcp:127.0.0.1:${androidTransport.port}`, 'usb', port)
        toast.success(settings.t('home.connectedToast', { port }))
        return info
      }
      const info = await openSession(port, 'usb')
      toast.success(settings.t('home.connectedToast', { port }))
      return info
    } catch (error) {
      if (androidTransport) {
        const transport = androidTransport
        androidTransport = null
        await transport.close().catch(() => {})
      }
      toast.error(settings.t('home.connectionFailed'), errorMessage(error))
      throw error
    }
  }

  async function connectSpp(port: string) {
    lastSerial.value = port
    lastBleDevice.value = null
    try {
      if (isAndroid) {
        if (isConnected.value) await disconnect()
        androidTransport = await openAndroidTransport('spp', port, () => { void handleDeviceLost() })
        const info = await openSession(`tcp:127.0.0.1:${androidTransport.port}`, 'spp', port)
        toast.success(settings.t('home.connectedToast', { port }))
        return info
      }
      const info = await openSession(port, 'spp')
      toast.success(settings.t('home.connectedToast', { port }))
      return info
    } catch (error) {
      if (androidTransport) {
        const transport = androidTransport
        androidTransport = null
        await transport.close().catch(() => {})
      }
      toast.error(settings.t('home.connectionFailed'), errorMessage(error))
      throw error
    }
  }

  async function connectBle(bleDevice: BleDeviceInfo, config?: BleUartConfig) {
    if (isConnected.value) await disconnect()
    status.value = 'connecting'
    transport.value = 'ble'
    portLabel.value = `BLE 路 ${bleDevice.name}`
    lastError.value = ''
    lastBleDevice.value = bleDevice
    lastSerial.value = ''

    try {
      bridge = await openBleBridge(bleDevice, {
        config,
        onDisconnect: () => {
          void handleBleDisconnect()
        },
      })
      bridgePort.value = bridge.port
      const info = await withTimeout(
        pm3.connect(`tcp:127.0.0.1:${bridge.port}`),
        45000,
        'Timed out while opening the Proxmark session over BLE',
      )
      if (!info.open || info.offline) throw new Error('The client could not start a hardware session over BLE.')
      applySession(info, 'ble', `BLE 路 ${bleDevice.name}`)
      toast.success(settings.t('toast.bleConnected'))
      void probeDevice()
      return info
    } catch (error) {
      lastError.value = errorMessage(error)
      status.value = 'error'
      await bridge?.close().catch(() => {})
      bridge = null
      bridgePort.value = null
      try {
        await pm3.disconnect()
      } catch {
        /* ignore cleanup failures */
      }
      toast.error(settings.t('home.connectionFailed'), lastError.value)
      throw error
    }
  }

  async function handleBleDisconnect() {
    if (transport.value !== 'ble' || bleDisconnectInFlight) return
    bleDisconnectInFlight = true
    try {
      lastError.value = 'Bluetooth LE link lost'
      status.value = 'error'
      session.value = null
      bridgePort.value = null
      toast.error(settings.t('toast.bleDisconnected'))
      try {
        await pm3.disconnect()
      } catch {
        /* ignore */
      }
      await bridge?.close().catch(() => {})
      bridge = null
    } finally {
      bleDisconnectInFlight = false
    }
  }

  async function disconnect() {
    const wasConnected = Boolean(session.value?.open)
    const bleBridge = bridge
    const transport = androidTransport
    bridge = null
    androidTransport = null
    try {
      if (wasConnected) await pm3.disconnect()
    } catch (error) {
      lastError.value = errorMessage(error)
    } finally {
      if (bleBridge) await bleBridge.close().catch(() => {})
      if (transport) await transport.close().catch(() => {})
      resetSession()
      if (wasConnected) toast.info(settings.t('home.disconnectedToast'))
    }
  }

  async function reconnect() {
    if (lastBleDevice.value) return connectBle(lastBleDevice.value)
    if (lastSerial.value) {
      return transport.value === 'spp' ? connectSpp(lastSerial.value) : connectUsb(lastSerial.value)
    }
    return null
  }

  function clearError() {
    if (status.value === 'error') {
      status.value = session.value?.open ? 'connected' : 'idle'
      lastError.value = ''
    }
  }

  return {
    status,
    transport,
    portLabel,
    session,
    device,
    clientVersion,
    connectedAt,
    lastError,
    probing,
    serialPorts,
    loadingPorts,
    bridgePort,
    lastBleDevice,
    lastSerial,
    isConnected,
    isBusy,
    hasDeviceDetails,
    usbPorts,
    sppPorts,
    transportKey,
    init,
    refreshSession,
    refreshPorts,
    probeDevice,
    connectUsb,
    connectSpp,
    connectBle,
    disconnect,
    reconnect,
    clearError,
  }
})
