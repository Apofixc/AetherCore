<script setup lang="ts">
const props = withDefaults(
  defineProps<{
    modelValue: number
    min?: number
    max?: number
    step?: number
    widthClass?: string
  }>(),
  {
    min: 0,
    max: 99999,
    step: 1,
    widthClass: 'w-24'
  }
)

const emit = defineEmits<{
  (e: 'update:modelValue', val: number): void
}>()

function increment() {
  const nextVal = Math.min(props.max, (props.modelValue || 0) + props.step)
  emit('update:modelValue', nextVal)
}

function decrement() {
  const nextVal = Math.max(props.min, (props.modelValue || 0) - props.step)
  emit('update:modelValue', nextVal)
}

function onInput(e: Event) {
  const val = Number((e.target as HTMLInputElement).value)
  if (!isNaN(val)) {
    const clamped = Math.min(props.max, Math.max(props.min, val))
    emit('update:modelValue', clamped)
  }
}
</script>

<template>
  <div
    class="inline-flex items-center bg-surface-container-highest/60 hover:bg-surface-container-highest border border-outline-variant/60 hover:border-outline-variant rounded-lg p-0.5 focus-within:border-primary-fixed-dim/60 focus-within:ring-1 focus-within:ring-primary-fixed-dim/30 transition-all shadow-sm select-none"
    :class="widthClass"
  >
    <!-- Soft Decrement Button (-) -->
    <button
      type="button"
      @click="decrement"
      class="w-6 h-6 rounded-md flex items-center justify-center text-on-surface-variant/70 hover:text-primary-fixed-dim hover:bg-primary-fixed-dim/10 active:scale-90 transition-all duration-150 cursor-pointer shrink-0 disabled:opacity-30 disabled:cursor-not-allowed"
      :disabled="modelValue <= min"
      title="Уменьшить"
    >
      <span class="material-symbols-outlined text-[14px]">remove</span>
    </button>

    <!-- Number Value Field -->
    <input
      :value="modelValue"
      type="number"
      :min="min"
      :max="max"
      :step="step"
      @input="onInput"
      class="flex-1 min-w-0 w-full px-0.5 text-center font-body-mono text-xs font-semibold text-on-surface bg-transparent border-none outline-none select-text"
    />

    <!-- Soft Increment Button (+) -->
    <button
      type="button"
      @click="increment"
      class="w-6 h-6 rounded-md flex items-center justify-center text-on-surface-variant/70 hover:text-primary-fixed-dim hover:bg-primary-fixed-dim/10 active:scale-90 transition-all duration-150 cursor-pointer shrink-0 disabled:opacity-30 disabled:cursor-not-allowed"
      :disabled="modelValue >= max"
      title="Увеличить"
    >
      <span class="material-symbols-outlined text-[14px]">add</span>
    </button>
  </div>
</template>
