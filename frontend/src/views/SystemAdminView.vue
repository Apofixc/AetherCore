<script setup lang="ts">
import { ref, onMounted } from 'vue'
import SettingsNav from '@/components/layout/SettingsNav.vue'
import { useI18n } from '@/i18n'
import { systemApi, type SystemInfo, type AuditLogEntry } from '@/api/system'

const { t } = useI18n()
const info = ref<SystemInfo>({
  version: '1.0.4',
  uptime_seconds: 14420,
  is_dev: true,
  is_safe_mode: false
})
const logs = ref<AuditLogEntry[]>([])

onMounted(async () => {
  try {
    info.value = await systemApi.getInfo()
    logs.value = await systemApi.getAuditLogs()
  } catch (e) {
    // Default mock logs for local inspection
    logs.value = [
      {
        id: '1',
        timestamp: new Date().toISOString(),
        user_id: 'ROOT-001',
        username: 'admin',
        action: 'system.startup',
        details: 'Core engine initialized in DEV mode with WASM plugin loader'
      },
      {
        id: '2',
        timestamp: new Date(Date.now() - 3600000).toISOString(),
        user_id: 'ROOT-001',
        username: 'admin',
        action: 'auth.login',
        details: 'Operator session established via Operator ID auth'
      }
    ]
  }
})
</script>

<template>
  <div class="flex-1 flex flex-col bg-background min-h-[calc(100vh-64px-32px)]">
    <SettingsNav />

    <main class="flex-1 main-content-scroll bg-background overflow-y-auto pb-xl relative">
      <div class="p-lg max-w-[1600px] mx-auto flex flex-col gap-lg">
        <div>
          <h1 class="font-display-lg text-display-lg text-on-surface font-bold">{{ t('system.title') }}</h1>
          <p class="text-sm text-on-surface-variant mt-1">{{ t('system.subtitle') }}</p>
        </div>

        <div class="grid grid-cols-1 sm:grid-cols-3 gap-md">
          <div class="bg-surface-container border border-outline-variant p-lg rounded-xl shadow-[0_0_15px_rgba(0,0,0,0.1)]">
            <p class="text-label-caps font-label-caps text-on-surface-variant uppercase mb-1">{{ t('system.coreUptime') }}</p>
            <p class="text-2xl font-bold font-body-mono text-primary-fixed-dim">{{ Math.floor(info.uptime_seconds / 60) }} min</p>
          </div>
          <div class="bg-surface-container border border-outline-variant p-lg rounded-xl shadow-[0_0_15px_rgba(0,0,0,0.1)]">
            <p class="text-label-caps font-label-caps text-on-surface-variant uppercase mb-1">{{ t('system.devMode') }}</p>
            <p class="text-2xl font-bold font-body-mono text-tertiary-fixed-dim">{{ info.is_dev ? t('common.yes') : t('common.no') }}</p>
          </div>
          <div class="bg-surface-container border border-outline-variant p-lg rounded-xl shadow-[0_0_15px_rgba(0,0,0,0.1)]">
            <p class="text-label-caps font-label-caps text-on-surface-variant uppercase mb-1">{{ t('system.safeMode') }}</p>
            <p class="text-2xl font-bold font-body-mono text-on-surface">{{ info.is_safe_mode ? t('common.yes') : t('common.no') }}</p>
          </div>
        </div>

        <div class="bg-surface-container border border-outline-variant rounded-xl shadow-[0_0_15px_rgba(0,0,0,0.2)] p-lg">
          <h3 class="font-title-sm text-on-surface font-bold mb-md">{{ t('system.auditLogs') }}</h3>
          <div class="overflow-x-auto">
            <table class="w-full text-left border-collapse font-body-base">
              <thead class="bg-surface-variant/50 border-b border-outline-variant">
                <tr>
                  <th class="py-sm px-md font-label-caps text-label-caps text-on-surface-variant uppercase">{{ t('system.timestamp') }}</th>
                  <th class="py-sm px-md font-label-caps text-label-caps text-on-surface-variant uppercase">{{ t('system.user') }}</th>
                  <th class="py-sm px-md font-label-caps text-label-caps text-on-surface-variant uppercase">{{ t('system.action') }}</th>
                  <th class="py-sm px-md font-label-caps text-label-caps text-on-surface-variant uppercase">{{ t('system.details') }}</th>
                </tr>
              </thead>
              <tbody>
                <tr
                  v-for="l in logs"
                  :key="l.id"
                  class="border-b border-outline-variant/20 hover:bg-surface-container-highest/40 transition-colors font-body-mono text-xs"
                >
                  <td class="py-md px-md text-on-surface-variant">{{ new Date(l.timestamp).toLocaleString() }}</td>
                  <td class="py-md px-md text-primary-fixed-dim font-bold">{{ l.username }}</td>
                  <td class="py-md px-md text-on-surface">{{ l.action }}</td>
                  <td class="py-md px-md text-on-surface-variant">{{ l.details }}</td>
                </tr>
              </tbody>
            </table>
          </div>
        </div>
      </div>
    </main>
  </div>
</template>
