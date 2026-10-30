<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { useRoute } from 'vue-router'
import {
  Activity,
  Check,
  ChevronRight,
  FolderOpen,
  Github,
  Home,
  Languages,
  Palette,
  Settings,
  Terminal,
  Usb,
} from 'lucide-vue-next'
import BrandMark from './BrandMark.vue'
import { useSettingsStore } from '@/stores/settings'
import { useConnectionStore } from '@/stores/connection'
import { usePlatformStore } from '@/stores/platform'
import { openExternal } from '@/services/backend'
import type { ThemeName } from '@/stores/settings'
import { APP_REPOSITORY } from '@/config/links'

const route = useRoute()
const settings = useSettingsStore()
const connection = useConnectionStore()
const platform = usePlatformStore()

const navItems = computed(() => [
  { name: 'home', to: '/', label: settings.t('nav.home'), icon: Home },
  { name: 'files', to: '/files', label: settings.t('nav.files'), icon: FolderOpen },
  { name: 'settings', to: '/settings', label: settings.t('nav.settings'), icon: Settings },
])

const statusLabel = computed(() => {
  if (connection.isConnected) return settings.t('shell.statusOnline')
  if (connection.status === 'connecting') return settings.t('shell.statusBusy')
  if (connection.status === 'error') return settings.t('home.statusError')
  return settings.t('shell.statusIdle')
})

const statusClass = computed(() => {
  if (connection.isConnected) return 'online'
  if (connection.status === 'connecting') return 'busy'
  if (connection.status === 'error') return 'error'
  return 'idle'
})

const version = computed(() => platform.info.version || '0.0.0-local')

const themes: ThemeName[] = ['abyss', 'midnight', 'carbon', 'nebula']

const languageOpen = ref(false)
const languageRoot = ref<HTMLElement | null>(null)
const languageOptions = computed(() => [
  { id: 'system' as const, label: settings.t('settings.followSystem') },
  { id: 'en' as const, label: settings.t('settings.english') },
  { id: 'zh' as const, label: settings.t('settings.chinese') },
])

function cycleTheme() {
  const index = themes.indexOf(settings.theme)
  settings.setTheme(themes[(index + 1) % themes.length])
}

function selectLocale(locale: 'system' | 'en' | 'zh') {
  settings.setLocale(locale)
  languageOpen.value = false
}

function onDocumentPointer(event: MouseEvent) {
  if (!languageOpen.value) return
  if (languageRoot.value && !languageRoot.value.contains(event.target as Node)) {
    languageOpen.value = false
  }
}

function onDocumentKey(event: KeyboardEvent) {
  if (event.key === 'Escape') languageOpen.value = false
}

onMounted(() => {
  document.addEventListener('pointerdown', onDocumentPointer)
  document.addEventListener('keydown', onDocumentKey)
})

onUnmounted(() => {
  document.removeEventListener('pointerdown', onDocumentPointer)
  document.removeEventListener('keydown', onDocumentKey)
})

function openRepository() {
  void openExternal(APP_REPOSITORY)
}
</script>

<template>
  <aside class="sidebar">
    <div class="sidebar__glow" aria-hidden="true" />
    <header class="brand">
      <BrandMark :size="38" />
      <div class="brand__text">
        <span class="brand__name">PROXMARK</span>
        <span class="brand__sub">STUDIO <em>{{ settings.t('shell.tagline') }}</em></span>
      </div>
    </header>

    <nav class="nav" :aria-label="settings.t('nav.main')">
      <span class="nav__label">{{ settings.t('nav.workspace') }}</span>
      <RouterLink
        v-for="item in navItems"
        :key="item.name"
        :to="item.to"
        class="nav__item"
        :class="{ 'nav__item--active': route.name === item.name }"
      >
        <span class="nav__icon">
          <component :is="item.icon" :size="17" :stroke-width="1.9" />
        </span>
        <span>{{ item.label }}</span>
        <span class="nav__marker" />
      </RouterLink>
    </nav>

    <div class="sidebar__spacer" />

    <RouterLink v-if="connection.isConnected" to="/terminal" class="session-card session-card--online">
      <div class="session-card__top">
        <span class="status-dot status-dot--online" />
        <span class="session-card__state">{{ statusLabel }}</span>
        <ChevronRight :size="15" class="session-card__chevron" />
      </div>
      <div class="session-card__port mono">{{ connection.portLabel }}</div>
      <div class="session-card__transport">
        <Terminal :size="12" />
        {{ settings.t('shell.openTerminal') }}
      </div>
    </RouterLink>

    <div v-else class="session-card">
      <div class="session-card__top">
        <span class="status-dot" :class="`status-dot--${statusClass}`" />
        <span class="session-card__state">{{ statusLabel }}</span>
        <Activity :size="14" class="session-card__chevron" />
      </div>
      <div class="session-card__port session-card__port--muted">{{ settings.t('shell.noSession') }}</div>
      <div class="session-card__transport">
        <Usb :size="12" />
        {{ platform.osLabel }} · {{ platform.info.arch }}
      </div>
    </div>

    <footer class="sidebar__foot">
      <div class="foot-actions">
        <div ref="languageRoot" class="language-wrap">
          <button
            class="icon-btn icon-btn--bordered"
            :title="settings.t('shell.language')"
            aria-haspopup="menu"
            :aria-expanded="languageOpen"
            @click="languageOpen = !languageOpen"
          >
            <Languages :size="16" />
          </button>
          <Transition name="fade">
            <div v-if="languageOpen" class="language-menu" role="menu">
              <button
                v-for="option in languageOptions"
                :key="option.id"
                class="language-menu__item"
                :class="{ 'language-menu__item--active': settings.localePreference === option.id }"
                role="menuitemradio"
                :aria-checked="settings.localePreference === option.id"
                @click="selectLocale(option.id)"
              >
                <span>{{ option.label }}</span>
                <Check v-if="settings.localePreference === option.id" :size="14" />
              </button>
            </div>
          </Transition>
        </div>
        <button class="icon-btn icon-btn--bordered" :title="settings.t('shell.theme')" @click="cycleTheme">
          <Palette :size="16" />
        </button>
        <button class="icon-btn icon-btn--bordered" title="GitHub" @click="openRepository">
          <Github :size="16" />
        </button>
      </div>
      <div class="foot-meta">
        <span class="mono">{{ settings.t('shell.version', { version }) }}</span>
        <span class="foot-dot" />
        <span>{{ settings.resolvedLocale === 'zh' ? '中文' : 'EN' }}</span>
      </div>
    </footer>
  </aside>
</template>

<style scoped>
.sidebar {
  position: sticky;
  top: 0;
  display: flex;
  flex-direction: column;
  width: var(--sidebar-width);
  height: 100vh;
  height: 100dvh;
  padding: 26px 18px 18px;
  background: linear-gradient(180deg, rgba(9, 15, 26, 0.86), rgba(5, 8, 15, 0.94));
  border-right: 1px solid var(--border);
  overflow: visible;
  flex: none;
  z-index: 10;
}

[data-theme='carbon'] .sidebar {
  background: linear-gradient(180deg, rgba(20, 17, 14, 0.9), rgba(8, 7, 6, 0.96));
}

[data-theme='nebula'] .sidebar {
  background: linear-gradient(180deg, rgba(16, 13, 33, 0.9), rgba(7, 5, 16, 0.96));
}

.sidebar__glow {
  position: absolute;
  top: -140px;
  left: -80px;
  width: 300px;
  height: 300px;
  border-radius: 50%;
  background: radial-gradient(circle, var(--accent-faint), transparent 68%);
  pointer-events: none;
}

.brand {
  position: relative;
  display: flex;
  align-items: center;
  gap: 11px;
  padding: 0 6px 26px;
}

.brand__text {
  display: flex;
  flex-direction: column;
  line-height: 1.1;
}

.brand__name {
  font-size: 14px;
  font-weight: 760;
  letter-spacing: 0.15em;
  color: var(--text);
}

.brand__sub {
  display: flex;
  align-items: center;
  gap: 5px;
  margin-top: 4px;
  font-family: var(--font-mono);
  font-size: 10px;
  font-weight: 600;
  letter-spacing: 0.08em;
  white-space: nowrap;
  color: var(--muted);
}

.brand__sub em {
  font-style: normal;
  color: var(--accent);
  opacity: 0.85;
}

.nav {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.nav__label {
  padding: 0 10px 9px;
  font-family: var(--font-mono);
  font-size: 11.5px;
  font-weight: 600;
  letter-spacing: 0.18em;
  text-transform: uppercase;
  color: var(--faint);
}

.nav__item {
  position: relative;
  display: flex;
  align-items: center;
  gap: 11px;
  height: 43px;
  padding: 0 11px;
  border-radius: 11px;
  border: 1px solid transparent;
  color: var(--muted);
  font-size: 13px;
  font-weight: 550;
  transition:
    color var(--transition),
    background var(--transition),
    border-color var(--transition);
}

.nav__item:hover {
  color: var(--text-soft);
  background: rgba(255, 255, 255, 0.025);
}

.nav__item--active {
  color: var(--text);
  background: linear-gradient(90deg, var(--accent-dim), rgba(255, 255, 255, 0.015));
  border-color: var(--border);
}

.nav__icon {
  display: grid;
  place-items: center;
  width: 26px;
  height: 26px;
  border-radius: 8px;
  color: var(--faint);
  transition: color var(--transition), background var(--transition);
}

.nav__item--active .nav__icon {
  color: var(--accent);
  background: var(--accent-faint);
}

.nav__item:hover .nav__icon {
  color: var(--text-soft);
}

.nav__marker {
  position: absolute;
  left: -19px;
  top: 50%;
  width: 3px;
  height: 20px;
  border-radius: 0 3px 3px 0;
  background: var(--accent);
  transform: translateY(-50%) scaleY(0);
  transition: transform var(--transition);
  box-shadow: 0 0 12px var(--accent);
}

.nav__item--active .nav__marker {
  transform: translateY(-50%) scaleY(1);
}

.sidebar__spacer {
  flex: 1;
  min-height: 26px;
}

.session-card {
  display: block;
  padding: 13px 13px 12px;
  border-radius: 13px;
  border: 1px solid var(--border);
  background: var(--panel-inset);
  transition:
    border-color var(--transition),
    background var(--transition),
    transform var(--transition);
}

.session-card--online {
  border-color: var(--border-accent);
  background: linear-gradient(145deg, var(--accent-faint), rgba(8, 13, 23, 0.5));
}

.session-card--online:hover {
  transform: translateY(-1px);
  border-color: var(--accent);
}

.session-card__top {
  display: flex;
  align-items: center;
  gap: 7px;
}

.session-card__state {
  flex: 1;
  font-size: 11.5px;
  font-weight: 600;
  color: var(--text-soft);
}

.session-card__chevron {
  color: var(--faint);
}

.session-card__port {
  margin-top: 7px;
  font-size: 11px;
  color: var(--accent);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.session-card__port--muted {
  color: var(--faint);
  font-family: var(--font-sans);
}

.session-card__transport {
  display: flex;
  align-items: center;
  gap: 5px;
  margin-top: 8px;
  padding-top: 8px;
  border-top: 1px dashed var(--border);
  font-size: 11px;
  color: var(--muted);
}

.sidebar__foot {
  margin-top: 14px;
  padding: 13px 3px 2px;
  border-top: 1px solid var(--border);
}

.foot-actions {
  display: flex;
  gap: 7px;
}

.foot-actions .icon-btn {
  width: 34px;
  height: 34px;
  flex: 1;
}

.language-wrap {
  position: relative;
  display: flex;
  flex: 1;
}

.language-wrap .icon-btn {
  width: 100%;
}

.language-menu {
  position: absolute;
  left: 0;
  bottom: calc(100% + 8px);
  z-index: 50;
  display: flex;
  flex-direction: column;
  gap: 2px;
  width: 188px;
  padding: 5px;
  border: 1px solid var(--border-strong);
  border-radius: 12px;
  background: var(--panel-solid);
  box-shadow: 0 18px 48px rgba(0, 0, 0, 0.48);
}

.language-menu__item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  height: 35px;
  padding: 0 10px;
  border-radius: 8px;
  color: var(--text-soft);
  font-size: 12.5px;
  text-align: left;
}

.language-menu__item:hover {
  background: var(--panel-hover);
  color: var(--text);
}

.language-menu__item--active {
  color: var(--accent);
  background: var(--accent-faint);
}

.foot-meta {
  display: flex;
  align-items: center;
  gap: 7px;
  margin-top: 12px;
  padding: 0 3px;
  font-size: 11.5px;
  color: var(--faint);
}

.foot-meta .mono {
  font-size: 11.5px;
}

.foot-dot {
  width: 2px;
  height: 2px;
  border-radius: 50%;
  background: var(--faint);
}
</style>
