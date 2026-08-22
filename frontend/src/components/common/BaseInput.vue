<script setup lang="ts">
import { ref } from 'vue'
import { useI18n } from '@/i18n'

const { t } = useI18n()

const props = withDefaults(
  defineProps<{
    modelValue: string | number
    label?: string
    placeholder?: string
    type?: string
    icon?: string
    clearable?: boolean
    disabled?: boolean
    readonly?: boolean
    required?: boolean
    error?: string
    hint?: string
    id?: string
    size?: 'sm' | 'md' | 'lg'
    autocomplete?: string
  }>(),
  {
    type: 'text',
    clearable: false,
    disabled: false,
    readonly: false,
    required: false,
    size: 'sm',
    autocomplete: 'off'
  }
)

const emit = defineEmits<{
  (e: 'update:modelValue', val: string): void
  (e: 'clear'): void
  (e: 'focus', event: FocusEvent): void
  (e: 'blur', event: FocusEvent): void
}>()

const inputRef = ref<HTMLInputElement | null>(null)
const isPasswordVisible = ref(false)

function onInput(e: Event) {
  emit('update:modelValue', (e.target as HTMLInputElement).value)
}

function handleClear() {
  emit('update:modelValue', '')
  emit('clear')
  inputRef.value?.focus()
}

function togglePassword() {
  isPasswordVisible.value = !isPasswordVisible.value
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
      <span>
        {{ label }}
        <span v-if="required" class="text-error ml-0.5">*</span>
      </span>
      <slot name="labelRight" />
    </label>

    <!-- Input Wrapper -->
    <div
      class="relative flex items-center w-full rounded-lg transition-all duration-150 border bg-surface-container-highest focus-within:ring-1 focus-within:ring-primary-fixed-dim"
      :class="[
        error
          ? 'border-error focus-within:border-error focus-within:ring-error'
          : 'border-outline-variant hover:border-outline-variant focus-within:border-primary-fixed-dim',
        disabled || readonly ? 'opacity-70 bg-surface-variant/30 border-outline-variant/50 cursor-not-allowed' : ''
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

      <!-- Input Field -->
      <input
        ref="inputRef"
        :id="id"
        :value="modelValue"
        :type="type === 'password' ? (isPasswordVisible ? 'text' : 'password') : type"
        :placeholder="placeholder"
        :disabled="disabled"
        :readonly="readonly"
        :required="required"
        :autocomplete="autocomplete"
        @input="onInput"
        @focus="(e) => emit('focus', e)"
        @blur="(e) => emit('blur', e)"
        class="w-full bg-transparent text-on-surface font-body-mono placeholder:text-on-surface-variant/50 outline-none transition-colors"
        :class="[
          size === 'sm' ? 'h-8 py-1.5 text-xs' : size === 'lg' ? 'h-11 py-2.5 text-base' : 'h-9 py-2 text-xs',
          icon ? 'pl-8' : 'pl-3',
          (clearable && modelValue) || type === 'password' ? 'pr-8' : 'pr-3',
          disabled || readonly ? 'cursor-not-allowed text-on-surface-variant' : ''
        ]"
      />

      <!-- Password Visibility Toggle -->
      <button
        v-if="type === 'password' && !disabled"
        type="button"
        @click="togglePassword"
        class="absolute right-2 w-6 h-6 rounded flex items-center justify-center text-on-surface-variant/70 hover:text-on-surface hover:bg-surface-container transition-colors cursor-pointer"
        :title="isPasswordVisible ? t('common.hidePassword') : t('common.showPassword')"
      >
        <span class="material-symbols-outlined text-base">
          {{ isPasswordVisible ? 'visibility_off' : 'visibility' }}
        </span>
      </button>

      <!-- Clear Button -->
      <button
        v-else-if="clearable && modelValue && !disabled && !readonly"
        type="button"
        @click="handleClear"
        class="absolute right-2 w-5 h-5 rounded flex items-center justify-center text-on-surface-variant/70 hover:text-on-surface hover:bg-surface-container transition-colors cursor-pointer"
        :title="t('common.clear')"
      >
        <span class="material-symbols-outlined text-[14px]">close</span>
      </button>
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
