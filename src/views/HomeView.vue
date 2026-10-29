<script setup lang="ts">
import { computed, onMounted, onUnmounted, reactive, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import {
  ArrowRight,
  Bluetooth,
  BookOpen,
  Cable,
  Check,
  ChevronDown,
  ChevronRight,
  Cpu,
  Info,
  PlugZap,
  Radio,
  RefreshCw,
  ShieldCheck,
  SlidersHorizontal,
  Terminal,
  Unplug,
  Usb,
  X,
} from 'lucide-vue-next'
import DeviceVisual from '@/components/DeviceVisual.vue'
import { useSettingsStore } from '@/stores/settings'
import { useConnectionStore } from '@/stores/connection'
import { usePlatformStore } from '@/stores/platform'
import { useToastStore } from '@/stores/toast'
import { openExternal, isAndroid, scanSppDevices, type SerialPortInfo } from '@/services/backend'
import {
  ensureBluetoothPermissions,
  getBluetoothState,
  scanForDevices,
  stopScanning,
  type BleDeviceInfo,
  type BleUartConfig,
} from '@/services/ble'
import { errorMessage, formatDuration } from '@/utils/format'
import { APP_DOCUMENTATION } from '@/config/links'

type DeviceKind = 'usb' | 'spp' | 'ble'

interface UnifiedDevice {
  id: string
  kind: DeviceKind
  name: string
  detail: string
  meta: string
  port?: SerialPortInfo
  ble?: BleDeviceEntry
}

interface BleDeviceEntry extends BleDeviceInfo {
  lastSeen: number
}

const BLE_STALE_MS = 15000

const router = useRouter()
const settings = useSettingsStore()
const connection = useConnectionStore()
const platform = usePlatformStore()
const toast = useToastStore()

const selectedId = ref('')
const bleDevices = ref<BleDeviceEntry[]>([])
const discoveredSpp = ref<SerialPortInfo[]>([])
const scanning = ref(false)
const scanned = ref(false)
const refreshing = ref(false)
const advancedOpen = ref(false)
const useCustomUart = ref(false)
const now = ref(Date.now())

const uart = reactive({
  service: settings.bleService,
  write: settings.bleRx,
  notify: settings.bleTx,
})

let scanTimer: number | null = null
let clockTimer: number | null = null

const capabilities = computed(() => platform.info.capabilities)

function isPm3Name(name: string): boolean {
  return /pm|proxmark/i.test(name)
}

function compareDevices(a: UnifiedDevice, b: UnifiedDevice): number {
  const pmA = isPm3Name(a.name) ? 1 : 0
  const pmB = isPm3Name(b.name) ? 1 : 0
  if (pmA !== pmB) return pmB - pmA
  const order: Record<DeviceKind, number> = { usb: 0, spp: 1, ble: 2 }
  if (order[a.kind] !== order[b.kind]) return order[a.kind] - order[b.kind]
  return a.name.localeCompare(b.name, undefined, { numeric: true, sensitivity: 'base' })
}

const allSppPorts = computed<SerialPortInfo[]>(() => {
  const ports = new Map<string, SerialPortInfo>()
  for (const port of connection.sppPorts) ports.set(port.path, port)
  for (const port of discoveredSpp.value) {
    if (!ports.has(port.path)) ports.set(port.path, port)
  }
  return [...ports.values()]
})

const unifiedDevices = computed<UnifiedDevice[]>(() => {
  const devices: UnifiedDevice[] = []
  if (capabilities.value.usbSerial) {
    for (const port of connection.usbPorts) {
      devices.push({
        id: `usb:${port.path}`,
        kind: 'usb',
        name: port.product || port.path,
        detail: [port.path, port.manufacturer].filter(Boolean).join(' · '),
        meta: '',
        port,
      })
    }
  }
  if (capabilities.value.spp) {
    for (const port of allSppPorts.value) {
      devices.push({
        id: `spp:${port.path}`,
        kind: 'spp',
        name: port.product || port.path,
        detail: port.path,
        meta:
          port.paired === false
            ? settings.t('home.unpaired')
            : settings.t('home.paired'),
        port,
      })
    }
  }
  if (capabilities.value.ble) {
    for (const device of bleDevices.value) {
      const age = Math.max(0, Math.floor((now.value - device.lastSeen) / 1000))
      devices.push({
        id: `ble:${device.address}`,
        kind: 'ble',
        name: device.name,
        detail: device.address,
        meta:
          age > 2
            ? settings.t('home.bleLastSeen', { seconds: age })
            : settings.t('home.rssi', { rssi: device.rssi }),
        ble: device,
      })
    }
  }
  return devices.sort(compareDevices)
})

const selectedDevice = computed(
  () => unifiedDevices.value.find((device) => device.id === selectedId.value) ?? null,
)

function kindLabel(kind: DeviceKind): string {
  if (kind === 'usb') return settings.t('home.deviceUsb')
  if (kind === 'spp') return settings.t('home.deviceSpp')
  return settings.t('home.deviceBle')
}

const isConnected = computed(() => connection.isConnected)
const connecting = computed(() => connection.status === 'connecting' && !connection.isConnected)
const disconnecting = computed(() => connection.disconnecting)

const heroTitle = computed(() => {
  if (isConnected.value) return settings.t('home.heroConnectedTitle')
  if (connection.status === 'error') return settings.t('home.statusError')
  return settings.t('home.heroConnectTitle')
})

const heroBody = computed(() => {
  if (isConnected.value) return settings.t('home.heroConnectedBody')
  if (connection.status === 'error' && connection.lastError) return connection.lastError
  return settings.t('home.heroConnectBody')
})

const heroState = computed(() => {
  if (isConnected.value) return settings.t('home.statusOnline')
  if (connecting.value) return settings.t('home.statusConnecting')
  if (connection.status === 'error') return settings.t('home.statusError')
  return settings.t('home.statusIdle')
})

const heroStateClass = computed(() => {
  if (isConnected.value) return 'online'
  if (connecting.value) return 'busy'
  if (connection.status === 'error') return 'error'
  return 'idle'
})

const selectedPortValid = computed(() => Boolean(selectedDevice.value))

const connectDisabled = computed(() => {
  if (connecting.value || disconnecting.value) return true
  return !selectedPortValid.value
})

const connectLabel = computed(() => {
  if (selectedDevice.value?.kind === 'ble') {
    return settings.t('home.connectTo', { name: selectedDevice.value.name })
  }
  return settings.t('home.connect')
})

const connectingTarget = computed(() => selectedDevice.value?.name || '')

const deviceRows = computed(() => {
  const device = connection.device
  if (!device) return []
  return [
    { label: settings.t('home.target'), value: device.target },
    { label: settings.t('home.firmware'), value: device.firmware },
    { label: settings.t('home.bootrom'), value: device.bootrom },
    { label: settings.t('home.hardware'), value: device.hardware || 'Proxmark3' },
    { label: 'MCU', value: device.mcu },
    { label: settings.t('home.protocol'), value: device.protocol },
  ].filter((row) => Boolean(row.value))
})

const clientVersion = computed(
  () => connection.clientVersion || connection.session?.version || connection.device?.client || '',
)

const sessionRows = computed(() => [
  {
    label: settings.t('home.clientVersion'),
    value: clientVersion.value || '—',
    mono: true,
  },
  {
    label: settings.t('home.transport'),
    value: isConnected.value ? settings.t(connection.transportKey) : '—',
  },
  {
    label: settings.t('home.port'),
    value: connection.portLabel || '—',
    mono: true,
  },
  {
    label: settings.t('home.session'),
    value: isConnected.value ? settings.t('common.connected') : settings.t('common.disconnected'),
  },
])

const uptime = computed(() => {
  if (!connection.connectedAt) return ''
  const seconds = Math.max(0, Math.floor((now.value - connection.connectedAt) / 1000))
  return formatDuration(seconds)
})

const bluetoothState = ref('Unknown')

function selectDevice(device: UnifiedDevice) {
  if (connecting.value || disconnecting.value) return
  selectedId.value = device.id
}

function syncSelection() {
  if (selectedId.value && unifiedDevices.value.some((device) => device.id === selectedId.value)) {
    return
  }
  selectedId.value = unifiedDevices.value[0]?.id ?? ''
}

async function refreshAll() {
  refreshing.value = true
  try {
    if (isAndroid) await ensureBluetoothPermissions().catch(() => {})
    await connection.refreshPorts()
    syncSelection()
    if (isConnected.value) await connection.probeDevice()
  } finally {
    refreshing.value = false
  }
}

async function connect() {
  const device = selectedDevice.value
  if (connectDisabled.value || !device) return
  connection.clearError()
  try {
    if (device.kind === 'usb') {
      await connection.connectUsb(device.port?.path ?? device.detail)
    } else if (device.kind === 'ble') {
      if (!device.ble) return
      const config: BleUartConfig | undefined = useCustomUart.value
        ? {
            serviceUuid: uart.service,
            writeUuid: uart.write,
            notifyUuid: uart.notify,
          }
        : undefined
      await connection.connectBle(device.ble, config)
    } else {
      await connection.connectSpp(device.port?.path ?? device.detail)
    }
  } catch {
    /* the store reports a toast and keeps lastError */
  }
}

async function disconnect() {
  await connection.disconnect()
}

function openTerminal() {
  if (!isConnected.value) {
    toast.warning(settings.t('terminal.connectFirst'), settings.t('terminal.connectFirstBody'))
    return
  }
  void router.push('/terminal')
}

async function scanDevices() {
  if (scanning.value) {
    await stopScanning()
    scanning.value = false
    if (scanTimer) window.clearTimeout(scanTimer)
    scanTimer = null
    return
  }
  refreshing.value = true
  try {
    if (isAndroid) await ensureBluetoothPermissions().catch(() => {})
    await connection.refreshPorts()
    syncSelection()
  } finally {
    refreshing.value = false
  }

  const sppPromise =
    isAndroid && capabilities.value.spp ? scanSppDevices().catch(() => []) : Promise.resolve([])
  if (capabilities.value.ble) {
    await startBleScan()
  } else if (capabilities.value.spp) {
    scanning.value = true
  }

  if (isAndroid && capabilities.value.spp) {
    const devices = await sppPromise
    discoveredSpp.value = devices.map((device) => ({
      path: device.address,
      kind: 'bluetooth' as const,
      manufacturer: null,
      product: device.name,
      serialNumber: null,
      vid: null,
      pid: null,
      paired: device.paired,
    }))
    syncSelection()
    if (!capabilities.value.ble) scanning.value = false
  }
}

async function startBleScan() {
  if (scanning.value) return
  scanning.value = true
  scanned.value = false
  try {
    bluetoothState.value = await getBluetoothState()
    void scanForDevices((devices) => {
      const seenAt = Date.now()
      const observed = new Set(devices.map((device) => device.address))
      for (const device of devices) {
        const existing = bleDevices.value.find((entry) => entry.address === device.address)
        if (existing) {
          Object.assign(existing, device, { lastSeen: seenAt })
        } else {
          bleDevices.value.push({ ...device, lastSeen: seenAt })
        }
      }
      bleDevices.value = bleDevices.value
        .filter(
          (device) => observed.has(device.address) || seenAt - device.lastSeen <= BLE_STALE_MS,
        )
        .sort((a, b) => {
          const pmA = isPm3Name(a.name) ? 1 : 0
          const pmB = isPm3Name(b.name) ? 1 : 0
          if (pmA !== pmB) return pmB - pmA
          return a.name.localeCompare(b.name, undefined, { numeric: true, sensitivity: 'base' })
        })
      scanned.value = true
      syncSelection()
    }, 12000).catch((error) => {
      scanning.value = false
      const message = errorMessage(error)
      toast.error(
        settings.t('common.error'),
        message === 'BLE_PERMISSION_DENIED' ? settings.t('home.blePermission') : message,
      )
    })
    scanTimer = window.setTimeout(() => {
      scanning.value = false
      scanned.value = true
      void stopScanning()
    }, 12200)
  } catch (error) {
    scanning.value = false
    toast.error(settings.t('common.error'), errorMessage(error))
  }
}

function signalBars(rssi: number) {
  if (rssi >= -55) return 4
  if (rssi >= -68) return 3
  if (rssi >= -80) return 2
  return 1
}

function openDocs() {
  void openExternal(APP_DOCUMENTATION)
}

watch(
  unifiedDevices,
  () => syncSelection(),
  { deep: true },
)

watch(
  () => settings.bleService,
  (value) => {
    uart.service = value
  },
)
watch(
  () => settings.bleRx,
  (value) => {
    uart.write = value
  },
)
watch(
  () => settings.bleTx,
  (value) => {
    uart.notify = value
  },
)

onMounted(async () => {
  await platform.init()
  if (isAndroid) await ensureBluetoothPermissions().catch(() => {})
  if (capabilities.value.usbSerial || capabilities.value.spp) await connection.refreshPorts()
  syncSelection()
  clockTimer = window.setInterval(() => {
    now.value = Date.now()
    const cutoff = now.value - BLE_STALE_MS
    if (bleDevices.value.some((device) => device.lastSeen < cutoff)) {
      bleDevices.value = bleDevices.value.filter((device) => device.lastSeen >= cutoff)
      syncSelection()
    }
  }, 5000)
  if (capabilities.value.ble) {
    void getBluetoothState().then((state) => {
      bluetoothState.value = state
    })
  }
})

onUnmounted(() => {
  if (scanTimer) window.clearTimeout(scanTimer)
  if (clockTimer) window.clearInterval(clockTimer)
  if (scanning.value) void stopScanning()
})
</script>

<template>
  <div class="page home">
    <header class="page-head">
      <div>
        <span class="eyebrow">{{ settings.t('home.eyebrow') }}</span>
        <h1 class="page-title">{{ settings.t('home.title') }}</h1>
        <p class="page-subtitle">{{ settings.t('home.subtitle') }}</p>
      </div>
      <div class="head-actions">
        <button class="btn btn--ghost btn--sm" @click="openDocs">
          <BookOpen :size="15" />
          {{ settings.t('home.documentation') }}
        </button>
        <button class="icon-btn icon-btn--bordered" :title="settings.t('common.refresh')" @click="refreshAll">
          <RefreshCw :size="16" :class="{ spinning: refreshing }" />
        </button>
      </div>
    </header>

    <section class="hero panel" :class="{ 'hero--connected': isConnected }">
      <div v-if="disconnecting" class="connect-loading connect-loading--hero">
        <div class="connect-loading__card">
          <span class="spinner" />
          <strong>{{ settings.t('home.disconnectingTitle') }}</strong>
          <p>{{ settings.t('home.disconnectingHint') }}</p>
          <span class="connect-loading__target mono">{{ connection.portLabel }}</span>
        </div>
      </div>

      <div class="hero__content">
        <div class="hero__status">
          <span class="status-dot" :class="`status-dot--${heroStateClass}`" />
          <span>{{ heroState }}</span>
          <span v-if="isConnected" class="badge badge--accent">{{ settings.t('common.online') }}</span>
          <span v-else-if="connecting" class="badge badge--warning">{{ settings.t('common.connecting') }}</span>
        </div>

        <h2 class="hero__title">{{ heroTitle }}</h2>
        <p class="hero__body">{{ heroBody }}</p>

        <div v-if="connection.status === 'error' && connection.lastError" class="error-banner">
          <Info :size="16" />
          <span>{{ connection.lastError }}</span>
          <button class="error-banner__close" @click="connection.clearError()">
            <X :size="14" />
          </button>
        </div>

        <template v-if="!isConnected">
          <div class="connect-panel">
            <div class="connect-panel__head">
              <div>
                <span class="connect-panel__title">{{ settings.t('home.devicesTitle') }}</span>
                <span class="connect-panel__hint">{{ settings.t('home.devicesHint') }}</span>
              </div>
              <div class="connect-panel__actions">
                <button
                  class="btn btn--sm"
                  :class="scanning ? 'btn--outline' : 'btn--ghost'"
                  @click="scanDevices"
                >
                  <RefreshCw v-if="!scanning" :size="14" :class="{ spinning: refreshing }" />
                  <span v-else class="spinner spinner--sm" />
                  {{
                    scanning
                      ? settings.t('home.stopScan')
                      : refreshing
                        ? settings.t('home.scanning')
                        : settings.t('home.scanDevices')
                  }}
                </button>
              </div>
            </div>

            <div v-if="unifiedDevices.length" class="device-list device-list--unified">
              <button
                v-for="device in unifiedDevices"
                :key="device.id"
                class="device-option"
                :class="{ 'device-option--selected': selectedId === device.id }"
                @click="selectDevice(device)"
              >
                <span
                  class="device-option__icon"
                  :class="{
                    'device-option__icon--usb': device.kind === 'usb',
                    'device-option__icon--ble': device.kind !== 'usb',
                  }"
                >
                  <Usb v-if="device.kind === 'usb'" :size="17" />
                  <Bluetooth v-else-if="device.kind === 'spp'" :size="17" />
                  <Radio v-else :size="17" />
                </span>
                <span class="device-option__main">
                  <strong>
                    {{ device.name }}
                    <span class="device-kind">{{ kindLabel(device.kind) }}</span>
                  </strong>
                  <span class="mono">{{ device.detail }}</span>
                </span>
                <span class="device-option__meta">
                  <span v-if="device.kind === 'ble' && device.ble" class="signal" :title="device.meta">
                    <i
                      v-for="bar in 4"
                      :key="bar"
                      class="signal__bar"
                      :class="{ 'signal__bar--on': bar <= signalBars(device.ble.rssi) }"
                      :style="{ height: `${5 + bar * 3}px` }"
                    />
                  </span>
                  <span v-if="device.meta" class="mono device-option__meta-text">{{ device.meta }}</span>
                </span>
                <span class="device-option__check" :class="{ 'device-option__check--on': selectedId === device.id }">
                  <Check :size="13" />
                </span>
              </button>
            </div>
            <div v-else class="mini-empty">
              <Cable :size="20" />
              <div>
                <strong>{{ settings.t('home.noDevices') }}</strong>
                <p>{{ settings.t('home.noDevicesHint') }}</p>
              </div>
            </div>

            <div v-if="capabilities.ble" class="advanced">
              <button class="advanced__toggle" @click="advancedOpen = !advancedOpen">
                <SlidersHorizontal :size="14" />
                <span>{{ settings.t('home.bleAdvanced') }}</span>
                <ChevronDown :size="14" :class="{ rotated: advancedOpen }" />
              </button>
              <div v-if="advancedOpen" class="advanced__body">
                <div class="setting-line">
                  <div>
                    <strong>{{ settings.t('home.bleAuto') }}</strong>
                    <span>{{ settings.t('home.bleBridgeNote') }}</span>
                  </div>
                  <button
                    class="switch"
                    :class="{ 'switch--on': useCustomUart }"
                    role="switch"
                    :aria-checked="useCustomUart"
                    @click="useCustomUart = !useCustomUart"
                  />
                </div>
                <div v-if="useCustomUart" class="uuid-grid">
                  <label class="field">
                    <span class="field-label">{{ settings.t('home.bleService') }}</span>
                    <input v-model="uart.service" class="input input--mono" spellcheck="false" />
                  </label>
                  <label class="field">
                    <span class="field-label">{{ settings.t('home.bleRx') }}</span>
                    <input v-model="uart.write" class="input input--mono" spellcheck="false" />
                  </label>
                  <label class="field">
                    <span class="field-label">{{ settings.t('home.bleTx') }}</span>
                    <input v-model="uart.notify" class="input input--mono" spellcheck="false" />
                  </label>
                </div>
                <div class="bridge-note mono">
                  <Radio :size="12" />
                  {{ bluetoothState }} · {{ settings.t('home.bleBridge') }}
                </div>
              </div>
            </div>

            <footer class="connect-panel__foot">
              <span class="connect-panel__note">
                <ShieldCheck :size="14" />
                {{ settings.t('home.heroConnectBody') }}
              </span>
              <button class="btn btn--primary" :disabled="connectDisabled" @click="connect">
                <span v-if="connecting" class="spinner spinner--sm" />
                <PlugZap v-else :size="16" />
                {{ connecting ? settings.t('common.connecting') : connectLabel }}
                <ArrowRight v-if="!connecting" :size="15" />
              </button>
            </footer>

            <div v-if="connecting" class="connect-loading">
              <div class="connect-loading__card">
                <span class="spinner" />
                <strong>{{ settings.t('home.connectingTitle') }}</strong>
                <p>
                  {{
                    selectedDevice?.kind === 'spp'
                      ? settings.t('home.connectingSppHint')
                      : settings.t('home.connectingHint')
                  }}
                </p>
                <span class="connect-loading__target mono">{{ connectingTarget }}</span>
              </div>
            </div>
          </div>
        </template>
        <template v-else>
          <div class="connected-strip">
            <div class="connected-strip__main">
              <span class="connected-strip__label">{{ settings.t('home.port') }}</span>
              <span class="connected-strip__value mono">{{ connection.portLabel }}</span>
              <span class="connected-strip__meta">
                {{ settings.t(connection.transportKey) }} · {{ settings.t('home.uptime', { time: uptime }) }}
              </span>
            </div>
            <div class="connected-strip__actions">
              <button class="btn btn--outline" :disabled="disconnecting" @click="disconnect">
                <span v-if="disconnecting" class="spinner spinner--sm" />
                <Unplug v-else :size="15" />
                {{ disconnecting ? settings.t('home.disconnecting') : settings.t('home.disconnect') }}
              </button>
              <button class="btn btn--primary" @click="openTerminal">
                <Terminal :size="16" />
                {{ settings.t('home.terminal') }}
                <ArrowRight :size="15" />
              </button>
            </div>
          </div>
        </template>
      </div>

      <div class="hero__visual">
        <DeviceVisual :connected="isConnected" :connecting="connecting" />
      </div>

      <div v-if="isConnected" class="session-detail">
        <div class="session-detail__head">
          <div class="session-detail__title">
            <Cpu :size="15" />
            {{ settings.t('home.deviceDetails') }}
          </div>
          <div class="session-detail__actions">
            <span v-if="connection.device?.target" class="badge badge--accent">{{ connection.device.target }}</span>
            <button
              class="btn btn--ghost btn--sm"
              :disabled="connection.probing"
              @click="connection.probeDevice()"
            >
              <RefreshCw :size="13" :class="{ spinning: connection.probing }" />
              {{ settings.t('home.probeAgain') }}
            </button>
          </div>
        </div>

        <div class="session-detail__grid">
          <div class="detail-column">
            <span class="detail-kicker">{{ settings.t('home.deviceInfo') }}</span>
            <div v-if="connection.probing && !deviceRows.length" class="info-skeleton">
              <div v-for="index in 4" :key="index" class="skeleton info-skeleton__row" />
            </div>
            <div v-else-if="deviceRows.length" class="info-table">
              <div v-for="row in deviceRows" :key="row.label" class="info-row">
                <span>{{ row.label }}</span>
                <strong>{{ row.value }}</strong>
              </div>
            </div>
            <div v-else class="detail-empty">
              <span>{{ settings.t('home.noDeviceInfo') }}</span>
              <span v-if="connection.lastError" class="mono">{{ connection.lastError }}</span>
            </div>
          </div>

          <div class="detail-column">
            <span class="detail-kicker">{{ settings.t('home.clientInfo') }}</span>
            <div class="info-table">
              <div v-for="row in sessionRows" :key="row.label" class="info-row">
                <span>{{ row.label }}</span>
                <strong :class="{ mono: row.mono }">{{ row.value }}</strong>
              </div>
            </div>
          </div>
        </div>

        <details v-if="connection.device?.raw" class="raw-details raw-details--wide">
          <summary>
            <span>{{ settings.t('home.rawOutput') }}</span>
            <ChevronRight :size="14" />
          </summary>
          <pre class="raw-output">{{ connection.device.raw }}</pre>
        </details>
      </div>
    </section>
  </div>
</template>

<style scoped>
.home {
  padding-bottom: 72px;
}

.spinning {
  animation: spin 0.9s linear infinite;
}

.hero {
  position: relative;
  display: grid;
  grid-template-columns: minmax(0, 1.22fr) minmax(300px, 0.78fr);
  gap: 20px;
  min-height: 420px;
  padding: 30px 32px;
  background: var(--hero-wash);
  overflow: hidden;
}

.hero--connected {
  grid-template-columns: 1fr;
  min-height: 0;
}

.hero--connected .hero__visual {
  display: none;
}

.hero::before {
  content: '';
  position: absolute;
  top: 0;
  left: 32px;
  right: 32px;
  height: 1px;
  background: linear-gradient(90deg, transparent, rgba(255, 255, 255, 0.16), transparent);
}

.hero__content {
  position: relative;
  z-index: 2;
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  min-width: 0;
}

.hero__status {
  display: flex;
  align-items: center;
  gap: 8px;
  font-family: var(--font-mono);
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0.12em;
  text-transform: uppercase;
  color: var(--muted);
}

.hero__title {
  margin-top: 19px;
  font-size: clamp(28px, 3.6vw, 43px);
  font-weight: 650;
  letter-spacing: -0.05em;
  line-height: 1.02;
  color: var(--text);
}

.hero__body {
  max-width: 500px;
  margin-top: 12px;
  color: var(--muted);
  font-size: 13.5px;
  line-height: 1.62;
}

.error-banner {
  display: flex;
  align-items: flex-start;
  gap: 9px;
  width: 100%;
  margin-top: 14px;
  padding: 10px 12px;
  border-radius: 10px;
  border: 1px solid rgba(255, 107, 129, 0.26);
  background: var(--danger-dim);
  color: var(--danger);
  font-size: 11.5px;
  line-height: 1.5;
}

.error-banner span {
  flex: 1;
  word-break: break-word;
}

.error-banner__close {
  color: var(--danger);
  opacity: 0.7;
  padding: 1px;
}

.error-banner__close:hover {
  opacity: 1;
}

.method-tabs {
  display: flex;
  gap: 4px;
  width: 100%;
  margin-top: 22px;
  padding: 4px;
  border: 1px solid var(--border);
  border-radius: 12px;
  background: rgba(4, 8, 15, 0.42);
}

.method-tab {
  position: relative;
  display: flex;
  flex: 1;
  align-items: center;
  justify-content: center;
  gap: 7px;
  height: 35px;
  padding: 0 8px;
  border-radius: 8px;
  color: var(--muted);
  font-size: 11.5px;
  font-weight: 600;
  transition:
    background var(--transition),
    color var(--transition);
  min-width: 0;
}

.method-tab span:nth-child(2) {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.method-tab:hover {
  color: var(--text-soft);
  background: rgba(255, 255, 255, 0.025);
}

.method-tab--active {
  color: var(--text);
  background: var(--panel-strong);
  box-shadow:
    0 1px 0 rgba(255, 255, 255, 0.04) inset,
    0 6px 16px rgba(0, 0, 0, 0.32);
}

.method-tab--active svg {
  color: var(--accent);
}

.method-tab--disabled {
  opacity: 0.5;
}

.method-tab__dot {
  width: 4px;
  height: 4px;
  border-radius: 50%;
  background: var(--faint);
  flex: none;
}

.method-tab__dot--on {
  background: var(--accent);
  box-shadow: 0 0 5px var(--accent);
}

.connect-panel {
  position: relative;
  width: 100%;
  margin-top: 14px;
  border: 1px solid var(--border);
  border-radius: 15px;
  background: rgba(5, 10, 18, 0.5);
  overflow: hidden;
}

.connect-loading {
  position: absolute;
  inset: 0;
  z-index: 6;
  display: grid;
  place-items: center;
  padding: 18px;
  background: rgba(4, 8, 16, 0.78);
}

.connect-loading__card {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 8px;
  width: min(100%, 320px);
  padding: 20px 18px;
  border: 1px solid var(--border-strong);
  border-radius: 14px;
  background: var(--panel-solid);
  box-shadow: 0 18px 50px rgba(0, 0, 0, 0.45);
  text-align: center;
}

.connect-loading__card .spinner {
  width: 26px;
  height: 26px;
  margin-bottom: 2px;
  border-width: 2px;
}

.connect-loading__card strong {
  font-size: 14px;
  color: var(--text);
}

.connect-loading__card p {
  font-size: 11.5px;
  line-height: 1.5;
  color: var(--muted);
}

.connect-loading__target {
  max-width: 100%;
  margin-top: 2px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 11px;
  color: var(--accent);
}

.connect-panel__head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 14px;
  padding: 14px 15px 11px;
}

.connect-panel__head > div {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}

.connect-panel__actions {
  flex-direction: row !important;
  align-items: center;
  gap: 5px;
  flex-wrap: wrap;
  justify-content: flex-end;
}

.device-list--unified {
  max-height: 340px;
}

.device-option__meta {
  display: flex;
  flex-direction: column;
  align-items: flex-end;
  gap: 2px;
  max-width: 96px;
  flex: none;
}

.device-option__meta-text {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 10.5px;
  color: var(--faint);
}

.device-kind {
  display: inline-flex;
  align-items: center;
  height: 16px;
  padding: 0 5px;
  border-radius: 4px;
  border: 1px solid var(--border);
  background: var(--panel-inset);
  font-family: var(--font-mono);
  font-size: 9px;
  font-weight: 600;
  letter-spacing: 0.04em;
  color: var(--muted);
}

.device-option__icon--usb {
  background: var(--secondary-dim);
  color: var(--secondary);
}

.connect-panel__title {
  font-size: 12px;
  font-weight: 650;
  color: var(--text-soft);
}

.connect-panel__hint {
  font-size: 11px;
  line-height: 1.45;
  color: var(--muted);
}

.device-list {
  display: flex;
  flex-direction: column;
  gap: 5px;
  max-height: 196px;
  padding: 0 10px 10px;
  overflow: auto;
  overscroll-behavior: contain;
}

.device-list--ble {
  max-height: 220px;
}

.device-option {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
  padding: 9px 10px;
  border: 1px solid transparent;
  border-radius: 9px;
  background: rgba(255, 255, 255, 0.016);
  text-align: left;
  transition:
    border-color var(--transition),
    background var(--transition);
}

.device-option:hover {
  background: rgba(255, 255, 255, 0.035);
  border-color: var(--border);
}

.device-option--selected {
  border-color: var(--border-accent);
  background: var(--accent-faint);
}

.device-option__icon {
  display: grid;
  place-items: center;
  width: 30px;
  height: 30px;
  border-radius: 8px;
  background: var(--secondary-dim);
  color: var(--secondary);
  flex: none;
}

.device-option__icon--ble {
  background: var(--accent-dim);
  color: var(--accent);
}

.device-option__main {
  display: flex;
  flex-direction: column;
  gap: 1px;
  min-width: 0;
  flex: 1;
}

.device-option__main strong {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 11.5px;
  font-weight: 600;
  color: var(--text-soft);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.device-option__main span {
  font-size: 11.5px;
  color: var(--faint);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.mini-badge {
  display: inline-flex;
  align-items: center;
  height: 15px;
  padding: 0 4px;
  border-radius: 4px;
  background: var(--accent-dim);
  color: var(--accent);
  font-family: var(--font-mono);
  font-size: 9.5px;
  letter-spacing: 0.08em;
}

.device-option__serial {
  font-size: 11px;
  color: var(--faint);
  display: none;
}

.device-option__check {
  display: grid;
  place-items: center;
  width: 19px;
  height: 19px;
  border-radius: 50%;
  border: 1px solid var(--border);
  color: transparent;
  flex: none;
}

.device-option__check--on {
  border-color: var(--accent);
  background: var(--accent);
  color: var(--accent-contrast);
}

.signal {
  display: flex;
  align-items: flex-end;
  gap: 2px;
  flex: none;
}

.signal__bar {
  width: 2.5px;
  border-radius: 2px;
  background: var(--border-strong);
}

.signal__bar--on {
  background: var(--accent);
}

.signal em {
  margin-left: 4px;
  font-size: 11px;
  font-style: normal;
  color: var(--faint);
}

.port-skeletons {
  display: flex;
  flex-direction: column;
  gap: 5px;
  padding: 0 10px 10px;
}

.port-skeleton {
  height: 50px;
}

.mini-empty {
  display: flex;
  align-items: flex-start;
  gap: 11px;
  margin: 0 10px 10px;
  padding: 13px;
  border-radius: 10px;
  border: 1px dashed var(--border);
  color: var(--faint);
}

.mini-empty strong {
  display: block;
  font-size: 11.5px;
  font-weight: 600;
  color: var(--muted);
}

.mini-empty p {
  margin-top: 3px;
  font-size: 11.5px;
  line-height: 1.5;
  color: var(--faint);
}

.mini-empty--plain {
  margin: 0;
  align-items: center;
}

.mini-empty--plain .btn {
  margin-left: auto;
}

.scan-state {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 10px;
  padding: 18px 12px;
  font-size: 11px;
  color: var(--muted);
}

.scan-orbit {
  position: relative;
  width: 26px;
  height: 26px;
  border-radius: 50%;
  border: 1px solid var(--border-accent);
}

.scan-orbit span {
  position: absolute;
  inset: 0;
  border-radius: 50%;
  border: 1px solid var(--accent);
  animation: orbit 2.2s var(--ease-out) infinite;
  opacity: 0;
}

.scan-orbit span:nth-child(2) {
  animation-delay: 0.7s;
}
.scan-orbit span:nth-child(3) {
  animation-delay: 1.4s;
}

@keyframes orbit {
  0% {
    transform: scale(0.3);
    opacity: 0.8;
  }
  100% {
    transform: scale(1.6);
    opacity: 0;
  }
}

.advanced {
  border-top: 1px solid var(--border);
}

.advanced__toggle {
  display: flex;
  align-items: center;
  gap: 7px;
  width: 100%;
  padding: 10px 15px;
  color: var(--muted);
  font-size: 11.5px;
  font-weight: 550;
}

.advanced__toggle svg:last-child {
  margin-left: auto;
  transition: transform var(--transition);
}

.advanced__toggle .rotated {
  transform: rotate(180deg);
}

.advanced__body {
  padding: 2px 15px 13px;
}

.setting-line {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
}

.setting-line strong {
  display: block;
  font-size: 11px;
  color: var(--text-soft);
}

.setting-line span {
  display: block;
  margin-top: 2px;
  font-size: 11px;
  line-height: 1.4;
  color: var(--faint);
}

.uuid-grid {
  display: grid;
  gap: 8px;
  margin-top: 11px;
}

.uuid-grid .field-label {
  font-size: 11.5px;
}

.uuid-grid .input {
  height: 34px;
  font-size: 11px;
}

.bridge-note {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-top: 9px;
  font-size: 11px;
  color: var(--faint);
}

.spp-mac {
  display: flex;
  flex-direction: column;
  gap: 6px;
  margin: 0 10px 10px;
  padding: 11px 12px;
  border-radius: 9px;
  border: 1px dashed var(--border);
}

.spp-mac__label {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}

.spp-mac__label span:first-child {
  font-size: 11.5px;
  letter-spacing: 0.08em;
  text-transform: uppercase;
  color: var(--muted);
}

.spp-mac__label span:last-child {
  font-size: 11.5px;
  color: var(--faint);
}

.spp-mac .input {
  height: 34px;
  font-size: 11px;
}

.connect-panel__foot {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 12px 14px;
  border-top: 1px solid var(--border);
  background: rgba(0, 0, 0, 0.12);
}

.connect-panel__note {
  display: flex;
  align-items: center;
  gap: 6px;
  max-width: 260px;
  font-size: 11.5px;
  line-height: 1.4;
  color: var(--faint);
}

.connect-panel__note svg {
  flex: none;
  color: var(--accent);
  opacity: 0.7;
}

.unsupported {
  display: flex;
  align-items: flex-start;
  gap: 12px;
  padding: 18px 16px;
}

.unsupported__icon {
  display: grid;
  place-items: center;
  width: 38px;
  height: 38px;
  border-radius: 10px;
  background: var(--warning-dim);
  color: var(--warning);
  flex: none;
}

.unsupported strong {
  font-size: 12px;
  color: var(--text-soft);
}

.unsupported p {
  margin-top: 4px;
  font-size: 11px;
  line-height: 1.55;
  color: var(--muted);
}

.connected-strip {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 18px;
  width: 100%;
  margin-top: 26px;
  padding: 16px 17px;
  border-radius: 14px;
  border: 1px solid var(--border-accent);
  background: linear-gradient(120deg, var(--accent-faint), rgba(4, 9, 15, 0.4));
}

.connected-strip__main {
  display: flex;
  flex-direction: column;
  gap: 3px;
  min-width: 0;
}

.connected-strip__label {
  font-family: var(--font-mono);
  font-size: 11px;
  letter-spacing: 0.14em;
  text-transform: uppercase;
  color: var(--accent);
}

.connected-strip__value {
  font-size: 15px;
  font-weight: 600;
  color: var(--text);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.connected-strip__meta {
  font-size: 11px;
  color: var(--muted);
}

.connected-strip__actions {
  display: flex;
  gap: 8px;
  flex: none;
}

.connected-meta {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin-top: 12px;
}

.connected-meta__item {
  display: flex;
  align-items: center;
  gap: 5px;
  height: 25px;
  padding: 0 9px;
  border-radius: 7px;
  border: 1px solid var(--border);
  background: rgba(0, 0, 0, 0.12);
  font-size: 11.5px;
  color: var(--muted);
}

.connected-meta__item svg {
  color: var(--faint);
}

.session-detail {
  grid-column: 1 / -1;
  display: flex;
  flex-direction: column;
  gap: 16px;
  margin-top: 4px;
  padding-top: 20px;
  border-top: 1px solid var(--border);
}

.session-detail__head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 14px;
}

.session-detail__title {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12px;
  font-weight: 620;
  color: var(--text-soft);
}

.session-detail__title svg {
  color: var(--accent);
}

.session-detail__actions {
  display: flex;
  align-items: center;
  gap: 8px;
}

.session-detail__grid {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
  gap: 30px;
}

.detail-column {
  display: flex;
  flex-direction: column;
  min-width: 0;
}

.detail-kicker {
  display: block;
  margin-bottom: 5px;
  font-family: var(--font-mono);
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0.14em;
  text-transform: uppercase;
  color: var(--faint);
}

.detail-column .info-row {
  padding: 7px 0;
}

.detail-column .info-row > strong.mono {
  font-size: 11.5px;
}

.detail-empty {
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 12px 0;
  font-size: 11.5px;
  line-height: 1.5;
  color: var(--muted);
}

.detail-empty .mono {
  color: var(--danger);
  font-size: 11.5px;
}

.raw-details--wide {
  margin-top: 0;
  padding-top: 12px;
}

.raw-details--wide .raw-output {
  max-height: 280px;
}

.hero__visual {
  display: flex;
  align-items: center;
  justify-content: center;
  min-width: 0;
}

.home-grid {
  display: grid;
  grid-template-columns: minmax(0, 1.55fr) minmax(320px, 0.85fr);
  gap: 16px;
  margin-top: 16px;
  align-items: start;
}

.home-col {
  display: flex;
  flex-direction: column;
  gap: 16px;
  min-width: 0;
}

.info-panel .panel-body {
  padding-top: 14px;
}

.info-table {
  display: flex;
  flex-direction: column;
}

.info-row {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 20px;
  padding: 9px 0;
  border-bottom: 1px dashed var(--border);
}

.info-row:last-child {
  border-bottom: 0;
}

.info-row > span {
  font-size: 11.5px;
  color: var(--faint);
  flex: none;
}

.info-row > strong {
  font-size: 11.5px;
  font-weight: 550;
  color: var(--text-soft);
  text-align: right;
  word-break: break-word;
}

.info-row > strong.mono {
  font-size: 11px;
  color: var(--accent);
}

.info-table--compact .info-row {
  padding: 8px 0;
}

.raw-details {
  margin-top: 10px;
  border-top: 1px solid var(--border);
  padding-top: 8px;
}

.raw-details summary {
  display: flex;
  align-items: center;
  justify-content: space-between;
  cursor: pointer;
  list-style: none;
  font-size: 11px;
  color: var(--faint);
}

.raw-details summary::-webkit-details-marker {
  display: none;
}

.raw-details[open] summary svg {
  transform: rotate(90deg);
}

.raw-details summary svg {
  transition: transform var(--transition);
}

.raw-output {
  max-height: 240px;
  margin: 10px 0 0;
  padding: 12px;
  overflow: auto;
  border-radius: 8px;
  border: 1px solid var(--border);
  background: var(--bg-deep);
  font-family: var(--font-mono);
  font-size: 11.5px;
  line-height: 1.65;
  color: var(--muted);
  white-space: pre-wrap;
  word-break: break-word;
}

.info-skeleton {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.info-skeleton__row {
  height: 27px;
}

.tips {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.tips__title {
  display: flex;
  align-items: center;
  gap: 7px;
  margin-bottom: 7px;
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0.08em;
  text-transform: uppercase;
  color: var(--faint);
}

.tips__title svg {
  color: var(--accent);
}

.tip-row {
  display: flex;
  gap: 12px;
  padding: 9px 0;
  border-bottom: 1px dashed var(--border);
}

.tip-row:last-child {
  border: 0;
  padding-bottom: 0;
}

.tip-row__index {
  padding-top: 1px;
  font-family: var(--font-mono);
  font-size: 11.5px;
  color: var(--accent);
  opacity: 0.7;
  flex: none;
}

.tip-row p {
  font-size: 11px;
  line-height: 1.55;
  color: var(--muted);
}

.quick-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 8px;
}

.quick-card {
  display: flex;
  align-items: flex-start;
  gap: 10px;
  padding: 13px;
  border-radius: 11px;
  border: 1px solid var(--border);
  background: rgba(255, 255, 255, 0.014);
  text-align: left;
  transition:
    border-color var(--transition),
    background var(--transition),
    transform var(--transition);
}

.quick-card:hover {
  border-color: var(--border-accent);
  background: var(--accent-faint);
  transform: translateY(-1px);
}

.quick-card--muted {
  opacity: 0.66;
}

.quick-card__icon {
  display: grid;
  place-items: center;
  width: 32px;
  height: 32px;
  border-radius: 9px;
  background: var(--accent-dim);
  color: var(--accent);
  flex: none;
}

.quick-card__icon--blue {
  background: var(--secondary-dim);
  color: var(--secondary);
}
.quick-card__icon--amber {
  background: var(--warning-dim);
  color: var(--warning);
}
.quick-card__icon--violet {
  background: rgba(177, 140, 255, 0.12);
  color: #b18cff;
}

.quick-card > span:nth-child(2) {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}

.quick-card strong {
  font-size: 11.5px;
  font-weight: 600;
  color: var(--text-soft);
}

.quick-card em {
  font-style: normal;
  font-size: 11px;
  line-height: 1.4;
  color: var(--faint);
}

.quick-card__arrow {
  margin-left: auto;
  color: var(--faint);
  flex: none;
  opacity: 0;
  transition: opacity var(--transition);
}

.quick-card:hover .quick-card__arrow {
  opacity: 1;
}

.session-state {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 11px;
  margin-bottom: 10px;
  border-radius: 10px;
  border: 1px solid var(--border);
  background: var(--panel-inset);
}

.session-state--online {
  border-color: var(--border-accent);
  background: var(--accent-faint);
}

.session-state__icon {
  display: grid;
  place-items: center;
  width: 32px;
  height: 32px;
  border-radius: 9px;
  background: rgba(255, 255, 255, 0.03);
  color: var(--faint);
  flex: none;
}

.session-state--online .session-state__icon {
  color: var(--accent);
  background: var(--accent-dim);
}

.session-state strong {
  display: block;
  font-size: 11.5px;
  font-weight: 600;
  color: var(--text-soft);
}

.session-state span {
  font-size: 11px;
  color: var(--faint);
}

.copy-line {
  display: flex;
  align-items: center;
  gap: 6px;
  width: 100%;
  margin-top: 10px;
  padding: 7px 8px;
  border-radius: 7px;
  border: 1px dashed var(--border);
  color: var(--faint);
  text-align: left;
}

.copy-line:hover {
  color: var(--muted);
  border-color: var(--border-strong);
}

.copy-line span {
  font-size: 11px;
}

.workspace-card {
  padding: 14px;
  border-radius: 12px;
  border: 1px solid var(--border);
  background:
    radial-gradient(circle at 88% 0%, var(--accent-faint), transparent 54%),
    var(--panel-inset);
}

.workspace-card__top {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.workspace-card__icon {
  display: grid;
  place-items: center;
  width: 34px;
  height: 34px;
  border-radius: 9px;
  background: var(--accent-dim);
  color: var(--accent);
}

.workspace-card__path {
  margin-top: 11px;
  font-family: var(--font-mono);
  font-size: 11.5px;
  line-height: 1.45;
  color: var(--text-soft);
  word-break: break-all;
}

.workspace-card__meta {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-top: 5px;
  font-size: 11.5px;
  color: var(--faint);
}

.workspace-card__link {
  display: flex;
  align-items: center;
  gap: 5px;
  margin-top: 12px;
  padding: 0;
  font-size: 11.5px;
  font-weight: 600;
  color: var(--accent);
}

.release-link {
  margin-top: 10px;
  border-top: 1px solid var(--border);
  padding-top: 10px;
}

.release-link__button {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  width: 100%;
  padding: 2px 1px;
  color: var(--muted);
  text-align: left;
}

.release-link__button:hover {
  color: var(--text-soft);
}

.release-link__button span {
  display: flex;
  flex-direction: column;
  gap: 1px;
}

.release-link__button strong {
  font-size: 11.5px;
  font-weight: 600;
}

.release-link__button em {
  font-style: normal;
  font-size: 11.5px;
  color: var(--faint);
}

@media (max-width: 1100px) {
  .hero {
    grid-template-columns: minmax(0, 1fr) minmax(260px, 0.65fr);
    padding: 26px;
  }

  .home-grid {
    grid-template-columns: minmax(0, 1.35fr) minmax(280px, 0.8fr);
  }

  .connect-panel__note {
    display: none;
  }
}

@media (max-width: 980px) {
  .hero {
    grid-template-columns: 1fr;
  }

  .hero__visual {
    display: none;
  }

  .session-detail {
    grid-column: 1;
  }

  .home-grid {
    grid-template-columns: 1fr;
  }

  .home-col--side {
    display: grid;
    grid-template-columns: 1fr 1fr;
    align-items: start;
  }
}

@media (max-width: 640px) {
  .home {
    padding-bottom: 92px;
  }

  .hero {
    padding: 20px 16px;
    min-height: 0;
    border-radius: 16px;
  }

  .hero__title {
    margin-top: 15px;
    font-size: 30px;
  }

  .hero__body {
    font-size: 12px;
  }

  .method-tabs {
    margin-top: 17px;
    overflow-x: auto;
  }

  .method-tab {
    flex: none;
    min-width: 95px;
  }

  .connect-panel__head {
    padding: 12px 12px 9px;
  }

  .connect-panel__hint {
    max-width: 230px;
  }

  .connect-panel__foot {
    flex-direction: column;
    align-items: stretch;
  }

  .connect-panel__foot .btn {
    width: 100%;
  }

  .connected-strip {
    flex-direction: column;
    align-items: stretch;
    margin-top: 20px;
  }

  .connected-strip__actions {
    display: grid;
    grid-template-columns: 1fr 1fr;
  }

  .connected-strip__actions .btn {
    width: 100%;
    padding-inline: 8px;
    font-size: 11.5px;
  }

  .session-detail {
    padding-top: 16px;
  }

  .session-detail__head {
    align-items: flex-start;
    flex-direction: column;
    gap: 8px;
  }

  .session-detail__actions {
    width: 100%;
    justify-content: space-between;
  }

  .session-detail__grid {
    grid-template-columns: 1fr;
    gap: 18px;
  }

  .home-col--side {
    display: flex;
  }

  .quick-grid {
    grid-template-columns: 1fr;
  }

  .info-row {
    gap: 12px;
  }

  .info-row > strong {
    font-size: 11px;
  }
}


@media (max-width: 640px) {
  .home {
    display: flex;
    flex-direction: column;
    height: calc(100dvh - var(--header-height) - env(safe-area-inset-top));
    min-height: 0;
    overflow: hidden;
  }

  .home > .page-head {
    flex: none;
  }

  .hero:not(.hero--connected) {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-height: 0;
    overflow: hidden;
    padding: 12px;
  }

  .hero:not(.hero--connected) .hero__content {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-height: 0;
    overflow: hidden;
  }

  .hero:not(.hero--connected) .hero__status,
  .hero:not(.hero--connected) .hero__title,
  .hero:not(.hero--connected) .hero__body {
    display: none;
  }

  .hero:not(.hero--connected) .method-tabs {
    margin-top: 0;
  }

  .connect-panel {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-height: 0;
    overflow: hidden;
    margin-top: 10px;
  }

  .connect-panel__head {
    flex: none;
    align-items: center;
    padding: 10px 11px 8px;
  }

  .connect-panel__title {
    font-size: 12.5px;
  }

  .connect-panel__hint {
    display: none;
  }

  .device-list,
  .device-list--ble {
    max-height: min(42vh, 390px);
  }

  .device-list--unified {
    flex: 1;
    min-height: 0;
    max-height: none;
    overflow-y: auto;
    overscroll-behavior: contain;
    -webkit-overflow-scrolling: touch;
    touch-action: pan-y;
  }

  .device-option {
    padding: 10px 9px;
  }

  .mini-empty {
    margin: 0 8px 8px;
    padding: 10px;
  }

  .mini-empty p {
    display: none;
  }

  .connect-panel__foot {
    flex: none;
    padding: 9px;
  }

  .connect-panel__foot .btn {
    min-height: 44px;
  }
}

@media (max-width: 640px) {
  .home .head-actions {
    display: none;
  }
}

</style>
