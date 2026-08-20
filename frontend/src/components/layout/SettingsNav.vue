<script setup lang="ts">
import { useRoute } from 'vue-router'
import { useI18n } from '@/i18n'

const route = useRoute()
const { t } = useI18n()

const navItems = [
  { path: '/modules', title: 'Module Management', key: 'nav.moduleManagement' },
  { path: '/settings/access-identity', title: 'Access & Identity', key: 'nav.accessIdentity' },
  { path: '/users', title: 'Users Management', key: 'nav.usersManagement' },
  { path: '/system', title: 'System Administration', key: 'nav.systemAdmin' },
  { path: '/profile', title: 'User Profile', key: 'nav.userProfile' }
]

function isActive(path: string) {
  if (path === '/settings/access-identity' && (route.path === '/settings/access-identity' || route.path === '/settings/access')) return true
  return route.path === path
}
</script>

<template>
  <nav class="sticky top-0 right-0 z-30 bg-surface-container-lowest border-b border-outline-variant px-lg w-full select-none">
    <div class="flex items-center gap-lg overflow-x-auto">
      <router-link
        v-for="item in navItems"
        :key="item.path"
        :to="item.path"
        class="py-4 px-2 border-b-2 text-sm transition-all duration-200 shrink-0"
        :class="isActive(item.path)
          ? 'border-primary-fixed-dim text-primary-fixed-dim font-bold hover:text-primary'
          : 'border-transparent text-on-surface-variant hover:text-on-surface hover:border-outline-variant'"
      >
        {{ item.title }}
      </router-link>
    </div>
  </nav>
</template>
