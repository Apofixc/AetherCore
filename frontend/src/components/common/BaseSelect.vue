<script setup lang="ts">
import { computed } from 'vue'

export interface SelectOption {
  label: string
  value: string | number
  disabled?: boolean
}

const props = withDefaults(
  defineProps<{
    modelValue: string | number
    options: (SelectOption | string)[]
    label?: string
    placeholder?: string
    disabled?: boolean
    error?: string
    hint?: string
    id?: string
    size?: 'sm' | 'md' | 'lg'
    icon?: string
  }>(),
  {
    disabled: false,
    size: 'sm'
  }
)

const emit = defineEmits<{
  (e: 'update:modelValue', val: any): void
}>()

const normalizedOptions = computed<SelectOption[]>(() => {
  return props.options.map((opt) => {
    if (typeof opt === 'object' && opt !== null && 'value' in opt) {
      return opt as SelectOption
    }
    return {
      label: String(opt),
      value: opt
    }
  })
})

function onChange(e: Event) {
  const val = (e.target as HTMLSelectElement).value
  emit('update:modelValue', val)
}
</script>

<template>
  <div class="flex flex-col gap-1 w-full">
    <!-- Label -->
    <label
      v-if="label"
      :for="id"
      class="text-[10px] font-label-caps text-on-surface-variant uppercase flex items-center justify-between"
    >
      <span>{{ label }}</span>
      <slot name="labelRight" />
    </label>

    <!-- Select Wrapper -->
    <div
      class="relative flex items-center w-full rounded-lg transition-all duration-150 border bg-surface-container-highest focus-within:ring-1 focus-within:ring-primary-fixed-dim"
      :class="[
        error
          ? 'border-error focus-within:border-error focus-within:ring-error'
          : 'border-outline-variant hover:border-outline-variant focus-within:border-primary-fixed-dim',
        disabled ? 'opacity-50 cursor-not-allowed bg-surface-container-low' : ''
      ]"
    >
      <!-- Leading Icon -->
      <span
        v-if="icon"
        class="material-symbols-outlined absolute left-2.5 text-on-surface-variant/70 pointer-events-none select-none"
        :class="size === 'sm' ? 'text-base' : 'text-lg'"
      >
        {{ icon }}
      </span>

      <!-- Native Select -->
      <select
        :id="id"
        :value="modelValue"
        :disabled="disabled"
        @change="onChange"
        class="w-full bg-transparent text-on-surface font-body-mono outline-none appearance-none cursor-pointer transition-colors"
        :class="[
          size === 'sm' ? 'h-8 py-1 text-xs' : size === 'lg' ? 'h-11 py-2 text-base' : 'h-9 py-1.5 text-xs',
          icon ? 'pl-8' : 'pl-3',
          'pr-8'
        ]"
      >
        <option
          v-if="placeholder"
          value=""
          disabled
          class="bg-surface-container text-on-surface-variant"
        >
          {{ placeholder }}
        </option>
        <option
          v-for="opt in normalizedOptions"
          :key="opt.value"
          :value="opt.value"
          :disabled="opt.disabled"
          class="bg-surface-container text-on-surface py-1"
        >
          {{ opt.label }}
        </option>
      </select>

      <!-- Dropdown Chevron Icon -->
      <span
        class="material-symbols-outlined absolute right-2 text-on-surface-variant/70 pointer-events-none select-none"
        :class="size === 'sm' ? 'text-base' : 'text-lg'"
      >
        expand_more
      </span>
    </div>

    <!-- Error Message or Hint -->
    <p v-if="error" class="text-xs text-error font-medium flex items-center gap-1 mt-0.5">
      <span class="material-symbols-outlined text-[14px]">error</span>
      <span>{{ error }}</span>
    </p>
    <p v-else-if="hint" class="text-[11px] text-on-surface-variant/70 mt-0.5">
      {{ hint }}
    </p>
  </div>
</template>
