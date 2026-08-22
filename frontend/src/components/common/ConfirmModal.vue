<script setup lang="ts">
import { computed } from 'vue'
import BaseModal from './BaseModal.vue'
import AppButton from './AppButton.vue'
import { useI18n } from '@/i18n'

const { t } = useI18n()

const props = withDefaults(
  defineProps<{
    modelValue: boolean
    title?: string
    message?: string
    confirmText?: string
    cancelText?: string
    variant?: 'danger' | 'warning' | 'primary' | 'info'
    icon?: string
    loading?: boolean
    maxWidth?: string
  }>(),
  {
    title: '',
    message: '',
    confirmText: '',
    cancelText: '',
    variant: 'danger',
    loading: false,
    maxWidth: 'max-w-sm'
  }
)

const emit = defineEmits<{
  (e: 'update:modelValue', val: boolean): void
  (e: 'confirm'): void
  (e: 'cancel'): void
}>()

const iconComputed = computed(() => {
  if (props.icon) return props.icon
  switch (props.variant) {
    case 'danger':
      return 'lock'
    case 'warning':
      return 'warning'
    case 'info':
      return 'info'
    case 'primary':
    default:
      return 'check_circle'
  }
})

const iconColorClass = computed(() => {
  switch (props.variant) {
    case 'danger':
      return 'text-error'
    case 'warning':
      return 'text-amber-600 dark:text-amber-400'
    case 'info':
      return 'text-sky-600 dark:text-sky-400'
    case 'primary':
    default:
      return 'text-primary-fixed-dim'
  }
})

const confirmButtonVariant = computed(() => {
  switch (props.variant) {
    case 'danger':
      return 'danger'
    case 'warning':
      return 'secondary'
    case 'info':
    case 'primary':
    default:
      return 'primary'
  }
})

function handleConfirm() {
  emit('confirm')
}

function handleCancel() {
  emit('update:modelValue', false)
  emit('cancel')
}
</script>

<template>
  <BaseModal
    :model-value="modelValue"
    @update:model-value="(val) => emit('update:modelValue', val)"
    @close="handleCancel"
    :title="title"
    :icon="iconComputed"
    :icon-color-class="iconColorClass"
    :max-width="maxWidth"
  >
    <div class="text-xs text-on-surface leading-relaxed">
      <slot>
        <p>{{ message }}</p>
      </slot>
    </div>

    <template #footer>
      <button
        type="button"
        class="px-4 py-1.5 text-xs font-semibold rounded-lg border border-outline-variant text-on-surface-variant hover:bg-surface-variant transition-colors cursor-pointer"
        :disabled="loading"
        @click="handleCancel"
      >
        {{ cancelText || t('common.cancel') }}
      </button>
      <button
        type="button"
        class="px-4 py-1.5 text-xs font-bold rounded-lg transition-all cursor-pointer disabled:opacity-50"
        :class="variant === 'danger'
          ? 'bg-error text-on-error hover:bg-error/90'
          : 'bg-primary-fixed-dim text-on-primary-fixed hover:bg-primary-fixed-dim/90 shadow-glow-primary-sm'"
        :disabled="loading"
        @click="handleConfirm"
      >
        {{ confirmText || t('common.confirm') }}
      </button>
    </template>
  </BaseModal>
</template>
