<script setup lang="ts">
import { ref } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n, type Locale } from '@/i18n'
import { useTheme } from '@/theme'
import { useAuthStore } from '@/stores/auth'

const { locale, setLocale, t } = useI18n()
const { theme, isDark, toggleTheme } = useTheme()
const authStore = useAuthStore()
const router = useRouter()

const userMenuOpen = ref(false)

defineEmits(['toggleSidebar'])

function handleLocaleChange(newLocale: Locale) {
  setLocale(newLocale)
}

function handleLogout() {
  authStore.logout()
  router.push('/login')
}
</script>

<template>
  <header
    class="bg-surface-dim/80 backdrop-blur-sm text-primary font-title-sm text-title-sm h-16 sticky top-0 z-40 border-b border-outline-variant flex items-center px-lg justify-between w-full shrink-0 select-none"
  >
    <!-- Left: Menu Toggle -->
    <div class="flex items-center gap-md">
      <button
        type="button"
        class="p-sm text-on-surface-variant hover:text-primary transition-colors cursor-pointer active:opacity-70 rounded-full hover:bg-surface-variant/50 flex items-center justify-center"
        @click="$emit('toggleSidebar')"
        title="Toggle Menu"
      >
        <span class="material-symbols-outlined">menu</span>
      </button>
    </div>

    <!-- Right: Search & Actions -->
    <div class="flex items-center gap-lg">
      <!-- Search placeholder for future layout alignment -->
      <div class="relative w-64 hidden lg:block"></div>

      <!-- Actions -->
      <div class="flex items-center gap-sm text-on-surface-variant">
        <!-- Notifications -->
        <button
          type="button"
          class="p-sm hover:text-primary transition-colors cursor-pointer active:opacity-70 rounded-full hover:bg-surface-variant/50 flex items-center justify-center relative"
          title="Notifications"
        >
          <span class="material-symbols-outlined" data-icon="notifications_active">notifications_active</span>
          <span class="absolute top-2 right-2 w-2 h-2 rounded-full bg-primary glow-primary"></span>
        </button>

        <!-- User Profile Header Element -->
        <div class="relative">
          <div
            class="flex items-center gap-md ml-sm pl-sm border-l border-outline-variant cursor-pointer hover:bg-surface-variant/30 px-sm py-1 rounded-lg transition-all group"
            @click="userMenuOpen = !userMenuOpen"
          >
            <div class="flex flex-col items-end hidden lg:flex">
              <span class="text-sm font-bold text-on-surface leading-none font-title-sm tracking-tight">
                {{ authStore.user?.full_name || t('auth.adminUser') }}
              </span>
              <span class="text-[10px] text-primary font-body-mono uppercase tracking-widest opacity-80 mt-1">
                {{ authStore.isSuperuser ? t('auth.superuser') : 'OPERATOR' }}
              </span>
            </div>

            <div class="relative">
              <div class="w-10 h-10 rounded-full bg-primary/10 border-2 border-primary/30 flex items-center justify-center glow-primary group-hover:glow-primary-hover group-hover:border-primary/60 transition-all overflow-hidden">
                <span class="text-xs font-bold text-primary font-body-mono">
                  {{ (authStore.user?.username || 'AD').substring(0, 2).toUpperCase() }}
                </span>
              </div>
              <div class="absolute -bottom-0.5 -right-0.5 w-3 h-3 bg-tertiary rounded-full border-2 border-background"></div>
            </div>

            <span
              class="material-symbols-outlined text-on-surface-variant text-sm group-hover:text-primary transition-colors"
              :class="{ 'rotate-180': userMenuOpen }"
            >
              expand_more
            </span>
          </div>

          <!-- User Dropdown Menu -->
          <div
            v-if="userMenuOpen"
            class="absolute right-0 mt-2 w-56 bg-surface-container border border-outline-variant rounded-xl shadow-2xl py-2 z-50 animate-fade-in"
          >
            <div class="px-4 py-2 border-b border-outline-variant/40">
              <p class="text-xs font-bold text-on-surface">{{ authStore.user?.full_name || t('auth.adminUser') }}</p>
              <p class="text-[11px] text-on-surface-variant font-body-mono">{{ authStore.user?.email || 'admin@nms.local' }}</p>
            </div>

            <!-- Language Switch in dropdown -->
            <div class="px-4 py-2 border-b border-outline-variant/40 flex items-center justify-between text-xs">
              <span class="text-on-surface-variant font-label-caps uppercase">{{ t('profile.language') }}</span>
              <div class="flex items-center bg-surface-dim rounded p-0.5 border border-outline-variant/40">
                <button
                  type="button"
                  class="px-2 py-0.5 font-bold font-body-mono rounded transition-colors"
                  :class="locale === 'ru' ? 'bg-primary-fixed-dim text-on-primary-fixed' : 'text-on-surface-variant hover:text-on-surface'"
                  @click="handleLocaleChange('ru')"
                >
                  RU
                </button>
                <button
                  type="button"
                  class="px-2 py-0.5 font-bold font-body-mono rounded transition-colors"
                  :class="locale === 'en' ? 'bg-primary-fixed-dim text-on-primary-fixed' : 'text-on-surface-variant hover:text-on-surface'"
                  @click="handleLocaleChange('en')"
                >
                  EN
                </button>
              </div>
            </div>

            <!-- Theme Switch in dropdown -->
            <div class="px-4 py-2 border-b border-outline-variant/40 flex items-center justify-between text-xs">
              <span class="text-on-surface-variant font-label-caps uppercase">{{ t('profile.theme') }}</span>
              <button
                type="button"
                class="flex items-center gap-1 text-xs font-bold font-body-mono bg-surface-dim px-2 py-0.5 rounded border border-outline-variant/40 text-on-surface hover:text-primary transition-colors"
                @click="toggleTheme"
              >
                <span class="material-symbols-outlined text-[14px]">{{ isDark ? 'dark_mode' : 'light_mode' }}</span>
                <span>{{ isDark ? 'Dark' : 'Light' }}</span>
              </button>
            </div>

            <button
              type="button"
              class="w-full text-left px-4 py-2 text-sm text-on-surface hover:bg-surface-variant flex items-center gap-2 cursor-pointer transition-colors"
              @click="userMenuOpen = false; router.push('/profile')"
            >
              <span class="material-symbols-outlined text-sm">person</span>
              {{ t('nav.userProfile') }}
            </button>
            <button
              type="button"
              class="w-full text-left px-4 py-2 text-sm text-on-surface hover:bg-surface-variant flex items-center gap-2 cursor-pointer transition-colors"
              @click="userMenuOpen = false; router.push('/modules')"
            >
              <span class="material-symbols-outlined text-sm">settings</span>
              {{ t('nav.settings') }}
            </button>
            <div class="border-t border-outline-variant/40 my-1"></div>
            <button
              type="button"
              class="w-full text-left px-4 py-2 text-sm text-error hover:bg-error-container/20 flex items-center gap-2 cursor-pointer transition-colors"
              @click="userMenuOpen = false; handleLogout()"
            >
              <span class="material-symbols-outlined text-sm">logout</span>
              {{ t('auth.logout') }}
            </button>
          </div>
        </div>
      </div>
    </div>
  </header>
</template>
