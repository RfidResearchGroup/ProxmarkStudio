<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { useRoute } from 'vue-router'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { Copy, Minus, Square, X } from 'lucide-vue-next'
import BrandMark from './BrandMark.vue'
import { useSettingsStore } from '@/stores/settings'

const route = useRoute()
const settings = useSettingsStore()
const appWindow = getCurrentWindow()
const maximized = ref(false)
let unlisten: (() => void) | null = null

const pageName = computed(() => {
  if (route.name === 'terminal') return settings.t('nav.terminal')
  if (route.name === 'files') return settings.t('nav.files')
  if (route.name === 'settings') return settings.t('nav.settings')
  return settings.t('nav.home')
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

    <div class="window-controls" @dblclick.stop>
      <button class="window-control" :title="settings.t('window.minimize')" :aria-label="settings.t('window.minimize')" @click.stop="minimize">
        <Minus :size="14" />
      </button>
      <button
        class="window-control"
        :title="settings.t(maximized ? 'window.restore' : 'window.maximize')"
        :aria-label="settings.t(maximized ? 'window.restore' : 'window.maximize')"
        @click.stop="toggleMaximize"
      >
        <Copy v-if="maximized" :size="12" />
        <Square v-else :size="11" />
      </button>
      <button class="window-control window-control--close" :title="settings.t('window.close')" :aria-label="settings.t('window.close')" @click.stop="close">
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
  font-size: 11px;
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
  font-size: 11px;
  color: var(--faint);
}

.titlebar__section {
  font-size: 11px;
  font-weight: 500;
  color: var(--muted);
}

.titlebar__drag {
  flex: 1;
  height: 100%;
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
