<script setup lang="ts">
import { computed } from 'vue'

const props = withDefaults(
  defineProps<{
    modelValue: boolean
    label?: string
    description?: string
    disabled?: boolean
    size?: 'sm' | 'md'
    badge?: string
    icon?: string
    card?: boolean
  }>(),
  {
    disabled: false,
    size: 'md',
    card: true
  }
)

const emit = defineEmits<{
  (e: 'update:modelValue', val: boolean): void
}>()

const isCard = computed(() => props.card && Boolean(props.label || props.description || props.icon))

function toggle() {
  if (!props.disabled) {
    emit('update:modelValue', !props.modelValue)
  }
}
</script>

<template>
  <div
    v-if="isCard"
    class="p-md bg-surface-container border border-outline-variant rounded-lg flex items-start justify-between gap-4 select-none"
    :class="disabled ? 'opacity-50 cursor-not-allowed' : 'cursor-pointer'"
    @click="toggle"
  >
    <!-- Label and Description -->
    <div class="flex items-start gap-2.5">
      <span
        v-if="icon"
        class="material-symbols-outlined text-primary-fixed-dim text-lg shrink-0 mt-0.5"
      >
        {{ icon }}
      </span>

      <div class="flex flex-col gap-1">
        <div class="flex items-center gap-2">
          <h3 class="text-sm font-bold text-on-surface">
            {{ label }}
          </h3>
          <span
            v-if="badge"
            class="text-[10px] font-mono px-1.5 py-0.2 rounded bg-primary-fixed-dim/15 text-primary-fixed-dim border border-primary-fixed-dim/30"
          >
            {{ badge }}
          </span>
        </div>
        <p v-if="description" class="text-[11px] text-on-surface-variant leading-relaxed">
          {{ description }}
        </p>
      </div>
    </div>

    <!-- Exact NMS Peer Toggle -->
    <label class="relative inline-flex items-center cursor-pointer shrink-0 mt-1" @click.stop>
      <input
        class="sr-only peer"
        type="checkbox"
        :checked="modelValue"
        :disabled="disabled"
        @change="toggle"
      />
      <div class="w-10 h-5 bg-surface-container-highest rounded-full border border-outline-variant peer-checked:bg-primary-fixed-dim peer-checked:border-primary-fixed-dim transition-colors relative after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-on-surface-variant peer-checked:after:bg-on-primary peer-checked:after:translate-x-5 after:rounded-full after:h-3.5 after:w-3.5 after:transition-transform"></div>
    </label>
  </div>

  <!-- Non-card inline switch -->
  <label v-else class="relative inline-flex items-center cursor-pointer shrink-0" @click.stop>
    <input
      class="sr-only peer"
      type="checkbox"
      :checked="modelValue"
      :disabled="disabled"
      @change="toggle"
    />
    <div class="w-10 h-5 bg-surface-container-highest rounded-full border border-outline-variant peer-checked:bg-primary-fixed-dim peer-checked:border-primary-fixed-dim transition-colors relative after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-on-surface-variant peer-checked:after:bg-on-primary peer-checked:after:translate-x-5 after:rounded-full after:h-3.5 after:w-3.5 after:transition-transform"></div>
  </label>
</template>
