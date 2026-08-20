<script setup lang="ts">
import StatusBadge from './StatusBadge.vue'

withDefaults(
  defineProps<{
    title?: string
    subtitle?: string
    icon?: string
    badge?: string
    badgeVariant?: any
    noPadding?: boolean
    hoverable?: boolean
    borderGlow?: boolean
  }>(),
  {
    noPadding: false,
    hoverable: false,
    borderGlow: false
  }
)
</script>

<template>
  <div
    class="bg-surface-container-low border border-outline-variant rounded-lg flex flex-col transition-all duration-200 overflow-hidden shadow-card-dark"
    :class="[
      hoverable ? 'hover:border-primary-fixed-dim/40' : '',
      borderGlow ? 'border-primary-fixed-dim/50 shadow-glow-primary-sm' : ''
    ]"
  >
    <!-- Header -->
    <div
      v-if="title || $slots.header"
      class="flex items-center justify-between p-md border-b border-outline-variant bg-surface-container shrink-0 flex-wrap gap-md"
    >
      <slot name="header">
        <div class="flex items-center gap-sm min-w-0">
          <div
            v-if="icon"
            class="w-10 h-10 rounded-lg bg-primary-fixed-dim/10 border border-primary-fixed-dim/30 flex items-center justify-center text-primary-fixed-dim shrink-0"
          >
            <span class="material-symbols-outlined text-xl">{{ icon }}</span>
          </div>
          <div class="flex flex-col min-w-0">
            <div class="flex items-center gap-2">
              <h2 class="font-title-sm font-bold text-on-surface text-sm truncate">
                {{ title }}
              </h2>
              <StatusBadge
                v-if="badge"
                :variant="badgeVariant || 'neutral'"
                size="xs"
              >
                {{ badge }}
              </StatusBadge>
            </div>
            <p v-if="subtitle" class="text-xs text-on-surface-variant mt-0.5 truncate">
              {{ subtitle }}
            </p>
          </div>
        </div>
      </slot>

      <!-- Header Actions -->
      <div v-if="$slots.headerActions" class="flex items-center gap-2 shrink-0 ml-auto">
        <slot name="headerActions" />
      </div>
    </div>

    <!-- Body -->
    <div :class="noPadding ? '' : 'p-lg'" class="flex-1">
      <slot />
    </div>

    <!-- Footer -->
    <div
      v-if="$slots.footer"
      class="p-md bg-surface-container border-t border-outline-variant shrink-0 flex items-center justify-between flex-wrap gap-md"
    >
      <slot name="footer" />
    </div>
  </div>
</template>
