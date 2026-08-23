import { ref } from 'vue'

export type ToastType = 'success' | 'error' | 'warning' | 'info'

export interface ToastItem {
  id: string
  message: string
  title?: string
  type: ToastType
  duration: number
  timer?: ReturnType<typeof setTimeout>
}

export interface ToastOptions {
  title?: string
  duration?: number
}

const DEFAULT_DURATION = 4000
const MAX_TOASTS = 5

// Глобальное реактивное состояние тостов
const toasts = ref<ToastItem[]>([])

let toastCounter = 0

function removeToast(id: string) {
  const index = toasts.value.findIndex(t => t.id === id)
  if (index !== -1) {
    const item = toasts.value[index]
    if (item.timer) {
      clearTimeout(item.timer)
    }
    toasts.value.splice(index, 1)
  }
}

function addToast(
  message: string,
  type: ToastType = 'info',
  options?: ToastOptions
): string {
  const id = `toast-${Date.now()}-${++toastCounter}`
  const duration = options?.duration !== undefined ? options.duration : DEFAULT_DURATION

  let timer: ReturnType<typeof setTimeout> | undefined

  if (duration > 0) {
    timer = setTimeout(() => {
      removeToast(id)
    }, duration)
  }

  const toastItem: ToastItem = {
    id,
    message,
    title: options?.title,
    type,
    duration,
    timer
  }

  // Ограничиваем максимальное количество тостов
  if (toasts.value.length >= MAX_TOASTS) {
    const oldest = toasts.value.shift()
    if (oldest?.timer) {
      clearTimeout(oldest.timer)
    }
  }

  toasts.value.push(toastItem)
  return id
}

export function useToast() {
  return {
    toasts,
    show: addToast,
    success: (message: string, options?: ToastOptions | string) => {
      const opts = typeof options === 'string' ? { title: options } : options
      return addToast(message, 'success', opts)
    },
    error: (message: string, options?: ToastOptions | string) => {
      const opts = typeof options === 'string' ? { title: options } : options
      return addToast(message, 'error', opts)
    },
    warning: (message: string, options?: ToastOptions | string) => {
      const opts = typeof options === 'string' ? { title: options } : options
      return addToast(message, 'warning', opts)
    },
    info: (message: string, options?: ToastOptions | string) => {
      const opts = typeof options === 'string' ? { title: options } : options
      return addToast(message, 'info', opts)
    },
    remove: removeToast,
    clear: () => {
      toasts.value.forEach(t => {
        if (t.timer) clearTimeout(t.timer)
      })
      toasts.value = []
    }
  }
}
