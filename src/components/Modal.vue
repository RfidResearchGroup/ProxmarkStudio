<script setup lang="ts">
import { onBeforeUnmount, watch } from 'vue'
import { X } from 'lucide-vue-next'

const props = withDefaults(
  defineProps<{
    open: boolean
    title?: string
    subtitle?: string
    wide?: boolean
    full?: boolean
    flush?: boolean
    closeOnBackdrop?: boolean
    closeOnEscape?: boolean
  }>(),
  {
    title: '',
    subtitle: '',
    wide: false,
    full: false,
    flush: false,
    closeOnBackdrop: true,
    closeOnEscape: true,
  },
)

const emit = defineEmits<{
  close: []
}>()

let previousOverflow = ''

function close() {
  emit('close')
}

function onBackdrop(event: MouseEvent) {
  if (!props.closeOnBackdrop) return
  if (event.target === event.currentTarget) close()
}

function onKeydown(event: KeyboardEvent) {
  if (event.key === 'Escape' && props.open && props.closeOnEscape) close()
}

watch(
  () => props.open,
  (open) => {
    if (typeof document === 'undefined') return
    if (open) {
      previousOverflow = document.body.style.overflow
      document.body.style.overflow = 'hidden'
      window.addEventListener('keydown', onKeydown)
    } else {
      document.body.style.overflow = previousOverflow
      window.removeEventListener('keydown', onKeydown)
    }
  },
  { immediate: true },
)

onBeforeUnmount(() => {
  if (typeof document !== 'undefined') document.body.style.overflow = previousOverflow
  window.removeEventListener('keydown', onKeydown)
})
</script>

<template>
  <Teleport to="body">
    <Transition name="fade">
      <div v-if="open" class="modal-backdrop" @mousedown="onBackdrop">
        <section
          class="modal"
          :class="{ 'modal--wide': wide, 'modal--full': full }"
          role="dialog"
          aria-modal="true"
          :aria-label="title"
          @mousedown.stop
        >
          <header v-if="title || $slots.head" class="modal__head">
            <slot name="head">
              <div>
                <h2 class="modal__title">{{ title }}</h2>
                <p v-if="subtitle" class="modal__subtitle">{{ subtitle }}</p>
              </div>
            </slot>
            <button class="icon-btn" aria-label="Close" @click="close">
              <X :size="18" />
            </button>
          </header>
          <div class="modal__body" :class="{ 'modal__body--flush': flush }">
            <slot />
          </div>
          <footer v-if="$slots.foot" class="modal__foot">
            <slot name="foot" />
          </footer>
        </section>
      </div>
    </Transition>
  </Teleport>
</template>
