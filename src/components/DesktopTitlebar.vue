<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { useRoute } from 'vue-router'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { Copy, Minus, Square, X } from 'lucide-vue-next'
import BrandMark from './BrandMark.vue'
import { useSettingsStore } from '@/stores/settings'
import { useConnectionStore } from '@/stores/connection'

const route = useRoute()
const settings = useSettingsStore()
const connection = useConnectionStore()
const appWindow = getCurrentWindow()
const maximized = ref(false)
let unlisten: (() => void) | null = null

const pageName = computed(() => {
  if (route.name === 'terminal') return settings.t('nav.terminal')
  if (route.name === 'files') return settings.t('nav.files')
  if (route.name === 'settings') return settings.t('nav.settings')
  return settings.t('nav.home')
})

const stateLabel = computed(() => {
  if (connection.isConnected) return settings.t('shell.statusOnline')
  if (connection.status === 'connecting') return settings.t('shell.statusBusy')
  if (connection.status === 'error') return settings.t('home.statusError')
  return settings.t('shell.statusIdle')
})

const statusDotClass = computed(() => {
  if (connection.isConnected) return 'status-dot--online'
  if (connection.status === 'connecting') return 'status-dot--busy'
  if (connection.status === 'error') return 'status-dot--error'
  return ''
})

async function refreshMaximized() {
  try {
    maximized.value = await appWindow.isMaximized()
  } catch {
    maximized.value = false
  }
}

async function minimize() {
  await appWindow.minimize()
}

async function toggleMaximize() {
  await appWindow.toggleMaximize()
  await refreshMaximized()
}

async function close() {
  await appWindow.close()
}



onMounted(async () => {
  await refreshMaximized()
  unlisten = await appWindow.onResized(() => {
    void refreshMaximized()
  })
})

onUnmounted(() => {
  unlisten?.()
})
</script>

<template>
  <header class="titlebar" data-tauri-drag-region @dblclick="toggleMaximize">
    <div class="titlebar__identity" data-tauri-drag-region>
      <BrandMark :size="18" />
      <span class="titlebar__name" data-tauri-drag-region>PROXMARK <em>STUDIO</em></span>
      <span class="titlebar__slash" data-tauri-drag-region>/</span>
      <span class="titlebar__section" data-tauri-drag-region>{{ pageName }}</span>
    </div>

    <div class="titlebar__drag" data-tauri-drag-region />

    <div class="titlebar__status" data-tauri-drag-region>
      <span class="status-dot" :class="statusDotClass" />
      <span>{{ stateLabel }}</span>
      <span v-if="connection.isConnected" class="titlebar__port mono">{{ connection.portLabel }}</span>
    </div>

    <div class="window-controls" @dblclick.stop>
      <button class="window-control" title="Minimize" aria-label="Minimize" @click.stop="minimize">
        <Minus :size="14" />
      </button>
      <button
        class="window-control"
        :title="maximized ? 'Restore' : 'Maximize'"
        :aria-label="maximized ? 'Restore' : 'Maximize'"
        @click.stop="toggleMaximize"
      >
        <Copy v-if="maximized" :size="12" />
        <Square v-else :size="11" />
      </button>
      <button class="window-control window-control--close" title="Close" aria-label="Close" @click.stop="close">
        <X :size="15" />
      </button>
    </div>
  </header>
</template>

<style scoped>
.titlebar {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  z-index: 90;
  display: flex;
  align-items: center;
  height: 38px;
  padding-left: 12px;
  background:
    linear-gradient(180deg, rgba(10, 16, 28, 0.96), rgba(6, 10, 18, 0.92)),
    var(--bg-deep);
  border-bottom: 1px solid var(--border);
  backdrop-filter: blur(20px) saturate(1.2);
  -webkit-backdrop-filter: blur(20px) saturate(1.2);
  user-select: none;
  -webkit-user-select: none;
}

[data-theme='carbon'] .titlebar {
  background: linear-gradient(180deg, rgba(21, 18, 15, 0.96), rgba(10, 9, 8, 0.94));
}
[data-theme='nebula'] .titlebar {
  background: linear-gradient(180deg, rgba(18, 14, 36, 0.96), rgba(8, 6, 18, 0.94));
}

.titlebar__identity {
  display: flex;
  align-items: center;
  gap: 7px;
  flex: none;
}

.titlebar__name {
  font-family: var(--font-mono);
  font-size: 8.5px;
  font-weight: 700;
  letter-spacing: 0.14em;
  color: var(--text-soft);
}

.titlebar__name em {
  font-style: normal;
  color: var(--accent);
}

.titlebar__slash {
  margin-left: 3px;
  font-size: 10px;
  color: var(--faint);
}

.titlebar__section {
  font-size: 10px;
  font-weight: 500;
  color: var(--muted);
}

.titlebar__drag {
  flex: 1;
  height: 100%;
}

.titlebar__status {
  display: flex;
  align-items: center;
  gap: 6px;
  height: 100%;
  padding: 0 14px;
  font-family: var(--font-mono);
  font-size: 8px;
  letter-spacing: 0.06em;
  color: var(--faint);
  white-space: nowrap;
}

.titlebar__port {
  max-width: 220px;
  overflow: hidden;
  text-overflow: ellipsis;
  color: var(--muted);
}

.window-controls {
  display: flex;
  align-items: stretch;
  height: 100%;
  flex: none;
}

.window-control {
  display: grid;
  place-items: center;
  width: 46px;
  height: 100%;
  border-radius: 0;
  color: var(--muted);
  transition:
    background var(--transition),
    color var(--transition);
}

.window-control:hover {
  color: var(--text);
  background: rgba(255, 255, 255, 0.06);
}

.window-control--close:hover {
  color: #ffffff;
  background: #c42b1c;
}

.window-control:focus-visible {
  outline-offset: -3px;
}

@media (max-width: 860px) {
  .titlebar {
    display: none;
  }
}
</style>
