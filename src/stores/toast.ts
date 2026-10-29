import { defineStore } from 'pinia'

export interface Toast {
  id: number
  title: string
  message?: string
  type: 'success' | 'error' | 'info' | 'warning'
  duration: number
}

let nextId = 1

export const useToastStore = defineStore('toast', {
  state: () => ({
    items: [] as Toast[],
  }),
  actions: {
    push(toast: Omit<Toast, 'id' | 'duration'> & { duration?: number }) {
      const item: Toast = {
        ...toast,
        id: nextId++,
        duration: toast.duration ?? (toast.type === 'error' ? 6000 : 3600),
      }
      this.items.push(item)
      if (item.duration > 0) {
        window.setTimeout(() => this.dismiss(item.id), item.duration)
      }
      return item.id
    },
    success(title: string, message?: string) {
      return this.push({ title, message, type: 'success' })
    },
    error(title: string, message?: string) {
      return this.push({ title, message, type: 'error', duration: 7000 })
    },
    info(title: string, message?: string) {
      return this.push({ title, message, type: 'info' })
    },
    warning(title: string, message?: string) {
      return this.push({ title, message, type: 'warning' })
    },
    dismiss(id: number) {
      this.items = this.items.filter((item) => item.id !== id)
    },
  },
})
