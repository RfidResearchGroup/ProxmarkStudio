import { computed, ref } from 'vue'
import { defineStore } from 'pinia'
import { en, type TranslationKey } from '@/i18n/en'
import { zh } from '@/i18n/zh'

export type ThemeName = 'midnight' | 'carbon' | 'nebula'
export type LocalePreference = 'system' | 'en' | 'zh'
export type SupportedLocale = 'en' | 'zh'

const dictionaries: Record<SupportedLocale, Record<TranslationKey, string>> = { en, zh }

const THEME_KEY = 'pm3-studio.theme'
const LOCALE_KEY = 'pm3-studio.locale'
const CONN_KEY = 'pm3-studio.connection'
const BLE_KEY = 'pm3-studio.ble'

function detectSystemLocale(): SupportedLocale {
  if (typeof navigator === 'undefined') return 'en'
  const languages = navigator.languages?.length ? navigator.languages : [navigator.language]
  for (const language of languages) {
    const normalized = (language || '').toLowerCase()
    if (normalized.startsWith('zh')) return 'zh'
    if (normalized.startsWith('en')) return 'en'
  }
  return 'en'
}

function readStorage(key: string): string | null {
  try {
    return localStorage.getItem(key)
  } catch {
    return null
  }
}

function writeStorage(key: string, value: string) {
  try {
    localStorage.setItem(key, value)
  } catch {
    /* localStorage can be unavailable in private webviews */
  }
}

// Proxmark5 BWM ESP32 uses one bidirectional SPP characteristic.
// See RfidResearchGroup/Proxmark5_BWM_esp32 DEV.md, section 5.2.
export const PM5_BLE_SERVICE = '0000AE86-0000-1000-8000-00805F9B34FB'
export const PM5_BLE_CHAR = '0000AE88-0000-1000-8000-00805F9B34FB'
export const NUS_BLE_SERVICE = '6E400001-B5A3-F393-E0A9-E50E24DCCA9E'
export const NUS_BLE_RX = '6E400002-B5A3-F393-E0A9-E50E24DCCA9E'
export const NUS_BLE_TX = '6E400003-B5A3-F393-E0A9-E50E24DCCA9E'
export const DEFAULT_BLE_SERVICE = PM5_BLE_SERVICE
export const DEFAULT_BLE_RX = PM5_BLE_CHAR
export const DEFAULT_BLE_TX = PM5_BLE_CHAR

export const useSettingsStore = defineStore('settings', () => {
  const systemLocale = ref<SupportedLocale>(detectSystemLocale())
  const localePreference = ref<LocalePreference>('system')
  const theme = ref<ThemeName>('midnight')
  const preferredTransport = ref<'usb' | 'ble' | 'spp'>('usb')
  const autoReconnect = ref(true)
  const bleService = ref(DEFAULT_BLE_SERVICE)
  const bleRx = ref(DEFAULT_BLE_RX)
  const bleTx = ref(DEFAULT_BLE_TX)

  const resolvedLocale = computed<SupportedLocale>(() =>
    localePreference.value === 'system' ? systemLocale.value : localePreference.value,
  )
  const dictionary = computed(() => dictionaries[resolvedLocale.value])

  function hydrate() {
    const storedTheme = readStorage(THEME_KEY) as ThemeName | null
    if (storedTheme === 'midnight' || storedTheme === 'carbon' || storedTheme === 'nebula') {
      theme.value = storedTheme
    }
    const storedLocale = readStorage(LOCALE_KEY) as LocalePreference | null
    if (storedLocale === 'system' || storedLocale === 'en' || storedLocale === 'zh') {
      localePreference.value = storedLocale
    }
    const storedConnection = readStorage(CONN_KEY)
    if (storedConnection) {
      try {
        const parsed = JSON.parse(storedConnection)
        if (parsed.preferredTransport === 'usb' || parsed.preferredTransport === 'ble' || parsed.preferredTransport === 'spp') {
          preferredTransport.value = parsed.preferredTransport
        }
        if (typeof parsed.autoReconnect === 'boolean') autoReconnect.value = parsed.autoReconnect
      } catch {
        /* ignore malformed preferences */
      }
    }
    const storedBle = readStorage(BLE_KEY)
    if (storedBle) {
      try {
        const parsed = JSON.parse(storedBle)
        if (typeof parsed.service === 'string') bleService.value = parsed.service
        if (typeof parsed.rx === 'string') bleRx.value = parsed.rx
        if (typeof parsed.tx === 'string') bleTx.value = parsed.tx
      } catch {
        /* ignore malformed preferences */
      }
    }
    applyTheme()
    applyLanguage()
  }

  function applyTheme() {
    if (typeof document === 'undefined') return
    document.documentElement.dataset.theme = theme.value
    document.documentElement.classList.add('dark')
    const colors: Record<ThemeName, string> = {
      midnight: '#050810',
      carbon: '#0b0a09',
      nebula: '#080714',
    }
    const meta = document.querySelector('meta[name="theme-color"]')
    if (meta) meta.setAttribute('content', colors[theme.value])
  }

  function applyLanguage() {
    if (typeof document === 'undefined') return
    document.documentElement.lang = resolvedLocale.value === 'zh' ? 'zh-CN' : 'en'
  }

  function setTheme(value: ThemeName) {
    theme.value = value
    writeStorage(THEME_KEY, value)
    applyTheme()
  }

  function setLocale(value: LocalePreference) {
    localePreference.value = value
    writeStorage(LOCALE_KEY, value)
    applyLanguage()
  }

  function setPreferredTransport(value: 'usb' | 'ble' | 'spp') {
    preferredTransport.value = value
    saveConnection()
  }

  function setAutoReconnect(value: boolean) {
    autoReconnect.value = value
    saveConnection()
  }

  function saveConnection() {
    writeStorage(
      CONN_KEY,
      JSON.stringify({ preferredTransport: preferredTransport.value, autoReconnect: autoReconnect.value }),
    )
  }

  function setBleUuids(service: string, rx: string, tx: string) {
    bleService.value = service.trim()
    bleRx.value = rx.trim()
    bleTx.value = tx.trim()
    writeStorage(BLE_KEY, JSON.stringify({ service: bleService.value, rx: bleRx.value, tx: bleTx.value }))
  }

  function resetBleUuids() {
    bleService.value = DEFAULT_BLE_SERVICE
    bleRx.value = DEFAULT_BLE_RX
    bleTx.value = DEFAULT_BLE_TX
    writeStorage(BLE_KEY, JSON.stringify({ service: bleService.value, rx: bleRx.value, tx: bleTx.value }))
  }

  function reset() {
    setTheme('midnight')
    setLocale('system')
    setPreferredTransport('usb')
    setAutoReconnect(true)
    resetBleUuids()
  }

  function listenForSystemLocale() {
    if (typeof window === 'undefined') return () => {}
    const update = () => {
      systemLocale.value = detectSystemLocale()
      applyLanguage()
    }
    window.addEventListener('languagechange', update)
    const media = window.matchMedia?.('(prefers-color-scheme: dark)')
    media?.addEventListener?.('change', update)
    return () => {
      window.removeEventListener('languagechange', update)
      media?.removeEventListener?.('change', update)
    }
  }

  const t = (key: TranslationKey, params?: Record<string, string | number>) => {
    let value = dictionary.value[key] ?? en[key] ?? key
    if (params) {
      for (const [name, replacement] of Object.entries(params)) {
        value = value.replaceAll(`{${name}}`, String(replacement))
      }
    }
    return value
  }

  return {
    systemLocale,
    localePreference,
    resolvedLocale,
    theme,
    preferredTransport,
    autoReconnect,
    bleService,
    bleRx,
    bleTx,
    hydrate,
    applyTheme,
    applyLanguage,
    setTheme,
    setLocale,
    setPreferredTransport,
    setAutoReconnect,
    setBleUuids,
    resetBleUuids,
    reset,
    listenForSystemLocale,
    t,
  }
})

export type SettingsStore = ReturnType<typeof useSettingsStore>
