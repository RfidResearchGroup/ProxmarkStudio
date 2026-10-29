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
  ScanLine,
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
import { openExternal } from '@/services/backend'
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

type Method = 'usb' | 'ble' | 'spp'

const router = useRouter()
const settings = useSettingsStore()
const connection = useConnectionStore()
const platform = usePlatformStore()
const toast = useToastStore()

const method = ref<Method>('usb')
const selectedUsb = ref('')
const selectedSpp = ref('')
const bleDevices = ref<BleDeviceInfo[]>([])
const selectedBle = ref<BleDeviceInfo | null>(null)
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
const methods = computed(() => [
  {
    id: 'usb' as const,
    label: settings.t('home.methodUsb'),
    short: settings.t('home.methodUsbShort'),
    icon: Usb,
    available: capabilities.value.usbSerial,
  },
  {
    id: 'ble' as const,
    label: settings.t('home.methodBle'),
    short: settings.t('home.methodBleShort'),
    icon: Bluetooth,
    available: capabilities.value.ble,
  },
  {
    id: 'spp' as const,
    label: settings.t('home.methodSpp'),
    short: settings.t('home.methodSppShort'),
    icon: Radio,
    available: capabilities.value.spp,
  },
])

const isConnected = computed(() => connection.isConnected)
const connecting = computed(() => connection.status === 'connecting' && !connection.isConnected)

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

const selectedPortValid = computed(() => {
  if (method.value === 'usb') return Boolean(selectedUsb.value)
  if (method.value === 'spp') return Boolean(selectedSpp.value)
  return Boolean(selectedBle.value)
})

const connectDisabled = computed(() => {
  if (connecting.value) return true
  if (!methods.value.find((entry) => entry.id === method.value)?.available) return true
  return !selectedPortValid.value
})

const connectLabel = computed(() => {
  if (method.value === 'ble' && selectedBle.value) {
    return settings.t('home.connectTo', { name: selectedBle.value.name })
  }
  return settings.t('home.connect')
})

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

function selectMethod(next: Method) {
  if (method.value === next) return
  method.value = next
  settings.setPreferredTransport(next)
  if (next === 'spp') {
    // SPP needs BLUETOOTH_CONNECT on Android 12+; the BLE plugin exposes the
    // same runtime permission request used for scanning.
    void ensureBluetoothPermissions().then(() => connection.refreshPorts())
  }
}

function initialiseSelections() {
  if (!selectedUsb.value && connection.usbPorts.length) selectedUsb.value = connection.usbPorts[0].path
  if (!selectedSpp.value && connection.sppPorts.length) selectedSpp.value = connection.sppPorts[0].path
}

function syncSelections() {
  if (selectedUsb.value && !connection.usbPorts.some((port) => port.path === selectedUsb.value)) {
    selectedUsb.value = ''
  }
  if (selectedSpp.value && !connection.sppPorts.some((port) => port.path === selectedSpp.value)) {
    selectedSpp.value = ''
  }
  initialiseSelections()
}

async function refreshAll() {
  refreshing.value = true
  try {
    await connection.refreshPorts()
    initialiseSelections()
    if (isConnected.value) await connection.probeDevice()
  } finally {
    refreshing.value = false
  }
}

async function connect() {
  if (connectDisabled.value) return
  connection.clearError()
  try {
    if (method.value === 'usb') {
      await connection.connectUsb(selectedUsb.value)
    } else if (method.value === 'ble') {
      if (!selectedBle.value) return
      const config: BleUartConfig | undefined = useCustomUart.value
        ? {
            serviceUuid: uart.service,
            writeUuid: uart.write,
            notifyUuid: uart.notify,
          }
        : undefined
      await connection.connectBle(selectedBle.value, config)
    } else if (method.value === 'spp') {
      await connection.connectSpp(selectedSpp.value)
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

async function startBleScan() {
  if (scanning.value) {
    await stopScanning()
    scanning.value = false
    if (scanTimer) window.clearTimeout(scanTimer)
    return
  }
  scanning.value = true
  scanned.value = false
  bleDevices.value = []
  try {
    bluetoothState.value = await getBluetoothState()
    void scanForDevices((devices) => {
      const sorted = [...devices].sort((a, b) => {
        const pm3A = /pm3|proxmark/i.test(a.name) ? 1 : 0
        const pm3B = /pm3|proxmark/i.test(b.name) ? 1 : 0
        if (pm3A !== pm3B) return pm3B - pm3A
        return b.rssi - a.rssi
      })
      bleDevices.value = sorted
      scanned.value = true
      if (!selectedBle.value && sorted.length) selectedBle.value = sorted[0]
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
  () => connection.usbPorts,
  () => syncSelections(),
  { deep: true },
)

watch(
  () => connection.sppPorts,
  () => syncSelections(),
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
  if (!capabilities.value.usbSerial && capabilities.value.ble) method.value = 'ble'
  else if (!capabilities.value.usbSerial && capabilities.value.spp) method.value = 'spp'
  else method.value = settings.preferredTransport
  if (!methods.value.find((entry) => entry.id === method.value)?.available) {
    method.value = methods.value.find((entry) => entry.available)?.id ?? 'usb'
  }
  if (capabilities.value.usbSerial || capabilities.value.spp) await connection.refreshPorts()
  initialiseSelections()
  clockTimer = window.setInterval(() => {
    now.value = Date.now()
  }, 5000)
  if (method.value === 'ble') {
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
          <div class="method-tabs">
            <button
              v-for="entry in methods"
              :key="entry.id"
              class="method-tab"
              :class="{
                'method-tab--active': method === entry.id,
                'method-tab--disabled': !entry.available,
              }"
              @click="selectMethod(entry.id)"
            >
              <component :is="entry.icon" :size="15" />
              <span>{{ entry.label }}</span>
              <span class="method-tab__dot" :class="{ 'method-tab__dot--on': entry.available }" />
            </button>
          </div>

          <div class="connect-panel">
            <div v-if="!methods.find((entry) => entry.id === method)?.available" class="unsupported">
              <div class="unsupported__icon">
                <Info :size="20" />
              </div>
              <div>
                <strong>
                  {{
                    method === 'usb'
                      ? settings.t('home.usbUnsupported')
                      : method === 'ble'
                        ? settings.t('home.bleUnsupported')
                        : settings.t('home.sppUnsupported')
                  }}
                </strong>
                <p>
                  {{
                    method === 'usb'
                      ? settings.t('home.usbUnsupportedBody')
                      : method === 'ble'
                        ? settings.t('home.bleUnsupportedBody')
                        : settings.t('home.sppUnsupportedBody')
                  }}
                </p>
              </div>
            </div>

            <template v-else-if="method === 'usb'">
              <div class="connect-panel__head">
                <div>
                  <span class="connect-panel__title">{{ settings.t('home.usbTitle') }}</span>
                  <span class="connect-panel__hint">{{ settings.t('home.usbHint') }}</span>
                </div>
                <button
                  class="btn btn--ghost btn--sm"
                  :disabled="connection.loadingPorts"
                  @click="connection.refreshPorts().then(initialiseSelections)"
                >
                  <RefreshCw :size="14" :class="{ spinning: connection.loadingPorts }" />
                  {{ settings.t('home.usbScan') }}
                </button>
              </div>

              <div v-if="connection.loadingPorts && !connection.usbPorts.length" class="port-skeletons">
                <div v-for="index in 2" :key="index" class="skeleton port-skeleton" />
              </div>
              <div v-else-if="connection.usbPorts.length" class="device-list">
                <button
                  v-for="port in connection.usbPorts"
                  :key="port.path"
                  class="device-option"
                  :class="{ 'device-option--selected': selectedUsb === port.path }"
                  @click="selectedUsb = port.path"
                >
                  <span class="device-option__icon"><Usb :size="17" /></span>
                  <span class="device-option__main">
                    <strong>{{ port.product || port.path }}</strong>
                    <span class="mono">{{ port.path }}<template v-if="port.manufacturer"> · {{ port.manufacturer }}</template></span>
                  </span>
                  <span v-if="port.serialNumber" class="device-option__serial mono">{{ port.serialNumber }}</span>
                  <span class="device-option__check" :class="{ 'device-option__check--on': selectedUsb === port.path }">
                    <Check :size="13" />
                  </span>
                </button>
              </div>
              <div v-else class="mini-empty">
                <Cable :size="20" />
                <div>
                  <strong>{{ settings.t('home.usbEmpty') }}</strong>
                  <p>{{ settings.t('home.usbEmptyHint') }}</p>
                </div>
              </div>
            </template>

            <template v-else-if="method === 'ble'">
              <div class="connect-panel__head">
                <div>
                  <span class="connect-panel__title">{{ settings.t('home.bleTitle') }}</span>
                  <span class="connect-panel__hint">{{ settings.t('home.bleHint') }}</span>
                </div>
                <button class="btn btn--sm" :class="scanning ? 'btn--outline' : 'btn--primary'" @click="startBleScan">
                  <ScanLine v-if="!scanning" :size="14" />
                  <span v-else class="spinner spinner--sm" />
                  {{ scanning ? settings.t('home.bleScanning') : settings.t('home.bleScan') }}
                </button>
              </div>

              <div v-if="bleDevices.length" class="device-list device-list--ble">
                <button
                  v-for="device in bleDevices"
                  :key="device.address"
                  class="device-option"
                  :class="{ 'device-option--selected': selectedBle?.address === device.address }"
                  @click="selectedBle = device"
                >
                  <span class="device-option__icon device-option__icon--ble"><Bluetooth :size="17" /></span>
                  <span class="device-option__main">
                    <strong>
                      {{ device.name }}
                      <span v-if="/pm3|proxmark/i.test(device.name)" class="mini-badge">PM3</span>
                    </strong>
                    <span class="mono">{{ device.address }}</span>
                  </span>
                  <span class="signal" :title="settings.t('home.rssi', { rssi: device.rssi })">
                    <i
                      v-for="bar in 4"
                      :key="bar"
                      class="signal__bar"
                      :class="{ 'signal__bar--on': bar <= signalBars(device.rssi) }"
                      :style="{ height: `${5 + bar * 3}px` }"
                    />
                    <em class="mono">{{ device.rssi }}</em>
                  </span>
                  <span class="device-option__check" :class="{ 'device-option__check--on': selectedBle?.address === device.address }">
                    <Check :size="13" />
                  </span>
                </button>
              </div>
              <div v-else-if="scanning" class="scan-state">
                <div class="scan-orbit"><span /><span /><span /></div>
                <span>{{ settings.t('home.bleScanning') }}</span>
              </div>
              <div v-else class="mini-empty">
                <Bluetooth :size="20" />
                <div>
                  <strong>{{ scanned ? settings.t('home.bleEmpty') : settings.t('home.bleTitle') }}</strong>
                  <p>{{ settings.t('home.bleEmptyHint') }}</p>
                </div>
              </div>

              <div class="advanced">
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
            </template>

            <template v-else>
              <div class="connect-panel__head">
                <div>
                  <span class="connect-panel__title">{{ settings.t('home.sppTitle') }}</span>
                  <span class="connect-panel__hint">{{ settings.t('home.sppHint') }}</span>
                </div>
                <button class="btn btn--ghost btn--sm" @click="connection.refreshPorts().then(initialiseSelections)">
                  <RefreshCw :size="14" />
                  {{ settings.t('home.usbScan') }}
                </button>
              </div>

              <div v-if="connection.sppPorts.length" class="device-list">
                <button
                  v-for="port in connection.sppPorts"
                  :key="port.path"
                  class="device-option"
                  :class="{ 'device-option--selected': selectedSpp === port.path }"
                  @click="selectedSpp = port.path"
                >
                  <span class="device-option__icon device-option__icon--ble"><Bluetooth :size="17" /></span>
                  <span class="device-option__main">
                    <strong>{{ port.product || port.path }}</strong>
                    <span class="mono">{{ port.path }}<template v-if="port.manufacturer"> · {{ port.manufacturer }}</template></span>
                  </span>
                  <span class="badge">{{ settings.t('home.paired') }}</span>
                  <span class="device-option__check" :class="{ 'device-option__check--on': selectedSpp === port.path }">
                    <Check :size="13" />
                  </span>
                </button>
              </div>
              <div v-else class="mini-empty">
                <Bluetooth :size="20" />
                <div>
                  <strong>{{ settings.t('home.sppEmpty') }}</strong>
                  <p>{{ settings.t('home.sppEmptyHint') }}</p>
                </div>
              </div>

            </template>

            <footer v-if="methods.find((entry) => entry.id === method)?.available" class="connect-panel__foot">
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
              <button class="btn btn--outline" @click="disconnect">
                <Unplug :size="15" />
                {{ settings.t('home.disconnect') }}
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
  font-size: 10px;
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
  width: 100%;
  margin-top: 14px;
  border: 1px solid var(--border);
  border-radius: 15px;
  background: rgba(5, 10, 18, 0.5);
  overflow: hidden;
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
  font-size: 9.5px;
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
  font-size: 7px;
  letter-spacing: 0.08em;
}

.device-option__serial {
  font-size: 8.5px;
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
  font-size: 8px;
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
  font-size: 10.5px;
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
  font-size: 10.5px;
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
  font-size: 10px;
  line-height: 1.4;
  color: var(--faint);
}

.uuid-grid {
  display: grid;
  gap: 8px;
  margin-top: 11px;
}

.uuid-grid .field-label {
  font-size: 9px;
}

.uuid-grid .input {
  height: 34px;
  font-size: 10px;
}

.bridge-note {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-top: 9px;
  font-size: 8.5px;
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
  font-size: 9px;
  letter-spacing: 0.08em;
  text-transform: uppercase;
  color: var(--muted);
}

.spp-mac__label span:last-child {
  font-size: 9px;
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
  font-size: 9.5px;
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
  font-size: 8px;
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
  font-size: 10px;
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
  font-size: 9.5px;
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
  font-size: 8px;
  font-weight: 600;
  letter-spacing: 0.14em;
  text-transform: uppercase;
  color: var(--faint);
}

.detail-column .info-row {
  padding: 7px 0;
}

.detail-column .info-row > strong.mono {
  font-size: 9.5px;
}

.detail-empty {
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 12px 0;
  font-size: 10.5px;
  line-height: 1.5;
  color: var(--muted);
}

.detail-empty .mono {
  color: var(--danger);
  font-size: 9px;
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
  font-size: 10.5px;
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
  font-size: 10px;
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
  font-size: 10px;
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
  font-size: 9.5px;
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
  font-size: 10px;
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
  font-size: 9px;
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
  font-size: 10px;
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
  font-size: 10px;
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
  font-size: 8.5px;
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
  font-size: 10.5px;
  line-height: 1.45;
  color: var(--text-soft);
  word-break: break-all;
}

.workspace-card__meta {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-top: 5px;
  font-size: 9.5px;
  color: var(--faint);
}

.workspace-card__link {
  display: flex;
  align-items: center;
  gap: 5px;
  margin-top: 12px;
  padding: 0;
  font-size: 10.5px;
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
  font-size: 10.5px;
  font-weight: 600;
}

.release-link__button em {
  font-style: normal;
  font-size: 9.5px;
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
</style>
