<script setup lang="ts">
import { ref, computed } from 'vue'
import { useRouter, useRoute } from 'vue-router'
import { useI18n, type Locale } from '@/i18n'
import { useTheme } from '@/theme'
import { useAuthStore } from '@/stores/auth'

const { locale, setLocale, t } = useI18n()
const { isDark, toggleTheme } = useTheme()
const authStore = useAuthStore()
const router = useRouter()
const route = useRoute()

const userMenuOpen = ref(false)

defineEmits(['toggleSidebar'])

const pageTitle = computed(() => {
  if (route.path === '/dashboard' || route.path === '/') return 'Dashboard'
  if (route.path === '/settings/access-identity') return 'Access & Identity'
  if (route.path === '/modules') return 'Module Management'
  if (route.path === '/users') return 'Users Management'
  if (route.path === '/system') return 'System Administration'
  if (route.path === '/profile') return 'User Profile'
  return 'AetherCore NMS'
})

function handleLogout() {
  userMenuOpen.value = false
  authStore.logout()
  router.push('/login')
}
</script>

<template>
  <header
    class="bg-surface-container-lowest backdrop-blur-sm text-primary font-title-sm text-title-sm h-16 sticky top-0 z-40 border-b border-outline-variant flex items-center px-lg justify-between w-full shrink-0 select-none"
  >
    <!-- Left: Menu Toggle & Title -->
    <div class="flex items-center gap-md">
      <button
        type="button"
        class="p-sm text-on-surface-variant hover:text-primary-fixed-dim transition-colors cursor-pointer active:opacity-70 rounded-lg hover:bg-surface-variant/50 flex items-center justify-center"
        @click="$emit('toggleSidebar')"
        title="Toggle Menu"
      >
        <span class="material-symbols-outlined">menu</span>
      </button>
      <h2 class="font-title-sm text-title-sm text-on-surface">{{ pageTitle }}</h2>
    </div>

    <!-- Right: Controls & User Profile -->
    <div class="flex items-center gap-lg">
      <!-- Quick Language & Theme Controls -->
      <div class="flex items-center gap-2">
        <div class="flex items-center bg-surface-container/80 backdrop-blur-sm rounded-lg p-0.5 border border-outline-variant/60">
          <button
            type="button"
            class="px-2 py-1 text-xs font-bold rounded font-body-mono transition-all cursor-pointer"
            :class="locale === 'ru' ? 'bg-primary-fixed-dim text-on-primary-fixed shadow' : 'text-on-surface-variant hover:text-on-surface'"
            @click="setLocale('ru')"
          >
            RU
          </button>
          <button
            type="button"
            class="px-2 py-1 text-xs font-bold rounded font-body-mono transition-all cursor-pointer"
            :class="locale === 'en' ? 'bg-primary-fixed-dim text-on-primary-fixed shadow' : 'text-on-surface-variant hover:text-on-surface'"
            @click="setLocale('en')"
          >
            EN
          </button>
        </div>

        <button
          type="button"
          class="p-2 text-on-surface-variant hover:text-primary-fixed-dim transition-colors bg-surface-container/80 backdrop-blur-sm border border-outline-variant/60 rounded-lg flex items-center justify-center cursor-pointer"
          @click="toggleTheme"
          :title="isDark ? 'Switch to Light' : 'Switch to Dark'"
        >
          <span class="material-symbols-outlined text-sm">{{ isDark ? 'light_mode' : 'dark_mode' }}</span>
        </button>
      </div>

      <!-- Actions -->
      <div class="flex items-center gap-sm text-on-surface-variant">
        <button
          type="button"
          class="p-sm hover:text-primary-fixed-dim transition-colors cursor-pointer active:opacity-70 rounded-full hover:bg-surface-variant/50 flex items-center justify-center"
          title="Notifications"
        >
          <span class="material-symbols-outlined" data-icon="notifications_active">notifications_active</span>
        </button>

        <!-- User Profile Dropdown -->
        <div class="relative">
          <div
            class="flex items-center gap-md ml-sm pl-sm border-l border-outline-variant cursor-pointer hover:bg-surface-variant/30 px-sm py-1 rounded-lg transition-all group"
            @click="userMenuOpen = !userMenuOpen"
          >
            <div class="flex flex-col items-end hidden lg:flex">
              <span class="text-sm font-bold text-on-surface leading-none font-title-sm tracking-tight">
                {{ authStore.user?.full_name || 'Admin User' }}
              </span>
              <span class="text-[10px] text-primary-fixed-dim font-body-mono uppercase tracking-widest opacity-80 mt-0.5">
                {{ authStore.isSuperuser ? 'SUPERUSER' : 'OPERATOR' }}
              </span>
            </div>

            <div class="relative">
              <div class="w-10 h-10 rounded-full bg-surface-variant border border-outline-variant flex items-center justify-center overflow-hidden">
                <span class="text-xs font-bold text-on-surface font-body-mono">
                  {{ (authStore.user?.username || 'AD').substring(0, 2).toUpperCase() }}
                </span>
              </div>
              <div class="absolute -bottom-0.5 -right-0.5 w-3 h-3 bg-tertiary-fixed-dim rounded-full border-2 border-background"></div>
            </div>

            <span
              class="material-symbols-outlined text-on-surface-variant text-sm group-hover:text-primary-fixed-dim transition-colors"
              :class="{ 'rotate-180': userMenuOpen }"
            >
              expand_more
            </span>
          </div>

          <!-- Dropdown Menu -->
          <div
            v-if="userMenuOpen"
            class="absolute right-0 mt-2 w-56 bg-surface-container-low border border-outline-variant rounded-lg shadow-card-dark py-2 z-50 animate-fade-in"
          >
            <div class="px-4 py-2 border-b border-outline-variant/50">
              <p class="text-xs font-bold text-on-surface">{{ authStore.user?.full_name || 'Admin User' }}</p>
              <p class="text-[10px] font-body-mono text-on-surface-variant">{{ authStore.user?.email || 'root@nms.local' }}</p>
            </div>
            <router-link
              to="/profile"
              class="flex items-center gap-2 px-4 py-2 text-xs text-on-surface-variant hover:text-on-surface hover:bg-surface-variant/50 transition-colors"
              @click="userMenuOpen = false"
            >
              <span class="material-symbols-outlined text-sm">person</span> User Profile
            </router-link>
            <router-link
              to="/settings/access-identity"
              class="flex items-center gap-2 px-4 py-2 text-xs text-on-surface-variant hover:text-on-surface hover:bg-surface-variant/50 transition-colors"
              @click="userMenuOpen = false"
            >
              <span class="material-symbols-outlined text-sm">settings</span> System Settings
            </router-link>
            <div class="border-t border-outline-variant/50 my-1"></div>
            <button
              type="button"
              class="w-full flex items-center gap-2 px-4 py-2 text-xs text-error hover:bg-error-container/20 transition-colors text-left cursor-pointer"
              @click="handleLogout"
            >
              <span class="material-symbols-outlined text-sm">logout</span> Sign Out
            </button>
          </div>
        </div>
      </div>
    </div>
  </header>
</template>
