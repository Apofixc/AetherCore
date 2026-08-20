<script setup lang="ts">
import { computed } from 'vue'

const props = withDefaults(
  defineProps<{
    variant?:
      | 'success'
      | 'danger'
      | 'warning'
      | 'info'
      | 'neutral'
      | 'primary'
      | 'superuser'
      | 'admin'
      | 'operator'
      | 'viewer'
      | 'online'
      | 'offline'
      | 'active'
      | 'inactive'
    size?: 'xs' | 'sm' | 'md'
    dot?: boolean
    pulse?: boolean
    icon?: string
  }>(),
  {
    variant: 'neutral',
    size: 'sm',
    dot: false,
    pulse: false,
    icon: ''
  }
)

const sizeClasses = computed(() => {
  switch (props.size) {
    case 'xs':
      return 'px-1.5 py-0.5 text-[10px] gap-1'
    case 'md':
      return 'px-3 py-1 text-xs gap-1.5'
    case 'sm':
    default:
      return 'px-2 py-0.5 text-[11px] gap-1'
  }
})

const dotSizeClasses = computed(() => {
  switch (props.size) {
    case 'xs':
      return 'w-1.5 h-1.5'
    case 'md':
      return 'w-2 h-2'
    case 'sm':
    default:
      return 'w-1.5 h-1.5'
  }
})

const iconSizeClasses = computed(() => {
  switch (props.size) {
    case 'xs':
      return 'text-[12px]'
    case 'md':
      return 'text-[16px]'
    case 'sm':
    default:
      return 'text-[13px]'
  }
})

const variantClasses = computed(() => {
  switch (props.variant) {
    case 'success':
    case 'online':
    case 'active':
      return 'bg-tertiary-container/15 text-tertiary-fixed-dim border border-tertiary-fixed-dim/30'
    case 'danger':
    case 'offline':
    case 'inactive':
      return 'bg-error-container/15 text-error border border-error/30'
    case 'warning':
      return 'bg-amber-500/15 text-amber-400 border border-amber-500/30'
    case 'info':
      return 'bg-sky-500/15 text-sky-400 border border-sky-500/30'
    case 'primary':
      return 'bg-primary-fixed-dim/15 text-primary-fixed-dim border border-primary-fixed-dim/30'
    case 'superuser':
      return 'bg-purple-500/15 text-purple-300 border border-purple-500/30'
    case 'admin':
      return 'bg-cyan-500/15 text-cyan-300 border border-cyan-500/30'
    case 'operator':
      return 'bg-emerald-500/15 text-emerald-300 border border-emerald-500/30'
    case 'viewer':
      return 'bg-slate-500/15 text-slate-300 border border-slate-500/30'
    case 'neutral':
    default:
      return 'bg-surface-container-highest text-on-surface-variant border border-outline-variant/60'
  }
})

const dotColorClasses = computed(() => {
  switch (props.variant) {
    case 'success':
    case 'online':
    case 'active':
      return 'bg-tertiary-fixed-dim'
    case 'danger':
    case 'offline':
    case 'inactive':
      return 'bg-error'
    case 'warning':
      return 'bg-amber-400'
    case 'info':
      return 'bg-sky-400'
    case 'primary':
      return 'bg-primary-fixed-dim'
    case 'superuser':
      return 'bg-purple-400'
    case 'admin':
      return 'bg-cyan-400'
    case 'operator':
      return 'bg-emerald-400'
    case 'viewer':
      return 'bg-slate-400'
    case 'neutral':
    default:
      return 'bg-on-surface-variant'
  }
})
</script>

<template>
  <span
    class="inline-flex items-center font-medium font-mono rounded-full shrink-0 select-none transition-colors"
    :class="[sizeClasses, variantClasses]"
  >
    <!-- Dot Indicator -->
    <span
      v-if="dot || pulse"
      class="rounded-full shrink-0"
      :class="[
        dotSizeClasses,
        dotColorClasses,
        pulse ? 'animate-pulse' : ''
      ]"
    />

    <!-- Icon -->
    <span
      v-if="icon"
      class="material-symbols-outlined shrink-0"
      :class="iconSizeClasses"
    >
      {{ icon }}
    </span>

    <!-- Text -->
    <slot />
  </span>
</template>
