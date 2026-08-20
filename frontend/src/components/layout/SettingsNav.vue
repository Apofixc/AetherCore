<script setup lang="ts">
import { useRoute } from 'vue-router'
import { useI18n } from '@/i18n'

const route = useRoute()
const { t } = useI18n()

const navItems = [
  { path: '/modules', key: 'nav.moduleManagement' },
  { path: '/settings/access', key: 'nav.accessIdentity' },
  { path: '/users', key: 'nav.usersManagement' },
  { path: '/settings/system', key: 'nav.systemAdmin' },
  { path: '/profile', key: 'nav.userProfile' }
]

function isActive(path: string) {
  return route.path === path
}
</script>

<template>
  <nav class="sticky top-16 right-0 z-30 bg-surface-container-lowest border-b border-outline-variant px-lg w-full select-none">
    <div class="flex items-center gap-lg overflow-x-auto">
      <router-link
        v-for="item in navItems"
        :key="item.path"
        :to="item.path"
        class="py-4 px-2 border-b-2 text-sm transition-all shrink-0 font-body-base"
        :class="isActive(item.path)
          ? 'border-primary text-primary font-bold glow-primary'
          : 'border-transparent text-on-surface-variant hover:text-on-surface hover:border-outline-variant/60'"
      >
        {{ t(item.key) }}
      </router-link>
    </div>
  </nav>
</template>
