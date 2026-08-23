<script setup lang="ts">
import { ref, computed, nextTick, onMounted, onUnmounted } from 'vue'

export interface SelectOption {
  label: string
  value: string | number
  disabled?: boolean
  icon?: string
  description?: string
}

const props = withDefaults(
  defineProps<{
    modelValue: string | number
    options: (SelectOption | string | number)[]
    label?: string
    placeholder?: string
    searchPlaceholder?: string
    disabled?: boolean
    error?: string
    hint?: string
    id?: string
    size?: 'sm' | 'md' | 'lg'
    icon?: string
    searchable?: boolean
    placement?: 'left' | 'right'
  }>(),
  {
    disabled: false,
    size: 'sm',
    searchable: false,
    placement: 'left'
  }
)

const emit = defineEmits<{
  (e: 'update:modelValue', val: any): void
}>()

const rootRef = ref<HTMLElement | null>(null)
const searchInputRef = ref<HTMLInputElement | null>(null)
const isOpen = ref(false)
const searchQuery = ref('')

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

const selectedOption = computed(() => {
  return normalizedOptions.value.find((opt) => opt.value === props.modelValue)
})

const selectedLabel = computed(() => {
  if (selectedOption.value) {
    return selectedOption.value.label
  }
  return props.placeholder || ''
})

const currentIcon = computed(() => {
  return props.icon || selectedOption.value?.icon
})

const filteredOptions = computed(() => {
  if (!props.searchable || !searchQuery.value.trim()) {
    return normalizedOptions.value
  }
  const query = searchQuery.value.trim().toLowerCase()
  return normalizedOptions.value.filter(
    (opt) =>
      opt.label.toLowerCase().includes(query) ||
      String(opt.value).toLowerCase().includes(query) ||
      String(opt.value).toLowerCase().replace(/_/g, ' ').includes(query)
  )
})

function toggleDropdown() {
  if (props.disabled) return
  isOpen.value = !isOpen.value
  if (isOpen.value) {
    searchQuery.value = ''
    if (props.searchable) {
      nextTick(() => {
        searchInputRef.value?.focus()
      })
    }
  }
}

function selectOption(val: string | number) {
  emit('update:modelValue', val)
  isOpen.value = false
  searchQuery.value = ''
}

function handleClickOutside(e: MouseEvent) {
  if (rootRef.value && !rootRef.value.contains(e.target as Node)) {
    isOpen.value = false
  }
}

function handleKeydown(e: KeyboardEvent) {
  if (e.key === 'Escape' && isOpen.value) {
    isOpen.value = false
  }
}

onMounted(() => {
  document.addEventListener('click', handleClickOutside)
  document.addEventListener('keydown', handleKeydown)
})

onUnmounted(() => {
  document.removeEventListener('click', handleClickOutside)
  document.removeEventListener('keydown', handleKeydown)
})
</script>

<template>
  <div ref="rootRef" class="flex flex-col gap-1 relative" :class="isOpen ? 'z-30' : ''">
    <!-- Label -->
    <label
      v-if="label"
      :for="id"
      class="text-[10px] font-label-caps text-on-surface-variant uppercase flex items-center justify-between"
    >
      <span>{{ label }}</span>
      <slot name="labelRight" />
    </label>

    <!-- Trigger (Slot or Default Input) -->
    <div class="relative w-full" :class="isOpen ? 'z-30' : ''">
      <slot
        name="trigger"
        :is-open="isOpen"
        :toggle="toggleDropdown"
        :selected-option="selectedOption"
        :selected-label="selectedLabel"
        :disabled="disabled"
      >
        <button
          type="button"
          :id="id"
          :disabled="disabled"
          @click="toggleDropdown"
          class="relative flex items-center justify-between w-full rounded-lg transition-all duration-150 border bg-surface-container-highest text-left outline-none font-body-mono select-none text-on-surface"
          :class="[
            isOpen ? 'ring-1 ring-primary-fixed-dim border-primary-fixed-dim' : '',
            error
              ? 'border-error focus:border-error focus:ring-1 focus:ring-error'
              : 'border-outline-variant hover:border-outline-variant focus:border-primary-fixed-dim focus:ring-1 focus:ring-primary-fixed-dim',
            disabled ? 'opacity-50 cursor-not-allowed bg-surface-container-low' : 'cursor-pointer',
            size === 'sm' ? 'h-8 py-1 text-xs' : size === 'lg' ? 'h-11 py-2 text-base' : 'h-9 py-1.5 text-xs',
            currentIcon ? 'pl-8' : 'pl-3',
            'pr-8'
          ]"
        >
          <!-- Leading Icon -->
          <span
            v-if="currentIcon"
            class="material-symbols-outlined absolute left-2.5 text-on-surface-variant/70 pointer-events-none select-none"
            :class="size === 'sm' ? 'text-base' : 'text-lg'"
          >
            {{ currentIcon }}
          </span>

          <!-- Current Value / Placeholder / Custom Slot -->
          <span
            class="truncate mr-2"
            :class="selectedOption ? 'text-on-surface' : 'text-on-surface-variant/70'"
          >
            <slot name="selected" :option="selectedOption">
              {{ selectedLabel }}
            </slot>
          </span>

          <!-- Dropdown Chevron Icon -->
          <span
            class="material-symbols-outlined absolute right-2 text-on-surface-variant/70 pointer-events-none select-none transition-transform duration-150"
            :class="[
              size === 'sm' ? 'text-base' : 'text-lg',
              isOpen ? 'rotate-180 text-primary-fixed-dim' : ''
            ]"
          >
            expand_more
          </span>
        </button>
      </slot>

      <!-- Custom Popover Menu -->
      <div
        v-if="isOpen"
        class="absolute top-full mt-1 min-w-[200px] bg-surface-container-high border border-outline-variant rounded-lg shadow-2xl z-[100] overflow-hidden flex flex-col backdrop-blur-md animate-fade-in"
        :class="[
          placement === 'right' ? 'right-0 w-52' : 'left-0 w-full'
        ]"
      >
        <!-- Search Input Box (when searchable is true) -->
        <div
          v-if="searchable"
          class="p-2 border-b border-outline-variant/40 bg-surface-container-highest/60 sticky top-0 z-10"
        >
          <div class="relative flex items-center">
            <span class="material-symbols-outlined absolute left-2 text-on-surface-variant text-base pointer-events-none select-none">
              search
            </span>
            <input
              ref="searchInputRef"
              v-model="searchQuery"
              type="text"
              :placeholder="searchPlaceholder || 'Поиск...'"
              class="w-full bg-surface-container text-on-surface text-xs rounded-md pl-7 pr-7 py-1.5 outline-none border border-outline-variant focus:border-primary-fixed-dim font-body-mono"
              @keydown.stop
            />
            <button
              v-if="searchQuery"
              type="button"
              class="absolute right-2 text-on-surface-variant hover:text-on-surface flex items-center justify-center cursor-pointer"
              @click.stop="searchQuery = ''"
            >
              <span class="material-symbols-outlined text-sm">close</span>
            </button>
          </div>
        </div>

        <!-- Scrollable Options List -->
        <div class="max-h-60 overflow-y-auto p-1 flex flex-col gap-0.5 custom-scrollbar">
          <button
            v-for="opt in filteredOptions"
            :key="opt.value"
            type="button"
            :disabled="opt.disabled"
            @click="selectOption(opt.value)"
            class="w-full text-left px-2.5 py-1.5 rounded-md text-xs font-body-mono flex items-center justify-between transition-colors cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed"
            :class="[
              opt.value === modelValue
                ? 'bg-primary-fixed-dim/20 text-primary-fixed-dim font-bold'
                : 'text-on-surface hover:bg-surface-container-highest'
            ]"
          >
            <div class="flex items-center gap-2 truncate pr-2">
              <span v-if="opt.icon" class="material-symbols-outlined text-[16px] shrink-0 text-on-surface-variant/80">
                {{ opt.icon }}
              </span>
              <slot name="option" :option="opt">
                <span class="truncate">{{ opt.label }}</span>
              </slot>
            </div>
            <span
              v-if="opt.value === modelValue"
              class="material-symbols-outlined text-sm text-primary-fixed-dim shrink-0 ml-1"
            >
              check
            </span>
          </button>

          <!-- Empty Search State -->
          <div
            v-if="filteredOptions.length === 0"
            class="py-4 px-3 text-center text-xs text-on-surface-variant font-body-mono"
          >
            Ничего не найдено
          </div>
        </div>
      </div>
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
