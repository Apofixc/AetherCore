<script setup lang="ts">
import StatusBadge from './StatusBadge.vue'

withDefaults(
  defineProps<{
    title: string
    subtitle?: string
    icon?: string
    badge?: string
    badgeVariant?: any
  }>(),
  {
    subtitle: '',
    icon: '',
    badge: ''
  }
)
</script>

<template>
  <div class="flex items-center justify-between flex-wrap gap-md w-full select-none">
    <!-- Title & Icon -->
    <div class="flex items-center gap-sm text-on-surface min-w-0">
      <slot name="icon">
        <div
          v-if="icon"
          class="w-10 h-10 rounded-lg bg-primary-fixed-dim/10 border border-primary-fixed-dim/30 flex items-center justify-center text-primary-fixed-dim shrink-0"
        >
          <span class="material-symbols-outlined text-xl">{{ icon }}</span>
        </div>
      </slot>

      <div class="flex flex-col min-w-0">
        <div class="flex items-center gap-2 flex-wrap">
          <h1 class="font-display-lg text-display-lg text-on-surface font-bold truncate">
            {{ title }}
          </h1>
          <StatusBadge
            v-if="badge"
            :variant="badgeVariant || 'primary'"
            size="sm"
          >
            {{ badge }}
          </StatusBadge>
        </div>
        <slot name="subtitle">
          <p v-if="subtitle" class="text-xs text-on-surface-variant mt-0.5">
            {{ subtitle }}
          </p>
        </slot>
      </div>
    </div>

    <!-- Page Actions Slot -->
    <div v-if="$slots.actions" class="flex items-center gap-3 flex-wrap shrink-0">
      <slot name="actions" />
    </div>
  </div>
</template>
