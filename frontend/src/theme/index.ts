import { ref, computed } from 'vue'

export type ThemeMode = 'dark' | 'light' | 'system'

const savedTheme = (typeof localStorage !== 'undefined' ? (localStorage.getItem('aether_theme') || localStorage.getItem('nms_theme')) : null) as ThemeMode | null
const currentTheme = ref<ThemeMode>(
  savedTheme && ['dark', 'light', 'system'].includes(savedTheme) ? savedTheme : 'dark'
)
const systemPrefersDark = ref(
  typeof window !== 'undefined' && window.matchMedia
    ? window.matchMedia('(prefers-color-scheme: dark)').matches
    : false
)

function applyTheme(theme: ThemeMode) {
  if (typeof document === 'undefined') return
  const root = document.documentElement
  const isDark = theme === 'system' ? systemPrefersDark.value : theme === 'dark'

  if (isDark) {
    root.classList.add('dark')
    root.classList.remove('light')
    root.setAttribute('data-theme', 'dark')
  } else {
    root.classList.add('light')
    root.classList.remove('dark')
    root.setAttribute('data-theme', 'light')
  }
}

// Слушатель изменения системной темы
if (typeof window !== 'undefined' && window.matchMedia) {
  const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)')
  const updateSystemTheme = (e: MediaQueryListEvent | MediaQueryList) => {
    systemPrefersDark.value = e.matches
    if (currentTheme.value === 'system') {
      applyTheme('system')
    }
  }

  if (mediaQuery.addEventListener) {
    mediaQuery.addEventListener('change', updateSystemTheme)
  } else {
    mediaQuery.addListener(updateSystemTheme)
  }
}

// Применяем тему при инициализации
applyTheme(currentTheme.value)

export function setTheme(theme: ThemeMode) {
  currentTheme.value = theme
  if (typeof localStorage !== 'undefined') {
    localStorage.setItem('aether_theme', theme)
  }
  applyTheme(theme)
}

export function toggleTheme() {
  const next = currentTheme.value === 'dark' ? 'light' : 'dark'
  setTheme(next)
}

export function useTheme() {
  const isDark = computed(() => {
    if (currentTheme.value === 'system') {
      return systemPrefersDark.value
    }
    return currentTheme.value === 'dark'
  })

  return {
    theme: computed(() => currentTheme.value),
    isDark,
    setTheme,
    toggleTheme
  }
}

