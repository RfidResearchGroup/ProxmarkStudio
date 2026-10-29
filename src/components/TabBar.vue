<script setup lang="ts">
import { computed } from 'vue'
import { useRoute } from 'vue-router'
import { FolderOpen, Home, Settings } from 'lucide-vue-next'
import { useSettingsStore } from '@/stores/settings'

const route = useRoute()
const settings = useSettingsStore()

const items = computed(() => [
  { name: 'home', to: '/', label: settings.t('nav.home'), icon: Home },
  { name: 'files', to: '/files', label: settings.t('nav.files'), icon: FolderOpen },
  { name: 'settings', to: '/settings', label: settings.t('nav.settings'), icon: Settings },
])
</script>

<template>
  <nav class="tabbar" :aria-label="settings.t('nav.main')">
    <div class="tabbar__inner">
      <RouterLink
        v-for="item in items"
        :key="item.name"
        :to="item.to"
        class="tab"
        :class="{ 'tab--active': route.name === item.name }"
      >
        <span class="tab__icon">
          <component :is="item.icon" :size="20" :stroke-width="1.8" />
        </span>
        <span class="tab__label">{{ item.label }}</span>
        <span class="tab__indicator" />
      </RouterLink>
    </div>
  </nav>
</template>

<style scoped>
.tabbar {
  position: fixed;
  left: 0;
  right: 0;
  bottom: 0;
  z-index: 50;
  padding: 0 10px env(safe-area-inset-bottom);
  background: rgba(5, 8, 16, 0.98);
  border-top: 1px solid var(--border);
}

[data-theme='carbon'] .tabbar {
  background: linear-gradient(180deg, rgba(11, 10, 9, 0.4), rgba(11, 10, 9, 0.95) 32%);
}
[data-theme='nebula'] .tabbar {
  background: linear-gradient(180deg, rgba(8, 7, 20, 0.4), rgba(8, 7, 20, 0.95) 32%);
}

.tabbar__inner {
  display: flex;
  align-items: stretch;
  height: var(--tabbar-height);
}

.tab {
  position: relative;
  display: flex;
  flex: 1;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 4px;
  color: var(--faint);
  transition: color var(--transition);
  -webkit-tap-highlight-color: transparent;
}

.tab__icon {
  display: grid;
  place-items: center;
  width: 42px;
  height: 27px;
  border-radius: 9px;
  transition:
    background var(--transition),
    color var(--transition),
    transform var(--transition);
}

.tab--active {
  color: var(--accent);
}

.tab--active .tab__icon {
  background: var(--accent-dim);
  transform: translateY(-1px);
}

.tab__label {
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0.01em;
}

.tab__indicator {
  position: absolute;
  top: 0;
  width: 18px;
  height: 2px;
  border-radius: 0 0 3px 3px;
  background: var(--accent);
  opacity: 0;
  transform: scaleX(0.4);
  transition:
    opacity var(--transition),
    transform var(--transition);
  box-shadow: 0 0 10px var(--accent);
}

.tab--active .tab__indicator {
  opacity: 1;
  transform: scaleX(1);
}
</style>
