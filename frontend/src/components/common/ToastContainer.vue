<script setup lang="ts">
import { useToast, type ToastType } from '@/composables/useToast'

const { toasts, remove } = useToast()

function getIcon(type: ToastType): string {
  switch (type) {
    case 'success':
      return 'check_circle'
    case 'error':
      return 'error'
    case 'warning':
      return 'warning'
    case 'info':
    default:
      return 'info'
  }
}

function getIconClass(type: ToastType): string {
  switch (type) {
    case 'success':
      return 'text-primary-fixed-dim'
    case 'error':
      return 'text-error'
    case 'warning':
      return 'text-amber-400'
    case 'info':
    default:
      return 'text-secondary-fixed-dim'
  }
}

function getBorderClass(type: ToastType): string {
  switch (type) {
    case 'success':
      return 'border-primary-fixed-dim/40 shadow-glow-primary-sm'
    case 'error':
      return 'border-error/50 shadow-[0_0_15px_rgba(255,180,171,0.15)]'
    case 'warning':
      return 'border-amber-500/40 shadow-[0_0_15px_rgba(234,179,8,0.15)]'
    case 'info':
    default:
      return 'border-outline-variant'
  }
}
</script>

<template>
  <div
    class="fixed bottom-12 right-6 z-[100] flex flex-col gap-2.5 max-w-sm w-full pointer-events-none"
    aria-live="polite"
  >
    <TransitionGroup name="toast-list">
      <div
        v-for="toast in toasts"
        :key="toast.id"
        class="pointer-events-auto flex items-start gap-3 p-3.5 rounded-xl bg-surface-container-high/95 backdrop-blur-md border text-on-surface text-xs transition-all duration-300 select-none shadow-lg"
        :class="getBorderClass(toast.type)"
        role="alert"
      >
        <!-- Type Icon -->
        <span
          class="material-symbols-outlined text-lg shrink-0 mt-0.5"
          :class="getIconClass(toast.type)"
        >
          {{ getIcon(toast.type) }}
        </span>

        <!-- Content -->
        <div class="flex-1 min-w-0 flex flex-col gap-0.5">
          <div v-if="toast.title" class="font-bold text-xs leading-snug">
            {{ toast.title }}
          </div>
          <div class="text-xs leading-relaxed opacity-90 break-words font-mono">
            {{ toast.message }}
          </div>
        </div>

        <!-- Close Button -->
        <button
          type="button"
          class="shrink-0 text-on-surface-variant hover:text-on-surface p-0.5 rounded-md hover:bg-surface-container-highest transition-colors opacity-70 hover:opacity-100"
          aria-label="Close"
          @click="remove(toast.id)"
        >
          <span class="material-symbols-outlined text-base leading-none">close</span>
        </button>
      </div>
    </TransitionGroup>
  </div>
</template>

<style scoped>
.toast-list-enter-active,
.toast-list-leave-active {
  transition: all 0.3s cubic-bezier(0.16, 1, 0.3, 1);
}

.toast-list-enter-from {
  opacity: 0;
  transform: translateY(16px) scale(0.95);
}

.toast-list-leave-to {
  opacity: 0;
  transform: translateX(30px) scale(0.95);
}

.toast-list-move {
  transition: transform 0.3s ease;
}
</style>
