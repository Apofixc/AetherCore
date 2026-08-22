<script setup lang="ts">
import { ref, computed } from 'vue'
import { useI18n } from '@/i18n'

const { t } = useI18n()

const props = withDefaults(
  defineProps<{
    modelValue: string
    placeholder?: string
    clearable?: boolean
    widthClass?: string
    size?: 'sm' | 'md'
    shortcut?: string
  }>(),
  {
    placeholder: '',
    clearable: true,
    widthClass: 'w-64',
    size: 'sm'
  }
)

const resolvedPlaceholder = computed(() => props.placeholder || t('common.search'))

const emit = defineEmits<{
  (e: 'update:modelValue', val: string): void
  (e: 'clear'): void
}>()

const inputRef = ref<HTMLInputElement | null>(null)

function onInput(e: Event) {
  emit('update:modelValue', (e.target as HTMLInputElement).value)
}

function handleClear() {
  emit('update:modelValue', '')
  emit('clear')
  inputRef.value?.focus()
}
</script>

<template>
  <div
    class="relative flex items-center rounded-lg border border-outline-variant/60 bg-surface-container-highest/60 hover:border-outline-variant focus-within:border-primary-fixed-dim/80 focus-within:bg-surface-container-highest focus-within:ring-1 focus-within:ring-primary-fixed-dim/30 transition-all shadow-sm"
    :class="widthClass"
  >
    <!-- Search Icon -->
    <span
      class="material-symbols-outlined text-on-surface-variant/70 absolute left-2.5 pointer-events-none select-none"
      :class="size === 'sm' ? 'text-[16px]' : 'text-[18px]'"
    >
      search
    </span>

    <!-- Input -->
    <input
      ref="inputRef"
      :value="modelValue"
      type="text"
      :placeholder="resolvedPlaceholder"
      @input="onInput"
      class="w-full bg-transparent text-on-surface placeholder:text-on-surface-variant/40 outline-none transition-colors"
      :class="[
        size === 'sm' ? 'py-1.5 pl-8 pr-7 text-xs' : 'py-2 pl-9 pr-8 text-sm',
        shortcut && !modelValue ? 'pr-12' : ''
      ]"
    />

    <!-- Shortcut Hint -->
    <span
      v-if="shortcut && !modelValue"
      class="absolute right-2 text-[10px] font-mono text-on-surface-variant/40 border border-outline-variant/40 rounded px-1 pointer-events-none select-none"
    >
      {{ shortcut }}
    </span>

    <!-- Clear Button -->
    <button
      v-if="clearable && modelValue"
      type="button"
      @click="handleClear"
      class="absolute right-1.5 w-5 h-5 rounded flex items-center justify-center text-on-surface-variant/70 hover:text-on-surface hover:bg-surface-container transition-colors cursor-pointer"
      :title="t('common.clear')"
    >
      <span class="material-symbols-outlined text-[14px]">close</span>
    </button>
  </div>
</template>
