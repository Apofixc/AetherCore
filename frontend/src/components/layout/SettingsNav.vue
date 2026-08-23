<script setup lang="ts">
import { computed } from 'vue'
import { useRoute } from 'vue-router'
import { useI18n } from '@/i18n'
import { useAuthStore } from '@/stores/auth'

const route = useRoute()
const { t } = useI18n()
const authStore = useAuthStore()

const visibleNavItems = computed(() => {
  const items = []
  if (authStore.canViewModules) {
    items.push({ path: '/settings/modules', key: 'nav.moduleManagement' })
  }
  if (authStore.canViewAccess) {
    items.push({ path: '/settings/access-identity', key: 'nav.accessIdentity' })
  }
  if (authStore.canViewUsers) {
    items.push({ path: '/settings/users', key: 'nav.usersManagement' })
  }
  if (authStore.canViewSystem) {
    items.push({ path: '/settings/system', key: 'nav.systemAdmin' })
  }
  // Профиль доступен любому аутентифицированному пользователю
  items.push({ path: '/settings/profile', key: 'nav.userProfile' })
  return items
})

function isActive(path: string) {
  if (path === '/settings/modules') return route.path === '/settings/modules' || route.path === '/modules'
  if (path === '/settings/access-identity') return route.path === '/settings/access-identity' || route.path === '/settings/access'
  if (path === '/settings/users') return route.path === '/settings/users' || route.path === '/users'
  if (path === '/settings/system') return route.path === '/settings/system' || route.path === '/system'
  if (path === '/settings/profile') return route.path === '/settings/profile' || route.path === '/profile'
  return route.path === path
}
</script>

<template>
  <nav class="sticky top-0 right-0 z-30 bg-surface-container-lowest border-b border-outline-variant px-lg w-full select-none">
    <div class="flex items-center gap-lg overflow-x-auto">
      <router-link
        v-for="item in visibleNavItems"
        :key="item.path"
        :to="item.path"
        class="py-4 px-2 border-b-2 text-sm transition-all duration-200 shrink-0"
        :class="isActive(item.path)
          ? 'border-primary-fixed-dim text-primary-fixed-dim font-bold hover:text-primary'
          : 'border-transparent text-on-surface-variant hover:text-on-surface hover:border-outline-variant'"
      >
        {{ t(item.key) }}
      </router-link>
    </div>
  </nav>
</template>
