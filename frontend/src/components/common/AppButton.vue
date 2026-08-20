<script setup lang="ts">
import { computed } from 'vue'

const props = withDefaults(
  defineProps<{
    variant?: 'primary' | 'secondary' | 'outline' | 'danger' | 'ghost' | 'surface' | 'tertiary'
    size?: 'xs' | 'sm' | 'md' | 'lg'
    icon?: string
    iconRight?: string
    loading?: boolean
    disabled?: boolean
    type?: 'button' | 'submit' | 'reset'
    block?: boolean
    title?: string
    ariaLabel?: string
    uppercase?: boolean
  }>(),
  {
    variant: 'primary',
    size: 'sm',
    loading: false,
    disabled: false,
    type: 'button',
    block: false,
    uppercase: true
  }
)

const emit = defineEmits<{
  (e: 'click', event: MouseEvent): void
}>()

function handleClick(e: MouseEvent) {
  if (!props.disabled && !props.loading) {
    emit('click', e)
  }
}

const sizeClasses = computed(() => {
  switch (props.size) {
    case 'xs':
      return 'h-7 px-2.5 py-1 text-[11px] gap-1 rounded-lg'
    case 'lg':
      return 'h-10 px-5 py-2 text-sm gap-2 rounded-lg'
    case 'md':
      return 'h-9 px-4 py-2 text-xs gap-1.5 rounded-lg'
    case 'sm':
    default:
      return 'h-8 px-3.5 py-1.5 text-xs gap-1.5 rounded-lg'
  }
})

const iconSizes = computed(() => {
  switch (props.size) {
    case 'xs':
      return 'text-[14px]'
    case 'lg':
      return 'text-[20px]'
    case 'sm':
    case 'md':
    default:
      return 'text-[18px]'
  }
})

const variantClasses = computed(() => {
  switch (props.variant) {
    case 'primary':
      return 'bg-primary-fixed-dim text-on-primary-fixed border border-primary-fixed-dim hover:bg-primary-fixed-dim/90 shadow-glow-primary-sm hover:shadow-glow-primary-md active:scale-95'
    case 'secondary':
      return 'bg-secondary-container text-on-secondary-container hover:bg-secondary-container/80 active:scale-95'
    case 'tertiary':
      return 'bg-tertiary-fixed-dim text-on-tertiary-fixed border border-tertiary-fixed-dim hover:bg-tertiary-fixed-dim/90 shadow-glow-tertiary-sm active:scale-95'
    case 'danger':
      return 'bg-error-container/20 text-error hover:bg-error-container/40 border border-error/40 active:scale-95'
    case 'outline':
      return 'bg-surface-container-high hover:bg-surface-variant text-on-surface border border-outline-variant hover:border-primary-fixed-dim/40 active:scale-95'
    case 'surface':
      return 'bg-surface-container-highest hover:bg-surface-variant text-on-surface border border-outline-variant active:scale-95'
    case 'ghost':
      return 'bg-transparent hover:bg-surface-variant/50 text-on-surface-variant hover:text-on-surface active:scale-95'
    default:
      return 'bg-primary-fixed-dim text-on-primary-fixed border border-primary-fixed-dim'
  }
})
</script>

<template>
  <button
    :type="type"
    :disabled="disabled || loading"
    :title="title"
    :aria-label="ariaLabel || title"
    @click="handleClick"
    class="inline-flex items-center justify-center font-bold font-body-base transition-all duration-200 cursor-pointer select-none disabled:opacity-40 disabled:cursor-not-allowed disabled:pointer-events-none"
    :class="[
      sizeClasses,
      variantClasses,
      uppercase ? 'uppercase tracking-wider' : '',
      block ? 'w-full' : ''
    ]"
  >
    <!-- Loading Spinner -->
    <span
      v-if="loading"
      class="material-symbols-outlined animate-spin shrink-0"
      :class="iconSizes"
    >
      refresh
    </span>

    <!-- Leading Icon -->
    <span
      v-else-if="icon"
      class="material-symbols-outlined shrink-0"
      :class="iconSizes"
    >
      {{ icon }}
    </span>

    <slot name="icon" v-else />

    <!-- Button Text / Main Slot -->
    <span v-if="$slots.default" class="truncate">
      <slot />
    </span>

    <!-- Trailing Icon -->
    <span
      v-if="iconRight && !loading"
      class="material-symbols-outlined shrink-0 ml-auto"
      :class="iconSizes"
    >
      {{ iconRight }}
    </span>
    <slot name="iconRight" v-else />
  </button>
</template>
