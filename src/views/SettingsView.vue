<script setup lang="ts">
import { computed, onMounted, reactive, ref } from 'vue'
import {
  ArrowUpRight,
  Check,
  CircleAlert,
  Copy,
  Download,
  ExternalLink,
  Github,
  HardDrive,
  Info,
  Languages,
  Loader2,
  Palette,
  RefreshCw,
  RotateCcw,
  Save,
  Settings2,
  ShieldAlert,
  Sparkles,
  Usb,
  Bluetooth,
  Radio,
  FileCog,
} from 'lucide-vue-next'
import Modal from '@/components/Modal.vue'
import { useSettingsStore, type ThemeName } from '@/stores/settings'
import { usePlatformStore } from '@/stores/platform'
import { useResourcesStore } from '@/stores/resources'
import { useConnectionStore } from '@/stores/connection'
import { useToastStore } from '@/stores/toast'
import { getDiagnostics, openExternal, revealPath } from '@/services/backend'
import { copyText, errorMessage, formatBytes, formatDate, shortPath } from '@/utils/format'

const settings = useSettingsStore()
const platform = usePlatformStore()
const resources = useResourcesStore()
const connection = useConnectionStore()
const toast = useToastStore()

const diagnosticsBusy = ref(false)
const resetOpen = ref(false)
const bleDraft = reactive({
  service: settings.bleService,
  rx: settings.bleRx,
  tx: settings.bleTx,
})

const themes = computed<
  Array<{ id: ThemeName; name: string; description: string }>
>(() => [
  { id: 'midnight', name: settings.t('settings.themeMidnight'), description: settings.t('settings.themeMidnightDesc') },
  { id: 'carbon', name: settings.t('settings.themeCarbon'), description: settings.t('settings.themeCarbonDesc') },
  { id: 'nebula', name: settings.t('settings.themeNebula'), description: settings.t('settings.themeNebulaDesc') },
])

const languages = computed(() => [
  { id: 'system' as const, name: settings.t('settings.followSystem'), detail: navigator.language || '—', glyph: 'AUTO' },
  { id: 'en' as const, name: settings.t('settings.english'), detail: 'English', glyph: 'EN' },
  { id: 'zh' as const, name: settings.t('settings.chinese'), detail: '简体中文', glyph: '中' },
])

const currentLanguage = computed(() =>
  settings.resolvedLocale === 'zh' ? settings.t('settings.chinese') : settings.t('settings.english'),
)

const clientVersion = computed(
  () => connection.clientVersion || resources.status?.version || connection.session?.version || '—',
)

const commit = computed(() => {
  const value = resources.status?.expectedCommit
  if (value) return value
  const match = clientVersion.value.match(/g([0-9a-f]{7,40})/i)
  return match?.[1] ?? '—'
})

const shortCommit = computed(() => (commit.value && commit.value !== '—' ? commit.value.slice(0, 12) : '—'))

const resourceStage = computed(() => {
  const progress = resources.progress
  switch (progress.stage) {
    case 'fetching':
      return settings.t('settings.progressFetching')
    case 'downloading':
      return settings.t('settings.progressDownloading', { size: formatBytes(progress.total) })
    case 'verifying':
      return settings.t('settings.progressVerifying')
    case 'extracting':
      return settings.t('settings.progressExtracting')
    default:
      return settings.t('settings.progressDone')
  }
})

function selectTheme(id: ThemeName) {
  settings.setTheme(id)
}

function installTheme() {
  void resources.install().catch(() => {})
}

function checkRelease() {
  void resources.checkForRelease()
}

function saveBleDefaults() {
  settings.setBleUuids(bleDraft.service, bleDraft.rx, bleDraft.tx)
  toast.success(settings.t('settings.saved'))
}

function restoreBleDefaults() {
  settings.resetBleUuids()
  bleDraft.service = settings.bleService
  bleDraft.rx = settings.bleRx
  bleDraft.tx = settings.bleTx
}

async function copyDiagnostics() {
  diagnosticsBusy.value = true
  try {
    const text = await getDiagnostics()
    await copyText(text)
    toast.success(settings.t('settings.diagnosticsCopied'))
  } catch (error) {
    toast.error(settings.t('common.error'), errorMessage(error))
  } finally {
    diagnosticsBusy.value = false
  }
}

async function openWorkspace() {
  try {
    await revealPath('')
    toast.info(settings.t('toast.revealed'))
  } catch (error) {
    toast.error(settings.t('common.error'), errorMessage(error))
  }
}

function confirmReset() {
  settings.reset()
  bleDraft.service = settings.bleService
  bleDraft.rx = settings.bleRx
  bleDraft.tx = settings.bleTx
  resetOpen.value = false
  toast.success(settings.t('settings.resetDone'))
}

function openReleases() {
  const url =
    resources.release?.htmlUrl ?? 'https://github.com/xianglin1998/tauri-plugin-proxmark3/releases'
  void openExternal(url)
}

function openSource() {
  void openExternal('https://github.com/xianglin1998/tauri-plugin-proxmark3')
}

function openClientSource() {
  void openExternal('https://github.com/RfidResearchGroup/proxmark3')
}

onMounted(async () => {
  if (!platform.ready) await platform.init()
  await resources.init()
  if (window.location.hash === '#resources') {
    document.getElementById('resources')?.scrollIntoView({ behavior: 'smooth', block: 'start' })
  }
})
</script>

<template>
  <div class="page settings-page">
    <header class="page-head">
      <div>
        <span class="eyebrow">{{ settings.t('settings.eyebrow') }}</span>
        <h1 class="page-title">{{ settings.t('settings.title') }}</h1>
        <p class="page-subtitle">{{ settings.t('settings.subtitle') }}</p>
      </div>
      <div class="head-actions">
        <span class="autosave">{{ settings.t('settings.autosave') }}</span>
        <button class="btn btn--ghost btn--sm" @click="resetOpen = true">
          <RotateCcw :size="14" />
          {{ settings.t('settings.reset') }}
        </button>
      </div>
    </header>

    <div class="settings-grid">
      <div class="settings-col">
        <section class="panel settings-card">
          <div class="panel-head">
            <h3 class="panel-title"><Palette :size="15" />{{ settings.t('settings.appearance') }}</h3>
            <span class="badge">{{ settings.theme }}</span>
          </div>
          <div class="panel-body">
            <p class="card-hint">{{ settings.t('settings.appearanceHint') }}</p>
            <div class="theme-grid">
              <button
                v-for="theme in themes"
                :key="theme.id"
                class="theme-option"
                :class="{ 'theme-option--active': settings.theme === theme.id }"
                @click="selectTheme(theme.id)"
              >
                <span class="theme-preview" :class="`theme-preview--${theme.id}`">
                  <span class="theme-preview__sidebar" />
                  <span class="theme-preview__body">
                    <i />
                    <i />
                    <i class="short" />
                  </span>
                  <span class="theme-preview__accent" />
                </span>
                <span class="theme-option__text">
                  <strong>{{ theme.name }}</strong>
                  <em>{{ theme.description }}</em>
                </span>
                <span v-if="settings.theme === theme.id" class="theme-option__check"><Check :size="12" /></span>
              </button>
            </div>
            <p class="footnote"><Info :size="12" />{{ settings.t('settings.darkOnly') }}</p>
          </div>
        </section>

        <section class="panel settings-card">
          <div class="panel-head">
            <h3 class="panel-title"><Languages :size="15" />{{ settings.t('settings.language') }}</h3>
            <span class="badge">{{ currentLanguage }}</span>
          </div>
          <div class="panel-body">
            <p class="card-hint">{{ settings.t('settings.languageHint') }}</p>
            <div class="language-grid">
              <button
                v-for="language in languages"
                :key="language.id"
                class="language-option"
                :class="{ 'language-option--active': settings.localePreference === language.id }"
                @click="settings.setLocale(language.id)"
              >
                <span class="language-option__glyph">{{ language.glyph }}</span>
                <span class="language-option__text">
                  <strong>{{ language.name }}</strong>
                  <em>{{ language.detail }}</em>
                </span>
                <span v-if="settings.localePreference === language.id" class="language-option__dot" />
              </button>
            </div>
            <p class="footnote">{{ settings.t('settings.languageCurrent', { language: currentLanguage }) }}</p>
          </div>
        </section>

        <section id="resources" class="panel settings-card">
          <div class="panel-head">
            <h3 class="panel-title"><FileCog :size="15" />{{ settings.t('settings.resources') }}</h3>
            <span v-if="resources.installed" class="badge badge--accent">
              <Check :size="10" />
              {{ settings.t('common.installed') }}
            </span>
            <span v-else class="badge badge--warning">{{ settings.t('settings.resourceStatusMissing') }}</span>
          </div>
          <div class="panel-body">
            <p class="card-hint">{{ settings.t('settings.resourcesHint') }}</p>

            <div class="resource-summary">
              <div class="resource-summary__row">
                <span>{{ settings.t('settings.currentClient') }}</span>
                <strong class="mono">{{ clientVersion }}</strong>
              </div>
              <div class="resource-summary__row">
                <span>{{ settings.t('settings.commit') }}</span>
                <strong class="mono commit">{{ shortCommit }}</strong>
              </div>
              <div class="resource-summary__row">
                <span>{{ settings.t('settings.resourcesPath') }}</span>
                <strong class="mono path" :title="resources.status?.path">{{ shortPath(resources.status?.path ?? platform.paths.pm3Dir, 48) }}</strong>
              </div>
              <div v-if="resources.status?.installed" class="resource-summary__row">
                <span>{{ settings.t('settings.installedAt') }}</span>
                <strong>{{ formatDate(resources.status.installedAt, settings.resolvedLocale) }}</strong>
              </div>
            </div>

            <div v-if="resources.release" class="release-card">
              <div class="release-card__top">
                <div>
                  <span class="release-card__eyebrow">{{ settings.t('settings.latestRelease') }}</span>
                  <strong class="release-card__tag">{{ settings.t('settings.releaseTag', { tag: resources.release.tag }) }}</strong>
                </div>
                <button class="icon-btn icon-btn--bordered" :title="settings.t('settings.releaseNotes')" @click="openReleases">
                  <ArrowUpRight :size="15" />
                </button>
              </div>
              <div class="release-card__meta">
                <span class="mono">{{ resources.release.assetName }}</span>
                <span>·</span>
                <span>{{ formatBytes(resources.release.assetSize) }}</span>
              </div>
              <div class="release-card__sha mono">
                SHA-256 · {{ resources.release.checksum?.slice(0, 24) ?? '—' }}…
              </div>
            </div>

            <div v-else-if="resources.checking" class="release-card release-card--loading">
              <span class="spinner spinner--sm" />
              <span>{{ settings.t('settings.checking') }}</span>
            </div>

            <div v-else class="release-card release-card--empty">
              <Info :size="16" />
              <div>
                <strong>{{ settings.t('settings.noRelease') }}</strong>
                <p>{{ settings.t('settings.noReleaseBody') }}</p>
              </div>
            </div>

            <div v-if="resources.downloading" class="download-progress">
              <div class="download-progress__head">
                <span>{{ resourceStage }}</span>
                <span class="mono">{{ resources.progressPercent }}%</span>
              </div>
              <div class="progress-track">
                <span class="progress-fill" :style="{ width: `${resources.progressPercent}%` }" />
              </div>
            </div>

            <p v-if="resources.error" class="error-text">
              <CircleAlert :size="13" />
              {{ resources.error }}
            </p>

            <div class="resource-actions">
              <button
                class="btn btn--primary"
                :disabled="resources.downloading || resources.checking || !resources.release"
                @click="installTheme"
              >
                <span v-if="resources.downloading" class="spinner spinner--sm" />
                <Download v-else :size="15" />
                {{
                  resources.downloading
                    ? settings.t('settings.downloading')
                    : resources.installed
                      ? settings.t('settings.redownloadResources')
                      : settings.t('settings.downloadResources')
                }}
              </button>
              <button class="btn btn--outline" :disabled="resources.checking || resources.downloading" @click="checkRelease">
                <RefreshCw :size="14" :class="{ spinning: resources.checking }" />
                {{ resources.checking ? settings.t('settings.checking') : settings.t('settings.checkRelease') }}
              </button>
              <button class="btn btn--ghost" @click="openReleases">
                <ExternalLink :size="14" />
                {{ settings.t('settings.releaseNotes') }}
              </button>
            </div>
          </div>
        </section>

        <section class="panel settings-card">
          <div class="panel-head">
            <h3 class="panel-title"><Settings2 :size="15" />{{ settings.t('settings.connection') }}</h3>
          </div>
          <div class="panel-body">
            <p class="card-hint">{{ settings.t('settings.connectionHint') }}</p>
            <div class="preference-row">
              <div>
                <strong>{{ settings.t('settings.preferredTransport') }}</strong>
                <span>{{ settings.t('settings.connectionHint') }}</span>
              </div>
              <div class="segmented">
                <button
                  class="segmented__item"
                  :class="{ 'segmented__item--active': settings.preferredTransport === 'usb' }"
                  :disabled="!platform.info.capabilities.usbSerial"
                  @click="settings.setPreferredTransport('usb')"
                >
                  <Usb :size="14" />USB
                </button>
                <button
                  class="segmented__item"
                  :class="{ 'segmented__item--active': settings.preferredTransport === 'ble' }"
                  :disabled="!platform.info.capabilities.ble"
                  @click="settings.setPreferredTransport('ble')"
                >
                  <Bluetooth :size="14" />BLE
                </button>
                <button
                  class="segmented__item"
                  :class="{ 'segmented__item--active': settings.preferredTransport === 'spp' }"
                  :disabled="!platform.info.capabilities.spp"
                  @click="settings.setPreferredTransport('spp')"
                >
                  <Radio :size="14" />SPP
                </button>
              </div>
            </div>
            <div class="preference-row">
              <div>
                <strong>{{ settings.t('settings.autoReconnect') }}</strong>
                <span>{{ settings.t('settings.autoReconnectHint') }}</span>
              </div>
              <button
                class="switch"
                :class="{ 'switch--on': settings.autoReconnect }"
                role="switch"
                :aria-checked="settings.autoReconnect"
                @click="settings.setAutoReconnect(!settings.autoReconnect)"
              />
            </div>
          </div>
        </section>

        <section class="panel settings-card">
          <div class="panel-head">
            <h3 class="panel-title"><Bluetooth :size="15" />{{ settings.t('settings.bleService') }}</h3>
          </div>
          <div class="panel-body">
            <p class="card-hint">{{ settings.t('settings.bleServiceHint') }}</p>
            <p class="preset-note">{{ settings.t('settings.blePreset') }}</p>
            <div class="uuid-fields">
              <label class="field">
                <span class="field-label">{{ settings.t('settings.bleServiceUuid') }}</span>
                <input v-model="bleDraft.service" class="input input--mono" spellcheck="false" />
              </label>
              <label class="field">
                <span class="field-label">{{ settings.t('settings.bleRxUuid') }}</span>
                <input v-model="bleDraft.rx" class="input input--mono" spellcheck="false" />
              </label>
              <label class="field">
                <span class="field-label">{{ settings.t('settings.bleTxUuid') }}</span>
                <input v-model="bleDraft.tx" class="input input--mono" spellcheck="false" />
              </label>
            </div>
            <div class="inline-actions">
              <button class="btn btn--sm" @click="saveBleDefaults">
                <Save :size="13" />
                {{ settings.t('common.save') }}
              </button>
              <button class="btn btn--ghost btn--sm" @click="restoreBleDefaults">
                <RotateCcw :size="13" />
                {{ settings.t('settings.restoreBleDefaults') }}
              </button>
            </div>
          </div>
        </section>
      </div>

      <div class="settings-col settings-col--side">
        <section class="panel settings-card about-card">
          <div class="panel-head">
            <h3 class="panel-title"><Sparkles :size="15" />{{ settings.t('settings.about') }}</h3>
          </div>
          <div class="panel-body">
            <div class="about-brand">
              <span class="about-brand__icon"><Sparkles :size="20" /></span>
              <div>
                <strong>Proxmark Studio</strong>
                <span>{{ settings.t('shell.tagline') }}</span>
              </div>
            </div>
            <div class="about-rows">
              <div><span>{{ settings.t('settings.appVersion') }}</span><strong class="mono">{{ platform.info.version }}</strong></div>
              <div><span>{{ settings.t('settings.pluginVersion') }}</span><strong class="mono">0.2.1</strong></div>
              <div><span>{{ settings.t('settings.platform') }}</span><strong>{{ platform.osLabel }} · {{ platform.info.arch }}</strong></div>
              <div><span>{{ settings.t('settings.build') }}</span><strong class="mono">Tauri {{ platform.info.tauriVersion }}</strong></div>
              <div><span>{{ settings.t('settings.license') }}</span><strong class="mono">{{ settings.t('settings.licenseValue') }}</strong></div>
            </div>
            <div class="about-links">
              <button class="about-link" @click="openSource">
                <Github :size="14" />{{ settings.t('settings.viewSource') }}<ArrowUpRight :size="13" />
              </button>
              <button class="about-link" @click="openClientSource">
                <Github :size="14" />Proxmark3<ArrowUpRight :size="13" />
              </button>
            </div>
          </div>
        </section>

        <section class="panel settings-card">
          <div class="panel-head">
            <h3 class="panel-title"><HardDrive :size="15" />{{ settings.t('home.workdir') }}</h3>
          </div>
          <div class="panel-body">
            <div class="path-box mono" :title="platform.paths.pm3Dir">{{ shortPath(platform.paths.pm3Dir || '—', 42) }}</div>
            <p class="footnote">{{ settings.t('settings.workdirHint') }}</p>
            <button class="btn btn--outline btn--sm btn--block" @click="openWorkspace">
              <HardDrive :size="14" />
              {{ settings.t('settings.openWorkdir') }}
            </button>
          </div>
        </section>

        <section class="panel settings-card">
          <div class="panel-head">
            <h3 class="panel-title"><Info :size="15" />{{ settings.t('settings.diagnostics') }}</h3>
          </div>
          <div class="panel-body">
            <p class="card-hint">{{ settings.t('settings.diagnosticsHint') }}</p>
            <button class="btn btn--outline btn--sm btn--block" :disabled="diagnosticsBusy" @click="copyDiagnostics">
              <span v-if="diagnosticsBusy" class="spinner spinner--sm" />
              <Copy v-else :size="14" />
              {{ settings.t('settings.copyDiagnostics') }}
            </button>
          </div>
        </section>

        <section class="panel settings-card gpl-card">
          <div class="panel-head">
            <h3 class="panel-title"><ShieldAlert :size="15" />{{ settings.t('settings.gplTitle') }}</h3>
          </div>
          <div class="panel-body">
            <p>{{ settings.t('settings.gplBody') }}</p>
            <button class="license-link" @click="openClientSource">
              {{ settings.t('settings.licenses') }}
              <ArrowUpRight :size="13" />
            </button>
          </div>
        </section>
      </div>
    </div>

    <Modal :open="resetOpen" :title="settings.t('settings.reset')" @close="resetOpen = false">
      <p class="reset-copy">{{ settings.t('settings.resetHint') }}</p>
      <template #foot>
        <button class="btn btn--outline" @click="resetOpen = false">{{ settings.t('common.cancel') }}</button>
        <button class="btn btn--danger" @click="confirmReset">
          <RotateCcw :size="14" />
          {{ settings.t('settings.reset') }}
        </button>
      </template>
    </Modal>
  </div>
</template>

<style scoped>
.settings-page {
  padding-bottom: 72px;
}

.autosave {
  font-size: 9.5px;
  color: var(--faint);
}

.spinning {
  animation: spin 0.9s linear infinite;
}

.settings-grid {
  display: grid;
  grid-template-columns: minmax(0, 1.5fr) minmax(300px, 0.82fr);
  gap: 16px;
  align-items: start;
}

.settings-col {
  display: flex;
  flex-direction: column;
  gap: 16px;
  min-width: 0;
}

.settings-card .panel-body {
  padding-top: 14px;
}

.card-hint {
  margin-bottom: 14px;
  font-size: 11.5px;
  line-height: 1.6;
  color: var(--muted);
}

.theme-grid {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 9px;
}

.theme-option {
  position: relative;
  display: flex;
  flex-direction: column;
  gap: 9px;
  padding: 8px;
  border: 1px solid var(--border);
  border-radius: 11px;
  background: rgba(255, 255, 255, 0.012);
  text-align: left;
  transition:
    border-color var(--transition),
    background var(--transition),
    transform var(--transition);
}

.theme-option:hover {
  border-color: var(--border-strong);
  transform: translateY(-1px);
}

.theme-option--active {
  border-color: var(--border-accent);
  background: var(--accent-faint);
}

.theme-preview {
  position: relative;
  display: flex;
  height: 58px;
  border-radius: 7px;
  overflow: hidden;
  border: 1px solid rgba(255, 255, 255, 0.08);
}

.theme-preview--midnight {
  background: #070c16;
}
.theme-preview--carbon {
  background: #100d0a;
}
.theme-preview--nebula {
  background: #0b0918;
}

.theme-preview__sidebar {
  width: 23%;
  border-right: 1px solid rgba(255, 255, 255, 0.07);
}

.theme-preview--midnight .theme-preview__sidebar {
  background: #0d1624;
}
.theme-preview--carbon .theme-preview__sidebar {
  background: #191511;
}
.theme-preview--nebula .theme-preview__sidebar {
  background: #15112b;
}

.theme-preview__body {
  display: flex;
  flex-direction: column;
  gap: 5px;
  flex: 1;
  padding: 10px 8px;
}

.theme-preview__body i {
  width: 78%;
  height: 3px;
  border-radius: 3px;
  background: rgba(255, 255, 255, 0.17);
}

.theme-preview__body i:nth-child(2) {
  width: 54%;
  height: 8px;
  border-radius: 4px;
  opacity: 0.5;
}

.theme-preview__body i.short {
  width: 34%;
}

.theme-preview__accent {
  position: absolute;
  right: 8px;
  bottom: 8px;
  width: 7px;
  height: 7px;
  border-radius: 50%;
}

.theme-preview--midnight .theme-preview__accent {
  background: #4cf0c8;
  box-shadow: 0 0 7px #4cf0c8;
}
.theme-preview--carbon .theme-preview__accent {
  background: #ffb454;
  box-shadow: 0 0 7px #ffb454;
}
.theme-preview--nebula .theme-preview__accent {
  background: #b18cff;
  box-shadow: 0 0 7px #b18cff;
}

.theme-option__text {
  display: flex;
  flex-direction: column;
  gap: 1px;
  min-width: 0;
}

.theme-option__text strong {
  font-size: 11px;
  font-weight: 600;
  color: var(--text-soft);
}

.theme-option__text em {
  font-style: normal;
  font-size: 9px;
  line-height: 1.35;
  color: var(--faint);
}

.theme-option__check {
  position: absolute;
  top: 13px;
  right: 13px;
  display: grid;
  place-items: center;
  width: 18px;
  height: 18px;
  border-radius: 50%;
  background: var(--accent);
  color: var(--accent-contrast);
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.4);
}

.footnote {
  display: flex;
  align-items: center;
  gap: 5px;
  margin-top: 11px;
  font-size: 9.5px;
  color: var(--faint);
}

.language-grid {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 8px;
}

.language-option {
  position: relative;
  display: flex;
  align-items: center;
  gap: 9px;
  padding: 10px;
  border-radius: 10px;
  border: 1px solid var(--border);
  background: rgba(255, 255, 255, 0.012);
  text-align: left;
  transition:
    border-color var(--transition),
    background var(--transition);
}

.language-option:hover {
  border-color: var(--border-strong);
}

.language-option--active {
  border-color: var(--border-accent);
  background: var(--accent-faint);
}

.language-option__glyph {
  display: grid;
  place-items: center;
  width: 30px;
  height: 30px;
  border-radius: 8px;
  background: var(--panel-strong);
  color: var(--muted);
  font-family: var(--font-mono);
  font-size: 9px;
  font-weight: 600;
  flex: none;
}

.language-option--active .language-option__glyph {
  background: var(--accent-dim);
  color: var(--accent);
}

.language-option__text {
  display: flex;
  flex-direction: column;
  min-width: 0;
}

.language-option__text strong {
  font-size: 10.5px;
  font-weight: 600;
  color: var(--text-soft);
  white-space: nowrap;
}

.language-option__text em {
  font-style: normal;
  font-size: 8.5px;
  color: var(--faint);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.language-option__dot {
  position: absolute;
  top: 9px;
  right: 9px;
  width: 5px;
  height: 5px;
  border-radius: 50%;
  background: var(--accent);
}

.resource-summary {
  border: 1px solid var(--border);
  border-radius: 10px;
  padding: 4px 12px;
  background: var(--panel-inset);
}

.resource-summary__row {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 16px;
  padding: 8px 0;
  border-bottom: 1px dashed var(--border);
}

.resource-summary__row:last-child {
  border-bottom: 0;
}

.resource-summary__row > span {
  font-size: 10px;
  color: var(--faint);
  flex: none;
}

.resource-summary__row > strong {
  font-size: 10px;
  font-weight: 550;
  color: var(--text-soft);
  text-align: right;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.resource-summary__row > strong.commit {
  color: var(--accent);
}

.resource-summary__row > strong.path {
  font-size: 8.5px;
  color: var(--muted);
}

.release-card {
  margin-top: 11px;
  padding: 11px 12px;
  border: 1px solid var(--border-accent);
  border-radius: 10px;
  background: linear-gradient(135deg, var(--accent-faint), rgba(0, 0, 0, 0.1));
}

.release-card__top {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 8px;
}

.release-card__eyebrow {
  display: block;
  font-family: var(--font-mono);
  font-size: 7.5px;
  letter-spacing: 0.1em;
  text-transform: uppercase;
  color: var(--faint);
}

.release-card__tag {
  display: block;
  margin-top: 2px;
  font-size: 12px;
  font-weight: 620;
  color: var(--text);
}

.release-card__meta {
  display: flex;
  align-items: center;
  gap: 5px;
  margin-top: 7px;
  font-size: 8.5px;
  color: var(--muted);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.release-card__sha {
  margin-top: 4px;
  font-size: 7.5px;
  color: var(--faint);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.release-card--loading {
  display: flex;
  align-items: center;
  gap: 8px;
  border-color: var(--border);
  background: var(--panel-inset);
  font-size: 10px;
  color: var(--muted);
}

.release-card--empty {
  display: flex;
  align-items: flex-start;
  gap: 9px;
  border-color: rgba(255, 196, 107, 0.25);
  background: var(--warning-dim);
  color: var(--warning);
}

.release-card--empty strong {
  display: block;
  font-size: 10.5px;
  color: var(--warning);
}

.release-card--empty p {
  margin-top: 3px;
  font-size: 9.5px;
  line-height: 1.45;
  color: var(--muted);
}

.download-progress {
  margin-top: 10px;
}

.download-progress__head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  font-size: 9.5px;
  color: var(--muted);
}

.download-progress__head .mono {
  font-size: 9px;
  color: var(--accent);
}

.progress-track {
  height: 5px;
  margin-top: 6px;
  border-radius: 99px;
  background: var(--panel-strong);
  overflow: hidden;
}

.progress-fill {
  display: block;
  height: 100%;
  border-radius: inherit;
  background: linear-gradient(90deg, var(--accent), var(--accent-strong));
  transition: width 300ms var(--ease-out);
  box-shadow: 0 0 10px var(--accent);
}

.error-text {
  display: flex;
  align-items: flex-start;
  gap: 6px;
  margin-top: 9px;
  font-size: 9.5px;
  line-height: 1.5;
  color: var(--danger);
}

.resource-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  margin-top: 13px;
}

.preference-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 18px;
  padding: 11px 0;
  border-bottom: 1px dashed var(--border);
}

.preference-row:last-child {
  border-bottom: 0;
  padding-bottom: 0;
}

.preference-row > div:first-child {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}

.preference-row strong {
  font-size: 11px;
  font-weight: 600;
  color: var(--text-soft);
}

.preference-row span {
  font-size: 9.5px;
  line-height: 1.45;
  color: var(--faint);
}

.segmented__item:disabled {
  opacity: 0.35;
  cursor: not-allowed;
}

.preset-note {
  margin: -4px 0 13px;
  padding: 8px 10px;
  border-radius: 8px;
  border: 1px dashed var(--border);
  background: var(--panel-inset);
  font-size: 9px;
  line-height: 1.5;
  color: var(--faint);
}

.uuid-fields {
  display: flex;
  flex-direction: column;
  gap: 9px;
}

.uuid-fields .input {
  height: 36px;
  font-size: 10px;
}

.inline-actions {
  display: flex;
  gap: 6px;
  margin-top: 12px;
}

.about-brand {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-bottom: 13px;
}

.about-brand__icon {
  display: grid;
  place-items: center;
  width: 40px;
  height: 40px;
  border-radius: 11px;
  background: linear-gradient(135deg, var(--accent-dim), var(--secondary-dim));
  color: var(--accent);
}

.about-brand strong {
  display: block;
  font-size: 13px;
  font-weight: 650;
  color: var(--text);
}

.about-brand span {
  font-family: var(--font-mono);
  font-size: 7.5px;
  letter-spacing: 0.12em;
  color: var(--faint);
}

.about-rows {
  display: flex;
  flex-direction: column;
}

.about-rows > div {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 12px;
  padding: 7px 0;
  border-bottom: 1px dashed var(--border);
}

.about-rows > div:last-child {
  border-bottom: 0;
}

.about-rows span {
  font-size: 9.5px;
  color: var(--faint);
}

.about-rows strong {
  font-size: 9.5px;
  font-weight: 550;
  color: var(--text-soft);
  text-align: right;
}

.about-links {
  display: flex;
  gap: 6px;
  margin-top: 10px;
}

.about-link {
  display: flex;
  flex: 1;
  align-items: center;
  justify-content: center;
  gap: 5px;
  height: 32px;
  border: 1px solid var(--border);
  border-radius: 8px;
  color: var(--muted);
  font-size: 9px;
  font-weight: 550;
}

.about-link:hover {
  color: var(--text);
  border-color: var(--border-strong);
}

.path-box {
  padding: 9px 10px;
  border: 1px solid var(--border);
  border-radius: 8px;
  background: var(--panel-inset);
  font-size: 8.5px;
  line-height: 1.5;
  color: var(--text-soft);
  word-break: break-all;
}

.gpl-card .panel-body > p {
  font-size: 10px;
  line-height: 1.65;
  color: var(--muted);
}

.license-link {
  display: flex;
  align-items: center;
  gap: 5px;
  margin-top: 10px;
  padding: 0;
  color: var(--accent);
  font-size: 9.5px;
  font-weight: 600;
}

.reset-copy {
  font-size: 12px;
  line-height: 1.6;
  color: var(--muted);
}

@media (max-width: 980px) {
  .settings-grid {
    grid-template-columns: 1fr;
  }

  .settings-col--side {
    display: grid;
    grid-template-columns: 1fr 1fr;
    align-items: start;
  }
}

@media (max-width: 640px) {
  .settings-page {
    padding-bottom: 92px;
  }

  .autosave {
    display: none;
  }

  .head-actions {
    justify-content: flex-start;
  }

  .theme-grid {
    grid-template-columns: 1fr;
  }

  .theme-option {
    flex-direction: row;
    align-items: center;
    gap: 11px;
  }

  .theme-preview {
    width: 86px;
    height: 50px;
    flex: none;
  }

  .theme-option__text em {
    display: block;
  }

  .language-grid {
    grid-template-columns: 1fr;
  }

  .language-option {
    padding: 9px;
  }

  .resource-actions {
    flex-direction: column;
  }

  .resource-actions .btn {
    width: 100%;
  }

  .preference-row {
    flex-direction: column;
    align-items: stretch;
    gap: 9px;
  }

  .preference-row .switch {
    align-self: flex-end;
  }

  .settings-col--side {
    display: flex;
  }

  .about-links {
    flex-direction: column;
  }
}
</style>
