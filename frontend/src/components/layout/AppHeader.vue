<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { useRouter, useRoute } from 'vue-router'
import { useI18n, type Locale } from '@/i18n'
import { useTheme } from '@/theme'
import { useAuthStore } from '@/stores/auth'
import { getUserInitials } from '@/utils/user'

const { locale, setLocale, t } = useI18n()
const { isDark, theme, toggleTheme, setTheme } = useTheme()
const authStore = useAuthStore()
const router = useRouter()
const route = useRoute()

const userInitials = computed(() =>
  getUserInitials(authStore.user?.full_name, authStore.user?.username)
)

const userMenuOpen = ref(false)
const userDropdownRef = ref<HTMLElement | null>(null)

const props = withDefaults(
  defineProps<{
    sidebarCollapsed?: boolean
  }>(),
  {
    sidebarCollapsed: false
  }
)

defineEmits(['toggleSidebar'])

interface BreadcrumbItem {
  label: string
  to?: string
}

const breadcrumbs = computed<BreadcrumbItem[]>(() => {
  const path = route.path
  if (path === '/dashboard' || path === '/') {
    return [{ label: t('nav.dashboard') }]
  }
  if (path.startsWith('/settings') || path === '/modules' || path === '/users' || path === '/profile' || path === '/system') {
    const items: BreadcrumbItem[] = [
      { label: t('nav.settings'), to: '/settings/modules' }
    ]
    if (path.includes('/modules')) {
      items.push({ label: t('nav.moduleManagement') })
    } else if (path.includes('/access')) {
      items.push({ label: t('nav.accessIdentity') })
    } else if (path.includes('/users')) {
      items.push({ label: t('nav.usersManagement') })
    } else if (path.includes('/system')) {
      items.push({ label: t('nav.systemAdmin') })
    } else if (path.includes('/profile')) {
      items.push({ label: t('nav.userProfile') })
    }
    return items
  }
  return [{ label: t('nav.dashboard') }]
})

function handleClickOutside(e: MouseEvent) {
  if (userDropdownRef.value && !userDropdownRef.value.contains(e.target as Node)) {
    userMenuOpen.value = false
  }
}

onMounted(() => {
  window.addEventListener('click', handleClickOutside)
})

onUnmounted(() => {
  window.removeEventListener('click', handleClickOutside)
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
    <!-- Left: Menu Toggle, Brand (when sidebar collapsed) & Breadcrumbs -->
    <div class="flex items-center gap-sm">
      <!-- Toggle button with intuitive icon and title -->
      <button
        type="button"
        class="p-sm hover:text-primary-fixed-dim transition-colors cursor-pointer active:opacity-70 rounded-full hover:bg-surface-variant/50 flex items-center justify-center mr-xs"
        @click="$emit('toggleSidebar')"
        :title="props.sidebarCollapsed ? t('nav.expandSidebar') : t('nav.collapseSidebar')"
        :aria-label="props.sidebarCollapsed ? t('nav.expandSidebar') : t('nav.collapseSidebar')"
      >
        <span
          class="material-symbols-outlined transition-transform duration-200 inline-block"
          :class="{ 'scale-x-[-1]': props.sidebarCollapsed }"
        >
          menu_open
        </span>
      </button>

      <!-- App Brand when sidebar is collapsed -->
      <div
        v-if="props.sidebarCollapsed"
        class="flex items-center gap-2 cursor-pointer pr-3 border-r border-outline-variant/60 mr-1 animate-fade-in group select-none"
        @click="router.push('/dashboard')"
        title="AetherCore"
      >
        <div class="w-7 h-7 rounded-lg overflow-hidden flex items-center justify-center shrink-0 border border-outline-variant group-hover:border-primary-fixed-dim/60 transition-colors">
          <img
            alt="AetherCore Logo"
            class="w-full h-full object-cover"
            src="/logo.png"
          />
        </div>
        <span class="font-display-lg font-bold text-sm text-primary-fixed-dim tracking-wider group-hover:text-primary transition-colors">
          AetherCore
        </span>
      </div>

      <!-- Breadcrumbs Component -->
      <nav class="flex items-center gap-2 text-xs" aria-label="Breadcrumb">
        <template v-for="(crumb, idx) in breadcrumbs" :key="crumb.label">
          <span v-if="idx > 0" class="text-outline-variant/60 font-body-mono select-none">/</span>
          <router-link
            v-if="crumb.to && idx < breadcrumbs.length - 1"
            :to="crumb.to"
            class="text-on-surface-variant hover:text-primary-fixed-dim transition-colors font-medium"
          >
            {{ crumb.label }}
          </router-link>
          <span
            v-else
            class="font-bold text-on-surface"
          >
            {{ crumb.label }}
          </span>
        </template>
      </nav>
    </div>

    <!-- Right: Controls & User Profile -->
    <div class="flex items-center gap-md">
      <!-- Notifications Action -->
      <button
        type="button"
        class="p-sm text-on-surface-variant hover:text-primary-fixed-dim transition-colors cursor-pointer active:opacity-70 rounded-lg hover:bg-surface-variant/50 flex items-center justify-center"
        title="Notifications"
      >
        <span class="material-symbols-outlined text-xl" data-icon="notifications_active">notifications_active</span>
      </button>

      <!-- User Profile Dropdown -->
      <div ref="userDropdownRef" class="relative">
        <div
          class="flex items-center gap-md ml-xs pl-sm border-l border-outline-variant/60 cursor-pointer hover:bg-surface-variant/30 px-sm py-1 rounded-lg transition-all group"
          @click.stop="userMenuOpen = !userMenuOpen"
        >
          <div class="flex flex-col items-end hidden lg:flex">
            <span class="text-sm font-bold text-on-surface leading-none font-title-sm tracking-tight">
              {{ authStore.user?.full_name || t('auth.adminUser') }}
            </span>
            <span class="text-[10px] text-primary-fixed-dim font-body-mono uppercase tracking-widest opacity-80 mt-0.5">
              {{ authStore.isSuperuser ? t('auth.superuser') : 'OPERATOR' }}
            </span>
          </div>

          <div class="relative">
            <div class="w-9 h-9 rounded-xl bg-surface-variant border border-outline-variant/80 flex items-center justify-center overflow-hidden shadow-sm hover:border-primary-fixed-dim/60 transition-colors">
              <img
                v-if="authStore.avatar"
                :src="authStore.avatar"
                alt="Avatar"
                class="w-full h-full object-cover"
              />
              <span v-else class="text-xs font-bold text-on-surface font-body-mono">
                {{ userInitials }}
              </span>
            </div>
            <div class="absolute -bottom-0.5 -right-0.5 w-2.5 h-2.5 bg-emerald-500 rounded-full border-2 border-background shadow-[0_0_4px_#10b981]"></div>
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
          class="absolute right-0 mt-2 w-64 bg-surface-container-low border border-outline-variant rounded-xl shadow-card-dark py-2 z-50 animate-fade-in divide-y divide-outline-variant/30"
          @click.stop
        >
          <!-- User info -->
          <div class="px-4 py-2.5 flex items-center gap-3">
            <div class="w-10 h-10 rounded-xl bg-surface-variant border border-outline-variant/80 flex items-center justify-center overflow-hidden shrink-0 shadow-sm">
              <img
                v-if="authStore.avatar"
                :src="authStore.avatar"
                alt="Avatar"
                class="w-full h-full object-cover"
              />
              <span v-else class="text-xs font-bold text-on-surface font-body-mono">
                {{ userInitials }}
              </span>
            </div>
            <div class="overflow-hidden">
              <p class="text-xs font-bold text-on-surface truncate">{{ authStore.user?.full_name || t('auth.adminUser') }}</p>
              <p class="text-[10px] font-body-mono text-on-surface-variant mt-0.5 truncate">{{ authStore.user?.email || 'root@aethercore.local' }}</p>
            </div>
          </div>

          <!-- Navigation Links -->
          <div class="py-1">
            <router-link
              to="/profile"
              class="flex items-center gap-2.5 px-4 py-2 text-xs text-on-surface-variant hover:text-on-surface hover:bg-surface-variant/50 transition-colors"
              @click="userMenuOpen = false"
            >
              <span class="material-symbols-outlined text-base text-primary-fixed-dim">person</span>
              <span>{{ t('auth.userProfile') }}</span>
            </router-link>
          </div>

          <!-- Quick Theme & Language Toggles -->
          <div class="px-4 py-2.5 flex flex-col gap-2 bg-surface-container/40">
            <!-- Theme row -->
            <div class="flex items-center justify-between">
              <div class="flex items-center gap-2 text-xs text-on-surface-variant font-medium">
                <span class="material-symbols-outlined text-sm">palette</span>
                <span>{{ t('auth.theme') }}</span>
              </div>
              <div class="flex items-center bg-surface-container-highest rounded-lg p-0.5 border border-outline-variant/50">
                <button
                  type="button"
                  class="p-1 px-1.5 rounded transition-all cursor-pointer flex items-center justify-center"
                  :class="theme === 'dark' ? 'bg-primary-fixed-dim text-on-primary-fixed shadow-glow-primary-sm' : 'text-on-surface-variant hover:text-on-surface'"
                  @click="setTheme('dark')"
                  :title="t('auth.themeDark')"
                  :aria-label="t('auth.themeDark')"
                >
                  <span class="material-symbols-outlined text-sm">dark_mode</span>
                </button>
                <button
                  type="button"
                  class="p-1 px-1.5 rounded transition-all cursor-pointer flex items-center justify-center"
                  :class="theme === 'light' ? 'bg-primary-fixed-dim text-on-primary-fixed shadow-glow-primary-sm' : 'text-on-surface-variant hover:text-on-surface'"
                  @click="setTheme('light')"
                  :title="t('auth.themeLight')"
                  :aria-label="t('auth.themeLight')"
                >
                  <span class="material-symbols-outlined text-sm">light_mode</span>
                </button>
                <button
                  type="button"
                  class="p-1 px-1.5 rounded transition-all cursor-pointer flex items-center justify-center"
                  :class="theme === 'system' ? 'bg-primary-fixed-dim text-on-primary-fixed shadow-glow-primary-sm' : 'text-on-surface-variant hover:text-on-surface'"
                  @click="setTheme('system')"
                  :title="t('auth.themeSystem')"
                  :aria-label="t('auth.themeSystem')"
                >
                  <span class="material-symbols-outlined text-sm">desktop_windows</span>
                </button>
              </div>
            </div>

            <!-- Language row (top choices) -->
            <div class="flex items-center justify-between">
              <div class="flex items-center gap-2 text-xs text-on-surface-variant font-medium">
                <span class="material-symbols-outlined text-sm">language</span>
                <span>{{ t('auth.language') }}</span>
              </div>
              <div class="flex items-center bg-surface-container-highest rounded-lg p-0.5 border border-outline-variant/50">
                <button
                  type="button"
                  class="px-2.5 py-0.5 text-[10px] font-bold font-body-mono rounded transition-all cursor-pointer"
                  :class="locale === 'ru' ? 'bg-primary-fixed-dim text-on-primary-fixed shadow-glow-primary-sm' : 'text-on-surface-variant hover:text-on-surface'"
                  @click="setLocale('ru')"
                >
                  RU
                </button>
                <button
                  type="button"
                  class="px-2.5 py-0.5 text-[10px] font-bold font-body-mono rounded transition-all cursor-pointer"
                  :class="locale === 'en' ? 'bg-primary-fixed-dim text-on-primary-fixed shadow-glow-primary-sm' : 'text-on-surface-variant hover:text-on-surface'"
                  @click="setLocale('en')"
                >
                  EN
                </button>
              </div>
            </div>
          </div>

          <!-- Logout -->
          <div class="py-1">
            <button
              type="button"
              class="w-full flex items-center gap-2.5 px-4 py-2 text-xs text-error hover:bg-error-container/20 transition-colors text-left cursor-pointer"
              @click="handleLogout"
            >
              <span class="material-symbols-outlined text-base">logout</span>
              <span>{{ t('auth.logout') }}</span>
            </button>
          </div>
        </div>
      </div>
    </div>
  </header>
</template>
