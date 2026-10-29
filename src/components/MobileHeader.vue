<script setup lang="ts">
import { useRoute } from 'vue-router'
import { ChevronRight, Terminal } from 'lucide-vue-next'
import BrandMark from './BrandMark.vue'
import { useConnectionStore } from '@/stores/connection'
import { useSettingsStore } from '@/stores/settings'
import { usePlatformStore } from '@/stores/platform'

const route = useRoute()
const connection = useConnectionStore()
const settings = useSettingsStore()
const platform = usePlatformStore()

const pageName = () => {
  if (route.name === 'files') return settings.t('nav.files')
  if (route.name === 'settings') return settings.t('nav.settings')
  return settings.t('nav.home')
}

const statusLabel = () => {
  if (connection.isConnected) return settings.t('common.online')
  if (connection.status === 'connecting') return settings.t('common.connecting')
  return settings.t('common.offline')
}
</script>

<template>
  <header class="mobile-header">
    <RouterLink to="/" class="mobile-brand">
      <BrandMark :size="27" />
      <div class="mobile-brand__text">
        <span>PROXMARK</span>
        <em>{{ pageName() }}</em>
      </div>
    </RouterLink>

    <div class="mobile-header__actions">
      <RouterLink
        v-if="connection.isConnected"
        to="/terminal"
        class="online-chip"
        :title="settings.t('nav.terminal')"
      >
        <span class="status-dot status-dot--online" />
        <span>{{ statusLabel() }}</span>
        <ChevronRight :size="13" />
      </RouterLink>
      <span v-else class="online-chip online-chip--idle">
        <span class="status-dot" />
        <span>{{ statusLabel() }}</span>
      </span>
      <button
        v-if="connection.isConnected"
        class="terminal-shortcut"
        :aria-label="settings.t('nav.terminal')"
        @click="$router.push('/terminal')"
      >
        <Terminal :size="17" />
      </button>
    </div>
    <span class="mobile-header__platform">{{ platform.osLabel }}</span>
  </header>
</template>

<style scoped>
.mobile-header {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  z-index: 40;
  display: flex;
  align-items: center;
  gap: 10px;
  height: calc(var(--header-height) + env(safe-area-inset-top));
  padding: env(safe-area-inset-top) 16px 0;
  background: rgba(5, 8, 16, 0.76);
  border-bottom: 1px solid var(--border);
  backdrop-filter: blur(22px) saturate(1.2);
  -webkit-backdrop-filter: blur(22px) saturate(1.2);
}

[data-theme='carbon'] .mobile-header {
  background: rgba(11, 10, 9, 0.8);
}
[data-theme='nebula'] .mobile-header {
  background: rgba(8, 7, 20, 0.8);
}

.mobile-brand {
  display: flex;
  align-items: center;
  gap: 9px;
  min-width: 0;
  color: var(--text);
}

.mobile-brand__text {
  display: flex;
  flex-direction: column;
  line-height: 1.05;
}

.mobile-brand__text span {
  font-size: 11px;
  font-weight: 760;
  letter-spacing: 0.14em;
}

.mobile-brand__text em {
  margin-top: 3px;
  font-style: normal;
  font-size: 10px;
  font-weight: 500;
  color: var(--muted);
}

.mobile-header__actions {
  display: flex;
  align-items: center;
  gap: 7px;
  margin-left: auto;
}

.online-chip {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  height: 29px;
  padding: 0 9px;
  border-radius: 99px;
  border: 1px solid var(--border-accent);
  background: var(--accent-dim);
  color: var(--accent);
  font-size: 10px;
  font-weight: 600;
}

.online-chip--idle {
  border-color: var(--border);
  background: var(--panel-inset);
  color: var(--muted);
}

.terminal-shortcut {
  display: grid;
  place-items: center;
  width: 33px;
  height: 33px;
  border-radius: 10px;
  border: 1px solid var(--border);
  background: var(--panel-inset);
  color: var(--text-soft);
}

.mobile-header__platform {
  display: none;
}
</style>
