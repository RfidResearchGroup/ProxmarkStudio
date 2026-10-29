<script setup lang="ts">
import { computed, onMounted, watchEffect } from 'vue'
import AppShell from '@/components/AppShell.vue'
import DesktopTitlebar from '@/components/DesktopTitlebar.vue'
import ToastHost from '@/components/ToastHost.vue'
import { usePlatformStore } from '@/stores/platform'
import { useConnectionStore } from '@/stores/connection'
import { isTauri } from '@/services/backend'

const platform = usePlatformStore()
const connection = useConnectionStore()

// The window is frameless on desktop, so we draw our own chrome. Mobile
// platforms have no decorations to replace and keep their full height.
const frameless = computed(() => isTauri && platform.info.family === 'desktop')

watchEffect(() => {
  if (typeof document === 'undefined') return
  document.documentElement.classList.toggle('frameless', frameless.value)
})

onMounted(async () => {
  await platform.init()
  await connection.init()
})
</script>

<template>
  <DesktopTitlebar v-if="frameless" />
  <AppShell />
  <ToastHost />
</template>
