<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { useRoute } from 'vue-router'
import Sidebar from '@/components/Sidebar.vue'
import MobileHeader from '@/components/MobileHeader.vue'
import TabBar from '@/components/TabBar.vue'

const route = useRoute()
const isMobile = ref(false)
let media: MediaQueryList | null = null

function update() {
  isMobile.value = media?.matches ?? false
}

onMounted(() => {
  media = window.matchMedia('(max-width: 860px)')
  update()
  media.addEventListener('change', update)
})

onUnmounted(() => {
  media?.removeEventListener('change', update)
})

const isTerminal = computed(() => route.name === 'terminal')
</script>

<template>
  <div class="shell" :class="{ 'shell--mobile': isMobile, 'shell--terminal': isTerminal }">
    <Sidebar v-if="!isMobile && !isTerminal" />
    <div class="shell__main">
      <MobileHeader v-if="isMobile && !isTerminal" />
      <main class="shell__content">
        <RouterView v-slot="{ Component }">
          <Transition name="page" mode="out-in">
            <component :is="Component" />
          </Transition>
        </RouterView>
      </main>
      <TabBar v-if="isMobile && !isTerminal" />
    </div>
  </div>
</template>

<style scoped>
.shell {
  display: flex;
  min-height: 100vh;
  min-height: 100dvh;
}

.shell__main {
  position: relative;
  display: flex;
  flex-direction: column;
  flex: 1;
  min-width: 0;
  min-height: 100vh;
  min-height: 100dvh;
}

.shell__content {
  flex: 1;
  min-width: 0;
}

.shell--mobile .shell__content {
  padding-top: var(--header-height);
}

.shell--terminal {
  background: var(--bg-deep);
}

.shell--terminal .shell__content {
  padding-top: 0;
}

@media (max-width: 860px) {
  .shell--mobile .shell__content {
    padding-top: calc(var(--header-height) + env(safe-area-inset-top));
  }
}
</style>
