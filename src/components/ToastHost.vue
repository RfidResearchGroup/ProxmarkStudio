<script setup lang="ts">
import { CircleAlert, CircleCheck, Info, TriangleAlert, X } from 'lucide-vue-next'
import { useToastStore } from '@/stores/toast'

const toast = useToastStore()

const icons = {
  success: CircleCheck,
  error: CircleAlert,
  warning: TriangleAlert,
  info: Info,
}
</script>

<template>
  <Teleport to="body">
    <div class="toast-host" aria-live="polite" aria-atomic="false">
      <TransitionGroup name="toast">
        <div
          v-for="item in toast.items"
          :key="item.id"
          class="toast"
          :class="`toast--${item.type}`"
        >
          <span class="toast__icon">
            <component :is="icons[item.type]" :size="17" />
          </span>
          <div class="toast__content">
            <div class="toast__title">{{ item.title }}</div>
            <div v-if="item.message" class="toast__message">{{ item.message }}</div>
          </div>
          <button class="toast__close" aria-label="Close" @click="toast.dismiss(item.id)">
            <X :size="14" />
          </button>
        </div>
      </TransitionGroup>
    </div>
  </Teleport>
</template>

<style scoped>
.toast-leave-active {
  transition: opacity 180ms ease, transform 180ms ease;
  position: absolute;
  width: 100%;
}

.toast-leave-to {
  opacity: 0;
  transform: translateX(16px) scale(0.98);
}
</style>
