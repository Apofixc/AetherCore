import { ref, computed } from 'vue'
import { ru } from './locales/ru'
import { en } from './locales/en'

export type Locale = 'ru' | 'en'

const dictionaries: Record<Locale, Record<string, any>> = {
  ru,
  en
}

const savedLocale = ((localStorage.getItem('aether_locale')) as Locale) || 'ru'
const currentLocale = ref<Locale>(savedLocale in dictionaries ? savedLocale : 'ru')

/**
 * Получение переведенного текста по ключу вида 'section.key'
 */
export function t(path: string, params?: Record<string, string | number>): string {
  const keys = path.split('.')
  let val: any = dictionaries[currentLocale.value]

  for (const k of keys) {
    if (val && typeof val === 'object' && k in val) {
      val = val[k]
    } else {
      // Fallback на английский
      let fallback: any = dictionaries.en
      for (const fk of keys) {
        if (fallback && typeof fallback === 'object' && fk in fallback) {
          fallback = fallback[fk]
        } else {
          return path
        }
      }
      val = fallback
      break
    }
  }

  if (typeof val !== 'string') {
    return path
  }

  if (params) {
    return Object.entries(params).reduce((acc, [k, v]) => {
      return acc.replace(new RegExp(`\\{${k}\\}`, 'g'), String(v))
    }, val)
  }

  return val
}

export function setLocale(locale: Locale) {
  if (locale in dictionaries) {
    currentLocale.value = locale
    localStorage.setItem('aether_locale', locale)
    document.documentElement.lang = locale
  }
}

export function te(path: string): boolean {
  return t(path) !== path
}

export function useI18n() {
  return {
    locale: computed(() => currentLocale.value),
    setLocale,
    t,
    te
  }
}
