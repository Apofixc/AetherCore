<script setup lang="ts">
import { watch, onMounted, onUnmounted } from 'vue'
import { useI18n } from '@/i18n'

const { t } = useI18n()

const props = withDefaults(
  defineProps<{
    modelValue: boolean
    title?: string
    subtitle?: string
    icon?: string
    iconColorClass?: string
    maxWidth?: string
    showClose?: boolean
    closeOnEsc?: boolean
    closeOnClickOutside?: boolean
  }>(),
  {
    title: '',
    subtitle: '',
    icon: '',
    iconColorClass: 'text-primary-fixed-dim',
    maxWidth: 'max-w-md',
    showClose: true,
    closeOnEsc: true,
    closeOnClickOutside: true
  }
)

const emit = defineEmits<{
  (e: 'update:modelValue', val: boolean): void
  (e: 'close'): void
}>()

function close() {
  emit('update:modelValue', false)
  emit('close')
}

function handleBackdropClick() {
  if (props.closeOnClickOutside) {
    close()
  }
}

function handleKeydown(e: KeyboardEvent) {
  if (e.key === 'Escape' && props.modelValue && props.closeOnEsc) {
    close()
  }
}

watch(
  () => props.modelValue,
  (open) => {
    if (open) {
      document.body.style.overflow = 'hidden'
    } else {
      document.body.style.overflow = ''
    }
  }
)

onMounted(() => {
  window.addEventListener('keydown', handleKeydown)
  if (props.modelValue) {
    document.body.style.overflow = 'hidden'
  }
})

onUnmounted(() => {
  window.removeEventListener('keydown', handleKeydown)
  document.body.style.overflow = ''
})
</script>

<template>
  <Teleport to="body">
    <Transition
      enter-active-class="transition duration-150 ease-out"
      enter-from-class="opacity-0"
      enter-to-class="opacity-100"
      leave-active-class="transition duration-100 ease-in"
      leave-from-class="opacity-100"
      leave-to-class="opacity-0"
    >
      <div
        v-if="modelValue"
        class="fixed inset-0 z-50 flex items-center justify-center p-md bg-black/70 backdrop-blur-xs select-none"
        @click="handleBackdropClick"
        role="dialog"
        aria-modal="true"
      >
        <Transition
          enter-active-class="transition duration-150 ease-out"
          enter-from-class="opacity-0 scale-95"
          enter-to-class="opacity-100 scale-100"
          leave-active-class="transition duration-100 ease-in"
          leave-from-class="opacity-100 scale-100"
          leave-to-class="opacity-0 scale-95"
        >
          <div
            v-if="modelValue"
            class="bg-surface-container-low border border-outline-variant rounded-xl p-lg shadow-2xl w-full max-h-[90vh] flex flex-col gap-md overflow-hidden relative"
            :class="maxWidth"
            @click.stop
          >
            <!-- Modal Header -->
            <div
              v-if="title || $slots.header"
              class="flex items-center justify-between border-b border-outline-variant/60 pb-sm shrink-0"
            >
              <slot name="header">
                <div class="flex items-center gap-2" :class="iconColorClass">
                  <span v-if="icon" class="material-symbols-outlined text-xl">{{ icon }}</span>
                  <h3 class="text-sm font-bold text-on-surface">
                    {{ title }}
                  </h3>
                </div>
              </slot>

              <button
                v-if="showClose"
                type="button"
                @click="close"
                class="text-on-surface-variant hover:text-on-surface transition-colors cursor-pointer"
                :title="t('common.close')"
                :aria-label="t('common.close')"
              >
                <span class="material-symbols-outlined text-lg">close</span>
              </button>
            </div>

            <!-- Modal Subtitle -->
            <p v-if="subtitle" class="text-xs text-on-surface-variant -mt-1">
              {{ subtitle }}
            </p>

            <!-- Modal Body -->
            <div class="overflow-y-auto flex-1 text-xs text-on-surface">
              <slot />
            </div>

            <!-- Modal Footer -->
            <div
              v-if="$slots.footer"
              class="flex items-center justify-end gap-2 pt-sm border-t border-outline-variant/60 shrink-0"
            >
              <slot name="footer" />
            </div>
          </div>
        </Transition>
      </div>
    </Transition>
  </Teleport>
</template>
