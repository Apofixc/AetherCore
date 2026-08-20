import { ref, computed } from 'vue'

export type ThemeMode = 'dark' | 'light' | 'system'

const savedTheme = (localStorage.getItem('nms_theme') as ThemeMode) || 'dark'
const currentTheme = ref<ThemeMode>(['dark', 'light', 'system'].includes(savedTheme) ? savedTheme : 'dark')

function applyTheme(theme: ThemeMode) {
  const root = document.documentElement
  let isDark = false

  if (theme === 'system') {
    isDark = window.matchMedia && window.matchMedia('(prefers-color-scheme: dark)').matches
  } else {
    isDark = theme === 'dark'
  }

  if (isDark) {
    root.classList.add('dark')
    root.classList.remove('light')
  } else {
    root.classList.add('light')
    root.classList.remove('dark')
  }
}

// Слушатель системной темы
if (window.matchMedia) {
  window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', () => {
    if (currentTheme.value === 'system') {
      applyTheme('system')
    }
  })
}

// Применяем при загрузке
applyTheme(currentTheme.value)

export function setTheme(theme: ThemeMode) {
  currentTheme.value = theme
  localStorage.setItem('nms_theme', theme)
  applyTheme(theme)
}

export function toggleTheme() {
  const next = currentTheme.value === 'dark' ? 'light' : 'dark'
  setTheme(next)
}

export function useTheme() {
  return {
    theme: computed(() => currentTheme.value),
    isDark: computed(() => {
      if (currentTheme.value === 'system') {
        return window.matchMedia && window.matchMedia('(prefers-color-scheme: dark)').matches
      }
      return currentTheme.value === 'dark'
    }),
    setTheme,
    toggleTheme
  }
}
