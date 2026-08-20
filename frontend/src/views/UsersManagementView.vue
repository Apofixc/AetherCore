<script setup lang="ts">
import { ref, onMounted } from 'vue'
import SettingsNav from '@/components/layout/SettingsNav.vue'
import { useI18n } from '@/i18n'
import { usersApi } from '@/api/users'
import type { User } from '@/api/auth'

const { t } = useI18n()
const users = ref<User[]>([])
const loading = ref(false)

onMounted(async () => {
  loading.value = true
  try {
    users.value = await usersApi.list()
  } catch (e) {
    // Mock user for local preview
    users.value = [
      {
        id: 'ROOT-001',
        username: 'admin',
        full_name: 'Главный администратор (Root)',
        email: 'root@nms.local',
        is_active: true,
        is_superuser: true,
        roles: ['admin', 'superuser'],
        permissions: ['*'],
        created_at: '2026-08-15 12:00:00',
        last_login_at: '2026-08-17 21:00:00'
      }
    ]
  } finally {
    loading.value = false
  }
})
</script>

<template>
  <div class="flex-1 flex flex-col bg-background min-h-[calc(100vh-64px-32px)]">
    <SettingsNav />

    <main class="flex-1 main-content-scroll bg-background overflow-y-auto pb-xl relative">
      <div class="p-lg max-w-[1600px] mx-auto flex flex-col gap-lg">
        <div class="flex flex-col sm:flex-row items-start sm:items-center justify-between gap-md">
          <div>
            <h1 class="font-display-lg text-display-lg text-on-surface font-bold">{{ t('users.title') }}</h1>
            <p class="text-sm text-on-surface-variant mt-1">{{ t('users.subtitle') }}</p>
          </div>
          <button
            type="button"
            class="bg-primary-fixed-dim hover:bg-primary-fixed-dim/90 text-on-primary-fixed border border-primary-fixed-dim px-4 py-2 rounded-xl text-sm font-semibold transition-colors flex items-center gap-2 cursor-pointer"
          >
            <span class="material-symbols-outlined text-[18px]">person_add</span>
            <span>{{ t('users.addUser') }}</span>
          </button>
        </div>

        <div class="bg-surface-container border border-outline-variant rounded-xl shadow-[0_0_15px_rgba(0,0,0,0.2)] overflow-hidden">
          <div class="overflow-x-auto">
            <table class="w-full text-left border-collapse font-body-base">
              <thead class="bg-surface-variant/50 border-b border-outline-variant">
                <tr>
                  <th class="py-sm px-md font-label-caps text-label-caps text-on-surface-variant uppercase">{{ t('profile.fullName') }}</th>
                  <th class="py-sm px-md font-label-caps text-label-caps text-on-surface-variant uppercase">{{ t('users.username') }}</th>
                  <th class="py-sm px-md font-label-caps text-label-caps text-on-surface-variant uppercase">{{ t('users.roles') }}</th>
                  <th class="py-sm px-md font-label-caps text-label-caps text-on-surface-variant uppercase">{{ t('common.status') }}</th>
                  <th class="py-sm px-md font-label-caps text-label-caps text-on-surface-variant uppercase text-right">{{ t('common.actions') }}</th>
                </tr>
              </thead>
              <tbody>
                <tr
                  v-for="u in users"
                  :key="u.id"
                  class="border-b border-outline-variant/20 hover:bg-surface-container-highest/40 transition-colors"
                >
                  <td class="py-md px-md">
                    <div class="flex items-center gap-3">
                      <div class="w-8 h-8 rounded-full bg-primary-fixed-dim/20 text-primary-fixed-dim flex items-center justify-center font-bold text-xs font-body-mono border border-primary-fixed-dim/30">
                        {{ u.username.substring(0, 2).toUpperCase() }}
                      </div>
                      <div>
                        <p class="font-bold text-on-surface text-sm">{{ u.full_name }}</p>
                        <p class="text-xs text-on-surface-variant font-body-mono">{{ u.email }}</p>
                      </div>
                    </div>
                  </td>
                  <td class="py-md px-md font-body-mono text-xs text-on-surface">{{ u.username }}</td>
                  <td class="py-md px-md">
                    <div class="flex flex-wrap gap-1">
                      <span
                        v-for="r in u.roles"
                        :key="r"
                        class="px-2 py-0.5 rounded text-[10px] font-bold font-body-mono bg-primary-fixed-dim/10 text-primary-fixed-dim border border-primary-fixed-dim/30 uppercase"
                      >
                        {{ r }}
                      </span>
                    </div>
                  </td>
                  <td class="py-md px-md">
                    <span
                      class="px-2 py-0.5 rounded text-[10px] font-bold uppercase font-body-mono"
                      :class="u.is_active ? 'bg-tertiary-fixed-dim/15 text-tertiary-fixed-dim border border-tertiary-fixed-dim/30' : 'bg-surface-variant text-on-surface-variant'"
                    >
                      {{ u.is_active ? t('common.active') : t('common.disabled') }}
                    </span>
                  </td>
                  <td class="py-md px-md text-right">
                    <button
                      type="button"
                      class="p-1 text-on-surface-variant hover:text-primary-fixed-dim transition-colors cursor-pointer mr-1"
                      :title="t('common.edit')"
                    >
                      <span class="material-symbols-outlined text-sm">edit</span>
                    </button>
                    <button
                      type="button"
                      class="p-1 text-on-surface-variant hover:text-error transition-colors cursor-pointer"
                      :title="t('common.delete')"
                    >
                      <span class="material-symbols-outlined text-sm">delete</span>
                    </button>
                  </td>
                </tr>
              </tbody>
            </table>
          </div>
        </div>
      </div>
    </main>
  </div>
</template>
